pub mod id;
pub mod value;
pub mod table;
pub mod database;

pub use id::Id;
pub use value::Value;
pub use table::{Table, Row, Schema};
pub use database::Database;

use serde::{Deserialize, Serialize};

pub type FastHashMap<K, V> = std::collections::HashMap<K, V, ahash::RandomState>;
pub type RowData = FastHashMap<compact_str::CompactString, Value>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDefinition {
    pub name: compact_str::CompactString,
    pub data_type: DataType,
    pub nullable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DataType {
    Integer,
    Float,
    String,
    Boolean,
    Blob,
}
