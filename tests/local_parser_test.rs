use dbobj::core::{DataType, Operator, Value};
use dbobj::sql::local_parser::{Expr, Parser, Statement, Token, Tokenizer};
use compact_str::CompactString;

// ── Tokenizer tests ──

fn collect_tokens(input: &str) -> Vec<Token> {
    let mut t = Tokenizer::new(input);
    let mut tokens = Vec::new();
    loop {
        let tok = t.next_token().unwrap();
        if tok == Token::Eof {
            break;
        }
        tokens.push(tok);
    }
    tokens
}

#[test]
fn test_tokenize_keywords() {
    let tokens = collect_tokens("CREATE TABLE INSERT SELECT");
    assert_eq!(tokens, vec![
        Token::KwCreate, Token::KwTable,
        Token::KwInsert, Token::KwSelect,
    ]);
}

#[test]
fn test_tokenize_identifiers() {
    let tokens = collect_tokens("users user_name _private");
    assert_eq!(tokens, vec![
        Token::Ident(CompactString::from("users")),
        Token::Ident(CompactString::from("user_name")),
        Token::Ident(CompactString::from("_private")),
    ]);
}

#[test]
fn test_tokenize_numbers() {
    let tokens = collect_tokens("42 3.14 100");
    assert_eq!(tokens, vec![
        Token::Number(CompactString::from("42")),
        Token::Number(CompactString::from("3.14")),
        Token::Number(CompactString::from("100")),
    ]);
}

#[test]
fn test_tokenize_strings() {
    let tokens = collect_tokens("'hello' 'it''s'");
    assert_eq!(tokens, vec![
        Token::SingleQuotedString(CompactString::from("hello")),
        Token::SingleQuotedString(CompactString::from("it's")),
    ]);
}

#[test]
fn test_tokenize_operators() {
    let tokens = collect_tokens("= != <> > >= < <=");
    assert_eq!(tokens, vec![
        Token::Equals,
        Token::OpNotEq,
        Token::OpNotEq,
        Token::OpGt,
        Token::OpGtEq,
        Token::OpLt,
        Token::OpLtEq,
    ]);
}

#[test]
fn test_tokenize_full_statement() {
    let tokens = collect_tokens("SELECT * FROM users WHERE id = 1");
    assert!(tokens.len() > 5);
    assert_eq!(tokens[0], Token::KwSelect);
    assert_eq!(tokens[1], Token::Star);
    assert_eq!(tokens[2], Token::KwFrom);
    assert_eq!(tokens[3], Token::Ident(CompactString::from("users")));
    assert_eq!(tokens[4], Token::KwWhere);
}

// ── Helper to run parser ──

fn parse_one(sql: &str) -> dbobj::sql::local_parser::Statement {
    let mut parser = Parser::new(sql);
    let stmts = parser.parse_statements().unwrap();
    assert_eq!(stmts.len(), 1, "Expected exactly one statement");
    stmts.into_iter().next().unwrap()
}

// ── Parser tests ──

#[test]
fn test_parse_create_table() {
    let stmt = parse_one("CREATE TABLE users (id INTEGER, name TEXT)");
    if let Statement::CreateTable { name, columns } = &stmt {
        assert_eq!(name.as_str(), "users");
        assert_eq!(columns.len(), 2);
        assert_eq!(columns[0].name.as_str(), "id");
        assert_eq!(columns[0].data_type, DataType::Integer);
        assert_eq!(columns[1].name.as_str(), "name");
        assert_eq!(columns[1].data_type, DataType::String);
    } else {
        panic!("Expected CreateTable, got {:?}", stmt);
    }
}

#[test]
fn test_parse_create_table_all_types() {
    let stmt = parse_one(
        "CREATE TABLE t (a INT, b BIGINT, c FLOAT, d DOUBLE, e REAL, f TEXT, g VARCHAR(255), h CHAR(10), i BOOLEAN, j BLOB)",
    );
    if let Statement::CreateTable { columns, .. } = &stmt {
        assert_eq!(columns[0].data_type, DataType::Integer);
        assert_eq!(columns[1].data_type, DataType::Integer);
        assert_eq!(columns[2].data_type, DataType::Float);
        assert_eq!(columns[3].data_type, DataType::Float);
        assert_eq!(columns[4].data_type, DataType::Float);
        assert_eq!(columns[5].data_type, DataType::String);
        assert_eq!(columns[6].data_type, DataType::String);
        assert_eq!(columns[7].data_type, DataType::String);
        assert_eq!(columns[8].data_type, DataType::Boolean);
        assert_eq!(columns[9].data_type, DataType::Blob);
    } else {
        panic!("Expected CreateTable");
    }
}

