pub mod database;
pub mod id;
pub mod query;
pub mod table;
pub mod value;

pub use database::Database;
pub use id::Id;
pub use query::{Expr, Operator};
pub use table::{Row, Schema, Table};
pub use value::Value;

use serde::{Deserialize, Serialize};

pub type FastHashMap<K, V> = std::collections::HashMap<K, V, ahash::RandomState>;
pub type NoHashHashMap<K, V> = std::collections::HashMap<K, V, nohash_hasher::BuildNoHashHasher<K>>;
pub type RowData = FastHashMap<compact_str::CompactString, Value>;

#[derive(Debug, Clone, Serialize, Deserialize, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[archive(check_bytes)]
pub struct ColumnDefinition {
    pub name: compact_str::CompactString,
    pub data_type: DataType,
    pub nullable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[archive(check_bytes)]
pub enum DataType {
    Integer,
    Float,
    String,
    Boolean,
    Blob,
}
