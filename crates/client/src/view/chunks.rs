//! Chunk streaming: mesh the chunks the server sends around the camera on the
//! async compute pool, never blocking the frame.
//!
//! World sharing: received chunks live on the main thread in
//! [`ChunkStream::generated`] as `Arc<Chunk>`. A meshing task gets `Arc`
//! clones of its chunk and its six face neighbours (all `mesh_chunk` reads)
//! and builds a private snapshot [`World`] from them on the worker thread.
//! No locks; neighbours generated later trigger a re-mesh.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
};
use bevy::tasks::{AsyncComputeTaskPool, Task, futures::check_ready};
use terravibe_sim::net::VIEW_RADIUS_BLOCKS;
use terravibe_sim::{CHUNK_SIZE, Chunk, ChunkPos, Liquid, WorldBounds};

use super::block_models::{
    BlockModels, ChunkModelsOutput, build_chunk_models, collect_model_chunks, setup_block_models,
    spawn_chunk_models, spawn_chunk_models_output,
};
use super::camera::FlyCamera;
use super::liquid_mesh::mesh_liquid;
use super::to_bevy_image;
use crate::art::blocks::{BlockArt, TILE};
use crate::art::chunk_mesh::{ChunkMeshInput, mesh_chunk_greedy};
use crate::light::engine::{LightInput, light_chunk_input};
use crate::light::{
    ChunkMaterial, LIGHT_BORDER, LightExt, LightVolume, relight_below, relight_below_edit,
    relight_targets,
};
use crate::session::AppState;
/// Only chunk layers `0..CHUNK_LAYERS` have blocks.
const CHUNK_LAYERS: i32 = terravibe_sim::WORLD_CHUNK_LAYERS;
/// Max meshing tasks spawned per frame.
const SPAWN_BUDGET: usize = 12;
/// Max light tasks spawned per frame. Lighting is cheap enough to run on the
/// same compute pool as meshing, but a frame never queues more than this.
const LIGHT_SPAWN_BUDGET: usize = 8;
/// Quads per mesh part (16384 quads = 65536 vertices, u16 indices).
const PART_QUADS: usize = 16_384;
/// Mesh parts and vertex budget handed to the GPU per frame. Always uploads at
/// least one part per frame so large chunks never stall, while allowing many
/// small/medium chunks to finish in a single frame.
const UPLOAD_PARTS_PER_FRAME: usize = 4;
const UPLOAD_VERTS_PER_FRAME: usize = 65_536;
/// Max finished light volumes uploaded to GPU (`images.add`) per frame.
const LIGHT_UPLOADS_PER_FRAME: usize = 2;

const NEIGHBOURS: [[i32; 3]; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];

pub struct ChunksPlugin {
    pub art: Arc<BlockArt>,
}

impl Plugin for ChunksPlugin {
    fn build(&self, app: &mut App) {
        crate::light::material::register(app);
        app.insert_resource(ArtRes(self.art.clone()))
            .init_resource::<ChunkStream>()
            .add_systems(Startup, (setup_material, setup_block_models))
            .add_systems(
                Update,
                (
                    stream_chunks,
                    update_torch_flicker,
                    animate_liquid_materials,
                )
                    .run_if(in_state(AppState::InGame)),
            );
        if std::env::var_os("TERRAVIBE_SPIKE_LOG").is_some_and(|v| v == "1") {
            app.add_systems(
                Update,
                spike_log
                    .after(stream_chunks)
                    .run_if(in_state(AppState::InGame)),
            );
        }
    }
}

/// What `stream_chunks` did in one frame (for the `TERRAVIBE_SPIKE_LOG=1` log).
#[derive(Default, Clone, Copy, Debug)]
pub(crate) struct FrameStats {
    mesh_done: u32,
    inserted: u32,
    verts: u32,
    despawned: u32,
    mesh_spawned: u32,
    light_done: u32,
    light_spawned: u32,
    view_rebuilt: bool,
    stream_us: u32,
}

/// Logs every frame over 12 ms with the streaming work of it and the two
/// frames before it (render runs one frame behind the main world).
fn spike_log(
    time: Res<Time<Real>>,
    stream: Res<ChunkStream>,
    mut hist: Local<[FrameStats; 2]>,
    mut frame: Local<u64>,
) {
    *frame += 1;
    let dt = time.delta_secs() * 1000.0;
    if dt > 12.0 {
        info!(
            "SPIKE f={} t={:.2}s dt={dt:.2}ms now={:?} prev={:?} prev2={:?}",
            *frame,
            time.elapsed_secs(),
            stream.frame,
            hist[0],
            hist[1]
        );
    }
    hist[1] = hist[0];
    hist[0] = stream.frame;
}

#[derive(Resource)]
pub(crate) struct ArtRes(pub(crate) Arc<BlockArt>);

/// Shared inputs for every chunk material instance: the atlas-textured base
/// and the full-white light texture used until a chunk is lit.
/// Deterministic torch flicker factor around `1.0` (`0.85..=1.15`).
pub fn torch_flicker_factor(time: f32, seed: u32) -> f32 {
    let phase = (seed.wrapping_mul(0x9E37_79B1) as f32) * 0.0001;
    let wave = (time * 7.3 + phase).sin() * 0.65 + (time * 13.7 + phase * 1.7).sin() * 0.35;
    (1.0 + wave * 0.12).clamp(0.85, 1.15)
}

#[derive(Component)]
pub(crate) struct TorchFlicker {
    pub(crate) seed: u32,
}

fn update_torch_flicker(
    time: Res<Time>,
    settings: Option<Res<crate::settings::Settings>>,
    mut q: Query<(&TorchFlicker, &mut Transform)>,
) {
    let enabled = settings.as_ref().is_none_or(|s| s.atmosphere_fx);
    let t = time.elapsed_secs();
    for (flicker, mut tf) in &mut q {
        if enabled {
            let f = torch_flicker_factor(t, flicker.seed);
            tf.scale = Vec3::new(0.95 + 0.05 * f, f, 0.95 + 0.05 * f);
        } else if tf.scale != Vec3::ONE {
            tf.scale = Vec3::ONE;
        }
    }
}

/// Marker component for the liquid mesh entity of a chunk.
#[derive(Component)]
pub struct LiquidMeshEntity;

#[derive(Resource)]
pub(crate) struct ChunkMat {
    pub(crate) base: StandardMaterial,
    pub(crate) terrain_opaque_base: StandardMaterial,
    pub(crate) terrain_masked_base: StandardMaterial,
    pub(crate) liquid_base: StandardMaterial,
    pub(crate) tiles: Handle<Image>,
    pub(crate) default_light: Handle<Image>,
}

struct Meshed {
    /// One entity per mesh part; empty for an empty chunk.
    entities: Vec<Entity>,
    /// Separate liquid mesh entity so liquid updates avoid re-meshing terrain.
    liquid_entity: Option<Entity>,
    /// This chunk's material (`LightExt::light` is swapped when it relights).
    material: Handle<ChunkMaterial>,
    /// Liquid mesh material (`LightExt::light` is swapped when it relights).
    liquid_material: Handle<ChunkMaterial>,
    quads: usize,
}

