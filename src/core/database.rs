use super::{FastHashMap, Id, RowData, Schema, Table, Value};
use crate::versioning::{ChangeType, VersionLog};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(
    Debug, Clone, Serialize, Deserialize, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
pub struct DatabaseSnapshot {
    pub name: String,
    pub tables: Vec<(String, Table)>,
    pub version_log: VersionLog,
}

#[derive(Debug, Clone)]
pub struct Database {
    pub name: String,
    pub tables: Arc<RwLock<FastHashMap<String, Arc<RwLock<Table>>>>>,
    pub version_log: Arc<RwLock<VersionLog>>,
    pub wal: Option<Arc<RwLock<crate::storage::wal::Wal>>>,
}

impl Serialize for Database {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let tables = self.tables.read();
        let version_log = self.version_log.read();

        let mut state = serializer.serialize_struct("Database", 3)?;
        state.serialize_field("name", &self.name)?;

        // Serialize tables as a map of names to Table objects (read-locked)
        struct TablesSerializer<'a>(&'a FastHashMap<String, Arc<RwLock<Table>>>);
        impl Serialize for TablesSerializer<'_> {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(Some(self.0.len()))?;
                for (name, table_lock) in self.0.iter() {
                    map.serialize_entry(name, &*table_lock.read())?;
                }
                map.end()
            }
        }

        state.serialize_field("tables", &TablesSerializer(&tables))?;
        state.serialize_field("version_log", &*version_log)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for Database {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct DatabaseData {
            name: String,
            tables: FastHashMap<String, Table>,
            version_log: VersionLog,
        }

        let data = DatabaseData::deserialize(deserializer)?;

        let mut tables = FastHashMap::default();
        for (name, table) in data.tables {
            tables.insert(name, Arc::new(RwLock::new(table)));
        }

        Ok(Database {
            name: data.name,
            tables: Arc::new(RwLock::new(tables)),
            version_log: Arc::new(RwLock::new(data.version_log)),
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

    pub fn snapshot(&self) -> DatabaseSnapshot {
        let tables_lock = self.tables.read();
        let mut tables = Vec::with_capacity(tables_lock.len());
        for (name, table_lock) in tables_lock.iter() {
            let mut table = table_lock.read().clone();
            table.prepare_for_archive();
            tables.push((name.clone(), table));
        }

        let mut version_log = self.version_log.read().clone();
        version_log.prepare_for_archive();

        DatabaseSnapshot {
            name: self.name.clone(),
            tables,
            version_log,
        }
    }

    pub fn from_snapshot(snapshot: DatabaseSnapshot) -> Self {
        let mut tables = FastHashMap::default();
        for (name, mut table) in snapshot.tables {
            table.rebuild_from_archive();
            tables.insert(name, Arc::new(RwLock::new(table)));
        }

        let mut version_log = snapshot.version_log;
        version_log.rebuild_from_archive();

        Database {
            name: snapshot.name,
            tables: Arc::new(RwLock::new(tables)),
            version_log: Arc::new(RwLock::new(version_log)),
            wal: None,
        }
    }

    /// Serialize the database via `rkyv` to `path` for mmap-backed loading.
    pub fn save_to_mmap(
        &self,
        path: impl Into<std::path::PathBuf>,
    ) -> Result<(), crate::storage::StorageError> {
        crate::storage::MmapStorage::new(path).save(self)
    }

    /// Load a database from a memory-mapped file at `path`.
    pub fn load_from_mmap(
        path: impl Into<std::path::PathBuf>,
    ) -> Result<Self, crate::storage::StorageError> {
        let mut storage = crate::storage::MmapStorage::new(path);
        storage.load_database()
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
        let table = tables
            .get(table_name)
            .ok_or_else(|| crate::core::table::TableError::InvalidColumn(table_name.to_string()))?;
        table.write().create_index(column_name)
    }

    pub fn create_unique_index(
        &self,
        table_name: &str,
        column_name: &str,
    ) -> Result<(), crate::core::table::TableError> {
        let tables = self.tables.read();
        let table = tables
            .get(table_name)
            .ok_or_else(|| crate::core::table::TableError::InvalidColumn(table_name.to_string()))?;
        table.write().create_unique_index(column_name)
    }

    pub fn find_unique_by_id(
        &self,
        table_name: &str,
        column_idx: usize,
        value: &super::Value,
    ) -> Option<super::table::Row> {
        let tables = self.tables.read();
        let table_lock = tables.get(table_name)?;
        table_lock.read().find_unique_by_id(column_idx, value)
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
                Ok(table.select(|r| expr.is_true(r, &table.column_map)))
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
                    .filter(|r| expr.is_true(r, &table.column_map))
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

        let build_col_idx = build_table.get_column_index(build_col).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!(
                "Column {} not found in {}",
                build_col, build_table.name
            ))
        })?;
        let probe_col_idx = probe_table.get_column_index(probe_col).ok_or_else(|| {
            crate::core::table::TableError::SchemaViolation(format!(
                "Column {} not found in {}",
                probe_col, probe_table.name
            ))
        })?;

        let num_build_rows = build_table.ids.len();
        let num_probe_rows = probe_table.ids.len();
        if num_build_rows == 0 || num_probe_rows == 0 {
            return Ok(Vec::new());
        }

        // --- FAST PATH: Direct Index Join ---
        // Case 1: Build table is the ID table
        if build_col == "id" && build_table.is_sequential_ids {
            let mut results = Vec::with_capacity(num_probe_rows.min(num_build_rows));

            for i in 0..num_probe_rows {
                let val = probe_table.get_value_by_index(i, probe_col_idx);
                if let crate::core::Value::Integer(idx_val) = val {
                    let idx = idx_val as usize;
                    if idx < num_build_rows {
                        let build_row = build_table.get_row_by_index(idx);
                        let probe_row = probe_table.get_row_by_index(i);
                        if reversed {
                            results.push((probe_row, build_row));
                        } else {
                            results.push((build_row, probe_row));
                        }
                    }
                }
            }
            return Ok(results);
        }

        // Case 2: Probe table is the ID table
        if probe_col == "id" && probe_table.is_sequential_ids {
            let mut results = Vec::with_capacity(num_build_rows.min(num_probe_rows));

            for i in 0..num_build_rows {
                let val = build_table.get_value_by_index(i, build_col_idx);
                if let crate::core::Value::Integer(idx_val) = val {
                    let idx = idx_val as usize;
                    if idx < num_probe_rows {
                        let build_row = build_table.get_row_by_index(i);
                        let probe_row = probe_table.get_row_by_index(idx);
                        if reversed {
                            results.push((probe_row, build_row));
                        } else {
                            results.push((build_row, probe_row));
                        }
                    }
                }
            }
            return Ok(results);
        }

        // Optimize: Use Power-of-2 bucket count for bitwise masking
        let buckets_count = (num_build_rows * 2).next_power_of_two();
        let bucket_mask = (buckets_count - 1) as u64;

        let mut heads = vec![-1i32; buckets_count];
        let mut nexts = vec![-1i32; num_build_rows];
        let mut build_hashes = vec![0u64; num_build_rows];

        let hasher = ahash::RandomState::new();
        let mut bloom_filter = [0u64; 1024]; // 64k bits

        // --- BUILD PHASE ---
        for i in 0..num_build_rows {
            let val = build_table.get_value_by_index(i, build_col_idx);
            if !val.is_null() {
                let h = hasher.hash_one(&val);
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

        let num_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);

        // PROBE PHASE (Single-threaded fast path)
        if num_probe_rows < 5000 || num_threads <= 1 {
            let mut results = Vec::new();
            for i in 0..num_probe_rows {
                let val = probe_table.get_value_by_index(i, probe_col_idx);
                if !val.is_null() {
                    let h = hasher.hash_one(&val);
                    let bit = (h & 0xFFFF) as usize;

                    // Fast Bloom Filter check
                    if (bloom_filter[bit >> 6] & (1 << (bit & 0x3F))) != 0 {
                        let bucket = (h & bucket_mask) as usize;
                        let mut build_idx_ptr = heads[bucket];
                        let mut probe_row_cache = None;

                        while build_idx_ptr != -1 {
                            let idx = build_idx_ptr as usize;
                            if build_hashes[idx] == h {
                                // Optimized comparison: use get_value_ref if possible
                                let match_ok = if build_col_idx != -1 && probe_col_idx != -1 {
                                    build_table.get_value_ref(idx, build_col_idx as usize)
                                        == probe_table.get_value_ref(i, probe_col_idx as usize)
                                } else {
                                    build_table.get_value_by_index(idx, build_col_idx) == val
                                };

                                if match_ok {
                                    if probe_row_cache.is_none() {
                                        probe_row_cache = Some(probe_table.get_row_by_index(i));
                                    }
                                    let build_row = build_table.get_row_by_index(idx);
                                    let probe_row = probe_row_cache.as_ref().unwrap().clone();
                                    if reversed {
                                        results.push((probe_row, build_row));
                                    } else {
                                        results.push((build_row, probe_row));
                                    }
                                }
                            }
                            build_idx_ptr = nexts[idx];
                        }
                    }
                }
            }
            return Ok(results);
        }

        // PROBE PHASE (Multi-threaded)
        let chunk_size = num_probe_rows.div_ceil(num_threads);
        let heads_ref = &heads;
        let nexts_ref = &nexts;
        let build_hashes_ref = &build_hashes;
        let bloom_filter_ref = &bloom_filter;
        let hasher_ref = &hasher;

        let results = std::thread::scope(|s| {
            let mut handles = Vec::new();
            for i in 0..num_threads {
                let start_idx = i * chunk_size;
                if start_idx >= num_probe_rows {
                    break;
                }
                let end_idx = (start_idx + chunk_size).min(num_probe_rows);

                handles.push(s.spawn(move || {
                    let mut local_results = Vec::new();
                    for j in start_idx..end_idx {
                        let val = probe_table.get_value_by_index(j, probe_col_idx);
                        if !val.is_null() {
                            let h = hasher_ref.hash_one(&val);
                            let bit = (h & 0xFFFF) as usize;

                            if (bloom_filter_ref[bit >> 6] & (1 << (bit & 0x3F))) != 0 {
                                let bucket = (h & bucket_mask) as usize;
                                let mut build_idx_ptr = heads_ref[bucket];
                                let mut probe_row_cache = None;

                                while build_idx_ptr != -1 {
                                    let idx = build_idx_ptr as usize;
                                    if build_hashes_ref[idx] == h {
                                        // Optimized comparison
                                        let match_ok = if build_col_idx != -1 && probe_col_idx != -1
                                        {
                                            build_table.get_value_ref(idx, build_col_idx as usize)
                                                == probe_table
                                                    .get_value_ref(j, probe_col_idx as usize)
                                        } else {
                                            build_table.get_value_by_index(idx, build_col_idx)
                                                == val
                                        };

                                        if match_ok {
                                            if probe_row_cache.is_none() {
                                                probe_row_cache =
                                                    Some(probe_table.get_row_by_index(j));
                                            }
                                            let build_row = build_table.get_row_by_index(idx);
                                            let probe_row =
                                                probe_row_cache.as_ref().unwrap().clone();
                                            if reversed {
                                                local_results.push((probe_row, build_row));
                                            } else {
                                                local_results.push((build_row, probe_row));
                                            }
                                        }
                                    }
                                    build_idx_ptr = nexts_ref[idx];
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
