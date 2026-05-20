use crate::protocol::{Request, Response};
use async_trait::async_trait;
use std::sync::Arc;

/// A responder sends a response back to a specific client.
/// The transport implementation provides concrete responders.
#[async_trait]
pub trait Responder: Send + 'static {
    async fn respond(self: Box<Self>, resp: Response) -> Result<(), TransportError>;
}

/// Abstract transport: a server-side handle for receiving requests
/// and a client-side handle for sending requests.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Client side: send a request and await the response.
    async fn send(&self, req: Request) -> Result<Response, TransportError>;

    /// Server side: receive the next request along with a responder.
    async fn recv(&self) -> Result<(Request, Box<dyn Responder>), TransportError>;
}

/// Errors that can occur during transport operations
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Channel closed")]
    ChannelClosed,

    #[error("Timeout")]
    Timeout,
}

impl From<bincode::Error> for TransportError {
    fn from(e: bincode::Error) -> Self {
        TransportError::Serialization(e.to_string())
    }
}

/// Helper: serialize a value to a length-delimited binary frame
pub fn encode_frame<T: serde::Serialize>(msg: &T) -> Result<Vec<u8>, TransportError> {
    bincode::serialize(msg).map_err(|e| TransportError::Serialization(e.to_string()))
}

/// Helper: deserialize a value from binary data
pub fn decode_frame<T: serde::de::DeserializeOwned>(data: &[u8]) -> Result<T, TransportError> {
    bincode::deserialize(data).map_err(|e| TransportError::Serialization(e.to_string()))
}

/// Generic function to handle a client connection via any transport.
/// The transport's recv() is called in a loop, each request is dispatched
/// to the backend, and the response is sent back via the responder.
pub async fn handle_client(transport: impl Transport, backend: Arc<dyn crate::backend::Backend>) {
    loop {
        let (req, responder) = match transport.recv().await {
            Ok(r) => r,
            Err(TransportError::ConnectionClosed)
            | Err(TransportError::ChannelClosed)
            | Err(TransportError::Io(_)) => break,
            Err(e) => {
                eprintln!("Error receiving request: {e}");
                break;
            }
        };

        let resp = backend.execute(req).await;
        if let Err(e) = responder.respond(resp).await {
            eprintln!("Error sending response: {e}");
            break;
        }
    }
}
