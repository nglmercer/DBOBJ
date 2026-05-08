use dbobj::core::{ColumnDefinition, DataType, Database, Expr, Operator, RowData, Schema, Value};

#[test]
fn test_database_crud_integrity() {
    let db = Database::new("IntegrityDB".to_string());

    // Create Table
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
            ColumnDefinition {
                name: "age".into(),
                data_type: DataType::Integer,
                nullable: true,
            },
        ],
    };
    db.create_table("users".to_string(), schema);

    // Insert
    let mut row1 = RowData::default();
    row1.insert("id".into(), Value::from(1));
    row1.insert("name".into(), Value::from("Alice"));
    row1.insert("age".into(), Value::from(30));
    let id1 = db.insert_row("users", row1, None).unwrap();

    let mut row2 = RowData::default();
    row2.insert("id".into(), Value::from(2));
    row2.insert("name".into(), Value::from("Bob"));
    row2.insert("age".into(), Value::Null);
    let id2 = db.insert_row("users", row2, None).unwrap();

    // Read
    let table = db.get_table("users").unwrap();
    let table_read = table.read();
    assert_eq!(table_read.ids.len(), 2);

    let alice_row = table_read.get(&id1).unwrap();
    let alice = table_read.values_to_row(&alice_row.data);
    assert_eq!(alice.get("name").unwrap(), &Value::from("Alice"));

    let bob_row = table_read.get(&id2).unwrap();
    let bob = table_read.values_to_row(&bob_row.data);
    assert_eq!(bob.get("age").unwrap(), &Value::Null);
}

#[test]
fn test_complex_query_integrity() {
    let db = Database::new("QueryDB".to_string());
    let schema = Schema {
        columns: vec![ColumnDefinition {
            name: "score".into(),
            data_type: DataType::Integer,
            nullable: false,
        }],
    };
    db.create_table("scores".to_string(), schema);

    for i in 0..100 {
        let mut row = RowData::default();
        row.insert("score".into(), Value::from(i as i64));
        db.insert_row("scores", row, None).unwrap();
    }

    // Query: score > 90
    let expr = Expr::Binary(
        Box::new(Expr::Column("score".into())),
        Operator::Gt,
        Box::new(Expr::Literal(Value::from(90))),
    );

    let results = db.query_expr("scores", expr).unwrap();
    assert_eq!(results.len(), 9); // 91, 92, ..., 99
}

