use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use rand::RngExt;
use serde::{Deserialize, Serialize};

pub const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
pub const CODE_LEN: usize = 6;
pub const ROOM_TTL_SECS: u64 = 45;
pub const REAPER_INTERVAL_SECS: u64 = 10;

pub const MAX_WORLD_NAME_LEN: usize = 64;
pub const MAX_HOST_NAME_LEN: usize = 32;
pub const MAX_TICKET_LEN: usize = 4096;
pub const MAX_ROOMS: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateRoomReq {
    pub world_name: String,
    pub host_name: String,
    pub players: u8,
    pub max_players: u8,
    pub has_password: bool,
    pub public: bool,
    pub ticket: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateRoomResp {
    pub code: String,
    pub secret: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeartbeatReq {
    pub secret: String,
    pub players: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteRoomReq {
    pub secret: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomSummary {
    pub code: String,
    pub world_name: String,
    pub host_name: String,
    pub players: u8,
    pub max_players: u8,
    pub has_password: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolveRoomResp {
    pub code: String,
    pub ticket: String,
    pub world_name: String,
    pub has_password: bool,
    pub players: u8,
    pub max_players: u8,
}

#[derive(Clone, Debug)]
struct RoomEntry {
    world_name: String,
    host_name: String,
    players: u8,
    max_players: u8,
    has_password: bool,
    public: bool,
    ticket: String,
    secret: String,
    last_seen: Instant,
}

impl RoomEntry {
    fn is_expired(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.last_seen) >= Duration::from_secs(ROOM_TTL_SECS)
    }
}

#[derive(Clone, Default)]
pub struct LobbyState {
    rooms: Arc<RwLock<HashMap<String, RoomEntry>>>,
}

impl LobbyState {
    pub fn new() -> Self {
        Self::default()
    }
}

pub fn app(state: LobbyState) -> Router {
    Router::new()
        .route("/rooms", post(create_room).get(list_rooms))
        .route("/rooms/{code}", get(resolve_room).delete(delete_room))
        .route("/rooms/{code}/heartbeat", post(heartbeat_room))
        .with_state(state)
}

pub fn reap_expired(state: &LobbyState, now: Instant) -> usize {
    let mut rooms = state.rooms.write().unwrap_or_else(|e| e.into_inner());
    reap_expired_map(&mut rooms, now)
}

pub fn spawn_reaper(state: LobbyState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(REAPER_INTERVAL_SECS));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            reap_expired(&state, Instant::now());
        }
    });
}

fn reap_expired_map(rooms: &mut HashMap<String, RoomEntry>, now: Instant) -> usize {
    let before = rooms.len();
    rooms.retain(|_, entry| !entry.is_expired(now));
    before - rooms.len()
}

fn normalize_code(code: &str) -> String {
    code.trim().to_ascii_uppercase()
}

fn generate_code() -> String {
    let mut rng = rand::rng();
    (0..CODE_LEN)
        .map(|_| CODE_ALPHABET[rng.random_range(0..CODE_ALPHABET.len())] as char)
        .collect()
}

