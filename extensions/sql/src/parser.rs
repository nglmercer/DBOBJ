use crate::local_parser::{Parser as LocalParser, Statement};

pub struct SqlParser;

impl SqlParser {
    #[inline]
    pub fn parse(sql: &str) -> Result<Vec<Statement>, String> {
        let mut parser = LocalParser::new(sql);
        parser.parse_statements().map_err(|e| e.to_string())
    }
}
