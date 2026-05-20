use std::sync::Arc;

use dbobj::{Database, Value, RowData};
use dbobj_server::migration::{Migration, MigrationAction, MigrationRunner};
use dbobj_server::protocol::ColumnDef;

fn setup_db() -> Arc<Database> {
    let db = Database::new("test_migrations".to_string());
    let schema = dbobj::Schema {
        columns: vec![
            dbobj::ColumnDefinition {
                name: "name".into(),
                data_type: dbobj::DataType::String,
                nullable: false,
            },
            dbobj::ColumnDefinition {
                name: "age".into(),
                data_type: dbobj::DataType::Integer,
                nullable: false,
            },
        ],
    };
    db.create_table("users".to_string(), schema);
    Arc::new(db)
}

fn setup_runner() -> MigrationRunner {
    let db = setup_db();
    MigrationRunner::new(db)
}

#[test]
fn test_tracking_table_created() {
    let db = setup_db();
    let runner = MigrationRunner::new(db.clone());

    // Tracking table should exist
    let tables = db.tables.read();
    assert!(
        tables.contains_key(MigrationRunner::TRACKING_TABLE),
        "Tracking table should be created"
    );

    // Verify schema
    let tbl = tables.get(MigrationRunner::TRACKING_TABLE).unwrap();
    let guard = tbl.read();
    assert_eq!(guard.num_columns, 4);
    assert!(guard.column_map.contains_key("id"));
    assert!(guard.column_map.contains_key("name"));
    assert!(guard.column_map.contains_key("applied_at"));
    assert!(guard.column_map.contains_key("checksum"));
}

#[test]
fn test_no_pending_initially() {
    let runner = setup_runner();
    let pending = runner.pending_list();
    assert_eq!(pending.len(), 0, "No pending migrations initially");
}

#[test]
fn test_register_and_run_add_column() {
    let db = setup_db();
    let mut runner = MigrationRunner::new(db.clone());

    // Register a migration to add a column
    let migration = Migration::new("add_email", "Add email column to users")
        .add_step(MigrationAction::AddColumn {
            table: "users".into(),
            column: ColumnDef {
                name: "email".into(),
                data_type: "String".into(),
                nullable: true,
            },
            default_value: None,
        });

    runner.register(migration);

    // Should be pending
    let pending = runner.pending_list();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0], "add_email");

    // Run it
    let statuses = runner.run_pending().expect("Failed to run migrations");
    assert_eq!(statuses.len(), 1);
    assert!(statuses[0].success, "Migration should succeed");
    assert_eq!(statuses[0].name, "add_email");

    // Verify column was added
    let tables = db.tables.read();
    let users = tables.get("users").unwrap();
    let guard = users.read();
    assert!(guard.column_map.contains_key("email"));
    assert_eq!(guard.num_columns, 3);
}

#[test]
fn test_register_and_run_rename_column() {
    let db = setup_db();
    let mut runner = MigrationRunner::new(db.clone());

    let migration = Migration::new("rename_col", "Rename age to years")
        .add_step(MigrationAction::RenameColumn {
            table: "users".into(),
            old_name: "age".into(),
            new_name: "years".into(),
        });

    runner.register(migration);
    runner.run_pending().expect("Failed to run migration");

    let tables = db.tables.read();
    let users = tables.get("users").unwrap();
    let guard = users.read();
    assert!(!guard.column_map.contains_key("age"));
    assert!(guard.column_map.contains_key("years"));
    assert_eq!(guard.num_columns, 2);
}

#[test]
fn test_register_and_run_rename_table() {
    let db = setup_db();
    let mut runner = MigrationRunner::new(db.clone());

    let migration = Migration::new("rename_table", "Rename users to employees")
        .add_step(MigrationAction::RenameTable {
            old_name: "users".into(),
            new_name: "employees".into(),
        });

    runner.register(migration);
    runner.run_pending().expect("Failed to run migration");

    let tables = db.tables.read();
    assert!(!tables.contains_key("users"));
    assert!(tables.contains_key("employees"));
}

#[test]
fn test_register_and_run_drop_table() {
    let db = setup_db();
    let mut runner = MigrationRunner::new(db.clone());

    let migration = Migration::new("drop_users", "Drop the users table")
        .add_step(MigrationAction::DropTable {
            name: "users".into(),
        });

    runner.register(migration);
    runner.run_pending().expect("Failed to run migration");

    let tables = db.tables.read();
    assert!(!tables.contains_key("users"));
}

#[test]
fn test_run_named_migration() {
    let db = setup_db();
    let mut runner = MigrationRunner::new(db.clone());

    let migration = Migration::new("add_city", "Add city column")
        .add_step(MigrationAction::AddColumn {
            table: "users".into(),
            column: ColumnDef {
                name: "city".into(),
                data_type: "String".into(),
                nullable: true,
            },
            default_value: None,
        });

    runner.register(migration);

    // Run specific migration
    let status = runner
        .run_named("add_city")
        .expect("Failed to run named migration");
    assert!(status.success);
    assert_eq!(status.name, "add_city");

    // Running again should fail
    let result = runner.run_named("add_city");
    assert!(result.is_err(), "Should not allow running twice");
}

