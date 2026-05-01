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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
                if name == "id" {
                    // This is handled by a different evaluate overload or we need the ID here
                    return Value::Null; 
                }
                if let Some(&idx) = mapping.get(name.as_str()) {
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
    pub fn is_true(&self, row: &super::table::Row, mapping: &FastHashMap<String, usize>) -> bool {
        match self {
            Expr::Column(name) if name == "id" => {
                // Special case for ID column in expressions
                return false; // Need to implement Id comparison in Expr
            }
            _ => {}
        }
        
        match self.evaluate_with_row(row, mapping) {
            Value::Boolean(b) => b,
            _ => false,
        }
    }

    pub fn evaluate_with_row(&self, row: &super::table::Row, mapping: &FastHashMap<String, usize>) -> Value {
        match self {
            Expr::Column(name) if name == "id" => row.id.to_value(),
            Expr::Binary(left, op, right) => {
                let l = left.evaluate_with_row(row, mapping);
                let r = right.evaluate_with_row(row, mapping);
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
                | (Expr::Literal(val), Expr::Column(col)) => {
                    if table.indexes.contains_key(col) {
                        return QueryPlan::IndexScan(
                            table.name.clone().into(),
                            col.clone(),
                            val.clone(),
                        );
                    }
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
