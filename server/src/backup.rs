use crate::protocol::{BackupFormat, BackupInfo, RestoreMode};
use chrono::Utc;
use dbobj::{Database, DatabaseSnapshot};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

/// Errors that can occur during backup/restore operations
#[derive(Error, Debug)]
pub enum BackupError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Backup not found: {0}")]
    NotFound(String),

    #[error("Deserialization error: {0}")]
    Deserialization(String),
}

/// Metadata stored alongside each backup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMeta {
    pub id: String,           // UUID
    pub label: String,        // User-provided label
    pub format: BackupFormat,
    pub timestamp_ms: i64,    // Unix ms
    pub table_count: usize,
    pub total_rows: usize,
    pub file_size: u64,
}

impl BackupMeta {
    fn new(label: String, format: BackupFormat) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            label,
            format,
            timestamp_ms: Utc::now().timestamp_millis(),
            table_count: 0,
            total_rows: 0,
            file_size: 0,
        }
    }
}

/// Manages backup creation, listing, and restoration
pub struct BackupManager {
    backup_dir: PathBuf,
}

impl BackupManager {
    /// Create a backup manager that stores backups in `backup_dir`.
    pub fn new(backup_dir: impl Into<PathBuf>) -> Self {
        Self {
            backup_dir: backup_dir.into(),
        }
    }

    /// The directory where backups are stored.
    pub fn backup_dir(&self) -> &Path {
        &self.backup_dir
    }

    // ── create ──────────────────────────────────────────────────────

    /// Create a backup of the given database.
    ///
    /// The backup is stored as:
    ///   `{backup_dir}/{label}-{uuid}.dbobj` (native format)
    ///   or
    ///   `{backup_dir}/{label}-{uuid}.json` (JSON format)
    pub fn create_backup(
        &self,
        db: &Arc<Database>,
        label: String,
        format: BackupFormat,
    ) -> Result<BackupInfo, BackupError> {
        fs::create_dir_all(&self.backup_dir)?;

        let mut meta = BackupMeta::new(label.clone(), format);

        // Compute table/row counts
        let snapshot = db.snapshot();
        meta.table_count = snapshot.tables.len();
        meta.total_rows = snapshot
            .tables
            .iter()
            .map(|(_, t)| t.ids.len())
            .sum();

        // Serialize
        let bytes: Vec<u8> = match format {
            BackupFormat::Native => {
                bincode::serialize(&snapshot)
                    .map_err(|e| BackupError::Serialization(e.to_string()))?
            }
            BackupFormat::Json => {
                serde_json::to_vec_pretty(&snapshot)
                    .map_err(|e| BackupError::Serialization(e.to_string()))?
            }
        };

        meta.file_size = bytes.len() as u64;

        // Write data
        let ext = format.extension();
        let filename = format!("{}-{}.{}", sanitise_label(&label), meta.id, ext);
        let path = self.backup_dir.join(&filename);

        // Write metadata sidecar
        let meta_path = path.with_extension(format!("{}.meta", ext));
        let meta_bytes =
            serde_json::to_vec_pretty(&meta).map_err(|e| BackupError::Serialization(e.to_string()))?;
        fs::write(&meta_path, meta_bytes)?;

        // Write backup data
        fs::write(&path, bytes)?;

        Ok(BackupInfo {
            id: meta.id,
            label: meta.label,
            format: meta.format,
            timestamp_ms: meta.timestamp_ms,
            table_count: meta.table_count,
            total_rows: meta.total_rows,
            file_size: meta.file_size,
            path: path.to_string_lossy().to_string(),
        })
    }

    // ── restore ─────────────────────────────────────────────────────