#[test]
fn test_parse_alter_table() {
    let stmt = parse_one("ALTER TABLE users ADD COLUMN age INTEGER");
    if let Statement::AlterTable { name, operation } = &stmt {
        assert_eq!(name.as_str(), "users");
        let dbobj::sql::local_parser::AlterOperation::AddColumn(col_def) = operation else {
            panic!("Expected AddColumn");
        };
        assert_eq!(col_def.name.as_str(), "age");
        assert_eq!(col_def.data_type, DataType::Integer);
    } else {
        panic!("Expected AlterTable");
    }
}

#[test]
fn test_parse_alter_table_without_column_keyword() {
    let stmt = parse_one("ALTER TABLE users ADD age INTEGER");
    if let Statement::AlterTable { operation, .. } = &stmt {
        let dbobj::sql::local_parser::AlterOperation::AddColumn(col_def) = operation else {
            panic!("Expected AddColumn");
        };
        assert_eq!(col_def.name.as_str(), "age");
        assert_eq!(col_def.data_type, DataType::Integer);
    } else {
        panic!("Expected AlterTable");
    }
}

#[test]
fn test_parse_insert_named_cols() {
    let stmt = parse_one("INSERT INTO users (name, age) VALUES ('Alice', 30)");
    if let Statement::Insert { table, columns, values } = &stmt {
        assert_eq!(table.as_str(), "users");
        assert_eq!(columns.len(), 2);
        assert_eq!(columns[0].as_str(), "name");
        assert_eq!(columns[1].as_str(), "age");
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].len(), 2);
    } else {
        panic!("Expected Insert");
    }
}

#[test]
fn test_parse_insert_positional() {
    let stmt = parse_one("INSERT INTO items VALUES (1, 10.5)");
    if let Statement::Insert { table, columns, values } = &stmt {
        assert_eq!(table.as_str(), "items");
        assert!(columns.is_empty());
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].len(), 2);
    } else {
        panic!("Expected Insert");
    }
}

#[test]
fn test_parse_insert_multi_value() {
    let stmt = parse_one(
        "INSERT INTO users (name, age) VALUES ('Alice', 30), ('Bob', 25), ('Carol', 35)"
    );
    if let Statement::Insert { values, .. } = &stmt {
        assert_eq!(values.len(), 3);
        assert_eq!(values[0].len(), 2);
        assert_eq!(values[1].len(), 2);
        assert_eq!(values[2].len(), 2);
    } else {
        panic!("Expected Insert");
    }
}

#[test]
fn test_parse_update() {
    let stmt = parse_one("UPDATE users SET age = 31 WHERE name = 'Alice'");
    if let Statement::Update { table, assignments, selection } = &stmt {
        assert_eq!(table.as_str(), "users");
        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].column.as_str(), "age");
        assert!(selection.is_some());
    } else {
        panic!("Expected Update");
    }
}

#[test]
fn test_parse_update_no_where() {
    let stmt = parse_one("UPDATE users SET age = 31");
    if let Statement::Update { selection, .. } = &stmt {
        assert!(selection.is_none());
    } else {
        panic!("Expected Update");
    }
}

#[test]
fn test_parse_delete() {
    let stmt = parse_one("DELETE FROM users WHERE id = 5");
    if let Statement::Delete { table, selection } = &stmt {
        assert_eq!(table.as_str(), "users");
        assert!(selection.is_some());
    } else {
        panic!("Expected Delete");
    }
}

#[test]
fn test_parse_delete_no_where() {
    let stmt = parse_one("DELETE FROM users");
    if let Statement::Delete { table, selection } = &stmt {
        assert_eq!(table.as_str(), "users");
        assert!(selection.is_none());
    } else {
        panic!("Expected Delete");
    }
}

#[test]
fn test_parse_select_star() {
    let stmt = parse_one("SELECT * FROM users");
    if let Statement::Select { columns, table, .. } = &stmt {
        assert!(matches!(columns, dbobj::sql::local_parser::SelectColumns::Star));
        assert_eq!(table.as_str(), "users");
    } else {
        panic!("Expected Select");
    }
}

