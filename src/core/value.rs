use serde::{Deserialize, Serialize};
use compact_str::CompactString;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Null,
    Integer(i64),
    Float(f64),
    String(CompactString),
    Boolean(bool),
    Blob(Vec<u8>),
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Integer(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Float(v)
    }
}

impl From<CompactString> for Value {
    fn from(v: CompactString) -> Self {
        Value::String(v)
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::String(CompactString::from(v))
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::String(CompactString::from(v))
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Boolean(v)
    }
}

impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Self {
        Value::Blob(v)
    }
}
