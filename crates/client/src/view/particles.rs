//! Pooled voxel hit debris, death bursts, projectile trails, and muzzle flashes.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use terravibe_sim::net::{DamageKind, HitTargetKind};
use terravibe_sim::{
    ANVIL, ASH, BLUE_DUNGEON_BRICK, BlockId, CAMPFIRE, CHAIR, CHEST, CLOUD_BLOCK, COPPER,
    CORRUPT_GRASS, DEMON_ALTAR, DEMONITE_ORE, DIRT, DOOR_CLOSED, DOOR_OPEN, EBONSTONE, FURNACE,
    GLASS_BLOCK, GLOWING_MUSHROOM_PLANT, GOLD, GRASS, GREEN_DUNGEON_BRICK, HARDENED_SAND,
    HEART_LANTERN, HELLFORGE, HELLSTONE, HELLSTONE_BRICK, HIVE_BLOCK, ICE, IRON, JUNGLE_GRASS,
    LIFE_CRYSTAL, METEORITE_ORE, MUD, MUSHROOM_GRASS, OBSIDIAN, OBSIDIAN_BRICK, PINK_DUNGEON_BRICK,
    PLATFORM, SAND, SANDSTONE, SHADOW_ORB, SILVER, SNOW, SPIKES, STONE, SUNPLATE_BLOCK, TABLE,
    TINKERERS_WORKSHOP, TORCH, TRUNK, WOOD_BLOCK, WORKBENCH,
};

use crate::hud::combat_feedback::CombatHitEvent;
use crate::session::AppState;
use crate::settings::Settings;

/// Maximum concurrent particles in [`ParticlePool`].
pub const MAX_PARTICLES: usize = 256;
/// Maximum concurrent ambient particles in [`AmbientParticlePool`].
pub const MAX_AMBIENT_PARTICLES: usize = 96;

/// Effective [`ParticlePool`] cap scaled by `settings.quality_preset`.
pub fn max_particles_for(settings: &Settings) -> usize {
    ((MAX_PARTICLES as f32) * settings.quality_preset.particle_scale())
        .round()
        .max(1.0) as usize
}

/// Effective [`AmbientParticlePool`] cap scaled by `settings.quality_preset`.
pub fn max_ambient_particles_for(settings: &Settings) -> usize {
    ((MAX_AMBIENT_PARTICLES as f32) * settings.quality_preset.particle_scale())
        .round()
        .max(1.0) as usize
}

/// Surface or target material for hit debris coloring.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum HitMaterial {
    Wood,
    Stone,
    Dirt,
    Metal,
    Flesh,
}

/// Returns a distinct voxel particle color for the given [`HitMaterial`].
pub fn particle_color(mat: HitMaterial) -> Color {
    match mat {
        HitMaterial::Wood => Color::srgb(0.72, 0.50, 0.25),
        HitMaterial::Stone => Color::srgb(0.52, 0.54, 0.58),
        HitMaterial::Dirt => Color::srgb(0.42, 0.26, 0.14),
        HitMaterial::Metal => Color::srgb(0.78, 0.82, 0.88),
        HitMaterial::Flesh => Color::srgb(0.82, 0.16, 0.18),
    }
}

