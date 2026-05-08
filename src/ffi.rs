use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::Mutex;

use crate::core::{DataType, RowData, Schema, Value};
use compact_str::CompactString;

static DB_REGISTRY: Mutex<Option<std::collections::HashMap<u64, RegistryEntry>>> = Mutex::new(None);
static NEXT_HANDLE: Mutex<u64> = Mutex::new(1);

struct RegistryEntry {
    db: crate::core::Database,
}

fn get_registry(
) -> std::sync::MutexGuard<'static, Option<std::collections::HashMap<u64, RegistryEntry>>> {
    DB_REGISTRY.lock().unwrap()
}

fn register(db: crate::core::Database) -> u64 {
    let mut reg = get_registry();
    if reg.is_none() {
        *reg = Some(std::collections::HashMap::new());
    }
    let mut next = NEXT_HANDLE.lock().unwrap();
    let handle = *next;
    *next += 1;
    reg.as_mut().unwrap().insert(handle, RegistryEntry { db });
    handle
}

fn with_db<F, R>(handle: u64, f: F) -> Result<R, String>
where
    F: FnOnce(&crate::core::Database) -> Result<R, String>,
{
    let reg = get_registry();
    let entry = reg
        .as_ref()
        .and_then(|m| m.get(&handle))
        .ok_or_else(|| format!("Invalid database handle: {}", handle))?;
    f(&entry.db)
}

fn make_c_string(s: String) -> *mut c_char {
    CString::new(s)
        .unwrap_or_else(|_| CString::new("null").unwrap())
        .into_raw()
}

fn c_string_to_str(ptr: *const c_char) -> &'static str {
    unsafe { CStr::from_ptr(ptr) }.to_str().unwrap_or("")
}

fn json_value_to_value(v: &serde_json::Value) -> Result<Value, String> {
    match v {
        serde_json::Value::Null => Ok(Value::Null),
        serde_json::Value::Bool(b) => Ok(Value::Boolean(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(Value::Float(f))
            } else {
                Err(format!("Invalid number: {}", n))
            }
        }
        serde_json::Value::String(s) => Ok(Value::String(CompactString::from(s.as_str()))),
        serde_json::Value::Array(arr) => {
            let bytes: Result<Vec<u8>, _> = arr
                .iter()
                .map(|v| {
                    v.as_u64()
                        .and_then(|n| if n <= 255 { Some(n as u8) } else { None })
                        .ok_or_else(|| format!("Blob element out of range: {}", v))
                })
                .collect();
            Ok(Value::Blob(bytes?))
        }
        serde_json::Value::Object(_) => Err("Nested objects not supported as values".to_string()),
    }
}

fn build_row_json_string(
    buf: &mut String,
    values: &[Value],
    columns: &[crate::core::ColumnDefinition],
) {
    buf.push('{');
    for (i, (col, val)) in columns.iter().zip(values.iter()).enumerate() {
        if i > 0 {
            buf.push(',');
        }
        buf.push('"');
        buf.push_str(&col.name);
        buf.push_str("\":");
        value_to_json_string(buf, val);
    }
    buf.push('}');
}

fn value_to_json_string(buf: &mut String, v: &Value) {
    match v {
        Value::Null => buf.push_str("null"),
        Value::Integer(i) => {
            use std::fmt::Write;
            let _ = write!(buf, "{}", i);
        }
        Value::Float(f) => {
            use std::fmt::Write;
            let _ = write!(buf, "{}", f);
        }
        Value::String(s) => {
            buf.push('"');
            buf.push_str(s);
            buf.push('"');
        }
        Value::InternedString(_) => buf.push_str("\"interned\""),
        Value::Boolean(b) => buf.push_str(if *b { "true" } else { "false" }),
        Value::Blob(b) => {
            buf.push('[');
            for (i, &byte) in b.iter().enumerate() {
                if i > 0 {
                    buf.push(',');
                }
                use std::fmt::Write;
                let _ = write!(buf, "{}", byte);
            }
            buf.push(']');
        }
    }
}

