//! Weather presentation: rain/snowfall particles, lightning flash, thunder SFX, and wet ground darkening.

use bevy::prelude::*;
use terravibe_sim::weather::WeatherKind;
use terravibe_sim::worldgen::biome::{Biome, biome_weights, dominant};

use crate::audio::{SfxEvent, SfxKind};
use crate::session::AppState;
use crate::settings::Settings;
use crate::view::player::LocalPlayer;

/// Maximum concurrent weather particles in [`WeatherFxState`].
pub const MAX_WEATHER_PARTICLES: usize = 300;

/// Effective [`WeatherFxState`] particle cap scaled by `settings.quality_preset`.
pub fn max_weather_particles_for(settings: &Settings) -> usize {
    ((MAX_WEATHER_PARTICLES as f32) * settings.quality_preset.particle_scale())
        .round()
        .max(1.0) as usize
}

const WEATHER_SPAWN_RADIUS_XZ: f32 = 16.0;
const WEATHER_SPAWN_HEIGHT: f32 = 14.0;

/// A single raindrop or snowflake particle.
#[derive(Clone, Debug)]
pub struct WeatherParticle {
    pub pos: Vec3,
    pub vel: Vec3,
    pub is_snow: bool,
    pub age: f32,
    pub lifetime: f32,
    pub size: f32,
}

/// Client-side weather presentation state driven by [`WeatherKind`] from the server.
#[derive(Resource, Default, Debug)]
pub struct WeatherFxState {
    pub kind: WeatherKind,
    pub particles: Vec<WeatherParticle>,
    pub lightning_flash: f32,
    pub lightning_cooldown: f32,
    pub thunder_delay: Option<f32>,
    pub wet_ground_factor: f32,
    rng_state: u32,
}

impl WeatherFxState {
    fn next_unit(&mut self) -> f32 {
        let mut x = if self.rng_state == 0 {
            0xA341_316C
        } else {
            self.rng_state
        };
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng_state = x;
        (x as f32) / (u32::MAX as f32)
    }

    fn next_signed(&mut self) -> f32 {
        self.next_unit() * 2.0 - 1.0
    }

    /// Wet-ground ambient darkening multiplier in `[0.75, 1.0]`.
    pub fn ground_darkening_multiplier(&self) -> f32 {
        1.0 - 0.25 * self.wet_ground_factor.clamp(0.0, 1.0)
    }

