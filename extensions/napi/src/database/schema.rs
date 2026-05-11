use napi_derive::napi;
use super::Database;

#[napi]
pub struct Schema {
    pub(crate) db: std::sync::Arc<dbobj::Database>,
}

#[napi]
impl Schema {
    #[napi]
    pub fn get_column_names(&self, table_name: String) -> napi::Result<Vec<String>> {
        let info = self.db.table_info(&table_name).ok_or_else(|| {
            napi::Error::from_reason(format!("Table {} not found", table_name))
        })?;
        Ok(info.columns.into_iter().map(|c| c.name.to_string()).collect())
    }

    #[napi]
    pub fn get_column_type(&self, table_name: String, column_name: String) -> napi::Result<crate::types::DataType> {
        let info = self.db.table_info(&table_name).ok_or_else(|| {
            napi::Error::from_reason(format!("Table {} not found", table_name))
        })?;
        let col = info.columns.iter().find(|c| c.name.as_str() == column_name).ok_or_else(|| {
            napi::Error::from_reason(format!("Column '{}' not found in table '{}'", column_name, table_name))
        })?;
        Ok(match col.data_type {
            dbobj::DataType::Integer => crate::types::DataType::Integer,
            dbobj::DataType::Float => crate::types::DataType::Float,
            dbobj::DataType::String => crate::types::DataType::String,
            dbobj::DataType::Boolean => crate::types::DataType::Boolean,
            dbobj::DataType::Blob => crate::types::DataType::Blob,
        })
    }

    #[napi]
    pub fn has_column(&self, table_name: String, column_name: String) -> napi::Result<bool> {
        let info = self.db.table_info(&table_name).ok_or_else(|| {
            napi::Error::from_reason(format!("Table {} not found", table_name))
        })?;
        Ok(info.columns.iter().any(|c| c.name.as_str() == column_name))
    }
}
