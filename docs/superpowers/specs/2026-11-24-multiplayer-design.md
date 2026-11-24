# Multiplayer (No Port Forwarding) — Design Specification

**Date:** 2026-11-24  
**Status:** Draft (Pending User Approval)

---

## 1. Intent & Why

Enable full co-op multiplayer in TerraVibe without requiring the host to open router ports, configure UPnP, or share raw IP addresses.

- **Host flow:** Click **Multiplayer** -> choose **Host & Play** -> pick character -> pick world -> configure **Max Players** (`2..=16`), optional **Password**, and **Public / Private** visibility -> launch. The host receives a short 6-character join code (viewable and copyable from the in-game pause menu) and, if Public, appears in the public server browser.
- **Join flow:** Click **Multiplayer** -> choose **Join Game** -> pick character -> either select a room from the **Public / LAN Server Browser** (showing world name, host name, `players/max`, and lock icon) or enter a **Join Code** (plus password if locked) -> join.
- **In-game experience:** Full co-op — remote players render as 3D voxel avatars with their appearance, worn armor/vanity, held item, walk/swing animations, and overhead nametags; terrain, blocks, walls, liquids, chests, enemies, bosses, drops, projectiles, weather, time, and town NPCs stay synchronized; players can chat in-game; and both guest and host disconnects save character/world state cleanly.

---

## 2. Goals, Non-Goals, and Anti-Goals

### Goals
- **Zero port forwarding:** `iroh` QUIC transport with automatic UDP hole-punching and fallback to n0's public relays.
- **Short codes & public browser:** A tiny stateless Rust lobby directory (`crates/lobby`) issuing 6-character join codes and listing public rooms, with graceful degradation to LAN UDP broadcast (`7778`) and raw `iroh` tickets when no lobby server is reachable.
- **Full co-op replication:** Remote player avatars, gear, animations, nametags, and multi-player server behavior across combat, chests, drops, bosses, and NPCs.
- **In-game chat & host controls:** Bottom-left chat HUD (`Enter`/`T` to type), plus pause-menu join code, Copy button, and connected player list.
- **Safe persistence:** Joined guests save their own `CharacterState` locally; host saves the world and all connected character states on autosave and shutdown.

### Non-Goals (v1)
- PvP, friendly fire, or team selection.
- Dedicated headless server CLI.
- Host migration if the host quits.
- Mod or custom asset downloads.
- User accounts, authentication, or friends lists.

### Anti-Goals
- Requiring host port forwarding, UPnP, or manual IP entry.
- Proxying game packets through the lobby directory server.
- Breaking single-player (`in_process_pair`, `spawn_local_cfg`, or existing QA/automation tests).

---

## 3. Approaches Considered

1. **Approach 1 (Picked): Hybrid P2P (`iroh`) + Tiny Stateless Lobby Directory (`crates/lobby`) + LAN/Ticket Fallback**
   - Game traffic flows P2P over `iroh` QUIC (hole-punched UDP or n0 public relay fallback).
   - `crates/lobby` is a tiny stateless `axum` HTTP service that only stores room metadata, issues 6-character codes, and answers browser/resolve queries.
   - *Trade-off:* Practically zero hosting cost/bandwidth for the lobby, fast direct P2P, short codes, and reliable public browser; requires deploying `terravibe-lobby` for internet-wide browser/short-code discovery (with LAN + raw ticket fallback offline).
2. **Approach 2: Pure P2P (`iroh` + `iroh-gossip`) + LAN Broadcast**
   - *Trade-off:* Zero self-hosted servers, but P2P gossip discovery suffers from cold-start unreliability when few players are online, and join codes are ~60–90 character tickets.
3. **Approach 3: Unified Self-Hosted Lobby + Relay Server**
   - *Trade-off:* Simplest networking and works behind the strictest firewalls, but proxies 100% of game traffic through the self-hosted server.

---

## 4. Architecture & Crate Design