/// Maps a [`BlockId`] to a [`HitMaterial`] for tile hit/break debris.
pub fn material_for_block(block: BlockId) -> HitMaterial {
    match block {
        WOOD_BLOCK | TRUNK | WORKBENCH | TABLE | CHAIR | PLATFORM | DOOR_CLOSED | DOOR_OPEN
        | CHEST | TORCH | CAMPFIRE | TINKERERS_WORKSHOP => HitMaterial::Wood,
        IRON | COPPER | GOLD | SILVER | DEMONITE_ORE | HELLSTONE | ANVIL | SPIKES | HELLFORGE
        | METEORITE_ORE | HEART_LANTERN => HitMaterial::Metal,
        DIRT
        | GRASS
        | MUD
        | JUNGLE_GRASS
        | CORRUPT_GRASS
        | SAND
        | SNOW
        | ASH
        | MUSHROOM_GRASS
        | CLOUD_BLOCK
        | GLOWING_MUSHROOM_PLANT => HitMaterial::Dirt,
        LIFE_CRYSTAL => HitMaterial::Flesh,
        STONE | EBONSTONE | SANDSTONE | HARDENED_SAND | OBSIDIAN | OBSIDIAN_BRICK
        | HELLSTONE_BRICK | BLUE_DUNGEON_BRICK | GREEN_DUNGEON_BRICK | PINK_DUNGEON_BRICK
        | FURNACE | GLASS_BLOCK | ICE | DEMON_ALTAR | SHADOW_ORB | SUNPLATE_BLOCK | HIVE_BLOCK
        | _ => HitMaterial::Stone,
    }
}

/// A single pooled voxel particle.
#[derive(Clone, Debug)]
pub struct ParticleEntry {
    pub pos: Vec3,
    pub vel: Vec3,
    pub color: Color,
    pub size: f32,
    pub age: f32,
    pub lifetime: f32,
    pub gravity: f32,
}

/// Fixed-capacity particle pool capped at [`MAX_PARTICLES`].
#[derive(Resource, Default, Debug)]
pub struct ParticlePool {
    pub entries: Vec<ParticleEntry>,
    rng_state: u32,
}

impl ParticlePool {
    fn next_unit(&mut self) -> f32 {
        let mut x = if self.rng_state == 0 {
            0x9E37_79B9
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

    /// Spawns a single particle if `settings.reduced_motion` is false, evicting the oldest if full.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        &mut self,
        pos: Vec3,
        vel: Vec3,
        color: Color,
        size: f32,
        lifetime: f32,
        gravity: f32,
        settings: &Settings,
    ) {
        if settings.reduced_motion || lifetime <= 0.0 {
            return;
        }
        let cap = max_particles_for(settings);
        while self.entries.len() >= cap {
            self.entries.remove(0);
        }
        self.entries.push(ParticleEntry {
            pos,
            vel,
            color,
            size,
            age: 0.0,
            lifetime,
            gravity,
        });
    }

    /// Spawns a radial burst of `count` particles around `pos`.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_burst(
        &mut self,
        pos: Vec3,
        count: usize,
        color: Color,
        speed: f32,
        size: f32,
        lifetime: f32,
        gravity: f32,
        settings: &Settings,
    ) {
        if settings.reduced_motion {
            return;
        }
        for _ in 0..count {
            let dir = Vec3::new(
                self.next_signed(),
                self.next_unit() * 1.1 + 0.2,
                self.next_signed(),
            )
            .normalize_or(Vec3::Y);
            let mag = speed * (0.55 + 0.65 * self.next_unit());
            let jitter = Vec3::new(
                self.next_signed() * 0.12,
                self.next_signed() * 0.12,
                self.next_signed() * 0.12,
            );
            let s = size * (0.8 + 0.4 * self.next_unit());
            let lt = lifetime * (0.8 + 0.4 * self.next_unit());
            self.spawn(pos + jitter, dir * mag, color, s, lt, gravity, settings);
        }
    }

    /// Advances particle simulation by `dt` seconds and removes expired entries.
    pub fn tick(&mut self, dt: f32) {
        for entry in &mut self.entries {
            entry.age += dt;
            entry.vel.y -= entry.gravity * dt;
            entry.pos += entry.vel * dt;
        }
        self.entries.retain(|e| e.age < e.lifetime);
    }
}

/// Atmospheric ambient particle categories.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum AmbientParticleKind {
    DustMote,
    Firefly,
    UnderworldEmber,
    CaveDrip,
}

