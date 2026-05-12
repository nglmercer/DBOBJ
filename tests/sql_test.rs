use dbobj::core::Database;
use dbobj_sql::{SqlExecutor, SqlResult};

#[test]
fn test_sql_basic_flow() {
    let db = Database::new("test_db".to_string());
    let executor = SqlExecutor::new(&db);

    // CREATE TABLE
    executor
        .execute("CREATE TABLE users (id INTEGER, name TEXT, age INTEGER)")
        .unwrap();

    // INSERT
    executor
        .execute("INSERT INTO users (id, name, age) VALUES (1, 'Alice', 30)")
        .unwrap();
    executor
        .execute("INSERT INTO users (id, name, age) VALUES (2, 'Bob', 25)")
        .unwrap();

    // SELECT ALL
    let result = executor.execute("SELECT * FROM users").unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows.len(), 2);
    } else {
        panic!("Expected Rows result");
    }

    // SELECT WHERE
    let result = executor
        .execute("SELECT * FROM users WHERE age > 27")
        .unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("name").unwrap().clone(), "Alice".into());
    } else {
        panic!("Expected Rows result");
    }
}

#[test]
fn test_prepared_statements() {
    use dbobj::core::Value;
    let db = Database::new("test_db".to_string());
    let executor = SqlExecutor::new(&db);

    executor
        .execute("CREATE TABLE users (name TEXT, age INTEGER)")
        .unwrap();

    // Prepared INSERT
    let insert_stmt = executor
        .prepare("INSERT INTO users (name, age) VALUES (?, ?)")
        .unwrap();
    executor
        .execute_prepared(&insert_stmt, &[Value::from("Alice"), Value::from(30i64)])
        .unwrap();
    executor
        .execute_prepared(&insert_stmt, &[Value::from("Bob"), Value::from(25i64)])
        .unwrap();

    // Prepared SELECT
    let select_stmt = executor
        .prepare("SELECT * FROM users WHERE age > ?")
        .unwrap();
    let result = executor
        .execute_prepared(&select_stmt, &[Value::from(27i64)])
        .unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("name").unwrap().clone(), "Alice".into());
    } else {
        panic!("Expected Rows");
    }

    // Prepared UPDATE
    let update_stmt = executor
        .prepare("UPDATE users SET age = ? WHERE name = ?")
        .unwrap();
    executor
        .execute_prepared(&update_stmt, &[Value::from(31i64), Value::from("Alice")])
        .unwrap();

    let result = executor
        .execute_prepared(&select_stmt, &[Value::from(30i64)])
        .unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("name").unwrap().clone(), "Alice".into());
    }

    // Prepared DELETE
    let delete_stmt = executor
        .prepare("DELETE FROM users WHERE name = ?")
        .unwrap();
    executor
        .execute_prepared(&delete_stmt, &[Value::from("Alice")])
        .unwrap();

    let result = executor.execute("SELECT * FROM users").unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows.len(), 1);
    }
}

#[test]
fn test_sql_batch_insert() {
    let db = Database::new("test_db".to_string());
    let executor = SqlExecutor::new(&db);

    executor
        .execute("CREATE TABLE scores (id INTEGER, value INTEGER)")
        .unwrap();

    // Insert 100 rows via SQL, one at a time
    for i in 0..100 {
        let sql = format!("INSERT INTO scores (id, value) VALUES ({}, {})", i, i * 10);
        executor.execute(&sql).unwrap();
    }

    let result = executor.execute("SELECT * FROM scores").unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows.len(), 100);
    } else {
        panic!("Expected Rows result");
    }

    // Verify with WHERE
    let result = executor
        .execute("SELECT * FROM scores WHERE value = 500")
        .unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("id").unwrap().clone(), 50.into());
    } else {
        panic!("Expected Rows result");
    }
}

#[test]
fn test_sql_indexed_search() {
    let db = Database::new("test_db".to_string());
    let executor = SqlExecutor::new(&db);

    executor
        .execute("CREATE TABLE users (name TEXT, age INTEGER)")
        .unwrap();

    for i in 0..100 {
        let sql = format!("INSERT INTO users (name, age) VALUES ('user{}', {})", i, i);
        executor.execute(&sql).unwrap();
    }

    // Create index via direct API (SQL executor doesn't support CREATE INDEX)
    db.create_index("users", "name").unwrap();

    // SQL SELECT with WHERE on indexed column should still work
    let result = executor
        .execute("SELECT * FROM users WHERE name = 'user42'")
        .unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("age").unwrap().clone(), 42.into());
    } else {
        panic!("Expected Rows result");
    }

    // SQL SELECT with WHERE on non-indexed column
    let result = executor
        .execute("SELECT * FROM users WHERE age = 77")
        .unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("name").unwrap().clone(), "user77".into());
    } else {
        panic!("Expected Rows result");
    }
}

