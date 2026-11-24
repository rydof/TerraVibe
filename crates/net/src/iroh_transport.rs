//! `iroh` QUIC transport bridging async QUIC streams to `terravibe_sim`'s sync
//! `ClientTransport` and `ServerConn` traits.

use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::Duration;

use iroh::endpoint::presets;
use iroh::{Endpoint, EndpointAddr};
use terravibe_sim::net::{ClientMsg, ClientTransport, Disconnected, ServerConn, ServerMsg};
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::runtime::{Builder as RuntimeBuilder, Runtime};
use tokio::sync::mpsc as tokio_mpsc;
use tokio::task::JoinHandle;

use crate::ALPN;
use crate::wire::{
    WireError, decode_client_msg, decode_server_msg, encode_client_msg, encode_server_msg,
    read_frame, write_frame,
};

const TICKET_PREFIX: &str = "tv1:";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const ONLINE_WAIT_TIMEOUT: Duration = Duration::from_millis(250);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Error)]
pub enum NetError {
    #[error("failed to create tokio runtime: {0}")]
    Runtime(std::io::Error),
    #[error("failed to bind iroh endpoint: {0}")]
    Bind(String),
    #[error("invalid ticket: {0}")]
    InvalidTicket(String),
    #[error("failed to connect to host: {0}")]
    Connect(String),
    #[error("failed to open QUIC stream: {0}")]
    Stream(String),
    #[error("wire codec error: {0}")]
    Wire(#[from] WireError),
}

/// Encode an `EndpointAddr` into a compact copy-pasteable ticket string.
pub fn encode_ticket(addr: &EndpointAddr) -> Result<String, NetError> {
    let bytes = postcard::to_allocvec(addr).map_err(|e| NetError::InvalidTicket(e.to_string()))?;
    Ok(format!("{TICKET_PREFIX}{}", base64url_encode(&bytes)))
}

/// Decode a ticket string back into an `EndpointAddr`.
pub fn decode_ticket(ticket: &str) -> Result<EndpointAddr, NetError> {
    let trimmed = ticket.trim();
    let encoded = trimmed
        .strip_prefix(TICKET_PREFIX)
        .ok_or_else(|| NetError::InvalidTicket("missing tv1: prefix".into()))?;
    let bytes =
        base64url_decode(encoded).ok_or_else(|| NetError::InvalidTicket("bad base64url".into()))?;
    postcard::from_bytes(&bytes).map_err(|e| NetError::InvalidTicket(e.to_string()))
}

/// Host-side `iroh` QUIC listener accepting incoming client connections.
pub struct IrohHost {
    endpoint: Option<Endpoint>,
    incoming_rx: Option<Receiver<Box<dyn ServerConn>>>,
    accept_task: Option<JoinHandle<()>>,
    rt: Option<Runtime>,
}

impl IrohHost {
    /// Bind an `iroh::Endpoint` with `ALPN` and start accepting incoming connections.
    pub fn bind() -> Result<Self, NetError> {
        let rt = RuntimeBuilder::new_multi_thread()
            .worker_threads(2)
            .thread_name("terravibe-iroh-host")
            .enable_all()
            .build()
            .map_err(NetError::Runtime)?;

        let endpoint = rt.block_on(async {
            let ep = Endpoint::builder(presets::N0)
                .alpns(vec![ALPN.to_vec()])
                .bind()
                .await
                .map_err(|e| NetError::Bind(e.to_string()))?;
            // Give relay discovery a brief moment if online, without stalling offline/LAN starts.
            let _ = tokio::time::timeout(ONLINE_WAIT_TIMEOUT, ep.online()).await;
            Ok::<_, NetError>(ep)
        })?;

        let (incoming_tx, incoming_rx) = mpsc::channel::<Box<dyn ServerConn>>();
        let ep_accept = endpoint.clone();
        let accept_task = rt.spawn(async move {
            accept_loop(ep_accept, incoming_tx).await;
        });

        Ok(Self {
            endpoint: Some(endpoint),
            incoming_rx: Some(incoming_rx),
            accept_task: Some(accept_task),
            rt: Some(rt),
        })
    }

