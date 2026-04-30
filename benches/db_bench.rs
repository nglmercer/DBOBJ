use criterion::{criterion_group, criterion_main, Criterion, BatchSize};
use dbobj::core::{Database, Schema, ColumnDefinition, DataType, RowData, Value};
use rusqlite::{Connection, params};
use std::time::Duration;

fn bench_inserts(c: &mut Criterion) {
    let mut group = c.benchmark_group("Inserts");
    group.sample_size(10); // Reduced from 100
    group.measurement_time(Duration::from_secs(3)); // Reduced from 5

    // DBOBJ Setup
    group.bench_function("DBOBJ Insert", |b| {
        let mut db = Database::new("bench_db".to_string());
        let schema = Schema {
            columns: vec![
                ColumnDefinition { name: "username".into(), data_type: DataType::String, nullable: false },
                ColumnDefinition { name: "age".into(), data_type: DataType::Integer, nullable: false },
            ],
        };
        db.create_table("users".to_string(), schema);

        b.iter_batched(
            || {
                let mut data = RowData::default();
                data.insert("username".into(), Value::from("alice"));
                data.insert("age".into(), Value::from(30i64));
                data
            },
            |data| {
                db.insert_row("users", data, None).unwrap();
            },
            BatchSize::SmallInput,
        )
    });

    // SQLite Setup
    group.bench_function("SQLite Insert", |b| {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, username TEXT, age INTEGER)",
            [],
        ).unwrap();

        b.iter_batched(
            || ("alice", 30),
            |(name, age)| {
                conn.execute(
                    "INSERT INTO users (username, age) VALUES (?1, ?2)",
                    params![name, age],
                ).unwrap();
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

fn bench_reads(c: &mut Criterion) {
    let mut group = c.benchmark_group("Reads");
    group.sample_size(10); // Reduced from 100
    group.measurement_time(Duration::from_secs(3)); // Reduced from 5

    // DBOBJ Setup
    let mut db = Database::new("bench_db".to_string());
    let schema = Schema {
        columns: vec![
            ColumnDefinition { name: "username".into(), data_type: DataType::String, nullable: false },
            ColumnDefinition { name: "age".into(), data_type: DataType::Integer, nullable: false },
        ],
    };
    db.create_table("users".to_string(), schema);
    let mut ids = Vec::new();
    for i in 0..1000 {
        let mut data = RowData::default();
        data.insert("username".into(), Value::from(format!("user{}", i)));
        data.insert("age".into(), Value::from(i as i64));
        ids.push(db.insert_row("users", data, None).unwrap());
    }

    group.bench_function("DBOBJ Read", |b| {
        let mut i = 0;
        b.iter(|| {
            let id = &ids[i % ids.len()];
            let _ = db.get_table("users").unwrap().get(id).unwrap();
            i += 1;
        })
    });

    // SQLite Setup
    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, username TEXT, age INTEGER)",
        [],
    ).unwrap();
    for i in 0..1000 {
        conn.execute(
            "INSERT INTO users (username, age) VALUES (?1, ?2)",
            params![format!("user{}", i), i as i64],
        ).unwrap();
    }

    group.bench_function("SQLite Read", |b| {
        let mut i = 1;
        b.iter(|| {
            let id = (i % 1000) + 1;
            let mut stmt = conn.prepare("SELECT username, age FROM users WHERE id = ?1").unwrap();
            let _ = stmt.query_row(params![id], |row| {
                let name: String = row.get(0)?;
                let age: i64 = row.get(1)?;
                Ok((name, age))
            }).unwrap();
            i += 1;
        })
    });

    group.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default().warm_up_time(Duration::from_secs(1));
    targets = bench_inserts, bench_reads
);
criterion_main!(benches);