#[test]
fn test_sql_update_delete() {
    let db = Database::new("test_db".to_string());
    let executor = SqlExecutor::new(&db);

    executor
        .execute("CREATE TABLE users (id INTEGER, name TEXT, age INTEGER)")
        .unwrap();
    executor
        .execute("INSERT INTO users (id, name, age) VALUES (1, 'Alice', 30)")
        .unwrap();

    // UPDATE
    executor
        .execute("UPDATE users SET age = 31 WHERE name = 'Alice'")
        .unwrap();
    let result = executor
        .execute("SELECT * FROM users WHERE name = 'Alice'")
        .unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows[0].get("age").unwrap().clone(), 31.into());
    }

    // DELETE
    executor
        .execute("DELETE FROM users WHERE name = 'Alice'")
        .unwrap();
    let result = executor.execute("SELECT * FROM users").unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows.len(), 0);
    }
}

#[test]
fn test_sql_alter_table() {
    let db = Database::new("test_db".to_string());
    let executor = SqlExecutor::new(&db);

    executor
        .execute("CREATE TABLE users (id INTEGER, name TEXT)")
        .unwrap();
    executor
        .execute("INSERT INTO users (id, name) VALUES (1, 'Alice')")
        .unwrap();

    // ALTER TABLE
    executor
        .execute("ALTER TABLE users ADD COLUMN age INTEGER")
        .unwrap();

    // Check if new column exists and is Null for existing row
    let result = executor.execute("SELECT * FROM users").unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(
            rows[0].get("age").unwrap().clone(),
            dbobj::core::Value::Null
        );
    }

    // Update the new column
    executor
        .execute("UPDATE users SET age = 30 WHERE id = 1")
        .unwrap();
    let result = executor.execute("SELECT * FROM users").unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows[0].get("age").unwrap().clone(), 30.into());
    }
}

#[test]
fn test_sql_join() {
    let db = Database::new("test_sql_join_db".to_string());
    let executor = SqlExecutor::new(&db);

    executor
        .execute("CREATE TABLE users (user_id INTEGER, name TEXT)")
        .unwrap();
    executor
        .execute("CREATE TABLE orders (order_id INTEGER, user_id INTEGER, amount FLOAT)")
        .unwrap();

    executor
        .execute("INSERT INTO users VALUES (1, 'Alice')")
        .unwrap();
    executor
        .execute("INSERT INTO users VALUES (2, 'Bob')")
        .unwrap();
    executor
        .execute("INSERT INTO orders VALUES (101, 1, 50.5)")
        .unwrap();
    executor
        .execute("INSERT INTO orders VALUES (102, 1, 20.0)")
        .unwrap();

    let result = executor
        .execute("SELECT * FROM users INNER JOIN orders ON users.user_id = orders.user_id")
        .unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows.len(), 2, "Expected 2 joined rows");
        let alice_row = rows.iter().find(|r| {
            r.get("users.name")
                .map(|v| v == &"Alice".into())
                .unwrap_or(false)
        });
        assert!(alice_row.is_some());
    } else {
        panic!("Expected Rows result");
    }
}

#[test]
fn test_sql_errors() {
    let db = Database::new("test_db".to_string());
    let executor = SqlExecutor::new(&db);

    // Syntax error
    assert!(executor.execute("CREATE TABLE (id)").is_err());

    // Missing table
    assert!(executor.execute("SELECT * FROM non_existent").is_err());

    // DROP TABLE on non-existent table returns error
    assert!(executor.execute("DROP TABLE non_existent").is_err());
}

#[test]
fn test_sql_positional_insert() {
    let db = Database::new("test_db".to_string());
    let executor = SqlExecutor::new(&db);

    executor
        .execute("CREATE TABLE items (id INTEGER, price FLOAT)")
        .unwrap();
    executor
        .execute("INSERT INTO items VALUES (1, 10.5)")
        .unwrap();

    let result = executor
        .execute("SELECT * FROM items WHERE id = 1")
        .unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("id").unwrap().clone(), 1.into());
    } else {
        panic!("Expected Rows result");
    }
}

// ── New SQL feature tests ──

