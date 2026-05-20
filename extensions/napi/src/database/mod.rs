pub(crate) mod cursor;
pub(crate) mod error;
pub(crate) mod insert;
pub(crate) mod query;
pub(crate) mod query_builder;
pub(crate) mod schema;
pub(crate) mod update;

use crate::types::{ColumnDefinition, TableMetadata};
use dbobj::DataType;
use dbobj::Database as CoreDatabase;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::os::raw::c_char;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn arrow_to_db_type_napi(dt: &arrow::datatypes::DataType) -> Option<DataType> {
    match dt {
        arrow::datatypes::DataType::Int8
        | arrow::datatypes::DataType::Int16
        | arrow::datatypes::DataType::Int32
        | arrow::datatypes::DataType::Int64
        | arrow::datatypes::DataType::UInt8
        | arrow::datatypes::DataType::UInt16
        | arrow::datatypes::DataType::UInt32
        | arrow::datatypes::DataType::UInt64 => Some(DataType::Integer),
        arrow::datatypes::DataType::Float16
        | arrow::datatypes::DataType::Float32
        | arrow::datatypes::DataType::Float64 => Some(DataType::Float),
        arrow::datatypes::DataType::Utf8 | arrow::datatypes::DataType::LargeUtf8 => {
            Some(DataType::String)
        }
        arrow::datatypes::DataType::Boolean => Some(DataType::Boolean),
        arrow::datatypes::DataType::Binary
        | arrow::datatypes::DataType::LargeBinary
        | arrow::datatypes::DataType::FixedSizeBinary(_) => Some(DataType::Blob),
        _ => None,
    }
}

pub(crate) fn jv_helper(v: &Unknown) -> Result<serde_json::Value> {
    match v.get_type()? {
        napi::ValueType::Null | napi::ValueType::Undefined => Ok(serde_json::Value::Null),
        napi::ValueType::Boolean => Ok(serde_json::Value::Bool(unsafe { v.cast::<bool>()? })),
        napi::ValueType::Number => {
            let f = unsafe { v.cast::<f64>()? };
            if f.is_finite() {
                Ok(serde_json::Value::Number(
                    serde_json::Number::from_f64(f).unwrap_or(serde_json::Number::from(0)),
                ))
            } else {
                Ok(serde_json::Value::Null)
            }
        }
        napi::ValueType::String => Ok(serde_json::Value::String(unsafe { v.cast::<String>()? })),
        napi::ValueType::Object => {
            let obj = unsafe { v.cast::<Object>()? };
            if obj.is_array()? {
                let mut out = Vec::new();
                let len = obj.get_array_length()?;
                for i in 0..len {
                    out.push(jv_helper(&obj.get_element(i)?)?);
                }
                Ok(serde_json::Value::Array(out))
            } else {
                let mut map = serde_json::Map::new();
                let keys = Object::keys(&obj)?;
                for key in keys {
                    let val: Unknown = obj.get_named_property(&key)?;
                    map.insert(key, jv_helper(&val)?);
                }
                Ok(serde_json::Value::Object(map))
            }
        }
        _ => Ok(serde_json::Value::Null),
    }
}

pub(crate) fn json_to_db_value(val: Option<serde_json::Value>) -> dbobj::Value {
    match val {
        None => dbobj::Value::Null,
        Some(v) => match v {
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
            serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                // In dbobj, complex types can be stored as String (compact_str) if we want easy retrieval
                dbobj::Value::String(serde_json::to_string(&v).unwrap_or_default().into())
            }
        },
    }
}

pub(crate) fn object_to_db_row(
    obj: &Object,
    schema: &crate::dynamic_schema::CompiledSchema,
    keys: &[napi::JsString],
) -> Result<Vec<dbobj::Value>> {
    let mut row = Vec::with_capacity(schema.fields.len());
    for (i, field) in schema.fields.iter().enumerate() {
        let key = keys[i];
        match &field.type_ {
            crate::types::DataType::String => {
                let val: Option<String> = obj.get_property(key)?;
                match val {
                    Some(s) => row.push(dbobj::Value::String(s.into())),
                    None => {
                        if field.optional {
                            row.push(dbobj::Value::Null);
                        } else {
                            return Err(napi::Error::from_reason(format!(
                                "Missing required field: {}",
                                field.name
                            )));
                        }
                    }
                }
            }
            crate::types::DataType::Integer => {
                let val: Option<i64> = obj.get_property(key)?;
                match val {
                    Some(i) => row.push(dbobj::Value::Integer(i)),
                    None => {
                        if field.optional {
                            row.push(dbobj::Value::Null);
                        } else {
                            return Err(napi::Error::from_reason(format!(
                                "Missing required field: {}",
                                field.name
                            )));
                        }
                    }
                }
            }
            crate::types::DataType::Float => {
                let val: Option<f64> = obj.get_property(key)?;
                match val {
                    Some(f) => row.push(dbobj::Value::Float(f)),
                    None => {
                        if field.optional {
                            row.push(dbobj::Value::Null);
                        } else {
                            return Err(napi::Error::from_reason(format!(
                                "Missing required field: {}",
                                field.name
                            )));
                        }
                    }
                }
            }
            crate::types::DataType::Boolean => {
                let val: Option<bool> = obj.get_property(key)?;
                match val {
                    Some(b) => row.push(dbobj::Value::Boolean(b)),
                    None => {
                        if field.optional {
                            row.push(dbobj::Value::Null);
                        } else {
                            return Err(napi::Error::from_reason(format!(
                                "Missing required field: {}",
                                field.name
                            )));
                        }
                    }
                }
            }
            _ => {
                let val: Unknown = obj.get_property(key)?;
                let vtype = val.get_type()?;
                if vtype == napi::ValueType::Null || vtype == napi::ValueType::Undefined {
                    if field.optional {
                        row.push(dbobj::Value::Null);
                    } else {
                        return Err(napi::Error::from_reason(format!(
                            "Missing required field: {}",
                            field.name
                        )));
                    }
                } else {
                    let json_val = jv_helper(&val)?;
                    row.push(json_to_db_value(Some(json_val)));
                }
            }
        }
    }
    Ok(row)
}

#[napi]
pub struct Database {
    pub(crate) inner: Arc<CoreDatabase>,
    pub(crate) path: Option<String>,
    pub(crate) is_dirty: Arc<AtomicBool>,
}

