use serde::{Deserialize, Serialize};
use crate::core::{Id, RowData};
use chrono::{DateTime, Utc, TimeZone};
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};

#[derive(Debug, Clone, Serialize, Deserialize, Archive, RkyvSerialize, RkyvDeserialize)]
#[archive(check_bytes)]
pub enum ChangeType {
    Insert,
    Update,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize, Archive, RkyvSerialize, RkyvDeserialize)]
#[archive(check_bytes)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Default, Archive, RkyvSerialize, RkyvDeserialize)]
#[archive(check_bytes)]
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
            timestamp_ms: Utc::now().timestamp_millis(),
            table_name,
            row_id,
            change_type,
            data,
        };
        self.entries.push(entry);
    }
}
