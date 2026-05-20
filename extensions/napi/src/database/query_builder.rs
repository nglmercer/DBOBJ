use dbobj::core::query_builder::QueryBuilder as CoreQueryBuilder;
use dbobj::core::Database as CoreDatabase;
use dbobj::{Value, Id};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::database::query::db_value_to_json_no_table;

#[napi]
pub struct JsQueryBuilder {
    inner: CoreQueryBuilder,
    db: Arc<CoreDatabase>,
    is_dirty: Arc<AtomicBool>,
}

#[napi]
impl JsQueryBuilder {
    pub(crate) fn new(db: Arc<CoreDatabase>, is_dirty: Arc<AtomicBool>) -> Self {
        Self {
            inner: CoreQueryBuilder::select(""),
            db,
            is_dirty,
        }
    }

    #[napi]
    pub fn select(&mut self, table: String) -> &Self {
        self.inner = CoreQueryBuilder::select(table);
        self
    }

    #[napi]
    pub fn insert(&mut self, table: String) -> &Self {
        self.inner = CoreQueryBuilder::insert(table);
        self
    }

    #[napi]
    pub fn update(&mut self, table: String) -> &Self {
        self.inner = CoreQueryBuilder::update(table);
        self
    }

    #[napi]
    pub fn delete(&mut self, table: String) -> &Self {
        self.inner = CoreQueryBuilder::delete(table);
        self
    }

    #[napi]
    pub fn columns(&mut self, cols: Vec<String>) -> &Self {
        let old = std::mem::replace(&mut self.inner, CoreQueryBuilder::select(""));
        self.inner = old.columns(cols);
        self
    }

    #[napi]
    pub fn set(&mut self, column: String, value: serde_json::Value) -> &Self {
        let old = std::mem::replace(&mut self.inner, CoreQueryBuilder::select(""));
        self.inner = old.set(column, json_to_value(value));
        self
    }

    #[napi]
    pub fn where_eq(&mut self, column: String, value: serde_json::Value) -> &Self {
        let old = std::mem::replace(&mut self.inner, CoreQueryBuilder::select(""));
        self.inner = old.where_eq(column, json_to_value(value));
        self
    }

    #[napi]
    pub fn where_neq(&mut self, column: String, value: serde_json::Value) -> &Self {
        let old = std::mem::replace(&mut self.inner, CoreQueryBuilder::select(""));
        self.inner = old.where_neq(column, json_to_value(value));
        self
    }

    #[napi]
    pub fn where_gt(&mut self, column: String, value: serde_json::Value) -> &Self {
        let old = std::mem::replace(&mut self.inner, CoreQueryBuilder::select(""));
        self.inner = old.where_gt(column, json_to_value(value));
        self
    }

    #[napi]
    pub fn where_gte(&mut self, column: String, value: serde_json::Value) -> &Self {
        let old = std::mem::replace(&mut self.inner, CoreQueryBuilder::select(""));
        self.inner = old.where_gte(column, json_to_value(value));
        self
    }

    #[napi]
    pub fn where_lt(&mut self, column: String, value: serde_json::Value) -> &Self {
        let old = std::mem::replace(&mut self.inner, CoreQueryBuilder::select(""));
        self.inner = old.where_lt(column, json_to_value(value));
        self
    }

    #[napi]
    pub fn where_lte(&mut self, column: String, value: serde_json::Value) -> &Self {
        let old = std::mem::replace(&mut self.inner, CoreQueryBuilder::select(""));
        self.inner = old.where_lte(column, json_to_value(value));
        self
    }

    #[napi]
    pub fn where_like(&mut self, column: String, pattern: String) -> &Self {
        let old = std::mem::replace(&mut self.inner, CoreQueryBuilder::select(""));
        self.inner = old.where_like(column, pattern);
        self
    }

    #[napi]
    pub fn order_by(&mut self, column: String, descending: bool) -> &Self {
        let old = std::mem::replace(&mut self.inner, CoreQueryBuilder::select(""));
        self.inner = old.order_by(column, descending);
        self
    }

    #[napi]
    pub fn limit(&mut self, limit: u32) -> &Self {
        let old = std::mem::replace(&mut self.inner, CoreQueryBuilder::select(""));
        self.inner = old.limit(limit as usize);
        self
    }

    #[napi]
    pub fn offset(&mut self, offset: u32) -> &Self {
        let old = std::mem::replace(&mut self.inner, CoreQueryBuilder::select(""));
        self.inner = old.offset(offset as usize);
        self
    }

    #[napi]
    pub fn execute(&self) -> Result<serde_json::Value> {
        let rows = self.inner.run(&self.db)
            .map_err(|e| napi::Error::from_reason(e))?;

        let results = self.rows_to_json(&rows);
        self.is_dirty.store(true, Ordering::Relaxed);
        Ok(serde_json::Value::Array(results))
    }

    #[napi]
    pub fn first(&self) -> Result<Option<serde_json::Value>> {
        let row = self.inner.run_first(&self.db)
            .map_err(|e| napi::Error::from_reason(e))?;

        match row {
            Some(r) => {
                let mut results = self.rows_to_json(&[r]);
                self.is_dirty.store(true, Ordering::Relaxed);
                Ok(results.pop())
            }
            None => Ok(None),
        }
    }
}

impl JsQueryBuilder {
    fn rows_to_json(&self, rows: &[dbobj::core::table::Row]) -> Vec<serde_json::Value> {
        let tables_guard = self.db.tables.read();
        let table_name = self.inner.table_name();
        let table_lock = if table_name.is_empty() {
            None
        } else {
            tables_guard.get(table_name)
        };
        let table_read = table_lock.map(|t| t.read());

        rows.iter().map(|row| {
            let mut map = serde_json::Map::new();
            match &row.id {
                Id::Integer(i) => { map.insert("id".into(), serde_json::Value::Number((*i).into())); }
                Id::String(s) => { map.insert("id".into(), serde_json::Value::String(s.to_string())); }
            }
            if let Some(ref table) = table_read {
                for (col_idx, col_def) in table.schema.columns.iter().enumerate() {
                    if col_idx < row.data.len() {
                        map.insert(col_def.name.to_string(), db_value_to_json_no_table(&row.data[col_idx]));
                    }
                }
            }
            serde_json::Value::Object(map)
        }).collect()
    }
}

fn json_to_value(val: serde_json::Value) -> Value {
    match val {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Boolean(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Null
            }
        }
        serde_json::Value::String(s) => Value::String(s.into()),
        serde_json::Value::Array(arr) => {
            Value::String(serde_json::to_string(&arr).unwrap_or_default().into())
        }
        serde_json::Value::Object(obj) => {
            Value::String(serde_json::to_string(&obj).unwrap_or_default().into())
        }
    }
}
