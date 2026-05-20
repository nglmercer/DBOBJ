use crate::protocol::{ColumnDef, MigrationStep, MigrationStatus};
use dbobj::{Database, Value};
use std::fmt;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

/// Errors that can occur during migration operations
#[derive(Error, Debug)]
pub enum MigrationError {
    #[error("Table not found: {0}")]
    TableNotFound(String),

    #[error("Column not found: {0}")]
    ColumnNotFound(String),

    #[error("Column already exists: {0}")]
    ColumnExists(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Migration '{0}' not found")]
    NotFound(String),

    #[error("Dry-run validation failed: {0}")]
    DryRun(String),
}

/// Describes one schema transformation step.
/// This is serialisable so it can be sent over the wire.
#[derive(Debug, Clone)]
pub enum MigrationAction {
    AddColumn {
        table: String,
        column: ColumnDef,
        default_value: Option<Value>,
    },
    DropColumn {
        table: String,
        column: String,
    },
    RenameColumn {
        table: String,
        old_name: String,
        new_name: String,
    },
    RenameTable {
        old_name: String,
        new_name: String,
    },
    DropTable {
        name: String,
    },
    Custom {
        description: String,
        apply_fn: MigrationFn,
    },
}

/// Type alias for a custom migration function.
type MigrationFnInner = Arc<dyn Fn(&Database) -> Result<(), MigrationError> + Send + Sync>;

/// Wrapper for a custom migration function with manual Debug impl.
#[derive(Clone)]
pub struct MigrationFn(MigrationFnInner);

impl MigrationFn {
    pub fn new(f: MigrationFnInner) -> Self {
        Self(f)
    }

    pub fn call(&self, db: &Database) -> Result<(), MigrationError> {
        (self.0)(db)
    }
}

impl fmt::Debug for MigrationFn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<custom_fn>")
    }
}

/// A named, ordered collection of migration steps.
#[derive(Debug, Clone)]
pub struct Migration {
    pub id: String,
    pub name: String,
    pub description: String,
    pub actions: Vec<MigrationAction>,
}

impl Migration {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            description: description.into(),
            actions: Vec::new(),
        }
    }

    pub fn add_step(mut self, action: MigrationAction) -> Self {
        self.actions.push(action);
        self
    }
}

/// Runs migrations against a database, tracking which have been applied.
pub struct MigrationRunner {
    /// Name → Migration mapping (registered migrations)
    migrations: Vec<Migration>,
    /// Names of already-applied migrations (loaded from a tracking table)
    applied: std::sync::RwLock<Vec<String>>,
    /// Source database reference (for reading state)
    db: Arc<Database>,
}

impl MigrationRunner {
    /// The name of the internal tracking table.
    pub const TRACKING_TABLE: &'static str = "_dbobj_migrations";

    /// Create a new runner. If the tracking table doesn't exist, it is created.
    pub fn new(db: Arc<Database>) -> Self {
        // Ensure tracking table exists
        {
            let tables = db.tables.read();
            if !tables.contains_key(Self::TRACKING_TABLE) {
                drop(tables);
                let schema = dbobj::Schema {
                    columns: vec![
                        dbobj::ColumnDefinition {
                            name: "id".into(),
                            data_type: dbobj::DataType::String,
                            nullable: false,
                        },
                        dbobj::ColumnDefinition {
                            name: "name".into(),
                            data_type: dbobj::DataType::String,
                            nullable: false,
                        },
                        dbobj::ColumnDefinition {
                            name: "applied_at".into(),
                            data_type: dbobj::DataType::Integer,
                            nullable: false,
                        },
                        dbobj::ColumnDefinition {
                            name: "checksum".into(),
                            data_type: dbobj::DataType::String,
                            nullable: true,
                        },
                    ],
                };
                db.create_table(Self::TRACKING_TABLE.into(), schema);
            }
        }

        // Load applied migration names
        let applied = Self::load_applied(&db);
        Self {
            migrations: Vec::new(),
            applied: std::sync::RwLock::new(applied),
            db,
        }
    }

    /// Register a migration. Migrations are applied in registration order.
    pub fn register(&mut self, migration: Migration) {
        self.migrations.push(migration);
    }

    /// Returns the list of registered migration names.
    pub fn registered(&self) -> Vec<&str> {
        self.migrations.iter().map(|m| m.name.as_str()).collect()
    }

    /// Returns the list of applied migration names.
    pub fn applied_list(&self) -> Vec<String> {
        self.applied.read().unwrap().clone()
    }

    /// Returns the list of pending (registered but not yet applied) migration names.
    pub fn pending_list(&self) -> Vec<&str> {
        let applied = self.applied.read().unwrap();
        self.migrations
            .iter()
            .filter(|m| !applied.contains(&m.name))
            .map(|m| m.name.as_str())
            .collect()
    }

