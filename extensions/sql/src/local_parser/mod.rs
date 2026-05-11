pub mod ast;
pub mod tokenizer;

pub use ast::{
    AlterOperation, Assignment, ColumnDef, Expr, Join, ParseError, SelectColumns, Statement, Token,
};
pub use tokenizer::Tokenizer;

use dbobj::{DataType, Operator, Value};
use compact_str::CompactString;

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
            Token::KwBlob | Token::KwBytea | Token::KwVarbinary | Token::KwBinary => DataType::Blob,
            _ => {
                return Err(ParseError {
                    message: format!("Expected data type, got {:?}", self.current),
                    position: self.tokenizer.pos,
                });
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
                (left_col, left_name, right_col, right_name)
            } else {
                (right_col, right_name, left_col, left_name)
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

        if self.current == Token::KwLike {
            self.advance()?;
            let right = self.parse_atom()?;
            return Ok(Expr::Binary(Box::new(left), Operator::Like, Box::new(right)));
        }

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
        if let Statement::Select { columns, table, .. } = &stmts[0] {
            assert!(matches!(columns, SelectColumns::Star));
            assert_eq!(table.as_str(), "users");
        } else {
            panic!("Expected Select");
        }
    }

    #[test]
    fn test_parse_insert() {
        let mut parser = Parser::new("INSERT INTO users (name, age) VALUES ('Alice', 30)");
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
