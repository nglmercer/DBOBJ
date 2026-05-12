use super::ast::{ParseError, Token};
use compact_str::CompactString;

// ── Tokenizer ──

pub struct Tokenizer<'a> {
    pub(crate) input: &'a str,
    pub(crate) pos: usize,
    pub(crate) ch: Option<char>,
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
        self.advance();
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
                    self.advance();
                    if self.ch == Some('\'') {
                        content.push('\'');
                        self.advance();
                    } else {
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
        self.advance();
        let start = self.pos;
        let mut end = start;
        while let Some(ch) = self.ch {
            if ch == '"' {
                let content = &self.input[start..end];
                self.advance();
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
            "LIKE" => Ok(Token::KwLike),
            "NOT" => Ok(Token::KwNot),
            "ORDER" => Ok(Token::KwOrder),
            "BY" => Ok(Token::KwBy),
            "ASC" => Ok(Token::KwAsc),
            "DESC" => Ok(Token::KwDesc),
            "LIMIT" => Ok(Token::KwLimit),
            "OFFSET" => Ok(Token::KwOffset),
            "DROP" => Ok(Token::KwDrop),
            "DEFAULT" => Ok(Token::KwDefault),
            "COUNT" => Ok(Token::KwCount),
            "SUM" => Ok(Token::KwSum),
            "MIN" => Ok(Token::KwMin),
            "MAX" => Ok(Token::KwMax),
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
