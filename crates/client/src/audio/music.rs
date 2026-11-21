//! Situational music playback, crossfading, volume control and debug logging.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::audio::AudioSinkPlayback;
use bevy::prelude::*;
use terravibe_assets::TerrariaContent;
use terravibe_sim::worldgen::biome::Biome;

/// Situational music state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Situation {
    Title,
    Day,
    Night,
    Underground,
    Forest,
    Desert,
    Snow,
    Jungle,
    Corruption,
    Boss,
    Underworld,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MusicContext {
    pub in_menu: bool,
    pub boss: bool,
    pub depth: i32,
    pub night: bool,
    pub biome: Biome,
    pub corruption: bool,
    pub underworld: bool,
}

/// Priority: Title > Boss > Underworld > Corruption > Underground > Biome(Desert|Snow|Jungle) > Night > Day
pub fn choose(c: &MusicContext) -> Situation {
    if c.in_menu {
        Situation::Title
    } else if c.boss {
        Situation::Boss
    } else if c.underworld {
        Situation::Underworld
    } else if c.corruption {
        Situation::Corruption
    } else if c.depth > 0 {
        Situation::Underground
    } else {
        match c.biome {
            Biome::Desert => Situation::Desert,
            Biome::Snow => Situation::Snow,
            Biome::Jungle => Situation::Jungle,
            Biome::Forest => {
                if c.night {
                    Situation::Night
                } else {
                    Situation::Day
                }
            }
        }
    }
}

pub fn track_for(s: Situation) -> &'static str {
    match s {
        Situation::Title => "title",
        Situation::Day | Situation::Forest => "day",
        Situation::Night => "night",
        Situation::Underground => "underground",
        Situation::Desert => "desert",
        Situation::Snow => "snow",
        Situation::Jungle => "jungle",
        Situation::Corruption => "corruption",
        Situation::Boss => "boss1",
        Situation::Underworld => "underworld",
    }
}

pub struct Fade {
    pub from: Option<Entity>,
    pub to: Option<Entity>,
    pub t: f32,
}

/// 2-second linear crossfade: returns (from_gain, to_gain) where t in [0.0, 1.0].
pub fn fade_gains(t: f32) -> (f32, f32) {
    let clamped = t.clamp(0.0, 1.0);
    (1.0 - clamped, clamped)
}

pub fn music_gain(music_volume: u8) -> f32 {
    music_volume as f32 / 100.0
}

