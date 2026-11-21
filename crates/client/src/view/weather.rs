//! Weather presentation: rain/snowfall particles, lightning flash, thunder SFX, and wet ground darkening.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use terravibe_sim::weather::WeatherKind;
use terravibe_sim::world_bounds::WorldSize;
use terravibe_sim::worldgen::biome::{Biome, biome_weights, dominant};
use terravibe_sim::worldgen::terrain::surface_height_for_size;
use terravibe_sim::{CHUNK_SIZE, ChunkPos, WORLD_CHUNK_LAYERS};

use crate::audio::{SfxEvent, SfxKind};
use crate::session::{AppState, SelectedWorld};
use crate::settings::Settings;
use crate::view::chunks::ChunkStream;
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
const ROOF_RADIUS: i32 = 17;
const ROOF_GRID_SIDE: usize = (ROOF_RADIUS * 2 + 1) as usize;

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
        self.step_with_roof(dt, center, biome, settings, false, |_, _| f32::NEG_INFINITY)
    }

    /// Advances weather particles with a per-column sky-roof height query `roof_y(x, z)`.
    /// Particles do not spawn under solid ceilings/caves and despawn upon hitting `roof_y`.
    pub fn step_with_roof(
        &mut self,
        dt: f32,
        center: Vec3,
        biome: Biome,
        settings: &Settings,
        sheltered: bool,
        roof_y: impl Fn(f32, f32) -> f32,
    ) -> bool {
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

        let target_wet = if !sheltered
            && matches!(self.kind, WeatherKind::Rain | WeatherKind::Storm)
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
        self.particles
            .retain(|p| p.age < p.lifetime && p.pos.y > roof_y(p.pos.x, p.pos.z));
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
                let off_x = self.next_signed() * WEATHER_SPAWN_RADIUS_XZ;
                let off_y = WEATHER_SPAWN_HEIGHT * (0.65 + 0.35 * self.next_unit());
                let off_z = self.next_signed() * WEATHER_SPAWN_RADIUS_XZ;
                let px = center.x + off_x;
                let py = center.y + off_y;
                let pz = center.z + off_z;
                let floor_y = roof_y(px, pz);
                if floor_y > center.y + 4.0 || py <= floor_y + 0.5 {
                    continue;
                }
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
                    pos: Vec3::new(px, py, pz),
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

/// Cached 35×35 local sky-roof heightmap around the player so per-particle
/// roof/cave collision is O(1) with zero per-frame chunk lookups when stationary.
#[derive(Resource)]
struct SkyRoofCache {
    origin_x: i32,
    origin_z: i32,
    light_epoch: u64,
    seed: u32,
    initialized: bool,
    floors: Vec<i32>,
}

impl Default for SkyRoofCache {
    fn default() -> Self {
        Self {
            origin_x: 0,
            origin_z: 0,
            light_epoch: u64::MAX,
            seed: 0,
            initialized: false,
            floors: vec![0; ROOF_GRID_SIDE * ROOF_GRID_SIDE],
        }
    }
}

impl SkyRoofCache {
    fn refresh(&mut self, wx: i32, wz: i32, stream: &ChunkStream, seed: u32, size: WorldSize) {
        let epoch = stream.light_epoch();
        if self.initialized
            && self.origin_x == wx
            && self.origin_z == wz
            && self.light_epoch == epoch
            && self.seed == seed
        {
            return;
        }
        self.origin_x = wx;
        self.origin_z = wz;
        self.light_epoch = epoch;
        self.seed = seed;
        self.initialized = true;
        let chunks = stream.generated();
        for dz in -ROOF_RADIUS..=ROOF_RADIUS {
            let gz = (dz + ROOF_RADIUS) as usize;
            let cz_w = wz + dz;
            for dx in -ROOF_RADIUS..=ROOF_RADIUS {
                let gx = (dx + ROOF_RADIUS) as usize;
                let cx_w = wx + dx;
                self.floors[gz * ROOF_GRID_SIDE + gx] =
                    column_rain_floor(cx_w, cz_w, chunks, seed, size);
            }
        }
    }

    #[inline]
    fn sample(&self, x: f32, z: f32) -> f32 {
        if !self.initialized {
            return f32::NEG_INFINITY;
        }
        let ix = x.floor() as i32 - self.origin_x + ROOF_RADIUS;
        let iz = z.floor() as i32 - self.origin_z + ROOF_RADIUS;
        if (0..ROOF_GRID_SIDE as i32).contains(&ix) && (0..ROOF_GRID_SIDE as i32).contains(&iz) {
            self.floors[(iz as usize) * ROOF_GRID_SIDE + (ix as usize)] as f32
        } else {
            f32::NEG_INFINITY
        }
    }
}

fn column_rain_floor(
    wx: i32,
    wz: i32,
    chunks: &std::collections::HashMap<ChunkPos, std::sync::Arc<terravibe_sim::Chunk>>,
    seed: u32,
    size: WorldSize,
) -> i32 {
    let cx = wx.div_euclid(CHUNK_SIZE);
    let cz = wz.div_euclid(CHUNK_SIZE);
    let lx = wx.rem_euclid(CHUNK_SIZE) as usize;
    let lz = wz.rem_euclid(CHUNK_SIZE) as usize;

    let mut max_loaded_cy: Option<i32> = None;
    let mut loaded_top: Option<i32> = None;
    for cy in (0..WORLD_CHUNK_LAYERS).rev() {
        let Some(chunk) = chunks.get(&ChunkPos {
            x: cx,
            y: cy,
            z: cz,
        }) else {
            continue;
        };
        if max_loaded_cy.is_none() {
            max_loaded_cy = Some(cy);
        }
        if loaded_top.is_none() {
            for ly in (0..CHUNK_SIZE as usize).rev() {
                if chunk.get(lx, ly, lz).is_solid() {
                    loaded_top = Some(cy * CHUNK_SIZE + ly as i32 + 1);
                    break;
                }
            }
        }
        if loaded_top.is_some() {
            break;
        }
    }

    let surf = surface_height_for_size(seed, wx, wz, size) + 1;
    let Some(max_cy) = max_loaded_cy else {
        return surf;
    };
    let max_loaded_y = (max_cy + 1) * CHUNK_SIZE - 1;
    if max_loaded_y < surf {
        // Player is deep underground and surface chunks are outside vertical window.
        loaded_top.unwrap_or(surf).max(surf)
    } else {
        loaded_top.unwrap_or(0)
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
    prev_rain_count: usize,
    prev_snow_count: usize,
    last_biome_xz: Option<[i32; 2]>,
    cached_biome: Option<Biome>,
}

/// Plugin registering [`WeatherFxState`] and weather particle/lightning presentation systems.
pub struct WeatherPlugin;

impl Plugin for WeatherPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WeatherFxState>()
            .init_resource::<WeatherEntities>()
            .init_resource::<SkyRoofCache>()
            .add_systems(Startup, setup_weather_assets)
            .add_systems(
                OnExit(AppState::InGame),
                |mut state: ResMut<WeatherFxState>,
                 mut ents: ResMut<WeatherEntities>,
                 mut roof: ResMut<SkyRoofCache>| {
                    state.particles.clear();
                    state.lightning_flash = 0.0;
                    state.thunder_delay = None;
                    ents.rain = None;
                    ents.snow = None;
                    ents.prev_rain_count = 0;
                    ents.prev_snow_count = 0;
                    ents.last_biome_xz = None;
                    ents.cached_biome = None;
                    roof.initialized = false;
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

/// Allocates a fixed-capacity mesh once for `MAX_WEATHER_PARTICLES` quads so
/// per-frame updates only mutate `ATTRIBUTE_POSITION` in place without GPU buffer churn.
fn allocate_weather_mesh() -> Mesh {
    let vert_count = MAX_WEATHER_PARTICLES * 4;
    let index_count = MAX_WEATHER_PARTICLES * 6;
    let positions = vec![[0.0f32; 3]; vert_count];
    let normals = vec![[0.0f32, 0.0f32, 1.0f32]; vert_count];
    let mut uvs = Vec::with_capacity(vert_count);
    let mut indices = Vec::with_capacity(index_count);
    for i in 0..MAX_WEATHER_PARTICLES as u32 {
        uvs.extend_from_slice(&[[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
        let base = i * 4;
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn update_weather_mesh_in_place(
    mesh: &mut Mesh,
    particles: impl Iterator<Item = (Vec3, f32, f32)>,
    cam_right: Vec3,
    prev_count: &mut usize,
) -> usize {
    let Some(VertexAttributeValues::Float32x3(pos)) = mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    else {
        return 0;
    };
    let mut count = 0usize;
    for (center, half_w, half_h) in particles {
        if count >= MAX_WEATHER_PARTICLES {
            break;
        }
        let rx = cam_right.x * half_w;
        let rz = cam_right.z * half_w;
        let base = count * 4;
        pos[base] = [center.x - rx, center.y - half_h, center.z - rz];
        pos[base + 1] = [center.x + rx, center.y - half_h, center.z + rz];
        pos[base + 2] = [center.x + rx, center.y + half_h, center.z + rz];
        pos[base + 3] = [center.x - rx, center.y + half_h, center.z - rz];
        count += 1;
    }
    for i in count..*prev_count {
        let base = i * 4;
        pos[base] = [0.0, 0.0, 0.0];
        pos[base + 1] = [0.0, 0.0, 0.0];
        pos[base + 2] = [0.0, 0.0, 0.0];
        pos[base + 3] = [0.0, 0.0, 0.0];
    }
    *prev_count = count;
    count
}

fn setup_weather_assets(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.insert_resource(WeatherRenderAssets {
        rain_mesh: None,
        snow_mesh: None,
        rain_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(0.62, 0.76, 0.94, 0.60),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            fog_enabled: false,
            cull_mode: None,
            ..default()
        }),
        snow_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.95, 0.97, 1.0),
            unlit: true,
            fog_enabled: false,
            cull_mode: None,
            ..default()
        }),
    });
}

#[allow(clippy::too_many_arguments)]
fn update_weather_fx(
    mut commands: Commands,
    time: Res<Time<Real>>,
    settings: Res<Settings>,
    selected_world: Option<Res<SelectedWorld>>,
    stream: Option<Res<ChunkStream>>,
    mut state: ResMut<WeatherFxState>,
    mut ents: ResMut<WeatherEntities>,
    mut roof_cache: ResMut<SkyRoofCache>,
    assets: Option<ResMut<WeatherRenderAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    local_player: Option<Res<LocalPlayer>>,
    camera_q: Query<&Transform, (With<Camera3d>, Without<DirectionalLight>)>,
    mut ambient: Option<ResMut<GlobalAmbientLight>>,
    mut sun_q: Query<&mut DirectionalLight, With<DirectionalLight>>,
    mut sfx: MessageWriter<SfxEvent>,
    mut vis_q: Query<&mut Visibility, (Without<Camera3d>, Without<DirectionalLight>)>,
) {
    let cam_tf = camera_q.iter().next().copied();
    let center = local_player
        .as_ref()
        .map(|lp| Vec3::from_array(lp.0.body.pos))
        .or_else(|| cam_tf.map(|t| t.translation))
        .unwrap_or(Vec3::ZERO);

    let cam_right = cam_tf.map_or(Vec3::X, |t| {
        let fwd = t.forward().as_vec3();
        Vec3::new(-fwd.z, 0.0, fwd.x).normalize_or(Vec3::X)
    });

    let seed = selected_world
        .as_ref()
        .map_or(crate::view::SEED, |w| w.seed);
    let world_size = selected_world.as_ref().map_or(WorldSize::Small, |w| w.size);
    let wx = center.x.floor() as i32;
    let wz = center.z.floor() as i32;

    let biome = match (ents.last_biome_xz, ents.cached_biome) {
        (Some([lx, lz]), Some(b)) if (wx - lx).abs() < 4 && (wz - lz).abs() < 4 => b,
        _ => {
            let weights = biome_weights(seed, wx, wz);
            let b = dominant(&weights);
            ents.last_biome_xz = Some([wx, wz]);
            ents.cached_biome = Some(b);
            b
        }
    };

    if let Some(stream) = stream.as_deref() {
        roof_cache.refresh(wx, wz, stream, seed, world_size);
    }

    let player_roof = roof_cache.sample(center.x, center.z);
    let sheltered = player_roof > center.y + 2.2;
    let deep_underground = player_roof > center.y + 6.0;

    if state.step_with_roof(
        time.delta_secs(),
        center,
        biome,
        &settings,
        sheltered,
        |x, z| roof_cache.sample(x, z),
    ) {
        sfx.write(SfxEvent(SfxKind::Thunder));
    }

    if settings.weather_effects {
        let wet_mult = state.ground_darkening_multiplier();
        let flash = if deep_underground {
            0.0
        } else {
            state.lightning_flash
        };
        if let Some(amb) = ambient.as_mut() {
            amb.brightness = amb.brightness * wet_mult + flash * 900.0;
        }
        if flash > 0.0 {
            for mut light in &mut sun_q {
                light.illuminance += flash * 14_000.0;
            }
        }
    }

    let Some(mut assets) = assets else {
        return;
    };

    // Update rain mesh in-place (0 allocations, fixed GPU buffer).
    let has_rain_particles = state.particles.iter().any(|p| !p.is_snow);
    let has_rain = if has_rain_particles {
        let handle = assets
            .rain_mesh
            .get_or_insert_with(|| meshes.add(allocate_weather_mesh()))
            .clone();
        let count = if let Some(mut mesh) = meshes.get_mut(&handle) {
            update_weather_mesh_in_place(
                &mut mesh,
                state
                    .particles
                    .iter()
                    .filter(|p| !p.is_snow)
                    .map(|p| (p.pos, p.size * 0.5, p.size * 5.5)),
                cam_right,
                &mut ents.prev_rain_count,
            )
        } else {
            0
        };
        if ents.rain.is_none() && count > 0 {
            ents.rain = Some(
                commands
                    .spawn((
                        Mesh3d(handle),
                        MeshMaterial3d(assets.rain_mat.clone()),
                        Transform::default(),
                        Visibility::Visible,
                        NotShadowCaster,
                        NotShadowReceiver,
                        NoFrustumCulling,
                        DespawnOnExit(AppState::InGame),
                    ))
                    .id(),
            );
        }
        count > 0
    } else {
        false
    };
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

    // Update snow mesh in-place (0 allocations, fixed GPU buffer).
    let has_snow_particles = state.particles.iter().any(|p| p.is_snow);
    let has_snow = if has_snow_particles {
        let handle = assets
            .snow_mesh
            .get_or_insert_with(|| meshes.add(allocate_weather_mesh()))
            .clone();
        let count = if let Some(mut mesh) = meshes.get_mut(&handle) {
            update_weather_mesh_in_place(
                &mut mesh,
                state
                    .particles
                    .iter()
                    .filter(|p| p.is_snow)
                    .map(|p| (p.pos, p.size * 0.5, p.size * 0.5)),
                cam_right,
                &mut ents.prev_snow_count,
            )
        } else {
            0
        };
        if ents.snow.is_none() && count > 0 {
            ents.snow = Some(
                commands
                    .spawn((
                        Mesh3d(handle),
                        MeshMaterial3d(assets.snow_mat.clone()),
                        Transform::default(),
                        Visibility::Visible,
                        NotShadowCaster,
                        NotShadowReceiver,
                        NoFrustumCulling,
                        DespawnOnExit(AppState::InGame),
                    ))
                    .id(),
            );
        }
        count > 0
    } else {
        false
    };
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

    #[test]
    fn rain_does_not_spawn_or_fall_inside_caves() {
        let settings = Settings::default();
        let mut state = WeatherFxState {
            kind: WeatherKind::Storm,
            ..Default::default()
        };
        let player_pos = Vec3::new(10.0, 100.0, 10.0);
        // Simulate a solid cave ceiling at y = 270 everywhere around the player.
        for _ in 0..20 {
            state.step_with_roof(0.05, player_pos, Biome::Forest, &settings, true, |_, _| {
                270.0
            });
        }
        assert!(
            state.particles.is_empty(),
            "no rain particles should spawn inside a cave"
        );
        assert_eq!(
            state.wet_ground_factor, 0.0,
            "sheltered cave should not apply wet ground darkening"
        );

        // Existing particle falling onto a roof at y = 105 should despawn on impact.
        state.particles.push(WeatherParticle {
            pos: Vec3::new(10.0, 105.2, 10.0),
            vel: Vec3::new(0.0, -20.0, 0.0),
            is_snow: false,
            age: 0.0,
            lifetime: 1.0,
            size: 0.05,
        });
        state.step_with_roof(0.05, player_pos, Biome::Forest, &settings, true, |_, _| {
            105.0
        });
        assert!(
            state.particles.is_empty(),
            "particle hitting roof should be removed"
        );
    }
}
