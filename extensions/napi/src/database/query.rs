use super::Database;
use dbobj::{Id, Value};
use napi::bindgen_prelude::*;

pub(crate) fn get_column_i64(
    db: &Database,
    table_name: String,
    column_name: String,
) -> Result<BigInt64Array> {
    let table_lock = db
        .inner
        .get_table(&table_name)
        .ok_or_else(|| napi::Error::from_reason(format!("Table {} not found", table_name)))?;
    let table = table_lock.read();

    let mut data = table.export_column_i64(&column_name).ok_or_else(|| {
        napi::Error::from_reason(format!(
            "Column {} not found or not an integer column",
            column_name
        ))
    })?;

    let ptr = data.as_mut_ptr();
    let len = data.len();
    std::mem::forget(data);

    unsafe {
        Ok(BigInt64Array::with_external_data(ptr, len, |ptr, len| {
            let _ = Vec::from_raw_parts(ptr, len, len);
        }))
    }
}

pub(crate) fn get_column_float(
    db: &Database,
    table_name: String,
    column_name: String,
) -> Result<Vec<f64>> {
    let table_lock = db
        .inner
        .get_table(&table_name)
        .ok_or_else(|| napi::Error::from_reason(format!("Table {} not found", table_name)))?;
    let table = table_lock.read();
    table
        .export_column_f64(&column_name)
        .ok_or_else(|| napi::Error::from_reason(format!("Column {} not found", column_name)))
}

pub(crate) fn sum_column(db: &Database, table_name: String, column_name: String) -> Result<i64> {
    let table_lock = db
        .inner
        .get_table(&table_name)
        .ok_or_else(|| napi::Error::from_reason(format!("Table {} not found", table_name)))?;
    let table = table_lock.read();
    let col_idx = *table
        .column_map
        .get(&column_name)
        .ok_or_else(|| napi::Error::from_reason(format!("Column '{}' not found", column_name)))?;
    let mut sum: i64 = 0;
    for i in 0..table.ids.len() {
        if let dbobj::Value::Integer(v) = &table.data[i * table.num_columns + col_idx] {
            sum = sum.saturating_add(*v);
        }
    }
    Ok(sum)
}

pub(crate) fn min_column(db: &Database, table_name: String, column_name: String) -> Result<i64> {
    let table_lock = db
        .inner
        .get_table(&table_name)
        .ok_or_else(|| napi::Error::from_reason(format!("Table {} not found", table_name)))?;
    let table = table_lock.read();
    let col_idx = *table
        .column_map
        .get(&column_name)
        .ok_or_else(|| napi::Error::from_reason(format!("Column '{}' not found", column_name)))?;
    let mut min = i64::MAX;
    for i in 0..table.ids.len() {
        if let dbobj::Value::Integer(v) = &table.data[i * table.num_columns + col_idx] {
            if *v < min {
                min = *v;
            }
        }
    }
    if min == i64::MAX {
        Ok(0)
    } else {
        Ok(min)
    }
}

pub(crate) fn max_column(db: &Database, table_name: String, column_name: String) -> Result<i64> {
    let table_lock = db
        .inner
        .get_table(&table_name)
        .ok_or_else(|| napi::Error::from_reason(format!("Table {} not found", table_name)))?;
    let table = table_lock.read();
    let col_idx = *table
        .column_map
        .get(&column_name)
        .ok_or_else(|| napi::Error::from_reason(format!("Column '{}' not found", column_name)))?;
    let mut max = i64::MIN;
    for i in 0..table.ids.len() {
        if let dbobj::Value::Integer(v) = &table.data[i * table.num_columns + col_idx] {
            if *v > max {
                max = *v;
            }
        }
    }
    if max == i64::MIN {
        Ok(0)
    } else {
        Ok(max)
    }
}

pub(crate) fn avg_column(db: &Database, table_name: String, column_name: String) -> Result<f64> {
    let table_lock = db
        .inner
        .get_table(&table_name)
        .ok_or_else(|| napi::Error::from_reason(format!("Table {} not found", table_name)))?;
    let table = table_lock.read();
    let col_idx = *table
        .column_map
        .get(&column_name)
        .ok_or_else(|| napi::Error::from_reason(format!("Column '{}' not found", column_name)))?;
    let mut sum: f64 = 0.0;
    let mut count = 0u64;
    for i in 0..table.ids.len() {
        let val = &table.data[i * table.num_columns + col_idx];
        match val {
            dbobj::Value::Integer(v) => {
                sum += *v as f64;
                count += 1;
            }
            dbobj::Value::Float(v) => {
                sum += *v;
                count += 1;
            }
            _ => {}
        }
    }
    if count == 0 {
        Ok(0.0)
    } else {
        Ok(sum / count as f64)
    }
}

