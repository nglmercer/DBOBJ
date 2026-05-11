use super::{FastHashMap, Table, Value};
use compact_str::CompactString;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryPlan {
    FullScan(CompactString, Expr),
    IndexScan(CompactString, CompactString, Value),
    /// Use an index to get candidates, then filter them with the second expression
    IndexFilteredScan(CompactString, CompactString, Value, Expr),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Operator {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
    And,
    Or,
    Like,
    Not,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Expr {
    Literal(Value),
    Column(CompactString),
    Binary(Box<Expr>, Operator, Box<Expr>),
    Not(Box<Expr>),
}

impl Expr {
    pub fn evaluate(&self, data: &[Value], mapping: &FastHashMap<String, usize>) -> Value {
        match self {
            Expr::Literal(v) => v.clone(),
            Expr::Column(name) => {
                if let Some(&idx) = mapping.get(name.as_str()) {
                    if idx == usize::MAX {
                        return Value::Null;
                    }
                    data[idx].clone()
                } else {
                    Value::Null
                }
            }
            Expr::Binary(left, op, right) => {
                let l = left.evaluate(data, mapping);
                let r = right.evaluate(data, mapping);
                match op {
                    Operator::Eq => Value::Boolean(l == r),
                    Operator::Neq => Value::Boolean(l != r),
                    Operator::And => {
                        if let (Value::Boolean(lb), Value::Boolean(rb)) = (l, r) {
                            Value::Boolean(lb && rb)
                        } else {
                            Value::Boolean(false)
                        }
                    }
                    Operator::Or => {
                        if let (Value::Boolean(lb), Value::Boolean(rb)) = (l, r) {
                            Value::Boolean(lb || rb)
                        } else {
                            Value::Boolean(false)
                        }
                    }
                    Operator::Gt => Value::Boolean(l > r),
                    Operator::Gte => Value::Boolean(l >= r),
                    Operator::Lt => Value::Boolean(l < r),
                    Operator::Lte => Value::Boolean(l <= r),
                    Operator::Like => Value::Boolean(like_match(&l, &r)),
                    Operator::Not => {
                        if let Value::Boolean(b) = l {
                            Value::Boolean(!b)
                        } else {
                            Value::Boolean(false)
                        }
                    }
                }
            }
            Expr::Not(expr) => {
                if let Value::Boolean(b) = expr.evaluate(data, mapping) {
                    Value::Boolean(!b)
                } else {
                    Value::Boolean(false)
                }
            }
        }
    }

    /// Helper to check if the expression evaluates to true
    pub fn is_true(
        &self,
        row: &super::table::Row,
        mapping: &FastHashMap<String, usize>,
        table: &crate::core::Table,
    ) -> bool {
        match self.evaluate_with_row(row, mapping, table) {
            Value::Boolean(b) => b,
            _ => {
                // FALLBACK: if evaluation isn't boolean, maybe it's a type mismatch with interned strings
                false
            }
        }
    }

    pub fn evaluate_with_row(
        &self,
        row: &super::table::Row,
        mapping: &FastHashMap<String, usize>,
        table: &crate::core::Table,
    ) -> Value {
        match self {
            Expr::Column(name) => {
                if let Some(&idx) = mapping.get(name.as_str()) {
                    if idx == usize::MAX {
                        return row.id.to_value();
                    }
                    row.data[idx].clone()
                } else if name == "id" {
                    row.id.to_value()
                } else {
                    Value::Null
                }
            }
            Expr::Binary(left, op, right) => {
                let mut l = left.evaluate_with_row(row, mapping, table);
                let mut r = right.evaluate_with_row(row, mapping, table);

                // Handle InternedString comparisons
                match (&l, &r) {
                    (Value::InternedString(id), Value::String(s)) => {
                        if let Some(other_id) = table.string_pool.get_id(s.as_str()) {
                            l = Value::InternedString(*id);
                            r = Value::InternedString(other_id);
                        } else if let Some(resolved) = table.string_pool.resolve(*id) {
                            l = Value::String(resolved);
                        }
                    }
                    (Value::String(s), Value::InternedString(id)) => {
                        if let Some(other_id) = table.string_pool.get_id(s.as_str()) {
                            l = Value::InternedString(other_id);
                            r = Value::InternedString(*id);
                        } else if let Some(resolved) = table.string_pool.resolve(*id) {
                            r = Value::String(resolved);
                        }
                    }
                    (Value::InternedString(id1), Value::InternedString(id2)) => {
                        l = Value::Integer(*id1 as i64);
                        r = Value::Integer(*id2 as i64);
                    }
                    _ => {}
                }

                // Resolve InternedString for LIKE pattern matching
                if matches!(op, Operator::Like) {
                    if let Value::InternedString(id) = &l {
                        l = table.string_pool.resolve(*id).map_or(Value::Null, Value::String);
                    }
                    if let Value::InternedString(id) = &r {
                        r = table.string_pool.resolve(*id).map_or(Value::Null, Value::String);
                    }
                }

                match op {
                    Operator::Eq => Value::Boolean(l == r),
                    Operator::Neq => Value::Boolean(l != r),
                    Operator::Gt => Value::Boolean(l > r),
                    Operator::Gte => Value::Boolean(l >= r),
                    Operator::Lt => Value::Boolean(l < r),
                    Operator::Lte => Value::Boolean(l <= r),
                    Operator::Like => Value::Boolean(like_match(&l, &r)),
                    Operator::Not => {
                        if let Value::Boolean(b) = l {
                            Value::Boolean(!b)
                        } else {
                            Value::Boolean(false)
                        }
                    }
                    Operator::And | Operator::Or => self.evaluate(&row.data, mapping),
                }
            }
            _ => self.evaluate(&row.data, mapping),
        }
    }

    /// Try to optimize the expression into a query plan
    pub fn plan(&self, table: &Table) -> QueryPlan {
        match self {
            Expr::Binary(left, Operator::Eq, right) => match (left.as_ref(), right.as_ref()) {
                (Expr::Column(col), Expr::Literal(val))
                | (Expr::Literal(val), Expr::Column(col))
                    if table.indexes.contains_key(col) =>
                {
                    return QueryPlan::IndexScan(
                        table.name.clone().into(),
                        col.clone(),
                        val.clone(),
                    );
                }
                _ => {}
            },
            Expr::Binary(left, Operator::And, right) => {
                let left_plan = left.plan(table);
                let right_plan = right.plan(table);

                match (left_plan, right_plan) {
                    (QueryPlan::IndexScan(t, c, v), _) => {
                        return QueryPlan::IndexFilteredScan(t, c, v, *right.clone());
                    }
                    (_, QueryPlan::IndexScan(t, c, v)) => {
                        return QueryPlan::IndexFilteredScan(t, c, v, *left.clone());
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        QueryPlan::FullScan(table.name.clone().into(), self.clone())
    }
}

/// SQL LIKE pattern matching.
/// `%` matches any sequence of characters (including empty).
/// `_` matches any single character.
fn like_match(value: &Value, pattern: &Value) -> bool {
    let s = match value {
        Value::String(s) => s.as_str(),
        _ => return false,
    };
    let pat = match pattern {
        Value::String(p) => p.as_str(),
        _ => return false,
    };

    let s_chars: Vec<char> = s.chars().collect();
    let pat_chars: Vec<char> = pat.chars().collect();
    let (mut si, mut pi) = (0, 0);
    let (mut match_si, mut match_pi) = (0, 0);
    let mut star = false;

    while si < s_chars.len() {
        if pi < pat_chars.len() && (pat_chars[pi] == '_' || pat_chars[pi] == s_chars[si]) {
            si += 1;
            pi += 1;
        } else if pi < pat_chars.len() && pat_chars[pi] == '%' {
            star = true;
            match_si = si;
            match_pi = pi;
            pi += 1;
        } else if star {
            si = match_si + 1;
            match_si = si;
            pi = match_pi + 1;
        } else {
            return false;
        }
    }
    while pi < pat_chars.len() && pat_chars[pi] == '%' {
        pi += 1;
    }
    pi == pat_chars.len()
}