#[test]
fn test_sql_drop_table() {
    let db = Database::new("test_db".to_string());
    let executor = SqlExecutor::new(&db);
    executor.execute("CREATE TABLE t (id INTEGER)").unwrap();
    executor.execute("INSERT INTO t VALUES (1)").unwrap();
    executor.execute("DROP TABLE t").unwrap();
    assert!(executor.execute("SELECT * FROM t").is_err());
}

#[test]
fn test_sql_order_by() {
    let db = Database::new("test_db".to_string());
    let executor = SqlExecutor::new(&db);
    executor
        .execute("CREATE TABLE t (id INTEGER, name TEXT)")
        .unwrap();
    executor.execute("INSERT INTO t VALUES (2, 'Bob')").unwrap();
    executor
        .execute("INSERT INTO t VALUES (1, 'Alice')")
        .unwrap();
    executor
        .execute("INSERT INTO t VALUES (3, 'Charlie')")
        .unwrap();

    let result = executor.execute("SELECT * FROM t ORDER BY name").unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows[0].get("name").unwrap().clone(), "Alice".into());
        assert_eq!(rows[2].get("name").unwrap().clone(), "Charlie".into());
    }

    let result = executor
        .execute("SELECT * FROM t ORDER BY name DESC")
        .unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows[0].get("name").unwrap().clone(), "Charlie".into());
    }
}

#[test]
fn test_sql_limit_offset() {
    let db = Database::new("test_db".to_string());
    let executor = SqlExecutor::new(&db);
    executor.execute("CREATE TABLE t (id INTEGER)").unwrap();
    for i in 0..10 {
        executor
            .execute(&format!("INSERT INTO t VALUES ({})", i))
            .unwrap();
    }

    let result = executor
        .execute("SELECT * FROM t ORDER BY id LIMIT 3")
        .unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows.len(), 3);
    }

    let result = executor
        .execute("SELECT * FROM t ORDER BY id LIMIT 3 OFFSET 7")
        .unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows.len(), 3);
    } // rows 7,8,9
}

#[test]
fn test_sql_aggregation() {
    let db = Database::new("test_db".to_string());
    let executor = SqlExecutor::new(&db);
    executor.execute("CREATE TABLE t (val INTEGER)").unwrap();
    executor
        .execute("INSERT INTO t VALUES (10), (20), (30)")
        .unwrap();

    let result = executor.execute("SELECT COUNT(*) FROM t").unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows[0].get("COUNT(*)").unwrap().clone(), 3.into());
    }

    let result = executor.execute("SELECT SUM(val) FROM t").unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows[0].get("SUM").unwrap().clone(), 60.into());
    }

    let result = executor.execute("SELECT MIN(val) FROM t").unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows[0].get("MIN").unwrap().clone(), 10.into());
    }

    let result = executor.execute("SELECT MAX(val) FROM t").unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows[0].get("MAX").unwrap().clone(), 30.into());
    }
}

#[test]
fn test_sql_not_null_create_table() {
    let db = Database::new("test_db".to_string());
    let executor = SqlExecutor::new(&db);
    executor
        .execute("CREATE TABLE t (id INTEGER NOT NULL, name TEXT)")
        .unwrap();
    // Table created successfully
    executor
        .execute("INSERT INTO t (id, name) VALUES (1, 'Alice')")
        .unwrap();
    let result = executor.execute("SELECT * FROM t").unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows.len(), 1);
    }
}

#[test]
fn test_sql_like_with_order() {
    let db = Database::new("test_db".to_string());
    let executor = SqlExecutor::new(&db);
    executor
        .execute("CREATE TABLE t (id INTEGER, name TEXT)")
        .unwrap();
    executor
        .execute("INSERT INTO t VALUES (1, 'Alice'), (2, 'Bob'), (3, 'Alex')")
        .unwrap();

    let result = executor
        .execute("SELECT * FROM t WHERE name LIKE 'A%' ORDER BY id")
        .unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows.len(), 2);
    }
}

#[test]
fn test_multi_value_insert() {
    let db = Database::new("test_db".to_string());
    let executor = SqlExecutor::new(&db);

    executor
        .execute("CREATE TABLE users (name TEXT, age INTEGER)")
        .unwrap();

    // Multi-value INSERT
    let result = executor
        .execute("INSERT INTO users (name, age) VALUES ('Alice', 30), ('Bob', 25), ('Carol', 35)")
        .unwrap();
    assert!(matches!(result, SqlResult::Ok));

    let result = executor.execute("SELECT * FROM users").unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows.len(), 3);
    }
}