    /// Return the current `iroh` ticket string for clients to connect to this host.
    pub fn ticket(&self) -> String {
        let ep = self
            .endpoint
            .as_ref()
            .expect("IrohHost endpoint active until shutdown");
        encode_ticket(&ep.addr()).expect("EndpointAddr postcard serialization is infallible")
    }

    /// Take the receiver of incoming `ServerConn`s (can be taken once).
    pub fn take_incoming(&mut self) -> Option<Receiver<Box<dyn ServerConn>>> {
        self.incoming_rx.take()
    }

    /// Cleanly close the `iroh` endpoint and shut down the background runtime.
    pub fn shutdown(mut self) {
        self.shutdown_inner();
    }

    fn shutdown_inner(&mut self) {
        if let Some(task) = self.accept_task.take() {
            task.abort();
        }
        if let (Some(ep), Some(rt)) = (self.endpoint.take(), self.rt.as_ref()) {
            let _ = rt.block_on(async { tokio::time::timeout(SHUTDOWN_TIMEOUT, ep.close()).await });
        }
        if let Some(rt) = self.rt.take() {
            rt.shutdown_timeout(SHUTDOWN_TIMEOUT);
        }
    }
}

impl Drop for IrohHost {
    fn drop(&mut self) {
        self.shutdown_inner();
    }
}

async fn accept_loop(endpoint: Endpoint, incoming_tx: Sender<Box<dyn ServerConn>>) {
    while let Some(incoming) = endpoint.accept().await {
        let tx = incoming_tx.clone();
        tokio::spawn(async move {
            let Ok(conn) = incoming.await else {
                return;
            };
            let Ok((mut send_stream, mut recv_stream)) = conn.accept_bi().await else {
                return;
            };

            let (to_client_tx, mut to_client_rx) = tokio_mpsc::unbounded_channel::<ServerMsg>();
            let (from_client_tx, from_client_rx) = mpsc::channel::<ClientMsg>();

            let server_conn = IrohServerConn {
                tx: to_client_tx,
                rx: from_client_rx,
            };
            if tx.send(Box::new(server_conn)).is_err() {
                return;
            }

            let conn_for_writer = conn.clone();
            tokio::spawn(async move {
                while let Some(msg) = to_client_rx.recv().await {
                    let Ok(frame) = encode_server_msg(&msg) else {
                        break;
                    };
                    if write_frame(&mut send_stream, &frame).await.is_err() {
                        break;
                    }
                    if send_stream.flush().await.is_err() {
                        break;
                    }
                }
                let _ = send_stream.finish();
                let _ = tokio::time::timeout(Duration::from_millis(200), send_stream.stopped()).await;
                drop(conn_for_writer);
            });

            tokio::spawn(async move {
                while let Ok(frame) = read_frame(&mut recv_stream).await {
                    let Ok(msg) = decode_client_msg(&frame) else {
                        break;
                    };
                    if from_client_tx.send(msg).is_err() {
                        break;
                    }
                }
                drop(from_client_tx);
                drop(conn);
            });
        });
    }
}

/// Server-side connection for a single remote `iroh` client.
pub struct IrohServerConn {
    tx: tokio_mpsc::UnboundedSender<ServerMsg>,
    rx: Receiver<ClientMsg>,
}

impl ServerConn for IrohServerConn {
    fn send(&self, m: ServerMsg) -> Result<(), Disconnected> {
        self.tx.send(m).map_err(|_| Disconnected)
    }

