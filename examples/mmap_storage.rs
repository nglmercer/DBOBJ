use dbobj::core::{ColumnDefinition, DataType, Database, RowData, Schema, Value};
use dbobj::storage::MmapStorage;
use std::time::Instant;

fn main() {
    println!("--- DBOBJ MmapStorage Example ---");

    let path = "example_mmap.db";
    let _ = std::fs::remove_file(path);

    // 1. Build a small database
    let db = Database::new("MmapExample".to_string());
    let schema = Schema {
        columns: vec![
            ColumnDefinition {
                name: "id".into(),
                data_type: DataType::Integer,
                nullable: false,
            },
            ColumnDefinition {
                name: "name".into(),
                data_type: DataType::String,
                nullable: false,
            },
        ],
    };
    db.create_table("users".to_string(), schema);

    let row_count = 100_000;
    let start = Instant::now();
    for i in 0..row_count {
        let mut row = RowData::default();
        row.insert("id".into(), Value::from(i as i64));
        row.insert("name".into(), Value::from(format!("user_{}", i)));
        db.insert_row("users", row, None).unwrap();
    }
    println!("Inserted {} rows in {:?}", row_count, start.elapsed());

    // 2. Save with MmapStorage (rkyv serialization)
    let storage = MmapStorage::new(path);
    storage.save(&db).unwrap();
    println!("Saved to {}", path);

    // 3. Load via mmap — no allocation/copy for the "load" itself
    let mut storage = MmapStorage::new(path);
    let start = Instant::now();
    storage.load().unwrap();
    println!("Mmap load time: {:?}", start.elapsed());

    // 4. Zero-copy access
    let start = Instant::now();
    let archived = storage.access();
    println!("Zero-copy access time: {:?}", start.elapsed());
    println!("Archived DB name: {}", archived.name);
    println!("Archived table count: {}", archived.tables.len());
    println!("Archived row count: {}", archived.tables[0].1.ids.len());

    // 5. Full deserialization (allocates owned types + rebuilds indexes)
    let start = Instant::now();
    let snapshot = storage.deserialize().unwrap();
    let db_loaded = Database::from_snapshot(snapshot);
    println!("Full deserialize + rebuild time: {:?}", start.elapsed());
    println!(
        "Loaded DB table '{}' has {} rows",
        db_loaded.get_table("users").unwrap().read().name,
        db_loaded.get_table("users").unwrap().read().ids.len()
    );

    std::fs::remove_file(path).unwrap();
}