#[test]
fn test_multi_step_migration() {
    let db = setup_db();
    let mut runner = MigrationRunner::new(db.clone());

    let migration = Migration::new("multi_step", "Multi-step migration")
        .add_step(MigrationAction::AddColumn {
            table: "users".into(),
            column: ColumnDef {
                name: "email".into(),
                data_type: "String".into(),
                nullable: true,
            },
            default_value: None,
        })
        .add_step(MigrationAction::RenameColumn {
            table: "users".into(),
            old_name: "age".into(),
            new_name: "years".into(),
        });

    runner.register(migration);
    let statuses = runner.run_pending().expect("Failed to run migrations");
    assert_eq!(statuses.len(), 1);
    assert!(statuses[0].success);
    assert_eq!(statuses[0].steps.len(), 2);

    // Verify both actions applied
    let tables = db.tables.read();
    let users = tables.get("users").unwrap();
    let guard = users.read();
    assert!(guard.column_map.contains_key("email"));
    assert!(guard.column_map.contains_key("years"));
    assert!(!guard.column_map.contains_key("age"));
}

#[test]
fn test_dry_run() {
    let db = setup_db();
    let mut runner = MigrationRunner::new(db.clone());

    let migration = Migration::new("dry_check", "Dry run test")
        .add_step(MigrationAction::AddColumn {
            table: "users".into(),
            column: ColumnDef {
                name: "phone".into(),
                data_type: "String".into(),
                nullable: true,
            },
            default_value: None,
        });

    runner.register(migration);

    // Dry run should succeed
    let steps = runner.dry_run().expect("Dry run should succeed");
    assert_eq!(steps.len(), 1);
    assert!(steps[0].success);
    assert!(steps[0].description.contains("phone"));

    // Table should NOT have been modified by dry run
    let tables = db.tables.read();
    let users = tables.get("users").unwrap();
    let guard = users.read();
    assert!(!guard.column_map.contains_key("phone"));
}

#[test]
fn test_dry_run_with_data() {
    let db = setup_db();

    // Insert some data first
    let mut row = RowData::default();
    row.insert("name".into(), Value::String("Alice".into()));
    row.insert("age".into(), Value::Integer(30));
    db.insert_row("users", row, None).ok();

    let mut runner = MigrationRunner::new(db.clone());

    // Test adding a column with default value
    let migration = Migration::new("add_default", "Add active column with default")
        .add_step(MigrationAction::AddColumn {
            table: "users".into(),
            column: ColumnDef {
                name: "active".into(),
                data_type: "Boolean".into(),
                nullable: false,
            },
            default_value: Some(Value::Boolean(true)),
        });

    runner.register(migration);

    // Dry run first
    let steps = runner.dry_run().expect("Dry run should succeed");
    assert_eq!(steps.len(), 1);

    // Then actually run
    let statuses = runner.run_pending().expect("Should run successfully");
    assert!(statuses[0].success);

    // Verify the column was added with the right value
    let tables = db.tables.read();
    let users = tables.get("users").unwrap();
    let guard = users.read();
    assert!(guard.column_map.contains_key("active"));
}

#[test]
fn test_applied_list() {
    let db = setup_db();
    let mut runner = MigrationRunner::new(db.clone());

    assert_eq!(runner.applied_list().len(), 0);

    let migration = Migration::new("m1", "First migration")
        .add_step(MigrationAction::AddColumn {
            table: "users".into(),
            column: ColumnDef {
                name: "col1".into(),
                data_type: "String".into(),
                nullable: true,
            },
            default_value: None,
        });

    runner.register(migration);
    runner.run_pending().expect("Should run");

    let applied = runner.applied_list();
    assert_eq!(applied.len(), 1);
    assert_eq!(applied[0], "m1");
}

#[test]
fn test_pending_list() {
    let db = setup_db();
    let mut runner = MigrationRunner::new(db.clone());

    let m1 = Migration::new("m1", "First")
        .add_step(MigrationAction::AddColumn {
            table: "users".into(),
            column: ColumnDef {
                name: "c1".into(),
                data_type: "String".into(),
                nullable: true,
            },
            default_value: None,
        });
    let m2 = Migration::new("m2", "Second")
        .add_step(MigrationAction::AddColumn {
            table: "users".into(),
            column: ColumnDef {
                name: "c2".into(),
                data_type: "String".into(),
                nullable: true,
            },
            default_value: None,
        });

    runner.register(m1);
    runner.register(m2);

    // Both pending initially
    assert_eq!(runner.pending_list().len(), 2);

    // Run first
    runner.run_named("m1").expect("Should run m1");
    assert_eq!(runner.pending_list().len(), 1);
    assert_eq!(runner.pending_list()[0], "m2");

    // Run second
    runner.run_named("m2").expect("Should run m2");
    assert_eq!(runner.pending_list().len(), 0);
}

#[test]
fn test_drop_column() {
    let db = setup_db();
    let mut runner = MigrationRunner::new(db.clone());

    let migration = Migration::new("drop_age", "Drop age column")
        .add_step(MigrationAction::DropColumn {
            table: "users".into(),
            column: "age".into(),
        });

    runner.register(migration);
    runner.run_pending().expect("Should run");

    let tables = db.tables.read();
    let users = tables.get("users").unwrap();
    let guard = users.read();
    assert!(!guard.column_map.contains_key("age"));
    assert_eq!(guard.num_columns, 1);
    assert!(guard.column_map.contains_key("name"));
}