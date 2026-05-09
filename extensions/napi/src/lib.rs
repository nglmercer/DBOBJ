use napi_derive::napi;
use napi::bindgen_prelude::*;
use dbobj::Database as CoreDatabase;
use std::sync::Arc;

#[napi]
pub struct Database {
    inner: Arc<CoreDatabase>,
}

#[napi]
impl Database {
    #[napi(constructor)]
    pub fn new(name: String) -> Self {
        Self {
            inner: Arc::new(CoreDatabase::new(name)),
        }
    }

    #[napi]
    pub fn create_table(&self, name: String, column_names: Vec<String>, column_types: Vec<String>) -> Result<()> {
        use dbobj::{Schema, ColumnDefinition, DataType};
        let mut columns = Vec::new();
        for (name, ty) in column_names.into_iter().zip(column_types) {
            let data_type = match ty.as_str() {
                "integer" => DataType::Integer,
                "string" => DataType::String,
                "float" => DataType::Float,
                "boolean" => DataType::Boolean,
                "blob" => DataType::Blob,
                _ => DataType::Integer,
            };
            columns.push(ColumnDefinition {
                name: name.into(),
                data_type,
                nullable: true,
            });
        }
        self.inner.create_table(name, Schema { columns });
        Ok(())
    }

    #[napi]
    pub fn insert_row_i64(&self, table_name: String, values: Vec<i64>) -> Result<()> {
        use dbobj::Value;
        let mut row_values = Vec::new();
        for v in values {
            row_values.push(Value::Integer(v));
        }
        self.inner.insert_values(&table_name, row_values).map_err(|e| {
            napi::Error::from_reason(e.to_string())
        })?;
        Ok(())
    }

    #[napi]
    pub fn get_column_i64(&self, table_name: String, column_name: String, _env: Env) -> Result<BigInt64Array> {
        let table_lock = self.inner.get_table(&table_name).ok_or_else(|| {
            napi::Error::from_reason(format!("Table {} not found", table_name))
        })?;
        let table = table_lock.read();
        
        let mut data = table.export_column_i64(&column_name).ok_or_else(|| {
            napi::Error::from_reason(format!("Column {} not found or not an integer column", column_name))
        })?;

        let ptr = data.as_mut_ptr();
        let len = data.len();
        std::mem::forget(data); // Move ownership to the callback

        unsafe {
            // Zero-Copy Memory Sharing: Rust Vec -> Node.js BigInt64Array
            Ok(BigInt64Array::with_external_data(
                ptr,
                len,
                |ptr, len| {
                    // Recover the Vec to deallocate it properly
                    let _ = Vec::from_raw_parts(ptr, len, len);
                }
            ))
        }
    }
}