### 4.1 `crates/sim` — Protocol & Serialization
- Add `serde` (`Serialize`, `Deserialize`) derives to `ClientMsg`, `ServerMsg`, and all payload types used by them (`Chunk`, `PlayerInput`, `PlayerBody`, `Vitals`, `Inventory`, `ItemStack`, `Projectile`, `Star`, `WorldFacts`, `CharacterFacts`, etc.). Keep `crates/sim/src/codec.rs` unchanged for save-file compatibility.
- Add `pub const PROTOCOL_VERSION: u32 = 1;`.
- Extend `ClientMsg::Hello`:
  - `protocol_version: u32`
  - `password: Option<String>`
- Add `ClientMsg::Chat { text: String }`.
- Add `ServerMsg` variants:
  - `Rejected { reason: RejectReason }` (`ProtocolMismatch`, `BadPassword`, `ServerFull`, `HostClosing`)
  - `RemotePlayerSpawn { id: PlayerId, name: String, appearance: Appearance, pos: [f32; 3], yaw: f32 }`
  - `RemotePlayerState { id: PlayerId, pos: [f32; 3], vel: [f32; 3], yaw: f32, pitch: f32, swinging: bool, dead: bool }`
  - `RemotePlayerEquipment { id: PlayerId, armor: [Option<ItemId>; 3], vanity_armor: [Option<ItemId>; 3], held: Option<ItemId> }`
  - `RemotePlayerDespawn { id: PlayerId }`
  - `Chat { sender: Option<String>, text: String }`
  - `PlayerList { players: Vec<(PlayerId, String)> }`

### 4.2 `crates/net` (New Crate) — `iroh` Transport, Lobby Client & LAN Discovery
- **`iroh` QUIC Transport:**
  - ALPN: `b"terravibe/1"`.
  - Framing: length-prefixed `postcard` frames. `ServerMsg::Chunk` payloads are compressed with `lz4_flex` to keep chunk streaming fast.
  - `IrohClientTransport` implements `terravibe_sim::net::ClientTransport` using a dedicated `tokio` runtime/background task bridged to sync `send` / `try_recv`.
  - `IrohServerConn` implements `terravibe_sim::net::ServerConn`.
  - `IrohHost` binds an `iroh::Endpoint`, exposes its `EndpointAddr` / ticket, and accepts incoming QUIC streams into `mpsc::Receiver<Box<dyn ServerConn>>`.
- **Lobby HTTP Client:**
  - Endpoints: `POST /rooms`, `POST /rooms/:code/heartbeat`, `DELETE /rooms/:code`, `GET /rooms`, `GET /rooms/:code`.
  - Base URL configurable via `TERRAVIBE_LOBBY_URL`. If unreachable or unset, host/join gracefully degrade to LAN discovery + raw `iroh` tickets.
- **LAN Discovery:**
  - UDP broadcast beacon and listener on port `7778` advertising `{ world_name, host_name, players, max_players, has_password, ticket }` for local network play.

### 4.3 `crates/lobby` (New Crate, Binary `terravibe-lobby`)
- Stateless `axum` + `tokio` HTTP server with in-memory `RwLock<HashMap<String, RoomEntry>>`.
- Unambiguous 6-character room codes from alphabet `ABCDEFGHJKLMNPQRSTUVWXYZ23456789` (excluding `0/O/1/I`).
- Each room stores a host secret token (returned on `POST /rooms`) required for `POST /rooms/:code/heartbeat` and `DELETE /rooms/:code`.
- 45-second room TTL with a 10-second background reaper task.
- Basic input caps/validation (field lengths, max rooms).
- Bind address configurable via `LOBBY_BIND` env var (default `0.0.0.0:7777`).

