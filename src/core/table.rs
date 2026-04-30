use super::{ColumnDefinition, FastHashMap, Id, RowData, Value};
use compact_str::CompactString;
use rayon::iter::ParallelBridge;
use rayon::prelude::*;
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
    pub data: RowData,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Index {
    pub map: BTreeMap<Value, Vec<Id>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    pub name: String,
    pub schema: Schema,
    pub rows: crate::core::FastHashMap<Id, Row>,
    pub next_int_id: u64,
    pub indexes: crate::core::FastHashMap<CompactString, Index>,
}

impl Table {
    pub fn new(name: String, schema: Schema) -> Self {
        Self {
            name,
            schema,
            rows: crate::core::FastHashMap::default(),
            next_int_id: 1,
            indexes: crate::core::FastHashMap::default(),
        }
    }

    pub fn insert(&mut self, data: RowData, custom_id: Option<Id>) -> Result<Id, TableError> {
        let id = match custom_id {
            Some(id) => {
                if self.rows.contains_key(&id) {
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

        // Schema validation
        for col_def in &self.schema.columns {
            match data.get(&col_def.name) {
                Some(val) => {
                    // Check type
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

        let row = Row {
            id: id.clone(),
            data,
            version: 1,
        };

        // Update indexes
        for (col_name, index) in &mut self.indexes {
            if let Some(val) = row.data.get(col_name) {
                index.map.entry(val.clone()).or_default().push(id.clone());
            }
        }

        self.rows.insert(id.clone(), row);
        Ok(id)
    }

    pub fn get(&self, id: &Id) -> Option<&Row> {
        self.rows.get(id)
    }

    pub fn update(&mut self, id: &Id, data: RowData) -> Result<(), TableError> {
        if let Some(row) = self.rows.get_mut(id) {
            row.data = data;
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
        self.rows.remove(id)
    }

    pub fn select<F>(&self, predicate: F) -> Vec<&Row>
    where
        F: Fn(&Row) -> bool + Sync + Send,
    {
        self.rows
            .values()
            .par_bridge()
            .filter(|r| predicate(r))
            .collect()
    }

    pub fn find_by_column(&self, column_name: &str, value: &super::Value) -> Vec<&Row> {
        // Use index if available
        if let Some(index) = self.indexes.get(column_name) {
            if let Some(ids) = index.map.get(value) {
                return ids.iter().filter_map(|id| self.rows.get(id)).collect();
            }
            return Vec::new();
        }

        // Fallback to linear scan
        self.rows
            .values()
            .filter(|r| r.data.get(column_name) == Some(value))
            .collect()
    }

    pub fn create_index(&mut self, column_name: CompactString) -> Result<(), TableError> {
        // Validate column exists in schema
        if !self.schema.columns.iter().any(|c| c.name == column_name) {
            return Err(TableError::InvalidColumn(column_name.to_string()));
        }

        let mut index = Index::default();
        for (id, row) in &self.rows {
            if let Some(val) = row.data.get(&column_name) {
                index.map.entry(val.clone()).or_default().push(id.clone());
            }
        }
        self.indexes.insert(column_name, index);
        Ok(())
    }
}
