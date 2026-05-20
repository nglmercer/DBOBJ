use dbobj::core::query_builder::QueryBuilder as CoreQueryBuilder;
use dbobj::core::Database as CoreDatabase;
use dbobj::{Value, Id, DataType};
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
        self.inner = std::mem::take(&mut self.inner).columns(cols);
        self
    }

    #[napi]
    pub fn set(&mut self, column: String, value: serde_json::Value) -> &Self {
        self.inner = std::mem::take(&mut self.inner).set(column, json_to_value(value));
        self
    }

    #[napi]
    pub fn where_eq(&mut self, column: String, value: serde_json::Value) -> &Self {
        self.inner = std::mem::take(&mut self.inner).where_eq(column, json_to_value(value));
        self
    }

    #[napi]
    pub fn where_neq(&mut self, column: String, value: serde_json::Value) -> &Self {
        self.inner = std::mem::take(&mut self.inner).where_neq(column, json_to_value(value));
        self
    }

    #[napi]
    pub fn where_gt(&mut self, column: String, value: serde_json::Value) -> &Self {
        self.inner = std::mem::take(&mut self.inner).where_gt(column, json_to_value(value));
        self
    }

    #[napi]
    pub fn where_gte(&mut self, column: String, value: serde_json::Value) -> &Self {
        self.inner = std::mem::take(&mut self.inner).where_gte(column, json_to_value(value));
        self
    }

    #[napi]
    pub fn where_lt(&mut self, column: String, value: serde_json::Value) -> &Self {
        self.inner = std::mem::take(&mut self.inner).where_lt(column, json_to_value(value));
        self
    }

    #[napi]
    pub fn where_lte(&mut self, column: String, value: serde_json::Value) -> &Self {
        self.inner = std::mem::take(&mut self.inner).where_lte(column, json_to_value(value));
        self
    }

    #[napi]
    pub fn where_like(&mut self, column: String, pattern: String) -> &Self {
        self.inner = std::mem::take(&mut self.inner).where_like(column, pattern);
        self
    }

    #[napi]
    pub fn order_by(&mut self, column: String, descending: bool) -> &Self {
        self.inner = std::mem::take(&mut self.inner).order_by(column, descending);
        self
    }

    #[napi]
    pub fn limit(&mut self, limit: u32) -> &Self {
        self.inner = std::mem::take(&mut self.inner).limit(limit as usize);
        self
    }

    #[napi]
    pub fn offset(&mut self, offset: u32) -> &Self {
        self.inner = std::mem::take(&mut self.inner).offset(offset as usize);
        self
    }

    /// INNER JOIN: ON this_column = other_table.other_column
    #[napi]
    pub fn join(&mut self, other_table: String, this_column: String, other_column: String) -> &Self {
        self.inner = std::mem::take(&mut self.inner).join(other_table, this_column, other_column);
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

    /// Execute query and return data column-oriented: { colName: [values...] }
    /// Avoids per-row JSON object overhead.
    #[napi]
    pub fn execute_columnar(&self) -> Result<serde_json::Value> {
        let rows = self.inner.run(&self.db)
            .map_err(|e| napi::Error::from_reason(e))?;

        self.is_dirty.store(true, Ordering::Relaxed);

        let tables_guard = self.db.tables.read();
        let table_name = self.inner.table_name();
        if table_name.is_empty() {
            return Ok(serde_json::Value::Object(serde_json::Map::new()));
        }
        let table_lock = tables_guard.get(table_name)
            .ok_or_else(|| napi::Error::from_reason("Table not found"))?;
        let table = table_lock.read();

        let mut map = serde_json::Map::with_capacity(table.schema.columns.len() + 1);

        // id column
        let ids: Vec<serde_json::Value> = rows.iter().map(|row| match &row.id {
            Id::Integer(i) => serde_json::Value::Number((*i).into()),
            Id::String(s) => serde_json::Value::String(s.to_string()),
        }).collect();
        map.insert("id".into(), serde_json::Value::Array(ids));

        // data columns
        for (col_idx, col_def) in table.schema.columns.iter().enumerate() {
            let mut col_values = Vec::with_capacity(rows.len());
            for row in &rows {
                let val = if col_idx < row.data.len() {
                    db_value_to_json_no_table(&row.data[col_idx])
                } else {
                    serde_json::Value::Null
                };
                col_values.push(val);
            }
            map.insert(col_def.name.to_string(), serde_json::Value::Array(col_values));
        }

        Ok(serde_json::Value::Object(map))
    }

    /// Execute query and return results as Apache Arrow IPC buffer.
    /// Avoids JSON serialization — the JS side can parse with @apache-arrow.
    #[napi]
    pub fn execute_arrow(&self) -> Result<Buffer> {
        use std::sync::Arc as StdArc;
        use arrow::array::*;
        use arrow::datatypes::{Field, Schema as ArrowSchema};
        use arrow::ipc::writer::FileWriter;
        use arrow::record_batch::RecordBatch;

        let rows = self.inner.run(&self.db)
            .map_err(|e| napi::Error::from_reason(e))?;

        self.is_dirty.store(true, Ordering::Relaxed);

        let tables_guard = self.db.tables.read();
        let table_name = self.inner.table_name();
        if table_name.is_empty() {
            return Ok(Buffer::from(Vec::<u8>::new()));
        }
        let table_lock = tables_guard.get(table_name)
            .ok_or_else(|| napi::Error::from_reason("Table not found"))?;
        let table = table_lock.read();

        let num_rows = rows.len();
        if num_rows == 0 {
            return Ok(Buffer::from(Vec::<u8>::new()));
        }

        let num_cols = table.schema.columns.len();
        let mut arrow_fields = Vec::with_capacity(num_cols);
        let mut arrow_columns: Vec<ArrayRef> = Vec::with_capacity(num_cols);

        for (col_idx, col_def) in table.schema.columns.iter().enumerate() {
            let arrow_type = db_to_arrow_type(&col_def.data_type);
            arrow_fields.push(Field::new(col_def.name.as_str(), arrow_type, col_def.nullable));

            match col_def.data_type {
                DataType::Integer => {
                    let mut builder = Int64Builder::with_capacity(num_rows);
                    for row in &rows {
                        if col_idx < row.data.len() {
                            match &row.data[col_idx] {
                                Value::Integer(v) => builder.append_value(*v),
                                _ => builder.append_null(),
                            }
                        } else {
                            builder.append_null();
                        }
                    }
                    arrow_columns.push(Arc::new(builder.finish()));
                }
                DataType::Float => {
                    let mut builder = Float64Builder::with_capacity(num_rows);
                    for row in &rows {
                        if col_idx < row.data.len() {
                            match &row.data[col_idx] {
                                Value::Float(v) => builder.append_value(*v),
                                Value::Integer(v) => builder.append_value(*v as f64),
                                _ => builder.append_null(),
                            }
                        } else {
                            builder.append_null();
                        }
                    }
                    arrow_columns.push(Arc::new(builder.finish()));
                }
                DataType::String => {
                    let avg_len = 32usize;
                    let mut builder = StringBuilder::with_capacity(num_rows, num_rows * avg_len);
                    for row in &rows {
                        if col_idx < row.data.len() {
                            match &row.data[col_idx] {
                                Value::String(s) => builder.append_value(s.as_str()),
                                _ => builder.append_null(),
                            }
                        } else {
                            builder.append_null();
                        }
                    }
                    arrow_columns.push(Arc::new(builder.finish()));
                }
                DataType::Boolean => {
                    let mut builder = BooleanBuilder::with_capacity(num_rows);
                    for row in &rows {
                        if col_idx < row.data.len() {
                            match &row.data[col_idx] {
                                Value::Boolean(v) => builder.append_value(*v),
                                _ => builder.append_null(),
                            }
                        } else {
                            builder.append_null();
                        }
                    }
                    arrow_columns.push(Arc::new(builder.finish()));
                }
                DataType::Blob => {
                    let avg_len = 64usize;
                    let mut builder = BinaryBuilder::with_capacity(num_rows, num_rows * avg_len);
                    for row in &rows {
                        if col_idx < row.data.len() {
                            match &row.data[col_idx] {
                                Value::Blob(v) => builder.append_value(v),
                                _ => builder.append_null(),
                            }
                        } else {
                            builder.append_null();
                        }
                    }
                    arrow_columns.push(Arc::new(builder.finish()));
                }
            }
        }

        let schema = StdArc::new(ArrowSchema::new(arrow_fields));
        let batch = RecordBatch::try_new(schema.clone(), arrow_columns)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;

        let mut buffer = Vec::new();
        {
            let mut writer = FileWriter::try_new(&mut buffer, &schema)
                .map_err(|e| napi::Error::from_reason(e.to_string()))?;
            writer.write(&batch)
                .map_err(|e| napi::Error::from_reason(e.to_string()))?;
            writer.finish()
                .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        }

        Ok(Buffer::from(buffer))
    }

    /// Batch insert — flat row-major values. Much faster than per-row insert loops.
    /// `values` is a flat array: [row1col1, row1col2, row2col1, row2col2, ...]
    #[napi]
    pub fn insert_batch(
        &mut self,
        table: String,
        values: Vec<serde_json::Value>,
        num_columns: u32,
    ) -> Result<u32> {
        let db_values: Vec<Value> = values.into_iter().map(json_to_value).collect();
        let count = if num_columns > 0 { db_values.len() / num_columns as usize } else { 0 };
        if count == 0 { return Ok(0); }

        self.db.insert_batch_flat_values(&table, db_values, num_columns as usize)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        self.is_dirty.store(true, Ordering::Relaxed);
        Ok(count as u32)
    }

    /// Batch insert from interleaved i64 arrays.
    /// BigInt64Array interleaved: [row1col1, row1col2, row2col1, row2col2, ...]
    #[napi]
    pub fn insert_batch_i64(
        &mut self,
        table: String,
        values: BigInt64Array,
        num_columns: u32,
    ) -> Result<u32> {
        let count = if num_columns > 0 { values.len() / num_columns as usize } else { 0 };
        if count == 0 { return Ok(0); }

        let db_values: Vec<Value> = values.as_ref().iter().map(|&v| Value::Integer(v)).collect();
        self.db.insert_batch_flat_values(&table, db_values, num_columns as usize)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        self.is_dirty.store(true, Ordering::Relaxed);
        Ok(count as u32)
    }

    /// Batch insert from interleaved f64 arrays.
    #[napi]
    pub fn insert_batch_f64(
        &mut self,
        table: String,
        values: Float64Array,
        num_columns: u32,
    ) -> Result<u32> {
        let count = if num_columns > 0 { values.len() / num_columns as usize } else { 0 };
        if count == 0 { return Ok(0); }

        let db_values: Vec<Value> = values.as_ref().iter().map(|&v| Value::Float(v)).collect();
        self.db.insert_batch_flat_values(&table, db_values, num_columns as usize)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        self.is_dirty.store(true, Ordering::Relaxed);
        Ok(count as u32)
    }

    /// Batch insert from string arrays.
    #[napi]
    pub fn insert_batch_string(
        &mut self,
        table: String,
        values: Vec<String>,
        num_columns: u32,
    ) -> Result<u32> {
        let count = if num_columns > 0 { values.len() / num_columns as usize } else { 0 };
        if count == 0 { return Ok(0); }

        let db_values: Vec<Value> = values.into_iter().map(|v| Value::String(v.into())).collect();
        self.db.insert_batch_flat_values(&table, db_values, num_columns as usize)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        self.is_dirty.store(true, Ordering::Relaxed);
        Ok(count as u32)
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

fn db_to_arrow_type(dt: &DataType) -> arrow::datatypes::DataType {
    use arrow::datatypes::DataType as ArrowDataType;
    match dt {
        DataType::Integer => ArrowDataType::Int64,
        DataType::Float => ArrowDataType::Float64,
        DataType::String => ArrowDataType::Utf8,
        DataType::Boolean => ArrowDataType::Boolean,
        DataType::Blob => ArrowDataType::Binary,
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