### 4.4 `crates/server` — Multi-Player Host & Validation
- Extend `ServerCfg` with `max_players: u8` (default `8`) and `password: Option<String>`.
- Add `spawn_host_cfg(seed, cfg, incoming_conns: mpsc::Receiver<Box<dyn ServerConn>>, shutdown: ...)`:
  - Host connects locally via `in_process_pair()` (zero network overhead for the host).
  - Server drains `incoming_conns` each tick and calls `server.connect(conn)`.
  - When the host disconnects, the server broadcasts `RejectReason::HostClosing`, flushes `CharacterState` to connected guests, saves the world, and exits cleanly.
- **Handshake validation on `Hello`:**
  - Check `protocol_version == PROTOCOL_VERSION`, `conns.len() < max_players`, and `password` match; on failure send `ServerMsg::Rejected { reason }` and close the connection.
- **Replication & Multi-Player Audit:**
  - Broadcast `RemotePlayerSpawn`, 20 Hz `RemotePlayerState` (scoped to `VIEW_RADIUS_BLOCKS`), `RemotePlayerEquipment`, `RemotePlayerDespawn`, `Chat`, and `PlayerList`.
  - Verify multi-player behavior for enemy/boss targeting (nearest alive player), drops, chests (existing per-chest lock), weather/time, and remote client character saving (`CharacterState` sent on autosave/disconnect).

### 4.5 `crates/client` — Menus, Remote Avatars, Chat & Pause UI
- **Session Mode:** Add `SessionMode` (`SinglePlayer`, `Host`, `Join`).
- **Menu Flow:**
  - Wire `Multiplayer` screen (`Host & Play`, `Join Game`, `Back`), reusing `Characters` and `Worlds` screens via a `MenuFlow` resource.
  - **Host Setup screen/panel:** Max Players (`2..=16`), optional Password, Public/Private toggle.
  - **Join / Server Browser screen:** Join Code input + Join button, Refresh button, combined Public + `[LAN]` server list (world name, host name, `players/max`, lock icon), and password prompt modal when joining a locked room.
- **Remote Players (`crates/client/src/view/remote_players.rs`):**
  - Share avatar/armor/held-item builder helpers with `crates/client/src/view/avatar.rs`.
  - 20 Hz position/yaw interpolation, walk/swing animations, and overhead nametags.
- **In-Game Chat (`crates/client/src/hud/chat.rs`):**
  - Bottom-left fading chat log; `Enter` or `T` opens input bar to send `ClientMsg::Chat`.
- **Pause Menu & Guest Persistence:**
  - Pause menu displays a multiplayer info card with the join code, Copy button, and connected player list.
  - Joined guests save their local character file whenever `ServerMsg::CharacterState` arrives.

---

## 5. What's Left to the Builder

- Exact UI pixel layout, spacing, and styling matching `crates/client/src/menu/style.rs`.
- Exact `postcard` + `lz4_flex` wire envelope framing details in `crates/net`.
- Exact interpolation smoothing constants for remote players.
- Clipboard crate choice (e.g. `arboard` with graceful fallback if unavailable).
- Test helper structure and integration test fixtures.

---

## 6. Verification Criteria

- **`crates/sim`:** `cargo test -p terravibe_sim` — `serde` roundtrip tests for new/updated `ClientMsg` and `ServerMsg` variants.
- **`crates/lobby`:** `cargo test -p terravibe_lobby` — room creation, 6-char code uniqueness, heartbeat/TTL expiry, delete auth, and public/private filtering.
- **`crates/net`:** `cargo test -p terravibe_net` — `iroh` host/client connection, `postcard` + `lz4_flex` chunk roundtrip, and LAN discovery packet roundtrip.
- **`crates/server`:** `cargo test -p terravibe_server` — `spawn_host_cfg` multi-client join, password/max-players/version rejection, remote player state/equipment/chat broadcast, and host-closing shutdown.
- **`crates/client`:** `cargo test -p terravibe_client` — menu flow transitions, multiplayer/browser/chat UI tests, and remote player spawn/despawn.
- **Workspace:** `cargo test --workspace` and `cargo clippy --workspace --all-targets`.