fn result_ok() -> String {
    serde_json::json!({ "ok": true }).to_string()
}

fn json_to_rowdata(json: &str) -> Result<RowData, String> {
    let map: std::collections::HashMap<String, serde_json::Value> =
        serde_json::from_str(json).map_err(|e| format!("JSON parse error: {}", e))?;
    let mut row = RowData::default();
    for (k, v) in map {
        row.insert(CompactString::from(k), json_value_to_value(&v)?);
    }
    Ok(row)
}

fn parse_id(s: &str) -> Result<crate::core::Id, String> {
    if let Ok(i) = s.parse::<u64>() {
        Ok(crate::core::Id::Integer(i))
    } else {
        Ok(crate::core::Id::String(CompactString::from(s)))
    }
}

fn populate_id_map(table: &mut crate::core::Table) {
    if table.is_sequential_ids {
        table.is_sequential_ids = false;
        for (i, id) in table.ids.iter().enumerate() {
            table.id_map.insert(id.clone(), i);
        }
    }
}

fn execute_sql(db: &crate::core::Database, sql: &str) -> Result<String, String> {
    use crate::sql::executor::SqlExecutor;
    let executor = SqlExecutor::new(db);
    executor.execute(sql).map(|rs| match rs {
        crate::sql::executor::SqlResult::Ok => serde_json::json!({ "ok": [] }).to_string(),
        crate::sql::executor::SqlResult::Rows(rows) => {
            let mut buf = String::with_capacity(rows.len() * 64);
            buf.push_str("{\"ok\":[");
            for (i, row) in rows.iter().enumerate() {
                if i > 0 {
                    buf.push(',');
                }
                buf.push_str(&rowdata_to_json_string(row));
            }
            buf.push_str("]}");
            buf
        }
    })
}

fn rowdata_to_json_string(row: &RowData) -> String {
    let mut buf = String::with_capacity(row.len() * 32);
    buf.push('{');
    let mut first = true;
    for (k, v) in row {
        if !first {
            buf.push(',');
        }
        first = false;
        buf.push('"');
        buf.push_str(k);
        buf.push_str("\":");
        value_to_json_string(&mut buf, v);
    }
    buf.push('}');
    buf
}

/// Prepare a search value for comparison against stored values.
/// If the value is a String, try to intern it via the table's string pool
/// so it matches InternedString variants stored in the table.
fn prepare_search_value(table: &crate::core::Table, val: &Value) -> Value {
    if let Value::String(s) = val {
        if let Some(id) = table.string_pool.get_id(s.as_str()) {
            return Value::InternedString(id);
        }
    }
    val.clone()
}

// ─── C FFI Functions ──────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn dbobj_open(name: *const c_char) -> *mut c_char {
    let name = c_string_to_str(name);
    let db = crate::core::Database::new(name.to_string());
    let handle = register(db);
    make_c_string(serde_json::json!({ "ok": handle }).to_string())
}

