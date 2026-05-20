use std::sync::Arc;

use dbobj::{Database, Value};
use dbobj_server::backend::{Backend, DbobjBackend};
use dbobj_server::channel::{ChannelServer, ClientChannelTransport};
use dbobj_server::protocol::{ColumnDef, Request, Response};
use dbobj_server::transport::Transport;

fn setup_backend() -> Arc<dyn Backend> {
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
        ],
    };
    db.create_table("users".to_string(), schema);
    Arc::new(DbobjBackend::new(Arc::new(db)))
}

#[tokio::test]
async fn test_channel_transport_full_cycle() {
    let backend = setup_backend();
    let (tx, rx) = tokio::sync::mpsc::channel(64);
    let client = ClientChannelTransport::new(tx);
    let server = ChannelServer::new(backend, rx);

    // Spawn server in background
    tokio::spawn(async move {
        server.serve().await.expect("Channel server failed");
    });

    // Give server time to start
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // 1. Create table
    let resp = client
        .send(Request::CreateTable {
            name: "products".into(),
            columns: vec![ColumnDef {
                name: "title".into(),
                data_type: "String".into(),
                nullable: false,
            }],
        })
        .await
        .expect("Send failed");
    assert!(
        matches!(resp, Response::Ok(1)),
        "Expected Ok(1), got {:?}",
        resp
    );

    // 2. Insert data
    let resp = client
        .send(Request::InsertValues {
            table: "users".into(),
            values: vec![Value::String("Alice".into()), Value::Integer(30)],
        })
        .await
        .expect("Send failed");
    let _user_id = match resp {
        Response::Id(id) => id,
        other => panic!("Expected Id, got {:?}", other),
    };

    // 3. List tables
    let resp = client.send(Request::ListTables).await.expect("Send failed");
    match resp {
        Response::TableList(tables) => {
            assert!(tables.contains(&"users".to_string()));
            assert!(tables.contains(&"products".to_string()));
        }
        other => panic!("Expected TableList, got {:?}", other),
    }

    // 4. Ping (heartbeat)
    let resp = client.send(Request::Ping).await.expect("Send failed");
    assert!(matches!(resp, Response::Pong));

    // 5. Drop table
    let resp = client
        .send(Request::DropTable {
            name: "products".into(),
        })
        .await
        .expect("Send failed");
    assert!(matches!(resp, Response::Ok(1)));

    // 6. Error case: query nonexistent table
    let resp = client
        .send(Request::TableInfo {
            name: "nonexistent".into(),
        })
        .await
        .expect("Send failed");
    assert!(matches!(resp, Response::Error(_)));
}

#[tokio::test]
async fn test_channel_concurrent_requests() {
    let backend = setup_backend();
    let (tx, rx) = tokio::sync::mpsc::channel(64);
    let client = ClientChannelTransport::new(tx);
    let server = ChannelServer::new(backend, rx);

    // Spawn server
    tokio::spawn(async move {
        server.serve().await.expect("Channel server failed");
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Send multiple concurrent requests
    let mut handles = Vec::new();
    for i in 0..10 {
        let client = client.clone();
        handles.push(tokio::spawn(async move {
            let name: String = format!("User-{}", i);
            let resp = client
                .send(Request::InsertValues {
                    table: "users".into(),
                    values: vec![Value::String(name.into()), Value::Integer(i)],
                })
                .await
                .expect("Concurrent send failed");
            assert!(
                matches!(resp, Response::Id(_)),
                "Expected Id, got {:?}",
                resp
            );
        }));
    }

    // Wait for all concurrent inserts
    for handle in handles {
        handle.await.expect("Concurrent task failed");
    }

    // Verify all 10 rows + 1 initial (Alice) from previous test DB state? No — fresh DB per test.
    let resp = client
        .send(Request::TableInfo {
            name: "users".into(),
        })
        .await
        .expect("Send failed");
    match resp {
        Response::TableInfo { row_count, .. } => {
            // 10 concurrent inserts into a fresh DB
            assert_eq!(row_count, 10, "Should have 10 rows from concurrent inserts");
        }
        other => panic!("Expected TableInfo, got {:?}", other),
    }
}

#[tokio::test]
async fn test_channel_transport_channel_closed() {
    // Create channel with specific types
    let (tx, rx) = tokio::sync::mpsc::channel::<(
        dbobj_server::protocol::Request,
        tokio::sync::oneshot::Sender<dbobj_server::protocol::Response>,
    )>(1);
    let client = ClientChannelTransport::new(tx);

    // Drop the receiver immediately (simulates server crash)
    drop(rx);

    // Sending should fail with ChannelClosed error
    let result = client.send(Request::Ping).await;
    assert!(result.is_err(), "Expected error when channel is closed");
}

#[tokio::test]
async fn test_in_process_roundtrip() {
    let backend = setup_backend();
    let (tx, rx) = tokio::sync::mpsc::channel(64);
    let client = ClientChannelTransport::new(tx);
    let server = ChannelServer::new(backend, rx);

    tokio::spawn(async move {
        server.serve().await.ok();
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Insert Alice
    let resp = client
        .send(Request::InsertValues {
            table: "users".into(),
            values: vec![Value::String("Alice".into()), Value::Integer(30)],
        })
        .await
        .expect("Send failed");
    let alice_id = match resp {
        Response::Id(id) => id,
        other => panic!("Expected Id, got {:?}", other),
    };

    // Insert Bob
    let resp = client
        .send(Request::InsertValues {
            table: "users".into(),
            values: vec![Value::String("Bob".into()), Value::Integer(25)],
        })
        .await
        .expect("Send failed");
    let bob_id = match resp {
        Response::Id(id) => id,
        other => panic!("Expected Id, got {:?}", other),
    };

    assert_ne!(alice_id, bob_id, "IDs should be unique");
}
