use napi_derive::napi;
use napi::bindgen_prelude::*;
use dbobj::Database as CoreDatabase;
use std::sync::Arc;

#[napi(object)]
pub struct TableMetadata {
    pub name: String,
    pub row_count: u32,
    pub column_count: u32,
}

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

    #[napi(factory)]
    pub fn load(path: String) -> Result<Self> {
        let db = CoreDatabase::load_from_mmap(path).map_err(|e| {
            napi::Error::from_reason(e.to_string())
        })?;
        Ok(Self {
            inner: Arc::new(db),
        })
    }

    #[napi]
    pub fn save(&self, path: String) -> Result<()> {
        self.inner.save_to_mmap(path).map_err(|e| {
            napi::Error::from_reason(e.to_string())
        })?;
        Ok(())
    }

    #[napi]
    pub fn list_tables(&self) -> Vec<String> {
        self.inner.list_tables()
    }

    #[napi]
    pub fn create_index(&self, table_name: String, column_name: String) -> Result<()> {
        self.inner.create_index(&table_name, &column_name).map_err(|e| {
            napi::Error::from_reason(e.to_string())
        })?;
        Ok(())
    }

    #[napi]
    pub fn create_unique_index(&self, table_name: String, column_name: String) -> Result<()> {
        self.inner.create_unique_index(&table_name, &column_name).map_err(|e| {
            napi::Error::from_reason(e.to_string())
        })?;
        Ok(())
    }

    #[napi]
    pub fn get_table_metadata(&self, name: String) -> Result<Option<TableMetadata>> {
        Ok(self.inner.table_info(&name).map(|info| TableMetadata {
            name: info.name,
            row_count: info.row_count as u32,
            column_count: info.columns.len() as u32,
        }))
    }

    #[napi]
    pub fn insert_batch_i64(&self, table_name: String, values: BigInt64Array, num_columns: u32) -> Result<()> {
        use dbobj::Value;
        let num_cols = num_columns as usize;
        let mut batch = Vec::with_capacity(values.len() / num_cols);
        
        for chunk in values.as_ref().chunks(num_cols) {
            let mut row = Vec::with_capacity(num_cols);
            for &v in chunk {
                row.push(Value::Integer(v));
            }
            batch.push(row);
        }
        
        self.inner.insert_batch_values(&table_name, batch).map_err(|e| {
            napi::Error::from_reason(e.to_string())
        })?;
        Ok(())
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

    #[napi]
    pub fn update_row_i64(&self, table_name: String, id: u32, values: Vec<i64>) -> Result<()> {
        use dbobj::{Value, Id};
        let mut row_values = Vec::new();
        for v in values {
            row_values.push(Value::Integer(v));
        }
        self.inner.update_values(&table_name, &Id::Integer(id as u64), row_values).map_err(|e| {
            napi::Error::from_reason(e.to_string())
        })?;
        Ok(())
    }

    #[napi]
    pub fn delete_row(&self, table_name: String, id: u32) -> Result<()> {
        use dbobj::Id;
        self.inner.delete_row(&table_name, &Id::Integer(id as u64)).map_err(|e| {
            napi::Error::from_reason(e.to_string())
        })?;
        Ok(())
    }

    #[napi]
    pub fn find_by_i64(&self, table_name: String, column_name: String, value: i64) -> Result<BigInt64Array> {
        use dbobj::{Value, Id};
        let results = self.inner.find(&table_name, &column_name, Value::Integer(value)).map_err(|e| {
            napi::Error::from_reason(e.to_string())
        })?;
        
        let mut ids: Vec<i64> = results.into_iter().map(|r| {
            match r.id {
                Id::Integer(i) => i as i64,
                _ => 0, // Should not happen in this bench
            }
        }).collect();
        let ptr = ids.as_mut_ptr();
        let len = ids.len();
        std::mem::forget(ids);

        unsafe {
            Ok(BigInt64Array::with_external_data(ptr, len, |ptr, len| {
                let _ = Vec::from_raw_parts(ptr, len, len);
            }))
        }
    }

    #[napi]
    pub fn hash_join_i64(&self, table1: String, col1: String, table2: String, col2: String) -> Result<BigInt64Array> {
        use dbobj::Id;
        let results = self.inner.hash_join(&table1, &col1, &table2, &col2).map_err(|e| {
            napi::Error::from_reason(e.to_string())
        })?;
        
        let mut flat_results: Vec<i64> = Vec::with_capacity(results.len() * 2);
        for (r1, r2) in results {
            if let Id::Integer(id1) = r1.id { flat_results.push(id1 as i64); } else { flat_results.push(0); }
            if let Id::Integer(id2) = r2.id { flat_results.push(id2 as i64); } else { flat_results.push(0); }
        }
        
        let ptr = flat_results.as_mut_ptr();
        let len = flat_results.len();
        std::mem::forget(flat_results);

        unsafe {
            Ok(BigInt64Array::with_external_data(ptr, len, |ptr, len| {
                let _ = Vec::from_raw_parts(ptr, len, len);
            }))
        }
    }
}