    /// Advances weather particles, lightning, thunder timer, and wet-ground factor by `dt` seconds.
    /// Returns `true` when thunder SFX should be triggered this step.
    pub fn step(&mut self, dt: f32, center: Vec3, biome: Biome, settings: &Settings) -> bool {
        if dt <= 0.0 {
            return false;
        }

        if !settings.weather_effects || settings.reduced_motion {
            self.particles.clear();
            self.lightning_flash = 0.0;
            self.thunder_delay = None;
            self.wet_ground_factor = (self.wet_ground_factor - dt * 0.25).max(0.0);
            return false;
        }

        let target_wet = if matches!(self.kind, WeatherKind::Rain | WeatherKind::Storm)
            && biome != Biome::Snow
        {
            if self.kind == WeatherKind::Storm {
                1.0
            } else {
                0.65
            }
        } else {
            0.0
        };
        if self.wet_ground_factor < target_wet {
            self.wet_ground_factor = (self.wet_ground_factor + dt * 0.25).min(target_wet);
        } else {
            self.wet_ground_factor = (self.wet_ground_factor - dt * 0.15).max(target_wet);
        }

        for p in &mut self.particles {
            p.age += dt;
            p.pos += p.vel * dt;
        }
        self.particles.retain(|p| p.age < p.lifetime);
        let cap = max_weather_particles_for(settings);
        if self.particles.len() > cap {
            self.particles.truncate(cap);
        }

        if matches!(self.kind, WeatherKind::Rain | WeatherKind::Storm) {
            let is_snow = biome == Biome::Snow;
            let rate = match (self.kind, is_snow) {
                (WeatherKind::Storm, false) => 220.0,
                (WeatherKind::Rain, false) => 130.0,
                (WeatherKind::Storm, true) => 110.0,
                (WeatherKind::Rain, true) => 70.0,
                (WeatherKind::Clear, _) => 0.0,
            };
            let spawn_count = ((rate * dt).ceil() as usize).min(24);
            for _ in 0..spawn_count {
                if self.particles.len() >= cap {
                    break;
                }
                let offset = Vec3::new(
                    self.next_signed() * WEATHER_SPAWN_RADIUS_XZ,
                    WEATHER_SPAWN_HEIGHT * (0.65 + 0.35 * self.next_unit()),
                    self.next_signed() * WEATHER_SPAWN_RADIUS_XZ,
                );
                let (vel, lifetime, size) = if is_snow {
                    (
                        Vec3::new(
                            self.next_signed() * 0.8,
                            -2.2 - self.next_unit() * 1.2,
                            self.next_signed() * 0.8,
                        ),
                        4.2 + self.next_unit() * 1.5,
                        0.12 + self.next_unit() * 0.06,
                    )
                } else {
                    (
                        Vec3::new(
                            -1.5 + self.next_signed() * 0.4,
                            -19.0 - self.next_unit() * 4.0,
                            self.next_signed() * 0.4,
                        ),
                        0.85 + self.next_unit() * 0.25,
                        0.05,
                    )
                };
                self.particles.push(WeatherParticle {
                    pos: center + offset,
                    vel,
                    is_snow,
                    age: 0.0,
                    lifetime,
                    size,
                });
            }
        }

        self.lightning_flash = (self.lightning_flash - dt * 3.5).max(0.0);
        if self.kind == WeatherKind::Storm {
            self.lightning_cooldown -= dt;
            if self.lightning_cooldown <= 0.0 {
                self.lightning_flash = 1.0;
                self.lightning_cooldown = 6.0 + self.next_unit() * 10.0;
                self.thunder_delay = Some(0.15 + self.next_unit() * 0.35);
            }
        } else {
            self.lightning_cooldown = 0.0;
        }

        let mut thunder_triggered = false;
        if let Some(delay) = &mut self.thunder_delay {
            *delay -= dt;
            if *delay <= 0.0 {
                self.thunder_delay = None;
                thunder_triggered = true;
            }
        }
        thunder_triggered
    }
}

#[derive(Resource)]
struct WeatherRenderAssets {
    rain_mesh: Option<Handle<Mesh>>,
    snow_mesh: Option<Handle<Mesh>>,
    rain_mat: Handle<StandardMaterial>,
    snow_mat: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
struct WeatherEntities {
    rain: Option<Entity>,
    snow: Option<Entity>,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

/// Plugin registering [`WeatherFxState`] and weather particle/lightning presentation systems.
pub struct WeatherPlugin;

impl Plugin for WeatherPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WeatherFxState>()
            .init_resource::<WeatherEntities>()
            .add_systems(Startup, setup_weather_assets)
            .add_systems(
                OnExit(AppState::InGame),
                |mut state: ResMut<WeatherFxState>, mut ents: ResMut<WeatherEntities>| {
                    state.particles.clear();
                    state.lightning_flash = 0.0;
                    state.thunder_delay = None;
                    ents.rain = None;
                    ents.snow = None;
                },
            )
            .add_systems(
                Update,
                update_weather_fx
                    .after(crate::view::sky::update_sky)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

fn build_weather_mesh(ents: &WeatherEntities) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, ents.positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, ents.normals.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, ents.uvs.clone());
    mesh.insert_indices(Indices::U32(ents.indices.clone()));
    mesh
}

fn setup_weather_assets(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.insert_resource(WeatherRenderAssets {
        rain_mesh: None,
        snow_mesh: None,
        rain_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(0.62, 0.76, 0.94, 0.65),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        }),
        snow_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.95, 0.97, 1.0),
            unlit: true,
            cull_mode: None,
            ..default()
        }),
    });
}