    fn try_recv(&self) -> Result<Option<ClientMsg>, Disconnected> {
        match self.rx.try_recv() {
            Ok(m) => Ok(Some(m)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(Disconnected),
        }
    }
}

/// Client-side `iroh` QUIC transport implementing `ClientTransport`.
pub struct IrohClientTransport {
    tx: Option<tokio_mpsc::UnboundedSender<ClientMsg>>,
    rx: Receiver<ServerMsg>,
    endpoint: Option<Endpoint>,
    rt: Option<Runtime>,
}

impl IrohClientTransport {
    /// Connect to a host using its ticket string.
    pub fn connect(ticket: &str) -> Result<Self, NetError> {
        let addr = decode_ticket(ticket)?;
        let rt = RuntimeBuilder::new_multi_thread()
            .worker_threads(2)
            .thread_name("terravibe-iroh-client")
            .enable_all()
            .build()
            .map_err(NetError::Runtime)?;

        let (endpoint, conn, mut send_stream, mut recv_stream) = rt.block_on(async {
            let ep = Endpoint::builder(presets::N0)
                .bind()
                .await
                .map_err(|e| NetError::Bind(e.to_string()))?;

            let conn = tokio::time::timeout(CONNECT_TIMEOUT, ep.connect(addr, ALPN))
                .await
                .map_err(|_| NetError::Connect("connection timed out".into()))?
                .map_err(|e| NetError::Connect(e.to_string()))?;

            let (send, recv) = tokio::time::timeout(CONNECT_TIMEOUT, conn.open_bi())
                .await
                .map_err(|_| NetError::Stream("open_bi timed out".into()))?
                .map_err(|e| NetError::Stream(e.to_string()))?;

            Ok::<_, NetError>((ep, conn, send, recv))
        })?;

        let (to_server_tx, mut to_server_rx) = tokio_mpsc::unbounded_channel::<ClientMsg>();
        let (from_server_tx, from_server_rx) = mpsc::channel::<ServerMsg>();

        let conn_for_writer = conn.clone();
        rt.spawn(async move {
            while let Some(msg) = to_server_rx.recv().await {
                let Ok(frame) = encode_client_msg(&msg) else {
                    break;
                };
                if write_frame(&mut send_stream, &frame).await.is_err() {
                    break;
                }
                if send_stream.flush().await.is_err() {
                    break;
                }
            }
            let _ = send_stream.finish();
            drop(conn_for_writer);
        });

        rt.spawn(async move {
            while let Ok(frame) = read_frame(&mut recv_stream).await {
                let Ok(msg) = decode_server_msg(&frame) else {
                    break;
                };
                if from_server_tx.send(msg).is_err() {
                    break;
                }
            }
            drop(from_server_tx);
            drop(conn);
        });

        Ok(Self {
            tx: Some(to_server_tx),
            rx: from_server_rx,
            endpoint: Some(endpoint),
            rt: Some(rt),
        })
    }
}

impl ClientTransport for IrohClientTransport {
    fn send(&self, m: ClientMsg) -> Result<(), Disconnected> {
        self.tx
            .as_ref()
            .ok_or(Disconnected)?
            .send(m)
            .map_err(|_| Disconnected)
    }

