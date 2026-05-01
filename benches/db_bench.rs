use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use dbobj::core::{ColumnDefinition, DataType, Database, RowData, Schema, Value};
use rusqlite::{Connection, params as sqlite_params};
use std::time::Duration;

fn bench_inserts(c: &mut Criterion) {
    let mut group = c.benchmark_group("Inserts");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));

    // 1. DBOBJ
    group.bench_function("DBOBJ Insert", |b| {
        let db = Database::new("bench_db".to_string());
        let schema = Schema {
            columns: vec![
                ColumnDefinition {
                    name: "username".into(),
                    data_type: DataType::String,
                    nullable: false,
                },
                ColumnDefinition {
                    name: "age".into(),
                    data_type: DataType::Integer,
                    nullable: false,
                },
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
        )
        .unwrap();

        b.iter_batched(
            || ("alice", 30),
            |(name, age)| {
                conn.execute(
                    "INSERT INTO users (username, age) VALUES (?1, ?2)",
                    sqlite_params![name, age],
                )
                .unwrap();
            },
            BatchSize::SmallInput,
        )
    });

    // 3. DBOBJ Batch
    group.bench_function("DBOBJ Batch Insert (100 rows)", |b| {
        let db = Database::new("bench_db".to_string());
        let schema = Schema {
            columns: vec![
                ColumnDefinition {
                    name: "username".into(),
                    data_type: DataType::String,
                    nullable: false,
                },
                ColumnDefinition {
                    name: "age".into(),
                    data_type: DataType::Integer,
                    nullable: false,
                },
            ],
        };
        db.create_table("users_batch".to_string(), schema);

        b.iter_batched(
            || {
                let mut batch = Vec::with_capacity(100);
                for i in 0..100 {
                    let mut data = RowData::default();
                    data.insert("username".into(), Value::from(format!("user_{}", i)));
                    data.insert("age".into(), Value::from(i as i64));
                    batch.push(data);
                }
                batch
            },
            |batch| {
                db.insert_batch("users_batch", batch).unwrap();
            },
            BatchSize::SmallInput,
        )
    });

    // 4. DBOBJ Batch Values (optimized)
    group.bench_function("DBOBJ Batch Raw Insert (100 rows)", |b| {
        let db = Database::new("bench_db".to_string());
        let schema = Schema {
            columns: vec![
                ColumnDefinition {
                    name: "username".into(),
                    data_type: DataType::String,
                    nullable: false,
                },
                ColumnDefinition {
                    name: "age".into(),
                    data_type: DataType::Integer,
                    nullable: false,
                },
            ],
        };
        db.create_table("users_batch_raw".to_string(), schema);

        b.iter_batched(
            || {
                let mut batch = Vec::with_capacity(100);
                for i in 0..100 {
                    batch.push(vec![
                        Value::from(format!("user_{}", i)),
                        Value::from(i as i64),
                    ]);
                }
                batch
            },
            |batch| {
                db.insert_batch_values("users_batch_raw", batch).unwrap();
            },
            BatchSize::SmallInput,
        )
    });

    // 4. SQLite Batch
    group.bench_function("SQLite Batch Insert (100 rows)", |b| {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE users_batch (id INTEGER PRIMARY KEY, username TEXT, age INTEGER)",
            [],
        )
        .unwrap();

        b.iter_batched(
            || {
                let mut batch = Vec::with_capacity(100);
                for i in 0..100 {
                    batch.push((format!("user_{}", i), i));
                }
                batch
            },
            |batch| {
                let tx = conn.transaction().unwrap();
                {
                    let mut stmt = tx
                        .prepare_cached("INSERT INTO users_batch (username, age) VALUES (?1, ?2)")
                        .unwrap();
                    for (name, age) in batch {
                        stmt.execute(sqlite_params![name, age]).unwrap();
                    }
                }
                tx.commit().unwrap();
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

fn bench_reads(c: &mut Criterion) {
    let mut group = c.benchmark_group("Reads");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));

    // 1. DBOBJ
    let db = Database::new("bench_db".to_string());
    let schema = Schema {
        columns: vec![
            ColumnDefinition {
                name: "username".into(),
                data_type: DataType::String,
                nullable: false,
            },
            ColumnDefinition {
                name: "age".into(),
                data_type: DataType::Integer,
                nullable: false,
            },
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
            let _ = db.get_table("users").unwrap().read().get(id).unwrap();
            i += 1;
        })
    });

    // 2. SQLite
    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, username TEXT, age INTEGER)",
        [],
    )
    .unwrap();
    for i in 0..1000 {
        conn.execute(
            "INSERT INTO users (username, age) VALUES (?1, ?2)",
            sqlite_params![format!("user{}", i), i as i64],
        )
        .unwrap();
    }

    group.bench_function("SQLite Read", |b| {
        let mut i = 1;
        b.iter(|| {
            let id = (i % 1000) + 1;
            let mut stmt = conn
                .prepare("SELECT username, age FROM users WHERE id = ?1")
                .unwrap();
            let _ = stmt
                .query_row(sqlite_params![id], |row| {
                    let name: String = row.get(0)?;
                    let age: i64 = row.get(1)?;
                    Ok((name, age))
                })
                .unwrap();
            i += 1;
        })
    });

    group.finish();
}

