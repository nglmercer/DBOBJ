use dbobj::core::query_builder::QueryBuilder as CoreQueryBuilder;
use dbobj::core::Database as CoreDatabase;
use dbobj::{Value, Id, DataType};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::database::query::{db_value_to_json_no_table, db_value_to_json};
use crate::database::jv_helper;

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
    /// Avoids per-row JSON object overhead. Uses a direct table read for simple
    /// SELECT * (no filters, no joins, no order) — bypasses Row cloning entirely.
    #[napi]
    pub fn execute_columnar(&self) -> Result<serde_json::Value> {
        self.is_dirty.store(true, Ordering::Relaxed);

        let table_name = self.inner.table_name();
        if table_name.is_empty() {
            return Ok(serde_json::Value::Object(serde_json::Map::new()));
        }

        // Fast path: simple SELECT * — read directly from table.data, skip run()
        if self.inner.is_simple_select() {
            let tables_guard = self.db.tables.read();
            let table_lock = tables_guard.get(table_name)
                .ok_or_else(|| napi::Error::from_reason("Table not found"))?;
            let table = table_lock.read();

            let num_rows = table.ids.len();
            let mut map = serde_json::Map::with_capacity(table.schema.columns.len() + 1);

            // id column
            let ids: Vec<serde_json::Value> = table.ids.iter().map(|id| match id {
                Id::Integer(i) => serde_json::Value::Number((*i).into()),
                Id::String(s) => serde_json::Value::String(s.to_string()),
            }).collect();
            map.insert("id".into(), serde_json::Value::Array(ids));

            // data columns — read directly from table.data, skip Row creation
            // Use db_value_to_json (with table) to resolve InternedString
            for (col_idx, col_def) in table.schema.columns.iter().enumerate() {
                let mut col_values = Vec::with_capacity(num_rows);
                for row_idx in 0..num_rows {
                    let val = &table.data[row_idx * table.num_columns + col_idx];
                    col_values.push(db_value_to_json(val, &table));
                }
                map.insert(col_def.name.to_string(), serde_json::Value::Array(col_values));
            }

            return Ok(serde_json::Value::Object(map));
        }

        // Slow path: complex query with filters/joins — use run()
        let rows = self.inner.run(&self.db)
            .map_err(|e| napi::Error::from_reason(e))?;

        let tables_guard = self.db.tables.read();
        if table_name.is_empty() {
            return Ok(serde_json::Value::Object(serde_json::Map::new()));
        }
        let table_lock = tables_guard.get(table_name)
            .ok_or_else(|| napi::Error::from_reason("Table not found"))?;
        let table = table_lock.read();

        let mut map = serde_json::Map::with_capacity(table.schema.columns.len() + 1);

        let ids: Vec<serde_json::Value> = rows.iter().map(|row| match &row.id {
            Id::Integer(i) => serde_json::Value::Number((*i).into()),
            Id::String(s) => serde_json::Value::String(s.to_string()),
        }).collect();
        map.insert("id".into(), serde_json::Value::Array(ids));

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
    /// Uses a direct table read for simple SELECT * (no filters, no joins).
    #[napi]
    pub fn execute_arrow(&self) -> Result<Buffer> {
        use std::sync::Arc as StdArc;
        use arrow::array::*;
        use arrow::datatypes::{Field, Schema as ArrowSchema};
        use arrow::ipc::writer::FileWriter;
        use arrow::record_batch::RecordBatch;

        self.is_dirty.store(true, Ordering::Relaxed);

        let table_name = self.inner.table_name();
        if table_name.is_empty() {
            return Ok(Buffer::from(Vec::<u8>::new()));
        }

        // Fast path: simple SELECT * — read directly from table.data, skip run()
        if self.inner.is_simple_select() {
            let tables_guard = self.db.tables.read();
            let table_lock = tables_guard.get(table_name)
                .ok_or_else(|| napi::Error::from_reason("Table not found"))?;
            let table = table_lock.read();

            let num_rows = table.ids.len();
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
                        for row_idx in 0..num_rows {
                            match &table.data[row_idx * num_cols + col_idx] {
                                Value::Integer(v) => builder.append_value(*v),
                                _ => builder.append_null(),
                            }
                        }
                        arrow_columns.push(Arc::new(builder.finish()));
                    }
                    DataType::Float => {
                        let mut builder = Float64Builder::with_capacity(num_rows);
                        for row_idx in 0..num_rows {
                            match &table.data[row_idx * num_cols + col_idx] {
                                Value::Float(v) => builder.append_value(*v),
                                Value::Integer(v) => builder.append_value(*v as f64),
                                _ => builder.append_null(),
                            }
                        }
                        arrow_columns.push(Arc::new(builder.finish()));
                    }
                    DataType::String => {
                        let avg_len = 32usize;
                        let mut builder = StringBuilder::with_capacity(num_rows, num_rows * avg_len);
                        for row_idx in 0..num_rows {
                            match &table.data[row_idx * num_cols + col_idx] {
                                Value::String(s) => builder.append_value(s.as_str()),
                                Value::InternedString(id) => {
                                    if let Some(s) = table.string_pool.resolve(*id) {
                                        builder.append_value(s.as_str());
                                    } else {
                                        builder.append_null();
                                    }
                                }
                                _ => builder.append_null(),
                            }
                        }
                        arrow_columns.push(Arc::new(builder.finish()));
                    }
                    DataType::Boolean => {
                        let mut builder = BooleanBuilder::with_capacity(num_rows);
                        for row_idx in 0..num_rows {
                            match &table.data[row_idx * num_cols + col_idx] {
                                Value::Boolean(v) => builder.append_value(*v),
                                _ => builder.append_null(),
                            }
                        }
                        arrow_columns.push(Arc::new(builder.finish()));
                    }
                    DataType::Blob => {
                        let avg_len = 64usize;
                        let mut builder = BinaryBuilder::with_capacity(num_rows, num_rows * avg_len);
                        for row_idx in 0..num_rows {
                            match &table.data[row_idx * num_cols + col_idx] {
                                Value::Blob(v) => builder.append_value(v),
                                _ => builder.append_null(),
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

            return Ok(Buffer::from(buffer));
        }

        // Slow path: complex query with filters/joins — use run()
        let rows = self.inner.run(&self.db)
            .map_err(|e| napi::Error::from_reason(e))?;

        let tables_guard = self.db.tables.read();
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

    /// Insert from Arrow IPC buffer — zero-copy, no JSON serialization.
    /// Parses the RecordBatch and inserts all rows into the table.
    #[napi]
    pub fn insert_from_arrow(&mut self, table: String, buffer: Buffer) -> Result<u32> {
        use arrow::ipc::reader::FileReader;
        use std::io::Cursor;

        let cursor = Cursor::new(buffer.as_ref());
        let reader = FileReader::try_new(cursor, None)
            .map_err(|e| napi::Error::from_reason(format!("Failed to read Arrow IPC: {}", e)))?;

        let num_cols = reader.schema().fields().len();
        let mut total_rows = 0u32;

        for maybe_batch in reader {
            let batch = maybe_batch
                .map_err(|e| napi::Error::from_reason(format!("Failed to read batch: {}", e)))?;
            let num_rows = batch.num_rows();
            if num_rows == 0 { continue; }

            let mut flat: Vec<Value> = Vec::with_capacity(num_rows * num_cols);
            for row_idx in 0..num_rows {
                for col_idx in 0..num_cols {
                    let arr = batch.column(col_idx);
                    let val = arrow_value_to_db(arr, row_idx);
                    flat.push(val);
                }
            }

            self.db.insert_batch_flat_values(&table, flat, num_cols)
                .map_err(|e| napi::Error::from_reason(e.to_string()))?;
            total_rows += num_rows as u32;
        }

        self.is_dirty.store(true, Ordering::Relaxed);
        Ok(total_rows)
    }

    /// Update rows from Arrow IPC buffer — zero-copy, no JSON serialization.
    /// The Arrow data must include an "id" column to identify rows.
    /// Other columns in the Arrow buffer overwrite the corresponding table columns.
    /// Returns the number of rows updated.
    #[napi]
    pub fn update_from_arrow(&mut self, table: String, buffer: Buffer) -> Result<u32> {
        use arrow::ipc::reader::FileReader;
        use std::io::Cursor;

        let cursor = Cursor::new(buffer.as_ref());
        let reader = FileReader::try_new(cursor, None)
            .map_err(|e| napi::Error::from_reason(format!("Failed to read Arrow IPC: {}", e)))?;

        let arrow_schema = reader.schema();
        let fields = arrow_schema.fields();
        // Find the id column index and value column indices
        let mut id_col_idx = None;
        let mut value_cols: Vec<(usize, String)> = Vec::new();
        for (i, field) in fields.iter().enumerate() {
            if field.name() == "id" {
                id_col_idx = Some(i);
            } else {
                value_cols.push((i, field.name().to_string()));
            }
        }
        let id_col_idx = id_col_idx.ok_or_else(|| {
            napi::Error::from_reason("Arrow buffer must include an 'id' column for updates")
        })?;

        let mut total_updated = 0u32;

        for maybe_batch in reader {
            let batch = maybe_batch
                .map_err(|e| napi::Error::from_reason(format!("Failed to read batch: {}", e)))?;
            let num_rows = batch.num_rows();
            if num_rows == 0 { continue; }

            let id_arr = batch.column(id_col_idx);

            for row_idx in 0..num_rows {
                let id_val = arrow_value_to_db(id_arr, row_idx);

                // Build set_values map for this row
                let mut set_map: std::collections::HashMap<String, Value> = std::collections::HashMap::new();
                for &(col_idx, ref col_name) in &value_cols {
                    let val = arrow_value_to_db(batch.column(col_idx), row_idx);
                    set_map.insert(col_name.clone(), val);
                }

                // Convert id to dbobj Id
                let id = match id_val {
                    Value::Integer(i) => dbobj::Id::Integer(i as u64),
                    Value::Float(f) => dbobj::Id::Integer(f as u64),
                    Value::String(s) => dbobj::Id::String(s),
                    _ => continue,
                };

                // Read existing row, merge, update
                let tables_guard = self.db.tables.read();
                let table_lock = tables_guard.get(&table).ok_or_else(|| {
                    napi::Error::from_reason(format!("Table '{}' not found", table))
                })?;
                let table_read = table_lock.read();

                let row_idx_in_table = match table_read.get_index(&id) {
                    Some(idx) => idx,
                    None => continue, // row not found
                };

                let mut new_values = Vec::with_capacity(table_read.num_columns);
                for (col_idx, col_def) in table_read.schema.columns.iter().enumerate() {
                    let existing = &table_read.data[row_idx_in_table * table_read.num_columns + col_idx];
                    match set_map.get(col_def.name.as_str()) {
                        Some(val) => new_values.push(val.clone()),
                        None => new_values.push(existing.clone()),
                    }
                }
                drop(table_read);
                drop(tables_guard);

                self.db.update_values(&table, &id, new_values)
                    .map_err(|e| napi::Error::from_reason(e.to_string()))?;
                total_updated += 1;
            }
        }

        self.is_dirty.store(true, Ordering::Relaxed);
        Ok(total_updated)
    }

    /// Insert columnar — takes { colName: TypedArray | Array, ... }.
    /// Avoids both serde_json (for typed arrays) and Arrow IPC serialization.
    /// Column names are read from the table schema (order determined by schema).
    #[napi]
    pub fn insert_columnar(&mut self, _env: Env, table: String, columns: Object) -> Result<u32> {
        // Read table schema to get column order
        let tables_guard = self.db.tables.read();
        let table_lock = tables_guard.get(&table).ok_or_else(|| {
            napi::Error::from_reason(format!("Table '{}' not found", table))
        })?;
        let table_read = table_lock.read();
        let num_columns = table_read.num_columns;
        let col_names: Vec<String> = table_read.schema.columns.iter().map(|c| c.name.to_string()).collect();
        drop(table_read);
        drop(tables_guard);

        if num_columns == 0 { return Ok(0); }

        // Read column values
        enum ColSource {
            TypedI64(BigInt64Array),
            TypedF64(Float64Array),
            Generic(Array<'static>),
        }

        let mut sources: Vec<ColSource> = Vec::with_capacity(num_columns);
        let mut row_count = 0u32;

        for key in &col_names {
            let val: Unknown = columns.get_named_property(key)?;
            let len = if val.is_typedarray()? {
                if let Ok(arr) = unsafe { val.cast::<BigInt64Array>() } {
                    let l = arr.len() as u32;
                    sources.push(ColSource::TypedI64(arr));
                    l
                } else if let Ok(arr) = unsafe { val.cast::<Float64Array>() } {
                    let l = arr.len() as u32;
                    sources.push(ColSource::TypedF64(arr));
                    l
                } else {
                    return Err(napi::Error::from_reason("Unsupported TypedArray type"));
                }
            } else if val.is_array()? {
                let arr: Array = unsafe { val.cast()? };
                let l = arr.len();
                sources.push(ColSource::Generic(arr));
                l
            } else {
                return Err(napi::Error::from_reason("Column must be an array or TypedArray"));
            };

            if len == 0 { return Ok(0); }
            if row_count == 0 {
                row_count = len;
            } else if len != row_count {
                return Err(napi::Error::from_reason("Column lengths mismatch"));
            }
        }

        let total_cells = row_count as usize * num_columns;
        let mut flat_values: Vec<dbobj::Value> = Vec::with_capacity(total_cells);
        // Build row-major: for each row, push all column values
        for i in 0..row_count as usize {
            for source in &sources {
                match source {
                    ColSource::TypedI64(arr) => {
                        flat_values.push(dbobj::Value::Integer(arr[i]));
                    }
                    ColSource::TypedF64(arr) => {
                        flat_values.push(dbobj::Value::Float(arr[i]));
                    }
                    ColSource::Generic(arr) => {
                        let val: Unknown = Array::get_element(arr, i as u32)?;
                        flat_values.push(json_to_value(jv_helper(&val)?));
                    }
                }
            }
        }

        self.db.insert_batch_flat_values(&table, flat_values, num_columns)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        self.is_dirty.store(true, Ordering::Relaxed);
        Ok(row_count)
    }

    /// Update columnar — takes { id: TypedArray, colName: TypedArray | Array, ... }.
    /// Requires an "id" column to identify rows. All other columns are merged
    /// into the existing row data (column by column).
    /// Avoids both serde_json (for typed arrays) and Arrow IPC serialization.
    #[napi]
    pub fn update_columnar(&mut self, _env: Env, table: String, columns: Object) -> Result<u32> {
        // Read table schema
        let tables_guard = self.db.tables.read();
        let table_lock = tables_guard.get(&table).ok_or_else(|| {
            napi::Error::from_reason(format!("Table '{}' not found", table))
        })?;
        let table_read = table_lock.read();
        let num_columns = table_read.num_columns;
        let col_defs: Vec<(String, usize)> = table_read.schema.columns.iter().enumerate()
            .map(|(i, c)| (c.name.to_string(), i)).collect();
        drop(table_read);
        drop(tables_guard);

        if num_columns == 0 { return Ok(0); }

        // Determine which columns are provided and find "id"
        let keys = Object::keys(&columns)?;
        let _id_idx_in_input = keys.iter().position(|k| k == "id")
            .ok_or_else(|| napi::Error::from_reason("Columnar update requires an 'id' column"))?;
        let _value_keys: Vec<&String> = keys.iter().filter(|k| *k != "id").collect();

        // Read all column data
        enum ColValue {
            I64( BigInt64Array),
            F64(Float64Array),
            Generic(Vec<dbobj::Value>),
        }

        let mut id_data: Option<ColValue> = None;
        let mut value_data: std::collections::HashMap<String, ColValue> = std::collections::HashMap::new();
        let mut row_count = 0u32;

        for key in &keys {
            let key_str = key.as_str();
            let val: Unknown = columns.get_named_property(key)?;
            let (data, len) = if val.is_typedarray()? {
                if let Ok(arr) = unsafe { val.cast::<BigInt64Array>() } {
                    let l = arr.len() as u32;
                    (ColValue::I64(arr), l)
                } else if let Ok(arr) = unsafe { val.cast::<Float64Array>() } {
                    let l = arr.len() as u32;
                    (ColValue::F64(arr), l)
                } else {
                    return Err(napi::Error::from_reason("Unsupported TypedArray type"));
                }
            } else if val.is_array()? {
                let arr: Array = unsafe { val.cast()? };
                let l = arr.len();
                let mut vec = Vec::with_capacity(l as usize);
                for i in 0..l {
                    let elem: Unknown = Array::get_element(&arr, i)?;
                    vec.push(json_to_value(jv_helper(&elem)?));
                }
                (ColValue::Generic(vec), l)
            } else {
                return Err(napi::Error::from_reason("Column must be an array or TypedArray"));
            };

            if len == 0 { return Ok(0); }
            if row_count == 0 {
                row_count = len;
            } else if len != row_count {
                return Err(napi::Error::from_reason("Column lengths mismatch"));
            }

            if key_str == "id" {
                id_data = Some(data);
            } else {
                value_data.insert(key_str.to_string(), data);
            }
        }

        let id_data = id_data.unwrap();
        let mut total_updated = 0u32;

        for row_idx in 0..row_count as usize {
            // Get id
            let id_val = match &id_data {
                ColValue::I64(arr) => dbobj::Id::Integer(arr[row_idx] as u64),
                ColValue::F64(arr) => dbobj::Id::Integer(arr[row_idx] as u64),
                ColValue::Generic(v) => match &v[row_idx] {
                    dbobj::Value::Integer(i) => dbobj::Id::Integer(*i as u64),
                    dbobj::Value::Float(f) => dbobj::Id::Integer(*f as u64),
                    dbobj::Value::String(s) => dbobj::Id::String(s.clone()),
                    _ => continue,
                },
            };

            // Read existing row
            let tables_guard = self.db.tables.read();
            let table_lock = tables_guard.get(&table).ok_or_else(|| {
                napi::Error::from_reason(format!("Table '{}' not found", table))
            })?;
            let table_read = table_lock.read();

            let row_idx_in_table = match table_read.get_index(&id_val) {
                Some(idx) => idx,
                None => { continue; }
            };

            let mut new_values = Vec::with_capacity(num_columns);
            for (col_name, col_idx) in &col_defs {
                let existing = &table_read.data[row_idx_in_table * num_columns + col_idx];
                match value_data.get(col_name.as_str()) {
                    Some(val_source) => {
                        match val_source {
                            ColValue::I64(arr) => new_values.push(dbobj::Value::Integer(arr[row_idx])),
                            ColValue::F64(arr) => new_values.push(dbobj::Value::Float(arr[row_idx])),
                            ColValue::Generic(v) => new_values.push(v[row_idx].clone()),
                        }
                    }
                    None => new_values.push(existing.clone()),
                }
            }
            drop(table_read);
            drop(tables_guard);

            self.db.update_values(&table, &id_val, new_values)
                .map_err(|e| napi::Error::from_reason(e.to_string()))?;
            total_updated += 1;
        }

        self.is_dirty.store(true, Ordering::Relaxed);
        Ok(total_updated)
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

    /// Insert multiple rows from an array of JS objects.
    /// Uses the table schema to determine column order and types.
    /// Avoids manual columnar construction — schema-aware batch insert.
    #[napi]
    pub fn insert_from_objects(&mut self, table: String, objects: Vec<serde_json::Value>) -> Result<u32> {
        let num_rows = objects.len();
        if num_rows == 0 { return Ok(0); }

        let tables_guard = self.db.tables.read();
        let table_lock = tables_guard.get(&table).ok_or_else(|| {
            napi::Error::from_reason(format!("Table '{}' not found", table))
        })?;
        let table_read = table_lock.read();
        let num_columns = table_read.num_columns;
        if num_columns == 0 { return Ok(0); }
        // Pre-collect column names: must collect owned strings before dropping the read lock
        let col_names: Vec<(String, DataType)> = table_read.schema.columns.iter()
            .map(|c| (c.name.to_string(), c.data_type.clone()))
            .collect();
        drop(table_read);
        drop(tables_guard);

        let total_cells = num_rows * num_columns as usize;
        let mut flat: Vec<Value> = Vec::with_capacity(total_cells);

        for obj in &objects {
            let map = match obj {
                serde_json::Value::Object(m) => m,
                _ => return Err(napi::Error::from_reason("Each element must be a JSON object")),
            };
            for (col_name, _) in &col_names {
                match map.get(col_name.as_str()) {
                    Some(val) => flat.push(json_to_value(val.clone())),
                    None => flat.push(Value::Null),
                }
            }
        }

        self.db.insert_batch_flat_values(&table, flat, num_columns)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        self.is_dirty.store(true, Ordering::Relaxed);
        Ok(num_rows as u32)
    }

    /// Update multiple rows from an array of JS objects.
    /// Each object must include an "id" property to identify the row.
    /// Other properties are merged into the existing row data.
    /// Uses the table schema for column name resolution.
    #[napi]
    pub fn update_from_objects(&mut self, table: String, objects: Vec<serde_json::Value>) -> Result<u32> {
        let num_rows = objects.len();
        if num_rows == 0 { return Ok(0); }

        let tables_guard = self.db.tables.read();
        let table_lock = tables_guard.get(&table).ok_or_else(|| {
            napi::Error::from_reason(format!("Table '{}' not found", table))
        })?;
        let table_read = table_lock.read();
        let num_columns = table_read.num_columns;
        if num_columns == 0 { return Ok(0); }
        // Collect owned data before dropping read lock
        let col_defs: Vec<(String, DataType)> = table_read.schema.columns.iter()
            .map(|c| (c.name.to_string(), c.data_type.clone()))
            .collect();
        drop(table_read);
        drop(tables_guard);

        let mut total_updated = 0u32;

        for obj in &objects {
            let map = match obj {
                serde_json::Value::Object(m) => m,
                _ => continue,
            };

            // Extract id
            let id_val = match map.get("id") {
                Some(serde_json::Value::Number(n)) => {
                    if let Some(i) = n.as_i64() {
                        dbobj::Id::Integer(i as u64)
                    } else if let Some(f) = n.as_f64() {
                        dbobj::Id::Integer(f as u64)
                    } else {
                        continue;
                    }
                }
                Some(serde_json::Value::String(s)) => dbobj::Id::String(s.clone().into()),
                _ => continue,
            };

            // Build set map from remaining properties
            let mut set_map: std::collections::HashMap<&str, serde_json::Value> = std::collections::HashMap::new();
            for (key, val) in map.iter() {
                if key != "id" {
                    set_map.insert(key.as_str(), val.clone());
                }
            }

            // Read existing row, merge, update
            let tables_guard = self.db.tables.read();
            let table_lock = tables_guard.get(&table).ok_or_else(|| {
                napi::Error::from_reason(format!("Table '{}' not found", table))
            })?;
            let table_read = table_lock.read();

            let row_idx = match table_read.get_index(&id_val) {
                Some(idx) => idx,
                None => { continue; }
            };

            let mut new_values = Vec::with_capacity(num_columns);
            for (col_idx, (col_name, _)) in col_defs.iter().enumerate() {
                let existing = &table_read.data[row_idx * num_columns + col_idx];
                match set_map.get(col_name.as_str()) {
                    Some(val) => new_values.push(json_to_value(val.clone())),
                    None => new_values.push(existing.clone()),
                }
            }
            drop(table_read);
            drop(tables_guard);

            self.db.update_values(&table, &id_val, new_values)
                .map_err(|e| napi::Error::from_reason(e.to_string()))?;
            total_updated += 1;
        }

        self.is_dirty.store(true, Ordering::Relaxed);
        Ok(total_updated)
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

/// Convert an Arrow array value at a given index to a dbobj Value.
/// This avoids serde_json entirely — zero-copy for numeric types.
fn arrow_value_to_db(arr: &arrow::array::ArrayRef, idx: usize) -> Value {
    use arrow::array::*;
    use arrow::datatypes::DataType as ArrowDataType;
    match arr.data_type() {
        ArrowDataType::Int8 => {
            let a = arr.as_any().downcast_ref::<Int8Array>().unwrap();
            if a.is_null(idx) { Value::Null } else { Value::Integer(a.value(idx) as i64) }
        }
        ArrowDataType::Int16 => {
            let a = arr.as_any().downcast_ref::<Int16Array>().unwrap();
            if a.is_null(idx) { Value::Null } else { Value::Integer(a.value(idx) as i64) }
        }
        ArrowDataType::Int32 => {
            let a = arr.as_any().downcast_ref::<Int32Array>().unwrap();
            if a.is_null(idx) { Value::Null } else { Value::Integer(a.value(idx) as i64) }
        }
        ArrowDataType::Int64 => {
            let a = arr.as_any().downcast_ref::<Int64Array>().unwrap();
            if a.is_null(idx) { Value::Null } else { Value::Integer(a.value(idx)) }
        }
        ArrowDataType::UInt8 => {
            let a = arr.as_any().downcast_ref::<UInt8Array>().unwrap();
            if a.is_null(idx) { Value::Null } else { Value::Integer(a.value(idx) as i64) }
        }
        ArrowDataType::UInt16 => {
            let a = arr.as_any().downcast_ref::<UInt16Array>().unwrap();
            if a.is_null(idx) { Value::Null } else { Value::Integer(a.value(idx) as i64) }
        }
        ArrowDataType::UInt32 => {
            let a = arr.as_any().downcast_ref::<UInt32Array>().unwrap();
            if a.is_null(idx) { Value::Null } else { Value::Integer(a.value(idx) as i64) }
        }
        ArrowDataType::UInt64 => {
            let a = arr.as_any().downcast_ref::<UInt64Array>().unwrap();
            if a.is_null(idx) { Value::Null } else { Value::Integer(a.value(idx) as i64) }
        }
        ArrowDataType::Float32 => {
            let a = arr.as_any().downcast_ref::<Float32Array>().unwrap();
            if a.is_null(idx) { Value::Null } else { Value::Float(a.value(idx) as f64) }
        }
        ArrowDataType::Float64 => {
            let a = arr.as_any().downcast_ref::<Float64Array>().unwrap();
            if a.is_null(idx) { Value::Null } else { Value::Float(a.value(idx)) }
        }
        ArrowDataType::Utf8 | ArrowDataType::LargeUtf8 => {
            let a = arr.as_any().downcast_ref::<StringArray>().unwrap();
            if a.is_null(idx) { Value::Null } else { Value::String(compact_str::CompactString::from(a.value(idx))) }
        }
        ArrowDataType::Dictionary(_, value_type) => {
            // Handle dictionary-encoded strings (e.g. Utf8Dictionary from apache-arrow JS)
            match value_type.as_ref() {
                ArrowDataType::Utf8 | ArrowDataType::LargeUtf8 => {
                    if let Some(dict_arr) = arr.as_any().downcast_ref::<arrow::array::DictionaryArray<arrow::datatypes::Int32Type>>() {
                        if dict_arr.is_null(idx) { Value::Null } else {
                            let key = dict_arr.keys().value(idx);
                            let vals = dict_arr.values().as_any().downcast_ref::<StringArray>().unwrap();
                            Value::String(compact_str::CompactString::from(vals.value(key as usize)))
                        }
                    } else if let Some(dict_arr) = arr.as_any().downcast_ref::<arrow::array::DictionaryArray<arrow::datatypes::Int64Type>>() {
                        if dict_arr.is_null(idx) { Value::Null } else {
                            let key = dict_arr.keys().value(idx);
                            let vals = dict_arr.values().as_any().downcast_ref::<StringArray>().unwrap();
                            Value::String(compact_str::CompactString::from(vals.value(key as usize)))
                        }
                    } else if let Some(dict_arr) = arr.as_any().downcast_ref::<arrow::array::DictionaryArray<arrow::datatypes::UInt32Type>>() {
                        if dict_arr.is_null(idx) { Value::Null } else {
                            let key = dict_arr.keys().value(idx);
                            let vals = dict_arr.values().as_any().downcast_ref::<StringArray>().unwrap();
                            Value::String(compact_str::CompactString::from(vals.value(key as usize)))
                        }
                    } else {
                        Value::Null
                    }
                }
                _ => Value::Null,
            }
        }
        ArrowDataType::Boolean => {
            let a = arr.as_any().downcast_ref::<BooleanArray>().unwrap();
            if a.is_null(idx) { Value::Null } else { Value::Boolean(a.value(idx)) }
        }
        ArrowDataType::Binary | ArrowDataType::LargeBinary => {
            let a = arr.as_any().downcast_ref::<BinaryArray>().unwrap();
            if a.is_null(idx) { Value::Null } else { Value::Blob(a.value(idx).to_vec()) }
        }
        _ => Value::Null,
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
