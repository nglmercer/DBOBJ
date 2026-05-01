use dbobj::core::{ColumnDefinition, DataType, Database, Schema, Value};
use rusqlite::{Connection, params as sqlite_params};
use std::time::Instant;

fn main() {
    let row_count = 1_000_000;
    println!(
        "--- DBOBJ vs SQLite Performance Test ({} rows) ---",
        row_count
    );

    // --- 1. DBOBJ Setup ---
    let db = Database::new("million_db".to_string());
    let schema = Schema {
        columns: vec![
            ColumnDefinition {
                name: "id".into(),
                data_type: DataType::Integer,
                nullable: false,
            },
            ColumnDefinition {
                name: "username".into(),
                data_type: DataType::String,
                nullable: false,
            },
        ],
    };
    db.create_table("users".to_string(), schema);
    db.create_unique_index("users", "username").unwrap();

    // --- 2. SQLite Setup ---
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, username TEXT)",
        [],
    )
    .unwrap();
    conn.execute("CREATE INDEX idx_username ON users (username)", [])
        .unwrap();

    // --- 3. DBOBJ Inserts (Batch) ---
    println!("Inserting {} rows into DBOBJ...", row_count);
    let start = Instant::now();
    let mut batch = Vec::with_capacity(100_000);
    for i in 0..row_count {
        batch.push(vec![
            Value::from(i as i64),
            Value::from(format!("user_{}", i)),
        ]);
        if batch.len() == 100_000 {
            db.insert_batch_values("users", std::mem::take(&mut batch))
                .unwrap();
        }
    }
    if !batch.is_empty() {
        db.insert_batch_values("users", batch).unwrap();
    }
    let db_insert_time = start.elapsed();
    println!("DBOBJ Insert Time: {:?}", db_insert_time);

    // --- 4. SQLite Inserts (Batch) ---
    println!("Inserting {} rows into SQLite...", row_count);
    let start = Instant::now();
    let tx = conn.transaction().unwrap();
    {
        let mut stmt = tx
            .prepare_cached("INSERT INTO users (id, username) VALUES (?1, ?2)")
            .unwrap();
        for i in 0..row_count {
            stmt.execute(sqlite_params![i as i64, format!("user_{}", i)])
                .unwrap();
        }
    }
    tx.commit().unwrap();
    let sqlite_insert_time = start.elapsed();
    println!("SQLite Insert Time: {:?}", sqlite_insert_time);

    // --- 5. Indexed Search Comparison ---
    println!("\nSearching for username 'user_500000' (Optimized x1000)...");
    let table_lock = db.get_table("users").unwrap();
    let (username_col_idx, id_col_idx) = {
        let table = table_lock.read();
        (
            table.get_index_handle("username").unwrap(),
            table.get_column_index("id").unwrap(),
        )
    };
    let search_val = Value::from("user_500000");

    let mut lookup_val = search_val.clone();
    {
        let table = table_lock.read();
        if let Value::String(s) = &search_val
            && let Some(id) = table.string_pool.get_id(s.as_str())
        {
            lookup_val = Value::InternedString(id);
        }
    }

    let start = Instant::now();
    for _ in 0..1000 {
        let table = table_lock.read();
        let index = table.indexes.get("username").unwrap();
        
        let row_idx = *index.unique_map.get(&lookup_val).unwrap();
        let _val = table.get_value_by_index(row_idx, id_col_idx);
    }
    let db_search_time = start.elapsed() / 1000;
    println!("DBOBJ Search Time: {:?}", db_search_time);

    // --- 5.1 ID Lookup (Primary Key) ---
    println!("Looking up ID 500000 directly (x10000 amortized)...");
    let table = table_lock.read();
    let id_to_find = dbobj::core::Id::from(500000u64);

    let start = Instant::now();
    for _ in 0..10000 {
        if let dbobj::core::Id::Integer(i) = &id_to_find {
            let row_idx = *i as usize;
            let _val = table.get_value_by_index(row_idx, id_col_idx);
        }
    }
    let db_id_lookup_time = start.elapsed() / 10000;
    println!("DBOBJ ID Lookup Time: {:?}", db_id_lookup_time);

    {
        let mut stmt = conn
            .prepare("SELECT id FROM users WHERE username = ?1")
            .unwrap();
        let start = Instant::now();
        let sqlite_id: i64 = stmt
            .query_row(sqlite_params!["user_500000"], |r| r.get(0))
            .unwrap();
        let sqlite_search_time = start.elapsed();
        println!(
            "SQLite Search Result: Found ID {} in {:?}",
            sqlite_id, sqlite_search_time
        );
    }

    // --- 6. Memory Comparison (Approximate) ---
    println!("\n--- Performance Summary ---");
    println!(
        "Insert Speed: DBOBJ is {:.2}x {}",
        if db_insert_time < sqlite_insert_time {
            sqlite_insert_time.as_secs_f64() / db_insert_time.as_secs_f64()
        } else {
            db_insert_time.as_secs_f64() / sqlite_insert_time.as_secs_f64()
        },
        if db_insert_time < sqlite_insert_time {
            "faster"
        } else {
            "slower"
        }
    );
    println!(
        "Search Speed: DBOBJ is {:.2}x {}",
        if db_search_time < sqlite_search_time {
            sqlite_search_time.as_secs_f64() / db_search_time.as_secs_f64()
        } else {
            db_search_time.as_secs_f64() / sqlite_search_time.as_secs_f64()
        },
        if db_search_time < sqlite_search_time {
            "faster"
        } else {
            "slower"
        }
    );

    // --- 7. Join Test (Smaller subset for safety) ---
    let join_size = 100_000;
    println!("\nJoining {} rows...", join_size);

    // Setup posts for join
    let post_schema = Schema {
        columns: vec![
            ColumnDefinition {
                name: "user_id".into(),
                data_type: DataType::Integer,
                nullable: false,
            },
            ColumnDefinition {
                name: "title".into(),
                data_type: DataType::String,
                nullable: false,
            },
        ],
    };
    db.create_table("posts".to_string(), post_schema);
    let mut p_batch = Vec::with_capacity(join_size);
    for i in 0..join_size {
        p_batch.push(vec![
            Value::from(i as i64),
            Value::from(format!("Post {}", i)),
        ]);
    }
    db.insert_batch_values("posts", p_batch).unwrap();

    conn.execute("CREATE TABLE posts (user_id INTEGER, title TEXT)", [])
        .unwrap();
    let tx = conn.transaction().unwrap();
    {
        let mut stmt = tx
            .prepare_cached("INSERT INTO posts (user_id, title) VALUES (?1, ?2)")
            .unwrap();
        for i in 0..join_size {
            stmt.execute(sqlite_params![i as i64, format!("Post {}", i)])
                .unwrap();
        }
    }
    tx.commit().unwrap();

    let start = Instant::now();
    let joined = db.hash_join("users", "id", "posts", "user_id").unwrap();
    let db_join_time = start.elapsed();
    println!(
        "DBOBJ Hash Join ({} rows): {:?}",
        joined.len(),
        db_join_time
    );

    let start = Instant::now();
    {
        let mut stmt = conn.prepare("SELECT users.username, posts.title FROM users JOIN posts ON users.id = posts.user_id LIMIT ?1").unwrap();
        let sqlite_rows: Vec<_> = stmt.query_map([join_size], |_| Ok(())).unwrap().collect();
        println!(
            "SQLite Join ({} rows): {:?}",
            sqlite_rows.len(),
            start.elapsed()
        );
    }
}