fn push_crossed_quads(ents: &mut WeatherEntities, center: Vec3, half_w: f32, half_h: f32) {
    let base = ents.positions.len() as u32;
    // Quad 1 in XY plane
    ents.positions.extend_from_slice(&[
        [center.x - half_w, center.y - half_h, center.z],
        [center.x + half_w, center.y - half_h, center.z],
        [center.x + half_w, center.y + half_h, center.z],
        [center.x - half_w, center.y + half_h, center.z],
        // Quad 2 in ZY plane
        [center.x, center.y - half_h, center.z - half_w],
        [center.x, center.y - half_h, center.z + half_w],
        [center.x, center.y + half_h, center.z + half_w],
        [center.x, center.y + half_h, center.z - half_w],
    ]);
    ents.normals.extend_from_slice(&[
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
    ]);
    ents.uvs.extend_from_slice(&[
        [0.0, 1.0],
        [1.0, 1.0],
        [1.0, 0.0],
        [0.0, 0.0],
        [0.0, 1.0],
        [1.0, 1.0],
        [1.0, 0.0],
        [0.0, 0.0],
    ]);
    ents.indices.extend_from_slice(&[
        base,
        base + 1,
        base + 2,
        base,
        base + 2,
        base + 3,
        base + 4,
        base + 5,
        base + 6,
        base + 4,
        base + 6,
        base + 7,
    ]);
}

fn upload_weather_batch(
    mesh_handle: &Handle<Mesh>,
    meshes: &mut Assets<Mesh>,
    ents: &WeatherEntities,
) {
    use bevy::mesh::Indices;
    if let Some(mut mesh) = meshes.get_mut(mesh_handle) {
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, ents.positions.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, ents.normals.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, ents.uvs.clone());
        mesh.insert_indices(Indices::U32(ents.indices.clone()));
    }
}