/// A single pooled ambient particle.
#[derive(Clone, Debug)]
pub struct AmbientParticleEntry {
    pub kind: AmbientParticleKind,
    pub pos: Vec3,
    pub vel: Vec3,
    pub color: Color,
    pub size: f32,
    pub age: f32,
    pub lifetime: f32,
    pub gravity: f32,
}

/// Fixed-capacity ambient particle pool capped at [`MAX_AMBIENT_PARTICLES`].
#[derive(Resource, Default, Debug)]
pub struct AmbientParticlePool {
    pub entries: Vec<AmbientParticleEntry>,
    rng_state: u32,
}

impl AmbientParticlePool {
    fn next_unit(&mut self) -> f32 {
        let mut x = if self.rng_state == 0 {
            0xB7E1_5163
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

    /// Spawns a single ambient particle if `settings.ambient_particles` is true and `reduced_motion` is false.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        &mut self,
        kind: AmbientParticleKind,
        pos: Vec3,
        vel: Vec3,
        color: Color,
        size: f32,
        lifetime: f32,
        gravity: f32,
        settings: &Settings,
    ) {
        if !settings.ambient_particles || settings.reduced_motion || lifetime <= 0.0 {
            return;
        }
        let cap = max_ambient_particles_for(settings);
        while self.entries.len() >= cap {
            self.entries.remove(0);
        }
        self.entries.push(AmbientParticleEntry {
            kind,
            pos,
            vel,
            color,
            size,
            age: 0.0,
            lifetime,
            gravity,
        });
    }

    /// Spawns a contextual ambient particle (dust mote, night firefly, underworld ember, or cave drip).
    pub fn spawn_contextual(
        &mut self,
        center: Vec3,
        depth: i32,
        is_night: bool,
        is_underworld: bool,
        settings: &Settings,
    ) {
        if !settings.ambient_particles || settings.reduced_motion {
            return;
        }
        let offset = Vec3::new(
            self.next_signed() * 12.0,
            self.next_unit() * 7.0 - 1.5,
            self.next_signed() * 12.0,
        );
        if is_underworld {
            let vel = Vec3::new(
                self.next_signed() * 0.4,
                0.8 + self.next_unit() * 0.9,
                self.next_signed() * 0.4,
            );
            let size = 0.08 + self.next_unit() * 0.05;
            let lt = 2.5 + self.next_unit() * 1.5;
            self.spawn(
                AmbientParticleKind::UnderworldEmber,
                center + offset,
                vel,
                Color::srgb(1.0, 0.42, 0.12),
                size,
                lt,
                0.0,
                settings,
            );
        } else if depth > 0 {
            if self.next_unit() < 0.45 {
                let pos = center + Vec3::new(offset.x, 4.0 + self.next_unit() * 4.0, offset.z);
                let lt = 1.1 + self.next_unit() * 0.5;
                self.spawn(
                    AmbientParticleKind::CaveDrip,
                    pos,
                    Vec3::new(0.0, -1.5, 0.0),
                    Color::srgb(0.45, 0.78, 0.98),
                    0.06,
                    lt,
                    9.8,
                    settings,
                );
            } else {
                let vel = Vec3::new(
                    self.next_signed() * 0.15,
                    self.next_signed() * 0.08,
                    self.next_signed() * 0.15,
                );
                self.spawn(
                    AmbientParticleKind::DustMote,
                    center + offset,
                    vel,
                    Color::srgb(0.72, 0.72, 0.68),
                    0.05,
                    3.2,
                    0.0,
                    settings,
                );
            }
        } else if is_night {
            let vel = Vec3::new(
                self.next_signed() * 0.45,
                self.next_signed() * 0.25,
                self.next_signed() * 0.45,
            );
            let lt = 3.0 + self.next_unit() * 2.0;
            self.spawn(
                AmbientParticleKind::Firefly,
                center + offset,
                vel,
                Color::srgb(0.82, 1.0, 0.35),
                0.08,
                lt,
                0.0,
                settings,
            );
        } else {
            let vel = Vec3::new(
                self.next_signed() * 0.2,
                0.05 + self.next_signed() * 0.08,
                self.next_signed() * 0.2,
            );
            let lt = 3.5 + self.next_unit() * 1.5;
            self.spawn(
                AmbientParticleKind::DustMote,
                center + offset,
                vel,
                Color::srgb(0.95, 0.92, 0.80),
                0.05,
                lt,
                0.0,
                settings,
            );
        }
    }

    /// Advances ambient particle simulation by `dt` seconds and removes expired entries.
    pub fn tick(&mut self, dt: f32) {
        for entry in &mut self.entries {
            entry.age += dt;
            entry.vel.y -= entry.gravity * dt;
            entry.pos += entry.vel * dt;
        }
        self.entries.retain(|e| e.age < e.lifetime);
    }
}

#[derive(Resource, Default)]
struct ParticleBatchState {
    solid_entity: Option<Entity>,
    emissive_entity: Option<Entity>,
    solid_mesh: Option<Handle<Mesh>>,
    emissive_mesh: Option<Handle<Mesh>>,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

#[derive(Resource)]
struct ParticleRenderAssets {
    solid_mat: Handle<StandardMaterial>,
    emissive_mat: Handle<StandardMaterial>,
}

pub struct ParticlesPlugin;

impl Plugin for ParticlesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ParticlePool>()
            .init_resource::<AmbientParticlePool>()
            .init_resource::<ParticleBatchState>()
            .add_systems(Startup, setup_particle_assets)
            .add_systems(
                OnExit(AppState::InGame),
                |mut pool: ResMut<ParticlePool>,
                 mut amb_pool: ResMut<AmbientParticlePool>,
                 mut batch: ResMut<ParticleBatchState>| {
                    pool.entries.clear();
                    amb_pool.entries.clear();
                    batch.solid_entity = None;
                    batch.emissive_entity = None;
                },
            )
            .add_systems(
                Update,
                (consume_combat_hit_particles, step_and_render_particles)
                    .chain()
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

fn setup_particle_assets(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.insert_resource(ParticleRenderAssets {
        solid_mat: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.9,
            ..default()
        }),
        emissive_mat: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            alpha_mode: AlphaMode::Add,
            ..default()
        }),
    });
}