fn generate_secret() -> String {
    let bytes: [u8; 16] = rand::random();
    let mut out = String::with_capacity(32);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

async fn create_room(
    State(state): State<LobbyState>,
    Json(req): Json<CreateRoomReq>,
) -> Result<(StatusCode, Json<CreateRoomResp>), StatusCode> {
    let world_name = req.world_name.trim();
    let host_name = req.host_name.trim();
    let ticket = req.ticket.trim();

    if world_name.is_empty()
        || world_name.len() > MAX_WORLD_NAME_LEN
        || host_name.is_empty()
        || host_name.len() > MAX_HOST_NAME_LEN
        || ticket.is_empty()
        || ticket.len() > MAX_TICKET_LEN
        || req.max_players == 0
        || req.players > req.max_players
    {
        return Err(StatusCode::BAD_REQUEST);
    }

    let now = Instant::now();
    let mut rooms = state.rooms.write().unwrap_or_else(|e| e.into_inner());
    if rooms.len() >= MAX_ROOMS {
        reap_expired_map(&mut rooms, now);
        if rooms.len() >= MAX_ROOMS {
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
    }

    let code = (0..64)
        .map(|_| generate_code())
        .find(|candidate| !rooms.contains_key(candidate))
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let secret = generate_secret();

    rooms.insert(
        code.clone(),
        RoomEntry {
            world_name: world_name.to_string(),
            host_name: host_name.to_string(),
            players: req.players,
            max_players: req.max_players,
            has_password: req.has_password,
            public: req.public,
            ticket: ticket.to_string(),
            secret: secret.clone(),
            last_seen: now,
        },
    );

    Ok((StatusCode::CREATED, Json(CreateRoomResp { code, secret })))
}

async fn list_rooms(State(state): State<LobbyState>) -> Json<Vec<RoomSummary>> {
    let now = Instant::now();
    let rooms = state.rooms.read().unwrap_or_else(|e| e.into_inner());
    let list = rooms
        .iter()
        .filter(|(_, entry)| entry.public && !entry.is_expired(now))
        .map(|(code, entry)| RoomSummary {
            code: code.clone(),
            world_name: entry.world_name.clone(),
            host_name: entry.host_name.clone(),
            players: entry.players,
            max_players: entry.max_players,
            has_password: entry.has_password,
        })
        .collect();
    Json(list)
}

async fn resolve_room(
    State(state): State<LobbyState>,
    Path(code): Path<String>,
) -> Result<Json<ResolveRoomResp>, StatusCode> {
    let code = normalize_code(&code);
    let now = Instant::now();
    let rooms = state.rooms.read().unwrap_or_else(|e| e.into_inner());
    let entry = rooms.get(&code).ok_or(StatusCode::NOT_FOUND)?;
    if entry.is_expired(now) {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(Json(ResolveRoomResp {
        code,
        ticket: entry.ticket.clone(),
        world_name: entry.world_name.clone(),
        has_password: entry.has_password,
        players: entry.players,
        max_players: entry.max_players,
    }))
}

async fn heartbeat_room(
    State(state): State<LobbyState>,
    Path(code): Path<String>,
    Json(req): Json<HeartbeatReq>,
) -> StatusCode {
    let code = normalize_code(&code);
    let now = Instant::now();
    let mut rooms = state.rooms.write().unwrap_or_else(|e| e.into_inner());
    let Some(entry) = rooms.get_mut(&code) else {
        return StatusCode::NOT_FOUND;
    };
    if entry.is_expired(now) {
        rooms.remove(&code);
        return StatusCode::NOT_FOUND;
    }
    if !constant_time_eq(&entry.secret, &req.secret) {
        return StatusCode::FORBIDDEN;
    }
    entry.players = req.players.min(entry.max_players);
    entry.last_seen = now;
    StatusCode::NO_CONTENT
}

async fn delete_room(
    State(state): State<LobbyState>,
    Path(code): Path<String>,
    Json(req): Json<DeleteRoomReq>,
) -> StatusCode {
    let code = normalize_code(&code);
    let mut rooms = state.rooms.write().unwrap_or_else(|e| e.into_inner());
    let Some(entry) = rooms.get(&code) else {
        return StatusCode::NOT_FOUND;
    };
    if !constant_time_eq(&entry.secret, &req.secret) {
        return StatusCode::FORBIDDEN;
    }
    rooms.remove(&code);
    StatusCode::NO_CONTENT
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request, StatusCode, header};
    use tower::ServiceExt;

    async fn send_json(
        router: &Router,
        method: Method,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, Vec<u8>) {
        let mut builder = Request::builder().method(method).uri(uri);
        let req = match body {
            Some(v) => {
                builder = builder.header(header::CONTENT_TYPE, "application/json");
                builder.body(Body::from(v.to_string())).unwrap()
            }
            None => builder.body(Body::empty()).unwrap(),
        };
        let resp = router.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
        (status, bytes.to_vec())
    }

    #[tokio::test]
    async fn create_list_heartbeat_resolve_and_delete() {
        let state = LobbyState::new();
        let router = app(state);

        // Create a public room
        let (status, body) = send_json(
            &router,
            Method::POST,
            "/rooms",
            Some(serde_json::json!({
                "world_name": "Oakvale",
                "host_name": "Alice",
                "players": 1,
                "max_players": 8,
                "has_password": false,
                "public": true,
                "ticket": "iroh-ticket-public-1"
            })),
        )
        .await;
        assert!(status.is_success(), "expected 2xx, got {status}");
        let pub_room: CreateRoomResp = serde_json::from_slice(&body).unwrap();
        assert_eq!(pub_room.code.len(), CODE_LEN);
        assert!(
            pub_room.code.bytes().all(|b| CODE_ALPHABET.contains(&b)),
            "code {} has invalid chars",
            pub_room.code
        );
        assert!(!pub_room.secret.is_empty());

        // Create a private room
        let (status, body) = send_json(
            &router,
            Method::POST,
            "/rooms",
            Some(serde_json::json!({
                "world_name": "SecretBase",
                "host_name": "Bob",
                "players": 1,
                "max_players": 4,
                "has_password": true,
                "public": false,
                "ticket": "iroh-ticket-private-1"
            })),
        )
        .await;
        assert!(status.is_success(), "expected 2xx, got {status}");
        let priv_room: CreateRoomResp = serde_json::from_slice(&body).unwrap();
        assert_ne!(pub_room.code, priv_room.code);

        // GET /rooms lists public rooms only
        let (status, body) = send_json(&router, Method::GET, "/rooms", None).await;
        assert_eq!(status, StatusCode::OK);
        let list: Vec<RoomSummary> = serde_json::from_slice(&body).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].code, pub_room.code);
        assert_eq!(list[0].world_name, "Oakvale");
        assert_eq!(list[0].host_name, "Alice");
        assert_eq!(list[0].players, 1);
        assert_eq!(list[0].max_players, 8);
        assert!(!list[0].has_password);

        // Heartbeat with wrong secret -> 403
        let (status, _) = send_json(
            &router,
            Method::POST,
            &format!("/rooms/{}/heartbeat", pub_room.code),
            Some(serde_json::json!({
                "secret": "wrong-secret",
                "players": 3
            })),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        // Heartbeat with valid secret updates player count
        let (status, _) = send_json(
            &router,
            Method::POST,
            &format!("/rooms/{}/heartbeat", pub_room.code),
            Some(serde_json::json!({
                "secret": pub_room.secret,
                "players": 3
            })),
        )
        .await;
        assert!(status.is_success(), "expected 2xx, got {status}");

        // Resolve public room (also tests case-insensitive lookup)
        let (status, body) = send_json(
            &router,
            Method::GET,
            &format!("/rooms/{}", pub_room.code.to_ascii_lowercase()),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let resolved: ResolveRoomResp = serde_json::from_slice(&body).unwrap();
        assert_eq!(resolved.code, pub_room.code);
        assert_eq!(resolved.ticket, "iroh-ticket-public-1");
        assert_eq!(resolved.world_name, "Oakvale");
        assert!(!resolved.has_password);
        assert_eq!(resolved.players, 3);
        assert_eq!(resolved.max_players, 8);

        // Resolve private room still works by code
        let (status, body) = send_json(
            &router,
            Method::GET,
            &format!("/rooms/{}", priv_room.code),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let resolved_priv: ResolveRoomResp = serde_json::from_slice(&body).unwrap();
        assert_eq!(resolved_priv.ticket, "iroh-ticket-private-1");
        assert!(resolved_priv.has_password);

        // Delete with wrong secret -> 403
        let (status, _) = send_json(
            &router,
            Method::DELETE,
            &format!("/rooms/{}", pub_room.code),
            Some(serde_json::json!({ "secret": "wrong-secret" })),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        // Delete with right secret -> removes room
        let (status, _) = send_json(
            &router,
            Method::DELETE,
            &format!("/rooms/{}", pub_room.code),
            Some(serde_json::json!({ "secret": pub_room.secret })),
        )
        .await;
        assert!(status.is_success(), "expected 2xx, got {status}");

        // Resolving deleted room -> 404
        let (status, _) = send_json(
            &router,
            Method::GET,
            &format!("/rooms/{}", pub_room.code),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn expired_rooms_are_reaped() {
        let state = LobbyState::new();
        let now = Instant::now();
        let expired_time = now - Duration::from_secs(ROOM_TTL_SECS + 1);
        let fresh_time = now - Duration::from_secs(ROOM_TTL_SECS - 5);

        {
            let mut rooms = state.rooms.write().unwrap();
            rooms.insert(
                "AAAAAA".to_string(),
                RoomEntry {
                    world_name: "OldWorld".into(),
                    host_name: "OldHost".into(),
                    players: 1,
                    max_players: 4,
                    has_password: false,
                    public: true,
                    ticket: "old-ticket".into(),
                    secret: "s1".into(),
                    last_seen: expired_time,
                },
            );
            rooms.insert(
                "BBBBBB".to_string(),
                RoomEntry {
                    world_name: "FreshWorld".into(),
                    host_name: "FreshHost".into(),
                    players: 2,
                    max_players: 8,
                    has_password: false,
                    public: true,
                    ticket: "fresh-ticket".into(),
                    secret: "s2".into(),
                    last_seen: fresh_time,
                },
            );
        }

        let removed = reap_expired(&state, now);
        assert_eq!(removed, 1);

        let rooms = state.rooms.read().unwrap();
        assert!(!rooms.contains_key("AAAAAA"));
        assert!(rooms.contains_key("BBBBBB"));
    }

    #[tokio::test]
    async fn input_caps_and_max_rooms_enforced() {
        let state = LobbyState::new();
        let router = app(state.clone());

        // Oversized world_name (> 64)
        let (status, _) = send_json(
            &router,
            Method::POST,
            "/rooms",
            Some(serde_json::json!({
                "world_name": "w".repeat(MAX_WORLD_NAME_LEN + 1),
                "host_name": "Alice",
                "players": 1,
                "max_players": 4,
                "has_password": false,
                "public": true,
                "ticket": "t"
            })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        // Oversized host_name (> 32)
        let (status, _) = send_json(
            &router,
            Method::POST,
            "/rooms",
            Some(serde_json::json!({
                "world_name": "World",
                "host_name": "h".repeat(MAX_HOST_NAME_LEN + 1),
                "players": 1,
                "max_players": 4,
                "has_password": false,
                "public": true,
                "ticket": "t"
            })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        // Oversized ticket (> 4096)
        let (status, _) = send_json(
            &router,
            Method::POST,
            "/rooms",
            Some(serde_json::json!({
                "world_name": "World",
                "host_name": "Alice",
                "players": 1,
                "max_players": 4,
                "has_password": false,
                "public": true,
                "ticket": "x".repeat(MAX_TICKET_LEN + 1)
            })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        // Fill up to MAX_ROOMS
        {
            let now = Instant::now();
            let mut rooms = state.rooms.write().unwrap();
            for i in 0..MAX_ROOMS {
                rooms.insert(
                    format!("R{i:05}"),
                    RoomEntry {
                        world_name: "W".into(),
                        host_name: "H".into(),
                        players: 1,
                        max_players: 4,
                        has_password: false,
                        public: false,
                        ticket: "t".into(),
                        secret: "s".into(),
                        last_seen: now,
                    },
                );
            }
        }

        let (status, _) = send_json(
            &router,
            Method::POST,
            "/rooms",
            Some(serde_json::json!({
                "world_name": "Overflow",
                "host_name": "Alice",
                "players": 1,
                "max_players": 4,
                "has_password": false,
                "public": true,
                "ticket": "t"
            })),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }
}
