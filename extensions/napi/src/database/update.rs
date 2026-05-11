use super::Database;
use dbobj::{Id, Value};

pub(crate) fn update_row_i64(
    db: &Database,
    table_name: String,
    id: u32,
    values: Vec<i64>,
) -> Result<(), napi::Error> {
    let row_values: Vec<Value> = values.into_iter().map(Value::Integer).collect();
    db.inner
        .update_values(&table_name, &Id::Integer(id as u64), row_values)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    db.save_if_needed();
    Ok(())
}

pub(crate) fn update_row_string(
    db: &Database,
    table_name: String,
    id: u32,
    values: Vec<String>,
) -> Result<(), napi::Error> {
    let row_values: Vec<Value> = values
        .into_iter()
        .map(|s| Value::String(s.into()))
        .collect();
    db.inner
        .update_values(&table_name, &Id::Integer(id as u64), row_values)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    db.save_if_needed();
    Ok(())
}

pub(crate) fn update_row_bool(
    db: &Database,
    table_name: String,
    id: u32,
    values: Vec<bool>,
) -> Result<(), napi::Error> {
    let row_values: Vec<Value> = values.into_iter().map(Value::Boolean).collect();
    db.inner
        .update_values(&table_name, &Id::Integer(id as u64), row_values)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    db.save_if_needed();
    Ok(())
}

pub(crate) fn update_row_float(
    db: &Database,
    table_name: String,
    id: u32,
    values: Vec<f64>,
) -> Result<(), napi::Error> {
    let row_values: Vec<Value> = values.into_iter().map(Value::Float).collect();
    db.inner
        .update_values(&table_name, &Id::Integer(id as u64), row_values)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    db.save_if_needed();
    Ok(())
}

pub(crate) fn update_row(
    db: &Database,
    table_name: String,
    id: u32,
    values: Vec<Option<serde_json::Value>>,
) -> Result<(), napi::Error> {
    let row_values: Vec<Value> = values.into_iter().map(super::json_to_db_value).collect();
    db.inner
        .update_values(&table_name, &Id::Integer(id as u64), row_values)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    db.save_if_needed();
    Ok(())
}

pub(crate) fn delete_row(db: &Database, table_name: String, id: u32) -> Result<(), napi::Error> {
    db.inner
        .delete_row(&table_name, &Id::Integer(id as u64))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    db.save_if_needed();
    Ok(())
}

// ── Bulk update by column ──────────────────────────────────────────

pub(crate) fn update_batch_i64(
    db: &Database,
    table_name: String,
    column_name: String,
    flat_params: &[i64],
) -> Result<(), napi::Error> {
    let num_cols = db.inner.get_table(&table_name)
        .map(|t| t.read().num_columns)
        .unwrap_or(0);
    let col_idx = {
        let table_lock = db.inner.get_table(&table_name)
            .ok_or_else(|| napi::Error::from_reason(format!("Table {} not found", table_name)))?;
        let table = table_lock.read();
        *table.column_map.get(&column_name).ok_or_else(|| {
            napi::Error::from_reason(format!("Column {} not found", column_name))
        })?
    };
    let mut i = 0;
    while i + 1 < flat_params.len() {
        let val = flat_params[i];
        let row_id = flat_params[i + 1] as u64;
        let id = Id::Integer(row_id);
        // Read current row, replace target column, write back
        if let Some(row) = db.inner.get_table(&table_name)
            .and_then(|t| t.read().get_row_values(row_id as usize))
        {
            let mut new_values = row;
            if col_idx < new_values.len() {
                new_values[col_idx] = Value::Integer(val);
            }
            let _ = db.inner.update_values(&table_name, &id, new_values);
        }
        i += 2;
    }
    db.save_if_needed();
    Ok(())
}

// ── Delete by column value ────────────────────────────────────────

fn delete_by_column(db: &Database, table_name: String, column_name: String, value: Value) -> Result<u32, napi::Error> {
    let results = db.inner
        .find(&table_name, &column_name, value)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let ids: Vec<Id> = results.into_iter().map(|r| r.id).collect();
    let count = ids.len() as u32;
    for id in &ids {
        let _ = db.inner.delete_row(&table_name, id);
    }
    db.save_if_needed();
    Ok(count)
}

pub(crate) fn delete_by_column_i64(db: &Database, table_name: String, column_name: String, value: i64) -> Result<u32, napi::Error> {
    delete_by_column(db, table_name, column_name, Value::Integer(value))
}

pub(crate) fn delete_by_column_string(db: &Database, table_name: String, column_name: String, value: String) -> Result<u32, napi::Error> {
    delete_by_column(db, table_name, column_name, Value::String(value.into()))
}

pub(crate) fn delete_by_column_bool(db: &Database, table_name: String, column_name: String, value: bool) -> Result<u32, napi::Error> {
    delete_by_column(db, table_name, column_name, Value::Boolean(value))
}
