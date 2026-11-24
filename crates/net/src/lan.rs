//! LAN UDP broadcast beacon and room discovery on `LAN_PORT` (7778).

use std::collections::HashSet;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::LAN_PORT;
use crate::wire::WireError;

const LAN_MAGIC: &[u8; 4] = b"TVLN";
const LAN_VERSION: u8 = 1;
const MAX_LAN_PACKET_BYTES: usize = 8192;
const BEACON_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanRoomInfo {
    pub world_name: String,
    pub host_name: String,
    pub players: u8,
    pub max_players: u8,
    pub has_password: bool,
    pub ticket: String,
}

/// Encode a `LanRoomInfo` announcement packet with magic header and version byte.
pub fn encode_lan_packet(info: &LanRoomInfo) -> Result<Vec<u8>, WireError> {
    let body = postcard::to_allocvec(info)?;
    let total = LAN_MAGIC.len() + 1 + body.len();
    if total > MAX_LAN_PACKET_BYTES {
        return Err(WireError::FrameTooLarge {
            len: total,
            max: MAX_LAN_PACKET_BYTES,
        });
    }
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(LAN_MAGIC);
    out.push(LAN_VERSION);
    out.extend_from_slice(&body);
    Ok(out)
}

/// Decode a `LanRoomInfo` announcement packet.
pub fn decode_lan_packet(bytes: &[u8]) -> Result<LanRoomInfo, WireError> {
    if bytes.len() > MAX_LAN_PACKET_BYTES {
        return Err(WireError::FrameTooLarge {
            len: bytes.len(),
            max: MAX_LAN_PACKET_BYTES,
        });
    }
    if bytes.len() < LAN_MAGIC.len() + 1 {
        return Err(WireError::Truncated {
            expected: LAN_MAGIC.len() + 1,
            actual: bytes.len(),
        });
    }
    if &bytes[..4] != LAN_MAGIC {
        return Err(WireError::InvalidTag(bytes[0]));
    }
    if bytes[4] != LAN_VERSION {
        return Err(WireError::InvalidTag(bytes[4]));
    }
    Ok(postcard::from_bytes(&bytes[5..])?)
}

/// Background UDP beacon that broadcasts this host's `LanRoomInfo` on the local network.
pub struct LanBeacon {
    stop_flag: Arc<AtomicBool>,
    info: Arc<Mutex<LanRoomInfo>>,
    handle: Option<JoinHandle<()>>,
}

impl LanBeacon {
    /// Start broadcasting `info` on `LAN_PORT` (7778).
    pub fn start(info: LanRoomInfo) -> Result<Self, std::io::Error> {
        Self::start_on_port(info, LAN_PORT)
    }

    /// Start broadcasting `info` on a specific UDP `port` (useful for tests).
    pub fn start_on_port(info: LanRoomInfo, port: u16) -> Result<Self, std::io::Error> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        let _ = socket.set_broadcast(true);
        let broadcast_addr: SocketAddr = ([255, 255, 255, 255], port).into();
        let loopback_addr: SocketAddr = ([127, 0, 0, 1], port).into();

        let stop_flag = Arc::new(AtomicBool::new(false));
        let shared_info = Arc::new(Mutex::new(info));

        let stop_clone = Arc::clone(&stop_flag);
        let info_clone = Arc::clone(&shared_info);

        let handle = thread::Builder::new()
            .name("terravibe-lan-beacon".into())
            .spawn(move || {
                while !stop_clone.load(Ordering::Relaxed) {
                    let packet = {
                        let guard = info_clone.lock().unwrap_or_else(|e| e.into_inner());
                        encode_lan_packet(&guard)
                    };
                    if let Ok(bytes) = packet {
                        let _ = socket.send_to(&bytes, broadcast_addr);
                        let _ = socket.send_to(&bytes, loopback_addr);
                    }
                    // Sleep in short slices so stop() returns promptly.
                    let deadline = Instant::now() + BEACON_INTERVAL;
                    while !stop_clone.load(Ordering::Relaxed) && Instant::now() < deadline {
                        thread::sleep(Duration::from_millis(50));
                    }
                }
            })?;

        Ok(Self {
            stop_flag,
            info: shared_info,
            handle: Some(handle),
        })
    }

    /// Update the advertised player count while the beacon is running.
    pub fn update_players(&self, players: u8) {
        let mut guard = self.info.lock().unwrap_or_else(|e| e.into_inner());
        guard.players = players;
    }

    /// Stop broadcasting and join the background thread.
    pub fn stop(mut self) {
        self.stop_inner();
    }

    fn stop_inner(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for LanBeacon {
    fn drop(&mut self) {
        self.stop_inner();
    }
}

/// Listen for LAN room announcements on `LAN_PORT` (7778) for up to `timeout`.
pub fn discover_lan_rooms(timeout: Duration) -> Vec<LanRoomInfo> {
    discover_lan_rooms_on_port(LAN_PORT, timeout)
}

/// Listen for LAN room announcements on a specific UDP `port` for up to `timeout`.
pub fn discover_lan_rooms_on_port(port: u16, timeout: Duration) -> Vec<LanRoomInfo> {
    let Ok(socket) = UdpSocket::bind(("0.0.0.0", port)) else {
        return Vec::new();
    };
    let _ = socket.set_broadcast(true);
    let _ = socket.set_read_timeout(Some(Duration::from_millis(50)));

    let deadline = Instant::now() + timeout;
    let mut seen_tickets = HashSet::new();
    let mut rooms = Vec::new();
    let mut buf = [0u8; MAX_LAN_PACKET_BYTES];

    while Instant::now() < deadline {
        match socket.recv_from(&mut buf) {
            Ok((n, _src)) => {
                if let Ok(info) = decode_lan_packet(&buf[..n])
                    && seen_tickets.insert(info.ticket.clone())
                {
                    rooms.push(info);
                }
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(_) => break,
        }
    }

    rooms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beacon_packet_roundtrip() {
        let info = LanRoomInfo {
            world_name: "Oakvale".into(),
            host_name: "Alice".into(),
            players: 2,
            max_players: 8,
            has_password: true,
            ticket: "iroh-ticket-12345".into(),
        };
        let encoded = encode_lan_packet(&info).expect("encode lan packet");
        let decoded = decode_lan_packet(&encoded).expect("decode lan packet");
        assert_eq!(decoded, info);

        // Bad magic rejected
        let mut bad = encoded.clone();
        bad[0] = b'X';
        assert!(decode_lan_packet(&bad).is_err());

        // Truncated rejected
        assert!(decode_lan_packet(b"TV").is_err());
    }

    #[test]
    fn beacon_loopback_discovery() {
        // Reserve an ephemeral UDP port then release it for the test.
        let probe = UdpSocket::bind("127.0.0.1:0").expect("bind ephemeral udp");
        let port = probe.local_addr().unwrap().port();
        drop(probe);

        let info = LanRoomInfo {
            world_name: "LanWorld".into(),
            host_name: "Bob".into(),
            players: 1,
            max_players: 4,
            has_password: false,
            ticket: "tv1:test-ticket".into(),
        };
        let beacon = LanBeacon::start_on_port(info.clone(), port).expect("start beacon");
        let found = discover_lan_rooms_on_port(port, Duration::from_millis(800));
        beacon.stop();

        assert_eq!(found, vec![info]);
    }
}
