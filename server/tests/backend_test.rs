use std::sync::Arc;

use dbobj::{Database, Value};
use dbobj_server::backend::{Backend, DbobjBackend};
use dbobj_server::protocol::{ColumnDef, ComparisonOp, ExprData, Request, Response};

fn setup_test_db() -> Arc<Database> {
    let db = Database::new("test".to_string());
    let schema = dbobj::Schema {
        columns: vec![
            dbobj::ColumnDefinition {
                name: "name".into(),
                data_type: dbobj::DataType::String,
                nullable: false,
            },
            dbobj::ColumnDefinition {
                name: "age".into(),
                data_type: dbobj::DataType::Integer,
                nullable: false,
            },
            dbobj::ColumnDefinition {
                name: "active".into(),
                data_type: dbobj::DataType::Boolean,
                nullable: true,
            },
        ],
    };
    db.create_table("users".to_string(), schema);
    Arc::new(db)
}

fn create_backend() -> DbobjBackend {
    let db = setup_test_db();
    DbobjBackend::new(db)
}

#[tokio::test]
async fn test_create_table_and_list() {
    let backend = create_backend();

    let resp = backend
        .execute(Request::CreateTable {
            name: "products".into(),
            columns: vec![ColumnDef {
                name: "title".into(),
                data_type: "String".into(),
                nullable: false,
            }],
        })
        .await;
    assert!(matches!(resp, Response::Ok(1)), "Expected Ok(1), got {:?}", resp);

    let resp = backend.execute(Request::ListTables).await;
    match resp {
        Response::TableList(tables) => {
            assert!(tables.contains(&"products".to_string()), "Tables should contain 'products'");
            assert!(tables.contains(&"users".to_string()), "Tables should contain 'users'");
        }
        other => panic!("Expected TableList, got {:?}", other),
    }
}

#[tokio::test]
async fn test_table_info() {
    let backend = create_backend();

    let resp = backend
        .execute(Request::TableInfo {
            name: "users".into(),
        })
        .await;
    match resp {
        Response::TableInfo {
            name,
            columns,
            row_count,
        } => {
            assert_eq!(name, "users");
            assert_eq!(columns.len(), 3);
            assert_eq!(columns[0].name, "name");
            assert_eq!(columns[0].data_type, "String");
            assert!(!columns[0].nullable);
            assert_eq!(columns[1].name, "age");
            assert_eq!(columns[1].data_type, "Integer");
            assert_eq!(columns[2].name, "active");
            assert_eq!(columns[2].data_type, "Boolean");
            assert!(columns[2].nullable);
            assert_eq!(row_count, 0);
        }
        other => panic!("Expected TableInfo, got {:?}", other),
    }

    // Non-existent table
    let resp = backend
        .execute(Request::TableInfo {
            name: "nonexistent".into(),
        })
        .await;
    assert!(matches!(resp, Response::Error(_)), "Expected Error, got {:?}", resp);
}

#[tokio::test]
async fn test_insert_and_query() {
    let backend = create_backend();

    // Insert a row using InsertValues
    let resp = backend
        .execute(Request::InsertValues {
            table: "users".into(),
            values: vec![Value::String("Alice".into()), Value::Integer(30), Value::Boolean(true)],
        })
        .await;
    let inserted_id = match &resp {
        Response::Id(id) => id.clone(),
        other => panic!("Expected Id, got {:?}", other),
    };

    // Query back
    let resp = backend
        .execute(Request::Query {
            table: "users".into(),
            column_name: "name".into(),
            value: Value::String("Alice".into()),
        })
        .await;
    match resp {
        Response::Rows(rows) => {
            assert_eq!(rows.len(), 1, "Should find Alice");
            assert_eq!(rows[0].id, inserted_id);
            assert_eq!(rows[0].data.len(), 3);
        }
        other => panic!("Expected Rows, got {:?}", other),
    }
}

