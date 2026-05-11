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
