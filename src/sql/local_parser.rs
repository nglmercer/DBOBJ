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
        write!(f, "Parse error at position {}: {}", self.position, self.message)
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

// ── Tokenizer ──

pub struct Tokenizer<'a> {
    input: &'a str,
    pos: usize,
    ch: Option<char>,
}

impl<'a> Tokenizer<'a> {
    pub fn new(input: &'a str) -> Self {
        let first = input.chars().next();
        Self {
            input,
            pos: 0,
            ch: first,
        }
    }

    fn advance(&mut self) {
        if let Some(ch) = self.ch {
            self.pos += ch.len_utf8();
            self.ch = self.input[self.pos..].chars().next();
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.ch {
            if ch.is_ascii_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    pub fn next_token(&mut self) -> Result<Token, ParseError> {
        self.skip_whitespace();
        let ch = match self.ch {
            Some(c) => c,
            None => return Ok(Token::Eof),
        };

        let start = self.pos;

        match ch {
            '(' => {
                self.advance();
                Ok(Token::LeftParen)
            }
            ')' => {
                self.advance();
                Ok(Token::RightParen)
            }
            ',' => {
                self.advance();
                Ok(Token::Comma)
            }
            ';' => {
                self.advance();
                Ok(Token::Semicolon)
            }
            '.' => {
                self.advance();
                Ok(Token::Dot)
            }
            '*' => {
                self.advance();
                Ok(Token::Star)
            }
            '=' => {
                self.advance();
                Ok(Token::Equals)
            }
            '?' => {
                self.advance();
                Ok(Token::Question)
            }
            '!' => {
                self.advance();
                if self.ch == Some('=') {
                    self.advance();
                    Ok(Token::OpNotEq)
                } else {
                    Err(ParseError {
                        message: "Expected '=' after '!'".to_string(),
                        position: start,
                    })
                }
            }
            '<' => {
                self.advance();
                match self.ch {
                    Some('=') => {
                        self.advance();
                        Ok(Token::OpLtEq)
                    }
                    Some('>') => {
                        self.advance();
                        Ok(Token::OpNotEq)
                    }
                    _ => Ok(Token::OpLt),
                }
            }
            '>' => {
                self.advance();
                if self.ch == Some('=') {
                    self.advance();
                    Ok(Token::OpGtEq)
                } else {
                    Ok(Token::OpGt)
                }
            }
            '\'' => self.scan_single_quoted_string(),
            '"' => self.scan_double_quoted_string(),
            c if c.is_ascii_alphabetic() || c == '_' => self.scan_ident_or_keyword(),
            c if c.is_ascii_digit() => self.scan_number(),
            _ => Err(ParseError {
                message: format!("Unexpected character: {}", ch),
                position: start,
            }),
        }
    }

    fn scan_single_quoted_string(&mut self) -> Result<Token, ParseError> {
        self.advance(); // skip opening '
        let start = self.pos;
        let mut content = String::new();
        loop {
            match self.ch {
                None => {
                    return Err(ParseError {
                        message: "Unterminated string literal".to_string(),
                        position: start.saturating_sub(1),
                    });
                }
                Some('\'') => {
                    let quote_pos = self.pos;
                    self.advance(); // skip this quote
                    if self.ch == Some('\'') {
                        content.push('\'');
                        self.advance();
                    } else {
                        // Unescaped quote: end of string, but also include
                        // any content between start and the quote
                        if content.is_empty() {
                            content = self.input[start..quote_pos].to_string();
                        }
                        return Ok(Token::SingleQuotedString(CompactString::from(content)));
                    }
                }
                Some(ch) => {
                    content.push(ch);
                    self.advance();
                }
            }
        }
    }

    fn scan_double_quoted_string(&mut self) -> Result<Token, ParseError> {
        self.advance(); // skip opening "
        let start = self.pos;
        let mut end = start;
        while let Some(ch) = self.ch {
            if ch == '"' {
                let content = &self.input[start..end];
                self.advance(); // skip closing "
                let content = content.replace("\"\"", "\"");
                return Ok(Token::SingleQuotedString(CompactString::from(content)));
            }
            end = self.pos + ch.len_utf8();
            self.advance();
        }
        Err(ParseError {
            message: "Unterminated double-quoted string".to_string(),
            position: start.saturating_sub(1),
        })
    }

    fn scan_ident_or_keyword(&mut self) -> Result<Token, ParseError> {
        let start = self.pos;
        let mut end = start;
        while let Some(ch) = self.ch {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                end = self.pos + ch.len_utf8();
                self.advance();
            } else {
                break;
            }
        }
        let ident = &self.input[start..end];
        match ident.to_ascii_uppercase().as_str() {
            "CREATE" => Ok(Token::KwCreate),
            "TABLE" => Ok(Token::KwTable),
            "ALTER" => Ok(Token::KwAlter),
            "ADD" => Ok(Token::KwAdd),
            "COLUMN" => Ok(Token::KwColumn),
            "INSERT" => Ok(Token::KwInsert),
            "INTO" => Ok(Token::KwInto),
            "VALUES" => Ok(Token::KwValues),
            "UPDATE" => Ok(Token::KwUpdate),
            "SET" => Ok(Token::KwSet),
            "DELETE" => Ok(Token::KwDelete),
            "FROM" => Ok(Token::KwFrom),
            "SELECT" => Ok(Token::KwSelect),
            "WHERE" => Ok(Token::KwWhere),
            "INNER" => Ok(Token::KwInner),
            "JOIN" => Ok(Token::KwJoin),
            "ON" => Ok(Token::KwOn),
            "AND" => Ok(Token::KwAnd),
            "OR" => Ok(Token::KwOr),
            "TRUE" => Ok(Token::KwTrue),
            "FALSE" => Ok(Token::KwFalse),
            "NULL" => Ok(Token::KwNull),
            "AS" => Ok(Token::KwAs),
            "INTEGER" => Ok(Token::KwInteger),
            "INT" => Ok(Token::KwInt),
            "BIGINT" => Ok(Token::KwBigInt),
            "FLOAT" => Ok(Token::KwFloat),
            "DOUBLE" => Ok(Token::KwDouble),
            "REAL" => Ok(Token::KwReal),
            "STRING" => Ok(Token::KwString),
            "TEXT" => Ok(Token::KwText),
            "VARCHAR" => Ok(Token::KwVarchar),
            "CHAR" => Ok(Token::KwChar),
            "BOOLEAN" => Ok(Token::KwBoolean),
            "BLOB" => Ok(Token::KwBlob),
            "BYTEA" => Ok(Token::KwBytea),
            "VARBINARY" => Ok(Token::KwVarbinary),
            "BINARY" => Ok(Token::KwBinary),
            _ => Ok(Token::Ident(CompactString::from(ident))),
        }
    }

    fn scan_number(&mut self) -> Result<Token, ParseError> {
        let start = self.pos;
        let mut end = start;
        while let Some(ch) = self.ch {
            if ch.is_ascii_digit() || ch == '.' {
                end = self.pos + ch.len_utf8();
                self.advance();
            } else {
                break;
            }
        }
        let num = &self.input[start..end];
        Ok(Token::Number(CompactString::from(num)))
    }
}

// ── Parser ──

pub struct Parser<'a> {
    tokenizer: Tokenizer<'a>,
    current: Token,
    peek: Token,
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut tokenizer = Tokenizer::new(input);
        let current = tokenizer.next_token().unwrap_or(Token::Eof);
        let peek = tokenizer.next_token().unwrap_or(Token::Eof);
        Self {
            tokenizer,
            current,
            peek,
            pos: 0,
        }
    }

