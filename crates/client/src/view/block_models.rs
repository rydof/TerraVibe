//! Placed `.vox` models for the non-cube blocks: forest canopies on trunk tops,
//! furniture, torches and plants. One mesh per model, shared by every instance.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;
use terravibe_sim::{
    ANVIL, BEE_LARVA, BOTTLE, BlockId, CAMPFIRE, CHAIR, CHEST, CHUNK_SIZE, Chunk, ChunkPos,
    DEMON_ALTAR, DOOR_CLOSED, DOOR_OPEN, FURNACE, GLOWING_MUSHROOM_PLANT, HEART_LANTERN, HELLFORGE,
    LIFE_CRYSTAL, MOSS, MUSHROOM_PLANT, PLATFORM, POT, SHELF_MUSHROOM, SMALL_ROCK, STALACTITE,
    TABLE, TINKERERS_WORKSHOP, TORCH, TRUNK, WORKBENCH,
};

use crate::art::mesh_data::MeshData;
use crate::art::voxmesh::mesh;
use crate::art::{art_dir, is_model_block, vox};
use crate::light::ChunkMaterial;
use crate::session::AppState;

/// Canopy files, picked per tree by [`canopy_variant`].
const CANOPY_FILES: [&str; 3] = ["canopy_0.vox", "canopy_1.vox", "canopy_2.vox"];
/// Canopy bottom sits this far below the trunk top so it is not perched.
const CANOPY_SINK: f32 = 0.5;

/// File under `assets/models/blocks/` drawing model block `id`.
pub fn model_file(id: BlockId) -> Option<&'static str> {
    Some(match id {
        WORKBENCH => "workbench.vox",
        ANVIL => "anvil.vox",
        FURNACE => "furnace.vox",
        CHEST => "chest.vox",
        DOOR_CLOSED => "door_closed.vox",
        DOOR_OPEN => "door_open.vox",
        TABLE => "table.vox",
        CHAIR => "chair.vox",
        PLATFORM => "platform.vox",
        TORCH => "torch.vox",
        LIFE_CRYSTAL => "life_crystal.vox",
        BOTTLE => "bottle.vox",
        MUSHROOM_PLANT => "mushroom.vox",
        DEMON_ALTAR => "demon_altar.vox",
        HELLFORGE => "hellforge.vox",
        POT => "pot.vox",
        STALACTITE => "stalactite.vox",
        MOSS => "moss.vox",
        SMALL_ROCK => "small_rock.vox",
        SHELF_MUSHROOM => "shelf_mushroom.vox",
        HEART_LANTERN => "heart_lantern.vox",
        GLOWING_MUSHROOM_PLANT => "glowing_mushroom.vox",
        TINKERERS_WORKSHOP => "tinkerers_workshop.vox",
        CAMPFIRE => "campfire.vox",
        BEE_LARVA => "life_crystal.vox",
        _ => return None,
    })
}

/// Static non-torch, non-canopy model `MeshData` shared with async chunk tasks.
pub(crate) struct BlockModelMeshes {
    pub(crate) block_data: HashMap<BlockId, MeshData>,
}

/// One `TORCH` entity placement collected off the main thread.
pub(crate) struct TorchPlacement {
    pub(crate) transform: Transform,
    pub(crate) seed: u32,
}

/// CPU-built chunk model output ready for cheap main-thread upload/spawn.
pub(crate) struct ChunkModelsOutput {
    pub(crate) batched_mesh: Option<Mesh>,
    pub(crate) canopies: Vec<(usize, Vec3)>,
    pub(crate) torches: Vec<TorchPlacement>,
}

/// Shared meshes (bottom-centre pivot) and the one vertex-coloured material.
#[derive(Resource)]
pub(crate) struct BlockModels {
    blocks: HashMap<BlockId, Handle<Mesh>>,
    canopies: [Handle<Mesh>; 3],
    meshes_data: Arc<BlockModelMeshes>,
    material: Handle<StandardMaterial>,
}

impl BlockModels {
    pub(crate) fn meshes_data(&self) -> Arc<BlockModelMeshes> {
        Arc::clone(&self.meshes_data)
    }
}

fn load_model_data(file: &str) -> MeshData {
    let m = vox::load(&art_dir().join("models/blocks").join(file));
    // Pivot: bottom centre, in voxels (file y is depth, see `tools/art/vox.py`).
    mesh(&m, [m.size[0] as f32 / 2.0, 0.0, m.size[1] as f32 / 2.0])
}

