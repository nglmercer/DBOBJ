use crate::core::{Id, RowData};
use crate::versioning::ChangeType;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalEntry {
    pub table_name: String,
    pub row_id: Id,
    pub change_type: ChangeType,
    pub data: Option<RowData>,
}

pub struct Wal {
    file: File,
    path: PathBuf,
}

impl std::fmt::Debug for Wal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Wal").field("path", &self.path).finish()
    }
}

impl Wal {
    pub fn new(path: impl Into<PathBuf>) -> std::io::Result<Self> {
        let path = path.into();
        let file = OpenOptions::new().create(true).append(true).open(&path)?;

        Ok(Self { file, path })
    }

    pub fn append(&mut self, entry: &WalEntry) -> std::io::Result<()> {
        let bytes = serde_json::to_vec(entry).map_err(std::io::Error::other)?;
        self.file.write_all(&bytes)?;
        self.file.write_all(b"\n")?;
        self.file.flush()?;
        Ok(())
    }

    pub fn read_all(&self) -> std::io::Result<Vec<WalEntry>> {
        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }
            let entry: WalEntry = serde_json::from_str(&line).map_err(std::io::Error::other)?;
            entries.push(entry);
        }

        Ok(entries)
    }

    pub fn clear(&mut self) -> std::io::Result<()> {
        self.file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.path)?;
        Ok(())
    }
}