#[unsafe(no_mangle)]
pub extern "C" fn dbobj_close(handle: u64) -> *mut c_char {
    let mut reg = get_registry();
    match reg.as_mut().and_then(|m| m.remove(&handle)) {
        Some(_) => make_c_string(result_ok()),
        None => make_c_string(
            serde_json::json!({ "error": format!("Invalid handle: {}", handle) }).to_string(),
        ),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dbobj_execute(handle: u64, sql: *const c_char) -> *mut c_char {
    let sql = c_string_to_str(sql);
    let result = with_db(handle, |db| execute_sql(db, sql));
    make_c_string(match result {
        Ok(v) => v,
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dbobj_create_table(
    handle: u64,
    name: *const c_char,
    columns_json: *const c_char,
) -> *mut c_char {
    let table_name = c_string_to_str(name);
    let columns_str = c_string_to_str(columns_json);

    let result = (|| -> Result<String, String> {
        let cols: Vec<serde_json::Value> =
            serde_json::from_str(columns_str).map_err(|e| format!("JSON parse error: {}", e))?;

        let mut schema_cols = Vec::with_capacity(cols.len());
        for col in cols {
            let obj = col.as_object().ok_or("Each column must be an object")?;
            let col_name = obj
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Column must have a 'name' string")?;
            let col_type = obj
                .get("type")
                .and_then(|v| v.as_str())
                .ok_or("Column must have a 'type' string")?;
            let nullable = obj
                .get("nullable")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let data_type = match col_type.to_lowercase().as_str() {
                "integer" => DataType::Integer,
                "float" => DataType::Float,
                "string" => DataType::String,
                "boolean" => DataType::Boolean,
                "blob" => DataType::Blob,
                other => return Err(format!("Unknown type: {}", other)),
            };

            schema_cols.push(crate::core::ColumnDefinition {
                name: CompactString::from(col_name),
                data_type,
                nullable,
            });
        }

        with_db(handle, |db| {
            db.create_table(table_name.to_string(), Schema {
                columns: schema_cols,
            });
            Ok(result_ok())
        })
    })();

    make_c_string(match result {
        Ok(v) => v,
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    })
}

/// Insert positional values via direct API — no SQL parsing.
#[unsafe(no_mangle)]
pub extern "C" fn dbobj_insert(
    handle: u64,
    table: *const c_char,
    values_json: *const c_char,
) -> *mut c_char {
    let table_name = c_string_to_str(table);
    let vals_str = c_string_to_str(values_json);

    let result = (|| -> Result<String, String> {
        let values: Vec<serde_json::Value> =
            serde_json::from_str(vals_str).map_err(|e| format!("JSON parse error: {}", e))?;

        let db_values: Vec<Value> =
            values.iter().map(|v| json_value_to_value(v)).collect::<Result<_, _>>()?;

        with_db(handle, |db| {
            let id = db
                .insert_values(table_name, db_values)
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": id.to_string() }).to_string())
        })
    })();

    make_c_string(match result {
        Ok(v) => v,
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    })
}

/// Batch insert positional values via direct API — no SQL parsing.
#[unsafe(no_mangle)]
pub extern "C" fn dbobj_insert_batch(
    handle: u64,
    table: *const c_char,
    rows_json: *const c_char,
) -> *mut c_char {
    let table_name = c_string_to_str(table);
    let rows_str = c_string_to_str(rows_json);

    let result = (|| -> Result<String, String> {
        let rows: Vec<Vec<serde_json::Value>> =
            serde_json::from_str(rows_str).map_err(|e| format!("JSON parse error: {}", e))?;

        let batch: Vec<Vec<Value>> = rows
            .iter()
            .map(|row| row.iter().map(|v| json_value_to_value(v)).collect::<Result<_, _>>())
            .collect::<Result<_, _>>()?;

        with_db(handle, |db| {
            let ids = db
                .insert_batch_values(table_name, batch)
                .map_err(|e| e.to_string())?;
            let id_strings: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
            Ok(serde_json::json!({ "ok": id_strings }).to_string())
        })
    })();

    make_c_string(match result {
        Ok(v) => v,
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    })
}

/// Insert key-value object via direct API — no SQL parsing.
#[unsafe(no_mangle)]
pub extern "C" fn dbobj_insert_object(
    handle: u64,
    table: *const c_char,
    data_json: *const c_char,
) -> *mut c_char {
    let table_name = c_string_to_str(table);
    let data_str = c_string_to_str(data_json);

    let result = (|| -> Result<String, String> {
        let row = json_to_rowdata(data_str)?;
        with_db(handle, |db| {
            let id = db
                .insert_row(table_name, row, None)
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "ok": id.to_string() }).to_string())
        })
    })();

    make_c_string(match result {
        Ok(v) => v,
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    })
}

/// Scan table, match column value — direct API, handles interned strings.
#[unsafe(no_mangle)]
pub extern "C" fn dbobj_select(
    handle: u64,
    table: *const c_char,
    column: *const c_char,
    value_json: *const c_char,
) -> *mut c_char {
    let table_name = c_string_to_str(table);
    let col_name = c_string_to_str(column);
    let val_str = c_string_to_str(value_json);

    let result = (|| -> Result<String, String> {
        let search_val: serde_json::Value =
            serde_json::from_str(val_str).map_err(|e| format!("JSON parse error: {}", e))?;
        let search_val = json_value_to_value(&search_val)?;

        with_db(handle, |db| {
            let table_lock = db
                .get_table(table_name)
                .ok_or_else(|| format!("Table not found: {}", table_name))?;
            let table = table_lock.read();

            let col_idx = match table.get_column_index(col_name) {
                Some(idx) => idx,
                None => return Err(format!(
                    "Column '{}' not found in table '{}'",
                    col_name, table_name
                )),
            };

            // Prepare search value for comparison (intern strings)
            let lookup_val = prepare_search_value(&table, &search_val);

            let mut json_rows = Vec::new();

            // Try index first
            if let Some(index) = table.indexes.get(col_name) {
                let lookup = lookup_val.clone();
                if index.is_unique {
                    if let Some(&row_idx) = index.unique_map.get(&lookup) {
                        if let Some(values) = table.get_row_values(row_idx) {
                            let mut row_json = String::with_capacity(128);
                            build_row_json_string(&mut row_json, &values, &table.schema.columns);
                            json_rows.push(row_json);
                        }
                    }
                } else if let Some(ids) = index.map.get(&lookup) {
                    for id in ids {
                        if let Some(row) = table.get(id) {
                            let row_data = row.to_map(&table);
                            json_rows.push(rowdata_to_json_string(&row_data));
                        }
                    }
                }
            } else {
                // Linear scan
                for i in 0..table.ids.len() {
                    let cell_val = table.get_value_by_index(i, col_idx);
                    if cell_val == lookup_val {
                        if let Some(values) = table.get_row_values(i) {
                            let mut row_json = String::with_capacity(128);
                            build_row_json_string(&mut row_json, &values, &table.schema.columns);
                            json_rows.push(row_json);
                        }
                    }
                }
            }

            let mut result = String::with_capacity(json_rows.len() * 64 + 16);
            result.push_str("{\"ok\":[");
            for (i, row) in json_rows.iter().enumerate() {
                if i > 0 {
                    result.push(',');
                }
                result.push_str(row);
            }
            result.push_str("]}");
            Ok(result)
        })
    })();

    make_c_string(match result {
        Ok(v) => v,
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    })
}

/// Read all rows directly from table — no SQL parsing, string-builder JSON.
#[unsafe(no_mangle)]
pub extern "C" fn dbobj_select_all(handle: u64, table: *const c_char) -> *mut c_char {
    let table_name = c_string_to_str(table);

    let result = with_db(handle, |db| {
        let table_lock = db
            .get_table(table_name)
            .ok_or_else(|| format!("Table not found: {}", table_name))?;
        let table = table_lock.read();

        let schema = &table.schema.columns;
        let num_rows = table.ids.len();
        let mut json = String::with_capacity(num_rows * schema.len() * 32);
        json.push_str("{\"ok\":[");

        for i in 0..num_rows {
            if i > 0 {
                json.push(',');
            }
            if let Some(values) = table.get_row_values(i) {
                build_row_json_string(&mut json, &values, schema);
            }
        }

        json.push_str("]}");
        Ok(json)
    });

    make_c_string(match result {
        Ok(v) => v,
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    })
}

/// Update by row ID via direct API.
#[unsafe(no_mangle)]
pub extern "C" fn dbobj_update(
    handle: u64,
    table: *const c_char,
    row_id: *const c_char,
    values_json: *const c_char,
) -> *mut c_char {
    let table_name = c_string_to_str(table);
    let id_str = c_string_to_str(row_id);
    let vals_str = c_string_to_str(values_json);

    let result = (|| -> Result<String, String> {
        let id = parse_id(id_str)?;
        let values: Vec<serde_json::Value> =
            serde_json::from_str(vals_str).map_err(|e| format!("JSON parse error: {}", e))?;
        let db_values: Vec<Value> =
            values.iter().map(|v| json_value_to_value(v)).collect::<Result<_, _>>()?;

        with_db(handle, |db| {
            let table_lock = db
                .get_table(table_name)
                .ok_or_else(|| format!("Table not found: {}", table_name))?;
            let mut table = table_lock.write();
            populate_id_map(&mut table);
            table
                .update_values(&id, db_values)
                .map_err(|e| e.to_string())?;
            Ok(result_ok())
        })
    })();

    make_c_string(match result {
        Ok(v) => v,
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    })
}

/// Delete by row ID via direct API.
#[unsafe(no_mangle)]
pub extern "C" fn dbobj_delete(
    handle: u64,
    table: *const c_char,
    row_id: *const c_char,
) -> *mut c_char {
    let table_name = c_string_to_str(table);
    let id_str = c_string_to_str(row_id);

    let result = (|| -> Result<String, String> {
        let id = parse_id(id_str)?;

        with_db(handle, |db| {
            let table_lock = db
                .get_table(table_name)
                .ok_or_else(|| format!("Table not found: {}", table_name))?;
            let mut table = table_lock.write();
            populate_id_map(&mut table);
            if table.delete(&id).is_some() {
                Ok(result_ok())
            } else {
                Err(format!("Row with ID {} not found", id))
            }
        })
    })();

    make_c_string(match result {
        Ok(v) => v,
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dbobj_create_index(
    handle: u64,
    table: *const c_char,
    column: *const c_char,
    unique: bool,
) -> *mut c_char {
    let table_name = c_string_to_str(table);
    let col_name = c_string_to_str(column);

    let result = with_db(handle, |db| {
        if unique {
            db.create_unique_index(table_name, col_name)
        } else {
            db.create_index(table_name, col_name)
        }
        .map_err(|e| e.to_string())?;
        Ok(result_ok())
    });

    make_c_string(match result {
        Ok(v) => v,
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dbobj_list_tables(handle: u64) -> *mut c_char {
    let result = with_db(handle, |db| {
        let tables = db.list_tables();
        Ok(serde_json::json!({ "ok": tables }).to_string())
    });

    make_c_string(match result {
        Ok(v) => v,
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dbobj_table_info(handle: u64, table: *const c_char) -> *mut c_char {
    let table_name = c_string_to_str(table);
    let result = with_db(handle, |db| {
        let info = db
            .table_info(table_name)
            .ok_or_else(|| format!("Table not found: {}", table_name))?;
        Ok(serde_json::json!({ "ok": info }).to_string())
    });

    make_c_string(match result {
        Ok(v) => v,
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dbobj_save(handle: u64, path: *const c_char) -> *mut c_char {
    let file_path = c_string_to_str(path);
    let result = with_db(handle, |db| {
        let storage = crate::storage::Storage::new(file_path, crate::storage::BitcodeAdapter);
        storage.save(db).map_err(|e| e.to_string())?;
        Ok(result_ok())
    });

    make_c_string(match result {
        Ok(v) => v,
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dbobj_load(path: *const c_char) -> *mut c_char {
    let file_path = c_string_to_str(path);
    let result = (|| -> Result<String, String> {
        let storage = crate::storage::Storage::new(file_path, crate::storage::BitcodeAdapter);
        let db = storage.load().map_err(|e| e.to_string())?;
        let handle = register(db);
        Ok(serde_json::json!({ "ok": handle }).to_string())
    })();

    make_c_string(match result {
        Ok(v) => v,
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dbobj_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}
