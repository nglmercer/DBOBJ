use super::{Table, Schema, Id, RowData, FastHashMap};
use crate::versioning::{VersionLog, ChangeType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    pub fn create_index(&mut self, table_name: &str, column_name: &str) -> Result<(), crate::core::table::TableError> {
        let table = self.tables.get_mut(table_name).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table_name))
        })?;
        table.create_index(column_name.into())
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

    pub fn query<F>(&self, table_name: &str, predicate: F) -> Result<Vec<&super::table::Row>, crate::core::table::TableError>
    where
        F: Fn(&super::table::Row) -> bool,
    {
        let table = self.tables.get(table_name).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table_name))
        })?;
        Ok(table.select(predicate))
    }

    pub fn find(&self, table_name: &str, column_name: &str, value: super::Value) -> Result<Vec<&super::table::Row>, crate::core::table::TableError> {
        let table = self.tables.get(table_name).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table_name))
        })?;
        Ok(table.find_by_column(column_name, &value))
    }

    pub fn join<F>(&self, table1: &str, table2: &str, condition: F) -> Result<Vec<(&super::table::Row, &super::table::Row)>, crate::core::table::TableError>
    where
        F: Fn(&super::table::Row, &super::table::Row) -> bool,
    {
        let t1 = self.tables.get(table1).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table1))
        })?;
        let t2 = self.tables.get(table2).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table2))
        })?;

        let mut results = Vec::new();
        for r1 in t1.rows.values() {
            for r2 in t2.rows.values() {
                if condition(r1, r2) {
                    results.push((r1, r2));
                }
            }
        }
        Ok(results)
    }

    pub fn query_expr(&self, table_name: &str, expr: crate::core::query::Expr) -> Result<Vec<&super::table::Row>, crate::core::table::TableError> {
        let table = self.tables.get(table_name).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table_name))
        })?;
        
        let plan = expr.plan(table);
        self.execute_plan(plan)
    }

    pub fn execute_plan(&self, plan: crate::core::query::QueryPlan) -> Result<Vec<&super::table::Row>, crate::core::table::TableError> {
        match plan {
            crate::core::query::QueryPlan::FullScan(table_name, expr) => {
                let table = self.tables.get(table_name.as_str()).ok_or_else(|| {
                    crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table_name))
                })?;
                Ok(table.select(|r| expr.is_true(&r.data)))
            }
            crate::core::query::QueryPlan::IndexScan(table_name, col, val) => {
                let table = self.tables.get(table_name.as_str()).ok_or_else(|| {
                    crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table_name))
                })?;
                Ok(table.find_by_column(col.as_str(), &val))
            }
        }
    }

    pub fn begin_transaction(&mut self) -> Transaction<'_> {
        Transaction {
            original_tables: self.tables.clone(),
            db: self,
        }
    }

    pub fn hash_join(
        &self,
        table1: &str,
        col1: &str,
        table2: &str,
        col2: &str,
    ) -> Result<Vec<(&super::table::Row, &super::table::Row)>, crate::core::table::TableError> {
        let t1 = self.tables.get(table1).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table1))
        })?;
        let t2 = self.tables.get(table2).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table2))
        })?;

        let mut hash_map = std::collections::HashMap::new();
        // Build phase: use the smaller table if possible, but for simplicity let's use t1
        for r1 in t1.rows.values() {
            if let Some(val) = r1.data.get(col1) {
                hash_map.entry(val.clone()).or_insert_with(Vec::new).push(r1);
            }
        }

        let mut results = Vec::new();
        // Probe phase
        for r2 in t2.rows.values() {
            if let Some(val) = r2.data.get(col2) {
                if let Some(r1_list) = hash_map.get(val) {
                    for r1 in r1_list {
                        results.push((*r1, r2));
                    }
                }
            }
        }

        Ok(results)
    }
}

pub struct Transaction<'a> {
    pub db: &'a mut Database,
    pub original_tables: FastHashMap<String, Table>,
}

impl<'a> Transaction<'a> {
    pub fn commit(self) {
        // In this simple implementation, the changes are already in self.db.tables
        // So we just don't do anything (we "accept" the changes)
    }

    pub fn rollback(self) {
        // Restore the original state
        self.db.tables = self.original_tables;
    }
}
