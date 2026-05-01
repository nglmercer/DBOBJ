use super::{ColumnDefinition, Id, RowData, Value};
use compact_str::CompactString;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TableError {
    #[error("Row with ID {0} already exists")]
    DuplicateId(Id),
    #[error("Column {0} is missing or invalid")]
    InvalidColumn(String),
    #[error("Schema violation: {0}")]
    SchemaViolation(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub columns: Vec<ColumnDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Row {
    pub id: Id,
    pub data: std::sync::Arc<[Value]>, // Positional values for O(1) access
    pub version: u64,
}

impl Row {
    pub fn to_map(&self, table: &Table) -> RowData {
        table.values_to_row(&self.data)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Index {
    pub col_idx: usize,
    pub map: crate::core::FastHashMap<Value, Vec<Id>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    pub name: String,
    pub schema: Schema,
    pub column_map: crate::core::FastHashMap<String, usize>, // Map column name to index
    pub num_columns: usize, // Cached column count for fast validation
    pub data: Vec<Value>,   // ALL row values stored contiguously (Flat storage)
    pub ids: Vec<Id>,       // Contiguous IDs
    pub versions: Vec<u64>, // Contiguous versions
    pub id_map: crate::core::FastHashMap<Id, usize>, // Fast ID to index lookup
    pub string_pool: crate::core::value::StringPool,
    pub next_int_id: u64,
    pub indexes: crate::core::FastHashMap<CompactString, Index>,
}

impl Table {
    pub fn new(name: String, schema: Schema) -> Self {
        let mut column_map = crate::core::FastHashMap::default();
        for (i, col) in schema.columns.iter().enumerate() {
            column_map.insert(col.name.to_string(), i);
        }
        let num_columns = schema.columns.len();

        Self {
            name,
            schema,
            column_map,
            num_columns,
            data: Vec::new(),
            ids: Vec::new(),
            versions: Vec::new(),
            id_map: crate::core::FastHashMap::default(),
            string_pool: crate::core::value::StringPool::default(),
            next_int_id: 0,
            indexes: crate::core::FastHashMap::default(),
        }
    }

    pub(crate) fn get_row_by_index(&self, index: usize) -> Row {
        let start = index * self.num_columns;
        let end = start + self.num_columns;
        let mut row_data = self.data[start..end].to_vec();

        // Resolve interned strings back to CompactString for the public API
        for val in &mut row_data {
            if let Value::InternedString(id) = val {
                if let Some(s) = self.string_pool.resolve(*id) {
                    *val = Value::String(s.clone());
                }
            }
        }

        Row {
            id: self.ids[index].clone(),
            data: std::sync::Arc::from(row_data),
            version: self.versions[index],
        }
    }

    fn intern_row(&mut self, values: &mut [Value]) {
        for val in values {
            if let Value::String(s) = val {
                let id = self.string_pool.intern(s.clone());
                *val = Value::InternedString(id);
            }
        }
    }

    pub fn insert(&mut self, data: RowData, custom_id: Option<Id>) -> Result<Id, TableError> {
        let mut values = self.validate_and_convert(data)?;

        let id = match custom_id {
            Some(id) => {
                if self.id_map.contains_key(&id) {
                    return Err(TableError::DuplicateId(id));
                }
                id
            }
            None => {
                let id = Id::Integer(self.next_int_id);
                self.next_int_id += 1;
                id
            }
        };

        // Intern strings
        self.intern_row(&mut values);

        let index = self.ids.len();
        self.data.extend(values);
        self.ids.push(id.clone());
        self.versions.push(1);
        self.id_map.insert(id.clone(), index);

        // Update indexes
        for index_obj in self.indexes.values_mut() {
            let start = index * self.num_columns;
            let val = &self.data[start + index_obj.col_idx];
            index_obj
                .map
                .entry(val.clone())
                .or_default()
                .push(id.clone());
        }

        Ok(id)
    }

    pub fn insert_batch(&mut self, batch: Vec<RowData>) -> Result<Vec<Id>, TableError> {
        let batch_size = batch.len();
        self.data.reserve(batch_size * self.num_columns);
        self.ids.reserve(batch_size);
        self.versions.reserve(batch_size);
        self.id_map.reserve(batch_size);

        let mut ids = Vec::with_capacity(batch_size);

        for data in batch {
            let mut values = self.validate_and_convert(data)?;
            let id = Id::Integer(self.next_int_id);
            self.next_int_id += 1;

            self.intern_row(&mut values);

            let index = self.ids.len();
            self.data.extend(values);
            self.ids.push(id.clone());
            self.versions.push(1);
            self.id_map.insert(id.clone(), index);

            // Update indexes
            for index_obj in self.indexes.values_mut() {
                let start = index * self.num_columns;
                let val = &self.data[start + index_obj.col_idx];
                index_obj
                    .map
                    .entry(val.clone())
                    .or_default()
                    .push(id.clone());
            }
            ids.push(id);
        }

        Ok(ids)
    }

    pub fn insert_batch_raw(&mut self, batch: Vec<Box<[Value]>>) -> Result<Vec<Id>, TableError> {
        let batch_size = batch.len();
        self.data.reserve(batch_size * self.num_columns);
        self.ids.reserve(batch_size);
        self.versions.reserve(batch_size);
        self.id_map.reserve(batch_size);

        let mut ids = Vec::with_capacity(batch_size);
        let expected_cols = self.num_columns;

        for values in batch {
            if values.len() != expected_cols {
                return Err(TableError::SchemaViolation(format!(
                    "Raw batch row has {} columns, expected {}",
                    values.len(),
                    expected_cols
                )));
            }

            let id = Id::Integer(self.next_int_id);
            self.next_int_id += 1;

            let mut values_vec = values.into_vec();
            self.intern_row(&mut values_vec);

            let index = self.ids.len();
            self.data.extend(values_vec);
            self.ids.push(id.clone());
            self.versions.push(1);
            self.id_map.insert(id.clone(), index);

            // Update indexes
            for index_obj in self.indexes.values_mut() {
                let start = index * self.num_columns;
                let val = &self.data[start + index_obj.col_idx];
                index_obj
                    .map
                    .entry(val.clone())
                    .or_default()
                    .push(id.clone());
            }
            ids.push(id);
        }
        Ok(ids)
    }

    /// Optimized batch insert accepting Vec<Vec<Value>> directly.
    /// Avoids the Box<[Value]> → Arc<[Value]> double-allocation by going
    /// Vec<Value> → Arc<[Value]> in a single alloc cycle.
    pub fn insert_batch_values(&mut self, batch: Vec<Vec<Value>>) -> Result<Vec<Id>, TableError> {
        let batch_size = batch.len();
        self.data.reserve(batch_size * self.num_columns);
        self.ids.reserve(batch_size);
        self.versions.reserve(batch_size);
        self.id_map.reserve(batch_size);

        let mut ids = Vec::with_capacity(batch_size);
        let expected_cols = self.num_columns;

        for mut values in batch {
            if values.len() != expected_cols {
                return Err(TableError::SchemaViolation(format!(
                    "Batch row has {} columns, expected {}",
                    values.len(),
                    expected_cols
                )));
            }

            let id = Id::Integer(self.next_int_id);
            self.next_int_id += 1;

            self.intern_row(&mut values);

            let index = self.ids.len();
            self.data.extend(values);
            self.ids.push(id.clone());
            self.versions.push(1);
            self.id_map.insert(id.clone(), index);

            // Update indexes
            for index_obj in self.indexes.values_mut() {
                let start = index * self.num_columns;
                let val = &self.data[start + index_obj.col_idx];
                index_obj
                    .map
                    .entry(val.clone())
                    .or_default()
                    .push(id.clone());
            }
            ids.push(id);
        }
        Ok(ids)
    }

    fn validate_schema(&self, data: &RowData) -> Result<(), TableError> {
        for col_def in &self.schema.columns {
            match data.get(&col_def.name) {
                Some(val) => {
                    let type_matches = match (&col_def.data_type, val) {
                        (super::DataType::Integer, Value::Integer(_)) => true,
                        (super::DataType::Float, Value::Float(_)) => true,
                        (super::DataType::String, Value::String(_)) => true,
                        (super::DataType::Boolean, Value::Boolean(_)) => true,
                        (super::DataType::Blob, Value::Blob(_)) => true,
                        (super::DataType::Integer, Value::Null) if col_def.nullable => true,
                        (super::DataType::Float, Value::Null) if col_def.nullable => true,
                        (super::DataType::String, Value::Null) if col_def.nullable => true,
                        (super::DataType::Boolean, Value::Null) if col_def.nullable => true,
                        (super::DataType::Blob, Value::Null) if col_def.nullable => true,
                        (_, Value::Null) if !col_def.nullable => false,
                        _ => false,
                    };

                    if !type_matches {
                        return Err(TableError::SchemaViolation(format!(
                            "Type mismatch for column {}: expected {:?}, got {:?}",
                            col_def.name, col_def.data_type, val
                        )));
                    }
                }
                None => {
                    if !col_def.nullable {
                        return Err(TableError::SchemaViolation(format!(
                            "Column {} is not nullable but is missing",
                            col_def.name
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    /// Single-pass: validates schema and converts RowData to positional Vec<Value> simultaneously.
    /// Eliminates the double HashMap iteration of separate validate_schema + row_to_values.
    fn validate_and_convert(&self, data: RowData) -> Result<Vec<Value>, TableError> {
        let mut values = Vec::with_capacity(self.schema.columns.len());
        for col_def in &self.schema.columns {
            match data.get(&col_def.name) {
                Some(val) => {
                    let type_ok = match (&col_def.data_type, val) {
                        (super::DataType::Integer, Value::Integer(_)) => true,
                        (super::DataType::Float, Value::Float(_)) => true,
                        (super::DataType::String, Value::String(_)) => true,
                        (super::DataType::Boolean, Value::Boolean(_)) => true,
                        (super::DataType::Blob, Value::Blob(_)) => true,
                        (_, Value::Null) if col_def.nullable => true,
                        _ => false,
                    };
                    if !type_ok {
                        return Err(TableError::SchemaViolation(format!(
                            "Type mismatch for column {}: expected {:?}, got {:?}",
                            col_def.name, col_def.data_type, val
                        )));
                    }
                    values.push(val.clone());
                }
                None => {
                    if !col_def.nullable {
                        return Err(TableError::SchemaViolation(format!(
                            "Column {} is not nullable but is missing",
                            col_def.name
                        )));
                    }
                    values.push(Value::Null);
                }
            }
        }
        Ok(values)
    }

    pub fn row_to_values(&self, data: RowData) -> Vec<Value> {
        let mut values = Vec::with_capacity(self.schema.columns.len());
        for col in &self.schema.columns {
            values.push(data.get(&col.name).cloned().unwrap_or(Value::Null));
        }
        values
    }

    pub fn values_to_row(&self, values: &[Value]) -> RowData {
        let mut data = RowData::default();
        for col in &self.schema.columns {
            if let Some(&idx) = self.column_map.get(col.name.as_str()) {
                data.insert(col.name.clone(), values[idx].clone());
            }
        }
        data
    }

    pub fn get(&self, id: &Id) -> Option<Row> {
        self.id_map.get(id).map(|&idx| self.get_row_by_index(idx))
    }

    pub fn update(&mut self, id: &Id, data: RowData) -> Result<(), TableError> {
        let mut values = self.validate_and_convert(data)?;
        let idx = *self
            .id_map
            .get(id)
            .ok_or_else(|| TableError::SchemaViolation(format!("ID {} not found", id)))?;

        self.intern_row(&mut values);

        // Update indexes
        for index_obj in self.indexes.values_mut() {
            let start = idx * self.num_columns;
            let old_val = &self.data[start + index_obj.col_idx];
            let new_val = &values[index_obj.col_idx];

            if old_val != new_val {
                if let Some(list) = index_obj.map.get_mut(old_val) {
                    list.retain(|x| x != id);
                }
                index_obj
                    .map
                    .entry(new_val.clone())
                    .or_default()
                    .push(id.clone());
            }
        }

        let start = idx * self.num_columns;
        for (i, val) in values.into_iter().enumerate() {
            self.data[start + i] = val;
        }
        self.versions[idx] += 1;
        Ok(())
    }

    pub fn delete(&mut self, id: &Id) -> Option<Row> {
        let idx = self.id_map.remove(id)?;
        let row = self.get_row_by_index(idx);

        let last_idx = self.ids.len() - 1;
        if idx < last_idx {
            let last_id = self.ids.last().unwrap().clone();

            // Move block in data vec
            let start = idx * self.num_columns;
            let last_start = last_idx * self.num_columns;
            for i in 0..self.num_columns {
                self.data[start + i] = self.data[last_start + i].clone();
            }

            self.ids.swap_remove(idx);
            self.versions.swap_remove(idx);
            self.id_map.insert(last_id, idx);
        } else {
            self.ids.pop();
            self.versions.pop();
        }

        self.data.truncate(self.ids.len() * self.num_columns);
        Some(row)
    }

    pub fn select<F>(&self, predicate: F) -> Vec<Row>
    where
        F: Fn(&Row) -> bool + Send + Sync,
    {
        let num_rows = self.ids.len();
        let num_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);

        if num_rows < 5000 || num_threads <= 1 {
            let mut results = Vec::new();
            for i in 0..num_rows {
                let row = self.get_row_by_index(i);
                if predicate(&row) {
                    results.push(row);
                }
            }
            return results;
        }

        let chunk_size = (num_rows + num_threads - 1) / num_threads;
        let predicate_ref = &predicate;
        std::thread::scope(|s| {
            let mut handles = Vec::new();
            for i in 0..num_threads {
                let start = i * chunk_size;
                if start >= num_rows {
                    break;
                }
                let end = (start + chunk_size).min(num_rows);

                handles.push(s.spawn(move || {
                    let mut local_results = Vec::new();
                    for idx in start..end {
                        let row = self.get_row_by_index(idx);
                        if predicate_ref(&row) {
                            local_results.push(row);
                        }
                    }
                    local_results
                }));
            }
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap())
                .collect()
        })
    }

    pub fn find_by_column(&self, column_name: &str, value: &super::Value) -> Vec<Row> {
        // Use index if available
        if let Some(index) = self.indexes.get(column_name) {
            if let Some(ids) = index.map.get(value) {
                return ids.iter().filter_map(|id| self.get(id)).collect();
            }
            return Vec::new();
        }

        // Fallback to linear scan
        if let Some(&col_idx) = self.column_map.get(column_name) {
            let mut results = Vec::new();
            for i in 0..self.ids.len() {
                let start = i * self.num_columns;
                // Optimized: check value in flat data before reconstructing Row
                if &self.data[start + col_idx] == value {
                    results.push(self.get_row_by_index(i));
                }
            }
            return results;
        }

        Vec::new()
    }

    pub fn create_index(&mut self, column_name: CompactString) -> Result<(), TableError> {
        // Validate column exists in schema
        if !self.schema.columns.iter().any(|c| c.name == column_name) {
            return Err(TableError::InvalidColumn(column_name.to_string()));
        }

        let col_idx = *self.column_map.get(column_name.as_str()).unwrap();
        let mut index = Index {
            col_idx,
            map: crate::core::FastHashMap::default(),
        };
        for i in 0..self.ids.len() {
            let start = i * self.num_columns;
            let val = &self.data[start + col_idx];
            index
                .map
                .entry(val.clone())
                .or_default()
                .push(self.ids[i].clone());
        }
        self.indexes.insert(column_name, index);
        Ok(())
    }
}