fn upload_model_data(meshes: &mut Assets<Mesh>, data: &MeshData) -> Handle<Mesh> {
    let cloned = MeshData {
        positions: data.positions.clone(),
        normals: data.normals.clone(),
        uvs: data.uvs.clone(),
        colors: data.colors.clone(),
        indices: data.indices.clone(),
    };
    meshes.add(super::to_bevy_mesh(cloned).unwrap_or_else(|| Mesh::from(Cuboid::default())))
}

pub(crate) fn setup_block_models(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut blocks = HashMap::new();
    let mut block_data = HashMap::new();
    for id in (0..=u16::MAX).map(BlockId).filter(|b| is_model_block(*b)) {
        if let Some(f) = model_file(id) {
            let data = load_model_data(f);
            blocks.insert(id, upload_model_data(&mut meshes, &data));
            block_data.insert(id, data);
        }
    }
    let canopy_data = CANOPY_FILES.map(load_model_data);
    let canopies = [
        upload_model_data(&mut meshes, &canopy_data[0]),
        upload_model_data(&mut meshes, &canopy_data[1]),
        upload_model_data(&mut meshes, &canopy_data[2]),
    ];
    commands.insert_resource(BlockModels {
        blocks,
        canopies,
        meshes_data: Arc::new(BlockModelMeshes { block_data }),
        material: materials.add(StandardMaterial {
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            ..default()
        }),
    });
}

/// Deterministic canopy variant for the trunk top at block (x, z).
fn canopy_variant(x: i32, z: i32) -> usize {
    let h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (z as u32).wrapping_mul(0x85EB_CA77);
    ((h ^ (h >> 15)).wrapping_mul(0x2C1B_3C6D) >> 16) as usize % CANOPY_FILES.len()
}

/// Minimum contiguous trunk height required to render a canopy (natural trees
/// are at least 5 blocks tall; shorter remaining trunks are cut stumps).
const MIN_CANOPY_TRUNK_HEIGHT: i32 = 5;

/// Canopy origins for every TRUNK of `chunk` whose block above is loaded and
/// not TRUNK (the trunk top), provided at least `MIN_CANOPY_TRUNK_HEIGHT`
/// contiguous trunk blocks sit at and below it.
fn canopy_positions(
    p: ChunkPos,
    chunk: &Chunk,
    all: &HashMap<ChunkPos, Arc<Chunk>>,
) -> Vec<[f32; 3]> {
    let n = CHUNK_SIZE as usize;
    let mut out = Vec::new();
    let block_at = |x: i32, y: i32, z: i32| {
        all.get(&ChunkPos::of_block(x, y, z)).map(|c| {
            c.get(
                x.rem_euclid(CHUNK_SIZE) as usize,
                y.rem_euclid(CHUNK_SIZE) as usize,
                z.rem_euclid(CHUNK_SIZE) as usize,
            )
        })
    };
    for lx in 0..n {
        for ly in 0..n {
            for lz in 0..n {
                if chunk.get(lx, ly, lz) != TRUNK {
                    continue;
                }
                let x = p.x * CHUNK_SIZE + lx as i32;
                let y = p.y * CHUNK_SIZE + ly as i32;
                let z = p.z * CHUNK_SIZE + lz as i32;
                let above = block_at(x, y + 1, z);
                if above.is_some_and(|b| b != TRUNK)
                    && (1..MIN_CANOPY_TRUNK_HEIGHT).all(|dy| block_at(x, y - dy, z) == Some(TRUNK))
                {
                    out.push([x as f32 + 0.5, y as f32 + 1.0, z as f32 + 0.5]);
                }
            }
        }
    }
    out
}

fn spawn_model_entity(
    commands: &mut Commands,
    models: &BlockModels,
    mesh: &Handle<Mesh>,
    t: Transform,
    state: AppState,
    out: &mut Vec<Entity>,
) -> Entity {
    let id = commands
        .spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(models.material.clone()),
            t,
            DespawnOnExit(state),
        ))
        .id();
    out.push(id);
    id
}

fn append_model_data(dst: &mut MeshData, src: &MeshData, offset: [f32; 3], rot_y_neg_90: bool) {
    let base = dst.positions.len() as u32;
    if rot_y_neg_90 {
        dst.positions.extend(
            src.positions
                .iter()
                .map(|&[x, y, z]| [-z + offset[0], y + offset[1], x + offset[2]]),
        );
        dst.normals
            .extend(src.normals.iter().map(|&[nx, ny, nz]| [-nz, ny, nx]));
    } else {
        dst.positions.extend(
            src.positions
                .iter()
                .map(|&[x, y, z]| [x + offset[0], y + offset[1], z + offset[2]]),
        );
        dst.normals.extend_from_slice(&src.normals);
    }
    dst.uvs.extend_from_slice(&src.uvs);
    dst.colors.extend_from_slice(&src.colors);
    dst.indices.extend(src.indices.iter().map(|&i| base + i));
}

