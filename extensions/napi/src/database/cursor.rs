use napi::bindgen_prelude::*;
use napi_derive::napi;

#[napi]
pub struct Cursor {
    db: std::sync::Arc<dbobj::Database>,
    table_name: String,
    batch_size: u32,
    offset: u32,
    done: bool,
}

#[napi]
impl Cursor {
    #[napi]
    pub fn next(&mut self) -> Result<Option<serde_json::Value>> {
        if self.done {
            return Ok(None);
        }

        let table_lock = self.db.get_table(&self.table_name).ok_or_else(|| {
            napi::Error::from_reason(format!("Table '{}' not found", self.table_name))
        })?;
        let table = table_lock.read();
        let num_rows = table.ids.len();
        let start = self.offset as usize;
        if start >= num_rows {
            self.done = true;
            return Ok(None);
        }
        let end = (start + self.batch_size as usize).min(num_rows);
        drop(table);

        let mut rows = Vec::with_capacity(end - start);
        for row_idx in start..end {
            let table = table_lock.read();
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
                map.insert(col_def.name.to_string(), value_to_json(val, &table));
            }
            rows.push(serde_json::Value::Object(map));
        }

        let count = rows.len() as u32;
        self.offset += count;
        if count < self.batch_size {
            self.done = true;
        }
        Ok(Some(serde_json::Value::Array(rows)))
    }
}

fn value_to_json(val: &dbobj::Value, table: &dbobj::core::table::Table) -> serde_json::Value {
    match val {
        dbobj::Value::Null => serde_json::Value::Null,
        dbobj::Value::Integer(i) => serde_json::Value::Number((*i).into()),
        dbobj::Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        dbobj::Value::String(s) => serde_json::Value::String(s.to_string()),
        dbobj::Value::Boolean(b) => serde_json::Value::Bool(*b),
        dbobj::Value::Blob(b) => serde_json::Value::Array(
            b.iter()
                .map(|&x| serde_json::Value::Number(x.into()))
                .collect(),
        ),
        dbobj::Value::InternedString(id) => table.string_pool.resolve(*id).map_or_else(
            || serde_json::Value::String(format!("<interned:{}>", id)),
            |s| serde_json::Value::String(s.to_string()),
        ),
    }
}

pub(crate) fn create_cursor(
    db: std::sync::Arc<dbobj::Database>,
    table_name: String,
    batch_size: Option<u32>,
) -> Cursor {
    Cursor {
        db,
        table_name,
        batch_size: batch_size.unwrap_or(1000),
        offset: 0,
        done: false,
    }
}
