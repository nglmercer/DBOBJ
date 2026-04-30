use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Id {
    Integer(u64),
    String(String),
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Id::Integer(i) => write!(f, "{}", i),
            Id::String(s) => write!(f, "{}", s),
        }
    }
}

impl From<u64> for Id {
    fn from(id: u64) -> Self {
        Id::Integer(id)
    }
}

impl From<String> for Id {
    fn from(id: String) -> Self {
        Id::String(id)
    }
}

impl From<&str> for Id {
    fn from(id: &str) -> Self {
        Id::String(id.to_string())
    }
}