#[tokio::test]
async fn test_insert_batch() {
    let backend = create_backend();

    let batch = vec![
        vec![Value::String("Bob".into()), Value::Integer(25), Value::Boolean(true)],
        vec![Value::String("Carol".into()), Value::Integer(35), Value::Boolean(false)],
    ];

    let resp = backend
        .execute(Request::InsertBatchValues {
            table: "users".into(),
            batch,
        })
        .await;
    match resp {
        Response::Ids(ids) => {
            assert_eq!(ids.len(), 2, "Should return 2 IDs");
        }
        other => panic!("Expected Ids, got {:?}", other),
    }

    // Query - should have 2 rows
    let resp = backend
        .execute(Request::TableInfo {
            name: "users".into(),
        })
        .await;
    match resp {
        Response::TableInfo { row_count, .. } => {
            assert_eq!(row_count, 2, "Should have 2 rows");
        }
        other => panic!("Expected TableInfo, got {:?}", other),
    }
}

#[tokio::test]
async fn test_update_row() {
    let backend = create_backend();

    // Insert
    let resp = backend
        .execute(Request::InsertValues {
            table: "users".into(),
            values: vec![Value::String("Dave".into()), Value::Integer(40), Value::Boolean(true)],
        })
        .await;
    let id = match &resp {
        Response::Id(id) => id.clone(),
        other => panic!("Expected Id, got {:?}", other),
    };

    // Update by indices
    let resp = backend
        .execute(Request::UpdateByIndices {
            table: "users".into(),
            id: id.clone(),
            updates: vec![(1, Value::Integer(41))], // change age to 41
        })
        .await;
    assert!(matches!(resp, Response::Ok(1)), "Expected Ok(1), got {:?}", resp);

    // Verify
    let resp = backend
        .execute(Request::Query {
            table: "users".into(),
            column_name: "name".into(),
            value: Value::String("Dave".into()),
        })
        .await;
    match resp {
        Response::Rows(rows) => {
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].id, id);
        }
        other => panic!("Expected Rows, got {:?}", other),
    }
}

#[tokio::test]
async fn test_delete_row() {
    let backend = create_backend();

    // Insert
    let resp = backend
        .execute(Request::InsertValues {
            table: "users".into(),
            values: vec![
                Value::String("Eve".into()),
                Value::Integer(28),
                Value::Boolean(true),
            ],
        })
        .await;
    let id = match &resp {
        Response::Id(id) => id.clone(),
        other => panic!("Expected Id, got {:?}", other),
    };

    // Delete
    let resp = backend
        .execute(Request::DeleteRow {
            table: "users".into(),
            id,
        })
        .await;
    assert!(matches!(resp, Response::Ok(1)), "Expected Ok(1), got {:?}", resp);

    // Verify deleted
    let resp = backend
        .execute(Request::TableInfo {
            name: "users".into(),
        })
        .await;
    match resp {
        Response::TableInfo { row_count, .. } => {
            assert_eq!(row_count, 0, "Row count should be 0 after delete");
        }
        other => panic!("Expected TableInfo, got {:?}", other),
    }
}

#[tokio::test]
async fn test_query_predicate() {
    let backend = create_backend();

    // Insert test data
    let values = vec![
        vec![
            Value::String("Alice".into()),
            Value::Integer(30),
            Value::Boolean(true),
        ],
        vec![
            Value::String("Bob".into()),
            Value::Integer(25),
            Value::Boolean(false),
        ],
        vec![
            Value::String("Carol".into()),
            Value::Integer(35),
            Value::Boolean(true),
        ],
    ];

    backend
        .execute(Request::InsertBatchValues {
            table: "users".into(),
            batch: values,
        })
        .await;

    // Query age > 28 (column index 1 = age)
    let resp = backend
        .execute(Request::QueryPredicate {
            table: "users".into(),
            column_idx: 1,
            operator: ComparisonOp::Gt,
            value: Value::Integer(28),
        })
        .await;
    match resp {
        Response::Rows(rows) => {
            assert_eq!(rows.len(), 2, "Expected 2 rows with age > 28 (Alice 30, Carol 35)");
        }
        other => panic!("Expected Rows, got {:?}", other),
    }
}

