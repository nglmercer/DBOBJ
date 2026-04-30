use super::{Table, Schema, Id, RowData, FastHashMap};
use crate::versioning::{VersionLog, ChangeType};
use serde::{Deserialize, Serialize};
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};

#[derive(Debug, Clone, Serialize, Deserialize, Archive, RkyvSerialize, RkyvDeserialize)]
#[archive(check_bytes)]
pub struct Database {
    pub name: String,
    pub tables: FastHashMap<String, Table>,
    pub version_log: VersionLog,
}

impl Database {
    pub fn new(name: String) -> Self {
        Self {
            name,
            tables: FastHashMap::default(),
            version_log: VersionLog::new(),
        }
    }

    pub fn create_table(&mut self, name: String, schema: Schema) {
        let table = Table::new(name.clone(), schema);
        self.tables.insert(name, table);
    }

    pub fn get_table(&self, name: &str) -> Option<&Table> {
        self.tables.get(name)
    }

    pub fn get_table_mut(&mut self, name: &str) -> Option<&mut Table> {
        self.tables.get_mut(name)
    }

    pub fn insert_row(&mut self, table_name: &str, data: RowData, custom_id: Option<Id>) -> Result<Id, crate::core::table::TableError> {
        let table = self.tables.get_mut(table_name).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table_name))
        })?;
        
        let id = table.insert(data.clone(), custom_id)?;
        self.version_log.record(table_name.to_string(), id.clone(), ChangeType::Insert, Some(data));
        Ok(id)
    }

    pub fn update_row(&mut self, table_name: &str, id: &Id, data: RowData) -> Result<(), crate::core::table::TableError> {
        let table = self.tables.get_mut(table_name).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table_name))
        })?;
        
        table.update(id, data.clone())?;
        self.version_log.record(table_name.to_string(), id.clone(), ChangeType::Update, Some(data));
        Ok(())
    }

    pub fn delete_row(&mut self, table_name: &str, id: &Id) -> Result<(), crate::core::table::TableError> {
        let table = self.tables.get_mut(table_name).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table_name))
        })?;
        
        if table.delete(id).is_some() {
            self.version_log.record(table_name.to_string(), id.clone(), ChangeType::Delete, None);
            Ok(())
        } else {
            Err(crate::core::table::TableError::SchemaViolation(format!("Row with ID {} not found", id)))
        }
    }
}