    /// Run all pending migrations. Returns status for each step.
    pub fn run_pending(&self) -> Result<Vec<MigrationStatus>, MigrationError> {
        let new_applied = {
            let applied = self.applied.read().unwrap();
            let mut statuses = Vec::new();
            let mut new_applied = applied.clone();

            for migration in &self.migrations {
                if new_applied.contains(&migration.name) {
                    continue;
                }

                let status = self.apply_migration(migration)?;
                new_applied.push(migration.name.clone());
                statuses.push(status);
            }

            // Drop read lock before acquiring write
            drop(applied);
            (statuses, new_applied)
        };

        *self.applied.write().unwrap() = new_applied.1;
        Ok(new_applied.0)
    }

    /// Run a specific migration by name (if not already applied).
    pub fn run_named(&self, name: &str) -> Result<MigrationStatus, MigrationError> {
        let migration = self
            .migrations
            .iter()
            .find(|m| m.name == name)
            .ok_or_else(|| MigrationError::NotFound(name.to_string()))?;

        {
            let applied = self.applied.read().unwrap();
            if applied.contains(&migration.name) {
                return Err(MigrationError::Database(format!(
                    "Migration '{}' has already been applied",
                    name
                )));
            }
        }

        let status = self.apply_migration(migration)?;
        self.applied.write().unwrap().push(migration.name.clone());
        Ok(status)
    }

    /// Dry-run: validate all pending migrations without making changes.
    pub fn dry_run(&self) -> Result<Vec<MigrationStep>, MigrationError> {
        let applied = self.applied.read().unwrap();
        let mut steps = Vec::new();

        for migration in &self.migrations {
            if applied.contains(&migration.name) {
                continue;
            }

            for action in &migration.actions {
                let step = self.validate_action(action)?;
                steps.push(step);
            }
        }

        Ok(steps)
    }

    // ── private ─────────────────────────────────────────────────────

    fn apply_migration(&self, migration: &Migration) -> Result<MigrationStatus, MigrationError> {
        let mut steps = Vec::new();
        let mut ok = true;

        for (i, action) in migration.actions.iter().enumerate() {
            match self.execute_action(action) {
                Ok(desc) => {
                    steps.push(MigrationStep {
                        index: i,
                        description: desc,
                        success: true,
                        error: None,
                    });
                }
                Err(e) => {
                    steps.push(MigrationStep {
                        index: i,
                        description: format!("{:?}", action),
                        success: false,
                        error: Some(e.to_string()),
                    });
                    ok = false;
                    break;
                }
            }
        }

        // Record in tracking table
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        let checksum = format!("{}_steps", migration.actions.len());

        let _ = self.db.insert_values(
            Self::TRACKING_TABLE,
            vec![
                Value::String(migration.id.clone().into()),
                Value::String(migration.name.clone().into()),
                Value::Integer(now),
                Value::String(checksum.into()),
            ],
        );

        Ok(MigrationStatus {
            id: migration.id.clone(),
            name: migration.name.clone(),
            description: migration.description.clone(),
            success: ok,
            applied_at_ms: now,
            steps,
        })
    }

    fn execute_action(&self, action: &MigrationAction) -> Result<String, MigrationError> {
        match action {
            MigrationAction::AddColumn {
                table,
                column,
                default_value,
            } => self.exec_add_column(table, column, default_value.as_ref()),
            MigrationAction::DropColumn { table, column } => {
                self.exec_drop_column(table, column)
            }
            MigrationAction::RenameColumn {
                table,
                old_name,
                new_name,
            } => self.exec_rename_column(table, old_name, new_name),
            MigrationAction::RenameTable { old_name, new_name } => {
                self.exec_rename_table(old_name, new_name)
            }
            MigrationAction::DropTable { name } => self.exec_drop_table(name),
            MigrationAction::Custom { description, apply_fn } => {
                apply_fn.call(&self.db)?;
                Ok(description.clone())
            }
        }
    }

