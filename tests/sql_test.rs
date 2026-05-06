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
fn test_sql_positional_insert() {
    let db = Database::new("test_db".to_string());
    let executor = SqlExecutor::new(&db);

    executor.execute("CREATE TABLE items (id INTEGER, price FLOAT)").unwrap();
    executor.execute("INSERT INTO items VALUES (1, 10.5)").unwrap();

    let result = executor.execute("SELECT * FROM items WHERE price = 10.5").unwrap();
    if let SqlResult::Rows(rows) = result {
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("id").unwrap().clone(), 1.into());
    } else {
        panic!("Expected Rows result");
    }
}
