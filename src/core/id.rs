use compact_str::CompactString;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub enum Id {
    Integer(u64),
    String(CompactString),
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

impl From<CompactString> for Id {
    fn from(id: CompactString) -> Self {
        Id::String(id)
    }
}

impl From<String> for Id {
    fn from(id: String) -> Self {
        Id::String(CompactString::from(id))
    }
}

impl From<&str> for Id {
    fn from(id: &str) -> Self {
        Id::String(CompactString::from(id))
    }
}

impl Id {
    pub fn to_value(&self) -> crate::core::Value {
        match self {
            Id::Integer(i) => crate::core::Value::Integer(*i as i64),
            Id::String(s) => crate::core::Value::String(s.clone()),
        }
    }
}