fn push_cube(batch: &mut ParticleBatchState, center: Vec3, half: f32, color: [f32; 4]) {
    const FACES: [([f32; 3], [[f32; 3]; 4]); 6] = [
        // +X
        (
            [1.0, 0.0, 0.0],
            [
                [1.0, -1.0, 1.0],
                [1.0, -1.0, -1.0],
                [1.0, 1.0, -1.0],
                [1.0, 1.0, 1.0],
            ],
        ),
        // -X
        (
            [-1.0, 0.0, 0.0],
            [
                [-1.0, -1.0, -1.0],
                [-1.0, -1.0, 1.0],
                [-1.0, 1.0, 1.0],
                [-1.0, 1.0, -1.0],
            ],
        ),
        // +Y
        (
            [0.0, 1.0, 0.0],
            [
                [-1.0, 1.0, 1.0],
                [1.0, 1.0, 1.0],
                [1.0, 1.0, -1.0],
                [-1.0, 1.0, -1.0],
            ],
        ),
        // -Y
        (
            [0.0, -1.0, 0.0],
            [
                [-1.0, -1.0, -1.0],
                [1.0, -1.0, -1.0],
                [1.0, -1.0, 1.0],
                [-1.0, -1.0, 1.0],
            ],
        ),
        // +Z
        (
            [0.0, 0.0, 1.0],
            [
                [-1.0, -1.0, 1.0],
                [1.0, -1.0, 1.0],
                [1.0, 1.0, 1.0],
                [-1.0, 1.0, 1.0],
            ],
        ),
        // -Z
        (
            [0.0, 0.0, -1.0],
            [
                [1.0, -1.0, -1.0],
                [-1.0, -1.0, -1.0],
                [-1.0, 1.0, -1.0],
                [1.0, 1.0, -1.0],
            ],
        ),
    ];
    for (n, corners) in FACES {
        let base = batch.positions.len() as u32;
        for c in corners {
            batch.positions.push([
                center.x + c[0] * half,
                center.y + c[1] * half,
                center.z + c[2] * half,
            ]);
            batch.normals.push(n);
            batch.colors.push(color);
        }
        batch
            .indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

fn build_particle_mesh(batch: &ParticleBatchState) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, batch.positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, batch.normals.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, batch.colors.clone());
    mesh.insert_indices(Indices::U32(batch.indices.clone()));
    mesh
}

