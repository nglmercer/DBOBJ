use crate::core::{Id, RowData};
use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Insert,
    Update,
    Delete,
    BatchInsert { count: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionEntry {
    pub timestamp_ms: i64,
    pub table_name: String,
    pub row_id: Id,
    pub change_type: ChangeType,
    pub data: Option<RowData>,
}

impl VersionEntry {
    pub fn timestamp(&self) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(self.timestamp_ms).unwrap()
    }
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

    pub fn record(
        &mut self,
        table_name: String,
        row_id: Id,
        change_type: ChangeType,
        data: Option<RowData>,
    ) {
        let entry = VersionEntry {
            timestamp_ms: Utc::now().timestamp_millis(),
            table_name,
            row_id,
            change_type,
            data,
        };
        self.entries.push(entry);
    }

    /// Record a batch insert as a single entry. One timestamp, one String alloc.
    pub fn record_batch(&mut self, table_name: String, first_id: Id, count: usize) {
        let entry = VersionEntry {
            timestamp_ms: Utc::now().timestamp_millis(),
            table_name,
            row_id: first_id,
            change_type: ChangeType::BatchInsert { count },
            data: None,
        };
        self.entries.push(entry);
    }
}