pub fn music_gain_for_settings(settings: &crate::settings::Settings, focused: bool) -> f32 {
    if settings.mute_on_focus_loss && !focused {
        return 0.0;
    }
    music_gain(settings.music_volume)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MusicAction {
    PlayImmediate(Situation),
    Crossfade { from: Situation, to: Situation },
}

#[derive(Debug, Default)]
pub struct MusicStateMachine {
    pub current: Option<Situation>,
    pub candidate: Option<Situation>,
    pub candidate_timer: f32,
    pub active_fade: Option<f32>,
}

impl MusicStateMachine {
    pub fn update(&mut self, dt: f32, desired: Situation) -> Option<MusicAction> {
        let Some(current) = self.current else {
            self.current = Some(desired);
            return Some(MusicAction::PlayImmediate(desired));
        };

        if track_for(desired) == track_for(current) {
            self.current = Some(desired);
            self.candidate = None;
            self.candidate_timer = 0.0;
            return None;
        }

        if Some(desired) == self.candidate {
            self.candidate_timer += dt;
            if self.candidate_timer >= 1.5 {
                let from = current;
                self.current = Some(desired);
                self.candidate = None;
                self.candidate_timer = 0.0;
                self.active_fade = Some(0.0);
                return Some(MusicAction::Crossfade { from, to: desired });
            }
        } else {
            self.candidate = Some(desired);
            self.candidate_timer = dt;
        }

        None
    }
}

#[derive(Component)]
pub struct MusicPlayer {
    pub situation: Situation,
    pub volume_factor: f32,
}

#[derive(Resource)]
pub struct MusicManager {
    pub state: MusicStateMachine,
    pub content: Option<Arc<TerrariaContent>>,
    pub loaded_tracks: HashMap<&'static str, Option<Handle<AudioSource>>>,
    pub current_entity: Option<Entity>,
    pub fading_entity: Option<Entity>,
    pub debug_logging: bool,
    pub debug_timer: f32,
}

pub struct MusicPlugin {
    pub content: Option<Arc<TerrariaContent>>,
    pub debug_logging: bool,
}

impl Plugin for MusicPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<crate::audio::AudioFocusState>()
            .insert_resource(MusicManager {
                state: MusicStateMachine::default(),
                content: self.content.clone(),
                loaded_tracks: HashMap::new(),
                current_entity: None,
                fading_entity: None,
                debug_logging: self.debug_logging,
                debug_timer: 0.0,
            })
            .add_systems(
                Update,
                (crate::audio::update_audio_focus, update_situational_music).chain(),
            );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update_situational_music(
    mut commands: Commands,
    time: Res<Time>,
    mut manager: ResMut<MusicManager>,
    settings: Option<Res<crate::settings::Settings>>,
    focus: Option<Res<crate::audio::AudioFocusState>>,
    app_state: Option<Res<State<crate::session::AppState>>>,
    selected_world: Option<Res<crate::session::SelectedWorld>>,
    clock: Option<Res<crate::net::ClientClock>>,
    boss_bar: Option<Res<crate::hud::boss_bar::BossState>>,
    wof_bar: Option<Res<crate::hud::boss_bar::WofBossState>>,
    enemies: Query<&crate::view::enemies::ClientEnemy>,
    local_player: Option<Res<crate::view::player::LocalPlayer>>,
    mut sinks: Query<(&mut AudioSink, &mut MusicPlayer)>,
    mut audio_assets: ResMut<Assets<AudioSource>>,
) {
    use terravibe_sim::enemy::{EnemyKind, is_eater_of_worlds};
    let dt = time.delta_secs();

    let in_menu = app_state
        .as_ref()
        .map_or(true, |s| s.get() == &crate::session::AppState::Menu);
    let enemy_boss = enemies
        .iter()
        .any(|e| e.hp > 0 && (e.kind == EnemyKind::SkeletronHead || is_eater_of_worlds(e.kind)));
    let boss = boss_bar.as_ref().is_some_and(|b| b.0.is_some())
        || wof_bar.as_ref().is_some_and(|w| w.0.is_some())
        || enemy_boss;
    let night = clock.as_ref().map_or(false, |c| {
        !terravibe_sim::time::WorldClock {
            tick: c.effective_tick(),
        }
        .is_day()
    });

    let seed = selected_world
        .as_ref()
        .map_or(crate::view::SEED, |w| w.seed);
    let world_size = selected_world
        .as_ref()
        .map_or(terravibe_sim::WorldSize::Small, |w| w.size);
    let (biome, depth, corruption, underworld) = if let Some(lp) = &local_player {
        let p = lp.0.body.pos;
        let (wx, wy, wz) = (
            p[0].floor() as i32,
            p[1].floor() as i32,
            p[2].floor() as i32,
        );
        let surface =
            terravibe_sim::worldgen::terrain::surface_height_for_size(seed, wx, wz, world_size);
        let depth = (surface - wy) - 15;
        let w = terravibe_sim::worldgen::biome::biome_weights_for_size(seed, wx, wz, world_size);
        let b = terravibe_sim::worldgen::biome::dominant(&w);
        let corruption =
            terravibe_sim::worldgen::corruption::corruption_at_for_size(seed, wx, wz, world_size)
                > 0.0;
        let underworld = p[1] <= terravibe_sim::worldgen::liquid::LAVA_DEPTH_Y as f32;
        (b, depth, corruption, underworld)
    } else {
        (Biome::Forest, 0, false, false)
    };

    let ctx = MusicContext {
        in_menu,
        boss,
        depth,
        night,
        biome,
        corruption,
        underworld,
    };

    let desired = choose(&ctx);
    let focused = focus.as_ref().is_none_or(|f| f.focused);
    let m_gain = settings
        .as_deref()
        .map_or(0.5, |s| music_gain_for_settings(s, focused));

    if manager.debug_logging {
        manager.debug_timer += dt;
        if manager.debug_timer >= 1.0 {
            manager.debug_timer = 0.0;
            info!(
                "music: situation={:?} desired={:?} fade={:?}",
                manager.state.current, desired, manager.state.active_fade
            );
        }
    }

    if let Some(action) = manager.state.update(dt, desired) {
        match action {
            MusicAction::PlayImmediate(sit) => {
                let track = track_for(sit);
                let handle = get_or_load_track(&mut manager, track, &mut audio_assets);
                if let Some(h) = handle {
                    let e = commands
                        .spawn((
                            AudioPlayer(h),
                            PlaybackSettings::LOOP.with_volume(bevy::audio::Volume::Linear(m_gain)),
                            MusicPlayer {
                                situation: sit,
                                volume_factor: 1.0,
                            },
                        ))
                        .id();
                    manager.current_entity = Some(e);
                }
            }
            MusicAction::Crossfade { from: _, to } => {
                if let Some(old) = manager.current_entity.take() {
                    manager.fading_entity = Some(old);
                }
                let track = track_for(to);
                let handle = get_or_load_track(&mut manager, track, &mut audio_assets);
                if let Some(h) = handle {
                    let e = commands
                        .spawn((
                            AudioPlayer(h),
                            PlaybackSettings::LOOP.with_volume(bevy::audio::Volume::Linear(0.0)),
                            MusicPlayer {
                                situation: to,
                                volume_factor: 0.0,
                            },
                        ))
                        .id();
                    manager.current_entity = Some(e);
                }
            }
        }
    }

    // Handle crossfade progression
    if let Some(mut fade_t) = manager.state.active_fade {
        fade_t += dt / 2.0; // 2-second fade
        let (from_vol, to_vol) = fade_gains(fade_t);

        if let Some(from_e) = manager.fading_entity {
            if let Ok((mut sink, mut player)) = sinks.get_mut(from_e) {
                player.volume_factor = from_vol;
                sink.set_volume(bevy::audio::Volume::Linear(m_gain * from_vol));
            }
        }

        if let Some(to_e) = manager.current_entity {
            if let Ok((mut sink, mut player)) = sinks.get_mut(to_e) {
                player.volume_factor = to_vol;
                sink.set_volume(bevy::audio::Volume::Linear(m_gain * to_vol));
            }
        }

        if fade_t >= 1.0 {
            manager.state.active_fade = None;
            if let Some(from_e) = manager.fading_entity.take() {
                if let Ok((sink, _)) = sinks.get_mut(from_e) {
                    sink.stop();
                }
                commands.entity(from_e).despawn();
            }
        } else {
            manager.state.active_fade = Some(fade_t);
        }
    } else {
        // Not fading: keep current track volume in sync with settings
        if let Some(to_e) = manager.current_entity {
            if let Ok((mut sink, mut player)) = sinks.get_mut(to_e) {
                player.volume_factor = 1.0;
                sink.set_volume(bevy::audio::Volume::Linear(m_gain));
            }
        }
    }
}

fn get_or_load_track(
    manager: &mut MusicManager,
    stem: &'static str,
    audio_assets: &mut Assets<AudioSource>,
) -> Option<Handle<AudioSource>> {
    if let Some(handle) = manager.loaded_tracks.get(stem) {
        return handle.clone();
    }

    let Some(content) = &manager.content else {
        manager.loaded_tracks.insert(stem, None);
        return None;
    };

    match content.music(stem) {
        Ok(bytes) => {
            let handle = audio_assets.add(AudioSource {
                bytes: bytes.into(),
            });
            manager.loaded_tracks.insert(stem, Some(handle.clone()));
            Some(handle)
        }
        Err(e) => {
            warn!("music track '{stem}' not available: {e}");
            manager.loaded_tracks.insert(stem, None);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choose_priority_boss_over_everything_but_title() {
        // Title beats boss
        let c_title = MusicContext {
            in_menu: true,
            boss: true,
            depth: 0,
            night: false,
            biome: Biome::Forest,
            corruption: true,
            underworld: false,
        };
        assert_eq!(choose(&c_title), Situation::Title);

        // Boss beats corruption, underground, night, biome
        let c_boss = MusicContext {
            in_menu: false,
            boss: true,
            depth: 50,
            night: true,
            biome: Biome::Jungle,
            corruption: true,
            underworld: false,
        };
        assert_eq!(choose(&c_boss), Situation::Boss);
    }

    #[test]
    fn choose_underground_beats_biome_and_night() {
        let c = MusicContext {
            in_menu: false,
            boss: false,
            depth: 20,
            night: true,
            biome: Biome::Desert,
            corruption: false,
            underworld: false,
        };
        assert_eq!(choose(&c), Situation::Underground);
    }

    #[test]
    fn choose_corruption_over_biome() {
        let c = MusicContext {
            in_menu: false,
            boss: false,
            depth: 0,
            night: false,
            biome: Biome::Snow,
            corruption: true,
            underworld: false,
        };
        assert_eq!(choose(&c), Situation::Corruption);
    }

    #[test]
    fn choose_each_existing_biome_and_forest_day_night() {
        let make = |biome: Biome, night: bool| MusicContext {
            in_menu: false,
            boss: false,
            depth: 0,
            night,
            biome,
            corruption: false,
            underworld: false,
        };

        assert_eq!(choose(&make(Biome::Desert, false)), Situation::Desert);
        assert_eq!(choose(&make(Biome::Snow, false)), Situation::Snow);
        assert_eq!(choose(&make(Biome::Jungle, false)), Situation::Jungle);
        assert_eq!(choose(&make(Biome::Forest, false)), Situation::Day);
        assert_eq!(choose(&make(Biome::Forest, true)), Situation::Night);
    }

    #[test]
    fn debounce_ignores_flicker_under_1_5s() {
        let mut sm = MusicStateMachine::default();
        // Starts at Day
        assert_eq!(
            sm.update(0.0, Situation::Day),
            Some(MusicAction::PlayImmediate(Situation::Day))
        );

        // Flicker to Desert for 1.0s -> no transition
        assert_eq!(sm.update(1.0, Situation::Desert), None);
        assert_eq!(sm.current, Some(Situation::Day));

        // Switch to Jungle for 1.0s -> timer resets, no transition
        assert_eq!(sm.update(1.0, Situation::Jungle), None);
        assert_eq!(sm.current, Some(Situation::Day));

        // Stay in Jungle for another 0.6s (total 1.6s >= 1.5s) -> transitions
        assert_eq!(
            sm.update(0.6, Situation::Jungle),
            Some(MusicAction::Crossfade {
                from: Situation::Day,
                to: Situation::Jungle
            })
        );
        assert_eq!(sm.current, Some(Situation::Jungle));
    }

    #[test]
    fn same_track_never_restarts() {
        let mut sm = MusicStateMachine::default();
        sm.update(0.0, Situation::Day);

        // Forest maps to "day", same as Day -> no transition even after 2s
        sm.update(1.0, Situation::Forest);
        let trans = sm.update(1.0, Situation::Forest);
        assert_eq!(trans, None);
        assert_eq!(sm.current, Some(Situation::Forest));
    }

    #[test]
    fn fade_gains_sum_and_endpoints() {
        let (f0, t0) = fade_gains(0.0);
        assert_eq!((f0, t0), (1.0, 0.0));

        let (f1, t1) = fade_gains(1.0);
        assert_eq!((f1, t1), (0.0, 1.0));

        for i in 0..=10 {
            let t = i as f32 / 10.0;
            let (f, to) = fade_gains(t);
            assert!((f + to - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn music_gain_independent_of_sound_volume() {
        assert_eq!(music_gain(0), 0.0);
        assert_eq!(music_gain(50), 0.5);
        assert_eq!(music_gain(100), 1.0);
    }

    #[test]
    fn missing_pack_music_is_silent_not_panic() {
        let mut manager = MusicManager {
            state: MusicStateMachine::default(),
            content: None, // No assets installed
            loaded_tracks: HashMap::new(),
            current_entity: None,
            fading_entity: None,
            debug_logging: false,
            debug_timer: 0.0,
        };
        let mut assets = Assets::<AudioSource>::default();
        let track = get_or_load_track(&mut manager, "nonexistent", &mut assets);
        assert!(track.is_none());
    }

    #[test]
    fn underworld_music_wins_over_underground_and_biome() {
        let c_underworld = MusicContext {
            in_menu: false,
            boss: false,
            depth: 50,
            night: true,
            biome: Biome::Jungle,
            corruption: true,
            underworld: true,
        };
        assert_eq!(choose(&c_underworld), Situation::Underworld);

        let c_boss = MusicContext {
            in_menu: false,
            boss: true,
            depth: 50,
            night: true,
            biome: Biome::Jungle,
            corruption: true,
            underworld: true,
        };
        assert_eq!(choose(&c_boss), Situation::Boss);

        let c_title = MusicContext {
            in_menu: true,
            boss: false,
            depth: 50,
            night: true,
            biome: Biome::Forest,
            corruption: false,
            underworld: true,
        };
        assert_eq!(choose(&c_title), Situation::Title);
    }

    #[test]
    fn underworld_maps_to_underworld_track() {
        assert_eq!(track_for(Situation::Underworld), "underworld");
    }

    #[test]
    fn situational_music_detects_skeletron_eow_and_corruption() {
        use crate::session::{AppState, SelectedWorld};
        use crate::view::enemies::ClientEnemy;
        use crate::view::player::LocalPlayer;
        use terravibe_sim::enemy::{EnemyId, EnemyKind};

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .init_resource::<Assets<AudioSource>>()
            .insert_resource(MusicManager {
                state: MusicStateMachine::default(),
                content: None,
                loaded_tracks: HashMap::new(),
                current_entity: None,
                fading_entity: None,
                debug_logging: false,
                debug_timer: 0.0,
            })
            .insert_resource(SelectedWorld {
                name: "test".into(),
                seed: 1,
                size: terravibe_sim::WorldSize::Small,
                persist: false,
            })
            .add_systems(Update, update_situational_music);

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        // Spawn SkeletronHead -> situation becomes Boss
        app.world_mut().resource_mut::<MusicManager>().state = MusicStateMachine::default();
        let sk = app
            .world_mut()
            .spawn(ClientEnemy {
                id: EnemyId(1),
                kind: EnemyKind::SkeletronHead,
                hp: 4400,
                moving: false,
                vel: Vec3::ZERO,
                hurt: 0,
                telegraph_t: 0.0,
            })
            .id();
        app.update();
        assert_eq!(
            app.world().resource::<MusicManager>().state.current,
            Some(Situation::Boss)
        );
        app.world_mut().despawn(sk);

        // Reset state machine and place player in Corruption column
        app.world_mut().resource_mut::<MusicManager>().state = MusicStateMachine::default();
        let (corrupt_x, corrupt_z) = (-1000..=1000)
            .step_by(40)
            .flat_map(|x| (-1000..=1000).step_by(40).map(move |z| (x, z)))
            .find(|&(x, z)| {
                terravibe_sim::worldgen::corruption::corruption_at_for_size(
                    1,
                    x,
                    z,
                    terravibe_sim::WorldSize::Small,
                ) > 0.0
            })
            .expect("must find corruption column");
        let sy = terravibe_sim::worldgen::terrain::surface_height_for_size(
            1,
            corrupt_x,
            corrupt_z,
            terravibe_sim::WorldSize::Small,
        );
        app.world_mut()
            .insert_resource(LocalPlayer(terravibe_sim::player::Player::new([
                corrupt_x as f32 + 0.5,
                sy as f32 + 1.0,
                corrupt_z as f32 + 0.5,
            ])));
        app.update();
        assert_eq!(
            app.world().resource::<MusicManager>().state.current,
            Some(Situation::Corruption)
        );
    }
}
