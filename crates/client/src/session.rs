//! Application states, session lifecycle, and gameplay gating.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread::JoinHandle;

use bevy::prelude::*;
use terravibe_server::{SaveCfg, ServerCfg, WorldInfo, list_worlds, spawn_local_cfg};
use terravibe_sim::inventory::Inventory;
use terravibe_sim::item::Rng;
use terravibe_sim::net::ClientMsg;

use crate::net::{
    ClientClock, LocalAccessories, LocalAccessoryPrefixes, LocalArmor, LocalInventory, Net,
    NetState,
};
use crate::view::SEED;
use crate::view::chunks::ChunkStream;
use crate::view::player::{LocalPlayer, PlayerView};

/// Top-level application states.
#[derive(States, Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum AppState {
    #[default]
    Menu,
    InGame,
}

/// Menu screen sub-states, active while in `AppState::Menu`.
#[derive(SubStates, Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[source(AppState = AppState::Menu)]
pub enum MenuScreen {
    #[default]
    Title,
    Characters,
    CreateCharacter,
    Worlds,
    CreateWorld,
    Options,
    Multiplayer,
}

/// Currently chosen character and decoded Character data.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct SelectedCharacter(pub Option<terravibe_sim::Character>);

/// Information about the world selected to play.
#[derive(Resource, Clone, Debug, PartialEq, Default)]
pub struct SelectedWorld {
    pub name: String,
    pub seed: u32,
    pub size: terravibe_sim::WorldSize,
    pub persist: bool,
}

/// Root directory where worlds are saved.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct SaveRoot(pub PathBuf);

/// Handle to the in-process server thread during an active game session.
#[derive(Resource)]
pub struct SessionHandle(pub Option<JoinHandle<()>>);

impl SessionHandle {
    pub fn new(handle: JoinHandle<()>) -> Self {
        Self(Some(handle))
    }
}

impl Drop for SessionHandle {
    fn drop(&mut self) {
        if let Some(h) = self.0.take() {
            *SERVER_THREAD.lock().unwrap() = Some(h);
        }
    }
}

/// Handle to the in-process server thread during shutdown/saving.
#[derive(Resource)]
pub struct SessionStopping(pub Option<JoinHandle<()>>);

impl SessionStopping {
    pub fn new(handle: JoinHandle<()>) -> Self {
        Self(Some(handle))
    }
}

impl Drop for SessionStopping {
    fn drop(&mut self) {
        if let Some(h) = self.0.take() {
            if !h.is_finished() {
                *SERVER_THREAD.lock().unwrap() = Some(h);
            } else {
                let _ = h.join();
            }
        }
    }
}

/// Error message if session connection or loading fails.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct SessionError(pub Option<String>);

/// Active disconnect state when the connection to the server is lost.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct DisconnectState {
    pub reason: String,
}

/// Marker resource indicating a disconnect retry should re-enter `AppState::InGame` once teardown finishes.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct SessionRetryPending;

/// Marker resource present only during automation runs (`--bench`, screenshot, demo).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Automation;

/// In-game pause menu state.
#[derive(Resource, Default, Debug)]
pub struct GameMenu {
    pub panel: Option<GamePanel>,
}

/// In-game pause menu panels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GamePanel {
    Root,
    Options,
    Controls,
    ConfirmQuit,
}

/// How the client was launched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LaunchMode {
    Menu,
    DirectWorld { name: String },
    Automation,
}

/// Determines the launch mode from flags and environment.
pub fn launch_mode(automation: bool, world: Option<String>) -> LaunchMode {
    if automation {
        LaunchMode::Automation
    } else if let Some(name) = world {
        LaunchMode::DirectWorld { name }
    } else {
        LaunchMode::Menu
    }
}

/// Reads world info for direct launch before opening the window.
/// Exits with error if missing or damaged.
pub fn direct_world_info(root: &Path, name: &str) -> Result<WorldInfo, String> {
    let rows =
        list_worlds(root).map_err(|e| format!("cannot list worlds in {}: {e}", root.display()))?;
    let info = rows
        .into_iter()
        .find(|w| w.name == name)
        .ok_or_else(|| format!("world '{name}' does not exist in {}", root.display()))?;
    if info.damaged {
        if terravibe_server::save::has_world_backup(root, name) {
            return Err(format!(
                "world '{name}' primary save is damaged (backup .bak available)"
            ));
        }
        return Err(format!("world '{name}' is damaged"));
    }
    Ok(info)
}

/// Checks whether a persisted world's primary `world.bin` is damaged before starting a session.
pub fn preflight_world_load(root: &Path, name: &str) -> Result<(), String> {
    let wb = root
        .join(terravibe_server::save::sanitize(name, "world1"))
        .join("world.bin");
    if !wb.exists() {
        return Ok(());
    }
    let rows = list_worlds(root).map_err(|e| format!("cannot inspect world '{name}': {e}"))?;
    if rows.iter().any(|w| w.name == name && w.damaged) {
        if terravibe_server::save::has_world_backup(root, name) {
            return Err(format!(
                "Primary save for '{name}' is damaged. Backup (.bak) is available — use Load backup."
            ));
        }
        return Err(format!(
            "Primary save for '{name}' is damaged and no backup is available."
        ));
    }
    Ok(())
}