#[napi]
pub struct Transaction {
    db: Arc<CoreDatabase>,
    original_tables: std::collections::HashMap<String, dbobj::core::table::Table>,
}

#[napi]
impl Transaction {
    #[napi]
    pub fn commit(&self) -> bool {
        true
    }

    #[napi]
    pub fn rollback(&self) -> bool {
        let mut tables = self.db.tables.write();
        for (name, table) in &self.original_tables {
            tables.insert(
                name.clone(),
                Arc::new(parking_lot::RwLock::new(table.clone())),
            );
        }
        true
    }
}

#[napi]
pub struct PreparedStatement {
    pub(crate) inner: dbobj_sql::PreparedStatement,
    pub(crate) db: Arc<CoreDatabase>,
    pub(crate) is_dirty: Arc<AtomicBool>,
    pub(crate) params: Option<Vec<dbobj::Value>>,
}

#[napi]
impl PreparedStatement {
    #[napi]
    pub fn run(&self, params: Option<Vec<i64>>) -> Result<bool> {
        let executor = dbobj_sql::SqlExecutor::new(&self.db);
        let vals = if let Some(p) = params {
            p.into_iter().map(dbobj::Value::Integer).collect()
        } else if let Some(p) = &self.params {
            p.clone()
        } else {
            vec![]
        };
        executor
            .execute_prepared(&self.inner, &vals)
            .map_err(napi::Error::from_reason)?;
        self.is_dirty.store(true, Ordering::Relaxed);
        Ok(true)
    }

    #[napi]
    pub fn all_i64(&self, _params: Option<Vec<i64>>) -> Result<BigInt64Array> {
        let stmt = &self.inner;
        if stmt.statements.len() == 1 {
            if let dbobj_sql::local_parser::Statement::Select {
                columns: dbobj_sql::local_parser::SelectColumns::List(cols),
                table,
                selection: _,
                join,
                ..
            } = &stmt.statements[0]
            {
                if cols.len() == 1 && join.is_none() {
                    let table_name = table.to_string();
                    let table_lock = self.db.get_table(&table_name).ok_or_else(|| {
                        napi::Error::from_reason(format!("Table {} not found", table_name))
                    })?;
                    let table_ref = table_lock.read();
                    let col_name = match &cols[0].expr {
                        dbobj_sql::local_parser::Expr::Column(c) => c.as_str(),
                        _ => {
                            return Err(napi::Error::from_reason(
                                "allI64 requires a simple column reference".to_string(),
                            ))
                        }
                    };
                    let col_idx = *table_ref.column_map.get(col_name).ok_or_else(|| {
                        napi::Error::from_reason(format!("Column {} not found", col_name))
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
        Err(napi::Error::from_reason("Query not suitable for all_i64"))
    }

    #[napi]
    pub fn run_batch(&self, batch_params: Vec<Vec<i64>>) -> Result<bool> {
        let executor = dbobj_sql::SqlExecutor::new(&self.db);
        let batch: Vec<Vec<dbobj::Value>> = batch_params
            .into_iter()
            .map(|params| params.into_iter().map(dbobj::Value::Integer).collect())
            .collect();
        executor
            .execute_prepared_batch(&self.inner, &batch)
            .map_err(napi::Error::from_reason)?;
        self.is_dirty.store(true, Ordering::Relaxed);
        Ok(true)
    }

    #[napi]
    pub fn run_batch_values(
        &self,
        flat_params: Vec<Option<serde_json::Value>>,
        params_per_row: u32,
    ) -> Result<bool> {
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
                    None => {
                        batch.push(row);
                        break 'outer;
                    }
                }
            }
            batch.push(row);
        }
        executor
            .execute_prepared_batch(&self.inner, &batch)
            .map_err(napi::Error::from_reason)?;
        self.is_dirty.store(true, Ordering::Relaxed);
        Ok(true)
    }

    #[napi]
    pub fn run_batch_i64(&self, flat_params: BigInt64Array, params_per_row: u32) -> Result<bool> {
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
        Ok(true)
    }

    #[napi]
    pub fn run_batch_string(&self, flat_params: Vec<String>, params_per_row: u32) -> Result<bool> {
        let executor = dbobj_sql::SqlExecutor::new(&self.db);
        let pprow = params_per_row as usize;
        let total = flat_params.len();
        let mut iter = flat_params.into_iter();
        let mut batch = Vec::with_capacity(total / pprow);
        'outer: while let Some(v0) = iter.next() {
            let mut row = Vec::with_capacity(pprow);
            row.push(dbobj::Value::String(v0.into()));
            for _ in 1..pprow {
                match iter.next() {
                    Some(v) => row.push(dbobj::Value::String(v.into())),
                    None => {
                        batch.push(row);
                        break 'outer;
                    }
                }
            }
            batch.push(row);
        }
        executor
            .execute_prepared_batch(&self.inner, &batch)
            .map_err(napi::Error::from_reason)?;
        self.is_dirty.store(true, Ordering::Relaxed);
        Ok(true)
    }

    #[napi]
    pub fn run_batch_bool(&self, flat_params: Vec<bool>, params_per_row: u32) -> Result<bool> {
        let executor = dbobj_sql::SqlExecutor::new(&self.db);
        let pprow = params_per_row as usize;
        let total = flat_params.len();
        let mut iter = flat_params.into_iter();
        let mut batch = Vec::with_capacity(total / pprow);
        'outer: while let Some(v0) = iter.next() {
            let mut row = Vec::with_capacity(pprow);
            row.push(dbobj::Value::Boolean(v0));
            for _ in 1..pprow {
                match iter.next() {
                    Some(v) => row.push(dbobj::Value::Boolean(v)),
                    None => {
                        batch.push(row);
                        break 'outer;
                    }
                }
            }
            batch.push(row);
        }
        executor
            .execute_prepared_batch(&self.inner, &batch)
            .map_err(napi::Error::from_reason)?;
        self.is_dirty.store(true, Ordering::Relaxed);
        Ok(true)
    }

