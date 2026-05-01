use super::{FastHashMap, Id, RowData, Schema, Table, Value};
use crate::versioning::{ChangeType, VersionLog};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Database {
    pub name: String,
    pub tables: Arc<RwLock<FastHashMap<String, Arc<RwLock<Table>>>>>,
    pub version_log: Arc<RwLock<VersionLog>>,
    pub wal: Option<Arc<RwLock<crate::storage::wal::Wal>>>,
}

#[derive(Serialize, Deserialize)]
struct DatabaseProxy {
    name: String,
    tables: FastHashMap<String, Table>,
    version_log: VersionLog,
}

impl Serialize for Database {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let tables = self.tables.read();
        let version_log = self.version_log.read();

        let mut proxy_tables = FastHashMap::default();
        for (name, table_lock) in tables.iter() {
            proxy_tables.insert(name.clone(), table_lock.read().clone());
        }

        let proxy = DatabaseProxy {
            name: self.name.clone(),
            tables: proxy_tables,
            version_log: version_log.clone(),
        };
        proxy.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Database {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let proxy = DatabaseProxy::deserialize(deserializer)?;

        let mut tables = FastHashMap::default();
        for (name, table) in proxy.tables {
            tables.insert(name, Arc::new(RwLock::new(table)));
        }

        Ok(Database {
            name: proxy.name,
            tables: Arc::new(RwLock::new(tables)),
            version_log: Arc::new(RwLock::new(proxy.version_log)),
            wal: None,
        })
    }
}

impl Database {
    pub fn new(name: String) -> Self {
        Self {
            name,
            tables: Arc::new(RwLock::new(FastHashMap::default())),
            version_log: Arc::new(RwLock::new(VersionLog::new())),
            wal: None,
        }
    }

    pub fn with_wal(mut self, wal: crate::storage::wal::Wal) -> Self {
        self.wal = Some(Arc::new(RwLock::new(wal)));
        self
    }

    pub fn create_table(&self, name: String, schema: Schema) {
        let table = Table::new(name.clone(), schema);
        self.tables
            .write()
            .insert(name, Arc::new(RwLock::new(table)));
    }

    pub fn get_table(&self, name: &str) -> Option<Arc<RwLock<Table>>> {
        self.tables.read().get(name).cloned()
    }

