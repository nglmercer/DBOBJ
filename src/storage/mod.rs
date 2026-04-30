use std::fs;
use std::path::PathBuf;
use crate::core::Database;
use postcard;
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

        // Postcard serialization
        let bytes = postcard::to_stdvec(db)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

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
        
        let db: Database = postcard::from_bytes(&bytes)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        Ok(db)
    }
}
