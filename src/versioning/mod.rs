use crate::core::{Id, RowData};
use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use rkyv::{Archive, Serialize as RkyvSerialize, Deserialize as RkyvDeserialize};

#[derive(Debug, Clone, Serialize, Deserialize, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum ChangeType {
    Insert,
    Update,
    Delete,
    BatchInsert { count: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct VersionEntry {
    pub timestamp_ms: i64,
    pub table_name: String,
    pub row_id: Id,
    pub change_type: ChangeType,
    #[serde(skip)]
    #[rkyv(with = rkyv::with::Skip)]
    pub data: Option<RowData>,
    pub data_snapshot: Option<Vec<(compact_str::CompactString, crate::core::Value)>>, // Surrogate
}

impl VersionEntry {
    pub fn timestamp(&self) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(self.timestamp_ms).unwrap()
    }

    pub fn prepare_for_archive(&mut self) {
        if let Some(data) = &self.data {
            self.data_snapshot = Some(data.iter().map(|(k, v)| (k.clone(), v.clone())).collect());
        }
    }

    pub fn rebuild_from_archive(&mut self) {
        if let Some(snapshot) = &self.data_snapshot {
            self.data = Some(snapshot.iter().cloned().collect());
        }
        self.data_snapshot = None;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct VersionLog {
    pub entries: Vec<VersionEntry>,
}

impl VersionLog {
    pub fn prepare_for_archive(&mut self) {
        for entry in &mut self.entries {
            entry.prepare_for_archive();
        }
    }

    pub fn rebuild_from_archive(&mut self) {
        for entry in &mut self.entries {
            entry.rebuild_from_archive();
        }
    }

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
            data_snapshot: None,
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
            data_snapshot: None,
        };
        self.entries.push(entry);
    }
}
