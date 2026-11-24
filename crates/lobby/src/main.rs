use terravibe_lobby::{LobbyState, app, spawn_reaper};

#[tokio::main]
async fn main() {
    let bind_addr =
        std::env::var("LOBBY_BIND").unwrap_or_else(|_| "0.0.0.0:7777".to_string());
    let state = LobbyState::new();
    spawn_reaper(state.clone());

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|e| panic!("failed to bind {bind_addr}: {e}"));
    eprintln!("terravibe-lobby listening on {bind_addr}");

    axum::serve(listener, app(state))
        .await
        .expect("lobby server exited with error");
}