pub(crate) fn count_rows(db: &Database, table_name: String) -> Result<u32> {
    let table_lock = db
        .inner
        .get_table(&table_name)
        .ok_or_else(|| napi::Error::from_reason(format!("Table {} not found", table_name)))?;
    let len = table_lock.read().ids.len();
    Ok(len as u32)
}

pub(crate) fn get_column_string(
    db: &Database,
    table_name: String,
    column_name: String,
) -> Result<Vec<String>> {
    let table_lock = db
        .inner
        .get_table(&table_name)
        .ok_or_else(|| napi::Error::from_reason(format!("Table {} not found", table_name)))?;
    let table = table_lock.read();
    table
        .export_column_string(&column_name)
        .ok_or_else(|| napi::Error::from_reason(format!("Column {} not found", column_name)))
}

pub(crate) fn get_column_bool(
    db: &Database,
    table_name: String,
    column_name: String,
) -> Result<Vec<bool>> {
    let table_lock = db
        .inner
        .get_table(&table_name)
        .ok_or_else(|| napi::Error::from_reason(format!("Table {} not found", table_name)))?;
    let table = table_lock.read();
    table
        .export_column_bool(&column_name)
        .ok_or_else(|| napi::Error::from_reason(format!("Column {} not found", column_name)))
}

pub(crate) fn find_by_string(
    db: &Database,
    table_name: String,
    column_name: String,
    value: String,
) -> Result<BigInt64Array> {
    let results = db
        .inner
        .find(&table_name, &column_name, Value::String(value.into()))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let mut ids: Vec<i64> = results
        .into_iter()
        .map(|r| match r.id {
            Id::Integer(i) => i as i64,
            _ => 0,
        })
        .collect();
    let ptr = ids.as_mut_ptr();
    let len = ids.len();
    std::mem::forget(ids);
    unsafe {
        Ok(BigInt64Array::with_external_data(ptr, len, |ptr, len| {
            let _ = Vec::from_raw_parts(ptr, len, len);
        }))
    }
}

pub(crate) fn find_by_bool(
    db: &Database,
    table_name: String,
    column_name: String,
    value: bool,
) -> Result<BigInt64Array> {
    let results = db
        .inner
        .find(&table_name, &column_name, Value::Boolean(value))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let mut ids: Vec<i64> = results
        .into_iter()
        .map(|r| match r.id {
            Id::Integer(i) => i as i64,
            _ => 0,
        })
        .collect();
    let ptr = ids.as_mut_ptr();
    let len = ids.len();
    std::mem::forget(ids);
    unsafe {
        Ok(BigInt64Array::with_external_data(ptr, len, |ptr, len| {
            let _ = Vec::from_raw_parts(ptr, len, len);
        }))
    }
}

pub(crate) fn find_by_i64(
    db: &Database,
    table_name: String,
    column_name: String,
    value: i64,
) -> Result<BigInt64Array> {
    let results = db
        .inner
        .find(&table_name, &column_name, Value::Integer(value))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    let mut ids: Vec<i64> = results
        .into_iter()
        .map(|r| match r.id {
            Id::Integer(i) => i as i64,
            _ => 0,
        })
        .collect();
    let ptr = ids.as_mut_ptr();
    let len = ids.len();
    std::mem::forget(ids);

    unsafe {
        Ok(BigInt64Array::with_external_data(ptr, len, |ptr, len| {
            let _ = Vec::from_raw_parts(ptr, len, len);
        }))
    }
}

pub(crate) fn hash_join_i64(
    db: &Database,
    table1: String,
    col1: String,
    table2: String,
    col2: String,
) -> Result<BigInt64Array> {
    let results = db
        .inner
        .hash_join(&table1, &col1, &table2, &col2)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    let mut flat_results: Vec<i64> = Vec::with_capacity(results.len() * 2);
    for (r1, r2) in results {
        if let Id::Integer(id1) = r1.id {
            flat_results.push(id1 as i64);
        } else {
            flat_results.push(0);
        }
        if let Id::Integer(id2) = r2.id {
            flat_results.push(id2 as i64);
        } else {
            flat_results.push(0);
        }
    }

    let ptr = flat_results.as_mut_ptr();
    let len = flat_results.len();
    std::mem::forget(flat_results);

    unsafe {
        Ok(BigInt64Array::with_external_data(ptr, len, |ptr, len| {
            let _ = Vec::from_raw_parts(ptr, len, len);
        }))
    }
}

