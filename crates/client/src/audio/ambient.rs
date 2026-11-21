//! Situational ambient audio beds, procedural loopable synthesis, and crossfading.

use std::collections::HashMap;
use std::f32::consts::TAU;

use bevy::audio::AudioSinkPlayback;
use bevy::prelude::*;
use terravibe_sim::weather::WeatherKind;
use terravibe_sim::world_bounds::{BEACH_WIDTH, OCEAN_WIDTH, WorldBounds};
use terravibe_sim::worldgen::biome::Biome;

use crate::settings::{Settings, volume_gain};

/// Continuous ambient audio bed categories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AmbientBed {
    DayForest,
    NightCrickets,
    CaveDrips,
    Wind,
    Rain,
    LavaRumble,
    OceanWaves,
}

#[cfg(test)]
const ALL_BEDS: [AmbientBed; 7] = [
    AmbientBed::DayForest,
    AmbientBed::NightCrickets,
    AmbientBed::CaveDrips,
    AmbientBed::Wind,
    AmbientBed::Rain,
    AmbientBed::LavaRumble,
    AmbientBed::OceanWaves,
];

/// Depth threshold (blocks below surface) where deep/lava rumble replaces cave drips.
const LAVA_RUMBLE_DEPTH: i32 = 190;
/// Crossfade duration in seconds between ambient beds.
const CROSSFADE_SECS: f32 = 2.0;
/// Debounce duration in seconds before switching to a new ambient bed.
const DEBOUNCE_SECS: f32 = 1.0;

/// Selects the appropriate [`AmbientBed`] for the player's current biome, depth, time of day, and weather.
pub fn select_ambient_bed(
    biome: Biome,
    depth: i32,
    is_night: bool,
    weather: WeatherKind,
) -> AmbientBed {
    select_ambient_bed_with_ocean(biome, depth, is_night, weather, false)
}

/// Selects the [`AmbientBed`], additionally accounting for proximity to the ocean ring.
pub fn select_ambient_bed_with_ocean(
    biome: Biome,
    depth: i32,
    is_night: bool,
    weather: WeatherKind,
    near_ocean: bool,
) -> AmbientBed {
    if depth >= LAVA_RUMBLE_DEPTH {
        return AmbientBed::LavaRumble;
    }
    if depth > 0 {
        return AmbientBed::CaveDrips;
    }
    if near_ocean {
        return AmbientBed::OceanWaves;
    }
    if matches!(weather, WeatherKind::Rain | WeatherKind::Storm) {
        return match biome {
            Biome::Snow => AmbientBed::Wind,
            _ => AmbientBed::Rain,
        };
    }
    match biome {
        Biome::Desert | Biome::Snow => AmbientBed::Wind,
        Biome::Forest | Biome::Jungle => {
            if is_night {
                AmbientBed::NightCrickets
            } else {
                AmbientBed::DayForest
            }
        }
    }
}

/// Linear crossfade gains `(from_gain, to_gain)` for normalized progress `t` in `[0.0, 1.0]`.
pub fn ambient_fade_gains(t: f32) -> (f32, f32) {
    let clamped = t.clamp(0.0, 1.0);
    (1.0 - clamped, clamped)
}

/// Transition action emitted by [`AmbientStateMachine`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmbientAction {
    PlayImmediate(AmbientBed),
    Crossfade { from: AmbientBed, to: AmbientBed },
}

/// Debounced state machine managing transitions and crossfades between [`AmbientBed`]s.
#[derive(Debug, Default)]
pub struct AmbientStateMachine {
    pub current: Option<AmbientBed>,
    pub candidate: Option<AmbientBed>,
    pub candidate_timer: f32,
    pub active_fade: Option<f32>,
}

impl AmbientStateMachine {
    /// Advances the state machine by `dt` seconds toward `desired` and returns an action when a transition begins.
    pub fn update(&mut self, dt: f32, desired: AmbientBed) -> Option<AmbientAction> {
        let Some(current) = self.current else {
            self.current = Some(desired);
            return Some(AmbientAction::PlayImmediate(desired));
        };

        if desired == current {
            self.candidate = None;
            self.candidate_timer = 0.0;
            return None;
        }

        if Some(desired) == self.candidate {
            self.candidate_timer += dt;
            if self.candidate_timer >= DEBOUNCE_SECS {
                let from = current;
                self.current = Some(desired);
                self.candidate = None;
                self.candidate_timer = 0.0;
                self.active_fade = Some(0.0);
                return Some(AmbientAction::Crossfade { from, to: desired });
            }
        } else {
            self.candidate = Some(desired);
            self.candidate_timer = dt;
        }

        None
    }
}

