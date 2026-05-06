use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use dbobj::core::{ColumnDefinition, DataType, Database, RowData, Schema, Value};
use dbobj::sql::SqlExecutor;
use std::time::Duration;

fn bench_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("SQL vs Direct: Insert");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));

    // Direct API
    group.bench_function("Direct insert_row", |b| {
        let db = Database::new("bench_db".to_string());
        let schema = Schema {
            columns: vec![
                ColumnDefinition {
                    name: "username".into(),
                    data_type: DataType::String,
                    nullable: true,
                },
                ColumnDefinition {
                    name: "age".into(),
                    data_type: DataType::Integer,
                    nullable: true,
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

    // SQL API
    group.bench_function("SQL INSERT", |b| {
        let db = Database::new("bench_db".to_string());
        let schema = Schema {
            columns: vec![
                ColumnDefinition {
                    name: "username".into(),
                    data_type: DataType::String,
                    nullable: true,
                },
                ColumnDefinition {
                    name: "age".into(),
                    data_type: DataType::Integer,
                    nullable: true,
                },
            ],
        };
        db.create_table("users".to_string(), schema);
        let executor = SqlExecutor::new(&db);

        b.iter_batched(
            || "INSERT INTO users (username, age) VALUES ('alice', 30)",
            |sql| {
                executor.execute(sql).unwrap();
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

fn bench_batch_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("SQL vs Direct: Batch Insert (100 rows)");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));

    // Direct API — insert_batch_values
    group.bench_function("Direct insert_batch_values", |b| {
        let db = Database::new("bench_db".to_string());
        let schema = Schema {
            columns: vec![
                ColumnDefinition {
                    name: "username".into(),
                    data_type: DataType::String,
                    nullable: true,
                },
                ColumnDefinition {
                    name: "age".into(),
                    data_type: DataType::Integer,
                    nullable: true,
                },
            ],
        };
        db.create_table("users".to_string(), schema);

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
                db.insert_batch_values("users", batch).unwrap();
            },
            BatchSize::SmallInput,
        )
    });

    // SQL API — 100 individual INSERTs
    group.bench_function("SQL 100× INSERT", |b| {
        let db = Database::new("bench_db".to_string());
        let schema = Schema {
            columns: vec![
                ColumnDefinition {
                    name: "username".into(),
                    data_type: DataType::String,
                    nullable: true,
                },
                ColumnDefinition {
                    name: "age".into(),
                    data_type: DataType::Integer,
                    nullable: true,
                },
            ],
        };
        db.create_table("users".to_string(), schema);
        let executor = SqlExecutor::new(&db);

        b.iter_batched(
            || {
                let mut stmts = Vec::with_capacity(100);
                for i in 0..100 {
                    stmts.push(format!(
                        "INSERT INTO users (username, age) VALUES ('user_{}', {})",
                        i, i
                    ));
                }
                stmts
            },
            |stmts| {
                for sql in &stmts {
                    executor.execute(sql).unwrap();
                }
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

fn bench_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("SQL vs Direct: Read by ID");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));

    // Setup: 1000 rows
    let db = Database::new("bench_db".to_string());
    let schema = Schema {
        columns: vec![
            ColumnDefinition {
                name: "username".into(),
                data_type: DataType::String,
                nullable: true,
            },
            ColumnDefinition {
                name: "age".into(),
                data_type: DataType::Integer,
                nullable: true,
            },
        ],
    };
    db.create_table("users".to_string(), schema);
    let mut ids = Vec::with_capacity(1000);
    for i in 0..1000 {
        let mut data = RowData::default();
        data.insert("username".into(), Value::from(format!("user{}", i)));
        data.insert("age".into(), Value::from(i as i64));
        ids.push(db.insert_row("users", data, None).unwrap());
    }

    let executor = SqlExecutor::new(&db);

    // Direct API — get_table + get
    group.bench_function("Direct get_table + get", |b| {
        let mut i = 0;
        b.iter(|| {
            let id = &ids[i % ids.len()];
            let _ = db.get_table("users").unwrap().read().get(id).unwrap();
            i += 1;
        })
    });

    // SQL API — SELECT with WHERE on id
    group.bench_function("SQL SELECT ... WHERE id =", |b| {
        let mut i = 1;
        b.iter(|| {
            let sql = format!("SELECT username, age FROM users WHERE id = {}", i);
            let _ = executor.execute(&sql).unwrap();
            i = (i % 1000) + 1;
        })
    });

    group.finish();
}

fn bench_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("SQL vs Direct: Search (filter)");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));

    let row_count = 5000;

    // Setup
    let db = Database::new("bench_db".to_string());
    let schema = Schema {
        columns: vec![
            ColumnDefinition {
                name: "username".into(),
                data_type: DataType::String,
                nullable: true,
            },
            ColumnDefinition {
                name: "age".into(),
                data_type: DataType::Integer,
                nullable: true,
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

    let executor = SqlExecutor::new(&db);

    // Direct API — find (scan)
    group.bench_function("Direct find (scan)", |b| {
        b.iter(|| {
            let _ = db.find("users", "age", Value::from(2500i64)).unwrap();
        })
    });

    // SQL API — SELECT with WHERE
    group.bench_function("SQL SELECT ... WHERE age =", |b| {
        b.iter(|| {
            let _ = executor
                .execute("SELECT username FROM users WHERE age = 2500")
                .unwrap();
        })
    });

    // With index
    db.create_index("users", "username").unwrap();

    group.bench_function("Direct indexed find", |b| {
        b.iter(|| {
            let _ = db
                .find("users", "username", Value::from("user2500"))
                .unwrap();
        })
    });

    group.bench_function("SQL SELECT ... WHERE username =", |b| {
        b.iter(|| {
            let _ = executor
                .execute("SELECT age FROM users WHERE username = 'user2500'")
                .unwrap();
        })
    });

    group.finish();
}

fn bench_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("SQL vs Direct: Update");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));

    let row_count = 1000;

    // Setup
    let db = Database::new("bench_db".to_string());
    let schema = Schema {
        columns: vec![
            ColumnDefinition {
                name: "username".into(),
                data_type: DataType::String,
                nullable: true,
            },
            ColumnDefinition {
                name: "age".into(),
                data_type: DataType::Integer,
                nullable: true,
            },
        ],
    };
    db.create_table("users".to_string(), schema);
    let mut ids = Vec::with_capacity(row_count);
    for i in 0..row_count {
        let mut data = RowData::default();
        data.insert("username".into(), Value::from(format!("user{}", i)));
        data.insert("age".into(), Value::from(i as i64));
        ids.push(db.insert_row("users", data, None).unwrap());
    }

    // Pre-populate id_map so direct update works (same as SQL executor does)
    {
        let table_lock = db.get_table("users").unwrap();
        let mut table = table_lock.write();
        if table.is_sequential_ids {
            table.is_sequential_ids = false;
            for (i, id) in table.ids.clone().into_iter().enumerate() {
                table.id_map.insert(id, i);
            }
        }
    }

    let executor = SqlExecutor::new(&db);

    // Direct API — read + update
    group.bench_function("Direct update_row", |b| {
        let mut i = 0;
        b.iter(|| {
            let id = &ids[i % ids.len()];
            let mut data = RowData::default();
            data.insert("age".into(), Value::from(99i64));
            let _ = db.update_row("users", id, data).unwrap();
            i += 1;
        })
    });

    // SQL API — UPDATE
    group.bench_function("SQL UPDATE ... SET ... WHERE id =", |b| {
        let mut i = 1;
        b.iter(|| {
            let sql = format!("UPDATE users SET age = 99 WHERE id = {}", i);
            let _ = executor.execute(&sql).unwrap();
            i = (i % row_count) + 1;
        })
    });

    group.finish();
}

fn bench_delete(c: &mut Criterion) {
    let mut group = c.benchmark_group("SQL vs Direct: Delete");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));

    // Direct API
    group.bench_function("Direct delete_row", |b| {
        let db = Database::new("bench_db".to_string());
        let schema = Schema {
            columns: vec![
                ColumnDefinition {
                    name: "username".into(),
                    data_type: DataType::String,
                    nullable: true,
                },
                ColumnDefinition {
                    name: "age".into(),
                    data_type: DataType::Integer,
                    nullable: true,
                },
            ],
        };
        db.create_table("users".to_string(), schema);
        for i in 0..100 {
            let mut data = RowData::default();
            data.insert("username".into(), Value::from(format!("user{}", i)));
            data.insert("age".into(), Value::from(i as i64));
            db.insert_row("users", data, None).unwrap();
        }

        // Pre-populate id_map for direct delete (SQL executor does this internally)
        {
            let table_lock = db.get_table("users").unwrap();
            let mut table = table_lock.write();
            if table.is_sequential_ids {
                table.is_sequential_ids = false;
                for (i, id) in table.ids.clone().into_iter().enumerate() {
                    table.id_map.insert(id, i);
                }
            }
        }

        b.iter_batched(
            || {
                let mut data = RowData::default();
                data.insert("username".into(), Value::from("user_new"));
                data.insert("age".into(), Value::from(99i64));
                let id = db.insert_row("users", data, None).unwrap();
                id
            },
            |id| {
                db.delete_row("users", &id).unwrap();
            },
            BatchSize::SmallInput,
        )
    });

    // SQL API
    group.bench_function("SQL DELETE ... WHERE id =", |b| {
        let db = Database::new("bench_db".to_string());
        let schema = Schema {
            columns: vec![
                ColumnDefinition {
                    name: "username".into(),
                    data_type: DataType::String,
                    nullable: true,
                },
                ColumnDefinition {
                    name: "age".into(),
                    data_type: DataType::Integer,
                    nullable: true,
                },
            ],
        };
        db.create_table("users".to_string(), schema);
        for i in 0..100 {
            let mut data = RowData::default();
            data.insert("username".into(), Value::from(format!("user{}", i)));
            data.insert("age".into(), Value::from(i as i64));
            db.insert_row("users", data, None).unwrap();
        }
        let executor = SqlExecutor::new(&db);

        b.iter_batched(
            || {
                let mut data = RowData::default();
                data.insert("username".into(), Value::from("user_new"));
                data.insert("age".into(), Value::from(99i64));
                let id = db.insert_row("users", data, None).unwrap();
                id
            },
            |id| {
                let sql = format!("DELETE FROM users WHERE id = {}", id);
                executor.execute(&sql).unwrap();
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

fn bench_join(c: &mut Criterion) {
    let mut group = c.benchmark_group("SQL vs Direct: Join");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));

    let row_count = 1000;

    // Setup
    let db = Database::new("bench_db".to_string());
    db.create_table(
        "users".to_string(),
        Schema {
            columns: vec![
                ColumnDefinition {
                    name: "id".into(),
                    data_type: DataType::Integer,
                    nullable: true,
                },
                ColumnDefinition {
                    name: "name".into(),
                    data_type: DataType::String,
                    nullable: true,
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
                    nullable: true,
                },
                ColumnDefinition {
                    name: "title".into(),
                    data_type: DataType::String,
                    nullable: true,
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

    let executor = SqlExecutor::new(&db);

    // Direct API
    group.bench_function("Direct hash_join", |b| {
        b.iter(|| {
            let _ = db.hash_join("users", "id", "posts", "user_id").unwrap();
        })
    });

    // SQL API
    group.bench_function("SQL SELECT ... JOIN ... ON", |b| {
        b.iter(|| {
            let sql = "SELECT * FROM users INNER JOIN posts ON users.id = posts.user_id";
            let _ = executor.execute(sql).unwrap();
        })
    });

    group.finish();
}

criterion_group!(
    name = sql_benches;
    config = Criterion::default().warm_up_time(Duration::from_secs(1));
    targets = bench_insert, bench_batch_insert, bench_read, bench_search, bench_update, bench_delete, bench_join
);
criterion_main!(sql_benches);