pub static SERVER_THREAD: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

/// Joins the server thread at process exit.
pub fn join_server() {
    if let Some(h) = SERVER_THREAD.lock().unwrap().take() {
        let _ = h.join();
    }
}

pub struct SessionPlugin;

impl Plugin for SessionPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<SessionError>()
            .init_resource::<GameMenu>()
            .init_resource::<SelectedWorld>()
            .init_resource::<SelectedCharacter>()
            .add_systems(OnEnter(AppState::InGame), session_start)
            .add_systems(OnExit(AppState::InGame), session_teardown)
            .add_systems(
                Update,
                stopping_poll
                    .run_if(in_state(AppState::Menu))
                    .run_if(resource_exists::<SessionStopping>),
            )
            .add_systems(
                Update,
                retry_poll
                    .after(stopping_poll)
                    .run_if(in_state(AppState::Menu))
                    .run_if(resource_exists::<SessionRetryPending>),
            )
            .add_systems(
                Update,
                (loading_timeout, autosave_map).run_if(in_state(AppState::InGame)),
            );
    }
}

/// Builds the server configuration for a session.
///
/// Kept free of Bevy resources and environment reads so tests can pin the QA
/// flags. `qa_combat` is set only by the QA scenario marker; a world's name
/// never enables demo content.
pub(crate) fn session_cfg(
    name: &str,
    persist: bool,
    root: PathBuf,
    qa_combat: bool,
    qa_liquids: bool,
    qa_kill: bool,
    screenshot: bool,
    demo: Option<&str>,
) -> ServerCfg {
    let mut cfg = ServerCfg {
        clock_start: 13_500,
        save: persist.then_some(SaveCfg {
            root,
            world: name.to_string(),
        }),
        ..default()
    };
    if screenshot {
        cfg.natural_spawns = false;
    }
    if demo == Some("boss") {
        cfg.demo_boss = true;
    }
    if demo == Some("npc") {
        cfg.demo_npc = true;
        cfg.clock_start = terravibe_sim::time::DAY_TICKS / 2;
    }
    if qa_combat || demo == Some("enemies") {
        cfg.demo_enemies = true;
        cfg.clock_start = terravibe_sim::time::DAY_TICKS;
    }
    if qa_liquids || name == "qa-liquids" || demo == Some("liquids") {
        cfg.qa_liquids = true;
    }
    if qa_kill {
        cfg.qa_kill = true;
    }
    cfg
}

fn session_start(
    mut commands: Commands,
    mut session_error: ResMut<SessionError>,
    selected_world: Option<Res<SelectedWorld>>,
    selected_char: Option<Res<SelectedCharacter>>,
    save_root: Option<Res<SaveRoot>>,
    qa_combat_demo: Option<Res<crate::qa::QaCombatDemo>>,
    qa_liquids_demo: Option<Res<crate::qa::QaLiquidsDemo>>,
    qa_kill_demo: Option<Res<crate::qa::QaKillDemo>>,
) {
    let seed = selected_world.as_ref().map_or(SEED, |w| w.seed);
    let size = selected_world
        .as_ref()
        .map_or(terravibe_sim::WorldSize::Small, |w| w.size);
    let name = selected_world
        .as_ref()
        .map_or_else(|| "Default".to_string(), |w| w.name.clone());
    let persist = selected_world.as_ref().is_some_and(|w| w.persist);
    commands.remove_resource::<DisconnectState>();
    let root = save_root.as_ref().map_or_else(
        || SaveCfg::from_args_env(&[], |k| std::env::var(k).ok()).root,
        |r| r.0.clone(),
    );

    if persist {
        if let Err(msg) = preflight_world_load(&root, &name) {
            session_error.0 = Some(msg);
            return;
        }
        let last_char = selected_char
            .as_ref()
            .and_then(|sc| sc.0.as_ref().map(|c| c.name.clone()));
        let lp = crate::last_played::LastPlayed {
            last_character: last_char,
            last_world: Some(name.clone()),
        };
        let _ = lp.save(&root);
    }

    let demo = std::env::var("TERRAVIBE_DEMO").ok();
    let bench_scenario = std::env::var("TERRAVIBE_BENCH_SCENARIO").ok();
    let mut cfg = session_cfg(
        &name,
        persist,
        root.clone(),
        qa_combat_demo.is_some(),
        qa_liquids_demo.is_some(),
        qa_kill_demo.is_some(),
        std::env::var_os("TERRAVIBE_SCREENSHOT").is_some(),
        demo.as_deref(),
    );
    cfg.size = size;
    if matches!(
        bench_scenario.as_deref(),
        Some("liquid_stress") | Some("liquid-stress")
    ) {
        cfg.liquid_stress = true;
    }

    let (client, server) = spawn_local_cfg(seed, cfg);
    commands.insert_resource(Net::new(client));
    commands.insert_resource(SessionHandle::new(server));
    let p = terravibe_sim::player::Player::new([0.0, 0.0, 0.0]);
    let eye = crate::view::player::eye_of(&p);
    commands.insert_resource(LocalPlayer(p));
    commands.insert_resource(PlayerView {
        prev_eye: eye,
        eye,
        offset: Vec3::ZERO,
    });
    commands.insert_resource(LocalInventory(Inventory::starting(&mut Rng::new(
        seed as u64,
    ))));
    commands.insert_resource(LocalArmor([None, None, None]));
    commands.insert_resource(LocalAccessories::default());
    commands.insert_resource(LocalAccessoryPrefixes::default());
    commands.insert_resource(ClientClock::default());
    commands.insert_resource(NetState::default());
    commands.insert_resource(ChunkStream::default());

    let half_extent = size.half_extent();
    let map = if persist {
        let map_path = root
            .join(terravibe_server::save::sanitize(&name, "world1"))
            .join("map.bin");
        crate::hud::map::ExploredMap::load_from(&map_path, half_extent)
            .unwrap_or_else(|_| crate::hud::map::ExploredMap::new(half_extent))
    } else {
        crate::hud::map::ExploredMap::new(half_extent)
    };
    commands.insert_resource(map);
}

