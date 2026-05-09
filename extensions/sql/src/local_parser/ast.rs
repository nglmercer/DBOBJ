use crate::core::{DataType, Operator, Value};
use compact_str::CompactString;

// ── Error ──

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub position: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Parse error at position {}: {}",
            self.position, self.message
        )
    }
}

impl std::error::Error for ParseError {}

// ── Tokens ──

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    KwCreate,
    KwTable,
    KwAlter,
    KwAdd,
    KwColumn,
    KwInsert,
    KwInto,
    KwValues,
    KwUpdate,
    KwSet,
    KwDelete,
    KwFrom,
    KwSelect,
    KwWhere,
    KwInner,
    KwJoin,
    KwOn,
    KwAnd,
    KwOr,
    KwTrue,
    KwFalse,
    KwNull,
    KwAs,
    KwInteger,
    KwInt,
    KwBigInt,
    KwFloat,
    KwDouble,
    KwReal,
    KwString,
    KwText,
    KwVarchar,
    KwChar,
    KwBoolean,
    KwBlob,
    KwBytea,
    KwVarbinary,
    KwBinary,
    LeftParen,
    RightParen,
    Comma,
    Semicolon,
    Dot,
    Star,
    Equals,
    Question,
    Ident(CompactString),
    Number(CompactString),
    SingleQuotedString(CompactString),
    OpNotEq,
    OpGtEq,
    OpLtEq,
    OpGt,
    OpLt,
    Eof,
}

// ── AST types ──

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    CreateTable {
        name: CompactString,
        columns: Vec<ColumnDef>,
    },
    AlterTable {
        name: CompactString,
        operation: AlterOperation,
    },
    Insert {
        table: CompactString,
        columns: Vec<CompactString>,
        values: Vec<Vec<Expr>>,
    },
    Update {
        table: CompactString,
        assignments: Vec<Assignment>,
        selection: Option<Expr>,
    },
    Delete {
        table: CompactString,
        selection: Option<Expr>,
    },
    Select {
        columns: SelectColumns,
        table: CompactString,
        selection: Option<Expr>,
        join: Option<Join>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnDef {
    pub name: CompactString,
    pub data_type: DataType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelectColumns {
    Star,
    List(Vec<CompactString>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    pub column: CompactString,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterOperation {
    AddColumn(ColumnDef),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Join {
    pub table: CompactString,
    pub left_table: CompactString,
    pub left_col: CompactString,
    pub right_table: CompactString,
    pub right_col: CompactString,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Value),
    Column(CompactString),
    CompoundColumn(CompactString, CompactString),
    Placeholder,
    Binary(Box<Expr>, Operator, Box<Expr>),
    Nested(Box<Expr>),
}
