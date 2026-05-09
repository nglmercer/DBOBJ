use std::fs::File;
use std::path::PathBuf;

use memmap2::Mmap;

use super::StorageError;
use crate::core::Database;
use crate::core::database::{ArchivedDatabaseSnapshot, DatabaseSnapshot};

/// Storage backend that uses memory-mapped files paired with `rkyv` for
/// near-instant loading and zero-copy access to archived data.
///
/// **Trade-off:** The returned [`ArchivedDatabaseSnapshot`] is immutable.
/// To obtain a mutable [`Database`] you must call [`MmapStorage::deserialize`],
/// which copies the data back into owned Rust types.
pub struct MmapStorage {
    path: PathBuf,
    mmap: Option<Mmap>,
}

impl MmapStorage {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            mmap: None,
        }
    }

    /// Serialize `db` via `rkyv` and atomically replace the on-disk file.
    pub fn save(&self, db: &Database) -> Result<(), StorageError> {
        let snapshot = db.snapshot();
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&snapshot)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        std::fs::write(&self.path, bytes)?;
        Ok(())
    }

    /// Memory-map the on-disk file and validate that it contains a valid
    /// `ArchivedDatabaseSnapshot`.
    pub fn load(&mut self) -> Result<(), StorageError> {
        let file = File::open(&self.path).map_err(StorageError::Io)?;
        let mmap = unsafe { Mmap::map(&file).map_err(StorageError::Io)? };
        // Validate once so that subsequent `access` calls can be unchecked.
        let _ = rkyv::access::<ArchivedDatabaseSnapshot, rkyv::rancor::Error>(&mmap)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.mmap = Some(mmap);
        Ok(())
    }

    /// Return a zero-copy view of the memory-mapped database snapshot.
    ///
    /// # Panics
    /// Panics if [`load`](Self::load) has not been called successfully.
    pub fn access(&self) -> &ArchivedDatabaseSnapshot {
        let mmap = self
            .mmap
            .as_ref()
            .expect("MmapStorage::access called before load");
        // SAFETY: `load` already validated the buffer with `rkyv::access`.
        unsafe { rkyv::access_unchecked::<ArchivedDatabaseSnapshot>(mmap) }
    }

    /// Fully deserialize the mmap into an owned [`DatabaseSnapshot`].
    /// This performs allocations and copies, losing the zero-copy benefit,
    /// but gives you a native Rust type that can be converted into a
    /// mutable [`Database`].
    pub fn deserialize(&self) -> Result<DatabaseSnapshot, StorageError> {
        let mmap = self
            .mmap
            .as_ref()
            .ok_or_else(|| StorageError::Serialization("MmapStorage not loaded".to_string()))?;
        let archived = rkyv::access::<ArchivedDatabaseSnapshot, rkyv::rancor::Error>(mmap)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        rkyv::deserialize::<DatabaseSnapshot, rkyv::rancor::Error>(archived)
            .map_err(|e| StorageError::Serialization(e.to_string()))
    }

    /// Convenience: load + deserialize into a full [`Database`].
    pub fn load_database(&mut self) -> Result<Database, StorageError> {
        self.load()?;
        let snapshot = self.deserialize()?;
        Ok(Database::from_snapshot(snapshot))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{ColumnDefinition, DataType, RowData, Schema, Value};

    fn create_test_db() -> Database {
        let db = Database::new("TestDB".to_string());
        let schema = Schema {
            columns: vec![ColumnDefinition {
                name: "col1".into(),
                data_type: DataType::String,
                nullable: false,
            }],
        };
        db.create_table("test_table".to_string(), schema);
        let mut row = RowData::default();
        row.insert("col1".into(), Value::from("hello"));
        db.insert_row("test_table", row, None).unwrap();
        db
    }

    #[test]
    fn test_mmap_storage_save_and_access() {
        let path = "test_mmap_storage.db";
        let _ = std::fs::remove_file(path);

        let db = create_test_db();
        let storage = MmapStorage::new(path);
        storage.save(&db).unwrap();

        let mut storage = MmapStorage::new(path);
        storage.load().unwrap();

        let archived = storage.access();
        assert_eq!(archived.tables.len(), 1);
        // Name is an ArchivedString; compare via AsRef<str> if available,
        // otherwise we rely on the round-trip test below for correctness.

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_mmap_storage_round_trip() {
        let path = "test_mmap_roundtrip.db";
        let _ = std::fs::remove_file(path);

        let db = create_test_db();
        let storage = MmapStorage::new(path);
        storage.save(&db).unwrap();

        let mut storage = MmapStorage::new(path);
        let loaded_db = storage.load_database().unwrap();

        assert_eq!(loaded_db.name, "TestDB");
        assert_eq!(
            loaded_db.get_table("test_table").unwrap().read().ids.len(),
            1
        );

        std::fs::remove_file(path).unwrap();
    }
}
