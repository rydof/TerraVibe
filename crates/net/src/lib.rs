//! Networking crate for TerraVibe multiplayer: `iroh` QUIC transport, wire framing,
//! lobby HTTP client, and LAN UDP discovery.

pub mod iroh_transport;
pub mod lan;
pub mod lobby_client;
pub mod wire;

pub const ALPN: &[u8] = b"terravibe/1";
pub const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;
pub const LAN_PORT: u16 = 7778;

pub use iroh_transport::{
    IrohClientTransport, IrohHost, IrohServerConn, NetError, decode_ticket, encode_ticket,
};
pub use lan::{LanBeacon, LanRoomInfo, decode_lan_packet, discover_lan_rooms, encode_lan_packet};
pub use lobby_client::{LOBBY_URL_ENV, LobbyClient, LobbyError};
pub use wire::{
    WireError, decode_client_msg, decode_server_msg, encode_client_msg, encode_server_msg,
};