#[tokio::test]
async fn test_query_expr() {
    let backend = create_backend();

    // Insert test data
    let values = vec![
        vec![
            Value::String("Alice".into()),
            Value::Integer(30),
            Value::Boolean(true),
        ],
        vec![
            Value::String("Bob".into()),
            Value::Integer(25),
            Value::Boolean(false),
        ],
        vec![
            Value::String("Carol".into()),
            Value::Integer(35),
            Value::Boolean(true),
        ],
    ];

    backend
        .execute(Request::InsertBatchValues {
            table: "users".into(),
            batch: values,
        })
        .await;

    // Alice is active (active == true) AND age > 28
    let expr = ExprData::And(vec![
        ExprData::Binary {
            left: Box::new(ExprData::Column("active".into())),
            op: ComparisonOp::Eq,
            right: Box::new(ExprData::Literal(Value::Boolean(true))),
        },
        ExprData::Binary {
            left: Box::new(ExprData::Column("age".into())),
            op: ComparisonOp::Gt,
            right: Box::new(ExprData::Literal(Value::Integer(28))),
        },
    ]);

    let resp = backend
        .execute(Request::QueryExpr {
            table: "users".into(),
            expr,
        })
        .await;
    match resp {
        Response::Rows(rows) => {
            assert_eq!(
                rows.len(),
                2,
                "Expected 2 rows (Alice 30 active, Carol 35 active)"
            );
        }
        other => panic!("Expected Rows, got {:?}", other),
    }
}

#[tokio::test]
async fn test_drop_table() {
    let backend = create_backend();

    let resp = backend
        .execute(Request::DropTable {
            name: "users".into(),
        })
        .await;
    assert!(matches!(resp, Response::Ok(1)), "Expected Ok(1), got {:?}", resp);

    // Verify gone
    let resp = backend.execute(Request::ListTables).await;
    match resp {
        Response::TableList(tables) => {
            assert!(!tables.contains(&"users".to_string()));
        }
        other => panic!("Expected TableList, got {:?}", other),
    }
}

#[tokio::test]
async fn test_ping() {
    let backend = create_backend();
    let resp = backend.execute(Request::Ping).await;
    assert!(matches!(resp, Response::Pong), "Expected Pong, got {:?}", resp);
}

#[tokio::test]
async fn test_drop_nonexistent_table() {
    let backend = create_backend();
    let resp = backend
        .execute(Request::DropTable {
            name: "nonexistent".into(),
        })
        .await;
    assert!(matches!(resp, Response::Error(_)), "Expected Error, got {:?}", resp);
}

#[tokio::test]
async fn test_delete_batch() {
    let backend = create_backend();

    // Insert multiple rows
    let batch = vec![
        vec![Value::String("A".into()), Value::Integer(1), Value::Boolean(true)],
        vec![Value::String("B".into()), Value::Integer(2), Value::Boolean(true)],
        vec![Value::String("C".into()), Value::Integer(3), Value::Boolean(false)],
    ];

    let resp = backend
        .execute(Request::InsertBatchValues {
            table: "users".into(),
            batch,
        })
        .await;
    let ids = match resp {
        Response::Ids(ids) => ids,
        other => panic!("Expected Ids, got {:?}", other),
    };
    assert_eq!(ids.len(), 3);

    // Delete first two
    let resp = backend
        .execute(Request::DeleteBatch {
            table: "users".into(),
            ids: ids[0..2].to_vec(),
        })
        .await;
    assert!(matches!(resp, Response::Ok(2)) || matches!(resp, Response::Ok(1)));
}

#[tokio::test]
async fn test_insert_or_replace() {
    let backend = create_backend();

    // First insert
    let resp = backend
        .execute(Request::InsertValues {
            table: "users".into(),
            values: vec![
                Value::String("Original".into()),
                Value::Integer(10),
                Value::Boolean(true),
            ],
        })
        .await;
    let _first_id = match &resp {
        Response::Id(id) => id.clone(),
        other => panic!("Expected Id, got {:?}", other),
    };

    // Replace by name
    let resp = backend
        .execute(Request::InsertOrReplace {
            table: "users".into(),
            values: vec![Value::String("Replaced".into()), Value::Integer(99), Value::Boolean(false)],
            unique_column: "name".into(),
        })
        .await;
    // Should succeed (returns an id)
    assert!(matches!(resp, Response::Id(_)));
}