/// Collects the `3x3x3` neighborhood `Arc<Chunk>` pointers needed by
/// [`build_chunk_models`] in O(1) on the main thread.
pub(crate) fn collect_model_chunks(
    p: ChunkPos,
    all: &HashMap<ChunkPos, Arc<Chunk>>,
) -> HashMap<ChunkPos, Arc<Chunk>> {
    let mut out = HashMap::with_capacity(27);
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                let cp = ChunkPos {
                    x: p.x + dx,
                    y: p.y + dy,
                    z: p.z + dz,
                };
                if let Some(c) = all.get(&cp) {
                    out.insert(cp, Arc::clone(c));
                }
            }
        }
    }
    out
}

/// Scans `chunk` and builds the batched static model `Mesh` plus sparse
/// `TORCH` placements off the main thread.
pub(crate) fn build_chunk_models(
    data_set: &BlockModelMeshes,
    p: ChunkPos,
    chunk: &Chunk,
    all: &HashMap<ChunkPos, Arc<Chunk>>,
) -> ChunkModelsOutput {
    let mut batched = MeshData::default();
    let mut canopies = Vec::new();
    let mut torches = Vec::new();
    for [x, y, z] in canopy_positions(p, chunk, all) {
        let v = canopy_variant(x.floor() as i32, z.floor() as i32);
        canopies.push((v, Vec3::new(x, y - CANOPY_SINK, z)));
    }

    let block = |x: i32, y: i32, z: i32| {
        all.get(&ChunkPos::of_block(x, y, z)).map(|c| {
            c.get(
                x.rem_euclid(CHUNK_SIZE) as usize,
                y.rem_euclid(CHUNK_SIZE) as usize,
                z.rem_euclid(CHUNK_SIZE) as usize,
            )
        })
    };
    let solid = |x, y, z| block(x, y, z).is_some_and(|b| b.is_solid());
    let n = CHUNK_SIZE as usize;
    for lx in 0..n {
        for ly in 0..n {
            for lz in 0..n {
                let b = chunk.get(lx, ly, lz);
                let Some(data) = data_set.block_data.get(&b) else {
                    continue;
                };
                let x = p.x * CHUNK_SIZE + lx as i32;
                let y = p.y * CHUNK_SIZE + ly as i32;
                let z = p.z * CHUNK_SIZE + lz as i32;
                if b == DEMON_ALTAR {
                    let a = |dx: i32, dy: i32, dz: i32| {
                        block(x + dx, y + dy, z + dz) == Some(DEMON_ALTAR)
                    };
                    if a(0, -1, 0) || a(-1, 0, 0) || a(0, 0, -1) {
                        continue;
                    }
                    let along_x = a(1, 0, 0);
                    let (tx, tz) = if along_x {
                        (x as f32 + 1.5, z as f32 + 0.5)
                    } else {
                        (x as f32 + 0.5, z as f32 + 1.5)
                    };
                    append_model_data(&mut batched, data, [tx, y as f32, tz], !along_x);
                    continue;
                }
                if b == TORCH {
                    let (dx, lean) =
                        torch_lean(solid(x, y - 1, z), solid(x - 1, y, z), solid(x + 1, y, z));
                    let mut transform = Transform::from_xyz(
                        x as f32 + 0.5 + dx,
                        y as f32 + if dx == 0.0 { 0.0 } else { 0.1 },
                        z as f32 + 0.5,
                    );
                    transform.rotation = Quat::from_rotation_z(lean);
                    let seed = (x as u32).wrapping_mul(0x9E37_79B1)
                        ^ (y as u32).wrapping_mul(0x85EB_CA77)
                        ^ (z as u32).wrapping_mul(0xC2B2_AE35);
                    torches.push(TorchPlacement { transform, seed });
                    continue;
                }
                append_model_data(
                    &mut batched,
                    data,
                    [x as f32 + 0.5, y as f32, z as f32 + 0.5],
                    false,
                );
            }
        }
    }

    ChunkModelsOutput {
        batched_mesh: super::to_bevy_mesh(batched),
        canopies,
        torches,
    }
}