type MeshOut = (Vec<Mesh>, usize, bool, Option<Mesh>, ChunkModelsOutput);

/// A meshing job: running on the pool, then uploading its parts over frames.
enum MeshJob {
    Running(Task<MeshOut>),
    Uploading {
        /// Parts still to add to `Assets<Mesh>`, popped from the back.
        parts: Vec<Mesh>,
        handles: Vec<Handle<Mesh>>,
        liquid: Option<Mesh>,
        models_out: ChunkModelsOutput,
        quads: usize,
        has_glass: bool,
    },
}

#[derive(Resource, Default)]
pub struct ChunkStream {
    generated: HashMap<ChunkPos, Arc<Chunk>>,
    bounds: WorldBounds,
    /// Entities of removed chunks, despawned by the next `stream_chunks`.
    dead_entities: Vec<Entity>,
    mesh_tasks: HashMap<ChunkPos, MeshJob>,
    meshed: HashMap<ChunkPos, Meshed>,
    /// Meshed chunks whose neighbourhood changed since they were meshed.
    stale: HashSet<ChunkPos>,
    /// Edited chunks (and border neighbours) to re-mesh at once, this frame.
    urgent: Vec<ChunkPos>,
    /// Chunks whose liquid changed, re-meshing only their liquid mesh on the main thread.
    urgent_liquid: Vec<ChunkPos>,
    /// Light tasks running on the compute pool.
    light_tasks: HashMap<ChunkPos, Task<LightVolume>>,
    /// Chunks waiting for a light task, in queue order.
    light_pending: VecDeque<ChunkPos>,
    /// Membership of `light_pending`, for dedupe.
    light_pending_set: HashSet<ChunkPos>,
    /// Chunks whose light task started before a neighbour changed, so it must
    /// run again once the current result lands.
    light_dirty: HashSet<ChunkPos>,
    /// Bumped by every insert, unload or block edit. Light results are valid
    /// for the epoch they were spawned at.
    light_epoch: u64,
    /// Epoch at which each chunk's light task was spawned.
    light_lit_at: HashMap<ChunkPos, u64>,
    /// Latest lit texture per chunk, reused by re-meshes until a new one lands.
    light_images: HashMap<ChunkPos, Handle<Image>>,
    /// Light textures of removed chunks, freed by the next `stream_chunks`.
    dead_light_images: Vec<Handle<Image>>,
    /// True while anything is pending or wanted but not yet spawned.
    busy: bool,
    /// Quads in the live meshes (kept up to date on insert/remove).
    total_quads: usize,
    /// Candidate view positions for the camera chunk in `.0`, nearest first.
    view_cache: Option<(ChunkPos, Vec<ChunkPos>)>,
    /// This frame's streaming work.
    frame: FrameStats,
}

impl ChunkStream {
    /// Received chunks, for block queries (physics).
    pub fn generated(&self) -> &HashMap<ChunkPos, Arc<Chunk>> {
        &self.generated
    }

    pub fn light_epoch(&self) -> u64 {
        self.light_epoch
    }

    pub fn set_bounds(&mut self, bounds: WorldBounds) {
        self.bounds = bounds;
    }

    /// Stores a chunk from the server, replacing any existing one, and marks it
    /// and its six neighbours for re-meshing.
    pub fn insert_chunk(&mut self, pos: ChunkPos, chunk: Arc<Chunk>) {
        if !self.bounds.chunk_inside(pos) {
            return;
        }
        self.generated.insert(pos, chunk);
        self.light_epoch += 1;
        self.light_lit_at.remove(&pos);
        for d in std::iter::once([0, 0, 0]).chain(NEIGHBOURS) {
            let n = offset(pos, d);
            // A mesh task in flight was built without the new chunk: re-mesh when it lands.
            if self.meshed.contains_key(&n) || self.mesh_tasks.contains_key(&n) {
                self.stale.insert(n);
            }
        }
        // The chunk (and its neighbours' padded volumes) changed. Drop any
        // stale texture so a re-mesh cannot show the previous chunk's light.
        if let Some(h) = self.light_images.remove(&pos) {
            self.dead_light_images.push(h);
        }
        self.queue_light_around(pos);
        // The new chunk can shadow the sky for every chunk under it. Sky only
        // comes from above, so never relight chunks higher in this column.
        for p in relight_below(pos) {
            self.queue_light(p);
        }
    }

    /// Edits one block (copy-on-write) and marks the chunk, plus any face
    /// neighbour the block touches, for re-meshing.
    pub fn set_block(&mut self, p: [i32; 3], b: terravibe_sim::BlockId) {
        let pos = ChunkPos::of_block(p[0], p[1], p[2]);
        let Some(chunk) = self.generated.get_mut(&pos) else {
            return;
        };
        let l = |v: i32| v.rem_euclid(CHUNK_SIZE) as usize;
        let (lx, ly, lz) = (l(p[0]), l(p[1]), l(p[2]));
        let old = chunk.get(lx, ly, lz);
        Arc::make_mut(chunk).set(lx, ly, lz, b);
        let solidity_changed = old.is_solid() != b.is_solid();
        let mut touched = vec![[0, 0, 0]];
        for (axis, v) in p.iter().enumerate() {
            let lv = v.rem_euclid(CHUNK_SIZE);
            for (edge, step) in [(0, -1), (CHUNK_SIZE - 1, 1)] {
                if lv == edge {
                    let mut d = [0; 3];
                    d[axis] = step;
                    touched.push(d);
                }
            }
        }
        for d in touched {
            let n = offset(pos, d);
            if self.meshed.contains_key(&n) || self.mesh_tasks.contains_key(&n) {
                self.stale.insert(n);
                if !self.urgent.contains(&n) {
                    self.urgent.push(n);
                }
            }
        }
        // Light crosses chunk borders: relight every chunk whose padded
        // volume contains the edited block.
        self.light_epoch += 1;
        for n in relight_targets(p) {
            self.queue_light(n);
        }
        if solidity_changed {
            // Adding/removing a solid roof changes which lower chunks see the
            // sky, across the 3×3 columns a border edit can affect.
            for n in relight_below_edit(p) {
                self.queue_light(n);
            }
        }
    }

    /// Edits one wall bit. Wall panels live inside the walled cell, so only its
    /// own chunk re-meshes. Walls never affect light (see `art::chunk_mesh`).
    pub fn set_wall(&mut self, p: [i32; 3], on: bool) {
        let pos = ChunkPos::of_block(p[0], p[1], p[2]);
        let Some(chunk) = self.generated.get_mut(&pos) else {
            return;
        };
        let l = |v: i32| v.rem_euclid(CHUNK_SIZE) as usize;
        Arc::make_mut(chunk).set_wall(l(p[0]), l(p[1]), l(p[2]), on);
        if self.meshed.contains_key(&pos) || self.mesh_tasks.contains_key(&pos) {
            self.stale.insert(pos);
            if !self.urgent.contains(&pos) {
                self.urgent.push(pos);
            }
        }
    }

