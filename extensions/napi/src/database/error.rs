use napi_derive::napi;

#[napi]
pub struct DbError {
    code: String,
    message: String,
}

#[napi]
impl DbError {
    #[napi(getter)]
    pub fn code(&self) -> String { self.code.clone() }
    #[napi(getter)]
    pub fn message(&self) -> String { self.message.clone() }
}

impl From<&str> for DbError {
    fn from(msg: &str) -> Self {
        let code = if msg.contains("not found") { "TABLE_NOT_FOUND".into() }
            else if msg.contains("Schema violation") || msg.contains("schema") { "SCHEMA_VIOLATION".into() }
            else if msg.contains("already exists") || msg.contains("Duplicate") { "DUPLICATE_KEY".into() }
            else if msg.contains("not nullable") || msg.contains("nullable") { "NULLABLE_VIOLATION".into() }
            else { "UNKNOWN".into() };
        DbError { code, message: msg.to_string() }
    }
}

pub(crate) fn map_err<T>(result: Result<T, impl ToString>) -> napi::Result<T> {
    result.map_err(|e| {
        let msg = e.to_string();
        let code = if msg.contains("not found") { "TABLE_NOT_FOUND" }
            else if msg.contains("Schema violation") { "SCHEMA_VIOLATION" }
            else if msg.contains("Duplicate") || msg.contains("already exists") { "DUPLICATE_KEY" }
            else { "UNKNOWN" };
        napi::Error::new(napi::Status::GenericFailure, format!("[{}] {}", code, msg))
    })
}
