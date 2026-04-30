use dbobj::core::{Database, Schema, Id, Value, RowData, Expr, Operator};
use dbobj::storage::{Storage, wal::Wal};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- DBOBJ Proof of Concept (Optimized) ---");

    // 1. Initialize Database
    let db = Arc::new(Database::new("MyDatabase".to_string()));

    // 2. Define Schema
    let schema = Schema {
        columns: vec![
            dbobj::core::ColumnDefinition {
                name: "username".into(),
                data_type: dbobj::core::DataType::String,
                nullable: false,
            },
            dbobj::core::ColumnDefinition {
                name: "age".into(),
                data_type: dbobj::core::DataType::Integer,
                nullable: true,
            },
        ],
    };

    db.create_table("users".to_string(), schema);
    
    // Create an index on 'username' for O(log N) lookups
    db.create_index("users", "username")?;
    println!("Created index on users.username");

    // 3. Insert Rows
    println!("Inserting data...");
    
    // Default ID (auto-incrementing integer)
    let mut user1 = RowData::default();
    user1.insert("username".into(), Value::from("alice"));
    user1.insert("age".into(), Value::from(30i64));
    let id1 = db.insert_row("users", user1, None)?;
    println!("Inserted Alice with ID: {}", id1);

    // Custom String ID
    let mut user2 = RowData::default();
    user2.insert("username".into(), Value::from("bob"));
    user2.insert("age".into(), Value::from(25i64));
    let id2 = db.insert_row("users", user2, Some(Id::from("bob_unique_id")))?;
    println!("Inserted Bob with ID: {}", id2);

    // 4. Persistence
    use dbobj::storage::PostcardAdapter;
    let storage = Storage::new("my_database.db", PostcardAdapter);
    println!("Saving database to my_database.db...");
    storage.save(&db)?;

    // Size comparison (Technical Demonstration)
    let postcard_bytes = postcard::to_stdvec(&*db)?;
    let bincode_config = bincode::config::standard();
    let bincode_bytes = bincode::serde::encode_to_vec(&*db, bincode_config)?;
    println!("--- Size Comparison ---");
    println!("Bincode size: {} bytes", bincode_bytes.len());
    println!("Postcard size: {} bytes", postcard_bytes.len());
    println!("Postcard is {:.1}% smaller", 
        (1.0 - (postcard_bytes.len() as f64 / bincode_bytes.len() as f64)) * 100.0);

    // 5. Demonstrate Backups
    println!("Modifying and saving again to trigger backup...");
    let mut user3 = RowData::default();
    user3.insert("username".into(), Value::from("charlie"));
    db.insert_row("users", user3, None)?;
    storage.save(&db)?;
    println!("Check for my_database.bak in the directory.");

    // 6. Versioning History
    println!("\n--- Version History ---");
    for entry in &db.version_log.read().entries {
        println!("[{}] Table: {}, ID: {}, Action: {:?}", 
            entry.timestamp(), entry.table_name, entry.row_id, entry.change_type);
    }

    // 7. Loading back
    println!("\nLoading database from disk...");
    let loaded_db = storage.load()?;
    println!("Loaded database: {}", loaded_db.name);
    if let Some(table_lock) = loaded_db.get_table("users") {
        println!("Table 'users' has {} rows.", table_lock.read().rows.len());
    }

    // 8. Relational Search (Queries)
    println!("\n--- Relational Search Queries ---");
    
    // Find by exact column value
    println!("Searching for username 'alice'...");
    let alice_rows = db.find("users", "username", Value::from("alice"))?;
    for row in alice_rows {
        println!("Found: ID={}, Data={:?}", row.id, row.data);
    }

    // Predicate search (e.g., age > 26)
    println!("\nSearching for users with age > 26...");
    let older_users = db.query("users", |row| {
        if let Some(Value::Integer(age)) = row.data.get("age") {
            *age > 26
        } else {
            false
        }
    })?;
    for row in older_users {
        println!("Found: ID={}, Data={:?}", row.id, row.data);
    }

    // 9. Relational Joins
    println!("\n--- Relational Joins ---");
    // Create posts table
    let post_schema = Schema {
        columns: vec![
            dbobj::core::ColumnDefinition { name: "user_id".into(), data_type: dbobj::core::DataType::Integer, nullable: false },
            dbobj::core::ColumnDefinition { name: "title".into(), data_type: dbobj::core::DataType::String, nullable: false },
        ],
    };
    db.create_table("posts".to_string(), post_schema);

    // Insert a post for Alice (ID 1)
    let mut post1 = RowData::default();
    post1.insert("user_id".into(), Value::from(1i64));
    post1.insert("title".into(), Value::from("First Post"));
    db.insert_row("posts", post1, None)?;

    println!("Joining 'users' and 'posts' on users.id == posts.user_id...");
    let user_posts = db.join("users", "posts", |u, p| {
        if let Some(Value::Integer(p_uid)) = p.data.get("user_id") {
            if let Id::Integer(u_id) = &u.id {
                return *u_id == *p_uid as u64;
            }
        }
        false
    })?;

    for (user, post) in user_posts {
        println!("User '{:?}' posted: '{:?}'", 
            user.data.get("username").unwrap_or(&Value::from("Unknown")),
            post.data.get("title").unwrap_or(&Value::from("No Title"))
        );
    }

    // New: Optimized Hash Join
    println!("\n--- Optimized Hash Join (O(N+M)) ---");
    // We need to convert users.id to a column value for hash_join if we want to join on it, 
    // or join on user_id columns. Since users.id is an Id type and posts.user_id is a Value::Integer,
    // let's add a user_id column to users for this demo or just use the existing data.
    // For now, let's join on 'username' if we had it in both, but let's just show the API.
    
    // Let's create a temporary table for a more natural hash join demo
    let meta_schema = Schema {
        columns: vec![
            dbobj::core::ColumnDefinition { name: "username".into(), data_type: dbobj::core::DataType::String, nullable: false },
            dbobj::core::ColumnDefinition { name: "bio".into(), data_type: dbobj::core::DataType::String, nullable: true },
        ],
    };
    db.create_table("metadata".to_string(), meta_schema);
    let mut meta1 = RowData::default();
    meta1.insert("username".into(), Value::from("alice"));
    meta1.insert("bio".into(), Value::from("Software Engineer"));
    db.insert_row("metadata", meta1, None)?;

    println!("Performing Hash Join on 'username'...");
    let bio_join = db.hash_join("users", "username", "metadata", "username")?;
    for (user, meta) in bio_join {
        println!("User: {:?}, Bio: {:?}", 
            user.data.get("username").unwrap(),
            meta.data.get("bio").unwrap()
        );
    }

    // 10. Schema Validation Demo
    println!("\n--- Schema Validation Demo ---");
    let mut invalid_user = RowData::default();
    invalid_user.insert("username".into(), Value::from(123i64)); // Should be String
    match db.insert_row("users", invalid_user, None) {
        Err(e) => println!("Caught expected error: {}", e),
        Ok(_) => println!("Error: Should have failed validation!"),
    }

    // 11. Expression Queries & Optimizer Demo
    println!("\n--- Expression Queries & Optimizer ---");
    let alice_expr = Expr::Binary(
        Box::new(Expr::Column("username".into())),
        Operator::Eq,
        Box::new(Expr::Literal(Value::from("alice")))
    );
    
    // Check the plan
    if let Some(table_lock) = db.get_table("users") {
        let table = table_lock.read();
        let plan = alice_expr.plan(&table);
        println!("Plan for 'username == alice': {:?}", plan);
    }
    
    let results = db.query_expr("users", alice_expr)?;
    println!("Expr Query found {} rows.", results.len());

    // 12. Transactions Demo
    println!("\n--- Transactions Demo ---");
    println!("Current row count in 'users': {}", db.get_table("users").unwrap().read().rows.len());
    
    {
        let tx = db.begin_transaction();
        println!("Starting transaction and deleting Bob...");
        tx.db.delete_row("users", &Id::from("bob_unique_id"))?;
        println!("Temporary count: {}", tx.db.get_table("users").unwrap().read().rows.len());
        
        println!("Rolling back transaction...");
        tx.rollback();
    }
    
    println!("Count after rollback: {}", db.get_table("users").unwrap().read().rows.len());

    // 13. Concurrency Stress Test
    println!("\n--- Concurrency Stress Test ---");
    let mut handles = vec![];
    for i in 0..4 {
        let db_clone = Arc::clone(&db);
        let handle = std::thread::spawn(move || {
            for j in 0..100 {
                let mut data = RowData::default();
                data.insert("username".into(), Value::from(format!("user_{}_{}", i, j)));
                data.insert("age".into(), Value::from(20 + i + j));
                let _ = db_clone.insert_row("users", data, None);
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    println!("Total rows in 'users' after stress test: {}", db.get_table("users").unwrap().read().rows.len());

    // 14. WAL Recovery Demo
    println!("\n--- WAL Recovery Demo ---");
    let wal_path = "test_wal.log";
    let wal = Wal::new(wal_path)?;
    let db_wal = Database::new("WalDB".to_string()).with_wal(wal);
    
    let schema = Schema {
        columns: vec![dbobj::core::ColumnDefinition { name: "data".into(), data_type: dbobj::core::DataType::String, nullable: false }],
    };
    db_wal.create_table("logs".into(), schema);
    
    println!("Inserting rows into WAL-enabled DB...");
    let mut row = RowData::default();
    row.insert("data".into(), Value::from("Entry 1"));
    db_wal.insert_row("logs", row, None)?;
    
    println!("Simulating 'crash' by creating new DB instance and recovering from WAL...");
    let recovered_db = Database::new("RecoveredDB".to_string())
        .with_wal(Wal::new(wal_path)?);
    
    // We need to recreate the table schema first in this simple recovery model
    let schema = Schema {
        columns: vec![dbobj::core::ColumnDefinition { name: "data".into(), data_type: dbobj::core::DataType::String, nullable: false }],
    };
    recovered_db.create_table("logs".into(), schema);
    
    recovered_db.recover_from_wal()?;
    
    if let Some(table) = recovered_db.get_table("logs") {
        println!("Recovered {} rows from WAL.", table.read().rows.len());
    }

    Ok(())
}