fn session_teardown(world: &mut World) {
    if let Some(net) = world.get_resource::<Net>() {
        let _ = net.send(ClientMsg::Bye);
    }
    world.remove_resource::<Net>();
    world.remove_resource::<LocalPlayer>();
    world.remove_resource::<PlayerView>();
    world.remove_resource::<LocalInventory>();
    world.remove_resource::<LocalArmor>();
    world.remove_resource::<LocalAccessories>();
    world.remove_resource::<LocalAccessoryPrefixes>();
    world.remove_resource::<ClientClock>();
    world.remove_resource::<NetState>();
    world.insert_resource(ChunkStream::default());
    if let Some(map) = world.remove_resource::<crate::hud::map::ExploredMap>() {
        let selected_world = world.get_resource::<SelectedWorld>();
        let save_root = world.get_resource::<SaveRoot>();
        let persist = selected_world.as_ref().is_some_and(|w| w.persist);
        if persist && map.dirty {
            let name = selected_world.map_or("Default", |w| &w.name);
            let root = save_root.map_or_else(
                || SaveCfg::from_args_env(&[], |k| std::env::var(k).ok()).root,
                |r| r.0.clone(),
            );
            let map_path = root
                .join(terravibe_server::save::sanitize(name, "world1"))
                .join("map.bin");
            let _ = map.save_to(&map_path);
        }
    }
    if let Some(mut handle) = world.remove_resource::<SessionHandle>() {
        if let Some(join) = handle.0.take() {
            world.insert_resource(SessionStopping::new(join));
        }
    }
    if let Some(mut boss) = world.get_resource_mut::<crate::hud::boss_bar::BossState>() {
        boss.clear();
    }
    if let Some(mut wof) = world.get_resource_mut::<crate::hud::boss_bar::WofBossState>() {
        wof.clear();
    }
    if let Some(mut tc) = world.get_resource_mut::<crate::hud::boss_bar::BossTitleCardState>() {
        *tc = Default::default();
    }
    if let Some(mut dm) = world.get_resource_mut::<crate::hud::death::DeathModalState>() {
        *dm = Default::default();
    }
    if let Some(mut victory) = world.get_resource_mut::<crate::hud::victory::VictoryModalState>() {
        victory.active = false;
        victory.fade_alpha = 0.0;
    }
    if let Some(mut cp) = world.get_resource_mut::<crate::hud::chest::ChestPanel>() {
        *cp = Default::default();
    }
    if let Some(mut tp) = world.get_resource_mut::<crate::hud::town::TownPanel>() {
        *tp = Default::default();
    }
    if let Some(mut pt) = world.get_resource_mut::<crate::hud::pickup_toast::PickupToastQueue>() {
        *pt = Default::default();
    }
    if let Some(mut dnp) = world.get_resource_mut::<crate::hud::combat_feedback::DamageNumberPool>()
    {
        *dnp = Default::default();
    }
    if let Some(mut hms) = world.get_resource_mut::<crate::hud::combat_feedback::HitMarkerState>() {
        *hms = Default::default();
    }
    if let Some(mut dds) =
        world.get_resource_mut::<crate::hud::combat_feedback::DamageDirectionState>()
    {
        *dds = Default::default();
    }
    if let Some(mut mus) = world.get_resource_mut::<crate::hud::map::MapUiState>() {
        *mus = Default::default();
    }
    if let Some(mut lh) = world.get_resource_mut::<crate::net::LocalHeld>() {
        *lh = Default::default();
    }
    if let Some(mut lgs) = world.get_resource_mut::<crate::net::LocalGuideSlot>() {
        *lgs = Default::default();
    }
    if let Some(mut lcf) = world.get_resource_mut::<crate::net::LocalCharacterFacts>() {
        *lcf = Default::default();
    }
    if let Some(mut cf) = world.get_resource_mut::<crate::net::CharacterFacts>() {
        *cf = Default::default();
    }
    if let Some(mut lwf) = world.get_resource_mut::<crate::net::LocalWorldFacts>() {
        *lwf = Default::default();
    }
    if let Some(mut gas) = world.get_resource_mut::<crate::menu::game::GuideAdviceState>() {
        *gas = Default::default();
    }
    if let Some(mut hf) = world.get_resource_mut::<crate::hud::HurtFlash>() {
        *hf = Default::default();
    }
    if let Some(mut io) = world.get_resource_mut::<crate::hud::inventory::InventoryOpen>() {
        *io = Default::default();
    }
    if let Some(mut to) = world.get_resource_mut::<crate::hud::tooltip::TooltipOverlay>() {
        *to = Default::default();
    }
    world.insert_resource(SessionError(None));
    world.remove_resource::<DisconnectState>();
    world.insert_resource(GameMenu { panel: None });
}

