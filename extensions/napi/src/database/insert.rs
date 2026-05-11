use super::Database;
use dbobj::Value;

pub(crate) fn insert_batch_i64(
    db: &Database,
    table_name: String,
    values: &[i64],
    num_columns: usize,
) -> Result<(), napi::Error> {
    db.inner
        .insert_batch_flat_i64(&table_name, values, num_columns)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    db.save_if_needed();
    Ok(())
}

pub(crate) fn insert_row_i64(
    db: &Database,
    table_name: String,
    values: Vec<i64>,
) -> Result<(), napi::Error> {
    let row_values: Vec<Value> = values.into_iter().map(Value::Integer).collect();
    db.inner
        .insert_values(&table_name, row_values)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    db.save_if_needed();
    Ok(())
}

pub(crate) fn insert_row_string(
    db: &Database,
    table_name: String,
    values: Vec<String>,
) -> Result<(), napi::Error> {
    let row_values: Vec<Value> = values
        .into_iter()
        .map(|s| Value::String(s.into()))
        .collect();
    db.inner
        .insert_values(&table_name, row_values)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    db.save_if_needed();
    Ok(())
}

pub(crate) fn insert_row_bool(
    db: &Database,
    table_name: String,
    values: Vec<bool>,
) -> Result<(), napi::Error> {
    let row_values: Vec<Value> = values.into_iter().map(Value::Boolean).collect();
    db.inner
        .insert_values(&table_name, row_values)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    db.save_if_needed();
    Ok(())
}

pub(crate) fn insert_row(
    db: &Database,
    table_name: String,
    values: Vec<serde_json::Value>,
) -> Result<(), napi::Error> {
    let row_values: Vec<Value> = values.into_iter().map(super::json_to_db_value).collect();
    db.inner
        .insert_values(&table_name, row_values)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    db.save_if_needed();
    Ok(())
}

pub(crate) fn insert_batch_string(
    db: &Database,
    table_name: String,
    values: Vec<String>,
    num_columns: u32,
) -> Result<(), napi::Error> {
    let num_cols = num_columns as usize;
    let total = values.len();
    let mut iter = values.into_iter();
    let mut batch = Vec::with_capacity(total / num_cols);
    'outer: while let Some(v0) = iter.next() {
        let mut row = Vec::with_capacity(num_cols);
        row.push(Value::String(v0.into()));
        for _ in 1..num_cols {
            match iter.next() {
                Some(v) => row.push(Value::String(v.into())),
                None => {
                    batch.push(row);
                    break 'outer;
                }
            }
        }
        batch.push(row);
    }
    db.inner
        .insert_batch_values(&table_name, batch)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    db.save_if_needed();
    Ok(())
}

pub(crate) fn insert_batch_bool(
    db: &Database,
    table_name: String,
    values: Vec<bool>,
    num_columns: u32,
) -> Result<(), napi::Error> {
    let num_cols = num_columns as usize;
    let total = values.len();
    let mut iter = values.into_iter();
    let mut batch = Vec::with_capacity(total / num_cols);
    'outer: while let Some(v0) = iter.next() {
        let mut row = Vec::with_capacity(num_cols);
        row.push(Value::Boolean(v0));
        for _ in 1..num_cols {
            match iter.next() {
                Some(v) => row.push(Value::Boolean(v)),
                None => {
                    batch.push(row);
                    break 'outer;
                }
            }
        }
        batch.push(row);
    }
    db.inner
        .insert_batch_values(&table_name, batch)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    db.save_if_needed();
    Ok(())
}

pub(crate) fn insert_batch(
    db: &Database,
    table_name: String,
    values: Vec<serde_json::Value>,
    num_columns: u32,
) -> Result<(), napi::Error> {
    let num_cols = num_columns as usize;
    let total = values.len();
    let mut iter = values.into_iter();
    let mut batch = Vec::with_capacity(total / num_cols);
    'outer: while let Some(v0) = iter.next() {
        let mut row = Vec::with_capacity(num_cols);
        row.push(super::json_to_db_value(v0));
        for _ in 1..num_cols {
            match iter.next() {
                Some(v) => row.push(super::json_to_db_value(v)),
                None => {
                    batch.push(row);
                    break 'outer;
                }
            }
        }
        batch.push(row);
    }
    db.inner
        .insert_batch_values(&table_name, batch)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    db.save_if_needed();
    Ok(())
}
