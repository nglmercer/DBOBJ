use compact_str::CompactString;
use serde::{Deserialize, Serialize};

use rkyv::{Archive, Serialize as RkyvSerialize, Deserialize as RkyvDeserialize};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum Value {
    Null,
    Integer(i64),
    Float(f64),
    String(CompactString),
    InternedString(u32),
    Boolean(bool),
    Blob(Vec<u8>),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct StringPool {
    #[serde(skip)]
    #[rkyv(with = rkyv::with::Skip)]
    pub(crate) interner: string_interner::StringInterner<string_interner::DefaultBackend>,
    pub(crate) strings: Vec<String>, // Surrogate for serialization/archiving
}

impl StringPool {
    pub fn prepare_for_archive(&mut self) {
        self.strings = self.interner.iter().map(|(_, s)| s.to_string()).collect();
    }

    pub fn rebuild_from_archive(&mut self) {
        self.interner = string_interner::StringInterner::default();
        for s in &self.strings {
            self.interner.get_or_intern(s);
        }
    }

    pub fn intern(&mut self, s: CompactString) -> u32 {
        use string_interner::Symbol;
        self.interner.get_or_intern(s.as_str()).to_usize() as u32
    }

    pub fn get_id(&self, s: &str) -> Option<u32> {
        use string_interner::Symbol;
        self.interner.get(s).map(|s| s.to_usize() as u32)
    }

    pub fn resolve(&self, id: u32) -> Option<CompactString> {
        use string_interner::Symbol;
        let symbol = string_interner::DefaultSymbol::try_from_usize(id as usize)?;
        self.interner.resolve(symbol).map(CompactString::from)
    }

    pub fn reserve(&mut self, _additional: usize) {
        // string-interner handles resizing
    }
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
            Value::InternedString(id) => {
                3.hash(state); // Same as String to allow comparison
                id.hash(state);
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

            (Value::InternedString(a), Value::InternedString(b)) => a.cmp(b),
            (Value::InternedString(_), _) => std::cmp::Ordering::Less,
            (_, Value::InternedString(_)) => std::cmp::Ordering::Greater,

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