    /// Applies a liquid delta from the server to one chunk and marks it
    /// (plus any neighbours touched on chunk borders) for liquid-only re-meshing
    /// and, when liquid presence or kind changed, relighting.
    pub fn apply_liquid_delta(&mut self, pos: ChunkPos, cells: &[(u16, u16)]) {
        let Some(chunk) = self.generated.get_mut(&pos) else {
            return;
        };
        let ch = Arc::make_mut(chunk);
        let mut touched = vec![[0, 0, 0]];
        let mut light_changed = false;
        for &(idx, raw) in cells {
            let idx = idx as usize;
            let lx = idx % (CHUNK_SIZE as usize);
            let ly = (idx / (CHUNK_SIZE as usize)) % (CHUNK_SIZE as usize);
            let lz = idx / ((CHUNK_SIZE * CHUNK_SIZE) as usize);
            let old_kind = ch.liquid(lx, ly, lz).map(|l| l.kind);
            let new_liq = Liquid::unpack(raw);
            if old_kind != new_liq.map(|l| l.kind) {
                light_changed = true;
            }
            ch.set_liquid(lx, ly, lz, new_liq);
            if lx == 0 {
                touched.push([-1, 0, 0]);
            } else if lx == (CHUNK_SIZE as usize) - 1 {
                touched.push([1, 0, 0]);
            }
            if ly == 0 {
                touched.push([0, -1, 0]);
            } else if ly == (CHUNK_SIZE as usize) - 1 {
                touched.push([0, 1, 0]);
            }
            if lz == 0 {
                touched.push([0, 0, -1]);
            } else if lz == (CHUNK_SIZE as usize) - 1 {
                touched.push([0, 0, 1]);
            }
        }
        for d in touched {
            let n = offset(pos, d);
            if self.meshed.contains_key(&n) || self.mesh_tasks.contains_key(&n) {
                self.stale.insert(n);
                if !self.urgent_liquid.contains(&n) {
                    self.urgent_liquid.push(n);
                }
            }
        }
        if light_changed {
            self.light_epoch += 1;
            self.queue_light_around(pos);
        }
    }

    #[allow(dead_code)]
    pub fn set_liquid(&mut self, x: i32, y: i32, z: i32, l: Option<Liquid>) {
        let pos = ChunkPos::of_block(x, y, z);
        let lx = x.rem_euclid(CHUNK_SIZE) as usize;
        let ly = y.rem_euclid(CHUNK_SIZE) as usize;
        let lz = z.rem_euclid(CHUNK_SIZE) as usize;
        let idx =
            (lx + ly * (CHUNK_SIZE as usize) + lz * (CHUNK_SIZE * CHUNK_SIZE) as usize) as u16;
        let packed = Liquid::pack(l);
        self.apply_liquid_delta(pos, &[(idx, packed)]);
    }

    /// Drops a chunk the server unloaded and despawns its meshes.
    pub fn remove_chunk(&mut self, pos: ChunkPos) {
        self.generated.remove(&pos);
        self.mesh_tasks.remove(&pos); // dropping a Task cancels it
        self.light_tasks.remove(&pos); // dropping a Task cancels it
        self.stale.remove(&pos);
        self.light_epoch += 1;
        self.light_lit_at.remove(&pos);
        self.light_pending_set.remove(&pos);
        self.light_pending.retain(|p| *p != pos);
        self.light_dirty.remove(&pos);
        if let Some(h) = self.light_images.remove(&pos) {
            self.dead_light_images.push(h);
        }
        if let Some(m) = self.remove_meshed(pos) {
            self.frame.despawned += m.entities.len() as u32 + u32::from(m.liquid_entity.is_some());
            self.dead_entities.extend(m.entities);
            if let Some(le) = m.liquid_entity {
                self.dead_entities.push(le);
            }
        }
        // Neighbours' padded volumes no longer see this chunk.
        self.queue_light_around(pos);
        // A missing chunk counts as open sky, so the chunks below may become
        // sky-lit. Sky only comes from above.
        for p in relight_below(pos) {
            self.queue_light(p);
        }
    }

    /// Queues `p` for relighting unless it is already queued, running, or was
    /// lit after the latest change. A running task from an older epoch is
    /// marked dirty so it re-runs with the new neighbourhood.
    fn queue_light(&mut self, p: ChunkPos) {
        if !self.generated.contains_key(&p) {
            return;
        }
        if self
            .light_lit_at
            .get(&p)
            .is_some_and(|epoch| *epoch >= self.light_epoch)
        {
            return;
        }
        if self.light_tasks.contains_key(&p) {
            self.light_dirty.insert(p);
            return;
        }
        if self.light_pending_set.insert(p) {
            self.light_pending.push_back(p);
        }
    }

    /// Queues the 3×3×3 chunks around `p`, the ones whose padded region can
    /// overlap it.
    fn queue_light_around(&mut self, p: ChunkPos) {
        for dy in -1..=1 {
            for dz in -1..=1 {
                for dx in -1..=1 {
                    self.queue_light(offset(p, [dx, dy, dz]));
                }
            }
        }
    }

    /// Gives a chunk that just got a mesh its first light volume, unless one
    /// is already running or a texture is already available.
    fn ensure_light(&mut self, p: ChunkPos) {
        if self.light_images.contains_key(&p) || self.light_tasks.contains_key(&p) {
            return;
        }
        self.queue_light(p);
    }

    pub fn is_busy(&self) -> bool {
        self.busy
    }

    pub fn mesh_count(&self) -> usize {
        self.meshed
            .values()
            .filter(|m| !m.entities.is_empty() || m.liquid_entity.is_some())
            .count()
    }

    /// Total quads of the live meshes.
    pub fn quads(&self) -> usize {
        self.total_quads
    }

    /// Nothing pending and every chunk in view of `cam` is meshed.
    pub fn fully_meshed(&self, cam: Vec3) -> bool {
        !self.busy
            && self.mesh_tasks.is_empty()
            && chunks_in_view(cam)
                .iter()
                .filter(|&&p| self.bounds.chunk_inside(p))
                .all(|p| self.meshed.contains_key(p) && !self.stale.contains(p))
    }

    fn count(&mut self, m: &Meshed, sign: isize) {
        self.total_quads = self
            .total_quads
            .wrapping_add_signed(sign * m.quads as isize);
    }

    fn insert_meshed(&mut self, p: ChunkPos, m: Meshed) {
        self.count(&m, 1);
        if let Some(old) = self.meshed.insert(p, m) {
            self.count(&old, -1);
        }
    }

    fn remove_meshed(&mut self, p: ChunkPos) -> Option<Meshed> {
        let old = self.meshed.remove(&p)?;
        self.count(&old, -1);
        Some(old)
    }
}

