# Multiplayer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement full co-op multiplayer in TerraVibe without host port forwarding, featuring a public server browser, 6-character join codes, LAN discovery, remote 3D avatars, in-game chat, and clean persistence.

**Architecture:** Hybrid P2P (`iroh` QUIC with automatic UDP hole-punching + n0 public relay fallback) in `crates/net` implements `ClientTransport`/`ServerConn` from `crates/sim`, paired with a tiny stateless `axum` lobby directory (`crates/lobby`) for 6-char codes and public room listings (gracefully degrading to LAN UDP broadcast + raw `iroh` tickets when unreachable). `crates/server` gains `spawn_host_cfg`, handshake validation, and 20 Hz remote player replication; `crates/client` adds Host Setup / Server Browser UI, remote 3D avatars with nametags, in-game chat, and guest character saving.

**Tech Stack:** Rust (Edition 2024), Bevy 0.20, `iroh`, `postcard`, `lz4_flex`, `serde`, `tokio`, `axum`, `reqwest` (or `ureq`).

**Spec:** [docs/superpowers/specs/2026-11-24-multiplayer-design.md](file:///home/vlad/Programming/VibeCoded_Projects/TerraVibe/docs/superpowers/specs/2026-11-24-multiplayer-design.md)

## Global Constraints

- Zero host port forwarding: game traffic uses `iroh` QUIC (ALPN `b"terravibe/1"`) with automatic UDP hole-punching and n0 public relay fallback.
- `PROTOCOL_VERSION = 1` in `terravibe_sim::net`.
- Unambiguous 6-character room code alphabet: `ABCDEFGHJKLMNPQRSTUVWXYZ23456789`.
- Lobby directory (`terravibe-lobby`): 45-second room TTL, 10-second reaper task, `LOBBY_BIND` env var (default `0.0.0.0:7777`).
- Client/Net lobby URL: `TERRAVIBE_LOBBY_URL` env var; LAN UDP discovery on port `7778`.
- Host settings: `max_players` in `2..=16` (default `8`), optional `password`, Public/Private visibility.
- `RemotePlayerState` broadcast cadence: 20 Hz, scoped to `VIEW_RADIUS_BLOCKS`.
- Keep `crates/sim/src/codec.rs` unchanged for save-file compatibility.
- Preserve single-player (`in_process_pair`, `spawn_local_cfg`, and all existing QA/automation tests).
- Co-op only (no PvP, friendly fire, or teams).

## Review Focus

- Oversized (> 4 MiB) or truncated/corrupted wire frames -> rejected with `Disconnected` without OOM or panic (Task 3).
- Wrong password, full server, or protocol mismatch on `Hello` -> `ServerMsg::Rejected` sent and connection dropped with no ghost player or `RemotePlayerSpawn` broadcast (Task 4).
- Host disconnects while guests are connected -> `RejectReason::HostClosing` broadcast, `CharacterState` flushed & saved locally by guests, world saved by host, clean return to menu (Tasks 4 & 5).
- Lobby server unreachable or `TERRAVIBE_LOBBY_URL` unset -> host still starts `iroh` + LAN beacon and exposes raw `iroh` ticket; Join screen still shows `[LAN]` games and accepts raw tickets without hanging (Tasks 3 & 5).
- Typing in in-game chat (`W/A/S/D`, `Space`, `Esc`, hotbar keys) -> gameplay and pause input suppressed while `ChatInputOpen`, and empty/oversized (>200 char) chat messages rejected/clamped (Tasks 4 & 8).

---

### Task 1: Protocol & Serde Serialization (`crates/sim`)

**Files:**
- Modify: `crates/sim/Cargo.toml`
- Modify: `crates/sim/src/net.rs`
- Modify: `crates/sim/src/chunk.rs` (custom `Serialize`/`Deserialize` via `to_bytes`/`from_bytes`)
- Modify: `crates/sim/src/inventory.rs` (custom `Serialize`/`Deserialize` for `Inventory` since `SLOTS = 58 > 32`)
- Modify: payload types across `crates/sim/src/` (`block.rs`, `character.rs`, `crafting.rs`, `drop.rs`, `enemy.rs`, `facts.rs`, `interact.rs`, `item/mod.rs`, `item/prefix.rs`, `player/mod.rs`, `player/vitals.rs`, `projectile.rs`, `slots.rs`, `star.rs`, `town_npc.rs`, `weather.rs`, `world_bounds.rs`)
- Modify: existing `ClientMsg::Hello` call sites in `crates/sim/src/net.rs`, `crates/server/src/lib.rs`, `crates/client/src/net.rs`

**Interfaces:**
- Produces in `terravibe_sim::net`:
  - `pub const PROTOCOL_VERSION: u32 = 1;`
  - `#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)] pub enum RejectReason { ProtocolMismatch, BadPassword, ServerFull, HostClosing }`
  - `ClientMsg::Hello { seed: u32, name: Option<String>, character: Option<Vec<u8>>, protocol_version: u32, password: Option<String> }`
  - `ClientMsg::Chat { text: String }`
  - `ServerMsg::Rejected { reason: RejectReason }`
  - `ServerMsg::RemotePlayerSpawn { id: PlayerId, name: String, appearance: Appearance, pos: [f32; 3], yaw: f32 }`
  - `ServerMsg::RemotePlayerState { id: PlayerId, pos: [f32; 3], vel: [f32; 3], yaw: f32, pitch: f32, swinging: bool, dead: bool }`
  - `ServerMsg::RemotePlayerEquipment { id: PlayerId, armor: [Option<ItemId>; 3], vanity_armor: [Option<ItemId>; 3], held: Option<ItemId> }`
  - `ServerMsg::RemotePlayerDespawn { id: PlayerId }`
  - `ServerMsg::Chat { sender: Option<String>, text: String }`
  - `ServerMsg::PlayerList { players: Vec<(PlayerId, String)> }`

- [ ] **Step 1: Write the failing test in `crates/sim/src/net.rs`**
  Add `protocol_messages_roundtrip_over_in_proc` testing `PROTOCOL_VERSION == 1`, `ClientMsg::Hello` with `protocol_version` and `password`, `ClientMsg::Chat`, and all new `ServerMsg` variants (`Rejected`, `RemotePlayerSpawn`, `RemotePlayerState`, `RemotePlayerEquipment`, `RemotePlayerDespawn`, `Chat`, `PlayerList`).
- [ ] **Step 2: Run `cargo test -p terravibe_sim`**
  Expected: FAIL (missing `PROTOCOL_VERSION`, `RejectReason`, and new variants).
- [ ] **Step 3: Add `serde = { version = "1", features = ["derive"] }` to `crates/sim/Cargo.toml` and implement `Serialize`/`Deserialize` + new protocol types**
  - Derive `Serialize, Deserialize` on `ClientMsg`, `ServerMsg`, `RejectReason`, and all referenced sim types.
  - Implement custom `Serialize`/`Deserialize` for `Chunk` (delegating to `to_bytes()`/`from_bytes()`) and `Inventory` (serializing `slots.as_slice()` + `selected` and validating `len() == SLOTS` on deserialize).
  - Update existing `ClientMsg::Hello` struct literals in `sim`, `server`, and `client` to pass `protocol_version: PROTOCOL_VERSION, password: None`.
- [ ] **Step 4: Run `cargo test -p terravibe_sim`**
  Expected: PASS.
- [ ] **Step 5: Commit**
  ```bash
  git add crates/sim crates/server/src/lib.rs crates/client/src/net.rs
  git commit -m "feat(sim): add serde support and multiplayer protocol messages"
  ```

---

### Task 2: Stateless Lobby Directory Server (`crates/lobby`)

**Files:**
- Modify: `Cargo.toml` (workspace members already use `crates/*`)
- Create: `crates/lobby/Cargo.toml`
- Create: `crates/lobby/src/lib.rs`
- Create: `crates/lobby/src/main.rs`

**Interfaces:**
- Produces (`terravibe_lobby`):
  - `pub const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";`
  - `pub const ROOM_TTL_SECS: u64 = 45;`
  - `pub const REAPER_INTERVAL_SECS: u64 = 10;`
  - `pub struct CreateRoomReq { pub world_name: String, pub host_name: String, pub players: u8, pub max_players: u8, pub has_password: bool, pub public: bool, pub ticket: String }`
  - `pub struct CreateRoomResp { pub code: String, pub secret: String }`
  - `pub struct HeartbeatReq { pub secret: String, pub players: u8 }`
  - `pub struct DeleteRoomReq { pub secret: String }`
  - `pub struct RoomSummary { pub code: String, pub world_name: String, pub host_name: String, pub players: u8, pub max_players: u8, pub has_password: bool }`
  - `pub struct ResolveRoomResp { pub code: String, pub ticket: String, pub world_name: String, pub has_password: bool, pub players: u8, pub max_players: u8 }`
  - `pub fn app(state: LobbyState) -> axum::Router`
  - `pub fn spawn_reaper(state: LobbyState)`

- [ ] **Step 1: Write failing tests in `crates/lobby/src/lib.rs`**
  - `create_list_heartbeat_resolve_and_delete`: `POST /rooms` returns 6-char code from `CODE_ALPHABET` + secret; `GET /rooms` lists public rooms only; `POST /rooms/:code/heartbeat` updates player count (403 on wrong secret); `GET /rooms/:code` returns `ResolveRoomResp`; `DELETE /rooms/:code` removes room.
  - `expired_rooms_are_reaped`: room with `last_seen` older than `ROOM_TTL_SECS` is removed by `reap_expired(&state, now)`.
- [ ] **Step 2: Run `cargo test -p terravibe_lobby`**
  Expected: FAIL (crate does not exist yet).
- [ ] **Step 3: Implement `crates/lobby`**
  - `axum` router with `RwLock<HashMap<String, RoomEntry>>`, input length caps (`world_name <= 64`, `host_name <= 32`, `ticket <= 4096`, `max_rooms = 4096`), `LOBBY_BIND` env var (default `0.0.0.0:7777`) in `main.rs`.
- [ ] **Step 4: Run `cargo test -p terravibe_lobby`**
  Expected: PASS.
- [ ] **Step 5: Commit**
  ```bash
  git add crates/lobby
  git commit -m "feat(lobby): add stateless room directory server"
  ```

---

### Task 3: `iroh` QUIC Transport, Wire Framing, Lobby Client & LAN Discovery (`crates/net`)

**Files:**
- Create: `crates/net/Cargo.toml`
- Create: `crates/net/src/lib.rs`
- Create: `crates/net/src/wire.rs`
- Create: `crates/net/src/iroh_transport.rs`
- Create: `crates/net/src/lobby_client.rs`
- Create: `crates/net/src/lan.rs`

**Interfaces:**
- Consumes: `terravibe_sim::net::{ClientMsg, ClientTransport, Disconnected, ServerConn, ServerMsg}`, `terravibe_lobby` JSON schemas.
- Produces (`terravibe_net`):
  - `pub const ALPN: &[u8] = b"terravibe/1";`
  - `pub const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;`
  - `pub const LAN_PORT: u16 = 7778;`
  - `pub fn encode_client_msg(msg: &ClientMsg) -> Result<Vec<u8>, WireError>` / `decode_client_msg`
  - `pub fn encode_server_msg(msg: &ServerMsg) -> Result<Vec<u8>, WireError>` / `decode_server_msg` (compresses `ServerMsg::Chunk` with `lz4_flex`)
  - `pub struct IrohHost { ... }` with `pub fn bind() -> Result<Self, NetError>`, `pub fn ticket(&self) -> String`, `pub fn incoming(&self) -> mpsc::Receiver<Box<dyn ServerConn>>` (or `take_incoming`), `pub fn shutdown(self)`
  - `pub struct IrohClientTransport { ... }` implementing `ClientTransport`, with `pub fn connect(ticket: &str) -> Result<Self, NetError>`
  - `pub struct IrohServerConn { ... }` implementing `ServerConn`
  - `pub struct LobbyClient { ... }` with `from_env() -> Option<Self>` (`TERRAVIBE_LOBBY_URL`), `create_room`, `heartbeat`, `delete_room`, `list_rooms`, `resolve_room` (non-blocking/short timeout, returning `Err` cleanly when unreachable)
  - `pub struct LanBeacon` / `pub fn discover_lan_rooms(timeout: Duration) -> Vec<LanRoomInfo>`

- [ ] **Step 1: Write failing tests in `crates/net`**
  - `wire::chunk_roundtrip_and_oversized_frame_rejected` (Review Focus #1: > `MAX_FRAME_BYTES` or truncated/corrupted frame returns `Err` without OOM/panic).
  - `iroh_transport::host_and_client_roundtrip` (`IrohHost::bind`, `IrohClientTransport::connect(&host.ticket())`, `ClientMsg::Hello` -> `ServerMsg::Welcome` -> `ServerMsg::Chunk`).
  - `lobby_client::unreachable_lobby_returns_err_fast` (Review Focus #4: unreachable URL fails cleanly without hanging).
  - `lan::beacon_packet_roundtrip` (encode/decode LAN announcement).
- [ ] **Step 2: Run `cargo test -p terravibe_net`**
  Expected: FAIL (crate does not exist yet).
- [ ] **Step 3: Implement `crates/net` modules**
  - `wire.rs`: 4-byte LE length prefix, `MAX_FRAME_BYTES` check before allocation, `postcard` + `lz4_flex::compress_prepend_size` / `decompress_size_prepended` for `ServerMsg::Chunk`.
  - `iroh_transport.rs`: dedicated `tokio::runtime::Runtime` per host/client, `iroh::Endpoint` with `ALPN`, bidirectional QUIC stream per connection, background read/write tasks bridged to `std::sync::mpsc`.
  - `lobby_client.rs` and `lan.rs`: HTTP client with 3s timeout, UDP socket on `LAN_PORT`.
- [ ] **Step 4: Run `cargo test -p terravibe_net`**
  Expected: PASS.
- [ ] **Step 5: Commit**
  ```bash
  git add crates/net
  git commit -m "feat(net): add iroh QUIC transport, wire framing, lobby client, and LAN discovery"
  ```

---

### Task 4: Multi-Player Server Host, Validation & Remote Player Replication (`crates/server`)

**Files:**
- Modify: `crates/server/src/lib.rs`

**Interfaces:**
- Consumes: `terravibe_sim::net::{PROTOCOL_VERSION, RejectReason, ...}`.
- Produces (`terravibe_server`):
  - `ServerCfg::max_players: u8` (default `8`), `ServerCfg::password: Option<String>` (default `None`)
  - `pub fn spawn_host_cfg(seed: u32, cfg: ServerCfg, incoming_conns: std::sync::mpsc::Receiver<Box<dyn ServerConn>>, shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>) -> (InProcClient, std::thread::JoinHandle<()>)`

- [ ] **Step 1: Write failing tests in `crates/server/src/lib.rs`**
  - `hello_rejects_bad_password_full_server_and_wrong_version` (Review Focus #2: sends `ServerMsg::Rejected`, drops conn, no `RemotePlayerSpawn` broadcast).
  - `host_disconnect_broadcasts_host_closing_and_saves` (Review Focus #3: host `Bye` -> guests receive `ServerMsg::Rejected { reason: RejectReason::HostClosing }` and `CharacterState`, world saved).
  - `two_players_see_remote_spawn_state_equipment_and_chat` (`RemotePlayerSpawn`, 20 Hz `RemotePlayerState`, `RemotePlayerEquipment`, `ClientMsg::Chat` -> `ServerMsg::Chat` clamped to 200 chars and empty ignored (Review Focus #5), `PlayerList`, `RemotePlayerDespawn`).
  - `multiplayer_chest_lock_and_enemy_nearest_target`: second player cannot open chest already opened by first player; enemy targets nearest alive player.
- [ ] **Step 2: Run `cargo test -p terravibe_server`**
  Expected: FAIL.
- [ ] **Step 3: Implement multi-player host & replication in `crates/server/src/lib.rs`**
  - Add `max_players` and `password` to `ServerCfg` (default `8`, `None`).
  - Implement `spawn_host_cfg` using `in_process_pair()` for host (conn index `0`, always trusted/exempt from password/max-players) and draining `incoming_conns` each tick.
  - Validate `protocol_version`, `conns.len() < cfg.max_players as usize`, and `password` on `Hello` for non-host connections.
  - Broadcast `RemotePlayerSpawn`, 20 Hz `RemotePlayerState` (within `VIEW_RADIUS_BLOCKS`), `RemotePlayerEquipment` (on change/join), `RemotePlayerDespawn`, `Chat` (trim, ignore empty, truncate > 200 chars), and `PlayerList`.
  - On host disconnect, broadcast `RejectReason::HostClosing`, flush `CharacterState`, save world, and exit loop.
- [ ] **Step 4: Run `cargo test -p terravibe_server`**
  Expected: PASS.
- [ ] **Step 5: Commit**
  ```bash
  git add crates/server/src/lib.rs
  git commit -m "feat(server): add spawn_host_cfg, handshake validation, and remote player replication"
  ```

---

### Task 5: Client Session Modes, Network Pump & Guest Persistence (`crates/client/src/session.rs`, `crates/client/src/net.rs`)

**Files:**
- Modify: `crates/client/Cargo.toml` (add `terravibe_net = { path = "../net" }`)
- Modify: `crates/client/src/session.rs`
- Modify: `crates/client/src/net.rs`

**Interfaces:**
- Consumes: `terravibe_net::{IrohClientTransport, IrohHost, LanBeacon, LobbyClient}`, `terravibe_server::spawn_host_cfg`.
- Produces (`crates/client/src/session.rs` & `net.rs`):
  - `#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)] pub enum SessionMode { #[default] SinglePlayer, Host { max_players: u8, password: Option<String>, public: bool }, Join { ticket: String, password: Option<String> } }`
  - `#[derive(Resource, Clone, Debug, Default)] pub struct MultiplayerSessionInfo { pub join_code: Option<String>, pub ticket: Option<String>, pub players: Vec<(PlayerId, String)> }`
  - `#[derive(Message, Clone, Debug)] pub enum RemotePlayerMsg { Spawn { ... }, State { ... }, Equipment { ... }, Despawn { id: PlayerId } }`
  - `#[derive(Message, Clone, Debug)] pub struct ChatReceived { pub sender: Option<String>, pub text: String }`

- [ ] **Step 1: Write failing tests in `crates/client/src/session.rs` and `net.rs`**
  - `pump_handles_rejected_host_closing_and_remote_player_msgs`: `ServerMsg::Rejected` sets `DisconnectState`; `RemotePlayer*` and `Chat` emit Bevy messages; `PlayerList` updates `MultiplayerSessionInfo.players`.
  - `join_mode_saves_character_state_locally`: receiving `ServerMsg::CharacterState` in `SessionMode::Join` writes the character file to `characters_dir()`.
- [ ] **Step 2: Run `cargo test -p terravibe_client session::tests`**
  Expected: FAIL.
- [ ] **Step 3: Implement `SessionMode`, `MultiplayerSessionInfo`, `session_start`/`session_teardown` for Host/Join, and `pump` message handling**
  - `Host`: bind `IrohHost`, call `spawn_host_cfg`, start `LanBeacon` + optional `LobbyClient` registration/heartbeat thread (gracefully falling back to raw `host.ticket()` if `LobbyClient` is `None` or unreachable).
  - `Join`: connect via `IrohClientTransport::connect(&ticket)`, send `Hello` with `password`.
  - `session_teardown`: shut down `IrohHost` / `LanBeacon` / lobby heartbeat and delete lobby room if registered.
- [ ] **Step 4: Run `cargo test -p terravibe_client session::tests net::tests`**
  Expected: PASS.
- [ ] **Step 5: Commit**
  ```bash
  git add crates/client/Cargo.toml crates/client/src/session.rs crates/client/src/net.rs
  git commit -m "feat(client): wire Host/Join session modes, iroh net pump, and guest character save"
  ```

---

### Task 6: Multiplayer Menu UI — Host Setup, Server Browser & Join by Code (`crates/client/src/menu/`)

**Files:**
- Modify: `crates/client/src/session.rs` (add `MenuScreen::HostSetup`, `MenuScreen::JoinBrowser`)
- Modify: `crates/client/src/menu/mod.rs` (add `MenuFlow` resource, `Screen::{HostSetup, JoinBrowser}`, new `Action` variants, update `multi_screen_has_only_back_enabled` test)
- Modify: `crates/client/src/menu/multiplayer.rs` (enable `Host & Play` and `Join Game` buttons)
- Modify: `crates/client/src/menu/characters.rs` & `worlds.rs` (route next screen via `MenuFlow`)
- Create: `crates/client/src/menu/host_setup.rs`
- Create: `crates/client/src/menu/join_browser.rs`

**Interfaces:**
- Consumes: `SessionMode`, `terravibe_net::{LobbyClient, discover_lan_rooms}`.
- Produces: `MenuFlow` (`SinglePlayer`, `Host`, `Join`), `HostSetupPlugin`, `JoinBrowserPlugin`.

- [ ] **Step 1: Write failing tests in `crates/client/src/menu/`**
  - Update `multi_screen_has_only_back_enabled` in `menu/mod.rs` -> `multi_screen_has_host_join_and_back_enabled` (all 3 buttons enabled).
  - `host_setup_clamps_max_players_2_to_16_and_launches`: `HostSetup` screen spawns max-players `-`/`+`, password input, Public/Private toggle, and Launch button.
  - `join_browser_renders_code_input_refresh_and_room_list`: `JoinBrowser` spawns code input, Join button, Refresh button, and `[LAN]` / public room rows.
- [ ] **Step 2: Run `cargo test -p terravibe_client menu::`**
  Expected: FAIL.
- [ ] **Step 3: Implement `MenuFlow`, `multiplayer.rs`, `host_setup.rs`, and `join_browser.rs`**
  - `Host & Play` -> `MenuFlow::Host` -> `Characters` -> `Worlds` -> `HostSetup` -> `AppState::InGame`.
  - `Join Game` -> `MenuFlow::Join` -> `Characters` -> `JoinBrowser` (code input or room row + password modal if `has_password`) -> `AppState::InGame`.
- [ ] **Step 4: Run `cargo test -p terravibe_client menu::`**
  Expected: PASS.
- [ ] **Step 5: Commit**
  ```bash
  git add crates/client/src/session.rs crates/client/src/menu
  git commit -m "feat(client): add Host Setup, Server Browser, and Join by Code menu screens"
  ```

---

### Task 7: Remote Player 3D Avatars, Animations & Overhead Nametags (`crates/client/src/view/remote_players.rs`, `crates/client/src/view/avatar.rs`)

**Files:**
- Modify: `crates/client/src/view/avatar.rs` (extract shared `spawn_player_rig_parts`, `apply_armor_to_rig`, `update_held_item_on_rig` helpers)
- Create: `crates/client/src/view/remote_players.rs`
- Modify: `crates/client/src/view/mod.rs` (register `RemotePlayersPlugin`)

**Interfaces:**
- Consumes: `RemotePlayerMsg` from `crate::net`.
- Produces: `RemotePlayersPlugin`, `RemotePlayerEntity { pub id: PlayerId }`.

- [ ] **Step 1: Write failing tests in `crates/client/src/view/remote_players.rs`**
  - `remote_player_spawn_state_and_despawn`: `RemotePlayerMsg::Spawn` spawns entity with `RemotePlayerEntity`, `State` updates target position/yaw/swing/dead, `Despawn` removes entity.
- [ ] **Step 2: Run `cargo test -p terravibe_client view::remote_players`**
  Expected: FAIL.
- [ ] **Step 3: Extract shared rig helpers from `avatar.rs` and implement `RemotePlayersPlugin`**
  - 20 Hz position/yaw interpolation, walk animation from velocity, swing animation on `arm_r`, hide when `dead`, update armor/vanity/held item on `Equipment`, overhead `Text2d` / billboard nametag.
- [ ] **Step 4: Run `cargo test -p terravibe_client view::remote_players view::avatar`**
  Expected: PASS.
- [ ] **Step 5: Commit**
  ```bash
  git add crates/client/src/view/avatar.rs crates/client/src/view/remote_players.rs crates/client/src/view/mod.rs
  git commit -m "feat(client): render remote player 3D avatars, gear, animations, and nametags"
  ```

---

### Task 8: In-Game Chat HUD & Pause Menu Multiplayer Card (`crates/client/src/hud/chat.rs`, `crates/client/src/menu/game.rs`)

**Files:**
- Create: `crates/client/src/hud/chat.rs`
- Modify: `crates/client/src/hud/mod.rs` (register `ChatPlugin`)
- Modify: `crates/client/src/menu/game.rs` (add multiplayer info card with Join Code + Copy button + connected players list; suppress Esc/pause while `ChatInputOpen`)
- Modify: `crates/client/src/view/player.rs` (suppress movement/action input while `ChatInputOpen`)
- Modify: `crates/client/Cargo.toml` (add `arboard` or clipboard helper if needed)

**Interfaces:**
- Consumes: `ChatReceived`, `MultiplayerSessionInfo`, `Net`.
- Produces: `ChatPlugin`, `ChatInputOpen(pub bool)`.

- [ ] **Step 1: Write failing tests in `crates/client/src/hud/chat.rs` and `crates/client/src/menu/game.rs`**
  - `chat_open_suppresses_gameplay_and_sends_message` (Review Focus #5: `ChatInputOpen(true)` blocks movement/pause; `Enter` sends `ClientMsg::Chat` and closes).
  - `pause_menu_shows_join_code_and_players_when_hosting`: `MultiplayerSessionInfo` with `join_code: Some("TV4K9P")` renders join code text, Copy button, and player list.
- [ ] **Step 2: Run `cargo test -p terravibe_client hud::chat menu::game`**
  Expected: FAIL.
- [ ] **Step 3: Implement `ChatPlugin` and pause menu multiplayer card**
  - Bottom-left fading chat log (last 8 messages, 10s fade), `Enter`/`T` opens input bar.
  - Pause menu card shows Join Code + Copy button (using `arboard` with graceful fallback) + connected players list.
- [ ] **Step 4: Run full workspace verification**
  Run: `cargo test --workspace && cargo clippy --workspace --all-targets`
  Expected: All new and existing multiplayer/client/server/sim/lobby/net tests PASS.
- [ ] **Step 5: Commit**
  ```bash
  git add crates/client
  git commit -m "feat(client): add in-game chat HUD and pause menu multiplayer card"
  ```