    fn validate_action(&self, action: &MigrationAction) -> Result<MigrationStep, MigrationError> {
        // Validate without making changes
        match action {
            MigrationAction::AddColumn { table, column, .. } => {
                let tables = self.db.tables.read();
                let tbl = tables.get(table).ok_or_else(|| {
                    MigrationError::TableNotFound(table.clone())
                })?;
                let guard = tbl.read();
                if guard.column_map.contains_key(&column.name) {
                    return Err(MigrationError::ColumnExists(format!(
                        "Column '{}' already exists in '{}'",
                        column.name, table
                    )));
                }
                Ok(MigrationStep {
                    index: 0,
                    description: format!("Add column '{}' to '{}'", column.name, table),
                    success: true,
                    error: None,
                })
            }
            MigrationAction::DropColumn { table, column } => {
                let tables = self.db.tables.read();
                let tbl = tables.get(table).ok_or_else(|| {
                    MigrationError::TableNotFound(table.clone())
                })?;
                let guard = tbl.read();
                if !guard.column_map.contains_key(column) {
                    return Err(MigrationError::ColumnNotFound(format!(
                        "Column '{}' not found in '{}'",
                        column, table
                    )));
                }
                Ok(MigrationStep {
                    index: 0,
                    description: format!("Drop column '{}' from '{}'", column, table),
                    success: true,
                    error: None,
                })
            }
            MigrationAction::RenameColumn {
                table,
                old_name,
                new_name,
            } => {
                let tables = self.db.tables.read();
                let tbl = tables.get(table).ok_or_else(|| {
                    MigrationError::TableNotFound(table.clone())
                })?;
                let guard = tbl.read();
                if !guard.column_map.contains_key(old_name) {
                    return Err(MigrationError::ColumnNotFound(format!(
                        "Column '{}' not found in '{}'",
                        old_name, table
                    )));
                }
                if guard.column_map.contains_key(new_name) {
                    return Err(MigrationError::ColumnExists(format!(
                        "Column '{}' already exists in '{}'",
                        new_name, table
                    )));
                }
                Ok(MigrationStep {
                    index: 0,
                    description: format!(
                        "Rename column '{}' to '{}' in '{}'",
                        old_name, new_name, table
                    ),
                    success: true,
                    error: None,
                })
            }
            MigrationAction::RenameTable { old_name, new_name } => {
                let tables = self.db.tables.read();
                if !tables.contains_key(old_name) {
                    return Err(MigrationError::TableNotFound(old_name.clone()));
                }
                if tables.contains_key(new_name) {
                    return Err(MigrationError::Database(format!(
                        "Table '{}' already exists",
                        new_name
                    )));
                }
                Ok(MigrationStep {
                    index: 0,
                    description: format!("Rename table '{}' to '{}'", old_name, new_name),
                    success: true,
                    error: None,
                })
            }
            MigrationAction::DropTable { name } => {
                let tables = self.db.tables.read();
                if !tables.contains_key(name) {
                    return Err(MigrationError::TableNotFound(name.clone()));
                }
                if name == Self::TRACKING_TABLE {
                    return Err(MigrationError::DryRun(
                        "Cannot drop the migration tracking table".into(),
                    ));
                }
                Ok(MigrationStep {
                    index: 0,
                    description: format!("Drop table '{}'", name),
                    success: true,
                    error: None,
                })
            }
            MigrationAction::Custom { description, .. } => {
                // Can't validate custom functions without running them
                Ok(MigrationStep {
                    index: 0,
                    description: format!("[custom] {}", description),
                    success: true,
                    error: None,
                })
            }
        }
    }

    fn exec_add_column(
        &self,
        table: &str,
        column: &ColumnDef,
        default: Option<&Value>,
    ) -> Result<String, MigrationError> {
        let tables = self.db.tables.read();
        let tbl = tables.get(table).ok_or_else(|| {
            MigrationError::TableNotFound(table.to_string())
        })?;
        let mut guard = tbl.write();

        // Check column doesn't already exist
        if guard.column_map.contains_key(&column.name) {
            return Err(MigrationError::ColumnExists(format!(
                "Column '{}' already exists in '{}'",
                column.name, table
            )));
        }

        let col_idx = guard.num_columns;
        guard
            .column_map
            .insert(column.name.clone(), col_idx);
        guard.schema.columns.push(dbobj::ColumnDefinition {
            name: column.name.clone().into(),
            data_type: crate::protocol::data_type_to_dbobj(&column.data_type),
            nullable: column.nullable,
        });
        guard.num_columns += 1;

        // Extend existing rows with default value (or Null)
        let fill = default
            .cloned()
            .unwrap_or(Value::Null);
        let num_rows = guard.ids.len();
        let current_len = guard.data.len();
        guard.data.resize(current_len + num_rows, fill);

        Ok(format!("Added column '{}' to '{}'", column.name, table))
    }

