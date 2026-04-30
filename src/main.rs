use dbobj::core::{Database, Schema, Id, Value, RowData};
use dbobj::storage::Storage;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- DBOBJ Proof of Concept (Optimized) ---");

    // 1. Initialize Database
    let mut db = Database::new("MyDatabase".to_string());

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
    let postcard_bytes = postcard::to_stdvec(&db)?;
    let bincode_config = bincode::config::standard();
    let bincode_bytes = bincode::serde::encode_to_vec(&db, bincode_config)?;
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
    for entry in &db.version_log.entries {
        println!("[{}] Table: {}, ID: {}, Action: {:?}", 
            entry.timestamp(), entry.table_name, entry.row_id, entry.change_type);
    }

    // 7. Loading back
    println!("\nLoading database from disk...");
    let loaded_db = storage.load()?;
    println!("Loaded database: {}", loaded_db.name);
    if let Some(table) = loaded_db.get_table("users") {
        println!("Table 'users' has {} rows.", table.rows.len());
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

    Ok(())
}
