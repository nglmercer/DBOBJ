use super::{Table, Schema, Id, RowData, FastHashMap};
use crate::versioning::{VersionLog, ChangeType};
use serde::{Deserialize, Serialize};
use parking_lot::RwLock;
use std::sync::Arc;
use serde::ser::SerializeStruct;
use serde::de::{self, Deserializer, Visitor, MapAccess};
use std::fmt;

#[derive(Debug, Clone)]
pub struct Database {
    pub name: String,
    pub tables: Arc<RwLock<FastHashMap<String, Arc<RwLock<Table>>>>>,
    pub version_log: Arc<RwLock<VersionLog>>,
}

impl Serialize for Database {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let tables = self.tables.read();
        let version_log = self.version_log.read();
        
        let mut state = serializer.serialize_struct("Database", 3)?;
        state.serialize_field("name", &self.name)?;
        
        // Convert Arc<RwLock<Table>> to Table for serialization
        let mut serializable_tables = FastHashMap::default();
        for (name, table_lock) in tables.iter() {
            serializable_tables.insert(name.clone(), table_lock.read().clone());
        }
        
        state.serialize_field("tables", &serializable_tables)?;
        state.serialize_field("version_log", &*version_log)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for Database {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field { Name, Tables, VersionLog }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("`name`, `tables` or `version_log`")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "name" => Ok(Field::Name),
                            "tables" => Ok(Field::Tables),
                            "version_log" => Ok(Field::VersionLog),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct DatabaseVisitor;

        impl<'de> Visitor<'de> for DatabaseVisitor {
            type Value = Database;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct Database")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Database, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut name = None;
                let mut tables: Option<FastHashMap<String, Table>> = None;
                let mut version_log = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Name => {
                            if name.is_some() {
                                return Err(de::Error::duplicate_field("name"));
                            }
                            name = Some(map.next_value()?);
                        }
                        Field::Tables => {
                            if tables.is_some() {
                                return Err(de::Error::duplicate_field("tables"));
                            }
                            tables = Some(map.next_value()?);
                        }
                        Field::VersionLog => {
                            if version_log.is_some() {
                                return Err(de::Error::duplicate_field("version_log"));
                            }
                            version_log = Some(map.next_value()?);
                        }
                    }
                }
                let name = name.ok_or_else(|| de::Error::missing_field("name"))?;
                let raw_tables = tables.ok_or_else(|| de::Error::missing_field("tables"))?;
                let version_log_raw = version_log.ok_or_else(|| de::Error::missing_field("version_log"))?;

                let mut tables = FastHashMap::default();
                for (t_name, table) in raw_tables {
                    tables.insert(t_name, Arc::new(RwLock::new(table)));
                }

                Ok(Database {
                    name,
                    tables: Arc::new(RwLock::new(tables)),
                    version_log: Arc::new(RwLock::new(version_log_raw)),
                })
            }
        }

        const FIELDS: &[&str] = &["name", "tables", "version_log"];
        deserializer.deserialize_struct("Database", FIELDS, DatabaseVisitor)
    }
}

impl Database {
    pub fn new(name: String) -> Self {
        Self {
            name,
            tables: Arc::new(RwLock::new(FastHashMap::default())),
            version_log: Arc::new(RwLock::new(VersionLog::new())),
        }
    }

    pub fn create_table(&self, name: String, schema: Schema) {
        let table = Table::new(name.clone(), schema);
        self.tables.write().insert(name, Arc::new(RwLock::new(table)));
    }

    pub fn get_table(&self, name: &str) -> Option<Arc<RwLock<Table>>> {
        self.tables.read().get(name).cloned()
    }