    /// Restore a backup by ID. Returns a new `Database` reconstructed
    /// from the backup snapshot, leaving the current database untouched.
    pub fn restore_backup(
        &self,
        backup_id: &str,
        mode: RestoreMode,
    ) -> Result<Database, BackupError> {
        let backup_path = self.find_backup_file(backup_id)?;

        // Read the format from the metadata file
        let meta_path = backup_path.with_extension(
            backup_path
                .extension()
                .map(|e| format!("{}.meta", e.to_string_lossy()))
                .unwrap_or_else(|| "meta".to_string()),
        );

        let format = if meta_path.exists() {
            let meta_bytes = fs::read(&meta_path)?;
            let meta: BackupMeta = serde_json::from_slice(&meta_bytes)
                .map_err(|e| BackupError::Deserialization(e.to_string()))?;
            meta.format
        } else {
            // Infer from extension
            match backup_path.extension().and_then(|e| e.to_str()) {
                Some("json") => BackupFormat::Json,
                _ => BackupFormat::Native,
            }
        };

        let bytes = fs::read(&backup_path)?;
        let snapshot: DatabaseSnapshot = match format {
            BackupFormat::Native => bincode::deserialize(&bytes)
                .map_err(|e| BackupError::Deserialization(e.to_string()))?,
            BackupFormat::Json => serde_json::from_slice(&bytes)
                .map_err(|e| BackupError::Deserialization(e.to_string()))?,
        };

        let db = Database::from_snapshot(snapshot);

        // If mode is InPlace, we could merge into an existing DB,
        // but for now both modes return a fresh DB.  The caller
        // (backend) decides how to apply it.
        let _ = mode;
        Ok(db)
    }

    // ── list ────────────────────────────────────────────────────────

    /// List all backups stored in the backup directory.
    pub fn list_backups(&self) -> Result<Vec<BackupInfo>, BackupError> {
        if !self.backup_dir.exists() {
            return Ok(Vec::new());
        }

        let mut backups = Vec::new();

        // Look for .dbobj and .json files; skip meta files
        for entry in fs::read_dir(&self.backup_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            // Skip meta files themselves
            let fname = path.file_name().unwrap_or_default().to_string_lossy();
            if fname.ends_with(".meta") {
                continue;
            }

            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "dbobj" && ext != "json" {
                continue;
            }

            // Try to read metadata sidecar
            let meta_path = path.with_extension(format!("{}.meta", ext));
            let meta: Option<BackupMeta> = if meta_path.exists() {
                fs::read(&meta_path)
                    .ok()
                    .and_then(|b| serde_json::from_slice(&b).ok())
            } else {
                None
            };

            let info = match meta {
                Some(m) => BackupInfo {
                    id: m.id,
                    label: m.label,
                    format: m.format,
                    timestamp_ms: m.timestamp_ms,
                    table_count: m.table_count,
                    total_rows: m.total_rows,
                    file_size: m.file_size,
                    path: path.to_string_lossy().to_string(),
                },
                None => {
                    // Synthesize from filename
                    let label = fname
                        .trim_end_matches(&format!(".{}", ext))
                        .to_string();
                    BackupInfo {
                        id: "unknown".into(),
                        label,
                        format: if ext == "json" {
                            BackupFormat::Json
                        } else {
                            BackupFormat::Native
                        },
                        timestamp_ms: path
                            .metadata()
                            .ok()
                            .and_then(|m| m.created().ok())
                            .map(|t| {
                                t.duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis() as i64
                            })
                            .unwrap_or(0),
                        table_count: 0,
                        total_rows: 0,
                        file_size: entry.metadata().map(|m| m.len()).unwrap_or(0),
                        path: path.to_string_lossy().to_string(),
                    }
                }
            };
            backups.push(info);
        }

        // Most recent first
        backups.sort_by(|a, b| b.timestamp_ms.cmp(&a.timestamp_ms));
        Ok(backups)
    }

    // ── delete ──────────────────────────────────────────────────────

    /// Delete a backup by ID.
    pub fn delete_backup(&self, backup_id: &str) -> Result<(), BackupError> {
        let path = self.find_backup_file(backup_id)?;

        // Remove data file
        fs::remove_file(&path)?;

        // Remove metadata sidecar if present
        let ext = path.extension().unwrap_or_default();
        let meta_path = path.with_extension(format!("{}.meta", ext));
        if meta_path.exists() {
            let _ = fs::remove_file(&meta_path);
        }

        Ok(())
    }

    // ── helpers ─────────────────────────────────────────────────────

    fn find_backup_file(&self, backup_id: &str) -> Result<PathBuf, BackupError> {
        if !self.backup_dir.exists() {
            return Err(BackupError::NotFound(backup_id.to_string()));
        }

        for entry in fs::read_dir(&self.backup_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let fname = path.file_name().unwrap_or_default().to_string_lossy();
            if fname.ends_with(".meta") {
                continue;
            }

            // Check if filename contains the ID
            if fname.contains(backup_id) {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ext == "dbobj" || ext == "json" {
                    return Ok(path);
                }
            }
        }

        Err(BackupError::NotFound(backup_id.to_string()))
    }
}

/// Sanitise a label so it can safely be used as a filename component.
fn sanitise_label(label: &str) -> String {
    label
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}