/// Component tagging an active ambient bed audio entity.
#[derive(Component)]
pub struct AmbientPlayer {
    pub bed: AmbientBed,
    pub volume_factor: f32,
}

/// Resource managing looping ambient audio beds and crossfading.
#[derive(Resource, Default)]
pub struct AmbientManager {
    pub state: AmbientStateMachine,
    pub loaded_beds: HashMap<AmbientBed, Handle<AudioSource>>,
    pub current_entity: Option<Entity>,
    pub fading_entity: Option<Entity>,
}

/// Linear gain for the ambient bus given `Settings.ambient_volume` (`0..=100`).
pub fn ambient_gain(ambient_volume: u8) -> f32 {
    (volume_gain(ambient_volume) * 0.35).clamp(0.0, 1.0)
}

/// Linear gain for the ambient bus respecting `mute_on_focus_loss` and window focus.
pub fn ambient_gain_for_settings(settings: &Settings, focused: bool) -> f32 {
    if settings.mute_on_focus_loss && !focused {
        return 0.0;
    }
    ambient_gain(settings.ambient_volume)
}

/// Plugin registering [`AmbientManager`] and the [`update_ambient_audio`] system.
pub struct AmbientPlugin;

impl Plugin for AmbientPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<crate::audio::AudioFocusState>()
            .init_resource::<AmbientManager>()
            .add_systems(
                Update,
                (crate::audio::update_audio_focus, update_ambient_audio).chain(),
            );
    }
}

fn ensure_bed_handle(
    manager: &mut AmbientManager,
    bed: AmbientBed,
    audio_assets: &mut Assets<AudioSource>,
) -> Handle<AudioSource> {
    if let Some(handle) = manager.loaded_beds.get(&bed) {
        return handle.clone();
    }
    let wav = synthesize_ambient_wav(bed);
    let handle = audio_assets.add(AudioSource { bytes: wav.into() });
    manager.loaded_beds.insert(bed, handle.clone());
    handle
}