fn bench_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("Search");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));

    let row_count = 5000;

    // 1. DBOBJ (Indexed vs Unindexed)
    let db = Database::new("bench_db".to_string());
    let schema = Schema {
        columns: vec![
            ColumnDefinition {
                name: "username".into(),
                data_type: DataType::String,
                nullable: false,
            },
            ColumnDefinition {
                name: "age".into(),
                data_type: DataType::Integer,
                nullable: false,
            },
        ],
    };
    db.create_table("users".to_string(), schema);
    for i in 0..row_count {
        let mut data = RowData::default();
        data.insert("username".into(), Value::from(format!("user{}", i)));
        data.insert("age".into(), Value::from(i as i64));
        db.insert_row("users", data, None).unwrap();
    }

    group.bench_function("DBOBJ Scan (O(N))", |b| {
        b.iter(|| {
            // Searching on 'age' (no index)
            let _ = db.find("users", "age", Value::from(2500i64)).unwrap();
        })
    });

    db.create_index("users", "username").unwrap();
    group.bench_function("DBOBJ Indexed (O(log N))", |b| {
        b.iter(|| {
            // Searching on 'username' (with index)
            let _ = db
                .find("users", "username", Value::from("user2500"))
                .unwrap();
        })
    });

    // 2. SQLite (Indexed vs Unindexed)
    let conn = Connection::open_in_memory().unwrap();
    conn.execute("CREATE TABLE users (username TEXT, age INTEGER)", [])
        .unwrap();
    for i in 0..row_count {
        conn.execute(
            "INSERT INTO users (username, age) VALUES (?1, ?2)",
            sqlite_params![format!("user{}", i), i as i64],
        )
        .unwrap();
    }

    group.bench_function("SQLite Scan", |b| {
        b.iter(|| {
            let mut stmt = conn
                .prepare("SELECT username FROM users WHERE age = 2500")
                .unwrap();
            let _ = stmt
                .query_row([], |r| r.get::<_, String>(0))
                .unwrap_or_default();
        })
    });

    conn.execute("CREATE INDEX idx_username ON users (username)", [])
        .unwrap();
    group.bench_function("SQLite Indexed", |b| {
        b.iter(|| {
            let mut stmt = conn
                .prepare("SELECT age FROM users WHERE username = 'user2500'")
                .unwrap();
            let _ = stmt.query_row([], |r| r.get::<_, i64>(0)).unwrap();
        })
    });

    group.finish();
}

fn bench_joins(c: &mut Criterion) {
    let mut group = c.benchmark_group("Joins");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));

    let row_count = 1000;

    // 1. DBOBJ Hash Join
    let db = Database::new("bench_db".to_string());
    db.create_table(
        "users".to_string(),
        Schema {
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
        },
    );
    db.create_table(
        "posts".to_string(),
        Schema {
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
        },
    );

    for i in 0..row_count {
        let mut u = RowData::default();
        u.insert("id".into(), Value::from(i as i64));
        u.insert("name".into(), Value::from(format!("user{}", i)));
        db.insert_row("users", u, None).unwrap();

        let mut p = RowData::default();
        p.insert("user_id".into(), Value::from(i as i64));
        p.insert("title".into(), Value::from(format!("post{}", i)));
        db.insert_row("posts", p, None).unwrap();
    }

    group.bench_function("DBOBJ Hash Join", |b| {
        b.iter(|| {
            let _ = db.hash_join("users", "id", "posts", "user_id").unwrap();
        })
    });

    // 2. SQLite Join
    let conn = Connection::open_in_memory().unwrap();
    conn.execute("CREATE TABLE users (id INTEGER, name TEXT)", [])
        .unwrap();
    conn.execute("CREATE TABLE posts (user_id INTEGER, title TEXT)", [])
        .unwrap();
    for i in 0..row_count {
        conn.execute(
            "INSERT INTO users (id, name) VALUES (?1, ?2)",
            sqlite_params![i as i64, format!("user{}", i)],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO posts (user_id, title) VALUES (?1, ?2)",
            sqlite_params![i as i64, format!("post{}", i)],
        )
        .unwrap();
    }

    group.bench_function("SQLite Join", |b| {
        b.iter(|| {
            let mut stmt = conn.prepare("SELECT users.name, posts.title FROM users JOIN posts ON users.id = posts.user_id").unwrap();
            let _rows = stmt.query_map([], |_| Ok(())).unwrap().collect::<Vec<_>>();
        })
    });

    group.finish();
}