fn stopping_poll(mut commands: Commands, stopping: Res<SessionStopping>) {
    if stopping.0.as_ref().is_some_and(|h| h.is_finished()) {
        commands.remove_resource::<SessionStopping>();
    }
}

fn retry_poll(
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
    stopping: Option<Res<SessionStopping>>,
) {
    if stopping.is_none() {
        commands.remove_resource::<SessionRetryPending>();
        next_state.set(AppState::InGame);
    }
}

fn loading_timeout(
    time: Res<Time<Real>>,
    mut elapsed: Local<f32>,
    net_state: Option<Res<NetState>>,
    mut error: ResMut<SessionError>,
) {
    if let Some(st) = net_state {
        if st.is_added() {
            *elapsed = 0.0;
        }
        if st.is_ready() {
            *elapsed = 0.0;
            return;
        }
    }
    *elapsed += time.delta_secs();
    if *elapsed >= 30.0 && error.0.is_none() {
        error.0 = Some("session loading timed out after 30s".into());
    }
}

fn autosave_map(
    time: Res<Time>,
    mut timer: Local<f32>,
    selected_world: Option<Res<SelectedWorld>>,
    save_root: Option<Res<SaveRoot>>,
    map: Option<ResMut<crate::hud::map::ExploredMap>>,
) {
    let Some(mut map) = map else { return };
    *timer += time.delta_secs();
    if *timer >= 30.0 {
        *timer = 0.0;
        let persist = selected_world.as_ref().is_some_and(|w| w.persist);
        if persist && map.dirty {
            let name = selected_world.as_ref().map_or("Default", |w| &w.name);
            let root = save_root.as_ref().map_or_else(
                || SaveCfg::from_args_env(&[], |k| std::env::var(k).ok()).root,
                |r| r.0.clone(),
            );
            let map_path = root
                .join(terravibe_server::save::sanitize(name, "world1"))
                .join("map.bin");
            if map.save_to(&map_path).is_ok() {
                map.dirty = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use std::sync::Arc;
    use std::time::Duration;
    use terravibe_server::{SaveCfg, create_world};
    use terravibe_sim::net::in_process_pair;
    use terravibe_sim::{Chunk, ChunkPos};

    fn tmp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "tv-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn test_app(root: PathBuf) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .insert_resource(SaveRoot(root))
            .add_plugins(SessionPlugin)
            .add_plugins(crate::net::NetPlugin);
        app
    }

    fn set_state(app: &mut App, state: AppState) {
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(state);
    }

    /// Updates the app until the transport is ready, bounded by wall-clock time
    /// rather than a fixed update count so the in-process server's world
    /// generation cannot outrun a tight spin loop on a busy machine.
    fn wait_until_ready(app: &mut App) {
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while !app
            .world()
            .get_resource::<NetState>()
            .is_some_and(|st| st.is_ready())
        {
            assert!(
                std::time::Instant::now() < deadline,
                "transport never became ready within 30s"
            );
            app.update();
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    fn session_start_inserts_and_teardown_removes() {
        let root = tmp();
        let mut app = test_app(root.clone());
        app.insert_resource(SelectedWorld {
            name: "w".into(),
            seed: 7,
            persist: true,
            ..Default::default()
        });
        set_state(&mut app, AppState::InGame);
        app.update();
        app.update();
        app.update();
        assert!(app.world().contains_resource::<Net>());
        assert!(app.world().contains_resource::<LocalPlayer>());
        assert!(app.world().contains_resource::<LocalInventory>());
        assert!(app.world().contains_resource::<ClientClock>());
        assert!(root.join("w").is_dir());

        set_state(&mut app, AppState::Menu);
        app.update();
        assert!(!app.world().contains_resource::<Net>());
        assert!(!app.world().contains_resource::<LocalPlayer>());
        assert!(app.world().resource::<ChunkStream>().generated().is_empty());
        assert!(app.world().contains_resource::<SessionStopping>());
    }

    #[test]
    fn session_ignores_persist_false() {
        let root = tmp();
        let mut app = test_app(root.clone());
        app.insert_resource(SelectedWorld {
            name: "w".into(),
            seed: 7,
            persist: false,
            ..Default::default()
        });
        set_state(&mut app, AppState::InGame);
        app.update();
        assert!(!root.join("w").exists());
        assert!(app.world().contains_resource::<Net>());
    }

    #[test]
    fn save_and_quit_waits_for_final_write() {
        let root = tmp();
        let mut app = test_app(root.clone());
        app.insert_resource(SelectedWorld {
            name: "w".into(),
            seed: 7,
            persist: true,
            ..Default::default()
        });
        set_state(&mut app, AppState::InGame);
        wait_until_ready(&mut app);
        set_state(&mut app, AppState::Menu);
        app.update();
        let mut spins = 0;
        while app.world().contains_resource::<SessionStopping>() {
            app.update();
            spins += 1;
            assert!(spins < 10_000, "server never finished");
        }
        let rows = terravibe_server::list_worlds(&root).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "w");
        assert_eq!(rows[0].seed, 7);
        assert!(!rows[0].damaged);
    }

    #[test]
    fn teardown_resets_chunk_stream() {
        let root = tmp();
        let mut app = test_app(root);
        app.insert_resource(SelectedWorld {
            name: "w".into(),
            seed: 7,
            persist: false,
            ..Default::default()
        });
        set_state(&mut app, AppState::InGame);
        app.update();
        let chunk = Arc::new(Chunk::filled(terravibe_sim::DIRT));
        app.world_mut()
            .resource_mut::<ChunkStream>()
            .insert_chunk(ChunkPos { x: 0, y: 0, z: 0 }, chunk);
        assert!(!app.world().resource::<ChunkStream>().generated().is_empty());
        set_state(&mut app, AppState::Menu);
        app.update();
        assert!(app.world().resource::<ChunkStream>().generated().is_empty());
    }

    #[test]
    fn world_named_qa_combat_has_no_demo_enemies() {
        let cfg = session_cfg(
            "qa-combat",
            false,
            PathBuf::from("/tmp/terravibe-qa-combat"),
            false,
            false,
            false,
            false,
            None,
        );
        assert!(
            !cfg.demo_enemies,
            "a world named qa-combat must not enable demo enemies"
        );
        assert_eq!(
            cfg.clock_start, 13_500,
            "a world named qa-combat must not move the clock"
        );
        assert!(cfg.save.is_none(), "persist=false must not save");
        assert!(cfg.natural_spawns, "natural spawns must stay on");
    }

    #[test]
    fn demo_enemies_env_flag_still_enables_demo_enemies() {
        let cfg = session_cfg(
            "any-world",
            false,
            PathBuf::from("/tmp/terravibe-demo-enemies"),
            false,
            false,
            false,
            false,
            Some("enemies"),
        );
        assert!(cfg.demo_enemies, "TERRAVIBE_DEMO=enemies must opt in");
        assert_eq!(cfg.clock_start, terravibe_sim::time::DAY_TICKS);
    }

    #[test]
    fn qa_kill_flag_is_wired_into_server_cfg() {
        let cfg = session_cfg(
            "qa-first_hour",
            false,
            PathBuf::from("/tmp/terravibe-qa-first-hour"),
            false,
            false,
            true,
            false,
            None,
        );
        assert!(cfg.qa_kill, "the QA kill marker must enable qa_kill");

        let default_cfg = session_cfg(
            "any-world",
            false,
            PathBuf::from("/tmp/terravibe-no-qa-kill"),
            false,
            false,
            false,
            false,
            None,
        );
        assert!(!default_cfg.qa_kill, "qa_kill must default to false");
    }

    #[test]
    fn launch_mode_dispatch() {
        use LaunchMode::*;
        assert!(matches!(launch_mode(true, None), Automation));
        assert!(matches!(
            launch_mode(false, Some("Forest".into())),
            DirectWorld { .. }
        ));
        assert!(matches!(launch_mode(false, None), Menu));
        let root = tmp();
        let cfg = SaveCfg::from_args_env(
            &["--save-dir".into(), root.to_string_lossy().into()],
            |_| None,
        );
        assert_eq!(cfg.root, root);
    }

    #[test]
    fn direct_world_check_fails_loudly_before_window() {
        let root = tmp();
        assert!(direct_world_info(&root, "missing").is_err());
        create_world(&root, "broken", 1, terravibe_sim::WorldSize::Small).unwrap();
        std::fs::write(root.join("broken/world.bin"), b"corrupted bytes").unwrap();
        assert!(direct_world_info(&root, "broken").is_err());
        create_world(&root, "ok", 5, terravibe_sim::WorldSize::Small).unwrap();
        assert_eq!(direct_world_info(&root, "ok").unwrap().seed, 5);
    }

    #[test]
    fn pump_fail_fast_only_under_automation() {
        // (a) without Automation: disconnect the transport -> SessionError(Some(_)) set, app still runs;
        // (b) with Automation: disconnect -> AppExit::Error(_) observed.
        let root = tmp();
        let mut app = test_app(root.clone());
        app.insert_resource(SelectedWorld {
            name: "w".into(),
            seed: 7,
            persist: false,
            ..Default::default()
        });
        set_state(&mut app, AppState::InGame);
        app.update();

        let (client, conn) = in_process_pair();
        drop(conn); // Server end is dropped -> transport returns Disconnected
        app.insert_resource(Net::new(client));
        app.update();
        assert!(app.world().resource::<SessionError>().0.is_some());
        assert!(!app.world().resource::<NetState>().is_ready());

        // (b) With Automation
        let mut auto_app = test_app(root);
        auto_app.insert_resource(Automation);
        auto_app.insert_resource(SelectedWorld {
            name: "w".into(),
            seed: 7,
            persist: false,
            ..Default::default()
        });
        set_state(&mut auto_app, AppState::InGame);
        auto_app.update();

        let (client, conn) = in_process_pair();
        drop(conn);
        auto_app.insert_resource(Net::new(client));
        auto_app.update();
        let exits = auto_app.world().resource::<Messages<AppExit>>();
        assert!(!exits.is_empty(), "AppExit was expected under automation");
    }

    #[test]
    fn app_exit_during_session_saves_world() {
        let root = tmp();
        let mut app = test_app(root.clone());
        app.insert_resource(SelectedWorld {
            name: "w".into(),
            seed: 7,
            persist: true,
            ..Default::default()
        });
        set_state(&mut app, AppState::InGame);
        for _ in 0..5 {
            app.update();
        }
        drop(app);
        join_server();
        assert!(root.join("w/world.bin").exists());
    }

    #[test]
    fn app_exit_while_stopping_joins_server_thread() {
        let root = tmp();
        let mut app = test_app(root.clone());
        app.insert_resource(SelectedWorld {
            name: "w".into(),
            seed: 7,
            persist: true,
            ..Default::default()
        });
        set_state(&mut app, AppState::InGame);
        for _ in 0..5 {
            app.update();
        }
        // Leave to Menu: session_teardown moves handle to SessionStopping
        set_state(&mut app, AppState::Menu);
        app.update();
        assert!(app.world().contains_resource::<SessionStopping>());
        // Drop app while stopping is active
        drop(app);
        // join_server must find and join the thread
        join_server();
        assert!(root.join("w/world.bin").exists());
    }

    #[test]
    fn teardown_clears_boss_state() {
        let root = tmp();
        let mut app = test_app(root);
        app.insert_resource(SelectedWorld {
            name: "w".into(),
            seed: 7,
            persist: false,
            ..Default::default()
        });
        set_state(&mut app, AppState::InGame);
        app.update();
        app.world_mut()
            .resource_mut::<crate::hud::boss_bar::BossState>()
            .set(crate::hud::boss_bar::BossInfo {
                pos: [0.0; 3],
                vel: [0.0; 3],
                life: 100,
                life_max: 100,
                phase: 1,
                state: 0,
                spin: 0.0,
            });
        assert!(
            app.world()
                .resource::<crate::hud::boss_bar::BossState>()
                .0
                .is_some()
        );
        app.world_mut()
            .insert_resource(crate::hud::boss_bar::WofBossState(Some(
                crate::hud::boss_bar::WofBossInfo {
                    wall_x: 0.0,
                    dir: 1,
                    life: 5000,
                    life_max: 8000,
                },
            )));
        app.world_mut()
            .insert_resource(crate::hud::victory::VictoryModalState {
                active: true,
                fade_alpha: 1.0,
            });
        set_state(&mut app, AppState::Menu);
        app.update();
        assert!(
            app.world()
                .resource::<crate::hud::boss_bar::BossState>()
                .0
                .is_none()
        );
        assert!(
            app.world()
                .resource::<crate::hud::boss_bar::WofBossState>()
                .0
                .is_none()
        );
        assert!(
            !app.world()
                .resource::<crate::hud::victory::VictoryModalState>()
                .active
        );
    }

    #[test]
    fn session_persists_and_reloads_map() {
        let root = tmp();
        let mut app = test_app(root.clone());
        app.insert_resource(SelectedWorld {
            name: "test_map_world".into(),
            seed: 7,
            persist: true,
            ..Default::default()
        });
        set_state(&mut app, AppState::InGame);
        app.update();

        // Mark cell explored
        app.world_mut()
            .resource_mut::<crate::hud::map::ExploredMap>()
            .set_cell(5, 5, true, [100, 150, 200]);

        // Exit session
        set_state(&mut app, AppState::Menu);
        app.update();

        assert!(root.join("test_map_world/map.bin").exists());

        // Re-enter session
        set_state(&mut app, AppState::InGame);
        app.update();

        let map = app.world().resource::<crate::hud::map::ExploredMap>();
        assert!(map.is_explored(5 * 4, 5 * 4));
        assert_eq!(map.cell_color(5 * 4, 5 * 4), [100, 150, 200]);

        set_state(&mut app, AppState::Menu);
        app.update();
        drop(app);
        join_server();
    }

    #[test]
    fn leaving_ingame_leaves_no_stale_entities() {
        use crate::view::camera::FlyCamera;
        use crate::view::drops::ClientDrop;
        use crate::view::enemies::ClientEnemy;

        let root = tmp();
        let mut app = test_app(root.clone());
        app.insert_resource(SelectedWorld {
            name: "w".into(),
            seed: 7,
            persist: true,
            ..Default::default()
        });
        set_state(&mut app, AppState::InGame);
        wait_until_ready(&mut app);

        // Spawn instances of the gameplay entities to verify real cleanup
        let cam = app
            .world_mut()
            .spawn((
                FlyCamera {
                    yaw: 0.0,
                    pitch: 0.0,
                },
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let chunk_mesh = app
            .world_mut()
            .spawn((Mesh3d::default(), DespawnOnExit(AppState::InGame)))
            .id();
        let enemy = app
            .world_mut()
            .spawn((
                ClientEnemy::new(
                    terravibe_sim::enemy::EnemyId(1),
                    terravibe_sim::enemy::EnemyKind::Zombie,
                ),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let drop = app
            .world_mut()
            .spawn((
                ClientDrop {
                    id: terravibe_sim::drop::DropId(1),
                    stack: terravibe_sim::inventory::ItemStack {
                        id: terravibe_sim::item::WOODEN_ARROW,
                        count: 5,
                        prefix: terravibe_sim::item::Prefix(0),
                    },
                },
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let hud_node = app
            .world_mut()
            .spawn((Node::default(), DespawnOnExit(AppState::InGame)))
            .id();

        // Verify they exist while InGame
        assert!(app.world().get_entity(cam).is_ok());
        assert!(app.world().get_entity(chunk_mesh).is_ok());
        assert!(app.world().get_entity(enemy).is_ok());
        assert!(app.world().get_entity(drop).is_ok());
        assert!(app.world().get_entity(hud_node).is_ok());

        // Leave to Menu
        set_state(&mut app, AppState::Menu);
        app.update();

        // Verify zero gameplay entities remain
        assert!(
            app.world().get_entity(cam).is_err(),
            "FlyCamera was not despawned"
        );
        assert!(
            app.world().get_entity(chunk_mesh).is_err(),
            "chunk mesh was not despawned"
        );
        assert!(
            app.world().get_entity(enemy).is_err(),
            "enemy was not despawned"
        );
        assert!(
            app.world().get_entity(drop).is_err(),
            "drop was not despawned"
        );
        assert!(
            app.world().get_entity(hud_node).is_err(),
            "hud node was not despawned"
        );

        let mut q_cam = app.world_mut().query_filtered::<Entity, With<FlyCamera>>();
        assert_eq!(q_cam.iter(app.world()).count(), 0);
        let mut q_enemy = app
            .world_mut()
            .query_filtered::<Entity, With<ClientEnemy>>();
        assert_eq!(q_enemy.iter(app.world()).count(), 0);
        let mut q_drop = app.world_mut().query_filtered::<Entity, With<ClientDrop>>();
        assert_eq!(q_drop.iter(app.world()).count(), 0);
        let mut q_node = app.world_mut().query_filtered::<Entity, With<Node>>();
        assert_eq!(q_node.iter(app.world()).count(), 0);
        let mut q_mesh = app.world_mut().query_filtered::<Entity, With<Mesh3d>>();
        assert_eq!(q_mesh.iter(app.world()).count(), 0);

        assert!(!app.world().contains_resource::<Net>());
    }

    #[test]
    fn loading_timeout_sets_session_error() {
        let root = tmp();
        let mut app = test_app(root);
        app.insert_resource(SelectedWorld {
            name: "w".into(),
            seed: 7,
            persist: false,
            ..Default::default()
        });
        set_state(&mut app, AppState::InGame);
        app.update();
        // Stalled transport: never receives chunks
        let (client, _conn) = in_process_pair();
        app.insert_resource(Net::new(client));
        app.insert_resource(NetState::default());
        // Fast forward 31 seconds
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs(31)));
        app.update();
        assert!(app.world().resource::<SessionError>().0.is_some());
    }

    #[test]
    fn session_start_persists_last_played() {
        let root = tmp();
        let mut app = test_app(root.clone());
        let mut char_data = terravibe_sim::Character::default();
        char_data.name = "Hero123".to_string();
        app.insert_resource(SelectedCharacter(Some(char_data)));
        app.insert_resource(SelectedWorld {
            name: "World456".into(),
            seed: 7,
            persist: true,
            ..Default::default()
        });
        set_state(&mut app, AppState::InGame);
        app.update();

        let lp = crate::last_played::LastPlayed::load(&root);
        assert_eq!(lp.last_character.as_deref(), Some("Hero123"));
        assert_eq!(lp.last_world.as_deref(), Some("World456"));
    }

    #[test]
    fn corrupt_primary_with_backup_sets_clear_session_error() {
        let root = tmp();
        let dir = terravibe_server::save::SaveDir::open(&root, "broken").unwrap();
        let snap = terravibe_server::save::WorldSnapshot {
            seed: 42,
            clock_tick: 100,
            size: terravibe_sim::WorldSize::Small,
            chunks: vec![],
            players: vec![],
            chests: vec![],
            towns: Default::default(),
            facts: Default::default(),
            weather: Default::default(),
            locked_chests: vec![],
            shadow_chests: vec![],
        };
        dir.save_world(&snap).unwrap();
        dir.save_world(&snap).unwrap();
        std::fs::write(dir.path().join("world.bin"), b"corrupt!").unwrap();

        let mut app = test_app(root);
        app.insert_resource(SelectedWorld {
            name: "broken".into(),
            seed: 42,
            persist: true,
            ..Default::default()
        });
        set_state(&mut app, AppState::InGame);
        app.update();

        let err = app
            .world()
            .resource::<SessionError>()
            .0
            .as_deref()
            .unwrap_or("");
        assert!(
            err.contains("backup") || err.contains(".bak"),
            "SessionError should mention backup availability, got: {err:?}"
        );
    }

    #[test]
    fn disconnect_sets_readable_reason_and_offers_retry() {
        let root = tmp();
        let mut app = test_app(root);
        app.insert_resource(SelectedWorld {
            name: "w".into(),
            seed: 7,
            persist: false,
            ..Default::default()
        });
        set_state(&mut app, AppState::InGame);
        app.update();

        let (client, conn) = in_process_pair();
        drop(conn);
        app.world_mut().remove_resource::<SessionHandle>();
        app.insert_resource(Net::new(client));
        app.update();

        let disconnect = app
            .world()
            .get_resource::<DisconnectState>()
            .expect("DisconnectState must be inserted on disconnect");
        assert!(
            !disconnect.reason.trim().is_empty(),
            "disconnect reason must be non-empty"
        );
        assert!(
            disconnect.reason.to_lowercase().contains("disconnect")
                || disconnect.reason.to_lowercase().contains("connection"),
            "disconnect reason should be readable, got: {:?}",
            disconnect.reason
        );

        // Retry flow: inserting SessionRetryPending and returning to Menu re-enters InGame.
        app.world_mut().insert_resource(SessionRetryPending);
        set_state(&mut app, AppState::Menu);
        app.update();
        for _ in 0..200 {
            app.update();
            if *app.world().resource::<State<AppState>>().get() == AppState::InGame {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::InGame
        );
        assert!(
            app.world().get_resource::<DisconnectState>().is_none(),
            "DisconnectState must be cleared on retry"
        );
    }

    #[test]
    fn session_teardown_resets_all_hud_and_game_state_resources() {
        let mut world = World::new();
        world.insert_resource(crate::hud::death::DeathModalState {
            active: true,
            coins_lost: 100,
            ..Default::default()
        });
        world.insert_resource(crate::hud::boss_bar::BossTitleCardState {
            active: true,
            was_boss_alive: true,
            timer: 2.0,
        });
        world.insert_resource(crate::hud::chest::ChestPanel {
            open: Some([1, 2, 3]),
            slots: vec![None; 40],
        });
        world.insert_resource(crate::hud::town::TownPanel {
            text: "Hello".into(),
            ..Default::default()
        });
        world.insert_resource(crate::hud::map::MapUiState {
            fullscreen_visible: true,
            dungeon_marker: Some([10.0, 20.0]),
            ..Default::default()
        });
        world.insert_resource(crate::net::LocalWorldFacts(terravibe_sim::WorldFacts {
            downed_eye: true,
            ..Default::default()
        }));
        world.insert_resource(crate::hud::HurtFlash { ticks: 0.8 });

        session_teardown(&mut world);

        assert!(
            !world
                .resource::<crate::hud::death::DeathModalState>()
                .active
        );
        assert!(
            !world
                .resource::<crate::hud::boss_bar::BossTitleCardState>()
                .active
        );
        assert!(
            !world
                .resource::<crate::hud::boss_bar::BossTitleCardState>()
                .was_boss_alive
        );
        assert!(
            world
                .resource::<crate::hud::chest::ChestPanel>()
                .open
                .is_none()
        );
        assert!(
            world
                .resource::<crate::hud::town::TownPanel>()
                .text
                .is_empty()
        );
        assert!(
            !world
                .resource::<crate::hud::map::MapUiState>()
                .fullscreen_visible
        );
        assert!(
            world
                .resource::<crate::hud::map::MapUiState>()
                .dungeon_marker
                .is_none()
        );
        assert!(!world.resource::<crate::net::LocalWorldFacts>().0.downed_eye);
        assert_eq!(world.resource::<crate::hud::HurtFlash>().ticks, 0.0);
    }
}
