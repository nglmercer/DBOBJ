use dbobj::core::{Database, Schema, Id, Value, RowData};
use dbobj::storage::Storage;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- DBOBJ Proof of Concept ---");

    // 1. Initialize Database
    let mut db = Database::new("MyDatabase".to_string());

    // 2. Define Schema
    let schema = Schema {
        columns: vec![
            dbobj::core::ColumnDefinition {
                name: "username".to_string(),
                data_type: dbobj::core::DataType::String,
                nullable: false,
            },
            dbobj::core::ColumnDefinition {
                name: "age".to_string(),
                data_type: dbobj::core::DataType::Integer,
                nullable: true,
            },
        ],
    };

    db.create_table("users".to_string(), schema);

    // 3. Insert Rows
    println!("Inserting data...");
    
    let mut user1 = RowData::new();
    user1.insert("username".to_string(), Value::String("alice".to_string()));
    user1.insert("age".to_string(), Value::Integer(30));
    let id1 = db.insert_row("users", user1, None)?;
    println!("Inserted Alice with ID: {}", id1);

    let mut user2 = RowData::new();
    user2.insert("username".to_string(), Value::String("bob".to_string()));
    user2.insert("age".to_string(), Value::Integer(25));
    let id2 = db.insert_row("users", user2, Some(Id::String("bob_unique_id".to_string())))?;
    println!("Inserted Bob with ID: {}", id2);

    // 4. Persistence
    let storage = Storage::new("my_database.db");
    println!("Saving database to my_database.db...");
    storage.save(&db)?;

    // 5. Demonstrate Backups
    println!("Modifying and saving again to trigger backup...");
    let mut user3 = RowData::new();
    user3.insert("username".to_string(), Value::String("charlie".to_string()));
    db.insert_row("users", user3, None)?;
    storage.save(&db)?;
    println!("Check for my_database.bak in the directory.");

    // 6. Versioning History
    println!("\n--- Version History ---");
    for entry in &db.version_log.entries {
        println!("[{}] Table: {}, ID: {}, Action: {:?}", 
            entry.timestamp, entry.table_name, entry.row_id, entry.change_type);
    }

    // 7. Loading back
    println!("\nLoading database from disk...");
    let loaded_db = storage.load()?;
    println!("Loaded database: {}", loaded_db.name);
    if let Some(table) = loaded_db.get_table("users") {
        println!("Table 'users' has {} rows.", table.rows.len());
    }

    Ok(())
}
