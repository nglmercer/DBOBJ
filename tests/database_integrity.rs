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
