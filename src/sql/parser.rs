use crate::core::{DataType, Expr, Operator, Value};
use compact_str::CompactString;
use sqlparser::ast::{
    BinaryOperator, DataType as SqlDataType, Expr as SqlExpr, Statement, Value as SqlValue,
};
use sqlparser::dialect::SQLiteDialect;
use sqlparser::parser::Parser;

pub struct SqlParser;

impl SqlParser {
    #[inline]
    pub fn parse(sql: &str) -> Result<Vec<Statement>, String> {
        let dialect = SQLiteDialect {};
        Parser::parse_sql(&dialect, sql).map_err(|e| e.to_string())
    }

    #[inline]
    pub fn map_data_type(sql_type: &SqlDataType) -> Result<DataType, String> {
        match sql_type {
            SqlDataType::Integer(_) | SqlDataType::Int(_) | SqlDataType::BigInt(_) => {
                Ok(DataType::Integer)
            }
            SqlDataType::Float(_) | SqlDataType::Double(_) | SqlDataType::Real => {
                Ok(DataType::Float)
            }
            SqlDataType::String(_)
            | SqlDataType::Text
            | SqlDataType::Varchar(_)
            | SqlDataType::Char(_) => Ok(DataType::String),
            SqlDataType::Boolean => Ok(DataType::Boolean),
            SqlDataType::Blob(_)
            | SqlDataType::Bytea
            | SqlDataType::Varbinary(_)
            | SqlDataType::Binary(_) => Ok(DataType::Blob),
            _ => Err(format!("Unsupported data type: {:?}", sql_type)),
        }
    }

    #[inline]
    pub fn map_value(sql_value: &SqlValue) -> Result<Value, String> {
        match sql_value {
            SqlValue::Number(n, _) => {
                if let Ok(i) = n.parse::<i64>() {
                    Ok(Value::Integer(i))
                } else if let Ok(f) = n.parse::<f64>() {
                    Ok(Value::Float(f))
                } else {
                    Err(format!("Invalid number: {}", n))
                }
            }
            SqlValue::SingleQuotedString(s) | SqlValue::DoubleQuotedString(s) => {
                Ok(Value::String(CompactString::from(s.as_str())))
            }
            SqlValue::Boolean(b) => Ok(Value::Boolean(*b)),
            SqlValue::Null => Ok(Value::Null),
            _ => Err(format!("Unsupported value: {:?}", sql_value)),
        }
    }

    #[inline]
    pub fn map_expr(sql_expr: &SqlExpr) -> Result<Expr, String> {
        match sql_expr {
            SqlExpr::Identifier(ident) => {
                Ok(Expr::Column(CompactString::from(ident.value.as_str())))
            }
            SqlExpr::CompoundIdentifier(parts) => {
                Ok(Expr::Column(CompactString::from(
                    parts.last().unwrap().value.as_str(),
                )))
            }
            SqlExpr::Value(val_with_span) => {
                Ok(Expr::Literal(Self::map_value(&val_with_span.value)?))
            }
            SqlExpr::BinaryOp { left, op, right } => {
                let l = Box::new(Self::map_expr(left)?);
                let r = Box::new(Self::map_expr(right)?);
                let operator = match op {
                    BinaryOperator::Eq => Operator::Eq,
                    BinaryOperator::NotEq => Operator::Neq,
                    BinaryOperator::Gt => Operator::Gt,
                    BinaryOperator::GtEq => Operator::Gte,
                    BinaryOperator::Lt => Operator::Lt,
                    BinaryOperator::LtEq => Operator::Lte,
                    BinaryOperator::And => Operator::And,
                    BinaryOperator::Or => Operator::Or,
                    _ => return Err(format!("Unsupported operator: {:?}", op)),
                };
                Ok(Expr::Binary(l, operator, r))
            }
            SqlExpr::Nested(expr) => Self::map_expr(expr),
            _ => Err(format!("Unsupported expression: {:?}", sql_expr)),
        }
    }
}
