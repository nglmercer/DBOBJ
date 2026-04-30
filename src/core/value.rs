use compact_str::CompactString;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Null,
    Integer(i64),
    Float(f64),
    String(CompactString),
    Boolean(bool),
    Blob(Vec<u8>),
}

impl Value {
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
}

impl Eq for Value {}

impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Value::Null => 0.hash(state),
            Value::Integer(i) => {
                1.hash(state);
                i.hash(state);
            }
            Value::Float(f) => {
                2.hash(state);
                // Simple hash for floats: use bits
                f.to_bits().hash(state);
            }
            Value::String(s) => {
                3.hash(state);
                s.hash(state);
            }
            Value::Boolean(b) => {
                4.hash(state);
                b.hash(state);
            }
            Value::Blob(b) => {
                5.hash(state);
                b.hash(state);
            }
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Value {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Value::Null, Value::Null) => std::cmp::Ordering::Equal,
            (Value::Null, _) => std::cmp::Ordering::Less,
            (_, Value::Null) => std::cmp::Ordering::Greater,

            (Value::Integer(a), Value::Integer(b)) => a.cmp(b),
            (Value::Integer(_), _) => std::cmp::Ordering::Less,
            (_, Value::Integer(_)) => std::cmp::Ordering::Greater,

            (Value::Float(a), Value::Float(b)) => {
                a.partial_cmp(b).unwrap_or_else(|| {
                    // Fallback for NaN
                    if a.is_nan() && b.is_nan() {
                        std::cmp::Ordering::Equal
                    } else if a.is_nan() {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Greater
                    }
                })
            }
            (Value::Float(_), _) => std::cmp::Ordering::Less,
            (_, Value::Float(_)) => std::cmp::Ordering::Greater,

            (Value::String(a), Value::String(b)) => a.cmp(b),
            (Value::String(_), _) => std::cmp::Ordering::Less,
            (_, Value::String(_)) => std::cmp::Ordering::Greater,

            (Value::Boolean(a), Value::Boolean(b)) => a.cmp(b),
            (Value::Boolean(_), _) => std::cmp::Ordering::Less,
            (_, Value::Boolean(_)) => std::cmp::Ordering::Greater,

            (Value::Blob(a), Value::Blob(b)) => a.cmp(b),
        }
    }
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
