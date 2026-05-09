use napi_derive::napi;
use napi::bindgen_prelude::*;
use dbobj::Database as CoreDatabase;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::types::TableMetadata;

#[napi]
pub struct Database {
    pub(crate) inner: Arc<CoreDatabase>,
    pub(crate) path: Option<String>,
    pub(crate) is_dirty: Arc<AtomicBool>,
}

#[napi]
pub struct PreparedStatement {
    pub(crate) inner: dbobj_sql::PreparedStatement,
    pub(crate) db: Arc<CoreDatabase>,
    pub(crate) is_dirty: Arc<AtomicBool>,
}

#[napi]
impl PreparedStatement {
    #[napi]
    pub fn run(&self, params: Vec<i64>) -> Result<()> {
        let executor = dbobj_sql::SqlExecutor::new(&self.db);
        let vals: Vec<_> = params.into_iter().map(dbobj::Value::Integer).collect();
        executor.execute_prepared(&self.inner, &vals).map_err(|e| napi::Error::from_reason(e))?;
        self.is_dirty.store(true, Ordering::Relaxed);
        Ok(())
    }

    #[napi]
    pub fn all_i64(&self, params: Vec<i64>) -> Result<BigInt64Array> {
        let executor = dbobj_sql::SqlExecutor::new(&self.db);
        let vals: Vec<_> = params.into_iter().map(dbobj::Value::Integer).collect();
        
        // Use a specialized path for prepared columnar queries
        let stmt = &self.inner;
        if stmt.statements.len() == 1 {
            if let dbobj_sql::local_parser::Statement::Select { columns, table, selection: _, join } = &stmt.statements[0] {
                 if let dbobj_sql::local_parser::SelectColumns::List(cols) = columns 
                    && cols.len() == 1
                    && join.is_none()
                 {
                     let table_name = table.to_string();
                     let table_lock = self.db.get_table(&table_name)
                         .ok_or_else(|| napi::Error::from_reason(format!("Table {} not found", table_name)))?;
                     let table_ref = table_lock.read();
                     let col_idx = *table_ref.column_map.get(cols[0].as_str())
                         .ok_or_else(|| napi::Error::from_reason(format!("Column {} not found", cols[0])))?;
                     
                     let num_rows = table_ref.ids.len();
                     let mut result = Vec::with_capacity(num_rows);
                     for i in 0..num_rows {
                         let val = &table_ref.data[i * table_ref.num_columns + col_idx];
                         if let dbobj::Value::Integer(i) = val { result.push(*i); } else { result.push(0); }
                     }
                     let ptr = result.as_mut_ptr();
                     let len = result.len();
                     std::mem::forget(result);
                     unsafe {
                         return Ok(BigInt64Array::with_external_data(ptr, len, |ptr, len| {
                             let _ = Vec::from_raw_parts(ptr, len, len);
                         }));
                     }
                 }
            }
        }
        Err(napi::Error::from_reason("Query not suitable for all_i64"))
    }

    #[napi]
    pub fn run_batch(&self, batch_params: Vec<Vec<i64>>) -> Result<()> {
        let executor = dbobj_sql::SqlExecutor::new(&self.db);
        let batch: Vec<Vec<dbobj::Value>> = batch_params
            .into_iter()
            .map(|params| params.into_iter().map(dbobj::Value::Integer).collect())
            .collect();
        executor.execute_prepared_batch(&self.inner, &batch).map_err(|e| napi::Error::from_reason(e))?;
        self.is_dirty.store(true, Ordering::Relaxed);
        Ok(())
    }

