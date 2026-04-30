use std::fs;
use std::path::PathBuf;
use crate::core::Database;
use thiserror::Error;

pub mod adapter;
pub use adapter::{SerializerAdapter, BincodeAdapter, PostcardAdapter, FastBincodeAdapter};

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
}

pub struct Storage<S: SerializerAdapter> {
    path: PathBuf,
    adapter: S,
}

impl<S: SerializerAdapter> Storage<S> {
    pub fn new(path: impl Into<PathBuf>, adapter: S) -> Self {
        Self { 
            path: path.into(),
            adapter,
        }
    }

    pub fn save(&self, db: &Database) -> Result<(), StorageError> {
        // Implement backup: copy existing file to .bak
        if self.path.exists() {
            let mut backup_path = self.path.clone();
            backup_path.set_extension("bak");
            fs::copy(&self.path, backup_path)?;
        }

        let bytes = self.adapter.serialize(db)?;
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
        self.adapter.deserialize(&bytes)
    }
}