fn row_to_json(table: &dbobj::core::table::Table, row_idx: usize) -> serde_json::Value {
    let num_cols = table.num_columns;
    let mut map = serde_json::Map::with_capacity(num_cols + 1);
    match &table.ids[row_idx] {
        dbobj::Id::Integer(id) => { map.insert("id".into(), serde_json::Value::Number((*id).into())); }
        dbobj::Id::String(s) => { map.insert("id".into(), serde_json::Value::String(s.to_string())); }
    }
    let base = row_idx * num_cols;
    for (col_idx, col_def) in table.schema.columns.iter().enumerate() {
        let val = &table.data[base + col_idx];
        let json_val = match val {
            dbobj::Value::Null => serde_json::Value::Null,
            dbobj::Value::Integer(i) => serde_json::Value::Number((*i).into()),
            dbobj::Value::Float(f) => serde_json::Number::from_f64(*f)
                .map(serde_json::Value::Number).unwrap_or(serde_json::Value::Null),
            dbobj::Value::String(s) => serde_json::Value::String(s.to_string()),
            dbobj::Value::Boolean(b) => serde_json::Value::Bool(*b),
            dbobj::Value::Blob(b) => serde_json::Value::Array(
                b.iter().map(|&x| serde_json::Value::Number(x.into())).collect(),
            ),
            dbobj::Value::InternedString(id) => {
                table.string_pool.resolve(*id).map_or_else(
                    || serde_json::Value::String(format!("<interned:{}>", id)),
                    |s| serde_json::Value::String(s.to_string()),
                )
            }
        };
        map.insert(col_def.name.to_string(), json_val);
    }
    serde_json::Value::Object(map)
}

pub(crate) fn get_row_by_id(
    db: &Database,
    table_name: String,
    id: u32,
) -> Result<Option<serde_json::Value>> {
    let table_lock = db.inner.get_table(&table_name)
        .ok_or_else(|| napi::Error::from_reason(format!("Table {} not found", table_name)))?;
    let table = table_lock.read();
    let idx = table.get_index(&Id::Integer(id as u64));
    match idx {
        Some(idx) if idx < table.ids.len() => Ok(Some(row_to_json(&table, idx))),
        _ => Ok(None),
    }
}

pub(crate) fn get_row_by_column_i64(
    db: &Database,
    table_name: String,
    column_name: String,
    value: i64,
) -> Result<Option<serde_json::Value>> {
    let rows = db.inner.find(&table_name, &column_name, Value::Integer(value))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    if rows.is_empty() { return Ok(None); }
    let table_lock = db.inner.get_table(&table_name)
        .ok_or_else(|| napi::Error::from_reason(format!("Table {} not found", table_name)))?;
    let table = table_lock.read();
    let idx = table.get_index(&rows[0].id).unwrap_or(0);
    Ok(Some(row_to_json(&table, idx)))
}

pub(crate) fn get_row_by_column_string(
    db: &Database,
    table_name: String,
    column_name: String,
    value: String,
) -> Result<Option<serde_json::Value>> {
    let rows = db.inner.find(&table_name, &column_name, Value::String(value.into()))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    if rows.is_empty() { return Ok(None); }
    let table_lock = db.inner.get_table(&table_name)
        .ok_or_else(|| napi::Error::from_reason(format!("Table {} not found", table_name)))?;
    let table = table_lock.read();
    let idx = table.get_index(&rows[0].id).unwrap_or(0);
    Ok(Some(row_to_json(&table, idx)))
}

pub(crate) fn get_row_by_column_bool(
    db: &Database,
    table_name: String,
    column_name: String,
    value: bool,
) -> Result<Option<serde_json::Value>> {
    let rows = db.inner.find(&table_name, &column_name, Value::Boolean(value))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    if rows.is_empty() { return Ok(None); }
    let table_lock = db.inner.get_table(&table_name)
        .ok_or_else(|| napi::Error::from_reason(format!("Table {} not found", table_name)))?;
    let table = table_lock.read();
    let idx = table.get_index(&rows[0].id).unwrap_or(0);
    Ok(Some(row_to_json(&table, idx)))
}

pub(crate) fn get_rows(
    db: &Database,
    table_name: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<serde_json::Value> {
    let table_lock = db
        .inner
        .get_table(&table_name)
        .ok_or_else(|| napi::Error::from_reason(format!("Table {} not found", table_name)))?;
    let table = table_lock.read();

    let num_rows = table.ids.len();
    let start = offset.unwrap_or(0) as usize;
    let count = limit.unwrap_or(u32::MAX) as usize;

    if start >= num_rows {
        return Ok(serde_json::Value::Array(Vec::new()));
    }
    let end = (start + count).min(num_rows);
    let num_cols = table.num_columns;

    let mut results = Vec::with_capacity(end - start);
    for row_idx in start..end {
        results.push(row_to_json(&table, row_idx));
    }

    Ok(serde_json::Value::Array(results))
}