/// A per-chunk material instance: the shared base plus this chunk's light
/// texture and its world corner.
pub(crate) fn chunk_material(
    materials: &mut Assets<ChunkMaterial>,
    mat: &ChunkMat,
    pos: ChunkPos,
    light: Handle<Image>,
) -> Handle<ChunkMaterial> {
    materials.add(ChunkMaterial {
        base: mat.base.clone(),
        extension: LightExt {
            light,
            // The texture has one border texel on every side, so its minimum
            // corner is one block outside the chunk.
            origin: Vec4::new(
                (pos.x * CHUNK_SIZE - LIGHT_BORDER as i32) as f32,
                (pos.y * CHUNK_SIZE - LIGHT_BORDER as i32) as f32,
                (pos.z * CHUNK_SIZE - LIGHT_BORDER as i32) as f32,
                1.0, // sky brightness; update_sky corrects it
            ),
            tiles: mat.tiles.clone(),
            liquid_params: Vec4::ZERO,
        },
    })
}

fn terrain_chunk_material(
    materials: &mut Assets<ChunkMaterial>,
    mat: &ChunkMat,
    pos: ChunkPos,
    light: Handle<Image>,
    has_glass: bool,
) -> Handle<ChunkMaterial> {
    materials.add(ChunkMaterial {
        base: if has_glass {
            mat.terrain_masked_base.clone()
        } else {
            mat.terrain_opaque_base.clone()
        },
        extension: LightExt {
            light,
            origin: Vec4::new(
                (pos.x * CHUNK_SIZE - LIGHT_BORDER as i32) as f32,
                (pos.y * CHUNK_SIZE - LIGHT_BORDER as i32) as f32,
                (pos.z * CHUNK_SIZE - LIGHT_BORDER as i32) as f32,
                1.0,
            ),
            tiles: mat.tiles.clone(),
            liquid_params: Vec4::ZERO,
        },
    })
}

pub(crate) fn liquid_chunk_material(
    materials: &mut Assets<ChunkMaterial>,
    mat: &ChunkMat,
    pos: ChunkPos,
    light: Handle<Image>,
) -> Handle<ChunkMaterial> {
    materials.add(ChunkMaterial {
        base: mat.liquid_base.clone(),
        extension: LightExt {
            light,
            origin: Vec4::new(
                (pos.x * CHUNK_SIZE - LIGHT_BORDER as i32) as f32,
                (pos.y * CHUNK_SIZE - LIGHT_BORDER as i32) as f32,
                (pos.z * CHUNK_SIZE - LIGHT_BORDER as i32) as f32,
                1.0,
            ),
            tiles: mat.tiles.clone(),
            liquid_params: Vec4::new(1.0, 0.0, 0.0, 0.0),
        },
    })
}

fn to_tiles_array_image(art: &BlockArt) -> Image {
    let data: Vec<u8> = art
        .tiles
        .iter()
        .flat_map(|t| t.iter().flatten().copied())
        .collect();
    let mut out = Image::new(
        Extent3d {
            width: TILE,
            height: TILE,
            depth_or_array_layers: art.tiles.len().max(1) as u32,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    out.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Nearest,
        min_filter: ImageFilterMode::Nearest,
        mipmap_filter: ImageFilterMode::Nearest,
        ..default()
    });
    out.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..default()
    });
    out
}

fn setup_material(mut commands: Commands, art: Res<ArtRes>, mut images: ResMut<Assets<Image>>) {
    let image = images.add(to_bevy_image(&art.0.atlas));
    let tiles = images.add(to_tiles_array_image(&art.0));
    commands.insert_resource(ChunkMat {
        base: StandardMaterial {
            base_color_texture: Some(image),
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            // Glass is see-through between its frame pixels.
            alpha_mode: AlphaMode::Mask(0.5),
            ..default()
        },
        terrain_opaque_base: StandardMaterial {
            base_color_texture: None,
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            alpha_mode: AlphaMode::Opaque,
            ..default()
        },
        terrain_masked_base: StandardMaterial {
            base_color_texture: None,
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            alpha_mode: AlphaMode::Mask(0.5),
            ..default()
        },
        liquid_base: StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: None,
            perceptual_roughness: 0.15,
            reflectance: 0.35,
            alpha_mode: AlphaMode::Blend,
            ..default()
        },
        tiles,
        default_light: images.add(crate::light::default_light_image()),
    });
}

fn centre(p: ChunkPos) -> Vec3 {
    let h = CHUNK_SIZE as f32 / 2.0;
    Vec3::new(
        (p.x * CHUNK_SIZE) as f32 + h,
        (p.y * CHUNK_SIZE) as f32 + h,
        (p.z * CHUNK_SIZE) as f32 + h,
    )
}

fn horizontal_dist(p: ChunkPos, cam: Vec3) -> f32 {
    let c = centre(p);
    Vec2::new(c.x - cam.x, c.z - cam.z).length()
}

fn in_view(p: ChunkPos, cam: Vec3) -> bool {
    let cy = (cam.y / CHUNK_SIZE as f32).floor() as i32;
    (0..CHUNK_LAYERS).contains(&p.y)
        && (p.y - cy).abs() <= terravibe_sim::net::VERTICAL_CHUNK_RADIUS
        && horizontal_dist(p, cam) <= VIEW_RADIUS_BLOCKS
}

fn offset(p: ChunkPos, d: [i32; 3]) -> ChunkPos {
    ChunkPos {
        x: p.x + d[0],
        y: p.y + d[1],
        z: p.z + d[2],
    }
}

/// All chunk positions in view of `cam`, nearest first.
fn chunks_in_view(cam: Vec3) -> Vec<ChunkPos> {
    let r = (VIEW_RADIUS_BLOCKS / CHUNK_SIZE as f32).ceil() as i32 + 1;
    let c = ChunkPos::of_block(cam.x.floor() as i32, 0, cam.z.floor() as i32);
    let mut out = Vec::new();
    for x in c.x - r..=c.x + r {
        for z in c.z - r..=c.z + r {
            for y in terravibe_sim::net::vertical_layers_for_y(cam.y) {
                let p = ChunkPos { x, y, z };
                if in_view(p, cam) {
                    out.push(p);
                }
            }
        }
    }
    out.sort_by_cached_key(|p| ordered(centre(*p).distance(cam)));
    out
}

/// Non-negative `f32` as a sortable integer key.
fn ordered(d: f32) -> u32 {
    d.to_bits()
}

/// Superset of `chunks_in_view(cam)` for any camera inside chunk `c`, ordered by
/// distance to the centre of `c`. Rebuilt only when the camera changes chunk.
fn view_candidates(c: ChunkPos) -> Vec<ChunkPos> {
    let cc = centre(c);
    // Half the horizontal diagonal of a chunk: the furthest the camera can be from `cc`.
    let slack = CHUNK_SIZE as f32 * std::f32::consts::FRAC_1_SQRT_2;
    let r = ((VIEW_RADIUS_BLOCKS + slack) / CHUNK_SIZE as f32).ceil() as i32 + 1;
    let y_lo = (c.y - terravibe_sim::net::VERTICAL_CHUNK_RADIUS - 1).max(0);
    let y_hi = (c.y + terravibe_sim::net::VERTICAL_CHUNK_RADIUS + 1).min(CHUNK_LAYERS - 1);
    let mut out = Vec::new();
    for x in c.x - r..=c.x + r {
        for z in c.z - r..=c.z + r {
            for y in y_lo..=y_hi {
                let p = ChunkPos { x, y, z };
                if horizontal_dist(p, cc) <= VIEW_RADIUS_BLOCKS + slack {
                    out.push(p);
                }
            }
        }
    }
    out.sort_by_cached_key(|p| ordered(centre(*p).distance(cc)));
    out
}

