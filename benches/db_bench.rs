use criterion::{black_box, criterion_group, criterion_main, Criterion};
use dbobj::core::{Database, Schema, Id, RowData, Value, DataType, ColumnDefinition};
use serde_json;
use bincode;

fn create_sample_db(row_count: usize) -> Database {
    let mut db = Database::new("BenchDB".to_string());
    let schema = Schema {
        columns: vec![
            ColumnDefinition {
                name: "username".to_string(),
                data_type: DataType::String,
                nullable: false,
            },
            ColumnDefinition {
                name: "age".to_string(),
                data_type: DataType::Integer,
                nullable: true,
            },
        ],
    };
    db.create_table("users".to_string(), schema);

    for i in 0..row_count {
        let mut row = RowData::new();
        row.insert("username".to_string(), Value::String(format!("user_{}", i)));
        row.insert("age".to_string(), Value::Integer(i as i64));
        let _ = db.insert_row("users", row, Some(Id::String(format!("id_{}", i))));
    }
    db
}

fn bench_insertion(c: &mut Criterion) {
    let schema = Schema {
        columns: vec![
            ColumnDefinition {
                name: "username".to_string(),
                data_type: DataType::String,
                nullable: false,
            },
        ],
    };

    c.bench_function("insert_100_rows", |b| {
        b.iter(|| {
            let mut db = Database::new("Test".to_string());
            db.create_table("users".to_string(), schema.clone());
            for i in 0..100 {
                let mut row = RowData::new();
                row.insert("username".to_string(), Value::String(format!("user_{}", i)));
                let _ = db.insert_row("users", row, None);
            }
        })
    });
}

fn bench_serialization(c: &mut Criterion) {
    let db = create_sample_db(1000);
    
    let mut group = c.benchmark_group("Serialization_1000_Rows");

    group.bench_function("Bincode", |b| {
        let config = bincode::config::standard();
        b.iter(|| {
            let _ = bincode::serde::encode_to_vec(black_box(&db), config).unwrap();
        })
    });

    group.bench_function("JSON", |b| {
        b.iter(|| {
            let _ = serde_json::to_vec(black_box(&db)).unwrap();
        })
    });

    group.finish();
}

fn bench_deserialization(c: &mut Criterion) {
    let db = create_sample_db(1000);
    
    let config = bincode::config::standard();
    let bincode_data = bincode::serde::encode_to_vec(&db, config).unwrap();
    let json_data = serde_json::to_vec(&db).unwrap();

    let mut group = c.benchmark_group("Deserialization_1000_Rows");

    group.bench_function("Bincode", |b| {
        b.iter(|| {
            let (loaded_db, _): (Database, usize) = bincode::serde::decode_from_slice(black_box(&bincode_data), config).unwrap();
            black_box(loaded_db);
        })
    });

    group.bench_function("JSON", |b| {
        b.iter(|| {
            let loaded_db: Database = serde_json::from_slice(black_box(&json_data)).unwrap();
            black_box(loaded_db);
        })
    });

    group.finish();
}

criterion_group!(benches, bench_insertion, bench_serialization, bench_deserialization);
criterion_main!(benches);