#[test]
fn test_join_integrity() {
    let db = Database::new("JoinDB".to_string());

    db.create_table(
        "users".to_string(),
        Schema {
            columns: vec![
                ColumnDefinition {
                    name: "uid".into(),
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

    let _u1 = db
        .insert_row(
            "users",
            {
                let mut r = RowData::default();
                r.insert("uid".into(), Value::from(1));
                r.insert("name".into(), Value::from("Alice"));
                r
            },
            None,
        )
        .unwrap();

    db.insert_row(
        "posts",
        {
            let mut r = RowData::default();
            r.insert("user_id".into(), Value::from(1));
            r.insert("title".into(), Value::from("Post 1"));
            r
        },
        None,
    )
    .unwrap();

    let joined = db.hash_join("users", "uid", "posts", "user_id").unwrap();
    assert_eq!(joined.len(), 1);
}

#[test]
fn test_persistence_integrity() {
    let db = Database::new("PersistDB".to_string());
    db.create_table(
        "test".to_string(),
        Schema {
            columns: vec![ColumnDefinition {
                name: "val".into(),
                data_type: DataType::Integer,
                nullable: false,
            }],
        },
    );

    db.insert_row(
        "test",
        {
            let mut r = RowData::default();
            r.insert("val".into(), Value::from(42));
            r
        },
        None,
    )
    .unwrap();

    use dbobj::storage::{BitcodeAdapter, Storage};
    let path = "test_integrity.db";
    let storage = Storage::new(path, BitcodeAdapter);

    storage.save(&db).unwrap();
    let loaded_db = storage.load().unwrap();

    assert_eq!(loaded_db.name, "PersistDB");
    let table = loaded_db.get_table("test").unwrap();
    assert_eq!(table.read().ids.len(), 1);

    std::fs::remove_file(path).ok();
}

#[test]
fn test_direct_batch_insert_integrity() {
    let db = Database::new("BatchDB".to_string());
    let schema = Schema {
        columns: vec![
            ColumnDefinition {
                name: "name".into(),
                data_type: DataType::String,
                nullable: false,
            },
            ColumnDefinition {
                name: "age".into(),
                data_type: DataType::Integer,
                nullable: true,
            },
        ],
    };
    db.create_table("users".to_string(), schema);

    let mut batch = Vec::with_capacity(100);
    for i in 0..100 {
        batch.push(vec![
            Value::from(format!("user_{}", i)),
            Value::from(i as i64),
        ]);
    }
    db.insert_batch_values("users", batch).unwrap();

    let table = db.get_table("users").unwrap();
    let table_read = table.read();
    assert_eq!(table_read.ids.len(), 100);

    // Verify a few rows
    for i in [0, 50, 99] {
        let row = table_read.get(&(i as u64).into()).unwrap();
        let data = table_read.values_to_row(&row.data);
        assert_eq!(
            data.get("name").unwrap(),
            &Value::from(format!("user_{}", i))
        );
        assert_eq!(data.get("age").unwrap(), &Value::from(i as i64));
    }
}

#[test]
fn test_direct_update_integrity() {
    let db = Database::new("UpdateDB".to_string());
    let schema = Schema {
        columns: vec![
            ColumnDefinition {
                name: "name".into(),
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

    let id = db
        .insert_row(
            "users",
            {
                let mut r = RowData::default();
                r.insert("name".into(), Value::from("Alice"));
                r.insert("age".into(), Value::from(30));
                r
            },
            None,
        )
        .unwrap();

    // Pre-populate id_map so direct update works
    {
        let table_lock = db.get_table("users").unwrap();
        let mut table = table_lock.write();
        if table.is_sequential_ids {
            table.is_sequential_ids = false;
            for (i, existing_id) in table.ids.clone().into_iter().enumerate() {
                table.id_map.insert(existing_id, i);
            }
        }
    }

    // Update age — must include all non-nullable columns
    let mut new_data = RowData::default();
    new_data.insert("name".into(), Value::from("Alice"));
    new_data.insert("age".into(), Value::from(31));
    db.update_row("users", &id, new_data).unwrap();

    let table = db.get_table("users").unwrap();
    let row = table.read().get(&id).unwrap();
    let data = table.read().values_to_row(&row.data);
    assert_eq!(data.get("age").unwrap(), &Value::from(31));
    assert_eq!(data.get("name").unwrap(), &Value::from("Alice"));
}

#[test]
fn test_direct_delete_integrity() {
    let db = Database::new("DeleteDB".to_string());
    let schema = Schema {
        columns: vec![ColumnDefinition {
            name: "name".into(),
            data_type: DataType::String,
            nullable: false,
        }],
    };
    db.create_table("users".to_string(), schema);

    let id = db
        .insert_row(
            "users",
            {
                let mut r = RowData::default();
                r.insert("name".into(), Value::from("Alice"));
                r
            },
            None,
        )
        .unwrap();

    // Pre-populate id_map so direct delete works
    {
        let table_lock = db.get_table("users").unwrap();
        let mut table = table_lock.write();
        if table.is_sequential_ids {
            table.is_sequential_ids = false;
            for (i, existing_id) in table.ids.clone().into_iter().enumerate() {
                table.id_map.insert(existing_id, i);
            }
        }
    }

    db.delete_row("users", &id).unwrap();
    assert_eq!(db.get_table("users").unwrap().read().ids.len(), 0);
}

#[test]
fn test_index_operations_integrity() {
    let db = Database::new("IndexDB".to_string());
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

    for i in 0..100 {
        let mut r = RowData::default();
        r.insert("username".into(), Value::from(format!("user{}", i)));
        r.insert("age".into(), Value::from(i as i64));
        db.insert_row("users", r, None).unwrap();
    }

    // Scan search before index (O(N))
    let table = db.get_table("users").unwrap();
    let table_ref = table.read();

    let results = db.find("users", "age", Value::from(50)).unwrap();
    assert_eq!(results.len(), 1);
    let row_data = table_ref.values_to_row(&results[0].data);
    assert_eq!(row_data.get("username").unwrap(), &Value::from("user50"));

    drop(table_ref);

    // Create index and search again (O(log N))
    db.create_index("users", "username").unwrap();
    let results = db.find("users", "username", Value::from("user75")).unwrap();
    assert_eq!(results.len(), 1);
    let table_ref = table.read();
    let row_data = table_ref.values_to_row(&results[0].data);
    assert_eq!(row_data.get("age").unwrap(), &Value::from(75));

    // Non-existent value returns empty
    let results = db
        .find("users", "username", Value::from("nonexistent"))
        .unwrap();
    assert!(results.is_empty());
}