/// Returns whether another mesh part of `next_verts` fits in this frame's
/// upload budget. Always allows at least one part so large chunks never stall.
fn can_upload_part(
    part_budget: usize,
    vert_budget: usize,
    uploaded_any: bool,
    next_verts: usize,
) -> bool {
    !uploaded_any || (part_budget > 0 && vert_budget >= next_verts)
}

pub(crate) fn stream_chunks(
    mut commands: Commands,
    mut stream: ResMut<ChunkStream>,
    art: Res<ArtRes>,
    material: Option<Res<ChunkMat>>,
    models: Option<Res<BlockModels>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut chunk_materials: ResMut<Assets<ChunkMaterial>>,
    mut images: ResMut<Assets<Image>>,
    camera: Single<&Transform, With<FlyCamera>>,
) {
    let (Some(material), Some(models)) = (material, models) else {
        return;
    };
    let cam = camera.translation;
    let started = std::time::Instant::now();
    let s = &mut *stream;
    s.frame = FrameStats::default();

    // 1. Despawn the meshes of chunks the server unloaded, and free the light
    // textures that only they used.
    for e in s.dead_entities.drain(..) {
        commands.entity(e).despawn();
    }
    for h in s.dead_light_images.drain(..) {
        images.remove(h.id());
    }

    // 1b. Block edits: re-mesh now, on this thread, bypassing the queue, the
    // spawn budget and the upload budget. The old mesh is swapped out in the
    // same frame, so there is neither a hole nor a delay.
    for p in std::mem::take(&mut s.urgent) {
        let Some(center) = s.generated.get(&p) else {
            continue;
        };
        if !s.stale.contains(&p) {
            continue;
        }
        s.mesh_tasks.remove(&p); // an in-flight task lacks the edit
        s.stale.remove(&p);
        let neighbors = NEIGHBOURS.map(|d| s.generated.get(&offset(p, d)).map(|c| c.as_ref()));
        let input = ChunkMeshInput::from_chunks(p, center, neighbors);
        let has_glass = input.has_glass;
        let data = mesh_chunk_greedy(&input, &art.0);
        let quads = data.quad_count();
        let handles: Vec<Handle<Mesh>> = data
            .to_bevy_mesh_parts(PART_QUADS)
            .into_iter()
            .map(|m| meshes.add(m))
            .collect();
        let liquid_mesh = if center.has_liquid() {
            mesh_liquid(center, p, |wx, wy, wz| {
                let cp = ChunkPos::of_block(wx, wy, wz);
                s.generated.get(&cp).and_then(|c| {
                    c.liquid(
                        wx.rem_euclid(CHUNK_SIZE) as usize,
                        wy.rem_euclid(CHUNK_SIZE) as usize,
                        wz.rem_euclid(CHUNK_SIZE) as usize,
                    )
                })
            })
        } else {
            None
        };
        let liquid_handle = liquid_mesh.map(|m| meshes.add(m));
        if let Some(old) = s.remove_meshed(p) {
            for e in old.entities {
                commands.entity(e).despawn();
            }
            if let Some(le) = old.liquid_entity {
                commands.entity(le).despawn();
            }
        }
        let light = s
            .light_images
            .get(&p)
            .cloned()
            .unwrap_or_else(|| material.default_light.clone());
        let cm =
            terrain_chunk_material(&mut chunk_materials, &material, p, light.clone(), has_glass);
        let liquid_cm = liquid_chunk_material(&mut chunk_materials, &material, p, light);
        let mut entities: Vec<Entity> = handles
            .into_iter()
            .map(|h| {
                commands
                    .spawn((
                        Mesh3d(h),
                        MeshMaterial3d(cm.clone()),
                        Transform::IDENTITY,
                        DespawnOnExit(AppState::InGame),
                    ))
                    .id()
            })
            .collect();
        let liquid_entity = liquid_handle.map(|lh| {
            commands
                .spawn((
                    Mesh3d(lh),
                    MeshMaterial3d(liquid_cm.clone()),
                    Transform::IDENTITY,
                    LiquidMeshEntity,
                    DespawnOnExit(AppState::InGame),
                ))
                .id()
        });
        if let Some(chunk) = s.generated.get(&p) {
            spawn_chunk_models(
                &mut commands,
                &models,
                &cm,
                p,
                chunk,
                &s.generated,
                &mut meshes,
                &mut entities,
                AppState::InGame,
            );
        }
        s.insert_meshed(
            p,
            Meshed {
                entities,
                liquid_entity,
                material: cm,
                liquid_material: liquid_cm,
                quads,
            },
        );
        s.ensure_light(p);
    }

    // 1c. Liquid-only edits: re-mesh only `liquid_entity` for already-meshed chunks.
    for p in std::mem::take(&mut s.urgent_liquid) {
        let Some(center) = s.generated.get(&p) else {
            continue;
        };
        if !s.stale.contains(&p) || s.mesh_tasks.contains_key(&p) {
            continue;
        }
        if !s.meshed.contains_key(&p) {
            continue;
        }
        s.stale.remove(&p);
        let liquid_mesh = if center.has_liquid() {
            mesh_liquid(center, p, |wx, wy, wz| {
                let cp = ChunkPos::of_block(wx, wy, wz);
                s.generated.get(&cp).and_then(|c| {
                    c.liquid(
                        wx.rem_euclid(CHUNK_SIZE) as usize,
                        wy.rem_euclid(CHUNK_SIZE) as usize,
                        wz.rem_euclid(CHUNK_SIZE) as usize,
                    )
                })
            })
        } else {
            None
        };
        let liquid_handle = liquid_mesh.map(|m| meshes.add(m));
        if let Some(m) = s.meshed.get_mut(&p) {
            if let Some(old_le) = m.liquid_entity.take() {
                commands.entity(old_le).despawn();
            }
            m.liquid_entity = liquid_handle.map(|lh| {
                commands
                    .spawn((
                        Mesh3d(lh),
                        MeshMaterial3d(m.liquid_material.clone()),
                        Transform::IDENTITY,
                        LiquidMeshEntity,
                        DespawnOnExit(AppState::InGame),
                    ))
                    .id()
            });
        }
    }

    // 2. Collect finished meshing tasks, then upload a budget of their parts.
    let mut uploading: Vec<ChunkPos> = Vec::new();
    for (p, job) in s.mesh_tasks.iter_mut() {
        if let MeshJob::Running(t) = job
            && let Some((mut parts, quads, has_glass, liquid, models_out)) = check_ready(t)
        {
            s.frame.mesh_done += 1;
            parts.reverse();
            *job = MeshJob::Uploading {
                handles: Vec::with_capacity(parts.len()),
                parts,
                liquid,
                models_out,
                quads,
                has_glass,
            };
        }
        if matches!(job, MeshJob::Uploading { .. }) {
            uploading.push(*p);
        }
    }
    uploading.sort_by_cached_key(|p| ordered(centre(*p).distance(cam)));
    let mut part_budget = UPLOAD_PARTS_PER_FRAME;
    let mut vert_budget = UPLOAD_VERTS_PER_FRAME;
    let mut uploaded_any = false;
    for p in uploading {
        let Some(MeshJob::Uploading { parts, handles, .. }) = s.mesh_tasks.get_mut(&p) else {
            continue;
        };
        while let Some(verts) = parts.last().map(|m| m.count_vertices()) {
            if !can_upload_part(part_budget, vert_budget, uploaded_any, verts) {
                break;
            }
            let Some(m) = parts.pop() else {
                break;
            };
            uploaded_any = true;
            part_budget = part_budget.saturating_sub(1);
            vert_budget = vert_budget.saturating_sub(verts);
            s.frame.inserted += 1;
            s.frame.verts += verts as u32;
            handles.push(meshes.add(m));
        }
        if !parts.is_empty() {
            break;
        }
        // Cap chunk finalizations/swaps (including liquid & batched model uploads) to 1 per frame.
        if uploaded_any
            && let Some(MeshJob::Uploading {
                liquid, models_out, ..
            }) = s.mesh_tasks.get(&p)
        {
            let extra_verts = liquid.as_ref().map_or(0, |m| m.count_vertices())
                + models_out
                    .batched_mesh
                    .as_ref()
                    .map_or(0, |m| m.count_vertices());
            if extra_verts > vert_budget {
                break;
            }
        }
        // All parts uploaded: swap the chunk's entities in one go.
        let (quads, has_glass, handles, liquid, models_out) = match s.mesh_tasks.remove(&p) {
            Some(MeshJob::Uploading {
                quads,
                has_glass,
                handles,
                liquid,
                models_out,
                ..
            }) => (quads, has_glass, handles, liquid, models_out),
            _ => continue,
        };
        let liquid_handle = liquid.map(|m| meshes.add(m));
        if let Some(old) = s.remove_meshed(p) {
            s.frame.despawned += old.entities.len() as u32 + u32::from(old.liquid_entity.is_some());
            for e in old.entities {
                commands.entity(e).despawn();
            }
            if let Some(le) = old.liquid_entity {
                commands.entity(le).despawn();
            }
        }
        let light = s
            .light_images
            .get(&p)
            .cloned()
            .unwrap_or_else(|| material.default_light.clone());
        let cm =
            terrain_chunk_material(&mut chunk_materials, &material, p, light.clone(), has_glass);
        let liquid_cm = liquid_chunk_material(&mut chunk_materials, &material, p, light);
        let mut entities: Vec<Entity> = handles
            .into_iter()
            .map(|h| {
                commands
                    .spawn((
                        Mesh3d(h),
                        MeshMaterial3d(cm.clone()),
                        Transform::IDENTITY,
                        DespawnOnExit(AppState::InGame),
                    ))
                    .id()
            })
            .collect();
        let liquid_entity = liquid_handle.map(|lh| {
            commands
                .spawn((
                    Mesh3d(lh),
                    MeshMaterial3d(liquid_cm.clone()),
                    Transform::IDENTITY,
                    LiquidMeshEntity,
                    DespawnOnExit(AppState::InGame),
                ))
                .id()
        });
        spawn_chunk_models_output(
            &mut commands,
            &models,
            &cm,
            models_out,
            &mut meshes,
            &mut entities,
            AppState::InGame,
        );
        s.insert_meshed(
            p,
            Meshed {
                entities,
                liquid_entity,
                material: cm,
                liquid_material: liquid_cm,
                quads,
            },
        );
        s.ensure_light(p);
        break;
    }

    // 2b. Finished light tasks: upload the quantised 34³ (32³ plus a one-block
    // border) texture and swap the material's light handle in place. The
    // chunk's mesh is not touched.
    let pool = AsyncComputeTaskPool::get();
    let mut lit: Vec<(ChunkPos, LightVolume)> = Vec::new();
    for (p, task) in s.light_tasks.iter_mut() {
        if lit.len() >= LIGHT_UPLOADS_PER_FRAME {
            break;
        }
        if let Some(v) = check_ready(task) {
            lit.push((*p, v));
        }
    }
    for (p, v) in lit {
        s.light_tasks.remove(&p);
        s.frame.light_done += 1;
        if !s.generated.contains_key(&p) {
            continue;
        }
        let image = images.add(crate::light::light_image(&v));
        if let Some(old) = s.light_images.insert(p, image.clone()) {
            s.dead_light_images.push(old);
        }
        if let Some(m) = s.meshed.get(&p) {
            if let Some(mut mat) = chunk_materials.get_mut(&m.material) {
                mat.extension.light = image.clone();
            }
            if let Some(mut mat) = chunk_materials.get_mut(&m.liquid_material) {
                mat.extension.light = image;
            }
        }
        // A neighbour arrived while this volume was being computed.
        if s.light_dirty.remove(&p) {
            s.queue_light(p);
        }
    }

    // 2c. Spawn light work, nearest queued first, within the per-frame budget.
    // The snapshot is the 3×3×3 that the padded box overlaps plus every chunk
    // above it, so `sky_open_above` reads the real world column.
    let mut light_budget = LIGHT_SPAWN_BUDGET;
    while light_budget > 0 {
        let Some(p) = s.light_pending.pop_front() else {
            break;
        };
        if !s.light_pending_set.remove(&p) {
            continue; // cancelled unload or already spawned
        }
        if s.light_tasks.contains_key(&p) || !s.generated.contains_key(&p) {
            continue;
        }
        light_budget -= 1;
        s.frame.light_spawned += 1;
        let chunk_refs = LightInput::collect_chunk_refs(p, &s.generated);
        let task = pool.spawn(async move {
            let input = LightInput::from_chunk_map(p, &chunk_refs);
            light_chunk_input(&input)
        });
        s.light_lit_at.insert(p, s.light_epoch);
        s.light_dirty.remove(&p);
        s.light_tasks.insert(p, task);
    }

    // 3. Spawn meshing work, nearest first, within budget.
    let mut mesh_budget = SPAWN_BUDGET;
    let mut wanted_more = false;
    let cam_chunk = ChunkPos::of_block(
        cam.x.floor() as i32,
        cam.y.floor() as i32,
        cam.z.floor() as i32,
    );
    let view = match s.view_cache.take() {
        Some((c, v)) if c == cam_chunk => v,
        _ => {
            s.frame.view_rebuilt = true;
            view_candidates(cam_chunk)
        }
    };
    let bounds = s.bounds;
    for &p in view
        .iter()
        .filter(|&&p| bounds.chunk_inside(p) && in_view(p, cam))
    {
        let Some(center) = s.generated.get(&p) else {
            wanted_more = true; // still on its way from the server
            continue;
        };
        let up_to_date = s.meshed.contains_key(&p) && !s.stale.contains(&p);
        if up_to_date || s.mesh_tasks.contains_key(&p) {
            continue;
        }
        if mesh_budget == 0 {
            wanted_more = true;
            continue;
        }
        mesh_budget -= 1;
        s.frame.mesh_spawned += 1;
        s.stale.remove(&p);
        let center_arc = Arc::clone(center);
        let neighbors_arc = NEIGHBOURS.map(|d| s.generated.get(&offset(p, d)).cloned());
        let model_chunks = collect_model_chunks(p, &s.generated);
        let model_meshes = models.meshes_data();
        let art = art.0.clone();
        let task = pool.spawn(async move {
            let neighbor_refs = neighbors_arc.each_ref().map(|o| o.as_deref());
            let input = ChunkMeshInput::from_chunks(p, &center_arc, neighbor_refs);
            let has_glass = input.has_glass;
            let data = mesh_chunk_greedy(&input, &art);
            let quads = data.quad_count();
            let block_parts = data.to_bevy_mesh_parts(PART_QUADS);
            let liquid_mesh = if center_arc.has_liquid() {
                mesh_liquid(&center_arc, p, |wx, wy, wz| {
                    let cp = ChunkPos::of_block(wx, wy, wz);
                    let lx = wx.rem_euclid(CHUNK_SIZE) as usize;
                    let ly = wy.rem_euclid(CHUNK_SIZE) as usize;
                    let lz = wz.rem_euclid(CHUNK_SIZE) as usize;
                    if cp == p {
                        center_arc.liquid(lx, ly, lz)
                    } else {
                        NEIGHBOURS
                            .iter()
                            .position(|&d| offset(p, d) == cp)
                            .and_then(|idx| neighbors_arc[idx].as_ref())
                            .and_then(|nc| nc.liquid(lx, ly, lz))
                    }
                })
            } else {
                None
            };
            let models_out = build_chunk_models(&model_meshes, p, &center_arc, &model_chunks);
            (block_parts, quads, has_glass, liquid_mesh, models_out)
        });
        s.mesh_tasks.insert(p, MeshJob::Running(task));
    }
    s.view_cache = Some((cam_chunk, view));
    s.busy = wanted_more
        || !s.mesh_tasks.is_empty()
        || !s.light_pending.is_empty()
        || !s.light_tasks.is_empty();
    s.frame.stream_us = started.elapsed().as_micros() as u32;
}

