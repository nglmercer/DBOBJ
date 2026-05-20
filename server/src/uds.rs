use crate::protocol::{Request, Response};
use crate::transport::{decode_frame, encode_frame, Responder, Transport, TransportError};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;

/// Unix Domain Socket client transport.
pub struct UdsClientTransport {
    stream: Mutex<UnixStream>,
}

impl UdsClientTransport {
    pub async fn connect(path: &str) -> Result<Self, TransportError> {
        let stream = UnixStream::connect(path).await?;
        Ok(Self {
            stream: Mutex::new(stream),
        })
    }
}

#[async_trait]
impl Transport for UdsClientTransport {
    async fn send(&self, req: Request) -> Result<Response, TransportError> {
        let mut stream = self.stream.lock().await;
        let frame = encode_frame(&req)?;
        let len = frame.len() as u32;
        stream.write_all(&len.to_le_bytes()).await?;
        stream.write_all(&frame).await?;
        stream.flush().await?;

        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await?;
        let resp_len = u32::from_le_bytes(len_buf) as usize;
        let mut resp_buf = vec![0u8; resp_len];
        stream.read_exact(&mut resp_buf).await?;

        decode_frame(&resp_buf)
    }

    async fn recv(&self) -> Result<(Request, Box<dyn Responder>), TransportError> {
        Err(TransportError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "recv not supported on client transport",
        )))
    }
}

/// Responder that writes back on a shared UDS stream
pub struct UdsResponder {
    stream: Arc<Mutex<UnixStream>>,
}

impl UdsResponder {
    pub fn new(stream: Arc<Mutex<UnixStream>>) -> Self {
        Self { stream }
    }
}

#[async_trait]
impl Responder for UdsResponder {
    async fn respond(self: Box<Self>, resp: Response) -> Result<(), TransportError> {
        let mut stream = self.stream.lock().await;
        let frame = encode_frame(&resp)?;
        let len = frame.len() as u32;
        stream.write_all(&len.to_le_bytes()).await?;
        stream.write_all(&frame).await?;
        stream.flush().await?;
        Ok(())
    }
}

/// A single incoming UDS connection wrapped as a Transport (server side).
pub struct UdsConnectionTransport {
    stream: Arc<Mutex<UnixStream>>,
}

impl UdsConnectionTransport {
    pub fn new(stream: UnixStream) -> Self {
        Self {
            stream: Arc::new(Mutex::new(stream)),
        }
    }
}

#[async_trait]
impl Transport for UdsConnectionTransport {
    async fn send(&self, _req: Request) -> Result<Response, TransportError> {
        Err(TransportError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "send not supported on server connection transport",
        )))
    }

    async fn recv(&self) -> Result<(Request, Box<dyn Responder>), TransportError> {
        let request = {
            let mut stream = self.stream.lock().await;
            let mut len_buf = [0u8; 4];
            stream.read_exact(&mut len_buf).await?;
            let req_len = u32::from_le_bytes(len_buf) as usize;
            let mut req_buf = vec![0u8; req_len];
            stream.read_exact(&mut req_buf).await?;
            decode_frame(&req_buf)?
        };

        // Share the same stream for the responder
        let responder = Box::new(UdsResponder::new(self.stream.clone()));
        Ok((request, responder))
    }
}

/// Unix Domain Socket server
pub struct UdsServer {
    listener: UnixListener,
    backend: Arc<dyn crate::backend::Backend>,
}

impl UdsServer {
    pub async fn bind(
        path: &str,
        backend: Arc<dyn crate::backend::Backend>,
    ) -> Result<Self, TransportError> {
        // Remove existing socket file if present
        if std::path::Path::new(path).exists() {
            std::fs::remove_file(path)?;
        }
        let listener = UnixListener::bind(path)?;
        eprintln!("UDS server listening on {}", path);
        Ok(Self { listener, backend })
    }

    pub async fn serve(self) -> Result<(), TransportError> {
        loop {
            let (stream, _addr) = self.listener.accept().await?;
            eprintln!("New UDS connection");
            let backend = self.backend.clone();
            tokio::spawn(async move {
                let transport = UdsConnectionTransport::new(stream);
                crate::transport::handle_client(transport, backend).await;
            });
        }
    }
}
