use std::sync::Arc;
use std::fs;

use dbobj::{Database, Value, RowData};
use dbobj_server::backup::BackupManager;
use dbobj_server::protocol::{BackupFormat, RestoreMode};

fn setup_test_db() -> Arc<Database> {
    let db = Database::new("test_backup".to_string());
    let schema = dbobj::Schema {
        columns: vec![
            dbobj::ColumnDefinition {
                name: "name".into(),
                data_type: dbobj::DataType::String,
                nullable: false,
            },
            dbobj::ColumnDefinition {
                name: "value".into(),
                data_type: dbobj::DataType::Integer,
                nullable: false,
            },
        ],
    };
    db.create_table("items".to_string(), schema);

    // Insert some rows using RowData (HashMap)
    let mut row = RowData::default();
    row.insert("name".into(), Value::String("alpha".into()));
    row.insert("value".into(), Value::Integer(10));
    db.insert_row("items", row, None).ok();

    let mut row = RowData::default();
    row.insert("name".into(), Value::String("beta".into()));
    row.insert("value".into(), Value::Integer(20));
    db.insert_row("items", row, None).ok();

    let mut row = RowData::default();
    row.insert("name".into(), Value::String("gamma".into()));
    row.insert("value".into(), Value::Integer(30));
    db.insert_row("items", row, None).ok();

    Arc::new(db)
}

fn setup() -> (BackupManager, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");
    let mgr = BackupManager::new(dir.path().join("backups"));
    (mgr, dir)
}

#[test]
fn test_create_native_backup() {
    let (mgr, _dir) = setup();
    let db = setup_test_db();

    let info = mgr
        .create_backup(&db, "test-native".into(), BackupFormat::Native)
        .expect("Failed to create native backup");

    assert_eq!(info.label, "test-native");
    assert_eq!(info.format, BackupFormat::Native);
    assert_eq!(info.table_count, 1);
    assert_eq!(info.total_rows, 3);
    assert!(info.file_size > 0);
    assert!(!info.id.is_empty());
    assert!(!info.path.is_empty());
    assert!(std::path::Path::new(&info.path).exists());
}

#[test]
fn test_create_json_backup() {
    let (mgr, _dir) = setup();
    let db = setup_test_db();

    let info = mgr
        .create_backup(&db, "test-json".into(), BackupFormat::Json)
        .expect("Failed to create JSON backup");

    assert_eq!(info.format, BackupFormat::Json);
    assert!(info.path.ends_with(".json"));
    assert_eq!(info.total_rows, 3);
}

#[test]
fn test_list_backups() {
    let (mgr, _dir) = setup();
    let db = setup_test_db();

    // Initially empty
    let list = mgr.list_backups().expect("Failed to list backups");
    assert_eq!(list.len(), 0, "Should be empty initially");

    // Create a backup
    mgr.create_backup(&db, "first".into(), BackupFormat::Native)
        .expect("Failed to create backup");

    let list = mgr.list_backups().expect("Failed to list backups");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].label, "first");
    assert_eq!(list[0].total_rows, 3);

    // Create another
    mgr.create_backup(&db, "second".into(), BackupFormat::Json)
        .expect("Failed to create backup");

    let list = mgr.list_backups().expect("Failed to list backups");
    assert_eq!(list.len(), 2);
    // Most recent first
    assert_eq!(list[0].label, "second");
    assert_eq!(list[1].label, "first");
}

#[test]
fn test_delete_backup() {
    let (mgr, _dir) = setup();
    let db = setup_test_db();

    let info = mgr
        .create_backup(&db, "delete-me".into(), BackupFormat::Native)
        .expect("Failed to create backup");

    // Verify it exists
    assert!(std::path::Path::new(&info.path).exists());

    // Delete it
    mgr.delete_backup(&info.id)
        .expect("Failed to delete backup");

    // Verify it's gone
    assert!(!std::path::Path::new(&info.path).exists());

    // List should be empty
    let list = mgr.list_backups().expect("Failed to list backups");
    assert_eq!(list.len(), 0);
}

#[test]
fn test_restore_native_backup() {
    let (mgr, _dir) = setup();
    let db = setup_test_db();

    let info = mgr
        .create_backup(&db, "restore-test".into(), BackupFormat::Native)
        .expect("Failed to create backup");

    let restored = mgr
        .restore_backup(&info.id, RestoreMode::Replace)
        .expect("Failed to restore backup");

    // Verify restored database has the same data
    assert_eq!(restored.name, "test_backup");
    let tables = restored.tables.read();
    assert_eq!(tables.len(), 1);
    let items = tables.get("items").expect("Table 'items' should exist");
    let guard = items.read();
    assert_eq!(guard.ids.len(), 3, "Should have 3 rows restored");
}

#[test]
fn test_restore_json_backup() {
    let (mgr, _dir) = setup();
    let db = setup_test_db();

    let info = mgr
        .create_backup(&db, "restore-json".into(), BackupFormat::Json)
        .expect("Failed to create JSON backup");

    let restored = mgr
        .restore_backup(&info.id, RestoreMode::Replace)
        .expect("Failed to restore JSON backup");

    assert_eq!(restored.name, "test_backup");
    let tables = restored.tables.read();
    let items = tables.get("items").expect("Table 'items' should exist");
    let guard = items.read();
    assert_eq!(guard.ids.len(), 3);
}

#[test]
fn test_restore_nonexistent_backup() {
    let (mgr, _dir) = setup();

    let result = mgr.restore_backup("nonexistent-id", RestoreMode::Replace);
    assert!(result.is_err(), "Should fail for nonexistent backup");
}

#[test]
fn test_delete_nonexistent_backup() {
    let (mgr, _dir) = setup();

    let result = mgr.delete_backup("nonexistent-id");
    assert!(result.is_err(), "Should fail for nonexistent backup");
}

#[test]
fn test_backup_dir_creation() {
    let (mgr, _dir) = setup();
    let db = setup_test_db();

    // Dir should not exist before first backup
    assert!(!mgr.backup_dir().exists(), "Backup dir should not exist yet");

    mgr.create_backup(&db, "dir-test".into(), BackupFormat::Native)
        .expect("Failed to create backup");

    // Dir should now exist
    assert!(mgr.backup_dir().exists(), "Backup dir should exist after creating a backup");

    // Should have 2 files: data + .meta
    let entries: Vec<_> = fs::read_dir(mgr.backup_dir())
        .expect("Failed to read backup dir")
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(entries.len(), 2, "Should have data file and meta file");
}

#[test]
fn test_backup_metadata() {
    let (mgr, _dir) = setup();
    let db = setup_test_db();

    let info = mgr
        .create_backup(&db, "meta-test".into(), BackupFormat::Native)
        .expect("Failed to create backup");

    // Verify metadata file exists
    let backup_path = std::path::Path::new(&info.path);
    let meta_path = backup_path.with_extension("dbobj.meta");
    assert!(meta_path.exists(), "Metadata file should exist");

    // Read and verify metadata
    let meta_bytes = fs::read(&meta_path).expect("Failed to read meta file");
    let meta: serde_json::Value =
        serde_json::from_slice(&meta_bytes).expect("Failed to parse meta");
    assert_eq!(meta["label"], "meta-test");
    assert_eq!(meta["format"], "Native");
    assert_eq!(meta["table_count"], 1);
    assert_eq!(meta["total_rows"], 3);
}