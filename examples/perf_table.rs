use dbobj::core::{ColumnDefinition, DataType, Database, Schema, Value};
use rusqlite::{Connection, params as sqlite_params};
use std::time::{Duration, Instant};

fn main() {
    let row_count = 100_000;
    println!("--- DBOBJ vs SQLite Performance Summary ({} rows) ---", row_count);

    let mut results = Vec::new();

    // 1. Single Insert
    {
        let db = Database::new("bench_db".to_string());
        db.create_table("users".to_string(), Schema {
            columns: vec![ColumnDefinition { name: "val".into(), data_type: DataType::Integer, nullable: false }]
        });
        let start = Instant::now();
        for i in 0..1000 {
            let mut data = dbobj::core::RowData::default();
            data.insert("val".into(), Value::from(i as i64));
            db.insert_row("users", data, None).unwrap();
        }
        let db_time = start.elapsed() / 1000;

        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE users (val INTEGER)", []).unwrap();
        let start = Instant::now();
        for i in 0..1000 {
            conn.execute("INSERT INTO users (val) VALUES (?1)", sqlite_params![i as i64]).unwrap();
        }
        let sqlite_time = start.elapsed() / 1000;
        results.push(("Single Insert", db_time, sqlite_time));
    }

    // 2. Batch Insert
    {
        let db = Database::new("bench_db".to_string());
        db.create_table("users".to_string(), Schema {
            columns: vec![ColumnDefinition { name: "val".into(), data_type: DataType::Integer, nullable: false }]
        });
        let batch: Vec<Vec<Value>> = (0..100).map(|i| vec![Value::from(i as i64)]).collect();
        let start = Instant::now();
        for _ in 0..100 {
            db.insert_batch_values("users", batch.clone()).unwrap();
        }
        let db_time = start.elapsed() / 10000; // time per 100 rows

        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE users (val INTEGER)", []).unwrap();
        let start = Instant::now();
        for _ in 0..100 {
            let tx = conn.transaction().unwrap();
            {
                let mut stmt = tx.prepare_cached("INSERT INTO users (val) VALUES (?)").unwrap();
                for i in 0..100 {
                    stmt.execute(sqlite_params![i as i64]).unwrap();
                }
            }
            tx.commit().unwrap();
        }
        let sqlite_time = start.elapsed() / 10000;
        results.push(("Batch Insert (100)", db_time, sqlite_time));
    }

    // 3. Read by ID
    {
        let db = Database::new("bench_db".to_string());
        db.create_table("users".to_string(), Schema {
            columns: vec![ColumnDefinition { name: "val".into(), data_type: DataType::Integer, nullable: false }]
        });
        let batch: Vec<Vec<Value>> = (0..1000).map(|i| vec![Value::from(i as i64)]).collect();
        db.insert_batch_values("users", batch).unwrap();
        let start = Instant::now();
        for i in 0..10000 {
            let _ = db.get_table("users").unwrap().read().get(&dbobj::core::Id::from(i as u64 % 1000)).unwrap();
        }
        let db_time = start.elapsed() / 10000;

        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, val INTEGER)", []).unwrap();
        for i in 0..1000 {
            conn.execute("INSERT INTO users (id, val) VALUES (?1, ?2)", sqlite_params![i as i64, i as i64]).unwrap();
        }
        let start = Instant::now();
        for i in 0..10000 {
            let _: i64 = conn.query_row("SELECT val FROM users WHERE id = ?", sqlite_params![(i % 1000) as i64], |r| r.get(0)).unwrap();
        }
        let sqlite_time = start.elapsed() / 10000;
        results.push(("Read by ID", db_time, sqlite_time));
    }

    // 4. Indexed Search
    {
        let db = Database::new("bench_db".to_string());
        db.create_table("users".to_string(), Schema {
            columns: vec![ColumnDefinition { name: "username".into(), data_type: DataType::String, nullable: false }]
        });
        let batch: Vec<Vec<Value>> = (0..10000).map(|i| vec![Value::from(format!("user{}", i))]).collect();
        db.insert_batch_values("users", batch).unwrap();
        db.create_index("users", "username").unwrap();
        let start = Instant::now();
        for _ in 0..1000 {
            let _ = db.find("users", "username", Value::from("user5000")).unwrap();
        }
        let db_time = start.elapsed() / 1000;

        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE users (username TEXT)", []).unwrap();
        let tx = conn.transaction().unwrap();
        {
            let mut stmt = tx.prepare_cached("INSERT INTO users (username) VALUES (?)").unwrap();
            for i in 0..10000 {
                stmt.execute(sqlite_params![format!("user{}", i)]).unwrap();
            }
        }
        tx.commit().unwrap();
        conn.execute("CREATE INDEX idx_user ON users (username)", []).unwrap();
        let start = Instant::now();
        for _ in 0..1000 {
            let _: String = conn.query_row("SELECT username FROM users WHERE username = 'user5000'", [], |r| r.get(0)).unwrap();
        }
        let sqlite_time = start.elapsed() / 1000;
        results.push(("Indexed Search", db_time, sqlite_time));
    }

    // 5. Hash Join
    {
        let db = Database::new("bench_db".to_string());
        db.create_table("u".to_string(), Schema { columns: vec![ColumnDefinition { name: "id".into(), data_type: DataType::Integer, nullable: false }] });
        db.create_table("p".to_string(), Schema { columns: vec![ColumnDefinition { name: "uid".into(), data_type: DataType::Integer, nullable: false }] });
        let u_batch: Vec<Vec<Value>> = (0..row_count).map(|i| vec![Value::from(i as i64)]).collect();
        let p_batch: Vec<Vec<Value>> = (0..row_count).map(|i| vec![Value::from(i as i64)]).collect();
        db.insert_batch_values("u", u_batch).unwrap();
        db.insert_batch_values("p", p_batch).unwrap();
        
        let start = Instant::now();
        let _ = db.hash_join("u", "id", "p", "uid").unwrap();
        let db_time = start.elapsed();

        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE u (id INTEGER)", []).unwrap();
        conn.execute("CREATE TABLE p (uid INTEGER)", []).unwrap();
        let tx = conn.transaction().unwrap();
        {
            let mut u_stmt = tx.prepare_cached("INSERT INTO u (id) VALUES (?)").unwrap();
            let mut p_stmt = tx.prepare_cached("INSERT INTO p (uid) VALUES (?)").unwrap();
            for i in 0..row_count {
                u_stmt.execute(sqlite_params![i as i64]).unwrap();
                p_stmt.execute(sqlite_params![i as i64]).unwrap();
            }
        }
        tx.commit().unwrap();
        let start = Instant::now();
        let mut stmt = conn.prepare("SELECT u.id, p.uid FROM u JOIN p ON u.id = p.uid").unwrap();
        let _rows: Vec<_> = stmt.query_map([], |_| Ok(())).unwrap().collect();
        let sqlite_time = start.elapsed();
        results.push(("Hash Join (100k)", db_time, sqlite_time));
    }

    println!("\n| Operation | DBOBJ | SQLite | Ratio | Verdict |");
    println!("|:---|:---|:---|:---|:---|");
    for (op, db_t, sql_t) in results {
        let ratio = sql_t.as_secs_f64() / db_t.as_secs_f64();
        let verdict = if ratio > 1.1 { "✅ DBOBJ" } else if ratio < 0.9 { "❌ SQLite" } else { "⚖️ Equal" };
        println!("| {} | {:?} | {:?} | {:.2}x | {} |", op, db_t, sql_t, ratio, verdict);
    }
}
