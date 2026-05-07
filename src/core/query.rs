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

                match op {
                    Operator::Eq => Value::Boolean(l == r),
                    Operator::Neq => Value::Boolean(l != r),
                    Operator::Gt => Value::Boolean(l > r),
                    Operator::Gte => Value::Boolean(l >= r),
                    Operator::Lt => Value::Boolean(l < r),
                    Operator::Lte => Value::Boolean(l <= r),
                    _ => self.evaluate(&row.data, mapping), // Fallback for And/Or
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
