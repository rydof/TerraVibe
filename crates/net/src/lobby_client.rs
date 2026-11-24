//! HTTP client for the stateless `terravibe-lobby` directory server.

use std::time::Duration;

use terravibe_lobby::{
    CreateRoomReq, CreateRoomResp, DeleteRoomReq, HeartbeatReq, ResolveRoomResp, RoomSummary,
};
use thiserror::Error;

/// Environment variable configuring the lobby directory base URL.
pub const LOBBY_URL_ENV: &str = "TERRAVIBE_LOBBY_URL";

/// Short timeout so offline / unreachable lobbies never stall the UI.
const HTTP_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Error)]
pub enum LobbyError {
    #[error("lobby HTTP transport error: {0}")]
    Transport(String),
    #[error("lobby returned HTTP {status}: {body}")]
    HttpStatus { status: u16, body: String },
    #[error("lobby JSON decode error: {0}")]
    Decode(String),
}

#[derive(Clone, Debug)]
pub struct LobbyClient {
    base_url: String,
    agent: ureq::Agent,
}

impl LobbyClient {
    /// Construct a lobby client pointing at `base_url` (e.g. `http://127.0.0.1:7777`).
    pub fn new(base_url: impl Into<String>) -> Self {
        let base_url = base_url.into().trim().trim_end_matches('/').to_string();
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(HTTP_TIMEOUT)
            .timeout_read(HTTP_TIMEOUT)
            .timeout_write(HTTP_TIMEOUT)
            .timeout(HTTP_TIMEOUT)
            .build();
        Self { base_url, agent }
    }

    /// Construct a lobby client from `TERRAVIBE_LOBBY_URL`, or `None` if unset/empty.
    pub fn from_env() -> Option<Self> {
        let url = std::env::var(LOBBY_URL_ENV).ok()?;
        let trimmed = url.trim();
        if trimmed.is_empty() {
            return None;
        }
        Some(Self::new(trimmed))
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn create_room(&self, req: &CreateRoomReq) -> Result<CreateRoomResp, LobbyError> {
        let url = format!("{}/rooms", self.base_url);
        let resp = map_ureq(self.agent.post(&url).send_json(req))?;
        resp.into_json::<CreateRoomResp>()
            .map_err(|e| LobbyError::Decode(e.to_string()))
    }

    pub fn heartbeat(&self, code: &str, req: &HeartbeatReq) -> Result<(), LobbyError> {
        let url = format!("{}/rooms/{}/heartbeat", self.base_url, code.trim());
        let _ = map_ureq(self.agent.post(&url).send_json(req))?;
        Ok(())
    }

    pub fn delete_room(&self, code: &str, req: &DeleteRoomReq) -> Result<(), LobbyError> {
        let url = format!("{}/rooms/{}", self.base_url, code.trim());
        let _ = map_ureq(self.agent.delete(&url).send_json(req))?;
        Ok(())
    }

    pub fn list_rooms(&self) -> Result<Vec<RoomSummary>, LobbyError> {
        let url = format!("{}/rooms", self.base_url);
        let resp = map_ureq(self.agent.get(&url).call())?;
        resp.into_json::<Vec<RoomSummary>>()
            .map_err(|e| LobbyError::Decode(e.to_string()))
    }

    pub fn resolve_room(&self, code: &str) -> Result<ResolveRoomResp, LobbyError> {
        let url = format!("{}/rooms/{}", self.base_url, code.trim());
        let resp = map_ureq(self.agent.get(&url).call())?;
        resp.into_json::<ResolveRoomResp>()
            .map_err(|e| LobbyError::Decode(e.to_string()))
    }
}

fn map_ureq(res: Result<ureq::Response, ureq::Error>) -> Result<ureq::Response, LobbyError> {
    match res {
        Ok(resp) => Ok(resp),
        Err(ureq::Error::Status(status, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            Err(LobbyError::HttpStatus { status, body })
        }
        Err(ureq::Error::Transport(t)) => Err(LobbyError::Transport(t.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::time::Instant;

    #[test]
    fn unreachable_lobby_returns_err_fast() {
        // Bind an ephemeral port and immediately drop it so nothing is listening there.
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let client = LobbyClient::new(format!("http://127.0.0.1:{port}"));
        let started = Instant::now();
        let err = client.list_rooms().unwrap_err();
        assert!(
            matches!(err, LobbyError::Transport(_)),
            "expected Transport error, got {err:?}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(4),
            "expected fast failure, took {:?}",
            started.elapsed()
        );
    }
}