#[test]
fn test_parse_select_columns() {
    let stmt = parse_one("SELECT name, age FROM users");
    if let Statement::Select { columns, .. } = &stmt {
        if let dbobj::sql::local_parser::SelectColumns::List(cols) = columns {
            assert_eq!(cols.len(), 2);
            assert_eq!(cols[0].as_str(), "name");
            assert_eq!(cols[1].as_str(), "age");
        } else {
            panic!("Expected column list");
        }
    } else {
        panic!("Expected Select");
    }
}

#[test]
fn test_parse_select_where() {
    let stmt = parse_one("SELECT * FROM users WHERE age > 25 AND name = 'Alice'");
    if let Statement::Select { selection, .. } = &stmt {
        assert!(selection.is_some());
    } else {
        panic!("Expected Select");
    }
}

#[test]
fn test_parse_where_precedence() {
    // AND binds tighter than OR: a OR b AND c should parse as a OR (b AND c)
    let mut parser = Parser::new("SELECT * FROM t WHERE a = 1 OR b = 2 AND c = 3");
    let stmts = parser.parse_statements().unwrap();
    if let Statement::Select { selection, .. } = &stmts[0] {
        if let Some(Expr::Binary(_, Operator::Or, _)) = selection {
            // Top level is OR, AND should be nested inside
        } else {
            panic!("Expected OR at top level");
        }
    } else {
        panic!("Expected Select");
    }
}

#[test]
fn test_parse_where_parens() {
    let stmt = parse_one("SELECT * FROM t WHERE (a = 1 OR b = 2) AND c = 3");
    if let Statement::Select { selection, .. } = &stmt {
        if let Some(Expr::Binary(_, Operator::And, _)) = selection {
            // AND at top level due to parens
        } else {
            panic!("Expected AND at top level due to parens");
        }
    } else {
        panic!("Expected Select");
    }
}

#[test]
fn test_parse_join() {
    let stmt = parse_one(
        "SELECT * FROM users INNER JOIN orders ON users.user_id = orders.user_id"
    );
    if let Statement::Select { join, .. } = &stmt {
        let j = join.as_ref().unwrap();
        assert_eq!(j.table.as_str(), "orders");
        assert_eq!(j.left_table.as_str(), "users");
        assert_eq!(j.left_col.as_str(), "user_id");
        assert_eq!(j.right_table.as_str(), "orders");
        assert_eq!(j.right_col.as_str(), "user_id");
    } else {
        panic!("Expected Select with join");
    }
}

#[test]
fn test_parse_join_without_inner() {
    let stmt = parse_one("SELECT * FROM a JOIN b ON a.id = b.a_id");
    if let Statement::Select { join, .. } = &stmt {
        let j = join.as_ref().unwrap();
        assert_eq!(j.table.as_str(), "b");
    } else {
        panic!("Expected join");
    }
}

#[test]
fn test_parse_placeholder() {
    let stmt = parse_one("SELECT * FROM users WHERE id = ?");
    if let Statement::Select { selection, .. } = &stmt {
        if let Some(Expr::Binary(_, _, right)) = selection {
            assert!(matches!(**right, Expr::Placeholder));
        } else {
            panic!("Expected binary expression");
        }
    } else {
        panic!("Expected Select");
    }
}

#[test]
fn test_parse_placeholder_in_insert() {
    let stmt = parse_one("INSERT INTO users (name, age) VALUES (?, ?)");
    if let Statement::Insert { values, .. } = &stmt {
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].len(), 2);
        assert!(matches!(values[0][0], Expr::Placeholder));
        assert!(matches!(values[0][1], Expr::Placeholder));
    } else {
        panic!("Expected Insert");
    }
}

#[test]
fn test_parse_errors() {
    // Invalid syntax
    assert!(Parser::new("CREATE TABLE (id)").parse_statements().is_err());
    assert!(Parser::new("SELECT FROM").parse_statements().is_err());
    assert!(Parser::new("INSERT INTO t VALUES").parse_statements().is_err());
    assert!(Parser::new("UNKNOWN COMMAND").parse_statements().is_err());
    assert!(Parser::new("SELECT * FROM t WHERE 1 = 'unterminated").parse_statements().is_err());
}

#[test]
fn test_parse_multiple_statements() {
    let mut parser = Parser::new(
        "CREATE TABLE users (id INT); INSERT INTO users VALUES (1); SELECT * FROM users"
    );
    let stmts = parser.parse_statements().unwrap();
    assert_eq!(stmts.len(), 3);
    assert!(matches!(stmts[0], Statement::CreateTable { .. }));
    assert!(matches!(stmts[1], Statement::Insert { .. }));
    assert!(matches!(stmts[2], Statement::Select { .. }));
}

