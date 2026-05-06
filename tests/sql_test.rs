use dbobj::core::Database;
use dbobj::sql::{SqlExecutor, SqlResult};

#[test]
fn test_sql_basic_flow() {
    let db = Database::new("test_db".to_string());
    let executor = SqlExecutor::new(&db);

    // CREATE TABLE
    executor.execute("CREATE TABLE users (id INTEGER, name TEXT, age INTEGER)").unwrap();

    // INSERT
    executor.execute("INSERT INTO users (id, name, age) VALUES (1, 'Alice', 30)").unwrap();
    executor.execute("INSERT INTO users (id, name, age) VALUES (2, 'Bob', 25)").unwrap();

    // SELECT ALL
    let result = executor.execute("SELECT * FROM users").unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows.len(), 2);
    } else {
        panic!("Expected Rows result");
    }

    // SELECT WHERE
    let result = executor.execute("SELECT * FROM users WHERE age > 27").unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("name").unwrap().clone(), "Alice".into());
    } else {
        panic!("Expected Rows result");
    }
}

#[test]
fn test_sql_update_delete() {
    let db = Database::new("test_db".to_string());
    let executor = SqlExecutor::new(&db);

    executor.execute("CREATE TABLE users (id INTEGER, name TEXT, age INTEGER)").unwrap();
    executor.execute("INSERT INTO users (id, name, age) VALUES (1, 'Alice', 30)").unwrap();

    // UPDATE
    executor.execute("UPDATE users SET age = 31 WHERE name = 'Alice'").unwrap();
    let result = executor.execute("SELECT * FROM users WHERE name = 'Alice'").unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows[0].get("age").unwrap().clone(), 31.into());
    }

    // DELETE
    executor.execute("DELETE FROM users WHERE name = 'Alice'").unwrap();
    let result = executor.execute("SELECT * FROM users").unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows.len(), 0);
    }
}

#[test]
fn test_sql_alter_table() {
    let db = Database::new("test_db".to_string());
    let executor = SqlExecutor::new(&db);

    executor.execute("CREATE TABLE users (id INTEGER, name TEXT)").unwrap();
    executor.execute("INSERT INTO users (id, name) VALUES (1, 'Alice')").unwrap();

    // ALTER TABLE
    executor.execute("ALTER TABLE users ADD COLUMN age INTEGER").unwrap();

    // Check if new column exists and is Null for existing row
    let result = executor.execute("SELECT * FROM users").unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows[0].get("age").unwrap().clone(), dbobj::core::Value::Null);
    }

    // Update the new column
    executor.execute("UPDATE users SET age = 30 WHERE id = 1").unwrap();
    let result = executor.execute("SELECT * FROM users").unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows[0].get("age").unwrap().clone(), 30.into());
    }
}

#[test]
fn test_sql_join() {
    let db = Database::new("test_db".to_string());
    let executor = SqlExecutor::new(&db);

    executor.execute("CREATE TABLE users (user_id INTEGER, name TEXT)").unwrap();
    executor.execute("CREATE TABLE orders (order_id INTEGER, user_id INTEGER, amount FLOAT)").unwrap();

    executor.execute("INSERT INTO users VALUES (1, 'Alice')").unwrap();
    executor.execute("INSERT INTO users VALUES (2, 'Bob')").unwrap();
    executor.execute("INSERT INTO orders VALUES (101, 1, 50.5)").unwrap();
    executor.execute("INSERT INTO orders VALUES (102, 1, 20.0)").unwrap();

    let result = executor.execute("SELECT * FROM users INNER JOIN orders ON users.user_id = orders.user_id").unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows.len(), 2);
        // Find Alice in the results
        let alice_row = rows.iter().find(|r| r.get("users.name").map(|v| v == &"Alice".into()).unwrap_or(false));
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

    // Unsupported operation
    assert!(executor.execute("DROP TABLE users").is_err());
}

#[test]
fn test_sql_positional_insert() {
    let db = Database::new("test_db".to_string());
    let executor = SqlExecutor::new(&db);

    executor.execute("CREATE TABLE items (id INTEGER, price FLOAT)").unwrap();
    executor.execute("INSERT INTO items VALUES (1, 10.5)").unwrap();

    let result = executor.execute("SELECT * FROM items WHERE id = 1").unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("id").unwrap().clone(), 1.into());
    } else {
        panic!("Expected Rows result");
    }
}