    pub fn create_index(
        &self,
        table_name: &str,
        column_name: &str,
    ) -> Result<(), crate::core::table::TableError> {
        let tables = self.tables.read();
        let table_lock = tables.get(table_name).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!(
                "Table {} not found",
                table_name
            ))
        })?;
        table_lock.write().create_index(column_name.into())
    }

    pub fn insert_batch(
        &self,
        table_name: &str,
        batch: Vec<RowData>,
    ) -> Result<Vec<Id>, crate::core::table::TableError> {
        let tables = self.tables.read();
        let table_lock = tables.get(table_name).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!(
                "Table {} not found",
                table_name
            ))
        })?;
        let mut table = table_lock.write();

        let ids = table.insert_batch(batch)?;

        // Log changes if WAL is enabled
        if let Some(wal_lock) = &self.wal {
            let mut wal = wal_lock.write();
            for id in &ids {
                let row = table.get(id).unwrap();
                let _ = wal.append(&crate::storage::wal::WalEntry {
                    table_name: table_name.to_string(),
                    row_id: id.clone(),
                    change_type: ChangeType::Insert,
                    data: Some(table.values_to_row(&row.data)),
                });
            }
        }

        // Record batch in version log (single entry)
        if let Some(first_id) = ids.first() {
            self.version_log.write().record_batch(
                table_name.to_string(),
                first_id.clone(),
                ids.len(),
            );
        }

        Ok(ids)
    }

    pub fn insert_batch_raw(
        &self,
        table_name: &str,
        batch: Vec<Box<[Value]>>,
    ) -> Result<Vec<Id>, crate::core::table::TableError> {
        let tables = self.tables.read();
        let table_lock = tables.get(table_name).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!(
                "Table {} not found",
                table_name
            ))
        })?;
        let mut table = table_lock.write();

        let ids = table.insert_batch_raw(batch)?;

        // Log changes if WAL is enabled
        if let Some(wal_lock) = &self.wal {
            let mut wal = wal_lock.write();
            for id in &ids {
                let row = table.get(id).unwrap();
                let _ = wal.append(&crate::storage::wal::WalEntry {
                    table_name: table_name.to_string(),
                    row_id: id.clone(),
                    change_type: ChangeType::Insert,
                    data: Some(table.values_to_row(&row.data)),
                });
            }
        }

        // Record batch in version log (single entry)
        if let Some(first_id) = ids.first() {
            self.version_log.write().record_batch(
                table_name.to_string(),
                first_id.clone(),
                ids.len(),
            );
        }

        Ok(ids)
    }

    pub fn insert_batch_values(
        &self,
        table_name: &str,
        batch: Vec<Vec<Value>>,
    ) -> Result<Vec<Id>, crate::core::table::TableError> {
        let tables = self.tables.read();
        let table_lock = tables.get(table_name).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!(
                "Table {} not found",
                table_name
            ))
        })?;
        let mut table = table_lock.write();

        let ids = table.insert_batch_values(batch)?;

        // Log changes if WAL is enabled
        if let Some(wal_lock) = &self.wal {
            let mut wal = wal_lock.write();
            for id in &ids {
                let row = table.get(id).unwrap();
                let _ = wal.append(&crate::storage::wal::WalEntry {
                    table_name: table_name.to_string(),
                    row_id: id.clone(),
                    change_type: ChangeType::Insert,
                    data: Some(table.values_to_row(&row.data)),
                });
            }
        }

        // Record batch in version log (single entry)
        if let Some(first_id) = ids.first() {
            self.version_log.write().record_batch(
                table_name.to_string(),
                first_id.clone(),
                ids.len(),
            );
        }

        Ok(ids)
    }

    pub fn insert_row(
        &self,
        table_name: &str,
        data: RowData,
        custom_id: Option<Id>,
    ) -> Result<Id, crate::core::table::TableError> {
        let tables = self.tables.read();
        let table_lock = tables.get(table_name).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!(
                "Table {} not found",
                table_name
            ))
        })?;

        let mut table = table_lock.write();
        let id = table.insert(data, custom_id)?;

        // Version log: lightweight entry (data already lives in the table)
        self.version_log.write().record(
            table_name.to_string(),
            id.clone(),
            ChangeType::Insert,
            None,
        );

        if let Some(wal_lock) = &self.wal {
            let mut wal = wal_lock.write();
            let row = table.get(&id).unwrap();
            let _ = wal.append(&crate::storage::wal::WalEntry {
                table_name: table_name.to_string(),
                row_id: id.clone(),
                change_type: ChangeType::Insert,
                data: Some(table.values_to_row(&row.data)),
            });
        }
        Ok(id)
    }

    pub fn update_row(
        &self,
        table_name: &str,
        id: &Id,
        data: RowData,
    ) -> Result<(), crate::core::table::TableError> {
        let tables = self.tables.read();
        let table_lock = tables.get(table_name).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!(
                "Table {} not found",
                table_name
            ))
        })?;

        let mut table = table_lock.write();
        table.update(id, data.clone())?;
        self.version_log.write().record(
            table_name.to_string(),
            id.clone(),
            ChangeType::Update,
            Some(data.clone()),
        );

        if let Some(wal_lock) = &self.wal {
            let mut wal = wal_lock.write();
            let _ = wal.append(&crate::storage::wal::WalEntry {
                table_name: table_name.to_string(),
                row_id: id.clone(),
                change_type: ChangeType::Update,
                data: Some(data),
            });
        }
        Ok(())
    }

    pub fn delete_row(
        &self,
        table_name: &str,
        id: &Id,
    ) -> Result<(), crate::core::table::TableError> {
        let tables = self.tables.read();
        let table_lock = tables.get(table_name).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!(
                "Table {} not found",
                table_name
            ))
        })?;

        let mut table = table_lock.write();
        if table.delete(id).is_some() {
            self.version_log.write().record(
                table_name.to_string(),
                id.clone(),
                ChangeType::Delete,
                None,
            );

            if let Some(wal_lock) = &self.wal {
                let mut wal = wal_lock.write();
                let _ = wal.append(&crate::storage::wal::WalEntry {
                    table_name: table_name.to_string(),
                    row_id: id.clone(),
                    change_type: ChangeType::Delete,
                    data: None,
                });
            }
            Ok(())
        } else {
            Err(crate::core::table::TableError::SchemaViolation(format!(
                "Row with ID {} not found",
                id
            )))
        }
    }

    pub fn query<F>(
        &self,
        table_name: &str,
        predicate: F,
    ) -> Result<Vec<super::table::Row>, crate::core::table::TableError>
    where
        F: Fn(&super::table::Row) -> bool + Send + Sync,
    {
        let tables = self.tables.read();
        let table_lock = tables.get(table_name).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!(
                "Table {} not found",
                table_name
            ))
        })?;
        let table = table_lock.read();
        Ok(table.select(predicate))
    }

    pub fn find(
        &self,
        table_name: &str,
        column_name: &str,
        value: super::Value,
    ) -> Result<Vec<super::table::Row>, crate::core::table::TableError> {
        let tables = self.tables.read();
        let table_lock = tables.get(table_name).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!(
                "Table {} not found",
                table_name
            ))
        })?;
        let table = table_lock.read();
        Ok(table.find_by_column(column_name, &value))
    }

    pub fn join<F>(
        &self,
        table1: &str,
        table2: &str,
        condition: F,
    ) -> Result<Vec<(super::table::Row, super::table::Row)>, crate::core::table::TableError>
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
        for i in 0..t1.ids.len() {
            let r1 = t1.get_row_by_index(i);
            for j in 0..t2.ids.len() {
                let r2 = t2.get_row_by_index(j);
                if condition(&r1, &r2) {
                    results.push((r1.clone(), r2.clone()));
                }
            }
        }
        Ok(results)
    }

    pub fn query_expr(
        &self,
        table_name: &str,
        expr: crate::core::query::Expr,
    ) -> Result<Vec<super::table::Row>, crate::core::table::TableError> {
        let tables = self.tables.read();
        let table_lock = tables.get(table_name).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!(
                "Table {} not found",
                table_name
            ))
        })?;
        let table = table_lock.read();

        let plan = expr.plan(&table);
        self.execute_plan(plan)
    }

    pub fn execute_plan(
        &self,
        plan: crate::core::query::QueryPlan,
    ) -> Result<Vec<super::table::Row>, crate::core::table::TableError> {
        match plan {
            crate::core::query::QueryPlan::FullScan(table_name, expr) => {
                let tables = self.tables.read();
                let table_lock = tables.get(table_name.as_str()).ok_or_else(|| {
                    crate::core::table::TableError::SchemaViolation(format!(
                        "Table {} not found",
                        table_name
                    ))
                })?;
                let table = table_lock.read();
                Ok(table.select(|r| expr.is_true(&r.data, &table.column_map)))
            }
            crate::core::query::QueryPlan::IndexScan(table_name, col, val) => {
                let tables = self.tables.read();
                let table_lock = tables.get(table_name.as_str()).ok_or_else(|| {
                    crate::core::table::TableError::SchemaViolation(format!(
                        "Table {} not found",
                        table_name
                    ))
                })?;
                let table = table_lock.read();
                Ok(table.find_by_column(&col, &val))
            }
            crate::core::query::QueryPlan::IndexFilteredScan(table_name, col, val, expr) => {
                let tables = self.tables.read();
                let table_lock = tables.get(table_name.as_str()).ok_or_else(|| {
                    crate::core::table::TableError::SchemaViolation(format!(
                        "Table {} not found",
                        table_name
                    ))
                })?;
                let table = table_lock.read();
                // Get candidates from index
                let candidates = table.find_by_column(&col, &val);
                Ok(candidates
                    .into_iter()
                    .filter(|r| expr.is_true(&r.data, &table.column_map))
                    .collect())
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

        let (build_table, build_col, probe_table, probe_col, reversed) =
            if t1.ids.len() <= t2.ids.len() {
                (&*t1, col1, &*t2, col2, false)
            } else {
                (&*t2, col2, &*t1, col1, true)
            };

        let build_col_idx = *build_table.column_map.get(build_col).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!(
                "Column {} not found in {}",
                build_col, build_table.name
            ))
        })?;
        let probe_col_idx = *probe_table.column_map.get(probe_col).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!(
                "Column {} not found in {}",
                probe_col, probe_table.name
            ))
        })?;

        let num_build_rows = build_table.ids.len();
        if num_build_rows == 0 {
            return Ok(Vec::new());
        }

        // Optimize: Use Power-of-2 bucket count for bitwise masking
        let buckets_count = (num_build_rows * 2).next_power_of_two();
        let bucket_mask = (buckets_count - 1) as u64;

        let mut heads = vec![-1i32; buckets_count];
        let mut nexts = vec![-1i32; num_build_rows];
        let mut build_hashes = vec![0u64; num_build_rows];

        let hasher = ahash::RandomState::new();
        let mut bloom_filter = [0u64; 1024]; // 64k bits

        // BUILD PHASE
        for i in 0..num_build_rows {
            let start = i * build_table.num_columns;
            let val = &build_table.data[start + build_col_idx];
            if !val.is_null() {
                let h = hasher.hash_one(val);
                build_hashes[i] = h;

                // Update Bloom Filter
                let bit = (h & 0xFFFF) as usize;
                bloom_filter[bit >> 6] |= 1 << (bit & 0x3F);

                // Update Linear Multimap
                let bucket = (h & bucket_mask) as usize;
                nexts[i] = heads[bucket];
                heads[bucket] = i as i32;
            }
        }

        let num_probe_rows = probe_table.ids.len();
        let num_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);

        // PROBE PHASE (Single-threaded fast path)
        if num_probe_rows < 5000 || num_threads <= 1 {
            let mut results = Vec::new();
            for i in 0..num_probe_rows {
                let start = i * probe_table.num_columns;
                let val = &probe_table.data[start + probe_col_idx];
                if !val.is_null() {
                    let h = hasher.hash_one(val);
                    let bit = (h & 0xFFFF) as usize;

                    // Fast Bloom Filter check
                    if (bloom_filter[bit >> 6] & (1 << (bit & 0x3F))) != 0 {
                        let bucket = (h & bucket_mask) as usize;
                        let mut build_idx = heads[bucket];
                        
                        while build_idx != -1 {
                            let idx = build_idx as usize;
                            // Check hash first (fast collision filter)
                            if build_hashes[idx] == h {
                                let build_start = idx * build_table.num_columns;
                                // Exact value check
                                if &build_table.data[build_start + build_col_idx] == val {
                                    let build_row = build_table.get_row_by_index(idx);
                                    let probe_row = probe_table.get_row_by_index(i);
                                    if reversed {
                                        results.push((probe_row, build_row));
                                    } else {
                                        results.push((build_row, probe_row));
                                    }
                                }
                            }
                            build_idx = nexts[idx];
                        }
                    }
                }
            }
            return Ok(results);
        }

        // PROBE PHASE (Multi-threaded)
        let chunk_size = (num_probe_rows + num_threads - 1) / num_threads;
        let heads_ref = &heads;
        let nexts_ref = &nexts;
        let build_hashes_ref = &build_hashes;
        let bloom_filter_ref = &bloom_filter;
        let hasher_ref = &hasher;

        let results = std::thread::scope(|s| {
            let mut handles = Vec::new();
            for i in 0..num_threads {
                let start_idx = i * chunk_size;
                if start_idx >= num_probe_rows { break; }
                let end_idx = (start_idx + chunk_size).min(num_probe_rows);

                handles.push(s.spawn(move || {
                    let mut local_results = Vec::new();
                    for j in start_idx..end_idx {
                        let probe_start = j * probe_table.num_columns;
                        let val = &probe_table.data[probe_start + probe_col_idx];
                        if !val.is_null() {
                            let h = hasher_ref.hash_one(val);
                            let bit = (h & 0xFFFF) as usize;

                            if (bloom_filter_ref[bit >> 6] & (1 << (bit & 0x3F))) != 0 {
                                let bucket = (h & bucket_mask) as usize;
                                let mut build_idx = heads_ref[bucket];
                                
                                while build_idx != -1 {
                                    let idx = build_idx as usize;
                                    if build_hashes_ref[idx] == h {
                                        let build_start = idx * build_table.num_columns;
                                        if &build_table.data[build_start + build_col_idx] == val {
                                            let build_row = build_table.get_row_by_index(idx);
                                            let probe_row = probe_table.get_row_by_index(j);
                                            if reversed {
                                                local_results.push((probe_row, build_row));
                                            } else {
                                                local_results.push((build_row, probe_row));
                                            }
                                        }
                                    }
                                    build_idx = nexts_ref[idx];
                                }
                            }
                        }
                    }
                    local_results
                }));
            }
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap())
                .collect()
        });

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

impl Database {
    pub fn recover_from_wal(&self) -> Result<(), crate::core::table::TableError> {
        let entries = if let Some(wal_lock) = &self.wal {
            let wal = wal_lock.read();
            wal.read_all().unwrap_or_default()
        } else {
            return Ok(());
        };

        for entry in entries {
            match entry.change_type {
                ChangeType::Insert => {
                    if let Some(data) = entry.data {
                        let _ = self.insert_row(&entry.table_name, data, Some(entry.row_id));
                    }
                }
                ChangeType::Update => {
                    if let Some(data) = entry.data {
                        let _ = self.update_row(&entry.table_name, &entry.row_id, data);
                    }
                }
                ChangeType::Delete => {
                    let _ = self.delete_row(&entry.table_name, &entry.row_id);
                }
                ChangeType::BatchInsert { .. } => {
                    // Batch inserts are replayed via WAL entries (individual inserts)
                }
            }
        }
        Ok(())
    }
}