    fn try_recv(&self) -> Result<Option<ServerMsg>, Disconnected> {
        match self.rx.try_recv() {
            Ok(m) => Ok(Some(m)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(Disconnected),
        }
    }
}

impl Drop for IrohClientTransport {
    fn drop(&mut self) {
        self.tx.take();
        if let (Some(ep), Some(rt)) = (self.endpoint.take(), self.rt.as_ref()) {
            let _ = rt.block_on(async {
                tokio::time::timeout(Duration::from_millis(500), ep.close()).await
            });
        }
        if let Some(rt) = self.rt.take() {
            rt.shutdown_timeout(Duration::from_secs(1));
        }
    }
}

const B64_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn base64url_encode(input: &[u8]) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64_TABLE[((triple >> 18) & 0x3F) as usize] as char);
        out.push(B64_TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(B64_TABLE[((triple >> 6) & 0x3F) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(B64_TABLE[(triple & 0x3F) as usize] as char);
        }
    }
    out
}

fn base64url_decode(input: &str) -> Option<Vec<u8>> {
    let bytes = input.as_bytes();
    if bytes.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut vals = [0u32; 4];
        for (i, &b) in chunk.iter().enumerate() {
            vals[i] = match b {
                b'A'..=b'Z' => (b - b'A') as u32,
                b'a'..=b'z' => (b - b'a' + 26) as u32,
                b'0'..=b'9' => (b - b'0' + 52) as u32,
                b'-' => 62,
                b'_' => 63,
                _ => return None,
            };
        }
        let triple = (vals[0] << 18) | (vals[1] << 12) | (vals[2] << 6) | vals[3];
        out.push(((triple >> 16) & 0xFF) as u8);
        if chunk.len() > 2 {
            out.push(((triple >> 8) & 0xFF) as u8);
        }
        if chunk.len() > 3 {
            out.push((triple & 0xFF) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;
    use std::time::Instant;
    use terravibe_sim::block::{AIR, STONE};
    use terravibe_sim::chunk::{Chunk, ChunkPos};
    use terravibe_sim::net::PROTOCOL_VERSION;
    use terravibe_sim::world_bounds::WorldSize;

    fn wait_recv_client(conn: &dyn ServerConn, timeout: Duration) -> ClientMsg {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Some(msg) = conn.try_recv().expect("server conn still open") {
                return msg;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("timed out waiting for ClientMsg");
    }

    fn wait_recv_server(client: &dyn ClientTransport, timeout: Duration) -> ServerMsg {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Some(msg) = client.try_recv().expect("client transport still open") {
                return msg;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("timed out waiting for ServerMsg");
    }

    #[test]
    fn host_and_client_roundtrip() {
        let mut host = IrohHost::bind().expect("bind IrohHost");
        let incoming = host.take_incoming().expect("take incoming receiver");
        let ticket = host.ticket();
        assert!(ticket.starts_with(TICKET_PREFIX));

        let client = IrohClientTransport::connect(&ticket).expect("connect IrohClientTransport");

        // Send Hello so the QUIC stream is established and visible to accept_bi()
        client
            .send(ClientMsg::Hello {
                seed: 1234,
                name: Some("Tester".into()),
                character: None,
                protocol_version: PROTOCOL_VERSION,
                password: None,
            })
            .expect("send Hello");

        let server_conn = incoming
            .recv_timeout(Duration::from_secs(10))
            .expect("host receives incoming ServerConn");

        let hello = wait_recv_client(server_conn.as_ref(), Duration::from_secs(5));
        assert!(matches!(
            hello,
            ClientMsg::Hello {
                seed: 1234,
                protocol_version: PROTOCOL_VERSION,
                ..
            }
        ));

        // Server -> Client: Welcome
        server_conn
            .send(ServerMsg::Welcome {
                id: 3,
                spawn: [10.0, 64.0, -5.0],
                size: WorldSize::Small,
            })
            .expect("send Welcome");
        let welcome = wait_recv_server(&client, Duration::from_secs(5));
        assert!(matches!(welcome, ServerMsg::Welcome { id: 3, .. }));

        // Server -> Client: Chunk (lz4 compressed on wire)
        let mut chunk = Chunk::filled(AIR);
        chunk.set(3, 4, 5, STONE);
        let pos = ChunkPos { x: 1, y: 0, z: -1 };
        server_conn
            .send(ServerMsg::Chunk {
                pos,
                chunk: Arc::new(chunk),
            })
            .expect("send Chunk");

        let got_chunk = wait_recv_server(&client, Duration::from_secs(5));
        match got_chunk {
            ServerMsg::Chunk {
                pos: got_pos,
                chunk: got_c,
            } => {
                assert_eq!(got_pos, pos);
                assert_eq!(got_c.get(3, 4, 5), STONE);
            }
            other => panic!("expected Chunk, got {other:?}"),
        }

        drop(client);
        drop(server_conn);
        host.shutdown();
    }
}