fn upload_particle_batch(
    handle: &Handle<Mesh>,
    meshes: &mut Assets<Mesh>,
    batch: &ParticleBatchState,
) {
    if let Some(mut mesh) = meshes.get_mut(handle) {
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, batch.positions.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, batch.normals.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, batch.colors.clone());
        mesh.insert_indices(Indices::U32(batch.indices.clone()));
    }
}

fn consume_combat_hit_particles(
    mut events: MessageReader<CombatHitEvent>,
    mut pool: ResMut<ParticlePool>,
    settings: Res<Settings>,
) {
    for ev in events.read() {
        let hit_color = match ev.damage_kind {
            DamageKind::Magic => Color::srgb(0.45, 0.85, 1.0),
            _ => particle_color(HitMaterial::Flesh),
        };
        let hit_count = if ev.crit { 10 } else { 6 };
        pool.spawn_burst(
            ev.pos, hit_count, hit_color, 3.2, 0.10, 0.45, 12.0, &settings,
        );

        if ev.killed {
            let burst_count = if ev.target_kind == HitTargetKind::Boss {
                28
            } else {
                18
            };
            pool.spawn_burst(
                ev.pos,
                burst_count,
                particle_color(HitMaterial::Flesh),
                4.8,
                0.14,
                0.70,
                10.0,
                &settings,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn step_and_render_particles(
    mut commands: Commands,
    time: Res<Time<Real>>,
    settings: Res<Settings>,
    selected_world: Option<Res<crate::session::SelectedWorld>>,
    mut pool: ResMut<ParticlePool>,
    mut amb_pool: ResMut<AmbientParticlePool>,
    mut batch: ResMut<ParticleBatchState>,
    assets: Option<Res<ParticleRenderAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    local_player: Option<Res<crate::view::player::LocalPlayer>>,
    clock: Option<Res<crate::net::ClientClock>>,
    stream: Option<Res<crate::view::chunks::ChunkStream>>,
    mut vis_q: Query<&mut Visibility>,
) {
    let dt = time.delta_secs();
    if settings.reduced_motion {
        pool.entries.clear();
        amb_pool.entries.clear();
    } else {
        pool.tick(dt);
        if !settings.ambient_particles {
            amb_pool.entries.clear();
        } else {
            amb_pool.tick(dt);
            let center = local_player
                .as_ref()
                .map(|lp| Vec3::from_array(lp.0.body.pos))
                .unwrap_or(Vec3::ZERO);
            let wx = center.x.floor() as i32;
            let wy = center.y.floor() as i32;
            let wz = center.z.floor() as i32;
            let seed = selected_world
                .as_ref()
                .map_or(crate::view::SEED, |w| w.seed);
            let surface = terravibe_sim::worldgen::terrain::surface_height(seed, wx, wz);
            let depth = (surface - wy) - 15;
            let is_underworld = depth >= 120;
            let is_night = clock.as_ref().is_some_and(|c| {
                !terravibe_sim::time::WorldClock {
                    tick: c.effective_tick(),
                }
                .is_day()
            });
            if amb_pool.entries.len() < max_ambient_particles_for(&settings) {
                amb_pool.spawn_contextual(center, depth, is_night, is_underworld, &settings);
            }
            if let Some(stream) = stream.as_deref()
                && amb_pool.entries.len() < max_ambient_particles_for(&settings)
            {
                let chunks = stream.generated();
                for _ in 0..8 {
                    let ox = (amb_pool.next_signed() * 14.0).round() as i32;
                    let oy = (amb_pool.next_signed() * 8.0).round() as i32;
                    let oz = (amb_pool.next_signed() * 14.0).round() as i32;
                    let lx_w = wx + ox;
                    let ly_w = wy + oy;
                    let lz_w = wz + oz;
                    let cp = terravibe_sim::ChunkPos::of_block(lx_w, ly_w, lz_w);
                    let Some(ch) = chunks.get(&cp) else { continue };
                    let lx = lx_w.rem_euclid(terravibe_sim::CHUNK_SIZE) as usize;
                    let ly = ly_w.rem_euclid(terravibe_sim::CHUNK_SIZE) as usize;
                    let lz = lz_w.rem_euclid(terravibe_sim::CHUNK_SIZE) as usize;
                    let Some(liq) = ch.liquid(lx, ly, lz) else {
                        continue;
                    };
                    if liq.kind != terravibe_sim::LiquidKind::Lava {
                        continue;
                    }
                    let above_clear = if ly + 1 < terravibe_sim::CHUNK_SIZE as usize {
                        ch.get(lx, ly + 1, lz) == terravibe_sim::AIR
                            && ch.liquid(lx, ly + 1, lz).is_none()
                    } else {
                        let acp = terravibe_sim::ChunkPos::of_block(lx_w, ly_w + 1, lz_w);
                        chunks.get(&acp).is_some_and(|ac| {
                            ac.get(lx, 0, lz) == terravibe_sim::AIR
                                && ac.liquid(lx, 0, lz).is_none()
                        })
                    };
                    if !above_clear {
                        continue;
                    }
                    let top_y = ly_w as f32 + (liq.amount as f32 / 255.0).max(0.1);
                    let pos = Vec3::new(
                        lx_w as f32 + 0.15 + amb_pool.next_unit() * 0.7,
                        top_y + 0.05,
                        lz_w as f32 + 0.15 + amb_pool.next_unit() * 0.7,
                    );
                    let vel = Vec3::new(
                        amb_pool.next_signed() * 0.45,
                        1.2 + amb_pool.next_unit() * 1.6,
                        amb_pool.next_signed() * 0.45,
                    );
                    let col = if amb_pool.next_unit() < 0.4 {
                        Color::srgb(1.0, 0.85, 0.25)
                    } else {
                        Color::srgb(1.0, 0.48, 0.10)
                    };
                    let size = 0.08 + amb_pool.next_unit() * 0.07;
                    let lt = 0.8 + amb_pool.next_unit() * 1.0;
                    amb_pool.spawn(
                        AmbientParticleKind::UnderworldEmber,
                        pos,
                        vel,
                        col,
                        size,
                        lt,
                        0.0,
                        &settings,
                    );
                    break;
                }
            }
        }
    }

    let Some(assets) = assets else {
        return;
    };

    // 1. Solid/opaque particles batch (debris + dust motes + cave drips).
    batch.positions.clear();
    batch.normals.clear();
    batch.colors.clear();
    batch.indices.clear();
    for entry in &pool.entries {
        let life_frac = (1.0 - entry.age / entry.lifetime).clamp(0.1, 1.0);
        let half = entry.size * life_frac * 0.5;
        let col = entry.color.to_linear().to_f32_array();
        push_cube(&mut batch, entry.pos, half, col);
    }
    for entry in &amb_pool.entries {
        if matches!(
            entry.kind,
            AmbientParticleKind::Firefly | AmbientParticleKind::UnderworldEmber
        ) {
            continue;
        }
        let life_frac = (1.0 - entry.age / entry.lifetime).clamp(0.2, 1.0);
        let half = entry.size * life_frac * 0.5;
        let col = entry.color.to_linear().to_f32_array();
        push_cube(&mut batch, entry.pos, half, col);
    }
    let has_solid = !batch.positions.is_empty();
    if has_solid {
        if batch.solid_mesh.is_none() {
            batch.solid_mesh = Some(meshes.add(build_particle_mesh(&batch)));
        }
        let handle = batch.solid_mesh.as_ref().unwrap().clone();
        if batch.solid_entity.is_none() {
            batch.solid_entity = Some(
                commands
                    .spawn((
                        Mesh3d(handle),
                        MeshMaterial3d(assets.solid_mat.clone()),
                        Transform::default(),
                        Visibility::Visible,
                        DespawnOnExit(AppState::InGame),
                    ))
                    .id(),
            );
        } else {
            upload_particle_batch(&handle, &mut meshes, &batch);
        }
    }
    if let Some(e) = batch.solid_entity
        && let Ok(mut vis) = vis_q.get_mut(e)
    {
        let target = if has_solid {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *vis != target {
            *vis = target;
        }
    }

    // 2. Emissive/additive particles batch (fireflies + underworld embers).
    batch.positions.clear();
    batch.normals.clear();
    batch.colors.clear();
    batch.indices.clear();
    for entry in &amb_pool.entries {
        if !matches!(
            entry.kind,
            AmbientParticleKind::Firefly | AmbientParticleKind::UnderworldEmber
        ) {
            continue;
        }
        let life_frac = (1.0 - entry.age / entry.lifetime).clamp(0.2, 1.0);
        let pulse = if entry.kind == AmbientParticleKind::Firefly {
            0.75 + 0.35 * (entry.age * 5.0).sin()
        } else {
            1.0
        };
        let half = entry.size * life_frac * pulse * 0.5;
        let col = entry.color.to_linear().to_f32_array();
        push_cube(&mut batch, entry.pos, half, col);
    }
    let has_emissive = !batch.positions.is_empty();
    if has_emissive {
        if batch.emissive_mesh.is_none() {
            batch.emissive_mesh = Some(meshes.add(build_particle_mesh(&batch)));
        }
        let handle = batch.emissive_mesh.as_ref().unwrap().clone();
        if batch.emissive_entity.is_none() {
            batch.emissive_entity = Some(
                commands
                    .spawn((
                        Mesh3d(handle),
                        MeshMaterial3d(assets.emissive_mat.clone()),
                        Transform::default(),
                        Visibility::Visible,
                        DespawnOnExit(AppState::InGame),
                    ))
                    .id(),
            );
        } else {
            upload_particle_batch(&handle, &mut meshes, &batch);
        }
    }
    if let Some(e) = batch.emissive_entity
        && let Ok(mut vis) = vis_q.get_mut(e)
    {
        let target = if has_emissive {
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
    fn particle_pool_caps_at_256_and_expires() {
        let mut settings = Settings::default();
        settings.quality_preset = crate::settings::QualityPreset::High;
        let mut pool = ParticlePool::default();
        for i in 0..320 {
            pool.spawn(
                Vec3::new(i as f32 * 0.1, 1.0, 0.0),
                Vec3::Y,
                particle_color(HitMaterial::Stone),
                0.1,
                0.5,
                9.8,
                &settings,
            );
        }
        assert_eq!(pool.entries.len(), MAX_PARTICLES);
        assert_eq!(MAX_PARTICLES, 256);

        pool.tick(0.25);
        assert_eq!(pool.entries.len(), MAX_PARTICLES);

        pool.tick(0.30);
        assert!(pool.entries.is_empty());
    }

    #[test]
    fn material_colors_are_distinct() {
        let mats = [
            HitMaterial::Wood,
            HitMaterial::Stone,
            HitMaterial::Dirt,
            HitMaterial::Metal,
            HitMaterial::Flesh,
        ];
        let colors: Vec<Color> = mats.iter().copied().map(particle_color).collect();
        for i in 0..colors.len() {
            for j in (i + 1)..colors.len() {
                assert_ne!(
                    colors[i], colors[j],
                    "expected distinct colors for {:?} and {:?}",
                    mats[i], mats[j]
                );
            }
        }
    }

    #[test]
    fn reduced_motion_suppresses_particles() {
        let mut settings = Settings::default();
        settings.reduced_motion = true;
        let mut pool = ParticlePool::default();
        pool.spawn(
            Vec3::ZERO,
            Vec3::Y,
            particle_color(HitMaterial::Flesh),
            0.1,
            0.5,
            9.8,
            &settings,
        );
        pool.spawn_burst(
            Vec3::ZERO,
            12,
            particle_color(HitMaterial::Wood),
            3.0,
            0.1,
            0.5,
            9.8,
            &settings,
        );
        assert!(pool.entries.is_empty());
    }

    #[test]
    fn material_for_block_maps_categories() {
        assert_eq!(material_for_block(WOOD_BLOCK), HitMaterial::Wood);
        assert_eq!(material_for_block(TRUNK), HitMaterial::Wood);
        assert_eq!(material_for_block(STONE), HitMaterial::Stone);
        assert_eq!(material_for_block(DIRT), HitMaterial::Dirt);
        assert_eq!(material_for_block(IRON), HitMaterial::Metal);
        assert_eq!(material_for_block(LIFE_CRYSTAL), HitMaterial::Flesh);
    }

    #[test]
    fn ambient_particles_respect_cap_and_settings_toggle() {
        let mut settings = Settings::default();
        settings.quality_preset = crate::settings::QualityPreset::High;
        let mut pool = AmbientParticlePool::default();
        for i in 0..(MAX_AMBIENT_PARTICLES + 40) {
            pool.spawn(
                AmbientParticleKind::DustMote,
                Vec3::new(i as f32 * 0.1, 1.0, 0.0),
                Vec3::ZERO,
                Color::WHITE,
                0.08,
                2.0,
                0.0,
                &settings,
            );
        }
        assert_eq!(pool.entries.len(), MAX_AMBIENT_PARTICLES);

        settings.ambient_particles = false;
        let before = pool.entries.len();
        pool.spawn_contextual(Vec3::ZERO, 0, true, false, &settings);
        assert_eq!(pool.entries.len(), before);
    }

    #[test]
    fn quality_preset_scales_particle_caps() {
        use crate::settings::QualityPreset;

        let mut low = Settings::default();
        low.quality_preset = QualityPreset::Low;
        let mut med = Settings::default();
        med.quality_preset = QualityPreset::Medium;
        let mut high = Settings::default();
        high.quality_preset = QualityPreset::High;

        assert!(QualityPreset::Low.particle_scale() < QualityPreset::Medium.particle_scale());
        assert!(QualityPreset::Medium.particle_scale() < QualityPreset::High.particle_scale());
        assert_eq!(QualityPreset::High.particle_scale(), 1.0);

        let mut pool_low = ParticlePool::default();
        let mut pool_high = ParticlePool::default();
        for i in 0..400 {
            let p = Vec3::new(i as f32 * 0.1, 1.0, 0.0);
            pool_low.spawn(p, Vec3::Y, Color::WHITE, 0.1, 1.0, 9.8, &low);
            pool_high.spawn(p, Vec3::Y, Color::WHITE, 0.1, 1.0, 9.8, &high);
        }
        assert_eq!(pool_low.entries.len(), max_particles_for(&low));
        assert_eq!(pool_high.entries.len(), MAX_PARTICLES);
        assert!(pool_low.entries.len() < max_particles_for(&med));
        assert!(max_particles_for(&med) < pool_high.entries.len());
    }
}
