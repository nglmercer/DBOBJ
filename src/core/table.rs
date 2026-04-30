use super::{ColumnDefinition, Id, RowData, Value};
use compact_str::CompactString;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    pub data: std::sync::Arc<Box<[Value]>>, // Positional values for O(1) access
    pub version: u64,
}

impl Row {
    pub fn to_map(&self, table: &Table) -> RowData {
        table.values_to_row(&self.data)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Index {
    pub map: BTreeMap<Value, Vec<Id>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    pub name: String,
    pub schema: Schema,
    pub column_map: crate::core::FastHashMap<String, usize>, // Map column name to index
    pub rows: Vec<Row>, // Contiguous storage for better cache locality
    pub id_map: crate::core::FastHashMap<Id, usize>, // Fast ID lookup
    pub next_int_id: u64,
    pub indexes: crate::core::FastHashMap<CompactString, Index>,
}

impl Table {
    pub fn new(name: String, schema: Schema) -> Self {
        let mut column_map = crate::core::FastHashMap::default();
        for (i, col) in schema.columns.iter().enumerate() {
            column_map.insert(col.name.to_string(), i);
        }

        Self {
            name,
            schema,
            column_map,
            rows: Vec::new(),
            id_map: crate::core::FastHashMap::default(),
            next_int_id: 1,
            indexes: crate::core::FastHashMap::default(),
        }
    }

    pub fn insert(&mut self, data: RowData, custom_id: Option<Id>) -> Result<Id, TableError> {
        self.validate_schema(&data)?;

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

        let values = self.row_to_values(data);
        
        let row = Row {
            id: id.clone(),
            data: std::sync::Arc::new(values),
            version: 1,
        };

        // Update indexes
        for (col_name, index) in &mut self.indexes {
            if let Some(&col_idx) = self.column_map.get(col_name.as_str()) {
                let val = &row.data[col_idx];
                index.map.entry(val.clone()).or_default().push(id.clone());
            }
        }

        let index = self.rows.len();
        self.rows.push(row);
        self.id_map.insert(id.clone(), index);
        Ok(id)
    }

    pub fn insert_batch(&mut self, batch: Vec<RowData>) -> Result<Vec<Id>, TableError> {
        let batch_size = batch.len();
        self.rows.reserve(batch_size);
        self.id_map.reserve(batch_size);

        let mut ids = Vec::with_capacity(batch_size);

        for data in batch {
            self.validate_schema(&data)?;
            let id = Id::Integer(self.next_int_id);
            self.next_int_id += 1;

            let values = self.row_to_values(data);
            let row = Row {
                id: id.clone(),
                data: std::sync::Arc::new(values),
                version: 1,
            };

            // Update indexes
            for (col_name, index) in &mut self.indexes {
                if let Some(&col_idx) = self.column_map.get(col_name.as_str()) {
                    let val = &row.data[col_idx];
                    index.map.entry(val.clone()).or_default().push(id.clone());
                }
            }

            let index = self.rows.len();
            self.rows.push(row);
            self.id_map.insert(id.clone(), index);
            ids.push(id);
        }

        Ok(ids)
    }

    pub fn insert_batch_raw(
        &mut self,
        batch: Vec<Box<[Value]>>,
    ) -> Result<Vec<Id>, TableError> {
        let mut ids = Vec::with_capacity(batch.len());
        self.rows.reserve(batch.len());

        for values in batch {
            // Fast validation: just check column count
            if values.len() != self.schema.columns.len() {
                return Err(TableError::SchemaViolation(format!(
                    "Raw batch row has {} columns, expected {}",
                    values.len(),
                    self.schema.columns.len()
                )));
            }

            let id = Id::Integer(self.next_int_id);
            self.next_int_id += 1;

            let row = Row {
                id: id.clone(),
                data: std::sync::Arc::new(values),
                version: 1,
            };

            // Update indexes
            for (col_name, index) in &mut self.indexes {
                if let Some(&col_idx) = self.column_map.get(col_name.as_str()) {
                    let val = &row.data[col_idx];
                    index.map.entry(val.clone()).or_default().push(id.clone());
                }
            }

            let index = self.rows.len();
            self.rows.push(row);
            self.id_map.insert(id.clone(), index);
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

    pub fn row_to_values(&self, data: RowData) -> Box<[Value]> {
        let mut values = Vec::with_capacity(self.schema.columns.len());
        for col in &self.schema.columns {
            values.push(data.get(&col.name).cloned().unwrap_or(Value::Null));
        }
        values.into_boxed_slice()
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

    pub fn get(&self, id: &Id) -> Option<&Row> {
        self.id_map.get(id).map(|&idx| &self.rows[idx])
    }

    pub fn update(&mut self, id: &Id, data: RowData) -> Result<(), TableError> {
        if let Some(&idx) = self.id_map.get(id) {
            self.validate_schema(&data)?;
            let values = self.row_to_values(data);
            let row = &mut self.rows[idx];
            row.data = std::sync::Arc::new(values);
            row.version += 1;
            Ok(())
        } else {
            Err(TableError::SchemaViolation(format!(
                "Row with ID {} not found",
                id
            )))
        }
    }

    pub fn delete(&mut self, id: &Id) -> Option<Row> {
        if let Some(idx) = self.id_map.remove(id) {
            if idx < self.rows.len() - 1 {
                let last_id = &self.rows.last().unwrap().id;
                self.id_map.insert(last_id.clone(), idx);
            }
            Some(self.rows.swap_remove(idx))
        } else {
            None
        }
    }

    pub fn select<F>(&self, predicate: F) -> Vec<&Row>
    where
        F: Fn(&Row) -> bool + Send + Sync,
    {
        let num_rows = self.rows.len();
        let num_threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
        
        // Only parallelize for larger datasets to avoid thread overhead
        if num_rows < 5000 || num_threads <= 1 {
            return self.rows.iter().filter(|r| predicate(r)).collect();
        }

        let chunk_size = (num_rows + num_threads - 1) / num_threads;
        let predicate_ref = &predicate;
        std::thread::scope(|s| {
            let mut handles = Vec::new();
            for chunk in self.rows.chunks(chunk_size) {
                handles.push(s.spawn(move || {
                    chunk.iter().filter(|r| predicate_ref(r)).collect::<Vec<_>>()
                }));
            }
            handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
        })
    }

    pub fn find_by_column(&self, column_name: &str, value: &super::Value) -> Vec<&Row> {
        // Use index if available
        if let Some(index) = self.indexes.get(column_name) {
            if let Some(ids) = index.map.get(value) {
                return ids.iter().filter_map(|id| self.get(id)).collect();
            }
            return Vec::new();
        }

        // Fallback to linear scan
        if let Some(&col_idx) = self.column_map.get(column_name) {
            return self.rows
                .iter()
                .filter(|r| &r.data[col_idx] == value)
                .collect();
        }

        Vec::new()
    }

    pub fn create_index(&mut self, column_name: CompactString) -> Result<(), TableError> {
        // Validate column exists in schema
        if !self.schema.columns.iter().any(|c| c.name == column_name) {
            return Err(TableError::InvalidColumn(column_name.to_string()));
        }

        let col_idx = *self.column_map.get(column_name.as_str()).unwrap();
        let mut index = Index::default();
        for row in &self.rows {
            let val = &row.data[col_idx];
            index.map.entry(val.clone()).or_default().push(row.id.clone());
        }
        self.indexes.insert(column_name, index);
        Ok(())
    }
}
