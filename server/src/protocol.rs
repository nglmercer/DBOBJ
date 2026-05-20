use dbobj::{DataType, Id, RowData, Value};
use serde::{Deserialize, Serialize};

/// All operations the server supports
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
    // Database lifecycle
    CreateTable {
        name: String,
        columns: Vec<ColumnDef>,
    },
    DropTable {
        name: String,
    },
    ListTables,
    TableInfo {
        name: String,
    },

    // Insert operations
    Insert {
        table: String,
        data: RowData,
        custom_id: Option<Id>,
    },
    InsertValues {
        table: String,
        values: Vec<Value>,
    },
    InsertBatch {
        table: String,
        batch: Vec<RowData>,
    },
    InsertBatchValues {
        table: String,
        batch: Vec<Vec<Value>>,
    },
    InsertOrReplace {
        table: String,
        values: Vec<Value>,
        unique_column: String,
    },

    // Update operations
    UpdateRow {
        table: String,
        id: Id,
        data: RowData,
    },
    UpdateValues {
        table: String,
        id: Id,
        values: Vec<Value>,
    },
    UpdateByIndices {
        table: String,
        id: Id,
        updates: Vec<(usize, Value)>,
    },

    // Delete operations
    DeleteRow {
        table: String,
        id: Id,
    },
    DeleteBatch {
        table: String,
        ids: Vec<Id>,
    },

    // Query operations
    Query {
        table: String,
        column_name: String,
        value: Value,
    },
    QueryPredicate {
        /// Column index to compare
        table: String,
        column_idx: usize,
        operator: ComparisonOp,
        value: Value,
    },
    QueryExpr {
        table: String,
        expr: ExprData,
    },

    // Index operations
    CreateIndex {
        table: String,
        column: String,
    },
    CreateUniqueIndex {
        table: String,
        column: String,
    },

    // Join operations
    HashJoin {
        table1: String,
        col1: String,
        table2: String,
        col2: String,
    },

    // Transaction
    BeginTransaction,
    CommitTransaction,
    RollbackTransaction,

    // Persistence
    Save,
    Load {
        path: String,
    },

    // ── Backup operations ───────────────────────────────────────────
    /// Create a backup of the current database state
    CreateBackup {
        label: String,
        format: BackupFormat,
    },
    /// List all available backups
    ListBackups,
    /// Restore a backup by ID
    RestoreBackup {
        backup_id: String,
        mode: RestoreMode,
    },
    /// Delete a backup by ID
    DeleteBackup {
        backup_id: String,
    },

    // ── Migration operations ────────────────────────────────────────
    /// Register a migration with the runner
    RegisterMigration {
        name: String,
        description: String,
        actions: Vec<SchemaChange>,
    },
    /// Run all pending migrations
    RunPendingMigrations,
    /// Run a specific migration by name
    RunMigration {
        name: String,
    },
    /// List registered migrations and their status
    ListMigrations,
    /// Dry-run: validate pending migrations without applying
    DryRunMigrations,

    // Metadata
    Ping,
}

/// Backup serialization format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupFormat {
    /// Compact bincode format (default)
    Native,
    /// Human-readable JSON format
    Json,
}

impl BackupFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            BackupFormat::Native => "dbobj",
            BackupFormat::Json => "json",
        }
    }
}

/// How to apply a restored backup
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RestoreMode {
    /// Replace the current database entirely with the backup
    Replace,
    /// Merge backup tables into the current database (backup tables take precedence)
    Merge,
}

/// Metadata returned for a backup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    pub id: String,
    pub label: String,
    pub format: BackupFormat,
    pub timestamp_ms: i64,
    pub table_count: usize,
    pub total_rows: usize,
    pub file_size: u64,
    pub path: String,
}

/// A schema change step in a migration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SchemaChange {
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
}

/// Status of a single migration step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationStep {
    pub index: usize,
    pub description: String,
    pub success: bool,
    pub error: Option<String>,
}

/// Overall status of a migration run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationStatus {
    pub id: String,
    pub name: String,
    pub description: String,
    pub success: bool,
    pub applied_at_ms: i64,
    pub steps: Vec<MigrationStep>,
}

/// Summary of a registered migration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub applied: bool,
    pub applied_at_ms: Option<i64>,
    pub step_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComparisonOp {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
}

/// Serializable expression tree for remote query execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExprData {
    Column(String),
    Literal(Value),
    Binary {
        left: Box<ExprData>,
        op: ComparisonOp,
        right: Box<ExprData>,
    },
    And(Vec<ExprData>),
    Or(Vec<ExprData>),
    Not(Box<ExprData>),
}

/// Minimal column definition for CREATE TABLE over the wire
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: String, // "Integer", "Float", "String", "Boolean", "Blob"
    pub nullable: bool,
}

/// Convert a data type string to dbobj::DataType
pub fn data_type_to_dbobj(dt: &str) -> DataType {
    match dt {
        "Integer" | "Int" | "Int64" | "UInt64" => DataType::Integer,
        "Float" | "F64" | "Double" => DataType::Float,
        "String" | "Text" | "Str" | "Utf8" => DataType::String,
        "Boolean" | "Bool" => DataType::Boolean,
        "Blob" | "Bytes" | "Binary" => DataType::Blob,
        _ => DataType::String,
    }
}

/// All possible server responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
    Rows(Vec<SerializedRow>),
    Ok(u64),
    TableList(Vec<String>),
    TableInfo {
        name: String,
        columns: Vec<ColumnDef>,
        row_count: usize,
    },
    Id(Id),
    Ids(Vec<Id>),
    JoinedRows(Vec<(SerializedRow, SerializedRow)>),
    // Backup responses
    BackupCreated(BackupInfo),
    BackupList(Vec<BackupInfo>),
    BackupDeleted,
    // Migration responses
    MigrationStatuses(Vec<MigrationStatus>),
    MigrationStatus(MigrationStatus),
    MigrationList(Vec<MigrationSummary>),
    MigrationSteps(Vec<MigrationStep>),
    Pong,
    Error(String),
}

/// A row that can be serialized over the wire
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedRow {
    pub id: Id,
    pub data: Vec<Value>,
}