/// Uploads the prebuilt batched static model mesh and spawns instanced canopies
/// plus sparse `TORCH` entities on the main thread.
pub(crate) fn spawn_chunk_models_output(
    commands: &mut Commands,
    models: &BlockModels,
    chunk_mat: &Handle<ChunkMaterial>,
    output: ChunkModelsOutput,
    meshes: &mut Assets<Mesh>,
    out: &mut Vec<Entity>,
    state: AppState,
) {
    if let Some(mesh) = output.batched_mesh {
        let id = commands
            .spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(chunk_mat.clone()),
                Transform::IDENTITY,
                DespawnOnExit(state),
            ))
            .id();
        out.push(id);
    }
    for (v, pos) in output.canopies {
        spawn_model_entity(
            commands,
            models,
            &models.canopies[v],
            Transform::from_translation(pos),
            state,
            out,
        );
    }
    if let Some(torch_mesh) = models.blocks.get(&TORCH) {
        for t in output.torches {
            let id = spawn_model_entity(commands, models, torch_mesh, t.transform, state, out);
            commands
                .entity(id)
                .insert(super::chunks::TorchFlicker { seed: t.seed });
        }
    }
}

/// Batches all static non-torch models and canopies of `chunk` into a single
/// `ChunkMaterial`-lit mesh (1 draw call per chunk), while keeping `TORCH`
/// entities individual so [`TorchFlicker`] continues to animate.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_chunk_models(
    commands: &mut Commands,
    models: &BlockModels,
    chunk_mat: &Handle<ChunkMaterial>,
    p: ChunkPos,
    chunk: &Chunk,
    all: &HashMap<ChunkPos, Arc<Chunk>>,
    meshes: &mut Assets<Mesh>,
    out: &mut Vec<Entity>,
    state: AppState,
) {
    let output = build_chunk_models(&models.meshes_data, p, chunk, all);
    spawn_chunk_models_output(commands, models, chunk_mat, output, meshes, out, state);
}

/// Spawns the canopies and model blocks of `chunk`; the entities despawn with
/// the chunk's other entities.
pub(crate) fn spawn_models(
    commands: &mut Commands,
    models: &BlockModels,
    p: ChunkPos,
    chunk: &Chunk,
    all: &HashMap<ChunkPos, Arc<Chunk>>,
    out: &mut Vec<Entity>,
    state: AppState,
) {
    for [x, y, z] in canopy_positions(p, chunk, all) {
        let v = canopy_variant(x.floor() as i32, z.floor() as i32);
        spawn_model_entity(
            commands,
            models,
            &models.canopies[v],
            Transform::from_xyz(x, y - CANOPY_SINK, z),
            state,
            out,
        );
    }
    let block = |x: i32, y: i32, z: i32| {
        all.get(&ChunkPos::of_block(x, y, z)).map(|c| {
            c.get(
                x.rem_euclid(CHUNK_SIZE) as usize,
                y.rem_euclid(CHUNK_SIZE) as usize,
                z.rem_euclid(CHUNK_SIZE) as usize,
            )
        })
    };
    let solid = |x, y, z| block(x, y, z).is_some_and(|b| b.is_solid());
    let n = CHUNK_SIZE as usize;
    for lx in 0..n {
        for ly in 0..n {
            for lz in 0..n {
                let b = chunk.get(lx, ly, lz);
                let Some(mesh) = models.blocks.get(&b) else {
                    continue;
                };
                let x = p.x * CHUNK_SIZE + lx as i32;
                let y = p.y * CHUNK_SIZE + ly as i32;
                let z = p.z * CHUNK_SIZE + lz as i32;
                if b == DEMON_ALTAR {
                    // One coherent 3x2 model per group, spawned once at the
                    // group's origin (the cell with no neighbour below or
                    // before it; worldgen keeps all six cells in one chunk).
                    let a = |dx: i32, dy: i32, dz: i32| {
                        block(x + dx, y + dy, z + dz) == Some(DEMON_ALTAR)
                    };
                    if a(0, -1, 0) || a(-1, 0, 0) || a(0, 0, -1) {
                        continue;
                    }
                    spawn_model_entity(
                        commands,
                        models,
                        mesh,
                        altar_transform([x, y, z], a(1, 0, 0)),
                        state,
                        out,
                    );
                    continue;
                }
                let mut t = Transform::from_xyz(x as f32 + 0.5, y as f32, z as f32 + 0.5);
                let id = spawn_model_entity(commands, models, mesh, t, state, out);
                if b == TORCH {
                    // Floor torch stands up; a wall torch leans away from its wall.
                    let (dx, lean) =
                        torch_lean(solid(x, y - 1, z), solid(x - 1, y, z), solid(x + 1, y, z));
                    t.translation += Vec3::new(dx, if dx == 0.0 { 0.0 } else { 0.1 }, 0.0);
                    t.rotation = Quat::from_rotation_z(lean);
                    let seed = (x as u32).wrapping_mul(0x9E37_79B1)
                        ^ (y as u32).wrapping_mul(0x85EB_CA77)
                        ^ (z as u32).wrapping_mul(0xC2B2_AE35);
                    commands
                        .entity(id)
                        .insert((t, super::chunks::TorchFlicker { seed }));
                }
            }
        }
    }
}

