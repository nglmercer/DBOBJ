use dbobj::{Id, RowData, Value};
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

    // Metadata
    Ping,
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
    Pong,
    Error(String),
}

/// A row that can be serialized over the wire
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedRow {
    pub id: Id,
    pub data: Vec<Value>,
}