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

#[derive(
    Debug, Clone, Serialize, Deserialize, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
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

#[derive(
    Debug, Clone, Serialize, Deserialize, Default, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
pub struct Index {
    pub col_idx: usize,
    pub is_unique: bool,
    #[serde(skip)]
    #[rkyv(with = rkyv::with::Skip)]
    pub map: crate::core::FastHashMap<Value, Vec<Id>>,
    pub map_data: Vec<(Value, Vec<Id>)>, // Surrogate
    #[serde(skip)]
    #[rkyv(with = rkyv::with::Skip)]
    pub unique_map: crate::core::FastHashMap<Value, usize>,
    pub unique_map_data: Vec<(Value, usize)>, // Surrogate
}

impl Index {
    pub fn prepare_for_archive(&mut self) {
        self.map_data = self
            .map
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        self.unique_map_data = self
            .unique_map
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
    }

    pub fn rebuild_from_archive(&mut self) {
        self.map = self.map_data.iter().cloned().collect();
        self.unique_map = self.unique_map_data.iter().cloned().collect();
        self.map_data.clear();
        self.unique_map_data.clear();
    }
}

#[derive(
    Debug, Clone, Serialize, Deserialize, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
pub struct Table {
    pub name: String,
    pub schema: Schema,
    #[serde(skip)]
    #[rkyv(with = rkyv::with::Skip)]
    pub column_map: crate::core::FastHashMap<String, usize>,
    pub column_map_data: Vec<(String, usize)>, // Surrogate
    pub num_columns: usize,
    pub data: Vec<Value>,
    pub ids: Vec<Id>,
    pub versions: Vec<u64>,
    #[serde(skip)]
    #[rkyv(with = rkyv::with::Skip)]
    pub id_map: crate::core::FastHashMap<Id, usize>,
    pub id_map_data: Vec<(Id, usize)>, // Surrogate
    pub string_pool: crate::core::value::StringPool,
    pub next_int_id: u64,
    #[serde(skip)]
    #[rkyv(with = rkyv::with::Skip)]
    pub indexes: crate::core::FastHashMap<CompactString, Index>,
    pub indexes_data: Vec<(CompactString, Index)>, // Surrogate
    pub is_sequential_ids: bool,
}
impl Table {
    pub fn prepare_for_archive(&mut self) {
        self.column_map_data = self
            .column_map
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        self.id_map_data = self.id_map.iter().map(|(k, v)| (k.clone(), *v)).collect();

        self.indexes_data = self
            .indexes
            .iter()
            .map(|(k, v)| {
                let mut v = v.clone();
                v.prepare_for_archive();
                (k.clone(), v)
            })
            .collect();

        self.string_pool.prepare_for_archive();
    }

