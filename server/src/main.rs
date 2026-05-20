use std::sync::Arc;

use dbobj::{
    storage::{BitcodeAdapter, Storage, StorageError},
    Database,
};
use dbobj_server::backend::DbobjBackend;
use dbobj_server::channel;
use dbobj_server::tcp::TcpServer;
use dbobj_server::uds::UdsServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command-line arguments
    let args: Vec<String> = std::env::args().collect();
    let transport = std::env::var("TRANSPORT").unwrap_or_else(|_| "tcp".to_string());
    let db_path = if args.len() > 1 {
        args[1].clone()
    } else {
        "my_database.db".to_string()
    };

    eprintln!("DBOBJ Server starting...");
    eprintln!("  Database path: {}", db_path);
    eprintln!("  Transport:     {}", transport);

    // Load or create database
    let db: Arc<Database> = {
        let storage = Storage::new(&db_path, BitcodeAdapter);
        match storage.load() {
            Ok(database) => {
                eprintln!("  Loaded existing database: {}", database.name);
                Arc::new(database)
            }
            Err(StorageError::Io(io_err)) if io_err.kind() == std::io::ErrorKind::NotFound => {
                let database = Database::new("DBOBJ".to_string());
                eprintln!("  Created new database: {}", database.name);
                Arc::new(database)
            }
            Err(e) => {
                return Err(format!("Failed to load database: {}", e).into());
            }
        }
    };

    // Clone before moving into closures
    let db_auto_save = db.clone();
    let db_shutdown = db.clone();
    let db_path_save = db_path.clone();

    // Auto-save periodically (every 30s)
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            let storage = Storage::new(&db_path_save, BitcodeAdapter);
            if let Err(e) = storage.save(&db_auto_save) {
                eprintln!("Auto-save error: {}", e);
            } else {
                eprintln!("Auto-saved database to {}", db_path_save);
            }
        }
    });

    // Wrap in backend
    let backend: Arc<dyn dbobj_server::backend::Backend> = Arc::new(DbobjBackend::new(db));

    // Pick transport at startup
    match transport.as_str() {
        "tcp" => {
            let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:9876".to_string());
            eprintln!("  Binding TCP on {}", addr);
            TcpServer::bind(&addr, backend).await?.serve().await?;
        }
        "uds" => {
            let path = std::env::var("SOCK_PATH").unwrap_or_else(|_| "/tmp/dbobj.sock".to_string());
            eprintln!("  Binding UDS on {}", path);
            UdsServer::bind(&path, backend).await?.serve().await?;
        }
        "channel" => {
            eprintln!("  Using in-process channel transport");
            let (client_tx, server_rx) = tokio::sync::mpsc::channel(1024);
            let _client_transport = channel::ClientChannelTransport::new(client_tx);
            let server = channel::ChannelServer::new(backend, server_rx);

            tokio::spawn(async move {
                if let Err(e) = server.serve().await {
                    eprintln!("Channel server error: {}", e);
                }
            });

            eprintln!("Channel server running. Press Ctrl+C to stop.");
            tokio::signal::ctrl_c().await?;
            eprintln!("Shutting down...");
        }
        other => {
            return Err(format!("Unknown transport '{}'. Use tcp, uds, or channel.", other).into());
        }
    }

    // Save on shutdown
    let storage = Storage::new(&db_path, BitcodeAdapter);
    storage.save(&db_shutdown)?;
    eprintln!("Database saved on shutdown.");

    Ok(())
}
