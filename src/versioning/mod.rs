use serde::{Deserialize, Serialize};
use crate::core::{Id, RowData};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Insert,
    Update,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionEntry {
    pub timestamp: DateTime<Utc>,
    pub table_name: String,
    pub row_id: Id,
    pub change_type: ChangeType,
    pub data: Option<RowData>, // Snapshot of data after change
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VersionLog {
    pub entries: Vec<VersionEntry>,
}

impl VersionLog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn record(&mut self, table_name: String, row_id: Id, change_type: ChangeType, data: Option<RowData>) {
        let entry = VersionEntry {
            timestamp: Utc::now(),
            table_name,
            row_id,
            change_type,
            data,
        };
        self.entries.push(entry);
    }
}