    #[napi]
    pub fn run_batch_i64(&self, flat_params: BigInt64Array, params_per_row: u32) -> Result<()> {
        let executor = dbobj_sql::SqlExecutor::new(&self.db);
        let params_slice = flat_params.as_ref();
        let num_params = params_slice.len();
        let mut i = 0;
        let mut batch = Vec::with_capacity(num_params / params_per_row as usize);
        
        while i < num_params {
            let mut row = Vec::with_capacity(params_per_row as usize);
            for _ in 0..params_per_row {
                if i < num_params {
                    row.push(dbobj::Value::Integer(params_slice[i]));
                    i += 1;
                }
            }
            batch.push(row);
        }
        
        executor.execute_prepared_batch(&self.inner, &batch).map_err(|e| napi::Error::from_reason(e))?;
        self.is_dirty.store(true, Ordering::Relaxed);
        Ok(())
    }
}

#[napi]
impl Database {
    #[napi(constructor)]
    pub fn new(name: String) -> Self {
        if name == ":memory:" {
            return Self {
                inner: Arc::new(CoreDatabase::new(name)),
                path: None,
                is_dirty: Arc::new(AtomicBool::new(false)),
            };
        }

        let path = if name.ends_with(".dbobj") {
            name.clone()
        } else {
            format!("{}.dbobj", name)
        };

        // Try to load if exists
        let inner = if std::path::Path::new(&path).exists() {
            match CoreDatabase::load_from_mmap(&path) {
                Ok(db) => Arc::new(db),
                Err(_) => Arc::new(CoreDatabase::new(name)),
            }
        } else {
            Arc::new(CoreDatabase::new(name))
        };

        let is_dirty = Arc::new(AtomicBool::new(false));
        
        let db = Self {
            inner: inner.clone(),
            path: Some(path.clone()),
            is_dirty: is_dirty.clone(),
        };

        // Background auto-save thread (debounces to 1 second)
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                if is_dirty.swap(false, Ordering::Relaxed) {
                    let _ = inner.save_to_mmap(&path);
                }
            }
        });

        db
    }

    #[napi(factory)]
    pub fn load(path: String) -> Result<Self> {
        let db = CoreDatabase::load_from_mmap(&path).map_err(|e| {
            napi::Error::from_reason(e.to_string())
        })?;
        
        let inner = Arc::new(db);
        let is_dirty = Arc::new(AtomicBool::new(false));
        
        let path_clone = path.clone();
        let inner_clone = inner.clone();
        let is_dirty_clone = is_dirty.clone();
        
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                if is_dirty_clone.swap(false, Ordering::Relaxed) {
                    let _ = inner_clone.save_to_mmap(&path_clone);
                }
            }
        });

        Ok(Self {
            inner,
            path: Some(path),
            is_dirty,
        })
    }

    fn save_if_needed(&self) {
        if self.path.is_some() {
            self.is_dirty.store(true, Ordering::Relaxed);
        }
    }

    #[napi]
    pub fn save(&self, path: String) -> Result<()> {
        self.inner.save_to_mmap(path).map_err(|e| {
            napi::Error::from_reason(e.to_string())
        })?;
        self.is_dirty.store(false, Ordering::Relaxed);
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
        self.save_if_needed();
        Ok(())
    }

    #[napi]
    pub fn create_unique_index(&self, table_name: String, column_name: String) -> Result<()> {
        self.inner.create_unique_index(&table_name, &column_name).map_err(|e| {
            napi::Error::from_reason(e.to_string())
        })?;
        self.save_if_needed();
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
        self.save_if_needed();
        Ok(())
    }

    #[napi]
    pub fn create_table(&self, name: String, column_names: Vec<String>, column_types: Vec<String>) -> Result<()> {
        use dbobj::{Schema, ColumnDefinition, DataType};
        let mut columns = Vec::new();
        let mut has_id = false;
        
        for (col_name, ty) in column_names.into_iter().zip(column_types) {
            if col_name == "id" {
                has_id = true;
            }
            let data_type = match ty.as_str() {
                "integer" => DataType::Integer,
                "string" => DataType::String,
                "float" => DataType::Float,
                "boolean" => DataType::Boolean,
                "blob" => DataType::Blob,
                _ => DataType::Integer,
            };
            columns.push(ColumnDefinition {
                name: col_name.into(),
                data_type,
                nullable: true,
            });
        }
        
        self.inner.create_table(name.clone(), Schema { columns });
        
        // Auto-create unique index for "id" column
        if has_id {
            let _ = self.inner.create_unique_index(&name, "id");
        }
        
        self.save_if_needed();
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
        self.save_if_needed();
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
            Ok(BigInt64Array::with_external_data(
                ptr,
                len,
                |ptr, len| {
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
        self.save_if_needed();
        Ok(())
    }

    #[napi]
    pub fn delete_row(&self, table_name: String, id: u32) -> Result<()> {
        use dbobj::Id;
        self.inner.delete_row(&table_name, &Id::Integer(id as u64)).map_err(|e| {
            napi::Error::from_reason(e.to_string())
        })?;
        self.save_if_needed();
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
                _ => 0,
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

    #[napi]
    pub fn execute_sql(&self, sql: String) -> Result<serde_json::Value> {
        let executor = dbobj_sql::SqlExecutor::new(&self.inner);
        let result = executor.execute(&sql).map_err(|e| {
            napi::Error::from_reason(e)
        })?;
        
        let out = match result {
            dbobj_sql::SqlResult::Ok => Ok(serde_json::Value::String("OK".to_string())),
            dbobj_sql::SqlResult::Rows(rows) => {
                let mut results = Vec::new();
                for row in rows {
                    let mut map = serde_json::Map::new();
                    for (k, v) in row {
                        let json_val = match v {
                            dbobj::Value::Null => serde_json::Value::Null,
                            dbobj::Value::Integer(i) => serde_json::Value::Number(i.into()),
                            dbobj::Value::Float(f) => serde_json::Number::from_f64(f)
                                .map(serde_json::Value::Number)
                                .unwrap_or(serde_json::Value::Null),
                            dbobj::Value::String(s) => serde_json::Value::String(s.to_string()),
                            dbobj::Value::Boolean(b) => serde_json::Value::Bool(b),
                            dbobj::Value::Blob(b) => serde_json::Value::Array(
                                b.iter().map(|&x| serde_json::Value::Number(x.into())).collect()
                            ),
                            dbobj::Value::InternedString(id) => {
                                serde_json::Value::String(format!("<interned:{}>", id))
                            }
                        };
                        map.insert(k.to_string(), json_val);
                    }
                    results.push(serde_json::Value::Object(map));
                }
                Ok(serde_json::Value::Array(results))
            },
            dbobj_sql::SqlResult::I64(vals) => {
                let results = vals.into_iter().map(|i| serde_json::Value::Number(i.into())).collect();
                Ok(serde_json::Value::Array(results))
            }
        };

        // If it was a mutation (SQLResult::Ok), save if needed.
        // Actually, many queries might mutate.
        self.save_if_needed();
        
        out
    }

    #[napi]
    pub fn prepare(&self, sql: String) -> Result<PreparedStatement> {
        let executor = dbobj_sql::SqlExecutor::new(&self.inner);
        let stmt = executor.prepare(&sql).map_err(|e| napi::Error::from_reason(e))?;
        Ok(PreparedStatement {
            inner: stmt,
            db: self.inner.clone(),
            is_dirty: self.is_dirty.clone(),
        })
    }
    #[napi]
    pub fn query_i64(&self, sql: String) -> Result<BigInt64Array> {
        let executor = dbobj_sql::SqlExecutor::new(&self.inner);
        let mut result = executor.execute_i64(&sql).map_err(|e| napi::Error::from_reason(e))?;
        
        let ptr = result.as_mut_ptr();
        let len = result.len();
        std::mem::forget(result);

        unsafe {
            Ok(BigInt64Array::with_external_data(ptr, len, |ptr, len| {
                let _ = Vec::from_raw_parts(ptr, len, len);
            }))
        }
    }
}