    pub fn parse_statements(&mut self) -> Result<Vec<Statement>, ParseError> {
        let mut stmts = Vec::new();
        while self.current != Token::Eof {
            stmts.push(self.parse_statement()?);
            if self.current == Token::Semicolon {
                self.advance()?;
            }
        }
        Ok(stmts)
    }

    fn advance(&mut self) -> Result<(), ParseError> {
        self.pos = self.tokenizer.pos;
        self.current = std::mem::replace(
            &mut self.peek,
            self.tokenizer.next_token().unwrap_or(Token::Eof),
        );
        Ok(())
    }

    fn expect(&mut self, expected: Token) -> Result<(), ParseError> {
        if self.current == expected {
            self.advance()
        } else {
            Err(ParseError {
                message: format!("Expected {:?}, got {:?}", expected, self.current),
                position: self.pos,
            })
        }
    }

    fn current_ident_owned(&self) -> Result<CompactString, ParseError> {
        if let Token::Ident(s) = &self.current {
            Ok(s.clone())
        } else {
            Err(ParseError {
                message: format!("Expected identifier, got {:?}", self.current),
                position: self.tokenizer.pos,
            })
        }
    }

    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        match &self.current {
            Token::KwCreate => self.parse_create_table(),
            Token::KwAlter => self.parse_alter_table(),
            Token::KwInsert => self.parse_insert(),
            Token::KwUpdate => self.parse_update(),
            Token::KwDelete => self.parse_delete(),
            Token::KwSelect => self.parse_select(),
            _ => Err(ParseError {
                message: format!("Unexpected token: {:?}", self.current),
                position: self.tokenizer.pos,
            }),
        }
    }

    fn parse_create_table(&mut self) -> Result<Statement, ParseError> {
        self.expect(Token::KwCreate)?;
        self.expect(Token::KwTable)?;
        let name = self.current_ident_owned()?;
        self.advance()?;
        self.expect(Token::LeftParen)?;
        let mut columns = Vec::new();
        loop {
            let col_name = self.current_ident_owned()?;
            self.advance()?;
            let data_type = self.parse_data_type()?;
            columns.push(ColumnDef {
                name: col_name,
                data_type,
            });
            if self.current == Token::RightParen {
                break;
            }
            self.expect(Token::Comma)?;
        }
        self.expect(Token::RightParen)?;
        Ok(Statement::CreateTable { name, columns })
    }

    fn parse_alter_table(&mut self) -> Result<Statement, ParseError> {
        self.expect(Token::KwAlter)?;
        self.expect(Token::KwTable)?;
        let name = self.current_ident_owned()?;
        self.advance()?;
        self.expect(Token::KwAdd)?;
        if self.current == Token::KwColumn {
            self.advance()?;
        }
        let col_name = self.current_ident_owned()?;
        self.advance()?;
        let data_type = self.parse_data_type()?;
        Ok(Statement::AlterTable {
            name,
            operation: AlterOperation::AddColumn(ColumnDef {
                name: col_name,
                data_type,
            }),
        })
    }

    fn parse_data_type(&mut self) -> Result<DataType, ParseError> {
        let dt = match &self.current {
            Token::KwInteger | Token::KwInt | Token::KwBigInt => DataType::Integer,
            Token::KwFloat | Token::KwDouble | Token::KwReal => DataType::Float,
            Token::KwString | Token::KwText | Token::KwVarchar | Token::KwChar => DataType::String,
            Token::KwBoolean => DataType::Boolean,
            Token::KwBlob | Token::KwBytea | Token::KwVarbinary | Token::KwBinary => {
                DataType::Blob
            }
            _ => {
                return Err(ParseError {
                    message: format!("Expected data type, got {:?}", self.current),
                    position: self.tokenizer.pos,
                })
            }
        };
        self.advance()?;
        if self.current == Token::LeftParen {
            self.advance()?;
            let _size = self.current.clone();
            self.advance()?;
            self.expect(Token::RightParen)?;
        }
        Ok(dt)
    }

    fn parse_insert(&mut self) -> Result<Statement, ParseError> {
        self.expect(Token::KwInsert)?;
        self.expect(Token::KwInto)?;
        let table = self.current_ident_owned()?;
        self.advance()?;
        let columns = if self.current == Token::LeftParen {
            self.advance()?;
            let mut cols = Vec::new();
            loop {
                cols.push(self.current_ident_owned()?);
                self.advance()?;
                if self.current == Token::RightParen {
                    break;
                }
                self.expect(Token::Comma)?;
            }
            self.expect(Token::RightParen)?;
            cols
        } else {
            Vec::new()
        };
        self.expect(Token::KwValues)?;
        let mut all_values: Vec<Vec<Expr>> = Vec::new();
        loop {
            self.expect(Token::LeftParen)?;
            let mut row = Vec::new();
            loop {
                row.push(self.parse_expr()?);
                if self.current == Token::RightParen {
                    break;
                }
                self.expect(Token::Comma)?;
            }
            self.expect(Token::RightParen)?;
            all_values.push(row);
            if self.current != Token::Comma {
                break;
            }
            self.advance()?;
        }
        Ok(Statement::Insert {
            table,
            columns,
            values: all_values,
        })
    }

    fn parse_update(&mut self) -> Result<Statement, ParseError> {
        self.expect(Token::KwUpdate)?;
        let table = self.current_ident_owned()?;
        self.advance()?;
        self.expect(Token::KwSet)?;
        let mut assignments = Vec::new();
        loop {
            let column = self.current_ident_owned()?;
            self.advance()?;
            self.expect(Token::Equals)?;
            let value = self.parse_expr()?;
            assignments.push(Assignment { column, value });
            if self.current != Token::Comma {
                break;
            }
            self.advance()?;
        }
        let selection = if self.current == Token::KwWhere {
            self.advance()?;
            Some(self.parse_expr()?)
        } else {
            None
        };
        Ok(Statement::Update {
            table,
            assignments,
            selection,
        })
    }

    fn parse_delete(&mut self) -> Result<Statement, ParseError> {
        self.expect(Token::KwDelete)?;
        self.expect(Token::KwFrom)?;
        let table = self.current_ident_owned()?;
        self.advance()?;
        let selection = if self.current == Token::KwWhere {
            self.advance()?;
            Some(self.parse_expr()?)
        } else {
            None
        };
        Ok(Statement::Delete { table, selection })
    }

    fn parse_select(&mut self) -> Result<Statement, ParseError> {
        self.expect(Token::KwSelect)?;
        let columns = if self.current == Token::Star {
            self.advance()?;
            SelectColumns::Star
        } else {
            let mut cols = Vec::new();
            loop {
                cols.push(self.current_ident_owned()?);
                self.advance()?;
                if self.current != Token::Comma {
                    break;
                }
                self.advance()?;
            }
            SelectColumns::List(cols)
        };
        self.expect(Token::KwFrom)?;
        let table = self.current_ident_owned()?;
        self.advance()?;
        let join = if matches!(&self.current, Token::KwInner | Token::KwJoin) {
            Some(self.parse_join(&table)?)
        } else {
            None
        };
        let selection = if self.current == Token::KwWhere {
            self.advance()?;
            Some(self.parse_expr()?)
        } else {
            None
        };
        Ok(Statement::Select {
            columns,
            table,
            selection,
            join,
        })
    }

    fn parse_join(&mut self, left_table: &CompactString) -> Result<Join, ParseError> {
        if self.current == Token::KwInner {
            self.advance()?;
        }
        self.expect(Token::KwJoin)?;
        let right_table = self.current_ident_owned()?;
        self.advance()?;
        self.expect(Token::KwOn)?;
        let left_col = self.current_ident_owned()?;
        self.advance()?;
        self.expect(Token::Dot)?;
        let left_name = self.current_ident_owned()?;
        self.advance()?;
        self.expect(Token::Equals)?;
        let right_col = self.current_ident_owned()?;
        self.advance()?;
        self.expect(Token::Dot)?;
        let right_name = self.current_ident_owned()?;
        self.advance()?;
        let (left_table_name, left_column, right_table_name, right_column) =
            if left_col.as_str() == left_table.as_str() {
                (
                    left_col,
                    left_name,
                    right_col,
                    right_name,
                )
            } else {
                (
                    right_col,
                    right_name,
                    left_col,
                    left_name,
                )
            };
        Ok(Join {
            table: right_table,
            left_table: left_table_name,
            left_col: left_column,
            right_table: right_table_name,
            right_col: right_column,
        })
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_and()?;
        while self.current == Token::KwOr {
            self.advance()?;
            let right = self.parse_and()?;
            left = Expr::Binary(Box::new(left), Operator::Or, Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_comparison()?;
        while self.current == Token::KwAnd {
            self.advance()?;
            let right = self.parse_comparison()?;
            left = Expr::Binary(Box::new(left), Operator::And, Box::new(right));
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_atom()?;
        let op = match &self.current {
            Token::Equals => Operator::Eq,
            Token::OpNotEq => Operator::Neq,
            Token::OpGt => Operator::Gt,
            Token::OpGtEq => Operator::Gte,
            Token::OpLt => Operator::Lt,
            Token::OpLtEq => Operator::Lte,
            _ => return Ok(left),
        };
        self.advance()?;
        let right = self.parse_atom()?;
        Ok(Expr::Binary(Box::new(left), op, Box::new(right)))
    }

    fn parse_atom(&mut self) -> Result<Expr, ParseError> {
        match &self.current {
            Token::LeftParen => {
                self.advance()?;
                let expr = self.parse_expr()?;
                self.expect(Token::RightParen)?;
                Ok(Expr::Nested(Box::new(expr)))
            }
            Token::Ident(_) => {
                let first = self.current_ident_owned()?;
                self.advance()?;
                if self.current == Token::Dot {
                    self.advance()?;
                    let second = self.current_ident_owned()?;
                    self.advance()?;
                    Ok(Expr::CompoundColumn(first, second))
                } else {
                    Ok(Expr::Column(first))
                }
            }
            Token::Number(_) => {
                let num_str = if let Token::Number(s) = &self.current {
                    s.clone()
                } else {
                    unreachable!()
                };
                let value = if num_str.contains('.') {
                    if let Ok(f) = num_str.parse::<f64>() {
                        Value::Float(f)
                    } else {
                        return Err(ParseError {
                            message: format!("Invalid float: {}", num_str),
                            position: self.tokenizer.pos,
                        });
                    }
                } else if let Ok(i) = num_str.parse::<i64>() {
                    Value::Integer(i)
                } else {
                    return Err(ParseError {
                        message: format!("Invalid integer: {}", num_str),
                        position: self.tokenizer.pos,
                    });
                };
                self.advance()?;
                Ok(Expr::Literal(value))
            }
            Token::SingleQuotedString(_) => {
                let s = if let Token::SingleQuotedString(s) = &self.current {
                    s.clone()
                } else {
                    unreachable!()
                };
                self.advance()?;
                Ok(Expr::Literal(Value::String(s)))
            }
            Token::KwTrue => {
                self.advance()?;
                Ok(Expr::Literal(Value::Boolean(true)))
            }
            Token::KwFalse => {
                self.advance()?;
                Ok(Expr::Literal(Value::Boolean(false)))
            }
            Token::KwNull => {
                self.advance()?;
                Ok(Expr::Literal(Value::Null))
            }
            Token::Question => {
                self.advance()?;
                Ok(Expr::Placeholder)
            }
            _ => Err(ParseError {
                message: format!("Unexpected token in expression: {:?}", self.current),
                position: self.tokenizer.pos,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_create_table() {
        let mut parser = Parser::new("CREATE TABLE users (id INTEGER, name TEXT)");
        let stmts = parser.parse_statements().unwrap();
        assert_eq!(stmts.len(), 1);
        if let Statement::CreateTable { name, columns } = &stmts[0] {
            assert_eq!(name.as_str(), "users");
            assert_eq!(columns.len(), 2);
            assert_eq!(columns[0].name.as_str(), "id");
            assert_eq!(columns[0].data_type, DataType::Integer);
            assert_eq!(columns[1].name.as_str(), "name");
            assert_eq!(columns[1].data_type, DataType::String);
        } else {
            panic!("Expected CreateTable");
        }
    }

    #[test]
    fn test_parse_select() {
        let mut parser = Parser::new("SELECT * FROM users WHERE id = 1");
        let stmts = parser.parse_statements().unwrap();
        assert_eq!(stmts.len(), 1);
        if let Statement::Select {
            columns, table, ..
        } = &stmts[0]
        {
            assert!(matches!(columns, SelectColumns::Star));
            assert_eq!(table.as_str(), "users");
        } else {
            panic!("Expected Select");
        }
    }

    #[test]
    fn test_parse_insert() {
        let mut parser =
            Parser::new("INSERT INTO users (name, age) VALUES ('Alice', 30)");
        let stmts = parser.parse_statements().unwrap();
        assert_eq!(stmts.len(), 1);
        if let Statement::Insert {
            table,
            columns,
            values,
        } = &stmts[0]
        {
            assert_eq!(table.as_str(), "users");
            assert_eq!(columns.len(), 2);
            assert_eq!(values.len(), 1);
            assert_eq!(values[0].len(), 2);
        } else {
            panic!("Expected Insert");
        }
    }
}
