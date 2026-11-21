//! Diorama 3D background world and orbiting camera.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::camera::ClearColorConfig;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use terravibe_sim::{
    AIR, CHUNK_SIZE, CORRUPT_GRASS, Chunk, ChunkPos, DEMON_ALTAR, EBONSTONE, TORCH, TRUNK, World,
    generate_chunk,
};

use crate::art::chunk_mesh::mesh_chunk;
use crate::light::{ChunkMaterial, light_chunk, light_image};
use crate::session::AppState;
use crate::view::block_models::{BlockModels, spawn_models};
use crate::view::chunks::{ArtRes, ChunkMat, chunk_material, liquid_chunk_material};
use crate::view::liquid_mesh::mesh_liquid;
use crate::view::to_bevy_mesh_parts;

pub const DIORAMA_SEED: u32 = 1;

/// Orbiting camera parameters for the diorama view.
#[derive(Component)]
pub struct DioramaCamera {
    pub center: Vec3,
    pub radius: f32,
    pub height: f32,
    pub speed: f32,
}

pub(crate) fn setup_diorama(
    mut commands: Commands,
    state: Option<Res<State<AppState>>>,
    existing_cams: Query<&DioramaCamera>,
    art: Option<Res<ArtRes>>,
    chunk_mat: Option<Res<ChunkMat>>,
    models: Option<Res<BlockModels>>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut chunk_materials: Option<ResMut<Assets<ChunkMaterial>>>,
    mut images: Option<ResMut<Assets<Image>>>,
) {
    if let Some(s) = state {
        if *s.get() != AppState::Menu {
            return;
        }
    }

    if !existing_cams.is_empty() {
        return;
    }

    commands.insert_resource(ClearColor(Color::srgb(0.48, 0.72, 0.96)));

    // In minimal headless unit tests without full rendering assets, spawn test camera
    if images.is_none() {
        commands.spawn((
            Camera3d::default(),
            Camera {
                clear_color: ClearColorConfig::Custom(Color::srgb(0.48, 0.72, 0.96)),
                ..default()
            },
            IsDefaultUiCamera,
            Transform::from_xyz(40.0, 60.0, 40.0).looking_at(Vec3::ZERO, Vec3::Y),
            DioramaCamera {
                center: Vec3::ZERO,
                radius: 40.0,
                height: 20.0,
                speed: 0.06,
            },
            DespawnOnExit(AppState::Menu),
        ));
        return;
    }

    let (Some(art), Some(chunk_mat), Some(models)) = (art, chunk_mat, models) else {
        return;
    };
    let (Some(ref mut meshes), Some(ref mut chunk_materials), Some(ref mut images)) =
        (meshes.as_mut(), chunk_materials.as_mut(), images.as_mut())
    else {
        return;
    };

    // 1. Generate diorama mini world (5x5 horizontal columns, 4 vertical chunks each)
    let mut world = World::new();
    let mut chunks_map: HashMap<ChunkPos, Arc<Chunk>> = HashMap::new();

    for cx in -2..=2 {
        for cz in -2..=2 {
            for cy in 2..=5 {
                let cp = ChunkPos {
                    x: cx,
                    y: cy,
                    z: cz,
                };
                let chunk = generate_chunk(DIORAMA_SEED, cp);
                chunks_map.insert(cp, Arc::new(chunk.clone()));
                world.insert(cp, chunk);
            }
        }
    }

    // Find center surface height around (8, 8)
    let mut surface_y = 52.0;
    for y in (32..90).rev() {
        if world.block(8, y, 8).is_solid() {
            surface_y = y as f32;
            break;
        }
    }
    let center = Vec3::new(8.0, surface_y + 2.0, 8.0);
    let radius = 48.0;
    let height = 18.0;

    // Diorama Camera3d with sunlit sky clear color, DistanceFog, and slow orbit
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.48, 0.72, 0.96)),
            ..default()
        },
        DistanceFog {
            color: Color::srgb(0.55, 0.76, 0.96),
            falloff: FogFalloff::Linear {
                start: 42.0,
                end: 130.0,
            },
            ..default()
        },
        IsDefaultUiCamera,
        Transform::from_xyz(center.x + radius, center.y + height, center.z)
            .looking_at(center, Vec3::Y),
        DioramaCamera {
            center,
            radius,
            height,
            speed: 0.05,
        },
        DespawnOnExit(AppState::Menu),
    ));

    // Bright sunlit directional light
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.96, 0.88),
            illuminance: 18000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.75, 0.7, 0.0)),
        DespawnOnExit(AppState::Menu),
    ));

    // 2. Add signature diorama features: corruption patch, demon altar, tree trunk, torch
    for wx in 10..=14 {
        for wz in 4..=8 {
            for wy in (32..90).rev() {
                if world.block(wx, wy, wz).is_solid() && world.block(wx, wy + 1, wz) == AIR {
                    let cp = ChunkPos::of_block(wx, wy, wz);
                    if let Some(chunk_arc) = chunks_map.get_mut(&cp) {
                        let mut chunk = (**chunk_arc).clone();
                        let lx = wx.rem_euclid(CHUNK_SIZE) as usize;
                        let ly = wy.rem_euclid(CHUNK_SIZE) as usize;
                        let lz = wz.rem_euclid(CHUNK_SIZE) as usize;
                        chunk.set(lx, ly, lz, CORRUPT_GRASS);
                        if ly >= 1 {
                            chunk.set(lx, ly - 1, lz, EBONSTONE);
                        }
                        world.insert(cp, chunk.clone());
                        *chunk_arc = Arc::new(chunk);
                    }
                    break;
                }
            }
        }
    }

    // Place Demon Altar and Torch on the corruption surface
    let mut altar_y = None;
    for wy in (32..90).rev() {
        if world.block(12, wy, 6).is_solid() && world.block(12, wy + 1, 6) == AIR {
            altar_y = Some(wy + 1);
            break;
        }
    }
    if let Some(ay) = altar_y {
        let cp = ChunkPos::of_block(12, ay, 6);
        if let Some(chunk_arc) = chunks_map.get_mut(&cp) {
            let mut chunk = (**chunk_arc).clone();
            let lx = 12_i32.rem_euclid(CHUNK_SIZE) as usize;
            let ly = ay.rem_euclid(CHUNK_SIZE) as usize;
            let lz = 6_i32.rem_euclid(CHUNK_SIZE) as usize;
            chunk.set(lx, ly, lz, DEMON_ALTAR);
            world.insert(cp, chunk.clone());
            *chunk_arc = Arc::new(chunk);
        }
        // Place torch next to altar
        let cp_torch = ChunkPos::of_block(10, ay, 6);
        if let Some(chunk_arc) = chunks_map.get_mut(&cp_torch) {
            let mut chunk = (**chunk_arc).clone();
            let lx = 10_i32.rem_euclid(CHUNK_SIZE) as usize;
            let ly = ay.rem_euclid(CHUNK_SIZE) as usize;
            let lz = 6_i32.rem_euclid(CHUNK_SIZE) as usize;
            chunk.set(lx, ly, lz, TORCH);
            world.insert(cp_torch, chunk.clone());
            *chunk_arc = Arc::new(chunk);
        }
    }

    // Place extra trunk in forest
    for wy in (32..90).rev() {
        if world.block(4, wy, 10).is_solid() && world.block(4, wy + 1, 10) == AIR {
            let cp = ChunkPos::of_block(4, wy + 1, 10);
            if let Some(chunk_arc) = chunks_map.get_mut(&cp) {
                let mut chunk = (**chunk_arc).clone();
                let lx = 4_i32.rem_euclid(CHUNK_SIZE) as usize;
                let ly = (wy + 1).rem_euclid(CHUNK_SIZE) as usize;
                let lz = 10_i32.rem_euclid(CHUNK_SIZE) as usize;
                if ly + 2 < CHUNK_SIZE as usize {
                    chunk.set(lx, ly, lz, TRUNK);
                    chunk.set(lx, ly + 1, lz, TRUNK);
                    chunk.set(lx, ly + 2, lz, TRUNK);
                    world.insert(cp, chunk.clone());
                    *chunk_arc = Arc::new(chunk);
                }
            }
            break;
        }
    }

    // 3. Mesh and spawn chunk entities with lighting (including liquid/water meshes)
    for (&cp, chunk) in &chunks_map {
        let volume = light_chunk(cp, &world);
        let light = images.add(light_image(&volume));
        let cm = chunk_material(chunk_materials, &chunk_mat, cp, light.clone());

        let data = mesh_chunk(&world, cp, &art.0);
        let parts = to_bevy_mesh_parts(data, 2048);
        for mesh in parts {
            let handle = meshes.add(mesh);
            commands.spawn((
                Mesh3d(handle),
                MeshMaterial3d(cm.clone()),
                Transform::IDENTITY,
                DespawnOnExit(AppState::Menu),
            ));
        }

        if chunk.has_liquid() {
            if let Some(liquid_mesh) = mesh_liquid(chunk, cp, |x, y, z| world.liquid(x, y, z)) {
                let liquid_cm = liquid_chunk_material(chunk_materials, &chunk_mat, cp, light);
                let handle = meshes.add(liquid_mesh);
                commands.spawn((
                    Mesh3d(handle),
                    MeshMaterial3d(liquid_cm),
                    Transform::IDENTITY,
                    DespawnOnExit(AppState::Menu),
                ));
            }
        }

        let mut dummy = Vec::new();
        spawn_models(
            &mut commands,
            &models,
            cp,
            chunk,
            &chunks_map,
            &mut dummy,
            AppState::Menu,
        );
    }
}

pub fn orbit_diorama_camera(time: Res<Time>, mut cameras: Query<(&mut Transform, &DioramaCamera)>) {
    let t = time.elapsed_secs();
    for (mut transform, cam) in &mut cameras {
        let angle = t * cam.speed;
        let x = cam.center.x + cam.radius * angle.cos();
        let z = cam.center.z + cam.radius * angle.sin();
        let y = cam.center.y + cam.height + (angle * 0.7).sin() * 2.0;
        *transform = Transform::from_xyz(x, y, z).looking_at(cam.center, Vec3::Y);
    }
}

pub struct DioramaPlugin;

impl Plugin for DioramaPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, setup_diorama.run_if(in_state(AppState::Menu)))
            .add_systems(OnEnter(AppState::Menu), setup_diorama)
            .add_systems(
                Update,
                (setup_diorama, orbit_diorama_camera).run_if(in_state(AppState::Menu)),
            );
    }
}