    pub fn create_index(&self, table_name: &str, column_name: &str) -> Result<(), crate::core::table::TableError> {
        let tables = self.tables.read();
        let table_lock = tables.get(table_name).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table_name))
        })?;
        table_lock.write().create_index(column_name.into())
    }

    pub fn insert_row(&self, table_name: &str, data: RowData, custom_id: Option<Id>) -> Result<Id, crate::core::table::TableError> {
        let tables = self.tables.read();
        let table_lock = tables.get(table_name).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table_name))
        })?;
        
        let mut table = table_lock.write();
        let id = table.insert(data.clone(), custom_id)?;
        self.version_log.write().record(table_name.to_string(), id.clone(), ChangeType::Insert, Some(data));
        Ok(id)
    }

    pub fn update_row(&self, table_name: &str, id: &Id, data: RowData) -> Result<(), crate::core::table::TableError> {
        let tables = self.tables.read();
        let table_lock = tables.get(table_name).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table_name))
        })?;
        
        let mut table = table_lock.write();
        table.update(id, data.clone())?;
        self.version_log.write().record(table_name.to_string(), id.clone(), ChangeType::Update, Some(data));
        Ok(())
    }

    pub fn delete_row(&self, table_name: &str, id: &Id) -> Result<(), crate::core::table::TableError> {
        let tables = self.tables.read();
        let table_lock = tables.get(table_name).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table_name))
        })?;
        
        let mut table = table_lock.write();
        if table.delete(id).is_some() {
            self.version_log.write().record(table_name.to_string(), id.clone(), ChangeType::Delete, None);
            Ok(())
        } else {
            Err(crate::core::table::TableError::SchemaViolation(format!("Row with ID {} not found", id)))
        }
    }

    pub fn query<F>(&self, table_name: &str, predicate: F) -> Result<Vec<super::table::Row>, crate::core::table::TableError>
    where
        F: Fn(&super::table::Row) -> bool,
    {
        let tables = self.tables.read();
        let table_lock = tables.get(table_name).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table_name))
        })?;
        let table = table_lock.read();
        Ok(table.select(predicate).into_iter().cloned().collect())
    }

    pub fn find(&self, table_name: &str, column_name: &str, value: super::Value) -> Result<Vec<super::table::Row>, crate::core::table::TableError> {
        let tables = self.tables.read();
        let table_lock = tables.get(table_name).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table_name))
        })?;
        let table = table_lock.read();
        Ok(table.find_by_column(column_name, &value).into_iter().cloned().collect())
    }

    pub fn join<F>(&self, table1: &str, table2: &str, condition: F) -> Result<Vec<(super::table::Row, super::table::Row)>, crate::core::table::TableError>
    where
        F: Fn(&super::table::Row, &super::table::Row) -> bool,
    {
        let tables = self.tables.read();
        let t1_lock = tables.get(table1).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table1))
        })?;
        let t2_lock = tables.get(table2).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table2))
        })?;

        let t1 = t1_lock.read();
        let t2 = t2_lock.read();

        let mut results = Vec::new();
        for r1 in t1.rows.values() {
            for r2 in t2.rows.values() {
                if condition(r1, r2) {
                    results.push((r1.clone(), r2.clone()));
                }
            }
        }
        Ok(results)
    }

    pub fn query_expr(&self, table_name: &str, expr: crate::core::query::Expr) -> Result<Vec<super::table::Row>, crate::core::table::TableError> {
        let tables = self.tables.read();
        let table_lock = tables.get(table_name).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table_name))
        })?;
        let table = table_lock.read();
        
        let plan = expr.plan(&table);
        self.execute_plan(plan)
    }

    pub fn execute_plan(&self, plan: crate::core::query::QueryPlan) -> Result<Vec<super::table::Row>, crate::core::table::TableError> {
        match plan {
            crate::core::query::QueryPlan::FullScan(table_name, expr) => {
                let tables = self.tables.read();
                let table_lock = tables.get(table_name.as_str()).ok_or_else(|| {
                    crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table_name))
                })?;
                let table = table_lock.read();
                Ok(table.select(|r| expr.is_true(&r.data)).into_iter().cloned().collect())
            }
            crate::core::query::QueryPlan::IndexScan(table_name, col, val) => {
                let tables = self.tables.read();
                let table_lock = tables.get(table_name.as_str()).ok_or_else(|| {
                    crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table_name))
                })?;
                let table = table_lock.read();
                Ok(table.find_by_column(col.as_str(), &val).into_iter().cloned().collect())
            }
        }
    }

    pub fn begin_transaction(&self) -> Transaction<'_> {
        let mut original_tables = FastHashMap::default();
        let tables_guard = self.tables.read();
        for (name, table_lock) in tables_guard.iter() {
            original_tables.insert(name.clone(), table_lock.read().clone());
        }
        
        Transaction {
            db: self,
            original_tables,
        }
    }

    pub fn hash_join(
        &self,
        table1: &str,
        col1: &str,
        table2: &str,
        col2: &str,
    ) -> Result<Vec<(super::table::Row, super::table::Row)>, crate::core::table::TableError> {
        let tables = self.tables.read();
        let t1_lock = tables.get(table1).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table1))
        })?;
        let t2_lock = tables.get(table2).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!("Table {} not found", table2))
        })?;

        let t1 = t1_lock.read();
        let t2 = t2_lock.read();

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
                        results.push(((*r1).clone(), r2.clone()));
                    }
                }
            }
        }

        Ok(results)
    }
}

pub struct Transaction<'a> {
    pub db: &'a Database,
    pub original_tables: FastHashMap<String, Table>,
}

impl<'a> Transaction<'a> {
    pub fn commit(self) {
        // Acceptance is implicit in this model
    }

    pub fn rollback(self) {
        let mut tables = self.db.tables.write();
        for (name, table) in self.original_tables {
            tables.insert(name, Arc::new(RwLock::new(table)));
        }
    }
}
