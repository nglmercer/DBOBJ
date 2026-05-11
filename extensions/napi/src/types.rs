use napi_derive::napi;

#[napi]
pub enum DataType {
    Integer,
    Float,
    String,
    Boolean,
    Blob,
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
