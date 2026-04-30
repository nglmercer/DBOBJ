use std::fs;
use std::path::PathBuf;
use crate::core::Database;
use rkyv::{self, ser::serializers::AllocSerializer, ser::Serializer, Deserialize as RkyvDeserialize, Archive};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
}

pub struct Storage {
    path: PathBuf,
}

impl Storage {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn save(&self, db: &Database) -> Result<(), StorageError> {
        // Implement backup: copy existing file to .bak
        if self.path.exists() {
            let mut backup_path = self.path.clone();
            backup_path.set_extension("bak");
            fs::copy(&self.path, backup_path)?;
        }

        // rkyv serialization
        let mut serializer = AllocSerializer::<4096>::default();
        serializer.serialize_value(db)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let bytes = serializer.into_serializer().into_inner();

        fs::write(&self.path, bytes)?;
        Ok(())
    }

    pub fn load(&self) -> Result<Database, StorageError> {
        if !self.path.exists() {
            return Err(StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Database file not found",
            )));
        }

        let bytes = fs::read(&self.path)?;
        
        // Use validation for safety
        let archived = rkyv::check_archived_root::<Database>(&bytes)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        
        let db: Database = archived.deserialize(&mut rkyv::Infallible)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        Ok(db)
    }
}
