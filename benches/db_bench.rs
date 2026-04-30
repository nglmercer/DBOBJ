use criterion::{criterion_group, criterion_main, Criterion, BatchSize};
use dbobj::core::{Database, Schema, ColumnDefinition, DataType, RowData, Value};
use rusqlite::{Connection, params as sqlite_params};
use postgres::{Client, NoTls};
use std::time::Duration;

fn bench_inserts(c: &mut Criterion) {
    let mut group = c.benchmark_group("Inserts");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));

    // 1. DBOBJ
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

    // 2. SQLite
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
                    sqlite_params![name, age],
                ).unwrap();
            },
            BatchSize::SmallInput,
        )
    });

    // 3. Postgres (Optional: Requires setup_postgres.sh to be running)
    if let Ok(mut client) = Client::connect("host=localhost port=5433 dbname=bench_db", NoTls) {
        group.bench_function("Postgres Insert", |b| {
            client.execute("DROP TABLE IF EXISTS users_bench_insert", &[]).unwrap();
            client.execute(
                "CREATE TABLE users_bench_insert (id SERIAL PRIMARY KEY, username TEXT, age INTEGER)",
                &[],
            ).unwrap();

            b.iter_batched(
                || ("alice", 30i32),
                |(name, age)| {
                    client.execute(
                        "INSERT INTO users_bench_insert (username, age) VALUES ($1, $2)",
                        &[&name, &age],
                    ).unwrap();
                },
                BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

fn bench_reads(c: &mut Criterion) {
    let mut group = c.benchmark_group("Reads");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));

    // 1. DBOBJ
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

    // 2. SQLite
    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, username TEXT, age INTEGER)",
        [],
    ).unwrap();
    for i in 0..1000 {
        conn.execute(
            "INSERT INTO users (username, age) VALUES (?1, ?2)",
            sqlite_params![format!("user{}", i), i as i64],
        ).unwrap();
    }

    group.bench_function("SQLite Read", |b| {
        let mut i = 1;
        b.iter(|| {
            let id = (i % 1000) + 1;
            let mut stmt = conn.prepare("SELECT username, age FROM users WHERE id = ?1").unwrap();
            let _ = stmt.query_row(sqlite_params![id], |row| {
                let name: String = row.get(0)?;
                let age: i64 = row.get(1)?;
                Ok((name, age))
            }).unwrap();
            i += 1;
        })
    });

    // 3. Postgres (Optional)
    if let Ok(mut client) = Client::connect("host=localhost port=5433 dbname=bench_db", NoTls) {
        client.execute("DROP TABLE IF EXISTS users_bench_read", &[]).unwrap();
        client.execute(
            "CREATE TABLE users_bench_read (id SERIAL PRIMARY KEY, username TEXT, age INTEGER)",
            &[],
        ).unwrap();
        for i in 0..1000 {
            client.execute(
                "INSERT INTO users_bench_read (username, age) VALUES ($1, $2)",
                &[&format!("user{}", i), &(i as i32)],
            ).unwrap();
        }

        group.bench_function("Postgres Read", |b| {
            let mut i = 1;
            b.iter(|| {
                let id = (i % 1000) + 1;
                let row = client.query_one(
                    "SELECT username, age FROM users_bench_read WHERE id = $1",
                    &[&id],
                ).unwrap();
                let _: String = row.get(0);
                let _: i32 = row.get(1);
                i += 1;
            })
        });
    }

    group.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default().warm_up_time(Duration::from_secs(1));
    targets = bench_inserts, bench_reads
);
criterion_main!(benches);
