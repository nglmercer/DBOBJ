use napi_derive::napi;

#[derive(Clone, Debug)]
#[napi]
pub enum DataType {
    Integer,
    Float,
    String,
    Boolean,
    Blob,
    Json,
    ArrayString,
    ArrayI64,
    ArrayF64,
}

#[napi(object)]
pub struct ColumnDefinition {
    pub name: String,
    pub data_type: DataType,
    /// Defaults to true if not specified
    pub nullable: Option<bool>,
}

#[napi(object)]
pub struct TableMetadata {
    pub name: String,
    pub row_count: u32,
    pub column_count: u32,
}
