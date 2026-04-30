use super::{Id, ColumnDefinition, RowData};
use std::collections::HashMap;

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
    pub data: RowData,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    pub name: String,
    pub schema: Schema,
    pub rows: crate::core::FastHashMap<Id, Row>,
    pub next_int_id: u64,
}

impl Table {
    pub fn new(name: String, schema: Schema) -> Self {
        Self {
            name,
            schema,
            rows: crate::core::FastHashMap::default(),
            next_int_id: 1,
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

        // Basic schema validation could go here
        
        let row = Row {
            id: id.clone(),
            data,
            version: 1,
        };

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
            Err(TableError::SchemaViolation(format!("Row with ID {} not found", id)))
        }
    }

    pub fn delete(&mut self, id: &Id) -> Option<Row> {
        self.rows.remove(id)
    }
}