    pub fn rebuild_from_archive(&mut self) {
        self.column_map = self.column_map_data.iter().cloned().collect();
        self.id_map = self.id_map_data.iter().cloned().collect();

        self.indexes = self
            .indexes_data
            .iter()
            .map(|(k, v)| {
                let mut v = v.clone();
                v.rebuild_from_archive();
                (k.clone(), v)
            })
            .collect();

        self.string_pool.rebuild_from_archive();

        self.column_map_data.clear();
        self.id_map_data.clear();
        self.indexes_data.clear();
    }

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
            column_map_data: Vec::new(),
            num_columns,
            data: Vec::new(),
            ids: Vec::new(),
            versions: Vec::new(),
            id_map: crate::core::FastHashMap::default(),
            id_map_data: Vec::new(),
            string_pool: crate::core::value::StringPool::default(),
            next_int_id: 0,
            indexes: crate::core::FastHashMap::default(),
            indexes_data: Vec::new(),
            is_sequential_ids: true,
        }
    }

    pub fn get_index(&self, id: &Id) -> Option<usize> {
        if self.is_sequential_ids {
            if let Id::Integer(i) = id {
                let index = *i as usize;
                if index < self.ids.len() && self.ids[index] == *id {
                    return Some(index);
                }
            }
        }
        self.id_map.get(id).copied()
    }

    pub fn get_row_by_index(&self, index: usize) -> Row {
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

    /// Get a column index, supporting "id" as a special virtual column
    /// when the table does not have a real column named "id".
    pub fn get_column_index(&self, name: &str) -> Option<isize> {
        if name == "id" {
            if self.column_map.contains_key("id") {
                self.column_map.get("id").map(|&idx| idx as isize)
            } else {
                Some(-1)
            }
        } else {
            self.column_map.get(name).map(|&idx| idx as isize)
        }
    }

    /// Get a value from a row by column index (supports -1 for ID).
    pub fn get_value_by_index(&self, row_idx: usize, col_idx: isize) -> Value {
        if col_idx == -1 {
            return self.ids[row_idx].to_value();
        }
        self.data[row_idx * self.num_columns + col_idx as usize].clone()
    }

    /// Zero-copy reference to a cell value. For data columns only (not virtual ID column).
    #[inline]
    pub fn get_value_ref(&self, row_idx: usize, col_idx: usize) -> &Value {
        &self.data[row_idx * self.num_columns + col_idx]
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
                    .or_insert_with(|| Vec::with_capacity(1))
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
                    .or_insert_with(|| Vec::with_capacity(1))
                    .push(id.clone());
            }
            ids.push(id);
        }

        Ok(ids)
    }

    fn col_info(&self) -> String {
        let cols: Vec<String> = self
            .schema
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| format!("  {}: {} ({:?})", i, c.name, c.data_type))
            .collect();
        format!(
            "table `{}` ({} columns):\n{}",
            self.name,
            self.num_columns,
            cols.join("\n")
        )
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

        for index_obj in self.indexes.values_mut() {
            if index_obj.is_unique {
                index_obj.unique_map.reserve(batch_size);
            } else {
                index_obj.map.reserve(batch_size);
            }
        }

        for values in batch {
            if values.len() != expected_cols {
                return Err(TableError::SchemaViolation(format!(
                    "insert_batch_raw: row has {} columns, expected {}\n{}",
                    values.len(),
                    expected_cols,
                    self.col_info()
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

            for index_obj in self.indexes.values_mut() {
                let start = index * self.num_columns;
                let val = &self.data[start + index_obj.col_idx];
                index_obj
                    .map
                    .entry(val.clone())
                    .or_insert_with(|| Vec::with_capacity(1))
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

        // Optimization: Pre-size the string pool
        self.string_pool.reserve(batch_size);

        if !self.is_sequential_ids {
            self.id_map.reserve(batch_size);
        }

        let mut ids = Vec::with_capacity(batch_size);

        for index_obj in self.indexes.values_mut() {
            if index_obj.is_unique {
                index_obj.unique_map.reserve(batch_size);
            } else {
                index_obj.map.reserve(batch_size);
            }
        }

        for mut values in batch {
            if values.len() != expected_cols {
                return Err(TableError::SchemaViolation(format!(
                    "insert_batch_values: row has {} columns, expected {}\n{}",
                    values.len(),
                    expected_cols,
                    self.col_info()
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

            for index_obj in self.indexes.values_mut() {
                let start = index * self.num_columns;
                let val = &self.data[start + index_obj.col_idx];

                if index_obj.is_unique {
                    index_obj.unique_map.insert(val.clone(), index);
                } else {
                    let id = &self.ids[index];
                    index_obj
                        .map
                        .entry(val.clone())
                        .or_insert_with(|| Vec::with_capacity(1))
                        .push(id.clone());
                }
            }
            ids.push(self.ids[index].clone());
        }
        Ok(ids)
    }

    /// Ultra-fast batch insert from flat i64 slice — zero per-row allocations.
    /// Takes values as `[v0_col0, v0_col1, v1_col0, v1_col1, ...]` and directly
    /// extends the internal data Vec in one tight loop. Skips the Vec<Vec<Value>>
    /// intermediate entirely.
    pub fn insert_batch_flat_i64(
        &mut self,
        values: &[i64],
        num_columns: usize,
    ) -> Result<Vec<Id>, TableError> {
        if num_columns != self.num_columns {
            return Err(TableError::SchemaViolation(format!(
                "insert_batch_flat_i64: {} columns passed, expected {}\n{}",
                num_columns,
                self.num_columns,
                self.col_info()
            )));
        }
        let batch_size = values.len() / num_columns;
        if !values.len().is_multiple_of(num_columns) {
            return Err(TableError::SchemaViolation(
                "values length not divisible by num_columns".into(),
            ));
        }
        self.data.reserve(batch_size * num_columns);
        self.ids.reserve(batch_size);
        self.versions.reserve(batch_size);
        self.string_pool.reserve(batch_size);

        let starting_idx = self.ids.len();
        let start_id = self.next_int_id;
        self.next_int_id += batch_size as u64;

        for &v in values {
            self.data.push(Value::Integer(v));
        }

        for i in 0..batch_size {
            self.ids.push(Id::Integer(start_id + i as u64));
            self.versions.push(1);
        }

        let ids: Vec<Id> = (0..batch_size)
            .map(|i| Id::Integer(start_id + i as u64))
            .collect();

        if !self.is_sequential_ids {
            for i in 0..batch_size {
                self.id_map
                    .insert(Id::Integer(start_id + i as u64), starting_idx + i);
            }
        }

        // Pre-reserve index capacity
        for index_obj in self.indexes.values_mut() {
            if index_obj.is_unique {
                index_obj.unique_map.reserve(batch_size);
            } else {
                index_obj.map.reserve(batch_size);
            }
        }

        for index_obj in self.indexes.values_mut() {
            for row_offset in 0..batch_size {
                let actual_idx = starting_idx + row_offset;
                let val = &self.data[actual_idx * self.num_columns + index_obj.col_idx];
                if index_obj.is_unique {
                    index_obj.unique_map.insert(val.clone(), actual_idx);
                } else {
                    index_obj
                        .map
                        .entry(val.clone())
                        .or_insert_with(|| Vec::with_capacity(1))
                        .push(self.ids[actual_idx].clone());
                }
            }
        }

        Ok(ids)
    }

    /// Ultra-fast batch insert from flat string slice — zero per-row allocations.
    pub fn insert_batch_flat_string(
        &mut self,
        values: &[String],
        num_columns: usize,
    ) -> Result<Vec<Id>, TableError> {
        if num_columns != self.num_columns {
            return Err(TableError::SchemaViolation(format!(
                "insert_batch_flat_string: {} columns passed, expected {}\n{}",
                num_columns,
                self.num_columns,
                self.col_info()
            )));
        }
        let batch_size = values.len() / num_columns;
        if !values.len().is_multiple_of(num_columns) {
            return Err(TableError::SchemaViolation(
                "values length not divisible by num_columns".into(),
            ));
        }
        self.data.reserve(batch_size * num_columns);
        self.ids.reserve(batch_size);
        self.versions.reserve(batch_size);
        self.string_pool.reserve(batch_size);

        let starting_idx = self.ids.len();
        let start_id = self.next_int_id;
        self.next_int_id += batch_size as u64;

        // Batch intern all strings, then push as InternedString
        let interned: Vec<u32> = values
            .iter()
            .map(|s| {
                self.string_pool
                    .intern(compact_str::CompactString::from(s.as_str()))
            })
            .collect();
        for &id in &interned {
            self.data.push(Value::InternedString(id));
        }

        for i in 0..batch_size {
            self.ids.push(Id::Integer(start_id + i as u64));
            self.versions.push(1);
        }
        let ids: Vec<Id> = (0..batch_size)
            .map(|i| Id::Integer(start_id + i as u64))
            .collect();

        if !self.is_sequential_ids {
            for i in 0..batch_size {
                self.id_map
                    .insert(Id::Integer(start_id + i as u64), starting_idx + i);
            }
        }
        for index_obj in self.indexes.values_mut() {
            if index_obj.is_unique {
                index_obj.unique_map.reserve(batch_size);
            } else {
                index_obj.map.reserve(batch_size);
            }
        }
        for index_obj in self.indexes.values_mut() {
            for row_offset in 0..batch_size {
                let actual_idx = starting_idx + row_offset;
                let val = &self.data[actual_idx * self.num_columns + index_obj.col_idx];
                if index_obj.is_unique {
                    index_obj.unique_map.insert(val.clone(), actual_idx);
                } else {
                    index_obj
                        .map
                        .entry(val.clone())
                        .or_insert_with(|| Vec::with_capacity(1))
                        .push(self.ids[actual_idx].clone());
                }
            }
        }
        Ok(ids)
    }

    /// Ultra-fast batch insert from flat bool slice.
    pub fn insert_batch_flat_bool(
        &mut self,
        values: &[bool],
        num_columns: usize,
    ) -> Result<Vec<Id>, TableError> {
        if num_columns != self.num_columns {
            return Err(TableError::SchemaViolation(format!(
                "insert_batch_flat_bool: {} columns passed, expected {}\n{}",
                num_columns,
                self.num_columns,
                self.col_info()
            )));
        }
        let batch_size = values.len() / num_columns;
        if !values.len().is_multiple_of(num_columns) {
            return Err(TableError::SchemaViolation(
                "values length not divisible by num_columns".into(),
            ));
        }
        self.data.reserve(batch_size * num_columns);
        self.ids.reserve(batch_size);
        self.versions.reserve(batch_size);

        let starting_idx = self.ids.len();
        let start_id = self.next_int_id;
        self.next_int_id += batch_size as u64;

        for &v in values {
            self.data.push(Value::Boolean(v));
        }
        for i in 0..batch_size {
            self.ids.push(Id::Integer(start_id + i as u64));
            self.versions.push(1);
        }
        let ids: Vec<Id> = (0..batch_size)
            .map(|i| Id::Integer(start_id + i as u64))
            .collect();

        if !self.is_sequential_ids {
            for i in 0..batch_size {
                self.id_map
                    .insert(Id::Integer(start_id + i as u64), starting_idx + i);
            }
        }
        for index_obj in self.indexes.values_mut() {
            if index_obj.is_unique {
                index_obj.unique_map.reserve(batch_size);
            } else {
                index_obj.map.reserve(batch_size);
            }
        }
        for index_obj in self.indexes.values_mut() {
            for row_offset in 0..batch_size {
                let actual_idx = starting_idx + row_offset;
                let val = &self.data[actual_idx * self.num_columns + index_obj.col_idx];
                if index_obj.is_unique {
                    index_obj.unique_map.insert(val.clone(), actual_idx);
                } else {
                    index_obj
                        .map
                        .entry(val.clone())
                        .or_insert_with(|| Vec::with_capacity(1))
                        .push(self.ids[actual_idx].clone());
                }
            }
        }
        Ok(ids)
    }

    /// Ultra-fast batch insert from flat f64 slice.
    pub fn insert_batch_flat_f64(
        &mut self,
        values: &[f64],
        num_columns: usize,
    ) -> Result<Vec<Id>, TableError> {
        if num_columns != self.num_columns {
            return Err(TableError::SchemaViolation(format!(
                "insert_batch_flat_f64: {} columns passed, expected {}\n{}",
                num_columns,
                self.num_columns,
                self.col_info()
            )));
        }
        let batch_size = values.len() / num_columns;
        if !values.len().is_multiple_of(num_columns) {
            return Err(TableError::SchemaViolation(
                "values length not divisible by num_columns".into(),
            ));
        }
        self.data.reserve(batch_size * num_columns);
        self.ids.reserve(batch_size);
        self.versions.reserve(batch_size);

        let starting_idx = self.ids.len();
        let start_id = self.next_int_id;
        self.next_int_id += batch_size as u64;

        for &v in values {
            self.data.push(Value::Float(v));
        }
        for i in 0..batch_size {
            self.ids.push(Id::Integer(start_id + i as u64));
            self.versions.push(1);
        }
        let ids: Vec<Id> = (0..batch_size)
            .map(|i| Id::Integer(start_id + i as u64))
            .collect();

        if !self.is_sequential_ids {
            for i in 0..batch_size {
                self.id_map
                    .insert(Id::Integer(start_id + i as u64), starting_idx + i);
            }
        }
        for index_obj in self.indexes.values_mut() {
            if index_obj.is_unique {
                index_obj.unique_map.reserve(batch_size);
            } else {
                index_obj.map.reserve(batch_size);
            }
        }
        for index_obj in self.indexes.values_mut() {
            for row_offset in 0..batch_size {
                let actual_idx = starting_idx + row_offset;
                let val = &self.data[actual_idx * self.num_columns + index_obj.col_idx];
                if index_obj.is_unique {
                    index_obj.unique_map.insert(val.clone(), actual_idx);
                } else {
                    index_obj
                        .map
                        .entry(val.clone())
                        .or_insert_with(|| Vec::with_capacity(1))
                        .push(self.ids[actual_idx].clone());
                }
            }
        }
        Ok(ids)
    }

    /// Insert a single row from positional values — no RowData/HashMap overhead.
    /// For single inserts, this avoids the double conversion of
    /// RowData → validate_and_convert → Vec<Value>.
    pub fn insert_values(&mut self, values: Vec<Value>) -> Result<Id, TableError> {
        self.insert_batch_values(vec![values])
            .map(|mut ids| ids.remove(0))
    }

    /// Update a row from positional values — no RowData/HashMap overhead.
    /// Skips the `validate_and_convert` HashMap lookup for known column positions.
    pub fn update_values(&mut self, id: &Id, mut values: Vec<Value>) -> Result<(), TableError> {
        if values.len() != self.num_columns {
            return Err(TableError::SchemaViolation(format!(
                "update_values: expected {} values, got {}\n{}",
                self.num_columns,
                values.len(),
                self.col_info()
            )));
        }
        let idx = self
            .get_index(id)
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
                    .or_insert_with(|| Vec::with_capacity(1))
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

    /// Single-pass: validates schema and converts RowData to positional Vec<Value> simultaneously.
    /// Iterates over RowData entries and moves values directly, avoiding per-column HashMap lookups.
    fn validate_and_convert(&self, data: RowData) -> Result<Vec<Value>, TableError> {
        let num_cols = self.schema.columns.len();
        let mut values = vec![Value::Null; num_cols];
        for (col_name, val) in data {
            let idx = match self.column_map.get(col_name.as_str()) {
                Some(&idx) => idx,
                None => continue,
            };
            if !val.is_null() {
                let col_def = &self.schema.columns[idx];
                let type_ok = match (&col_def.data_type, &val) {
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
            }
            values[idx] = val;
        }
        // Check for missing non-nullable columns
        for (i, col_def) in self.schema.columns.iter().enumerate() {
            if matches!(values[i], Value::Null) && !col_def.nullable {
                return Err(TableError::SchemaViolation(format!(
                    "Column {} is not nullable but is missing",
                    col_def.name
                )));
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
        let mut data =
            RowData::with_capacity_and_hasher(self.schema.columns.len(), Default::default());
        for (idx, col) in self.schema.columns.iter().enumerate() {
            let mut val = values[idx].clone();
            if let Value::InternedString(id) = val {
                if let Some(s) = self.string_pool.resolve(id) {
                    val = Value::String(s);
                }
            }
            data.insert(col.name.clone(), val);
        }
        data
    }

    /// Return a row's values as a flat Vec<Value>, resolving interned strings.
    /// Much faster than values_to_row() for bulk reads — avoids HashMap and
    /// column-name cloning.
    pub fn get_row_values(&self, row_idx: usize) -> Option<Vec<Value>> {
        if row_idx >= self.ids.len() {
            return None;
        }
        let start = row_idx * self.num_columns;
        let end = start + self.num_columns;
        let mut values = Vec::with_capacity(self.num_columns);
        for val in &self.data[start..end] {
            let v = if let Value::InternedString(id) = val {
                if let Some(s) = self.string_pool.resolve(*id) {
                    Value::String(s)
                } else {
                    val.clone()
                }
            } else {
                val.clone()
            };
            values.push(v);
        }
        Some(values)
    }

    pub fn get(&self, id: &Id) -> Option<Row> {
        if self.is_sequential_ids {
            if let Id::Integer(i) = id {
                let idx = *i as usize;
                if idx < self.ids.len() {
                    return Some(self.get_row_by_index(idx));
                }
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
                    .or_insert_with(|| Vec::with_capacity(1))
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
        if let Value::String(s) = value {
            if let Some(id) = self.string_pool.get_id(s.as_str()) {
                lookup_val = Value::InternedString(id);
            }
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
        let idx = self.get_index(id)?;

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
            if let Value::String(s) = value {
                if let Some(id) = self.string_pool.get_id(s.as_str()) {
                    lookup_val = Value::InternedString(id);
                }
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

        // Fallback to linear scan — uses reference comparison to avoid Value clones
        if let Some(col_idx) = self.get_column_index(column_name) {
            let mut results = Vec::new();
            if col_idx == -1 {
                for i in 0..self.ids.len() {
                    if &self.ids[i].to_value() == value {
                        results.push(self.get_row_by_index(i));
                    }
                }
            } else {
                let ucol = col_idx as usize;
                let num_cols = self.num_columns;
                let data = &self.data;
                // Local ref avoids re-reading self.data for each iteration
                for i in 0..self.ids.len() {
                    if &data[i * num_cols + ucol] == value {
                        results.push(self.get_row_by_index(i));
                    }
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
            map_data: Vec::new(),
            unique_map: crate::core::FastHashMap::default(),
            unique_map_data: Vec::new(),
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
                index
                    .map
                    .entry(val.clone())
                    .or_insert_with(|| Vec::with_capacity(1))
                    .push(id.clone());
            }
        }

        self.indexes.insert(column_name.into(), index);
        Ok(())
    }

    pub fn export_column_i64(&self, column_name: &str) -> Option<Vec<i64>> {
        let col_idx = *self.column_map.get(column_name)?;
        let num_rows = self.ids.len();
        let mut result = Vec::with_capacity(num_rows);

        for i in 0..num_rows {
            match &self.data[i * self.num_columns + col_idx] {
                Value::Integer(v) => result.push(*v),
                _ => result.push(0),
            }
        }
        Some(result)
    }

    pub fn export_column_string(&self, column_name: &str) -> Option<Vec<String>> {
        let col_idx = *self.column_map.get(column_name)?;
        let num_rows = self.ids.len();
        let mut result = Vec::with_capacity(num_rows);

        for i in 0..num_rows {
            let val = &self.data[i * self.num_columns + col_idx];
            match val {
                Value::String(s) => result.push(s.to_string()),
                Value::InternedString(id) => {
                    result.push(
                        self.string_pool
                            .resolve(*id)
                            .map_or_else(|| format!("<interned:{}>", id), |s| s.to_string()),
                    );
                }
                _ => result.push(String::new()),
            }
        }
        Some(result)
    }

    pub fn export_column_bool(&self, column_name: &str) -> Option<Vec<bool>> {
        let col_idx = *self.column_map.get(column_name)?;
        let num_rows = self.ids.len();
        let mut result = Vec::with_capacity(num_rows);

        for i in 0..num_rows {
            match &self.data[i * self.num_columns + col_idx] {
                Value::Boolean(v) => result.push(*v),
                _ => result.push(false),
            }
        }
        Some(result)
    }

    pub fn export_column_f64(&self, column_name: &str) -> Option<Vec<f64>> {
        let col_idx = *self.column_map.get(column_name)?;
        let num_rows = self.ids.len();
        let mut result = Vec::with_capacity(num_rows);

        for i in 0..num_rows {
            match &self.data[i * self.num_columns + col_idx] {
                Value::Float(v) => result.push(*v),
                _ => result.push(0.0),
            }
        }
        Some(result)
    }
}
