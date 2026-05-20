use crate::protocol::{Request, Response};
use crate::transport::{decode_frame, encode_frame, Responder, Transport, TransportError};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

/// Framed TCP transport that uses a 4-byte LE length prefix + bincode payload.
/// Client side: send() writes a request frame and reads one response frame.
pub struct TcpClientTransport {
    stream: Mutex<TcpStream>,
}

impl TcpClientTransport {
    pub async fn connect(addr: &str) -> Result<Self, TransportError> {
        let stream = TcpStream::connect(addr).await?;
        Ok(Self {
            stream: Mutex::new(stream),
        })
    }
}

#[async_trait]
impl Transport for TcpClientTransport {
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

/// Responder that writes back on a shared stream
pub struct TcpResponder {
    stream: Arc<Mutex<TcpStream>>,
}

impl TcpResponder {
    pub fn new(stream: Arc<Mutex<TcpStream>>) -> Self {
        Self { stream }
    }
}

#[async_trait]
impl Responder for TcpResponder {
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

/// A single incoming TCP connection wrapped as a Transport (server side).
pub struct TcpConnectionTransport {
    stream: Arc<Mutex<TcpStream>>,
}

impl TcpConnectionTransport {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream: Arc::new(Mutex::new(stream)),
        }
    }
}

#[async_trait]
impl Transport for TcpConnectionTransport {
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
        let responder = Box::new(TcpResponder::new(self.stream.clone()));
        Ok((request, responder))
    }
}

/// TCP server that listens for connections and dispatches them to the backend
pub struct TcpServer {
    listener: TcpListener,
    backend: Arc<dyn crate::backend::Backend>,
}

impl TcpServer {
    pub async fn bind(
        addr: &str,
        backend: Arc<dyn crate::backend::Backend>,
    ) -> Result<Self, TransportError> {
        let listener = TcpListener::bind(addr).await?;
        eprintln!("TCP server listening on {}", addr);
        Ok(Self { listener, backend })
    }

    pub async fn serve(self) -> Result<(), TransportError> {
        loop {
            let (stream, addr) = self.listener.accept().await?;
            eprintln!("New TCP connection from {}", addr);
            let backend = self.backend.clone();
            tokio::spawn(async move {
                let transport = TcpConnectionTransport::new(stream);
                crate::transport::handle_client(transport, backend).await;
            });
        }
    }
}