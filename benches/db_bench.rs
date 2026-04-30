use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use dbobj::core::{Database, Schema, Id, RowData, Value, DataType, ColumnDefinition};
use serde_json;
use bincode;
use postcard;

fn bench_insertion(c: &mut Criterion) {
    let schema = Schema {
        columns: vec![
            ColumnDefinition {
                name: "username".into(),
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
                let mut row = RowData::default();
                row.insert("username".into(), Value::from(format!("user_{}", i)));
                let _ = db.insert_row("users", row, None);
            }
        })
    });
}

fn bench_serialization(c: &mut Criterion) {
    let mut rows = Vec::new();
    for i in 0..1000 {
        let mut row = RowData::default();
        row.insert("username".into(), Value::from(format!("user_{}", i)));
        row.insert("age".into(), Value::from(i as i64));
        rows.push(row);
    }
    
    let mut group = c.benchmark_group("Serialization_1000_Rows");

    group.bench_function("Bincode", |b| {
        let config = bincode::config::standard();
        b.iter(|| {
            let _ = bincode::serde::encode_to_vec(black_box(&rows), config).unwrap();
        })
    });

    group.bench_function("Postcard", |b| {
        b.iter(|| {
            let _ = postcard::to_stdvec(black_box(&rows)).unwrap();
        })
    });

    group.bench_function("JSON", |b| {
        b.iter(|| {
            let _ = serde_json::to_vec(black_box(&rows)).unwrap();
        })
    });

    group.finish();
}

fn bench_deserialization(c: &mut Criterion) {
    let mut rows = Vec::new();
    for i in 0..1000 {
        let mut row = RowData::default();
        row.insert("username".into(), Value::from(format!("user_{}", i)));
        row.insert("age".into(), Value::from(i as i64));
        rows.push(row);
    }
    
    let config = bincode::config::standard();
    let bincode_data = bincode::serde::encode_to_vec(&rows, config).unwrap();
    let postcard_data = postcard::to_stdvec(&rows).unwrap();
    let json_data = serde_json::to_vec(&rows).unwrap();

    let mut group = c.benchmark_group("Deserialization_1000_Rows");

    group.bench_function("Bincode", |b| {
        b.iter(|| {
            let (loaded_rows, _): (Vec<RowData>, usize) = bincode::serde::decode_from_slice(black_box(&bincode_data), config).unwrap();
            black_box(loaded_rows);
        })
    });

    group.bench_function("Postcard", |b| {
        b.iter(|| {
            let loaded_rows: Vec<RowData> = postcard::from_bytes(black_box(&postcard_data)).unwrap();
            black_box(loaded_rows);
        })
    });

    group.bench_function("JSON", |b| {
        b.iter(|| {
            let loaded_rows: Vec<RowData> = serde_json::from_slice(black_box(&json_data)).unwrap();
            black_box(loaded_rows);
        })
    });

    group.finish();
}

criterion_group!(benches, bench_insertion, bench_serialization, bench_deserialization);
criterion_main!(benches);
