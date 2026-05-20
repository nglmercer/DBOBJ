use crate::protocol::{Request, Response};
use crate::transport::{Responder, Transport, TransportError};
use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

/// In-process channel transport: client and server communicate via
/// tokio channels with zero serialization overhead.
///
/// Server side: listens for (Request, oneshot::Sender<Response>) pairs,
/// processes them, and sends the response back via the oneshot channel.
///
/// Client side: sends a (Request, oneshot::Sender<Response>) and awaits
/// the response.
type ChannelMessage = (Request, oneshot::Sender<Response>);

/// Server-side transport that receives requests
#[allow(dead_code)]
pub struct ServerChannelTransport {
    rx: mpsc::Receiver<ChannelMessage>,
}

impl ServerChannelTransport {
    pub fn new(rx: mpsc::Receiver<ChannelMessage>) -> Self {
        Self { rx }
    }
}

/// Server-side responder: sends the response via a oneshot channel
#[allow(dead_code)]
struct ChannelResponder {
    tx: Option<oneshot::Sender<Response>>,
}

#[async_trait]
impl Responder for ChannelResponder {
    async fn respond(mut self: Box<Self>, resp: Response) -> Result<(), TransportError> {
        if let Some(tx) = self.tx.take() {
            tx.send(resp).map_err(|_| TransportError::ChannelClosed)?;
        }
        Ok(())
    }
}

#[async_trait]
impl Transport for ServerChannelTransport {
    async fn send(&self, _req: Request) -> Result<Response, TransportError> {
        Err(TransportError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "send not supported on server channel transport",
        )))
    }

    async fn recv(&self) -> Result<(Request, Box<dyn Responder>), TransportError> {
        // We need a mutex around the receiver since recv takes &mut self.
        // Since Transport::recv takes &self, we use the concrete ServerChannelTransport
        // which wraps the receiver internally. But mpsc::Receiver requires &mut.
        // We'll handle this at a higher level via the server loop.
        Err(TransportError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "recv must be called via ChannelServer::serve, not directly",
        )))
    }
}

/// Client-side transport that sends requests via a channel
#[derive(Clone)]
pub struct ClientChannelTransport {
    tx: mpsc::Sender<ChannelMessage>,
}

impl ClientChannelTransport {
    pub fn new(tx: mpsc::Sender<ChannelMessage>) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl Transport for ClientChannelTransport {
    async fn send(&self, req: Request) -> Result<Response, TransportError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send((req, tx))
            .await
            .map_err(|_| TransportError::ChannelClosed)?;
        rx.await.map_err(|_| TransportError::ChannelClosed)
    }

    async fn recv(&self) -> Result<(Request, Box<dyn Responder>), TransportError> {
        Err(TransportError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "recv not supported on client channel transport",
        )))
    }
}

/// Creates a (client transport, server channel) pair.
/// The server must call `serve_loop` on the ServerChannel to process requests.
pub fn channel_pair(buffer: usize) -> (ClientChannelTransport, ServerChannelTransport) {
    let (tx, rx) = mpsc::channel(buffer);
    let client = ClientChannelTransport::new(tx);
    let server = ServerChannelTransport::new(rx);
    (client, server)
}

/// A channel-based server that processes requests in a loop.
pub struct ChannelServer {
    backend: std::sync::Arc<dyn crate::backend::Backend>,
    rx: tokio::sync::Mutex<mpsc::Receiver<ChannelMessage>>,
}

impl ChannelServer {
    pub fn new(
        backend: std::sync::Arc<dyn crate::backend::Backend>,
        rx: mpsc::Receiver<ChannelMessage>,
    ) -> Self {
        Self {
            backend,
            rx: tokio::sync::Mutex::new(rx),
        }
    }

    pub async fn serve(self) -> Result<(), TransportError> {
        loop {
            let msg = {
                let mut rx = self.rx.lock().await;
                rx.recv().await
            };

            match msg {
                Some((req, tx)) => {
                    let resp = self.backend.execute(req).await;
                    let _ = tx.send(resp);
                }
                None => break,
            }
        }
        Ok(())
    }
}
