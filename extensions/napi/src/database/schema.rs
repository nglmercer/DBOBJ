use napi_derive::napi;

#[napi]
pub struct Schema {
    pub(crate) db: std::sync::Arc<dbobj::Database>,
}

#[napi]
impl Schema {
    fn get_info(&self, table_name: &str) -> napi::Result<dbobj::core::TableInfo> {
        self.db.table_info(table_name).ok_or_else(|| {
            napi::Error::from_reason(format!("Table '{}' not found", table_name))
        })
    }

    /// Returns a description of schema violations in the given row values.
    /// Returns an empty array if the row is valid.
    #[napi]
    pub fn validate_row(&self, table_name: String, values: Vec<serde_json::Value>) -> napi::Result<Vec<String>> {
        let info = self.get_info(&table_name)?;
        let mut errors = Vec::new();

        if values.len() != info.columns.len() {
            errors.push(format!(
                "expected {} values, got {}", info.columns.len(), values.len()
            ));
            return Ok(errors);
        }

        for (i, col) in info.columns.iter().enumerate() {
            let val = &values[i];
            let ok = match (&col.data_type, val) {
                (_, serde_json::Value::Null) => col.nullable,
                (dbobj::DataType::Integer, serde_json::Value::Number(n)) => n.is_i64(),
                (dbobj::DataType::Float, serde_json::Value::Number(_)) => true,
                (dbobj::DataType::String, serde_json::Value::String(_)) => true,
                (dbobj::DataType::Boolean, serde_json::Value::Bool(_)) => true,
                (dbobj::DataType::Blob, serde_json::Value::Array(_)) => true,
                _ => false,
            };
            if !ok {
                errors.push(format!(
                    "column '{}': expected {:?}, got {:?}", col.name, col.data_type, val
                ));
            }
        }
        Ok(errors)
    }

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