    #[napi]
    pub fn all(&self, params: Option<Vec<Option<serde_json::Value>>>) -> Result<serde_json::Value> {
        let executor = dbobj_sql::SqlExecutor::new(&self.db);
        let vals = if let Some(p) = params {
            p.into_iter().map(json_to_db_value).collect()
        } else if let Some(p) = &self.params {
            p.clone()
        } else {
            vec![]
        };
        let result = executor
            .execute_prepared(&self.inner, &vals)
            .map_err(napi::Error::from_reason)?;

        match result {
            dbobj_sql::SqlResult::Rows(rows) => {
                let mut results = Vec::with_capacity(rows.len());
                for row in rows {
                    let mut map = serde_json::Map::with_capacity(row.len());
                    for (k, v) in row {
                        map.insert(k.to_string(), query::db_value_to_json_no_table(&v));
                    }
                    results.push(serde_json::Value::Object(map));
                }
                Ok(serde_json::Value::Array(results))
            }
            dbobj_sql::SqlResult::Ok => Ok(serde_json::Value::Array(vec![])),
            dbobj_sql::SqlResult::I64(vals) => {
                let results = vals
                    .into_iter()
                    .map(|i| serde_json::Value::Number(i.into()))
                    .collect();
                Ok(serde_json::Value::Array(results))
            }
        }
    }

