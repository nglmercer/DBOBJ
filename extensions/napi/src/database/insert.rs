use super::Database;
use dbobj::Value;

pub(crate) fn insert_batch_i64(
    db: &Database,
    table_name: String,
    values: &[i64],
    num_columns: usize,
) -> Result<bool, napi::Error> {
    db.inner
        .insert_batch_flat_i64(&table_name, values, num_columns)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    db.save_if_needed();
    Ok(true)
}

pub(crate) fn insert_or_replace(
    db: &Database,
    table_name: String,
    values: Vec<Option<serde_json::Value>>,
    unique_column: String,
) -> Result<bool, napi::Error> {
    let row_values: Vec<dbobj::Value> = values.into_iter().map(super::json_to_db_value).collect();
    db.inner
        .insert_or_replace(&table_name, row_values, &unique_column)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    db.save_if_needed();
    Ok(true)
}

pub(crate) fn insert_row_i64(
    db: &Database,
    table_name: String,
    values: Vec<i64>,
) -> Result<bool, napi::Error> {
    let row_values: Vec<Value> = values.into_iter().map(Value::Integer).collect();
    db.inner
        .insert_values(&table_name, row_values)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    db.save_if_needed();
    Ok(true)
}

pub(crate) fn insert_row_string(
    db: &Database,
    table_name: String,
    values: Vec<String>,
) -> Result<bool, napi::Error> {
    let row_values: Vec<Value> = values
        .into_iter()
        .map(|s| Value::String(s.into()))
        .collect();
    db.inner
        .insert_values(&table_name, row_values)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    db.save_if_needed();
    Ok(true)
}

pub(crate) fn insert_row_bool(
    db: &Database,
    table_name: String,
    values: Vec<bool>,
) -> Result<bool, napi::Error> {
    let row_values: Vec<Value> = values.into_iter().map(Value::Boolean).collect();
    db.inner
        .insert_values(&table_name, row_values)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    db.save_if_needed();
    Ok(true)
}

pub(crate) fn insert_row_float(
    db: &Database,
    table_name: String,
    values: Vec<f64>,
) -> Result<bool, napi::Error> {
    let row_values: Vec<Value> = values.into_iter().map(Value::Float).collect();
    db.inner
        .insert_values(&table_name, row_values)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    db.save_if_needed();
    Ok(true)
}

pub(crate) fn insert_batch_float(
    db: &Database,
    table_name: String,
    values: Vec<f64>,
    num_columns: u32,
) -> Result<bool, napi::Error> {
    db.inner
        .insert_batch_flat_f64(&table_name, &values, num_columns as usize)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    db.save_if_needed();
    Ok(true)
}

pub(crate) fn insert_row(
    db: &Database,
    table_name: String,
    values: Vec<Option<serde_json::Value>>,
) -> Result<bool, napi::Error> {
    let row_values: Vec<Value> = values.into_iter().map(super::json_to_db_value).collect();
    db.inner
        .insert_values(&table_name, row_values)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    db.save_if_needed();
    Ok(true)
}

pub(crate) fn insert_batch_string(
    db: &Database,
    table_name: String,
    values: Vec<String>,
    num_columns: u32,
) -> Result<bool, napi::Error> {
    db.inner
        .insert_batch_flat_string(&table_name, &values, num_columns as usize)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    db.save_if_needed();
    Ok(true)
}

pub(crate) fn insert_batch_bool(
    db: &Database,
    table_name: String,
    values: Vec<bool>,
    num_columns: u32,
) -> Result<bool, napi::Error> {
    db.inner
        .insert_batch_flat_bool(&table_name, &values, num_columns as usize)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    db.save_if_needed();
    Ok(true)
}

pub(crate) fn insert_batch(
    db: &Database,
    table_name: String,
    values: Vec<Option<serde_json::Value>>,
    num_columns: u32,
) -> Result<bool, napi::Error> {
    use dbobj::Value as V;
    let num_cols = num_columns as usize;
    let total = values.len();
    let mut iter = values.into_iter();
    let mut batch = Vec::with_capacity(total / num_cols);
    while let Some(v0) = iter.next() {
        let mut row = Vec::with_capacity(num_cols);
        row.push(super::json_to_db_value(v0));
        for _ in 1..num_cols {
            match iter.next() {
                Some(v) => row.push(super::json_to_db_value(v)),
                None => { batch.push(row); return Ok(true); }
            }
        }
        batch.push(row);
    }
    db.inner
        .insert_batch_values(&table_name, batch)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    db.save_if_needed();
    Ok(true)
}