/// The authoring file is 3 wide (x), 2 tall, 1 deep; above it rotates that
/// length onto +z without mirroring (`Quat::from_rotation_y(-t/2)` maps model
/// +x to world +z), so the six world cells stay covered either way.
fn altar_transform(origin: [i32; 3], along_x: bool) -> Transform {
    let [x, y, z] = origin.map(|v| v as f32);
    let (tx, tz) = if along_x {
        (x + 1.5, z + 0.5)
    } else {
        (x + 0.5, z + 1.5)
    };
    let t = Transform::from_xyz(tx, y, tz);
    if along_x {
        t
    } else {
        t.with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2))
    }
}
/// Torch x offset and lean (radians about z) for a torch on the ground
/// (`below`), else on a solid block to its left / right.
fn torch_lean(below: bool, left: bool, right: bool) -> (f32, f32) {
    if below {
        (0.0, 0.0)
    } else if left {
        (-0.3, -0.35)
    } else if right {
        (0.3, 0.35)
    } else {
        (0.0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::art::voxmesh::VOXELS_PER_UNIT;
    const VPU: usize = VOXELS_PER_UNIT as usize;
    use std::collections::{BTreeSet, HashSet};
    use terravibe_sim::DEMON_ALTAR;

    /// `(game position, rgba)` for every voxel of a loaded model. The file
    /// stores `[x, depth, height]`; game axes are `(x, y up, z front)`.
    fn game_voxels(m: &vox::VoxModel) -> Vec<([usize; 3], [u8; 4])> {
        let depth = m.size[1] as usize;
        m.voxels
            .iter()
            .map(|&[x, y, z, i]| {
                (
                    [x as usize, z as usize, depth - 1 - y as usize],
                    m.palette[i as usize],
                )
            })
            .collect()
    }

    fn game_dims(m: &vox::VoxModel) -> (usize, usize, usize) {
        (m.size[0] as usize, m.size[2] as usize, m.size[1] as usize)
    }

    #[test]
    fn every_model_block_has_file() {
        let dir = art_dir().join("models/blocks");
        let mut n = 0;
        for id in (0..=u16::MAX).map(BlockId).filter(|b| is_model_block(*b)) {
            let f = model_file(id).unwrap_or_else(|| panic!("no model file for {id:?}"));
            assert!(dir.join(f).exists(), "missing {f}");
            n += 1;
        }
        assert!(n >= 13);
        for f in CANOPY_FILES {
            assert!(dir.join(f).exists(), "missing {f}");
        }
    }

    #[test]
    fn canopy_variant_stable() {
        let mut seen = [false; 3];
        for x in 0..20 {
            for z in 0..20 {
                assert_eq!(canopy_variant(x, z), canopy_variant(x, z));
                seen[canopy_variant(x, z)] = true;
            }
        }
        assert!(seen.iter().all(|&b| b));
    }

    #[test]
    fn canopy_positions_are_trunk_tops() {
        let mut all = HashMap::new();
        let mut c = Chunk::filled(terravibe_sim::AIR);
        for y in 2..7 {
            c.set(3, y, 4, TRUNK);
        }
        all.insert(ChunkPos { x: 0, y: 0, z: 0 }, Arc::new(c));
        let mut low = Chunk::filled(terravibe_sim::AIR);
        for y in 0..CHUNK_SIZE as usize {
            low.set(1, y, 1, TRUNK);
        }
        let lp = ChunkPos { x: 5, y: 0, z: 5 };
        all.insert(lp, Arc::new(low.clone()));
        assert!(canopy_positions(lp, &low, &all).is_empty());
        all.insert(
            ChunkPos { x: 5, y: 1, z: 5 },
            Arc::new(Chunk::filled(terravibe_sim::AIR)),
        );
        let top = CHUNK_SIZE as f32;
        assert_eq!(
            canopy_positions(lp, &low, &all),
            vec![[5.0 * top + 1.5, top, 5.0 * top + 1.5]]
        );
        let c0 = all[&ChunkPos { x: 0, y: 0, z: 0 }].clone();
        assert_eq!(
            canopy_positions(ChunkPos { x: 0, y: 0, z: 0 }, &c0, &all),
            vec![[3.5, 7.0, 4.5]]
        );
    }

    #[test]
    fn canopies_are_four_by_three_by_four_blocks_on_a_six_voxel_trunk() {
        let dir = art_dir().join("models/blocks");
        let mut shapes = Vec::new();
        for f in CANOPY_FILES {
            let m = vox::load(&dir.join(f));
            assert_eq!(
                game_dims(&m),
                (4 * VPU, 3 * VPU, 4 * VPU),
                "{f}: canopies are 4 blocks wide/deep and 3 tall at 1/16"
            );
            let vox = game_voxels(&m);
            let base: Vec<_> = vox.iter().filter(|(p, _)| p[1] == 0).collect();
            assert_eq!(base.len(), 36, "{f}: base must be a 6x6 trunk stub");
            assert!(
                base.iter()
                    .all(|(p, _)| (29..=34).contains(&p[0]) && (29..=34).contains(&p[2])),
                "{f}: trunk stub must sit at x/z 29..=34, centred on x = 32"
            );
            let trunk = base[0].1;
            assert!(
                base.iter().all(|(_, c)| *c == trunk),
                "{f}: the base must be one bark colour"
            );
            // Exactly three leaf shades, none of them the bark.
            let leaves: HashSet<_> = vox
                .iter()
                .filter(|(_, c)| *c != trunk)
                .map(|(_, c)| *c)
                .collect();
            assert_eq!(leaves.len(), 3, "{f}: want exactly 3 leaf shades");
            // The darkest shade is the underside: every leaf voxel with empty
            // space directly below it must carry it (bark is excluded because
            // the stub is stamped after shading).
            let mut darkest = *leaves.iter().next().unwrap();
            for c in &leaves {
                if u32::from(c[0]) + u32::from(c[1]) + u32::from(c[2])
                    < u32::from(darkest[0]) + u32::from(darkest[1]) + u32::from(darkest[2])
                {
                    darkest = *c;
                }
            }
            let occupied: BTreeSet<_> = vox.iter().map(|(p, _)| *p).collect();
            for (p, c) in &vox {
                if *c == trunk {
                    continue;
                }
                let below_empty = p[1] == 0 || !occupied.contains(&[p[0], p[1] - 1, p[2]]);
                assert!(
                    !below_empty || *c == darkest,
                    "{f}: leaf voxel {p:?} has empty space below but is not the darkest shade"
                );
            }
            shapes.push(occupied);
        }
        assert_ne!(shapes[0], shapes[1], "canopy variants 0 and 1 must differ");
        assert_ne!(shapes[1], shapes[2], "canopy variants 1 and 2 must differ");
        assert_ne!(shapes[0], shapes[2], "canopy variants 0 and 2 must differ");
    }

    #[test]
    fn mushroom_is_a_small_red_herb_with_white_specks() {
        let m = vox::load(&art_dir().join("models/blocks/mushroom.vox"));
        let (w, h, d) = game_dims(&m);
        assert!(w <= 8 && d <= 8, "mushroom {w}x{d} wider than 8 voxels");
        assert!(h <= 6, "mushroom {h} voxels tall, want at most 6");
        let vox = game_voxels(&m);
        let base: Vec<_> = vox.iter().filter(|(p, _)| p[1] == 0).collect();
        assert!(!base.is_empty(), "mushroom must sit flush at y = 0");
        assert!(
            base.len() <= 4,
            "mushroom must have a thin stem, not an {}-voxel pedestal",
            base.len()
        );
        let red = vox
            .iter()
            .filter(|(_, c)| {
                c[0] > 150
                    && i32::from(c[0]) > i32::from(c[1]) + 40
                    && i32::from(c[0]) > i32::from(c[2]) + 40
            })
            .count();
        assert!(red >= 10, "mushroom cap needs red voxels, found {red}");
        let white: Vec<_> = vox
            .iter()
            .filter(|(_, c)| c[..3].iter().all(|&v| v >= 240))
            .collect();
        assert!(white.len() >= 3, "mushroom needs white specks");
        assert!(
            white.iter().any(|(p, _)| p[1] >= 2),
            "white specks belong on the cap"
        );
    }

    #[test]
    fn torch_has_a_three_by_three_handle_and_a_small_three_colour_flame() {
        let m = vox::load(&art_dir().join("models/blocks/torch.vox"));
        let vox = game_voxels(&m);
        let base: Vec<_> = vox.iter().filter(|(p, _)| p[1] == 0).collect();
        assert_eq!(base.len(), 9, "torch handle is a 3x3 column");
        assert!(
            base.iter()
                .all(|(p, _)| (1..=3).contains(&p[0]) && (1..=3).contains(&p[2])),
            "torch handle must be 3x3 at the base"
        );
        let flame: Vec<_> = vox
            .iter()
            .filter(|(_, c)| {
                c[0] > 200 && c[1] > 60 && c[2] < 150 && i32::from(c[0]) > i32::from(c[2]) + 100
            })
            .collect();
        let ys: Vec<_> = flame.iter().map(|(p, _)| p[1]).collect();
        let (lo, hi) = (*ys.iter().min().unwrap(), *ys.iter().max().unwrap());
        assert!(
            (4..=6).contains(&(hi - lo + 1)),
            "flame spans {} voxels, want 4..=6",
            hi - lo + 1
        );
        let colours: HashSet<_> = flame.iter().map(|(_, c)| *c).collect();
        assert_eq!(colours.len(), 3, "flame needs exactly 3 colours");
    }

    #[test]
    fn bottle_and_life_crystal_are_one_block_with_three_shades() {
        let dir = art_dir().join("models/blocks");
        for f in ["bottle.vox", "life_crystal.vox"] {
            let m = vox::load(&dir.join(f));
            assert_eq!(game_dims(&m), (VPU, VPU, VPU), "{f}: one 16-voxel block");
            let shades: HashSet<_> = m.voxels.iter().map(|v| m.palette[v[3] as usize]).collect();
            assert!(
                shades.len() >= 3,
                "{f}: {} shades, want at least 3",
                shades.len()
            );
        }
        // The bottle's three shades are glass (cool) colours, not cork or a
        // single white glint.
        let m = vox::load(&dir.join("bottle.vox"));
        let glass: HashSet<_> = m
            .voxels
            .iter()
            .map(|v| m.palette[v[3] as usize])
            .filter(|c| c[2] > c[0] && c[1] > c[0])
            .collect();
        assert!(glass.len() >= 3, "bottle needs 3 glass shades");
    }

    #[test]
    fn torch_lean_prefers_floor_then_left_then_right() {
        assert_eq!(torch_lean(true, true, true), (0.0, 0.0));
        assert!(torch_lean(false, true, true).0 < 0.0);
        assert!(torch_lean(false, false, true).0 > 0.0);
        assert_eq!(torch_lean(false, false, false), (0.0, 0.0));
    }

    #[test]
    fn model_meshes_stay_near_their_block() {
        let dir = art_dir().join("models/blocks");
        for id in (0..=u16::MAX).map(BlockId).filter(|b| is_model_block(*b)) {
            let m = vox::load(&dir.join(model_file(id).unwrap()));
            let d = mesh(&m, [m.size[0] as f32 / 2.0, 0.0, m.size[1] as f32 / 2.0]);
            assert!(!d.positions.is_empty());
            // One block around the pivot, except the 3x2x1 demon altar.
            let (wide, tall) = if id == DEMON_ALTAR {
                (1.51, 2.01)
            } else {
                (0.51, 1.01)
            };
            for p in &d.positions {
                assert!(p[0].abs() <= wide && (-0.01..=tall).contains(&p[1]) && p[2].abs() <= wide);
            }
        }
    }

    #[test]
    fn chest_is_one_by_one_block() {
        // The sim places a chest in one cell; the model fills that cell.
        let m = vox::load(&art_dir().join("models/blocks/chest.vox"));
        assert_eq!(game_dims(&m), (VPU, VPU, VPU), "chest: one 16-voxel block");
    }

    #[test]
    fn workbench_is_one_by_one_block() {
        let m = vox::load(&art_dir().join("models/blocks/workbench.vox"));
        assert_eq!(
            game_dims(&m),
            (VPU, VPU, VPU),
            "workbench: one 16-voxel block"
        );
    }

    #[test]
    fn door_is_one_by_one_block_and_four_voxels_thick() {
        // A placed door fills one cell (one block tall, like the other furniture),
        // so the 1x3-block door of the plan would overhang the cell above it.
        let dir = art_dir().join("models/blocks");
        let closed = vox::load(&dir.join("door_closed.vox"));
        assert_eq!(game_dims(&closed), (VPU, VPU, 4), "closed door: 16x16x4");
        let open = vox::load(&dir.join("door_open.vox"));
        assert_eq!(game_dims(&open), (VPU, VPU, VPU), "open door: one block");
        let leaf_x = game_voxels(&open).iter().map(|(p, _)| p[0]).max();
        assert!(
            leaf_x.is_some_and(|x| x < 4),
            "open door leaf is 4 voxels thick"
        );
    }

    #[test]
    fn altar_model_registers_and_spans_three_by_two_by_one() {
        // Voxel visuals for the worldgen altars: one coherent 3x2x1 model per
        // group, spawned once at the origin cell (see `altar_transform`).
        assert!(is_model_block(DEMON_ALTAR));
        assert_eq!(model_file(DEMON_ALTAR), Some("demon_altar.vox"));
        let dir = art_dir().join("models/blocks");
        let m = vox::load(&dir.join("demon_altar.vox"));
        // VOXELS_PER_UNIT voxels per block: game dims are 3 along
        // x, 2 up, 1 deep; the file stores them as [x, z, y].
        assert_eq!(
            (m.size[0] as usize, m.size[2] as usize, m.size[1] as usize),
            (3 * VPU, 2 * VPU, VPU)
        );
        let d = mesh(&m, [m.size[0] as f32 / 2.0, 0.0, m.size[1] as f32 / 2.0]);
        assert!(!d.positions.is_empty());
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        for p in &d.positions {
            for a in 0..3 {
                lo[a] = lo[a].min(p[a]);
                hi[a] = hi[a].max(p[a]);
            }
        }
        // VOXELS_PER_UNIT voxels per block: 3 blocks along x,
        // 2 tall, 1 deep, spanning the six world cells of one altar group.
        // Mesh positions are world units (voxel corners / VOXELS_PER_UNIT), so each span
        // is `want` units directly.
        for (a, want) in [0, 1, 2].into_iter().zip([3.0, 2.0, 1.0]) {
            let span = hi[a] - lo[a];
            assert!(
                (span - want).abs() < 0.01,
                "axis {a} spans {span}, want {want} blocks"
            );
        }
    }

    #[test]
    fn altar_transform_covers_the_six_cells() {
        // The coherent model reaches every cell of both rows when oriented
        // along the chasm axis (the pivot stays the bottom-centre of the
        // group, just like the per-cell furniture models).
        let (min, size) = (Vec3::new(-1.5, -0.0, -0.5), Vec3::new(3.0, 2.0, 1.0));
        for along_x in [true, false] {
            for origin in [[10, 2, 20], [-40, 65, -9]] {
                let t = altar_transform(origin, along_x);
                for i in 0..3 {
                    for j in 0..2 {
                        let cell = [
                            origin[0] + if along_x { i } else { 0 },
                            origin[1] + j,
                            origin[2] + if along_x { 0 } else { i },
                        ];
                        let world = Vec3::new(
                            cell[0] as f32 + 0.5,
                            cell[1] as f32 + 0.5,
                            cell[2] as f32 + 0.5,
                        );
                        // The cell centre in the model's local frame.
                        let local = t.rotation.inverse() * (world - t.translation);
                        assert!(
                            local.x >= min.x - 0.01
                                && local.x <= min.x + size.x + 0.01
                                && local.y >= min.y - 0.01
                                && local.y <= min.y + size.y + 0.01
                                && local.z >= min.z - 0.01
                                && local.z <= min.z + size.z + 0.01,
                            "cell {cell:?} not covered when along_x={along_x} (local {local:?})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn canopy_skips_cut_stumps_and_places_on_tall_trunks() {
        let p = ChunkPos { x: 0, y: 0, z: 0 };
        let mut chunk = Chunk::filled(terravibe_sim::block::AIR);
        // Cut 2-block stump at (2, 1..=2, 2): no canopy.
        chunk.set(2, 1, 2, TRUNK);
        chunk.set(2, 2, 2, TRUNK);
        // Full 5-block tree at (6, 1..=5, 6): spawns 1 canopy at top (6, 5, 6).
        for y in 1..=5 {
            chunk.set(6, y, 6, TRUNK);
        }
        let mut all = HashMap::new();
        all.insert(p, Arc::new(chunk));
        let chunk_ref = all.get(&p).unwrap();
        let canopies = canopy_positions(p, chunk_ref, &all);
        assert_eq!(canopies, vec![[6.5, 6.0, 6.5]]);
    }
}