fn animate_liquid_materials(
    time: Res<Time<Real>>,
    stream: Res<ChunkStream>,
    mut chunk_materials: ResMut<Assets<ChunkMaterial>>,
) {
    let t = time.elapsed_secs();
    for m in stream.meshed.values() {
        if m.liquid_entity.is_some()
            && let Some(mut mat) = chunk_materials.get_mut(&m.liquid_material)
        {
            mat.extension.liquid_params.y = t;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn border_edit_marks_neighbour() {
        let mut s = ChunkStream::default();
        let p = ChunkPos { x: 0, y: 0, z: 0 };
        let nx = ChunkPos { x: 1, y: 0, z: 0 };
        s.insert_chunk(p, Arc::new(Chunk::filled(terravibe_sim::DIRT)));
        for q in [p, nx] {
            s.meshed.insert(
                q,
                Meshed {
                    entities: vec![],
                    liquid_entity: None,
                    material: Handle::default(),
                    liquid_material: Handle::default(),
                    quads: 0,
                },
            );
        }
        s.set_block([15, 5, 5], terravibe_sim::AIR);
        assert!(s.stale.contains(&p) && !s.stale.contains(&nx));
        assert_eq!(s.generated()[&p].get(15, 5, 5), terravibe_sim::AIR);
        assert_eq!(s.urgent, vec![p]);
        s.stale.clear();
        s.set_block([31, 5, 5], terravibe_sim::AIR);
        assert!(s.stale.contains(&p) && s.stale.contains(&nx));
    }

    #[test]
    fn apply_liquid_delta_updates_chunk_and_marks_stale() {
        let mut s = ChunkStream::default();
        let p = ChunkPos { x: 0, y: 0, z: 0 };
        let nx = ChunkPos { x: 1, y: 0, z: 0 };
        s.insert_chunk(p, Arc::new(Chunk::filled(terravibe_sim::AIR)));
        for q in [p, nx] {
            s.meshed.insert(
                q,
                Meshed {
                    entities: vec![],
                    liquid_entity: None,
                    material: Handle::default(),
                    liquid_material: Handle::default(),
                    quads: 0,
                },
            );
        }
        let packed_water = Liquid::pack(Some(Liquid::water(200)));
        let idx_mid = (10 + 5 * 32 + 5 * 1024) as u16;
        s.apply_liquid_delta(p, &[(idx_mid, packed_water)]);
        assert!(s.stale.contains(&p) && !s.stale.contains(&nx));
        assert_eq!(s.generated()[&p].liquid(10, 5, 5), Some(Liquid::water(200)));

        s.stale.clear();
        let idx_border = (31 + 5 * 32 + 5 * 1024) as u16;
        s.apply_liquid_delta(p, &[(idx_border, packed_water)]);
        assert!(s.stale.contains(&p) && s.stale.contains(&nx));
        assert_eq!(s.generated()[&p].liquid(31, 5, 5), Some(Liquid::water(200)));
    }

    #[test]
    fn light_queue_dedupes_and_tracks_epochs() {
        let mut s = ChunkStream::default();
        let p = ChunkPos { x: 0, y: 0, z: 0 };
        s.insert_chunk(p, Arc::new(Chunk::filled(terravibe_sim::DIRT)));
        assert_eq!(s.light_pending.len(), 1);
        assert!(s.light_pending_set.contains(&p));
        s.queue_light(p);
        assert_eq!(s.light_pending.len(), 1);

        // A task spawned at the current epoch already saw every change.
        s.light_pending.clear();
        s.light_pending_set.clear();
        s.light_lit_at.insert(p, s.light_epoch);
        s.queue_light(p);
        assert!(s.light_pending.is_empty());

        // An edit bumps the epoch and queues the chunk again.
        s.set_block([5, 5, 5], terravibe_sim::AIR);
        assert!(s.light_pending_set.contains(&p));
    }

    #[test]
    fn upper_chunk_insert_and_remove_queue_lower_sky() {
        let mut s = ChunkStream::default();
        let col = |y| ChunkPos { x: 0, y, z: 0 };
        for y in 0..=2 {
            s.insert_chunk(col(y), Arc::new(Chunk::filled(terravibe_sim::AIR)));
        }
        let clear = |s: &mut ChunkStream| {
            s.light_pending.clear();
            s.light_pending_set.clear();
            s.light_lit_at.clear();
        };

        // Replacing the chunk at y = 2 changes sky for y = 1 and y = 0.
        clear(&mut s);
        s.insert_chunk(col(2), Arc::new(Chunk::filled(terravibe_sim::AIR)));
        assert!(s.light_pending_set.contains(&col(1)));
        assert!(s.light_pending_set.contains(&col(0)));

        // Removing it opens the sky for both lower chunks too.
        clear(&mut s);
        s.remove_chunk(col(2));
        assert!(s.light_pending_set.contains(&col(1)));
        assert!(s.light_pending_set.contains(&col(0)));
        assert!(!s.light_pending_set.contains(&col(3)));
    }

    #[test]
    fn solid_edit_queues_lower_sky_columns() {
        let mut s = ChunkStream::default();
        for x in -1..=1 {
            for y in 0..=2 {
                for z in -1..=1 {
                    let p = ChunkPos { x, y, z };
                    s.insert_chunk(p, Arc::new(Chunk::filled(terravibe_sim::STONE)));
                }
            }
        }
        s.light_pending.clear();
        s.light_pending_set.clear();
        s.light_lit_at.clear();

        // Local y = 16 in chunk layer 2 (world y = 80): a solid -> air edit
        // opens the sky for the 3×3 columns in layers 0 and 1.
        s.set_block([16, 80, 16], terravibe_sim::AIR);
        for y in 0..=1 {
            for x in -1..=1 {
                for z in -1..=1 {
                    assert!(
                        s.light_pending_set.contains(&ChunkPos { x, y, z }),
                        "missing lower chunk {x},{y},{z}"
                    );
                }
            }
        }
    }

    #[test]
    fn duplicate_chunk_replaces() {
        let mut s = ChunkStream::default();
        let p = ChunkPos { x: 1, y: 0, z: 2 };
        let a = Arc::new(Chunk::filled(terravibe_sim::DIRT));
        let b = Arc::new(Chunk::filled(terravibe_sim::AIR));
        s.insert_chunk(p, a);
        s.insert_chunk(p, b.clone());
        assert_eq!(s.generated().len(), 1);
        assert!(Arc::ptr_eq(&s.generated()[&p], &b));
    }

    #[test]
    fn view_covers_radius_and_only_block_layers() {
        let cam = Vec3::new(0.0, 100.0, 0.0);
        let v = chunks_in_view(cam);
        assert!(v.iter().all(|p| (0..CHUNK_LAYERS).contains(&p.y)));
        assert!(v.contains(&ChunkPos { x: 0, y: 0, z: 0 }));
        assert!(v.contains(&ChunkPos { x: 3, y: 3, z: 0 })); // centre 112 blocks away
        assert!(!v.contains(&ChunkPos { x: 5, y: 0, z: 0 })); // centre 176 blocks away
        // Nearest first.
        let d: Vec<f32> = v.iter().map(|p| centre(*p).distance(cam)).collect();
        assert!(d.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn candidates_cover_view_anywhere_in_chunk() {
        let c = ChunkPos { x: 2, y: 3, z: -1 };
        let cand: HashSet<ChunkPos> = view_candidates(c).into_iter().collect();
        let lo = Vec3::new(64.0, 96.0, -32.0); // min corner of chunk c
        let hi = lo + Vec3::splat(CHUNK_SIZE as f32 - 0.01);
        for cam in [lo, hi, Vec3::new(lo.x, hi.y, hi.z), (lo + hi) / 2.0] {
            assert_eq!(
                ChunkPos::of_block(cam.x as i32, cam.y as i32, cam.z.floor() as i32),
                c
            );
            assert!(chunks_in_view(cam).iter().all(|p| cand.contains(p)));
        }
    }

    #[test]
    fn torch_flicker_factor_is_bounded_and_varies_with_time_and_seed() {
        let mut min_v = f32::INFINITY;
        let mut max_v = f32::NEG_INFINITY;
        for step in 0..40 {
            let t = step as f32 * 0.13;
            let f = torch_flicker_factor(t, 42);
            assert!(
                (0.80..=1.20).contains(&f),
                "flicker factor {f} out of bounds at t={t}"
            );
            min_v = min_v.min(f);
            max_v = max_v.max(f);
        }
        assert!(max_v - min_v > 0.05, "flicker should vary over time");
        assert_ne!(
            torch_flicker_factor(1.25, 1),
            torch_flicker_factor(1.25, 2),
            "different seeds should produce different phases"
        );
    }

    #[test]
    fn upload_budget_uploads_multiple_parts_and_never_stalls_large_chunks() {
        assert!(UPLOAD_PARTS_PER_FRAME > 1);
        let mut part_budget = UPLOAD_PARTS_PER_FRAME;
        let mut vert_budget = UPLOAD_VERTS_PER_FRAME;
        let mut uploaded_any = false;
        let mut count = 0;

        // Multiple 16k-vertex parts from one or more chunks upload in a single frame.
        for _ in 0..UPLOAD_PARTS_PER_FRAME {
            assert!(can_upload_part(
                part_budget,
                vert_budget,
                uploaded_any,
                16_000
            ));
            uploaded_any = true;
            part_budget -= 1;
            vert_budget -= 16_000;
            count += 1;
        }
        assert_eq!(count, UPLOAD_PARTS_PER_FRAME);
        assert!(!can_upload_part(
            part_budget,
            vert_budget,
            uploaded_any,
            1_000
        ));

        // Even a part exceeding UPLOAD_VERTS_PER_FRAME still uploads if nothing has
        // been uploaded yet this frame.
        assert!(can_upload_part(
            UPLOAD_PARTS_PER_FRAME,
            UPLOAD_VERTS_PER_FRAME,
            false,
            UPLOAD_VERTS_PER_FRAME + 10_000
        ));
    }

    #[test]
    fn apply_liquid_delta_skips_relight_on_amount_only_change() {
        let mut s = ChunkStream::default();
        let p = ChunkPos { x: 0, y: 0, z: 0 };
        s.insert_chunk(p, Arc::new(Chunk::filled(terravibe_sim::AIR)));
        s.meshed.insert(
            p,
            Meshed {
                entities: vec![],
                liquid_entity: None,
                material: Handle::default(),
                liquid_material: Handle::default(),
                quads: 0,
            },
        );
        let idx = (5 + 5 * 32 + 5 * 1024) as u16;
        let epoch_before = s.light_epoch();
        s.apply_liquid_delta(p, &[(idx, Liquid::pack(Some(Liquid::water(200))))]);
        assert!(
            s.light_epoch() > epoch_before,
            "empty -> water must relight"
        );
        assert!(s.urgent_liquid.contains(&p));
        assert!(!s.urgent.contains(&p));

        let epoch_mid = s.light_epoch();
        s.urgent_liquid.clear();
        s.apply_liquid_delta(p, &[(idx, Liquid::pack(Some(Liquid::water(120))))]);
        assert_eq!(
            s.light_epoch(),
            epoch_mid,
            "water -> water amount change must not bump light epoch"
        );
        assert!(s.urgent_liquid.contains(&p));
    }
}
