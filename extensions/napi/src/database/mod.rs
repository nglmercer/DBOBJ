pub(crate) mod insert;
pub(crate) mod update;
pub(crate) mod query;

use crate::types::{ColumnDefinition, TableMetadata};
use dbobj::Database as CoreDatabase;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) fn json_to_db_value(val: serde_json::Value) -> dbobj::Value {
    match val {
        serde_json::Value::Null => dbobj::Value::Null,
        serde_json::Value::Bool(b) => dbobj::Value::Boolean(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                dbobj::Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                dbobj::Value::Float(f)
            } else {
                dbobj::Value::Null
            }
        }
        serde_json::Value::String(s) => dbobj::Value::String(s.into()),
        _ => dbobj::Value::Null,
    }
}

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
        executor
            .execute_prepared(&self.inner, &vals)
            .map_err(napi::Error::from_reason)?;
        self.is_dirty.store(true, Ordering::Relaxed);
        Ok(())
    }

    #[napi]
    pub fn all_i64(&self, _params: Vec<i64>) -> Result<BigInt64Array> {
        let stmt = &self.inner;
        if stmt.statements.len() == 1 {
            if let dbobj_sql::local_parser::Statement::Select {
                columns,
                table,
                selection: _,
                join,
            } = &stmt.statements[0] {
                if let dbobj_sql::local_parser::SelectColumns::List(cols) = columns {
                    if cols.len() == 1 && join.is_none() {
                        let table_name = table.to_string();
                        let table_lock = self.db.get_table(&table_name).ok_or_else(|| {
                            napi::Error::from_reason(format!("Table {} not found", table_name))
                        })?;
                        let table_ref = table_lock.read();
                        let col_idx = *table_ref.column_map.get(cols[0].as_str()).ok_or_else(|| {
                            napi::Error::from_reason(format!("Column {} not found", cols[0]))
                        })?;

                        let num_rows = table_ref.ids.len();
                        let mut result = Vec::with_capacity(num_rows);
                        for i in 0..num_rows {
                            let val = &table_ref.data[i * table_ref.num_columns + col_idx];
                            if let dbobj::Value::Integer(i) = val {
                                result.push(*i);
                            } else {
                                result.push(0);
                            }
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
        executor
            .execute_prepared_batch(&self.inner, &batch)
            .map_err(napi::Error::from_reason)?;
        self.is_dirty.store(true, Ordering::Relaxed);
        Ok(())
    }

    #[napi]
    pub fn run_batch_values(&self, flat_params: Vec<serde_json::Value>, params_per_row: u32) -> Result<()> {
        let executor = dbobj_sql::SqlExecutor::new(&self.db);
        let pprow = params_per_row as usize;
        let total = flat_params.len();
        let mut iter = flat_params.into_iter();
        let mut batch = Vec::with_capacity(total / pprow);
        'outer: while let Some(v0) = iter.next() {
            let mut row = Vec::with_capacity(pprow);
            row.push(json_to_db_value(v0));
            for _ in 1..pprow {
                match iter.next() {
                    Some(v) => row.push(json_to_db_value(v)),
                    None => { batch.push(row); break 'outer; }
                }
            }
            batch.push(row);
        }
        executor
            .execute_prepared_batch(&self.inner, &batch)
            .map_err(napi::Error::from_reason)?;
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
        executor
            .execute_prepared_batch(&self.inner, &batch)
            .map_err(napi::Error::from_reason)?;
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
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            if is_dirty.swap(false, Ordering::Relaxed) {
                let _ = inner.save_to_mmap(&path);
            }
        });
        db
    }

    pub(crate) fn save_if_needed(&self) {
        if self.path.is_some() {
            self.is_dirty.store(true, Ordering::Relaxed);
        }
    }

    // ── DDL ──────────────────────────────────────────────────────────

    #[napi]
    pub fn create_table(&self, name: String, columns: Vec<ColumnDefinition>) -> Result<()> {
        use dbobj::Schema;
        let mut has_id = false;
        let schema_columns: Vec<dbobj::ColumnDefinition> = columns
            .into_iter()
            .map(|col| {
                if col.name == "id" { has_id = true; }
                let data_type = match col.data_type {
                    crate::types::DataType::Integer => dbobj::DataType::Integer,
                    crate::types::DataType::Float => dbobj::DataType::Float,
                    crate::types::DataType::String => dbobj::DataType::String,
                    crate::types::DataType::Boolean => dbobj::DataType::Boolean,
                    crate::types::DataType::Blob => dbobj::DataType::Blob,
                };
                Ok(dbobj::ColumnDefinition { name: col.name.into(), data_type, nullable: col.nullable.unwrap_or(true) })
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.inner.create_table(name.clone(), Schema { columns: schema_columns });
        if has_id { let _ = self.inner.create_unique_index(&name, "id"); }
        self.save_if_needed();
        Ok(())
    }

    // ── INSERT ───────────────────────────────────────────────────────

    #[napi]
    pub fn insert_batch_i64(&self, table_name: String, values: BigInt64Array, num_columns: u32) -> Result<()> {
        insert::insert_batch_i64(self, table_name, values.as_ref(), num_columns as usize)
    }

    #[napi]
    pub fn insert_row_i64(&self, table_name: String, values: Vec<i64>) -> Result<()> {
        insert::insert_row_i64(self, table_name, values)
    }

    #[napi]
    pub fn insert_row_string(&self, table_name: String, values: Vec<String>) -> Result<()> {
        insert::insert_row_string(self, table_name, values)
    }

    #[napi]
    pub fn insert_row_bool(&self, table_name: String, values: Vec<bool>) -> Result<()> {
        insert::insert_row_bool(self, table_name, values)
    }

    #[napi]
    pub fn insert_row(&self, table_name: String, values: Vec<serde_json::Value>) -> Result<()> {
        insert::insert_row(self, table_name, values)
    }

    #[napi]
    pub fn insert_batch_string(&self, table_name: String, values: Vec<String>, num_columns: u32) -> Result<()> {
        insert::insert_batch_string(self, table_name, values, num_columns)
    }

    #[napi]
    pub fn insert_batch_bool(&self, table_name: String, values: Vec<bool>, num_columns: u32) -> Result<()> {
        insert::insert_batch_bool(self, table_name, values, num_columns)
    }

    #[napi]
    pub fn insert_batch(&self, table_name: String, values: Vec<serde_json::Value>, num_columns: u32) -> Result<()> {
        insert::insert_batch(self, table_name, values, num_columns)
    }

    // ── UPDATE ───────────────────────────────────────────────────────

    #[napi]
    pub fn update_row_i64(&self, table_name: String, id: u32, values: Vec<i64>) -> Result<()> {
        update::update_row_i64(self, table_name, id, values)
    }

    #[napi]
    pub fn update_row_string(&self, table_name: String, id: u32, values: Vec<String>) -> Result<()> {
        update::update_row_string(self, table_name, id, values)
    }

    #[napi]
    pub fn update_row_bool(&self, table_name: String, id: u32, values: Vec<bool>) -> Result<()> {
        update::update_row_bool(self, table_name, id, values)
    }

    #[napi]
    pub fn update_row(&self, table_name: String, id: u32, values: Vec<serde_json::Value>) -> Result<()> {
        update::update_row(self, table_name, id, values)
    }

    #[napi]
    pub fn delete_row(&self, table_name: String, id: u32) -> Result<()> {
        update::delete_row(self, table_name, id)
    }

    // ── QUERY ────────────────────────────────────────────────────────

    #[napi]
    pub fn get_column_i64(&self, table_name: String, column_name: String, _env: Env) -> Result<BigInt64Array> {
        query::get_column_i64(self, table_name, column_name)
    }

    #[napi]
    pub fn find_by_i64(&self, table_name: String, column_name: String, value: i64) -> Result<BigInt64Array> {
        query::find_by_i64(self, table_name, column_name, value)
    }

    #[napi]
    pub fn hash_join_i64(&self, table1: String, col1: String, table2: String, col2: String) -> Result<BigInt64Array> {
        query::hash_join_i64(self, table1, col1, table2, col2)
    }

    #[napi]
    pub fn get_rows(&self, table_name: String, limit: Option<u32>, offset: Option<u32>) -> Result<serde_json::Value> {
        query::get_rows(self, table_name, limit, offset)
    }

    // ── SQL ──────────────────────────────────────────────────────────

    #[napi]
    pub fn execute_sql(&self, sql: String) -> Result<serde_json::Value> {
        let executor = dbobj_sql::SqlExecutor::new(&self.inner);
        let result = executor.execute(&sql).map_err(napi::Error::from_reason)?;
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
                                .map(serde_json::Value::Number).unwrap_or(serde_json::Value::Null),
                            dbobj::Value::String(s) => serde_json::Value::String(s.to_string()),
                            dbobj::Value::Boolean(b) => serde_json::Value::Bool(b),
                            dbobj::Value::Blob(b) => serde_json::Value::Array(
                                b.iter().map(|&x| serde_json::Value::Number(x.into())).collect(),
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
            }
            dbobj_sql::SqlResult::I64(vals) => {
                let results: Vec<serde_json::Value> = vals.into_iter()
                    .map(|i| serde_json::Value::Number(i.into())).collect();
                Ok(serde_json::Value::Array(results))
            }
        };
        self.save_if_needed();
        out
    }

    #[napi]
    pub fn prepare(&self, sql: String) -> Result<PreparedStatement> {
        let executor = dbobj_sql::SqlExecutor::new(&self.inner);
        let stmt = executor.prepare(&sql).map_err(napi::Error::from_reason)?;
        Ok(PreparedStatement { inner: stmt, db: self.inner.clone(), is_dirty: self.is_dirty.clone() })
    }

    #[napi]
    pub fn query_i64(&self, sql: String) -> Result<BigInt64Array> {
        let executor = dbobj_sql::SqlExecutor::new(&self.inner);
        let mut result = executor.execute_i64(&sql).map_err(napi::Error::from_reason)?;
        let ptr = result.as_mut_ptr();
        let len = result.len();
        std::mem::forget(result);
        unsafe { Ok(BigInt64Array::with_external_data(ptr, len, |ptr, len| { let _ = Vec::from_raw_parts(ptr, len, len); })) }
    }

    #[napi]
    pub fn query_join_i64(&self, sql: String) -> Result<BigInt64Array> {
        let executor = dbobj_sql::SqlExecutor::new(&self.inner);
        let (mut result, _width) = executor.execute_join_i64(&sql).map_err(napi::Error::from_reason)?;
        let ptr = result.as_mut_ptr();
        let len = result.len();
        std::mem::forget(result);
        unsafe { Ok(BigInt64Array::with_external_data(ptr, len, |ptr, len| { let _ = Vec::from_raw_parts(ptr, len, len); })) }
    }

    // ── META ─────────────────────────────────────────────────────────

    #[napi(factory)]
    pub fn load(path: String) -> Result<Self> {
        let db = CoreDatabase::load_from_mmap(&path)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        let inner = Arc::new(db);
        let is_dirty = Arc::new(AtomicBool::new(false));
        let path_clone = path.clone();
        let inner_clone = inner.clone();
        let is_dirty_clone = is_dirty.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            if is_dirty_clone.swap(false, Ordering::Relaxed) {
                let _ = inner_clone.save_to_mmap(&path_clone);
            }
        });
        Ok(Self { inner, path: Some(path), is_dirty })
    }

    #[napi]
    pub fn save(&self, path: String) -> Result<()> {
        self.inner.save_to_mmap(path).map_err(|e| napi::Error::from_reason(e.to_string()))?;
        self.is_dirty.store(false, Ordering::Relaxed);
        Ok(())
    }

    #[napi]
    pub fn list_tables(&self) -> Vec<String> { self.inner.list_tables() }

    #[napi]
    pub fn create_index(&self, table_name: String, column_name: String) -> Result<()> {
        self.inner.create_index(&table_name, &column_name).map_err(|e| napi::Error::from_reason(e.to_string()))?;
        self.save_if_needed(); Ok(())
    }

    #[napi]
    pub fn create_unique_index(&self, table_name: String, column_name: String) -> Result<()> {
        self.inner.create_unique_index(&table_name, &column_name).map_err(|e| napi::Error::from_reason(e.to_string()))?;
        self.save_if_needed(); Ok(())
    }

    #[napi]
    pub fn get_table_metadata(&self, name: String) -> Result<Option<TableMetadata>> {
        Ok(self.inner.table_info(&name).map(|info| TableMetadata {
            name: info.name, row_count: info.row_count as u32, column_count: info.columns.len() as u32,
        }))
    }
}