/// Updates the active ambient bed based on player location, time, and weather, crossfading smoothly.
pub fn update_ambient_audio(
    mut commands: Commands,
    time: Res<Time>,
    mut manager: ResMut<AmbientManager>,
    settings: Option<Res<Settings>>,
    selected_world: Option<Res<crate::session::SelectedWorld>>,
    focus: Option<Res<crate::audio::AudioFocusState>>,
    app_state: Option<Res<State<crate::session::AppState>>>,
    clock: Option<Res<crate::net::ClientClock>>,
    local_player: Option<Res<crate::view::player::LocalPlayer>>,
    weather: Option<Res<crate::view::weather::WeatherFxState>>,
    mut sinks: Query<(&mut AudioSink, &mut AmbientPlayer)>,
    mut audio_assets: Option<ResMut<Assets<AudioSource>>>,
) {
    let in_menu = app_state
        .as_ref()
        .map_or(true, |s| s.get() == &crate::session::AppState::Menu);
    if in_menu {
        if let Some(e) = manager.current_entity.take() {
            if let Ok((sink, _)) = sinks.get_mut(e) {
                sink.stop();
            }
            commands.entity(e).despawn();
        }
        if let Some(e) = manager.fading_entity.take() {
            if let Ok((sink, _)) = sinks.get_mut(e) {
                sink.stop();
            }
            commands.entity(e).despawn();
        }
        manager.state = AmbientStateMachine::default();
        return;
    }

    let dt = time.delta_secs();
    let is_night = clock.as_ref().map_or(false, |c| {
        !terravibe_sim::time::WorldClock {
            tick: c.effective_tick(),
        }
        .is_day()
    });

    let (biome, depth, near_ocean, underground_sheltered) = if let Some(lp) = &local_player {
        let p = lp.0.body.pos;
        let wx = p[0].floor() as i32;
        let wy = p[1].floor() as i32;
        let wz = p[2].floor() as i32;
        let seed = selected_world
            .as_ref()
            .map_or(crate::view::SEED, |w| w.seed);
        let size = selected_world
            .as_ref()
            .map_or(terravibe_sim::WorldSize::Small, |w| w.size);
        let surface = terravibe_sim::worldgen::terrain::surface_height_for_size(seed, wx, wz, size);
        let depth = (surface - wy) - 15;
        let w = terravibe_sim::worldgen::biome::biome_weights_for_size(seed, wx, wz, size);
        let b = terravibe_sim::worldgen::biome::dominant(&w);
        let bounds = WorldBounds::new(size);
        let near_ocean = bounds.contains_column(wx, wz)
            && bounds.edge_distance(wx, wz) < OCEAN_WIDTH + BEACH_WIDTH;
        (b, depth, near_ocean, wy < surface - 6)
    } else {
        (Biome::Forest, 0, false, false)
    };

    let weather_kind = if underground_sheltered {
        WeatherKind::Clear
    } else {
        weather.as_deref().map_or(WeatherKind::Clear, |w| w.kind)
    };
    let desired = select_ambient_bed_with_ocean(biome, depth, is_night, weather_kind, near_ocean);
    let focused = focus.as_ref().is_none_or(|f| f.focused);
    let base_gain = settings
        .as_deref()
        .map_or(0.25, |s| ambient_gain_for_settings(s, focused));

    if let Some(action) = manager.state.update(dt, desired)
        && let Some(assets) = audio_assets.as_deref_mut()
    {
        match action {
            AmbientAction::PlayImmediate(bed) => {
                let handle = ensure_bed_handle(&mut manager, bed, assets);
                let e = commands
                    .spawn((
                        AudioPlayer(handle),
                        PlaybackSettings::LOOP.with_volume(bevy::audio::Volume::Linear(base_gain)),
                        AmbientPlayer {
                            bed,
                            volume_factor: 1.0,
                        },
                    ))
                    .id();
                manager.current_entity = Some(e);
            }
            AmbientAction::Crossfade { from: _, to } => {
                if let Some(prior) = manager.fading_entity.take() {
                    if let Ok((sink, _)) = sinks.get_mut(prior) {
                        sink.stop();
                    }
                    commands.entity(prior).despawn();
                }
                if let Some(old) = manager.current_entity.take() {
                    manager.fading_entity = Some(old);
                }
                let handle = ensure_bed_handle(&mut manager, to, assets);
                let e = commands
                    .spawn((
                        AudioPlayer(handle),
                        PlaybackSettings::LOOP.with_volume(bevy::audio::Volume::Linear(0.0)),
                        AmbientPlayer {
                            bed: to,
                            volume_factor: 0.0,
                        },
                    ))
                    .id();
                manager.current_entity = Some(e);
            }
        }
    }

    if let Some(mut fade_t) = manager.state.active_fade {
        fade_t += dt / CROSSFADE_SECS;
        let (from_vol, to_vol) = ambient_fade_gains(fade_t);

        if let Some(from_e) = manager.fading_entity
            && let Ok((mut sink, mut player)) = sinks.get_mut(from_e)
        {
            player.volume_factor = from_vol;
            sink.set_volume(bevy::audio::Volume::Linear(base_gain * from_vol));
        }

        if let Some(to_e) = manager.current_entity
            && let Ok((mut sink, mut player)) = sinks.get_mut(to_e)
        {
            player.volume_factor = to_vol;
            sink.set_volume(bevy::audio::Volume::Linear(base_gain * to_vol));
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
    } else if let Some(to_e) = manager.current_entity
        && let Ok((mut sink, mut player)) = sinks.get_mut(to_e)
    {
        player.volume_factor = 1.0;
        sink.set_volume(bevy::audio::Volume::Linear(base_gain));
    }
}

/// Generates a seamless 2-second 22050 Hz mono 16-bit PCM WAV buffer for `bed`.
pub fn synthesize_ambient_wav(bed: AmbientBed) -> Vec<u8> {
    const SAMPLE_RATE: u32 = 22_050;
    const DURATION_SECS: usize = 2;
    const NUM_SAMPLES: usize = SAMPLE_RATE as usize * DURATION_SECS;

    let mut samples = Vec::with_capacity(NUM_SAMPLES);
    let mut rng: u32 = 0x1234_5678 ^ (bed as u32).wrapping_mul(0x9E37_79B9);
    let mut lp_state = 0.0_f32;

    for i in 0..NUM_SAMPLES {
        let t = i as f32 / SAMPLE_RATE as f32;
        rng = rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let white = ((rng >> 16) as f32 / 32768.0) - 1.0;

        let sample = match bed {
            AmbientBed::DayForest => {
                // Gentle breeze + soft periodic bird-like harmonicchirp
                lp_state += 0.03 * (white * 0.15 - lp_state);
                let chirp_env = ((t * TAU * 0.5).sin().max(0.0)).powf(8.0);
                let chirp =
                    (t * TAU * 1800.0 + (t * TAU * 12.0).sin() * 2.0).sin() * 0.08 * chirp_env;
                lp_state + chirp
            }
            AmbientBed::NightCrickets => {
                // Soft cricket pulse train
                let pulse = if (t * 2.0).fract() < 0.35 {
                    ((t * TAU * 18.0).sin() * 0.5 + 0.5).powf(2.0)
                } else {
                    0.0
                };
                let carrier = (t * TAU * 4200.0).sin() * 0.07 * pulse;
                lp_state += 0.02 * (white * 0.08 - lp_state);
                lp_state + carrier
            }
            AmbientBed::CaveDrips => {
                // Low cave air + periodic water drip resonance
                lp_state += 0.015 * (white * 0.12 - lp_state);
                let drip_phase = (t * 0.5).fract();
                let drip = if drip_phase < 0.08 {
                    let env = (-drip_phase * 55.0).exp();
                    let freq = 900.0 + drip_phase * 3500.0;
                    (drip_phase * TAU * freq).sin() * 0.18 * env
                } else {
                    0.0
                };
                lp_state + drip
            }
            AmbientBed::Wind => {
                // Swelling filtered noise
                let swell = 0.5 + 0.5 * (t * TAU * 0.5).sin();
                lp_state += 0.04 * (white * (0.22 + 0.18 * swell) - lp_state);
                lp_state
            }
            AmbientBed::Rain => {
                // Steady mid-band rain patter
                lp_state += 0.18 * (white * 0.25 - lp_state);
                lp_state
            }
            AmbientBed::LavaRumble => {
                // Deep low-pass rumble + sub-bass hum
                lp_state += 0.012 * (white * 0.35 - lp_state);
                let sub = (t * TAU * 55.0).sin() * 0.08;
                lp_state + sub
            }
            AmbientBed::OceanWaves => {
                // Slow surf swell
                let wave = 0.5 + 0.5 * (t * TAU * 0.5).sin();
                lp_state += 0.06 * (white * (0.1 + 0.28 * wave) - lp_state);
                lp_state
            }
        };

        let clamped = (sample * 32767.0).clamp(-32767.0, 32767.0) as i16;
        samples.push(clamped);
    }

    let data_bytes = (NUM_SAMPLES * 2) as u32;
    let riff_size = 36 + data_bytes;
    let mut wav = Vec::with_capacity(44 + data_bytes as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_size.to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes()); // PCM chunk size
    wav.extend_from_slice(&1_u16.to_le_bytes()); // PCM format
    wav.extend_from_slice(&1_u16.to_le_bytes()); // Mono
    wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    wav.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // Byte rate
    wav.extend_from_slice(&2_u16.to_le_bytes()); // Block align
    wav.extend_from_slice(&16_u16.to_le_bytes()); // Bits per sample
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_bytes.to_le_bytes());
    for s in samples {
        wav.extend_from_slice(&s.to_le_bytes());
    }
    wav
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ambient_bed_selects_by_biome_depth_time_and_weather() {
        // Deep underground / underworld -> LavaRumble
        assert_eq!(
            select_ambient_bed(Biome::Forest, 210, false, WeatherKind::Clear),
            AmbientBed::LavaRumble
        );
        assert_eq!(
            select_ambient_bed(Biome::Snow, 195, true, WeatherKind::Rain),
            AmbientBed::LavaRumble
        );

        // Caves -> CaveDrips
        assert_eq!(
            select_ambient_bed(Biome::Forest, 25, false, WeatherKind::Clear),
            AmbientBed::CaveDrips
        );
        assert_eq!(
            select_ambient_bed(Biome::Desert, 40, true, WeatherKind::Storm),
            AmbientBed::CaveDrips
        );

        // Surface Rain / Storm -> Rain (non-Snow) or Wind (Snow blizzard)
        assert_eq!(
            select_ambient_bed(Biome::Forest, 0, false, WeatherKind::Rain),
            AmbientBed::Rain
        );
        assert_eq!(
            select_ambient_bed(Biome::Jungle, -5, true, WeatherKind::Storm),
            AmbientBed::Rain
        );
        assert_eq!(
            select_ambient_bed(Biome::Snow, 0, false, WeatherKind::Rain),
            AmbientBed::Wind
        );

        // Surface clear weather by biome & time
        assert_eq!(
            select_ambient_bed(Biome::Desert, 0, false, WeatherKind::Clear),
            AmbientBed::Wind
        );
        assert_eq!(
            select_ambient_bed(Biome::Snow, 0, false, WeatherKind::Clear),
            AmbientBed::Wind
        );
        assert_eq!(
            select_ambient_bed(Biome::Forest, 0, true, WeatherKind::Clear),
            AmbientBed::NightCrickets
        );
        assert_eq!(
            select_ambient_bed(Biome::Jungle, 0, true, WeatherKind::Clear),
            AmbientBed::NightCrickets
        );
        assert_eq!(
            select_ambient_bed(Biome::Forest, 0, false, WeatherKind::Clear),
            AmbientBed::DayForest
        );
        assert_eq!(
            select_ambient_bed(Biome::Jungle, 0, false, WeatherKind::Clear),
            AmbientBed::DayForest
        );

        // Ocean ring on surface
        assert_eq!(
            select_ambient_bed_with_ocean(Biome::Forest, 0, false, WeatherKind::Clear, true),
            AmbientBed::OceanWaves
        );
    }

    #[test]
    fn ambient_state_machine_debounces_and_crossfades() {
        let mut sm = AmbientStateMachine::default();
        assert_eq!(
            sm.update(0.1, AmbientBed::DayForest),
            Some(AmbientAction::PlayImmediate(AmbientBed::DayForest))
        );
        assert_eq!(sm.update(0.5, AmbientBed::DayForest), None);

        // Brief dip does not trigger crossfade immediately
        assert_eq!(sm.update(0.5, AmbientBed::CaveDrips), None);
        // Sustained change past debounce triggers crossfade
        assert_eq!(
            sm.update(0.6, AmbientBed::CaveDrips),
            Some(AmbientAction::Crossfade {
                from: AmbientBed::DayForest,
                to: AmbientBed::CaveDrips,
            })
        );
        assert_eq!(sm.active_fade, Some(0.0));
        assert_eq!(ambient_fade_gains(0.0), (1.0, 0.0));
        assert_eq!(ambient_fade_gains(0.5), (0.5, 0.5));
        assert_eq!(ambient_fade_gains(1.0), (0.0, 1.0));
    }

    #[test]
    fn synthesize_ambient_wav_produces_valid_wave_headers_for_all_beds() {
        for bed in ALL_BEDS {
            let wav = synthesize_ambient_wav(bed);
            assert!(wav.len() > 44, "expected WAV data for {bed:?}");
            assert_eq!(&wav[0..4], b"RIFF");
            assert_eq!(&wav[8..12], b"WAVE");
            assert_eq!(&wav[36..40], b"data");
        }
    }

    #[test]
    fn update_ambient_audio_cleans_up_on_menu() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<crate::session::AppState>();
        app.init_resource::<AmbientManager>();
        app.add_systems(Update, update_ambient_audio);

        let e1 = app.world_mut().spawn_empty().id();
        let e2 = app.world_mut().spawn_empty().id();
        {
            let mut mgr = app.world_mut().resource_mut::<AmbientManager>();
            mgr.current_entity = Some(e1);
            mgr.fading_entity = Some(e2);
            mgr.state.current = Some(AmbientBed::DayForest);
        }

        app.update();

        let mgr = app.world().resource::<AmbientManager>();
        assert_eq!(mgr.current_entity, None);
        assert_eq!(mgr.fading_entity, None);
        assert_eq!(mgr.state.current, None);
        assert!(app.world().get_entity(e1).is_err());
        assert!(app.world().get_entity(e2).is_err());
    }

    #[test]
    fn crossfade_despawns_prior_fading_entity() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<crate::session::AppState>();
        app.world_mut()
            .resource_mut::<NextState<crate::session::AppState>>()
            .set(crate::session::AppState::InGame);
        app.init_resource::<AmbientManager>();
        app.init_resource::<Assets<AudioSource>>();
        app.add_systems(Update, update_ambient_audio);

        let prior_fade = app.world_mut().spawn_empty().id();
        let cur = app.world_mut().spawn_empty().id();
        {
            let mut mgr = app.world_mut().resource_mut::<AmbientManager>();
            mgr.fading_entity = Some(prior_fade);
            mgr.current_entity = Some(cur);
            mgr.state.current = Some(AmbientBed::CaveDrips);
            mgr.state.candidate = Some(AmbientBed::DayForest);
            mgr.state.candidate_timer = DEBOUNCE_SECS;
        }

        app.update();

        assert!(app.world().get_entity(prior_fade).is_err());
        let mgr = app.world().resource::<AmbientManager>();
        assert_eq!(mgr.fading_entity, Some(cur));
        assert!(mgr.current_entity.is_some());
        assert_ne!(mgr.current_entity, Some(cur));
    }
}