fn bench_large_joins(c: &mut Criterion) {
    let mut group = c.benchmark_group("LargeJoins");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(5));

    let row_count = 100_000;

    // 1. DBOBJ Hash Join
    let db = Database::new("bench_db".to_string());
    db.create_table(
        "users".to_string(),
        Schema {
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
        },
    );
    db.create_table(
        "posts".to_string(),
        Schema {
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
        },
    );

    let mut u_batch = Vec::with_capacity(row_count);
    let mut p_batch = Vec::with_capacity(row_count);
    for i in 0..row_count {
        u_batch.push(vec![
            Value::from(i as i64),
            Value::from(format!("user{}", i)),
        ]);
        p_batch.push(vec![
            Value::from(i as i64),
            Value::from(format!("post{}", i)),
        ]);
    }
    db.insert_batch_values("users", u_batch).unwrap();
    db.insert_batch_values("posts", p_batch).unwrap();

    group.bench_function("DBOBJ Large Hash Join (100k)", |b| {
        b.iter(|| {
            let _ = db.hash_join("users", "id", "posts", "user_id").unwrap();
        })
    });

    // 2. SQLite Join
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute("CREATE TABLE users (id INTEGER, name TEXT)", [])
        .unwrap();
    conn.execute("CREATE TABLE posts (user_id INTEGER, title TEXT)", [])
        .unwrap();

    let tx = conn.transaction().unwrap();
    {
        let mut u_stmt = tx
            .prepare_cached("INSERT INTO users (id, name) VALUES (?1, ?2)")
            .unwrap();
        let mut p_stmt = tx
            .prepare_cached("INSERT INTO posts (user_id, title) VALUES (?1, ?2)")
            .unwrap();
        for i in 0..row_count {
            u_stmt
                .execute(sqlite_params![i as i64, format!("user{}", i)])
                .unwrap();
            p_stmt
                .execute(sqlite_params![i as i64, format!("post{}", i)])
                .unwrap();
        }
    }
    tx.commit().unwrap();

    group.bench_function("SQLite Large Join (100k)", |b| {
        b.iter(|| {
            let mut stmt = conn.prepare("SELECT users.name, posts.title FROM users JOIN posts ON users.id = posts.user_id").unwrap();
            let _rows = stmt.query_map([], |_| Ok(())).unwrap().collect::<Vec<_>>();
        })
    });

    group.finish();
}

fn bench_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("Serialization");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));

    let db = Database::new("bench_db".to_string());
    db.create_table(
        "users".to_string(),
        Schema {
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
        },
    );

    let mut batch = Vec::with_capacity(10_000);
    for i in 0..10_000 {
        batch.push(vec![
            Value::from(i as i64),
            Value::from(format!("user_{}", i)),
        ]);
    }
    db.insert_batch_values("users", batch).unwrap();

    let bitcode_bytes = bitcode::serialize(&db).unwrap();

    group.bench_function("Bitcode Serialize", |b| {
        b.iter(|| {
            bitcode::serialize(&db).unwrap();
        })
    });

    group.bench_function("Bitcode Deserialize", |b| {
        b.iter(|| {
            let _: Database = bitcode::deserialize(&bitcode_bytes).unwrap();
        })
    });

    group.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default().warm_up_time(Duration::from_secs(1));
    targets = bench_inserts, bench_reads, bench_search, bench_joins, bench_large_joins, bench_serialization
);
criterion_main!(benches);
