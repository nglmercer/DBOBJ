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
    pub is_unique: bool,
    pub map: crate::core::FastHashMap<Value, Vec<Id>>,
    pub unique_map: crate::core::FastHashMap<Value, usize>, // Faster path for unique indices
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
    pub is_sequential_ids: bool, // Optimization: true if IDs are 0, 1, 2...
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
            is_sequential_ids: true,
        }
    }

    pub(crate) fn get_row_by_index(&self, index: usize) -> Row {
        let start = index * self.num_columns;
        let end = start + self.num_columns;

        // Zero-copy optimization: Directly copy values into Arc.
        // We delay string resolution to Row::to_map to avoid massive cloning in queries/joins.
        let row_data: std::sync::Arc<[Value]> = std::sync::Arc::from(&self.data[start..end]);

        Row {
            id: self.ids[index].clone(),
            data: row_data,
            version: self.versions[index],
        }
    }

    /// Get a column index, supporting "id" as a special virtual column.
    pub fn get_column_index(&self, name: &str) -> Option<isize> {
        if name == "id" {
            return Some(-1); // Special sentinel for ID
        }
        self.column_map.get(name).map(|&idx| idx as isize)
    }

    /// Get a value from a row by column index (supports -1 for ID).
    pub fn get_value_by_index(&self, row_idx: usize, col_idx: isize) -> Value {
        if col_idx == -1 {
            return self.ids[row_idx].to_value();
        }
        self.data[row_idx * self.num_columns + col_idx as usize].clone()
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
                if self.is_sequential_ids {
                    self.is_sequential_ids = false;
                    for (i, existing_id) in self.ids.iter().enumerate() {
                        self.id_map.insert(existing_id.clone(), i);
                    }
                }

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
        
        if !self.is_sequential_ids {
            self.id_map.insert(id.clone(), index);
        }

        // Update indexes
        for index_obj in self.indexes.values_mut() {
            let start = index * self.num_columns;
            let val = &self.data[start + index_obj.col_idx];

            if index_obj.is_unique {
                index_obj.unique_map.insert(val.clone(), index);
            } else {
                index_obj
                    .map
                    .entry(val.clone())
                    .or_default()
                    .push(id.clone());
            }
        }

        Ok(id)
    }

    pub fn insert_batch(&mut self, batch: Vec<RowData>) -> Result<Vec<Id>, TableError> {
        let batch_size = batch.len();
        self.data.reserve(batch_size * self.num_columns);
        self.ids.reserve(batch_size);
        self.versions.reserve(batch_size);
        if !self.is_sequential_ids {
            self.id_map.reserve(batch_size);
        }

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
            if !self.is_sequential_ids {
                self.id_map.insert(id.clone(), index);
            }

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
        if !self.is_sequential_ids {
            self.id_map.reserve(batch_size);
        }

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
            if !self.is_sequential_ids {
                self.id_map.insert(id.clone(), index);
            }

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
        let expected_cols = self.num_columns;
        self.data.reserve(batch_size * expected_cols);
        self.ids.reserve(batch_size);
        self.versions.reserve(batch_size);
        if !self.is_sequential_ids {
            self.id_map.reserve(batch_size);
        }

        let mut ids = Vec::with_capacity(batch_size);

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
            if !self.is_sequential_ids {
                self.id_map.insert(id.clone(), index);
            }

            // Update indexes
            for index_obj in self.indexes.values_mut() {
                let start = index * self.num_columns;
                let val = &self.data[start + index_obj.col_idx];

                if index_obj.is_unique {
                    index_obj.unique_map.insert(val.clone(), index);
                } else {
                    index_obj
                        .map
                        .entry(val.clone())
                        .or_default()
                        .push(id.clone());
                }
            }
            ids.push(id);
        }
        Ok(ids)
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
                let mut val = values[idx].clone();
                // Resolve interned strings when converting back to a map
                if let Value::InternedString(id) = val
                    && let Some(s) = self.string_pool.resolve(id)
                {
                    val = Value::String(s.clone());
                }
                data.insert(col.name.clone(), val);
            }
        }
        data
    }

    pub fn get(&self, id: &Id) -> Option<Row> {
        if self.is_sequential_ids
            && let Id::Integer(i) = id
        {
            let idx = *i as usize;
            if idx < self.ids.len() {
                return Some(self.get_row_by_index(idx));
            }
        }
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

    pub fn get_index_handle(&self, column_name: &str) -> Option<usize> {
        self.column_map.get(column_name).copied()
    }

    pub fn find_unique_by_id(&self, column_idx: usize, value: &Value) -> Option<Row> {
        // Find index for this column
        let index = self
            .indexes
            .values()
            .find(|idx| idx.col_idx == column_idx)?;

        let mut lookup_val = value.clone();
        if let Value::String(s) = value
            && let Some(id) = self.string_pool.get_id(s.as_str())
        {
            lookup_val = Value::InternedString(id);
        }

        if index.is_unique {
            return index
                .unique_map
                .get(&lookup_val)
                .map(|&idx| self.get_row_by_index(idx));
        }
        None
    }

    pub fn delete(&mut self, id: &Id) -> Option<Row> {
        let idx = if self.is_sequential_ids {
            if let Id::Integer(i) = id {
                let index = *i as usize;
                if index < self.ids.len() && self.ids[index] == *id {
                    Some(index)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            self.id_map.get(id).copied()
        }?;

        if self.is_sequential_ids {
            self.is_sequential_ids = false;
            // Lazy populate id_map
            for (i, existing_id) in self.ids.iter().enumerate() {
                self.id_map.insert(existing_id.clone(), i);
            }
        }
        
        self.id_map.remove(id);
        
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

        let chunk_size = num_rows.div_ceil(num_threads);
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
            let mut lookup_val = value.clone();
            if let Value::String(s) = value
                && let Some(id) = self.string_pool.get_id(s.as_str())
            {
                lookup_val = Value::InternedString(id);
            }

            if index.is_unique {
                if let Some(&idx) = index.unique_map.get(&lookup_val) {
                    return vec![self.get_row_by_index(idx)];
                }
            } else if let Some(ids) = index.map.get(&lookup_val) {
                return ids.iter().filter_map(|id| self.get(id)).collect();
            }
            return Vec::new();
        }

        // Fallback to linear scan
        if let Some(col_idx) = self.get_column_index(column_name) {
            let mut results = Vec::new();
            for i in 0..self.ids.len() {
                // Optimized: check value before reconstructing Row
                if self.get_value_by_index(i, col_idx) == *value {
                    results.push(self.get_row_by_index(i));
                }
            }
            return results;
        }

        Vec::new()
    }

    pub fn create_index(&mut self, column_name: &str) -> Result<(), TableError> {
        self.create_index_internal(column_name, false)
    }

    pub fn create_unique_index(&mut self, column_name: &str) -> Result<(), TableError> {
        self.create_index_internal(column_name, true)
    }

    fn create_index_internal(
        &mut self,
        column_name: &str,
        is_unique: bool,
    ) -> Result<(), TableError> {
        let col_idx = *self
            .column_map
            .get(column_name)
            .ok_or_else(|| TableError::InvalidColumn(column_name.to_string()))?;

        let mut index = Index {
            col_idx,
            is_unique,
            map: crate::core::FastHashMap::default(),
            unique_map: crate::core::FastHashMap::default(),
        };

        // Populate index
        let num_rows = self.ids.len();
        for i in 0..num_rows {
            let start = i * self.num_columns;
            let val = &self.data[start + col_idx];
            let id = &self.ids[i];

            if is_unique {
                index.unique_map.insert(val.clone(), i);
            } else {
                index.map.entry(val.clone()).or_default().push(id.clone());
            }
        }

        self.indexes.insert(column_name.into(), index);
        Ok(())
    }
}