    fn exec_drop_column(&self, table: &str, column: &str) -> Result<String, MigrationError> {
        let tables = self.db.tables.read();
        let tbl = tables.get(table).ok_or_else(|| {
            MigrationError::TableNotFound(table.to_string())
        })?;
        let mut guard = tbl.write();

        let col_idx = *guard.column_map.get(column).ok_or_else(|| {
            MigrationError::ColumnNotFound(format!("Column '{}' not found in '{}'", column, table))
        })?;

        // Remove from column_map
        guard.column_map.remove(column);

        // Remove from schema
        guard
            .schema
            .columns
            .retain(|c| c.name.as_str() != column);

        // Remove data for this column for every row
        let num_cols = guard.num_columns;
        let num_rows = guard.ids.len();
        let mut new_data = Vec::with_capacity(guard.data.len() - num_rows);
        for row_idx in 0..num_rows {
            let base = row_idx * num_cols;
            for col in 0..num_cols {
                if col != col_idx {
                    new_data.push(guard.data[base + col].clone());
                }
            }
        }
        guard.data = new_data;
        guard.num_columns -= 1;

        // Adjust column_map indices for columns after the dropped one
        for (_, idx) in guard.column_map.iter_mut() {
            if *idx > col_idx {
                *idx -= 1;
            }
        }

        Ok(format!("Dropped column '{}' from '{}'", column, table))
    }

    fn exec_rename_column(
        &self,
        table: &str,
        old_name: &str,
        new_name: &str,
    ) -> Result<String, MigrationError> {
        let tables = self.db.tables.read();
        let tbl = tables.get(table).ok_or_else(|| {
            MigrationError::TableNotFound(table.to_string())
        })?;
        let mut guard = tbl.write();

        let idx = *guard.column_map.get(old_name).ok_or_else(|| {
            MigrationError::ColumnNotFound(format!(
                "Column '{}' not found in '{}'",
                old_name, table
            ))
        })?;

        guard.column_map.remove(old_name);
        guard.column_map.insert(new_name.into(), idx);

        if let Some(col) = guard
            .schema
            .columns
            .iter_mut()
            .find(|c| c.name.as_str() == old_name)
        {
            col.name = new_name.into();
        }

        Ok(format!(
            "Renamed column '{}' to '{}' in '{}'",
            old_name, new_name, table
        ))
    }

    fn exec_rename_table(
        &self,
        old_name: &str,
        new_name: &str,
    ) -> Result<String, MigrationError> {
        let mut tables = self.db.tables.write();
        let tbl = tables.remove(old_name).ok_or_else(|| {
            MigrationError::TableNotFound(old_name.to_string())
        })?;
        {
            let mut guard = tbl.write();
            guard.name = new_name.into();
        }
        tables.insert(new_name.into(), tbl);
        Ok(format!("Renamed table '{}' to '{}'", old_name, new_name))
    }

    fn exec_drop_table(&self, name: &str) -> Result<String, MigrationError> {
        if name == Self::TRACKING_TABLE {
            return Err(MigrationError::Database(
                "Cannot drop the migration tracking table".into(),
            ));
        }
        self.db
            .drop_table(name)
            .map_err(|e| MigrationError::Database(e.to_string()))?;
        Ok(format!("Dropped table '{}'", name))
    }

    fn load_applied(db: &Database) -> Vec<String> {
        let tables = db.tables.read();
        if !tables.contains_key(Self::TRACKING_TABLE) {
            return Vec::new();
        }
        drop(tables);

        // Read from tracking table — query all rows
        let tbl = db.get_table(Self::TRACKING_TABLE);
        if let Some(tbl_lock) = tbl {
            let guard = tbl_lock.read();
            let mut names = Vec::new();
            for id in &guard.ids {
                if let Some(idx) = guard.get_index(id) {
                    let start = idx * guard.num_columns;
                    if start + 1 < guard.data.len() {
                        if let Value::String(name) = &guard.data[start + 1] {
                            names.push(name.to_string());
                        }
                    }
                }
            }
            names
        } else {
            Vec::new()
        }
    }
}

/// Utility builders for common migration patterns
pub mod builders {
    use super::*;

    /// Create a migration that adds a column with a default value.
    pub fn add_column(
        name: &str,
        desc: &str,
        table: &str,
        column: ColumnDef,
        default: Option<Value>,
    ) -> Migration {
        Migration::new(name, desc).add_step(MigrationAction::AddColumn {
            table: table.into(),
            column,
            default_value: default,
        })
    }

    /// Create a migration that renames a column.
    pub fn rename_column(
        name: &str,
        desc: &str,
        table: &str,
        old_name: &str,
        new_name: &str,
    ) -> Migration {
        Migration::new(name, desc).add_step(MigrationAction::RenameColumn {
            table: table.into(),
            old_name: old_name.into(),
            new_name: new_name.into(),
        })
    }

    /// Create a migration that renames a table.
    pub fn rename_table(name: &str, desc: &str, old_name: &str, new_name: &str) -> Migration {
        Migration::new(name, desc).add_step(MigrationAction::RenameTable {
            old_name: old_name.into(),
            new_name: new_name.into(),
        })
    }

    /// Create a migration with multiple steps.
    pub fn multi_step(name: &str, desc: &str, actions: Vec<MigrationAction>) -> Migration {
        let mut m = Migration::new(name, desc);
        for a in actions {
            m = m.add_step(a);
        }
        m
    }
}