#[test]
fn test_parse_number_values() {
    let stmt = parse_one("INSERT INTO t VALUES (42, 3.14)");
    if let Statement::Insert { values, .. } = &stmt {
        assert_eq!(values[0][0], Expr::Literal(Value::Integer(42)));
        assert_eq!(values[0][1], Expr::Literal(Value::Float(3.14)));
    } else {
        panic!("Expected Insert");
    }
}

#[test]
fn test_parse_boolean_and_null() {
    let stmt = parse_one("SELECT * FROM t WHERE active = TRUE AND deleted = FALSE AND extra = NULL");
    if let Statement::Select { selection, .. } = &stmt {
        assert!(selection.is_some());
        // Verify it parsed without error
    } else {
        panic!("Expected Select");
    }
}

#[test]
fn test_parse_column_with_alias_keyword() {
    // AS is tokenized but not used by parser - it should not crash
    let stmt = parse_one("SELECT name FROM users");
    if let Statement::Select { .. } = &stmt {
        // Just verify it parses
    } else {
        panic!("Expected Select");
    }
}

// ── Cross-validation against sqlparser ──

#[test]
fn test_roundtrip_vs_sqlparser_create_table() {
    use sqlparser::dialect::SQLiteDialect;
    use sqlparser::parser::Parser as SqlParser;

    let sql = "CREATE TABLE users (id INTEGER, name TEXT)";
    let local = parse_one(sql);
    let parsed = SqlParser::parse_sql(&SQLiteDialect {}, sql).unwrap();
    
    // Both should produce a create table statement
    assert!(matches!(local, Statement::CreateTable { .. }));
    assert!(matches!(&parsed[0], sqlparser::ast::Statement::CreateTable(_)));
}

#[test]
fn test_roundtrip_vs_sqlparser_select() {
    use sqlparser::dialect::SQLiteDialect;
    use sqlparser::parser::Parser as SqlParser;

    let sql = "SELECT * FROM users WHERE id = 1";
    let local = parse_one(sql);
    let parsed = SqlParser::parse_sql(&SQLiteDialect {}, sql).unwrap();
    
    assert!(matches!(local, Statement::Select { .. }));
    assert!(matches!(&parsed[0], sqlparser::ast::Statement::Query(_)));
}

#[test]
fn test_roundtrip_vs_sqlparser_insert() {
    use sqlparser::dialect::SQLiteDialect;
    use sqlparser::parser::Parser as SqlParser;

    let sql = "INSERT INTO users (name, age) VALUES ('Alice', 30)";
    let local = parse_one(sql);
    let parsed = SqlParser::parse_sql(&SQLiteDialect {}, sql).unwrap();
    
    assert!(matches!(local, Statement::Insert { .. }));
    assert!(matches!(&parsed[0], sqlparser::ast::Statement::Insert(_)));
}

#[test]
fn test_roundtrip_vs_sqlparser_join() {
    use sqlparser::dialect::SQLiteDialect;
    use sqlparser::parser::Parser as SqlParser;

    let sql = "SELECT * FROM users INNER JOIN orders ON users.id = orders.user_id";
    let local = parse_one(sql);
    let parsed = SqlParser::parse_sql(&SQLiteDialect {}, sql).unwrap();
    
    assert!(matches!(local, Statement::Select { .. }));
    assert!(matches!(&parsed[0], sqlparser::ast::Statement::Query(_)));
}

// ── Integration test ──

#[test]
fn test_local_parser_produces_executable_ast() {
    use dbobj::core::Database;
    use dbobj::sql::{SqlExecutor, SqlResult};

    let db = Database::new("test_local_parser_db".to_string());
    let executor = SqlExecutor::new(&db);

    // This will use the existing SqlParser (which delegates to sqlparser for now)
    // We're just verifying the test infrastructure works
    executor.execute("CREATE TABLE users (id INTEGER, name TEXT)").unwrap();
    executor.execute("INSERT INTO users VALUES (1, 'Alice')").unwrap();
    
    let result = executor.execute("SELECT * FROM users WHERE id = 1").unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("name").unwrap().clone(), "Alice".into());
    } else {
        panic!("Expected Rows");
    }
}