    #[napi]
    pub fn get(
        &self,
        params: Option<Vec<Option<serde_json::Value>>>,
    ) -> Result<Option<serde_json::Value>> {
        let rows = self.all(params)?;
        if let serde_json::Value::Array(mut arr) = rows {
            if !arr.is_empty() {
                return Ok(Some(arr.remove(0)));
            }
        }
        Ok(None)
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
    pub fn create_table_from_schema(
        &self,
        table_name: String,
        dynamic_schema: &crate::DynamicSchema,
        schema_name: String,
    ) -> Result<bool> {
        let schema = dynamic_schema.get_schema(&schema_name).ok_or_else(|| {
            napi::Error::from_reason(format!("Schema '{}' not found", schema_name))
        })?;

        let mut has_id = false;
        let schema_columns: Vec<dbobj::ColumnDefinition> = schema
            .fields
            .iter()
            .map(|f| {
                if f.name == "id" {
                    has_id = true;
                }
                let data_type = match f.type_ {
                    crate::types::DataType::String => dbobj::DataType::String,
                    crate::types::DataType::Integer => dbobj::DataType::Integer,
                    crate::types::DataType::Float => dbobj::DataType::Float,
                    crate::types::DataType::Boolean => dbobj::DataType::Boolean,
                    _ => dbobj::DataType::Blob, // Fallback for Json, Arrays, Blob
                };
                dbobj::ColumnDefinition {
                    name: f.name.clone().into(),
                    data_type,
                    nullable: f.optional,
                }
            })
            .collect();

        self.inner.create_table(
            table_name.clone(),
            dbobj::Schema {
                columns: schema_columns,
            },
        );
        if has_id {
            let _ = self.inner.create_unique_index(&table_name, "id");
        }
        self.save_if_needed();
        Ok(true)
    }

    #[napi]
    pub fn create_table(&self, name: String, columns: Vec<ColumnDefinition>) -> Result<bool> {
        use dbobj::Schema;
        let mut has_id = false;
        let schema_columns: Vec<dbobj::ColumnDefinition> = columns
            .into_iter()
            .map(|col| {
                if col.name == "id" {
                    has_id = true;
                }
                let data_type = match col.data_type {
                    crate::types::DataType::Integer => dbobj::DataType::Integer,
                    crate::types::DataType::Float => dbobj::DataType::Float,
                    crate::types::DataType::String => dbobj::DataType::String,
                    crate::types::DataType::Boolean => dbobj::DataType::Boolean,
                    crate::types::DataType::Blob => dbobj::DataType::Blob,
                    _ => dbobj::DataType::Blob,
                };
                Ok(dbobj::ColumnDefinition {
                    name: col.name.into(),
                    data_type,
                    nullable: col.nullable.unwrap_or(true),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.inner.create_table(
            name.clone(),
            Schema {
                columns: schema_columns,
            },
        );
        if has_id {
            let _ = self.inner.create_unique_index(&name, "id");
        }
        self.save_if_needed();
        Ok(true)
    }

    // ── INSERT ───────────────────────────────────────────────────────

    #[napi]
    pub fn insert_batch_i64(
        &self,
        table_name: String,
        values: BigInt64Array,
        num_columns: u32,
    ) -> Result<bool> {
        insert::insert_batch_i64(self, table_name, values.as_ref(), num_columns as usize)
    }

    #[napi]
    pub fn insert_row_i64(&self, table_name: String, values: Vec<i64>) -> Result<bool> {
        insert::insert_row_i64(self, table_name, values)
    }

    #[napi]
    pub fn insert_row_string(&self, table_name: String, values: Vec<String>) -> Result<bool> {
        insert::insert_row_string(self, table_name, values)
    }

    #[napi]
    pub fn insert_row_bool(&self, table_name: String, values: Vec<bool>) -> Result<bool> {
        insert::insert_row_bool(self, table_name, values)
    }

    #[napi]
    pub fn insert_row_float(&self, table_name: String, values: Vec<f64>) -> Result<bool> {
        insert::insert_row_float(self, table_name, values)
    }

    #[napi]
    pub fn insert_object(
        &self,
        env: Env,
        table_name: String,
        obj: Object,
        dynamic_schema: &crate::DynamicSchema,
        schema_name: String,
    ) -> Result<bool> {
        let schema = dynamic_schema.get_schema(&schema_name).ok_or_else(|| {
            napi::Error::from_reason(format!("Schema '{}' not found", schema_name))
        })?;
        let keys: Vec<napi::JsString> = schema
            .fields
            .iter()
            .map(|f| env.create_string(&f.name))
            .collect::<Result<Vec<_>>>()?;
        let row_values = object_to_db_row(&obj, schema, &keys)?;
        self.inner
            .insert_values(&table_name, row_values)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        self.save_if_needed();
        Ok(true)
    }

    #[napi]
    pub fn insert_row(
        &self,
        table_name: String,
        values: Vec<Option<serde_json::Value>>,
    ) -> Result<bool> {
        insert::insert_row(self, table_name, values)
    }

    #[napi]
    pub fn insert_or_replace(
        &self,
        table_name: String,
        values: Vec<Option<serde_json::Value>>,
        unique_column: String,
    ) -> Result<bool> {
        insert::insert_or_replace(self, table_name, values, unique_column)
    }

    #[napi]
    pub fn insert_batch_string(
        &self,
        table_name: String,
        values: Vec<String>,
        num_columns: u32,
    ) -> Result<bool> {
        insert::insert_batch_string(self, table_name, values, num_columns)
    }

    #[napi]
    pub fn insert_batch_bool(
        &self,
        table_name: String,
        values: Vec<bool>,
        num_columns: u32,
    ) -> Result<bool> {
        insert::insert_batch_bool(self, table_name, values, num_columns)
    }

    #[napi]
    pub fn insert_batch_float(
        &self,
        table_name: String,
        values: Vec<f64>,
        num_columns: u32,
    ) -> Result<bool> {
        insert::insert_batch_float(self, table_name, values, num_columns)
    }

    #[napi]
    pub fn insert_batch_objects(
        &self,
        env: Env,
        table_name: String,
        objects: Array,
        dynamic_schema: &crate::DynamicSchema,
        schema_name: String,
    ) -> Result<bool> {
        let schema = dynamic_schema.get_schema(&schema_name).ok_or_else(|| {
            napi::Error::from_reason(format!("Schema '{}' not found", schema_name))
        })?;

        let num_fields = schema.fields.len();
        let len = objects.len();
        let mut flat_values = Vec::with_capacity(len as usize * num_fields);
        let mut string_buf = Vec::with_capacity(1024);

        // Pre-create JsString keys
        let keys: Vec<napi::sys::napi_value> = schema
            .fields
            .iter()
            .map(|f| {
                let mut js_string = std::ptr::null_mut();
                let status = unsafe {
                    napi::sys::napi_create_string_utf8(
                        env.raw(),
                        f.name.as_ptr() as *const c_char,
                        f.name.len() as isize,
                        &mut js_string,
                    )
                };
                if status != napi::sys::Status::napi_ok {
                    return Err(napi::Error::from_reason("Failed to create N-API string"));
                }
                Ok(js_string)
            })
            .collect::<Result<Vec<_>>>()?;

        for i in 0..len {
            let mut obj_ptr = std::ptr::null_mut();
            unsafe {
                napi::sys::napi_get_element(env.raw(), objects.raw(), i, &mut obj_ptr);
            }

            for (j, field) in schema.fields.iter().enumerate() {
                let mut val_ptr = std::ptr::null_mut();
                unsafe {
                    napi::sys::napi_get_property(env.raw(), obj_ptr, keys[j], &mut val_ptr);
                }

                let val = unsafe { Unknown::from_raw_unchecked(env.raw(), val_ptr) };
                let vtype = val.get_type()?;

                if vtype == napi::ValueType::Null || vtype == napi::ValueType::Undefined {
                    if field.optional {
                        flat_values.push(dbobj::Value::Null);
                    } else {
                        return Err(napi::Error::from_reason(format!(
                            "Missing required field: {}",
                            field.name
                        )));
                    }
                    continue;
                }

                match &field.type_ {
                    crate::types::DataType::Integer => {
                        let mut i = 0i64;
                        unsafe {
                            napi::sys::napi_get_value_int64(env.raw(), val_ptr, &mut i);
                        }
                        flat_values.push(dbobj::Value::Integer(i));
                    }
                    crate::types::DataType::String => {
                        let mut written = 0;
                        unsafe {
                            napi::sys::napi_get_value_string_utf8(
                                env.raw(),
                                val_ptr,
                                std::ptr::null_mut(),
                                0,
                                &mut written,
                            );
                        }
                        if written > 0 {
                            if string_buf.capacity() < written + 1 {
                                string_buf.reserve(written + 1 - string_buf.capacity());
                            }
                            unsafe {
                                string_buf.set_len(written + 1);
                                napi::sys::napi_get_value_string_utf8(
                                    env.raw(),
                                    val_ptr,
                                    string_buf.as_mut_ptr() as *mut c_char,
                                    written + 1,
                                    &mut written,
                                );
                                string_buf.set_len(written);
                            }
                            let s = unsafe { std::str::from_utf8_unchecked(&string_buf) };
                            flat_values.push(dbobj::Value::String(s.into()));
                        } else {
                            flat_values.push(dbobj::Value::String("".into()));
                        }
                    }
                    crate::types::DataType::Float => {
                        let mut f = 0.0f64;
                        unsafe {
                            napi::sys::napi_get_value_double(env.raw(), val_ptr, &mut f);
                        }
                        flat_values.push(dbobj::Value::Float(f));
                    }
                    crate::types::DataType::Boolean => {
                        let mut b = false;
                        unsafe {
                            napi::sys::napi_get_value_bool(env.raw(), val_ptr, &mut b);
                        }
                        flat_values.push(dbobj::Value::Boolean(b));
                    }
                    _ => {
                        let json_val = jv_helper(&val)?;
                        flat_values.push(json_to_db_value(Some(json_val)));
                    }
                }
            }
        }

        self.inner
            .insert_batch_flat_values(&table_name, flat_values, num_fields)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        self.save_if_needed();
        Ok(true)
    }

    #[napi]
    pub fn insert_batch(
        &self,
        table_name: String,
        values: Vec<Option<serde_json::Value>>,
        num_columns: u32,
    ) -> Result<bool> {
        insert::insert_batch(self, table_name, values, num_columns)
    }

    #[napi]
    pub fn insert_batch_columnar(
        &self,
        _env: Env,
        table_name: String,
        columns: Object,
    ) -> Result<bool> {
        let keys = Object::keys(&columns)?;
        if keys.is_empty() {
            return Ok(true);
        }

        let num_columns = keys.len();
        let mut row_count = 0;

        enum ColSource<'a> {
            TypedI64(BigInt64Array),
            TypedF64(Float64Array),
            Generic(Array<'a>),
        }

        let mut sources = Vec::with_capacity(num_columns as usize);

        for key in &keys {
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
                return Err(napi::Error::from_reason(
                    "Column must be an array or TypedArray",
                ));
            };

            if row_count == 0 {
                row_count = len;
            } else if len != row_count {
                return Err(napi::Error::from_reason("Column lengths mismatch"));
            }
        }

        let total_cells = row_count as usize * num_columns as usize;
        let mut flat_values = vec![dbobj::Value::Null; total_cells];
        for (j, source) in sources.iter().enumerate() {
            match source {
                ColSource::TypedI64(arr) => {
                    for i in 0..row_count {
                        flat_values[i as usize * num_columns as usize + j] =
                            dbobj::Value::Integer(arr[i as usize]);
                    }
                }
                ColSource::TypedF64(arr) => {
                    for i in 0..row_count {
                        flat_values[i as usize * num_columns as usize + j] =
                            dbobj::Value::Float(arr[i as usize]);
                    }
                }
                ColSource::Generic(arr) => {
                    for i in 0..row_count {
                        let val: Unknown = Array::get_element(arr, i)?;
                        let idx = i as usize * num_columns as usize + j;
                        flat_values[idx] = match val.get_type()? {
                            napi::ValueType::Boolean => {
                                dbobj::Value::Boolean(unsafe { val.cast::<bool>()? })
                            }
                            napi::ValueType::String => {
                                let s: String = unsafe { val.cast::<String>()? };
                                dbobj::Value::String(s.into())
                            }
                            napi::ValueType::Number => {
                                let f: f64 = unsafe { val.cast::<f64>()? };
                                if f.is_finite()
                                    && f.fract() == 0.0
                                    && f >= i64::MIN as f64
                                    && f <= i64::MAX as f64
                                {
                                    dbobj::Value::Integer(f as i64)
                                } else if f.is_finite() {
                                    dbobj::Value::Float(f)
                                } else {
                                    dbobj::Value::Null
                                }
                            }
                            _ => json_to_db_value(Some(jv_helper(&val)?)),
                        };
                    }
                }
            }
        }

        self.inner
            .insert_batch_flat_values(&table_name, flat_values, num_columns as usize)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;

        self.save_if_needed();
        Ok(true)
    }

    // ── UPDATE ───────────────────────────────────────────────────────

    #[napi]
    pub fn update_row_i64(&self, table_name: String, id: u32, values: Vec<i64>) -> Result<bool> {
        update::update_row_i64(self, table_name, id, values)
    }

    #[napi]
    pub fn update_row_string(
        &self,
        table_name: String,
        id: u32,
        values: Vec<String>,
    ) -> Result<bool> {
        update::update_row_string(self, table_name, id, values)
    }

    #[napi]
    pub fn update_row_bool(&self, table_name: String, id: u32, values: Vec<bool>) -> Result<bool> {
        update::update_row_bool(self, table_name, id, values)
    }

    #[napi]
    pub fn update_row_float(&self, table_name: String, id: u32, values: Vec<f64>) -> Result<bool> {
        update::update_row_float(self, table_name, id, values)
    }

    #[napi]
    pub fn update_object(
        &self,
        env: Env,
        table_name: String,
        id: u32,
        obj: Object,
        dynamic_schema: &crate::DynamicSchema,
        schema_name: String,
    ) -> Result<bool> {
        let schema = dynamic_schema.get_schema(&schema_name).ok_or_else(|| {
            napi::Error::from_reason(format!("Schema '{}' not found", schema_name))
        })?;

        let num_fields = schema.fields.len();
        let mut row_values = Vec::with_capacity(num_fields);

        // Pre-create JsString keys
        let keys: Vec<napi::sys::napi_value> = schema
            .fields
            .iter()
            .map(|f| {
                let mut js_string = std::ptr::null_mut();
                unsafe {
                    napi::sys::napi_create_string_utf8(
                        env.raw(),
                        f.name.as_ptr() as *const c_char,
                        f.name.len() as isize,
                        &mut js_string,
                    );
                }
                js_string
            })
            .collect();

        for (j, field) in schema.fields.iter().enumerate() {
            let mut val_ptr = std::ptr::null_mut();
            let status = unsafe {
                napi::sys::napi_get_property(env.raw(), obj.raw(), keys[j], &mut val_ptr)
            };
            if status != napi::sys::Status::napi_ok {
                return Err(napi::Error::from_reason(
                    "Failed to get property from object",
                ));
            }

            let val = unsafe { Unknown::from_raw_unchecked(env.raw(), val_ptr) };
            let vtype = val.get_type()?;

            if vtype == napi::ValueType::Null || vtype == napi::ValueType::Undefined {
                if field.optional {
                    row_values.push(dbobj::Value::Null);
                } else {
                    return Err(napi::Error::from_reason(format!(
                        "Missing required field: {}",
                        field.name
                    )));
                }
                continue;
            }

            match &field.type_ {
                crate::types::DataType::Integer => {
                    let i: i64 = unsafe { val.cast::<i64>()? };
                    row_values.push(dbobj::Value::Integer(i));
                }
                crate::types::DataType::String => {
                    let s: String = unsafe { val.cast::<String>()? };
                    row_values.push(dbobj::Value::String(s.into()));
                }
                crate::types::DataType::Float => {
                    let f: f64 = unsafe { val.cast::<f64>()? };
                    row_values.push(dbobj::Value::Float(f));
                }
                crate::types::DataType::Boolean => {
                    let b: bool = unsafe { val.cast::<bool>()? };
                    row_values.push(dbobj::Value::Boolean(b));
                }
                _ => {
                    let json_val = jv_helper(&val)?;
                    row_values.push(json_to_db_value(Some(json_val)));
                }
            }
        }

        self.inner
            .update_values(&table_name, &dbobj::Id::Integer(id as u64), row_values)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        self.save_if_needed();
        Ok(true)
    }

    #[napi]
    pub fn update_row(
        &self,
        table_name: String,
        id: u32,
        values: Vec<Option<serde_json::Value>>,
    ) -> Result<bool> {
        update::update_row(self, table_name, id, values)
    }

    #[napi]
    pub fn update_column_i64(
        &self,
        table_name: String,
        id: u32,
        column_name: String,
        value: i64,
    ) -> Result<bool> {
        update::update_column_i64(self, table_name, id, column_name, value)
    }

    #[napi]
    pub fn update_column_string(
        &self,
        table_name: String,
        id: u32,
        column_name: String,
        value: String,
    ) -> Result<bool> {
        update::update_column_string(self, table_name, id, column_name, value)
    }

    #[napi]
    pub fn update_column_bool(
        &self,
        table_name: String,
        id: u32,
        column_name: String,
        value: bool,
    ) -> Result<bool> {
        update::update_column_bool(self, table_name, id, column_name, value)
    }

    #[napi]
    pub fn update_column_float(
        &self,
        table_name: String,
        id: u32,
        column_name: String,
        value: f64,
    ) -> Result<bool> {
        update::update_column_float(self, table_name, id, column_name, value)
    }

    #[napi]
    pub fn update_batch_i64(
        &self,
        table_name: String,
        column_name: String,
        values: BigInt64Array,
    ) -> Result<bool> {
        update::update_batch_i64(self, table_name, column_name, values.as_ref())
    }

    #[napi]
    pub fn delete_row(&self, table_name: String, id: u32) -> Result<bool> {
        update::delete_row(self, table_name, id)
    }

    #[napi]
    pub fn delete_by_column_i64(
        &self,
        table_name: String,
        column_name: String,
        value: i64,
    ) -> Result<u32> {
        update::delete_by_column_i64(self, table_name, column_name, value)
    }

    #[napi]
    pub fn delete_by_column_string(
        &self,
        table_name: String,
        column_name: String,
        value: String,
    ) -> Result<u32> {
        update::delete_by_column_string(self, table_name, column_name, value)
    }

    #[napi]
    pub fn delete_by_column_bool(
        &self,
        table_name: String,
        column_name: String,
        value: bool,
    ) -> Result<u32> {
        update::delete_by_column_bool(self, table_name, column_name, value)
    }

    #[napi]
    pub fn delete_batch_i64(&self, table_name: String, ids: BigInt64Array) -> Result<u32> {
        update::delete_batch_i64(self, table_name, ids.as_ref())
    }

    // ── QUERY ────────────────────────────────────────────────────────

    #[napi]
    pub fn get_column_i64(
        &self,
        table_name: String,
        column_name: String,
        _env: Env,
    ) -> Result<BigInt64Array> {
        query::get_column_i64(self, table_name, column_name)
    }

    #[napi]
    pub fn find_by_i64(
        &self,
        table_name: String,
        column_name: String,
        value: i64,
    ) -> Result<BigInt64Array> {
        query::find_by_i64(self, table_name, column_name, value)
    }

    #[napi]
    pub fn find_by_string(
        &self,
        table_name: String,
        column_name: String,
        value: String,
    ) -> Result<BigInt64Array> {
        query::find_by_string(self, table_name, column_name, value)
    }

    #[napi]
    pub fn find_by_bool(
        &self,
        table_name: String,
        column_name: String,
        value: bool,
    ) -> Result<BigInt64Array> {
        query::find_by_bool(self, table_name, column_name, value)
    }

    #[napi]
    pub fn get_column_string(
        &self,
        table_name: String,
        column_name: String,
    ) -> Result<Vec<String>> {
        query::get_column_string(self, table_name, column_name)
    }

    #[napi]
    pub fn get_column_bool(&self, table_name: String, column_name: String) -> Result<Vec<bool>> {
        query::get_column_bool(self, table_name, column_name)
    }

    #[napi]
    pub fn get_column_float(&self, table_name: String, column_name: String) -> Result<Vec<f64>> {
        query::get_column_float(self, table_name, column_name)
    }

    #[napi]
    pub fn hash_join_i64(
        &self,
        table1: String,
        col1: String,
        table2: String,
        col2: String,
    ) -> Result<BigInt64Array> {
        query::hash_join_i64(self, table1, col1, table2, col2)
    }

    #[napi]
    pub fn get_rows(
        &self,
        table_name: String,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<serde_json::Value> {
        query::get_rows(self, table_name, limit, offset)
    }

    #[napi]
    pub fn get_row_by_id(&self, table_name: String, id: u32) -> Result<Option<serde_json::Value>> {
        query::get_row_by_id(self, table_name, id)
    }

    #[napi]
    pub fn get_row_by_column_i64(
        &self,
        table_name: String,
        column_name: String,
        value: i64,
    ) -> Result<Option<serde_json::Value>> {
        query::get_row_by_column_i64(self, table_name, column_name, value)
    }

    #[napi]
    pub fn get_row_by_column_string(
        &self,
        table_name: String,
        column_name: String,
        value: String,
    ) -> Result<Option<serde_json::Value>> {
        query::get_row_by_column_string(self, table_name, column_name, value)
    }

    #[napi]
    pub fn get_row_by_column_bool(
        &self,
        table_name: String,
        column_name: String,
        value: bool,
    ) -> Result<Option<serde_json::Value>> {
        query::get_row_by_column_bool(self, table_name, column_name, value)
    }

    #[napi]
    pub async fn get_rows_async(
        &self,
        table_name: String,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<serde_json::Value> {
        let inner = self.inner.clone();
        napi::tokio::task::spawn_blocking(move || {
            let table_lock = inner
                .get_table(&table_name)
                .ok_or_else(|| format!("Table {} not found", table_name))?;
            let table = table_lock.read();

            let num_rows = table.ids.len();
            let start = offset.unwrap_or(0) as usize;
            let count = limit.unwrap_or(u32::MAX) as usize;
            if start >= num_rows {
                return Ok(serde_json::Value::Array(Vec::new()));
            }
            let end = (start + count).min(num_rows);

            let mut results = Vec::with_capacity(end - start);
            for row_idx in start..end {
                let mut map = serde_json::Map::with_capacity(table.num_columns + 1);
                match &table.ids[row_idx] {
                    dbobj::Id::Integer(id) => {
                        map.insert("id".into(), serde_json::Value::Number((*id).into()));
                    }
                    dbobj::Id::String(s) => {
                        map.insert("id".into(), serde_json::Value::String(s.to_string()));
                    }
                }
                let base = row_idx * table.num_columns;
                for (col_idx, col_def) in table.schema.columns.iter().enumerate() {
                    let val = &table.data[base + col_idx];
                    let json_val = query::db_value_to_json(val, &table);
                    map.insert(col_def.name.to_string(), json_val);
                }
                results.push(serde_json::Value::Object(map));
            }
            Ok::<_, String>(serde_json::Value::Array(results))
        })
        .await
        .map_err(|e| napi::Error::from_reason(format!("Async task panicked: {}", e)))?
        .map_err(|e| napi::Error::from_reason(e))
    }

    #[napi]
    pub fn cursor(&self, table_name: String, batch_size: Option<u32>) -> cursor::Cursor {
        cursor::create_cursor(self.inner.clone(), table_name, batch_size)
    }

    #[napi]
    pub fn count_rows(&self, table_name: String) -> Result<u32> {
        query::count_rows(self, table_name)
    }

    #[napi]
    pub fn sum_column(&self, table_name: String, column_name: String) -> Result<i64> {
        query::sum_column(self, table_name, column_name)
    }

    #[napi]
    pub fn min_column(&self, table_name: String, column_name: String) -> Result<i64> {
        query::min_column(self, table_name, column_name)
    }

    #[napi]
    pub fn max_column(&self, table_name: String, column_name: String) -> Result<i64> {
        query::max_column(self, table_name, column_name)
    }

    #[napi]
    pub fn avg_column(&self, table_name: String, column_name: String) -> Result<f64> {
        query::avg_column(self, table_name, column_name)
    }

    #[napi(getter)]
    pub fn schema(&self) -> schema::Schema {
        schema::Schema {
            db: self.inner.clone(),
        }
    }

    // ── TRANSACTIONS ──────────────────────────────────────────────────

    #[napi]
    pub fn begin_transaction(&self) -> Transaction {
        let tx = self.inner.begin_transaction();
        let mut original_tables = std::collections::HashMap::new();
        for (name, table) in tx.original_tables {
            original_tables.insert(name.clone(), table);
        }
        Transaction {
            db: self.inner.clone(),
            original_tables,
        }
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
                        // SQL executor doesn't provide Table reference easily here,
                        // but we can try to use a dummy table or implement a simpler version
                        let json_val = match v {
                            dbobj::Value::String(s) if s.starts_with('{') || s.starts_with('[') => {
                                serde_json::from_str(s.as_str())
                                    .unwrap_or(serde_json::Value::String(s.to_string()))
                            }
                            dbobj::Value::Null => serde_json::Value::Null,
                            dbobj::Value::Integer(i) => serde_json::Value::Number(i.into()),
                            dbobj::Value::Float(f) => serde_json::Number::from_f64(f)
                                .map(serde_json::Value::Number)
                                .unwrap_or(serde_json::Value::Null),
                            dbobj::Value::String(s) => serde_json::Value::String(s.to_string()),
                            dbobj::Value::Boolean(b) => serde_json::Value::Bool(b),
                            dbobj::Value::Blob(b) => serde_json::Value::Array(
                                b.iter()
                                    .map(|&x| serde_json::Value::Number(x.into()))
                                    .collect(),
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
                let results: Vec<serde_json::Value> = vals
                    .into_iter()
                    .map(|i| serde_json::Value::Number(i.into()))
                    .collect();
                Ok(serde_json::Value::Array(results))
            }
        };
        self.save_if_needed();
        out
    }

    #[napi]
    pub fn query(
        &self,
        sql: String,
        params: Option<Vec<Option<serde_json::Value>>>,
    ) -> Result<PreparedStatement> {
        self.prepare_internal(sql, params)
    }

    #[napi]
    pub fn prepare(
        &self,
        sql: String,
        params: Option<Vec<Option<serde_json::Value>>>,
    ) -> Result<PreparedStatement> {
        self.prepare_internal(sql, params)
    }

    fn prepare_internal(
        &self,
        sql: String,
        params: Option<Vec<Option<serde_json::Value>>>,
    ) -> Result<PreparedStatement> {
        let executor = dbobj_sql::SqlExecutor::new(&self.inner);
        let stmt = executor.prepare(&sql).map_err(napi::Error::from_reason)?;
        let bound_params = params.map(|p| p.into_iter().map(json_to_db_value).collect());
        Ok(PreparedStatement {
            inner: stmt,
            db: self.inner.clone(),
            is_dirty: self.is_dirty.clone(),
            params: bound_params,
        })
    }

    #[napi]
    pub fn query_i64(&self, sql: String) -> Result<BigInt64Array> {
        let executor = dbobj_sql::SqlExecutor::new(&self.inner);
        let mut result = executor
            .execute_i64(&sql)
            .map_err(napi::Error::from_reason)?;
        let ptr = result.as_mut_ptr();
        let len = result.len();
        std::mem::forget(result);
        unsafe {
            Ok(BigInt64Array::with_external_data(ptr, len, |ptr, len| {
                let _ = Vec::from_raw_parts(ptr, len, len);
            }))
        }
    }

    #[napi]
    pub fn query_join_i64(&self, sql: String) -> Result<BigInt64Array> {
        let executor = dbobj_sql::SqlExecutor::new(&self.inner);
        let (mut result, _width) = executor
            .execute_join_i64(&sql)
            .map_err(napi::Error::from_reason)?;
        let ptr = result.as_mut_ptr();
        let len = result.len();
        std::mem::forget(result);
        unsafe {
            Ok(BigInt64Array::with_external_data(ptr, len, |ptr, len| {
                let _ = Vec::from_raw_parts(ptr, len, len);
            }))
        }
    }

    // ── ARROW ────────────────────────────────────────────────────────

    #[napi]
    pub fn export_table_to_arrow_ipc(&self, table_name: String) -> Result<Buffer> {
        let bytes = self
            .inner
            .export_table_to_arrow_ipc(&table_name)
            .map_err(|e| napi::Error::from_reason(e))?;
        Ok(Buffer::from(bytes))
    }

    /// Convert an array of JS objects to Arrow IPC buffer using the table's schema.
    /// Each object's properties are mapped to columns by name, with correct Arrow types.
    /// The returned buffer can be passed directly to insertFromArrow / updateFromArrow.
    #[napi]
    pub fn objects_to_arrow_ipc(
        &self,
        table_name: String,
        objects: Vec<serde_json::Value>,
    ) -> Result<Buffer> {
        use arrow::array::*;
        use arrow::datatypes::{Field, Schema as ArrowSchema};
        use arrow::ipc::writer::FileWriter;
        use arrow::record_batch::RecordBatch;
        use std::sync::Arc as StdArc;

        let tables = self.inner.tables.read();
        let table_lock = tables
            .get(&table_name)
            .ok_or_else(|| napi::Error::from_reason(format!("Table '{}' not found", table_name)))?;
        let table = table_lock.read();

        let num_rows = objects.len();
        if num_rows == 0 {
            return Ok(Buffer::from(Vec::<u8>::new()));
        }

        let num_cols = table.schema.columns.len();
        let mut arrow_fields = Vec::with_capacity(num_cols);
        let mut arrow_columns: Vec<ArrayRef> = Vec::with_capacity(num_cols);

        for col_def in &table.schema.columns {
            let arrow_type = crate::database::query_builder::db_to_arrow_type(&col_def.data_type);
            arrow_fields.push(Field::new(
                col_def.name.as_str(),
                arrow_type,
                col_def.nullable,
            ));

            match col_def.data_type {
                dbobj::DataType::Integer => {
                    let mut builder = Int64Builder::with_capacity(num_rows);
                    for obj in &objects {
                        if let serde_json::Value::Object(map) = obj {
                            match map.get(col_def.name.as_str()) {
                                Some(serde_json::Value::Number(n)) => {
                                    if let Some(i) = n.as_i64() {
                                        builder.append_value(i);
                                    } else {
                                        builder.append_null();
                                    }
                                }
                                Some(serde_json::Value::Null) | None => builder.append_null(),
                                _ => builder.append_null(),
                            }
                        } else {
                            builder.append_null();
                        }
                    }
                    arrow_columns.push(Arc::new(builder.finish()));
                }
                dbobj::DataType::Float => {
                    let mut builder = Float64Builder::with_capacity(num_rows);
                    for obj in &objects {
                        if let serde_json::Value::Object(map) = obj {
                            match map.get(col_def.name.as_str()) {
                                Some(serde_json::Value::Number(n)) => {
                                    if let Some(f) = n.as_f64() {
                                        builder.append_value(f);
                                    } else {
                                        builder.append_null();
                                    }
                                }
                                Some(serde_json::Value::Null) | None => builder.append_null(),
                                _ => builder.append_null(),
                            }
                        } else {
                            builder.append_null();
                        }
                    }
                    arrow_columns.push(Arc::new(builder.finish()));
                }
                dbobj::DataType::String => {
                    let avg_len = 32usize;
                    let mut builder = StringBuilder::with_capacity(num_rows, num_rows * avg_len);
                    for obj in &objects {
                        if let serde_json::Value::Object(map) = obj {
                            match map.get(col_def.name.as_str()) {
                                Some(serde_json::Value::String(s)) => {
                                    builder.append_value(s.as_str())
                                }
                                Some(serde_json::Value::Null) | None => builder.append_null(),
                                _ => builder.append_null(),
                            }
                        } else {
                            builder.append_null();
                        }
                    }
                    arrow_columns.push(Arc::new(builder.finish()));
                }
                dbobj::DataType::Boolean => {
                    let mut builder = BooleanBuilder::with_capacity(num_rows);
                    for obj in &objects {
                        if let serde_json::Value::Object(map) = obj {
                            match map.get(col_def.name.as_str()) {
                                Some(serde_json::Value::Bool(b)) => builder.append_value(*b),
                                Some(serde_json::Value::Null) | None => builder.append_null(),
                                _ => builder.append_null(),
                            }
                        } else {
                            builder.append_null();
                        }
                    }
                    arrow_columns.push(Arc::new(builder.finish()));
                }
                dbobj::DataType::Blob => {
                    let avg_len = 64usize;
                    let mut builder = BinaryBuilder::with_capacity(num_rows, num_rows * avg_len);
                    for obj in &objects {
                        if let serde_json::Value::Object(map) = obj {
                            match map.get(col_def.name.as_str()) {
                                Some(serde_json::Value::Array(arr)) => {
                                    let bytes: Vec<u8> = arr
                                        .iter()
                                        .filter_map(|v| v.as_u64().map(|u| u as u8))
                                        .collect();
                                    builder.append_value(&bytes);
                                }
                                Some(serde_json::Value::Null) | None => builder.append_null(),
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
            writer
                .write(&batch)
                .map_err(|e| napi::Error::from_reason(e.to_string()))?;
            writer
                .finish()
                .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        }

        Ok(Buffer::from(buffer))
    }

    #[napi]
    pub fn import_table_from_arrow_ipc(&self, table_name: String, buffer: Buffer) -> Result<bool> {
        self.inner
            .import_table_from_arrow_ipc(&table_name, buffer.as_ref())
            .map_err(|e| napi::Error::from_reason(e))?;
        self.save_if_needed();
        Ok(true)
    }

    /// Create a table from an Arrow IPC buffer's schema (zero-copy schema definition).
    /// The IPC buffer must contain at least a valid Arrow schema header (data is optional).
    /// Returns the number of columns created.
    #[napi]
    pub fn create_table_from_arrow_ipc(&self, table_name: String, buffer: Buffer) -> Result<u32> {
        use arrow::ipc::reader::FileReader;
        use std::io::Cursor;

        let cursor = Cursor::new(buffer.as_ref());
        let reader = FileReader::try_new(cursor, None)
            .map_err(|e| napi::Error::from_reason(format!("Failed to read Arrow IPC: {}", e)))?;

        let arrow_schema = reader.schema();
        let fields = arrow_schema.fields();

        let mut db_columns = Vec::with_capacity(fields.len());
        for field in fields {
            let db_type = arrow_to_db_type_napi(field.data_type()).ok_or_else(|| {
                napi::Error::from_reason(format!("Unsupported Arrow type: {:?}", field.data_type()))
            })?;
            db_columns.push(dbobj::ColumnDefinition {
                name: field.name().as_str().into(),
                data_type: db_type,
                nullable: field.is_nullable(),
            });
        }

        let schema = dbobj::Schema {
            columns: db_columns,
        };
        let count = schema.columns.len() as u32;
        self.inner.create_table(table_name, schema);
        self.save_if_needed();
        Ok(count)
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
        Ok(Self {
            inner,
            path: Some(path),
            is_dirty,
        })
    }

    #[napi]
    pub fn save(&self, path: String) -> Result<bool> {
        self.inner
            .save_to_mmap(path)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        self.is_dirty.store(false, Ordering::Relaxed);
        Ok(true)
    }

    #[napi]
    pub fn list_tables(&self) -> Vec<String> {
        self.inner.list_tables()
    }

    #[napi]
    pub fn create_index(&self, table_name: String, column_name: String) -> Result<bool> {
        self.inner
            .create_index(&table_name, &column_name)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        self.save_if_needed();
        Ok(true)
    }

    #[napi]
    pub fn create_unique_index(&self, table_name: String, column_name: String) -> Result<bool> {
        self.inner
            .create_unique_index(&table_name, &column_name)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        self.save_if_needed();
        Ok(true)
    }

    #[napi]
    pub fn create_composite_index(
        &self,
        table_name: String,
        column_names: Vec<String>,
    ) -> Result<bool> {
        for col in &column_names {
            self.inner
                .create_index(&table_name, col)
                .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        }
        self.save_if_needed();
        Ok(true)
    }

    #[napi]
    pub fn get_table_metadata(&self, name: String) -> Result<Option<TableMetadata>> {
        Ok(self.inner.table_info(&name).map(|info| TableMetadata {
            name: info.name,
            row_count: info.row_count as u32,
            column_count: info.columns.len() as u32,
        }))
    }

    // ── QUERY BUILDER ──────────────────────────────────────────────────

    #[napi]
    pub fn create_query_builder(&self) -> query_builder::JsQueryBuilder {
        query_builder::JsQueryBuilder::new(self.inner.clone(), self.is_dirty.clone())
    }
}
