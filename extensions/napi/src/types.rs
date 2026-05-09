use napi_derive::napi;

#[napi(object)]
pub struct TableMetadata {
    pub name: String,
    pub row_count: u32,
    pub column_count: u32,
}