#[allow(clippy::too_many_arguments)]
fn update_weather_fx(
    mut commands: Commands,
    time: Res<Time<Real>>,
    settings: Res<Settings>,
    mut state: ResMut<WeatherFxState>,
    mut ents: ResMut<WeatherEntities>,
    assets: Option<ResMut<WeatherRenderAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    local_player: Option<Res<LocalPlayer>>,
    camera_q: Query<&Transform, (With<Camera3d>, Without<DirectionalLight>)>,
    mut ambient: Option<ResMut<GlobalAmbientLight>>,
    mut sun_q: Query<&mut DirectionalLight, With<DirectionalLight>>,
    mut sfx: MessageWriter<SfxEvent>,
    mut vis_q: Query<&mut Visibility, (Without<Camera3d>, Without<DirectionalLight>)>,
) {
    let center = local_player
        .as_ref()
        .map(|lp| Vec3::from_array(lp.0.body.pos))
        .or_else(|| camera_q.iter().next().map(|t| t.translation))
        .unwrap_or(Vec3::ZERO);

    let weights = biome_weights(
        crate::view::SEED,
        center.x.floor() as i32,
        center.z.floor() as i32,
    );
    let biome = dominant(&weights);

    if state.step(time.delta_secs(), center, biome, &settings) {
        sfx.write(SfxEvent(SfxKind::Thunder));
    }

    if settings.weather_effects {
        let wet_mult = state.ground_darkening_multiplier();
        if let Some(amb) = ambient.as_mut() {
            amb.brightness = amb.brightness * wet_mult + state.lightning_flash * 900.0;
        }
        if state.lightning_flash > 0.0 {
            for mut light in &mut sun_q {
                light.illuminance += state.lightning_flash * 14_000.0;
            }
        }
    }

    let Some(mut assets) = assets else {
        return;
    };

    // Build batched rain mesh (at most 1 draw call; never create or upload an empty mesh).
    ents.positions.clear();
    ents.normals.clear();
    ents.uvs.clear();
    ents.indices.clear();
    for p in state.particles.iter().filter(|p| !p.is_snow) {
        push_crossed_quads(&mut ents, p.pos, p.size * 0.5, p.size * 5.5);
    }
    let has_rain = !ents.positions.is_empty();
    if has_rain {
        let handle = assets
            .rain_mesh
            .get_or_insert_with(|| meshes.add(build_weather_mesh(&ents)))
            .clone();
        if ents.rain.is_none() {
            ents.rain = Some(
                commands
                    .spawn((
                        Mesh3d(handle),
                        MeshMaterial3d(assets.rain_mat.clone()),
                        Transform::default(),
                        Visibility::Visible,
                        DespawnOnExit(AppState::InGame),
                    ))
                    .id(),
            );
        } else {
            upload_weather_batch(&handle, &mut meshes, &ents);
        }
    }
    if let Some(rain_e) = ents.rain
        && let Ok(mut vis) = vis_q.get_mut(rain_e)
    {
        let target = if has_rain {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *vis != target {
            *vis = target;
        }
    }

    // Build batched snow mesh (at most 1 draw call; never create or upload an empty mesh).
    ents.positions.clear();
    ents.normals.clear();
    ents.uvs.clear();
    ents.indices.clear();
    for p in state.particles.iter().filter(|p| p.is_snow) {
        let half = p.size * 0.5;
        push_crossed_quads(&mut ents, p.pos, half, half);
    }
    let has_snow = !ents.positions.is_empty();
    if has_snow {
        let handle = assets
            .snow_mesh
            .get_or_insert_with(|| meshes.add(build_weather_mesh(&ents)))
            .clone();
        if ents.snow.is_none() {
            ents.snow = Some(
                commands
                    .spawn((
                        Mesh3d(handle),
                        MeshMaterial3d(assets.snow_mat.clone()),
                        Transform::default(),
                        Visibility::Visible,
                        DespawnOnExit(AppState::InGame),
                    ))
                    .id(),
            );
        } else {
            upload_weather_batch(&handle, &mut meshes, &ents);
        }
    }
    if let Some(snow_e) = ents.snow
        && let Ok(mut vis) = vis_q.get_mut(snow_e)
    {
        let target = if has_snow {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *vis != target {
            *vis = target;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weather_particles_use_snow_in_snow_biome_and_respect_cap() {
        let settings = Settings::default();
        let mut state = WeatherFxState {
            kind: WeatherKind::Storm,
            ..Default::default()
        };
        for _ in 0..40 {
            state.step(0.05, Vec3::ZERO, Biome::Snow, &settings);
        }
        assert!(!state.particles.is_empty());
        assert!(state.particles.len() <= MAX_WEATHER_PARTICLES);
        assert!(state.particles.iter().all(|p| p.is_snow));
    }

    #[test]
    fn weather_storm_triggers_lightning_flash_and_thunder_and_wet_ground() {
        let settings = Settings::default();
        let mut state = WeatherFxState {
            kind: WeatherKind::Storm,
            ..Default::default()
        };
        let mut thunder_fired = false;
        for _ in 0..20 {
            if state.step(0.05, Vec3::ZERO, Biome::Forest, &settings) {
                thunder_fired = true;
            }
        }
        assert!(thunder_fired);
        assert!(state.wet_ground_factor > 0.0);
        assert!(state.ground_darkening_multiplier() < 1.0);
    }

    #[test]
    fn weather_effects_toggle_disables_and_clears_particles() {
        let mut settings = Settings::default();
        let mut state = WeatherFxState {
            kind: WeatherKind::Rain,
            ..Default::default()
        };
        state.step(0.05, Vec3::ZERO, Biome::Forest, &settings);
        assert!(!state.particles.is_empty());

        settings.weather_effects = false;
        state.step(0.05, Vec3::ZERO, Biome::Forest, &settings);
        assert!(state.particles.is_empty());
    }

    #[test]
    fn weather_particles_scale_with_quality_preset() {
        use crate::settings::QualityPreset;

        let mut low = Settings::default();
        low.quality_preset = QualityPreset::Low;
        let mut high = Settings::default();
        high.quality_preset = QualityPreset::High;

        let cap_low = max_weather_particles_for(&low);
        let cap_high = max_weather_particles_for(&high);
        assert!(cap_low < cap_high);
        assert_eq!(cap_high, MAX_WEATHER_PARTICLES);

        let mut state = WeatherFxState {
            kind: WeatherKind::Storm,
            ..Default::default()
        };
        for _ in 0..60 {
            state.step(0.1, Vec3::ZERO, Biome::Forest, &low);
        }
        assert!(state.particles.len() <= cap_low);
        assert!(!state.particles.is_empty());
    }
}
