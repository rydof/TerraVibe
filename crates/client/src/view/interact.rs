//! World-side item visuals and sounds: block cursor, tile cracks, the held
//! item, use sounds and the `TERRAVIBE_DEMO=dig|tunnel` screenshot scripts.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;
use terravibe_assets::{RgbaImage, TerrariaContent};
use terravibe_sim::interact::{Hit, RAY_MAX, UseEvent, in_reach, overlaps_player, raycast_pick};
use terravibe_sim::item::{
    BREAKER_BLADE, CLOCKWORK_ASSAULT_RIFLE, DEMON_BOW, EMPTY_BUCKET, FIERY_GREATSWORD,
    FLINTLOCK_PISTOL, GOLD_BROADSWORD, HANDGUN, ItemClass, ItemId, LAVA_BUCKET, LIGHTS_BANE,
    MINISHARK, MOLTEN_FURY, MOLTEN_HAMAXE, MOLTEN_PICKAXE, MURAMASA, MUSKET, NIGHTMARE_PICKAXE,
    PHOENIX_BLASTER, SILVER_BROADSWORD, WAND_OF_SPARKING, WAR_AXE_OF_THE_NIGHT, WATER_BUCKET,
    item_data, prefix_stats,
};
use terravibe_sim::{
    AIR, BLUE_DUNGEON_BRICK, BlockQuery, CHAIR, COPPER, ChunkPos, DEMONITE_ORE, EBONSTONE, GOLD,
    GREEN_DUNGEON_BRICK, HELLFORGE, HELLSTONE, HELLSTONE_BRICK, IRON, LIFE_CRYSTAL, OBSIDIAN,
    OBSIDIAN_BRICK, PINK_DUNGEON_BRICK, PlayerBody, SILVER, SPIKES, STONE, TABLE, TORCH,
};

use super::avatar::ItemModels;
use super::camera::{CameraMode, ControlMode, FlyCamera, OrbitDist};
use super::chunks::ChunkStream;
use super::held_anim::{
    HELD_POS, HeldAnimKind, HeldAnimTracker, compute_fp_transform, held_anim_kind,
};
use super::particles::{ParticlePool, material_for_block, particle_color};
use super::player::{LoadedBlocks, LocalPlayer, SampledInput, sample_input};
use super::{to_bevy_image, to_bevy_mesh};
use crate::art::mesh_data::flat_face;
use crate::art::mesh_data::{Face, MeshData, UvRect};
use crate::audio::{SfxEvent, SfxKind};
use crate::net::LocalInventory;
use crate::session::AppState;
use crate::settings::Settings;

/// Env var: `dig` runs a scripted dig-and-place through the normal inputs;
/// `tunnel` carves a sealed tunnel with a torch for the lighting screenshot.
const ENV_DEMO: &str = "TERRAVIBE_DEMO";
/// Crack sheet layout from `Main.tile crack rendering`: source rect
/// `(crackStyle * 18, stage * 18, 16, 16)`, so a column is a random variant
/// (`tile damage tracking.crackStyle`, 0..4) and a row is the damage stage (0..4).
/// Cell size and frame size, in pixels.
const CRACK_CELL: u32 = 18;
const CRACK_FRAME: u32 = 16;
const CRACK_VARIANTS: u32 = 4;
/// How far the overlay sits outside the block faces, clearing raised relief.
const CRACK_LIFT: f32 = 0.12;
/// Seconds a crack overlay outlives its last hit (tile damage tracking entries live 60 ticks).
const CRACK_TTL: f32 = 1.0;
const TICKS_PER_SEC: f32 = 60.0;

/// An item-use event from the server, for visuals and sounds.
#[derive(Message)]
pub struct UseEvents(pub UseEvent);

/// Terraria crack frame for a block's accumulated damage, `None` below the first.
/// `Main.tile crack rendering`: nothing below 20 damage, then stage 0 at 20, 1 at 40,
/// 2 at 60 and 3 from 80.
pub(crate) fn crack_stage(damage: i32) -> Option<usize> {
    (damage >= 20).then(|| ((damage / 20 - 1) as usize).min(3))
}

/// Top-left pixel of a crack frame: variant picks the column, stage the row.
pub(crate) fn crack_origin(stage: u32, variant: u32) -> (f32, f32) {
    ((variant * CRACK_CELL) as f32, (stage * CRACK_CELL) as f32)
}

/// Raw wav bytes for use sounds.
pub struct UseSounds {
    swing: Option<Vec<u8>>,
    dig: Vec<Vec<u8>>,
    tink: Vec<Vec<u8>>,
}

impl UseSounds {
    pub fn load(c: &TerrariaContent) -> Self {
        let get = |name: String| match c.sound(&name) {
            Ok(s) => Some(s.to_wav()),
            Err(e) => {
                warn!("{name}: {e}");
                None
            }
        };
        let many = |base: &str| {
            (0..3)
                .filter_map(|i| get(format!("Sounds/{base}_{i}")))
                .collect()
        };
        UseSounds {
            // Item.item defaults: tools, swords, blocks and torches use UseSound = Item_1.
            swing: get("Sounds/Item_1".into()),
            dig: many("Dig"),
            tink: many("Tink"),
        }
    }
}

#[derive(Resource)]
struct UseAudio {
    swing: Option<Handle<AudioSource>>,
    dig: Vec<Handle<AudioSource>>,
    tink: Vec<Handle<AudioSource>>,
}

pub struct InteractPlugin {
    pub cracks: Arc<RgbaImage>,
    #[allow(dead_code)]
    pub icons: Arc<HashMap<u16, RgbaImage>>,
    pub sounds: UseSounds,
}

#[derive(Resource)]
struct CrackAssets {
    /// Meshes by `[stage][variant]`.
    meshes: Vec<Vec<Handle<Mesh>>>,
    material: Handle<StandardMaterial>,
}

#[derive(Resource)]
struct CrackSheet(Arc<RgbaImage>);

#[derive(Resource, Default)]
struct Cracks(HashMap<[i32; 3], (Entity, f32)>);

/// Minimum swing animation duration (seconds) so items with `use_animation == 0`
/// still produce a visible swing arc.
pub(crate) const MIN_SWING_SECS: f32 = 0.18;

/// Computes normalized active-use progress in `[0.0, 1.0)` for a swing.
pub(crate) fn compute_swing_use_t(now: f32, start: f32, secs: f32) -> Option<f32> {
    let t = (now - start) / secs.max(MIN_SWING_SECS);
    (0.0..1.0).contains(&t).then_some(t)
}

/// Elemental / metallic glint particle tint `(primary, secondary, size)` for held & swung items.
pub fn weapon_particle_tint(id: ItemId) -> Option<(Color, Color, f32)> {
    match id {
        FIERY_GREATSWORD | MOLTEN_PICKAXE | MOLTEN_HAMAXE | MOLTEN_FURY | PHOENIX_BLASTER => {
            Some((
                Color::srgb(1.0, 0.45, 0.08),
                Color::srgb(1.0, 0.85, 0.20),
                0.13,
            ))
        }
        LIGHTS_BANE | NIGHTMARE_PICKAXE | WAR_AXE_OF_THE_NIGHT | DEMON_BOW => Some((
            Color::srgb(0.62, 0.22, 0.95),
            Color::srgb(0.35, 0.10, 0.65),
            0.12,
        )),
        MURAMASA => Some((
            Color::srgb(0.25, 0.65, 1.0),
            Color::srgb(0.75, 0.92, 1.0),
            0.12,
        )),
        WAND_OF_SPARKING => Some((
            Color::srgb(1.0, 0.72, 0.15),
            Color::srgb(1.0, 0.35, 0.05),
            0.10,
        )),
        id if id == ItemId(TORCH.0) => Some((
            Color::srgb(1.0, 0.60, 0.15),
            Color::srgb(1.0, 0.85, 0.35),
            0.09,
        )),
        GOLD_BROADSWORD | SILVER_BROADSWORD | BREAKER_BLADE => Some((
            Color::srgb(1.0, 0.96, 0.78),
            Color::srgb(0.85, 0.90, 1.0),
            0.10,
        )),
        _ => None,
    }
}

#[derive(Resource, Default)]
pub(crate) struct Swing {
    pub(crate) start: f32,
    pub(crate) secs: f32,
}

#[derive(Component)]
struct HeldItem;

#[derive(Component)]
struct PlacementGhost;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlacementShape {
    Cube,
    Wall,
    Torch,
}

#[derive(Resource)]
struct PlacementGhostAssets {
    cube_mesh: Handle<Mesh>,
    wall_mesh: Handle<Mesh>,
    torch_mesh: Handle<Mesh>,
    valid_mat: Handle<StandardMaterial>,
    invalid_mat: Handle<StandardMaterial>,
}

/// Present while the `TERRAVIBE_DEMO` script is running.
#[derive(Resource)]
pub struct DemoPending;

fn player_mode(m: Res<ControlMode>) -> bool {
    *m == ControlMode::Player
}

/// True when `TERRAVIBE_DEMO=tunnel` is set. The tunnel demo owns the camera,
/// so the view starts in fly mode.
pub(crate) fn demo_is_tunnel() -> bool {
    std::env::var(ENV_DEMO)
        .as_deref()
        .is_ok_and(|v| v == "tunnel" || v == "progression")
}

/// Tracks per-frame gamepad attack (RT) and secondary use/interact (LT) state.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub(crate) struct GamepadInteractState {
    pub attack_held: bool,
    pub use_just_pressed: bool,
}

/// Returns `(attack_held, use_just_pressed)` from `gamepad` triggers (RT / LT).
pub(crate) fn gamepad_attack_and_use(gamepad: &Gamepad) -> (bool, bool) {
    let rt = gamepad.pressed(GamepadButton::RightTrigger2)
        || gamepad.get(GamepadAxis::RightZ).unwrap_or(0.0) > 0.5;
    let lt = gamepad.just_pressed(GamepadButton::LeftTrigger2);
    (rt, lt)
}

fn apply_gamepad_interact(gamepads: Query<&Gamepad>, mut state: ResMut<GamepadInteractState>) {
    let mut attack_held = false;
    let mut use_just_pressed = false;
    for gp in &gamepads {
        let (rt, lt) = gamepad_attack_and_use(gp);
        attack_held |= rt;
        use_just_pressed |= lt;
    }
    state.attack_held = attack_held;
    state.use_just_pressed = use_just_pressed;
}

impl Plugin for InteractPlugin {
    fn build(&self, app: &mut App) {
        let mut add = |b: &Vec<u8>| {
            let bytes: Arc<[u8]> = b.clone().into();
            app.world_mut()
                .resource_mut::<Assets<AudioSource>>()
                .add(AudioSource { bytes })
        };
        let audio = UseAudio {
            swing: self.sounds.swing.as_ref().map(&mut add),
            dig: self.sounds.dig.iter().map(&mut add).collect(),
            tink: self.sounds.tink.iter().map(&mut add).collect(),
        };
        app.insert_resource(audio)
            .insert_resource(CrackSheet(self.cracks.clone()))
            .init_resource::<Cracks>()
            .init_resource::<Swing>()
            .init_resource::<HeldAnimTracker>()
            .init_resource::<GamepadInteractState>()
            .init_resource::<crate::hud::inventory::RulerEnabled>()
            .init_resource::<crate::hud::inventory::BuildGridEnabled>()
            .init_resource::<crate::hud::inventory::RulerReadout>()
            .add_message::<UseEvents>()
            .add_systems(Startup, setup_cracks)
            .add_systems(OnExit(AppState::InGame), |mut cracks: ResMut<Cracks>| {
                cracks.0.clear();
            })
            .add_systems(
                Update,
                (
                    apply_gamepad_interact.after(sample_input),
                    cursor,
                    on_use.after(crate::net::pump),
                    expire_cracks,
                    update_held_anim_tracker.after(on_use),
                    held_item.after(update_held_anim_tracker),
                )
                    .run_if(player_mode)
                    .run_if(resource_exists::<LocalPlayer>)
                    .run_if(in_state(AppState::InGame))
                    .run_if(crate::menu::menu_closed),
            );
        match std::env::var(ENV_DEMO).as_deref() {
            Ok("dig" | "crack") => {
                app.insert_resource(DemoPending).add_systems(
                    Update,
                    demo.after(sample_input)
                        .run_if(player_mode)
                        .run_if(in_state(AppState::InGame)),
                );
            }
            Ok("tunnel") => {
                app.insert_resource(DemoPending).add_systems(
                    Update,
                    tunnel_demo
                        .before(super::chunks::stream_chunks)
                        .run_if(in_state(AppState::InGame)),
                );
            }
            Ok("progression") => {
                app.insert_resource(DemoPending).add_systems(
                    Update,
                    progression_demo
                        .before(super::chunks::stream_chunks)
                        .run_if(in_state(AppState::InGame)),
                );
            }
            _ => {}
        }
    }
}

fn setup_cracks(
    mut commands: Commands,
    sheet: Res<CrackSheet>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let sheet = &sheet.0;
    let (w, h) = (sheet.width as f32, sheet.height as f32);
    let stages = (0..4u32)
        .map(|st| {
            (0..CRACK_VARIANTS)
                .map(|variant| {
                    let (x, y) = crack_origin(st, variant);
                    let uv = UvRect {
                        u0: x / w,
                        v0: y / h,
                        u1: (x + CRACK_FRAME as f32) / w,
                        v1: (y + CRACK_FRAME as f32) / h,
                    };
                    let mut m = MeshData::default();
                    for f in Face::ALL {
                        flat_face(f, [0.0; 3], uv, &mut m);
                    }
                    meshes.add(to_bevy_mesh(m).expect("cube"))
                })
                .collect()
        })
        .collect();
    commands.insert_resource(CrackAssets {
        meshes: stages,
        material: materials.add(StandardMaterial {
            base_color_texture: Some(images.add(to_bevy_image(sheet))),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
    });
    commands.insert_resource(PlacementGhostAssets {
        cube_mesh: meshes.add(Cuboid::new(0.99, 0.99, 0.99)),
        wall_mesh: meshes.add(Cuboid::new(0.98, 0.98, 0.22)),
        torch_mesh: meshes.add(Cuboid::new(0.24, 0.72, 0.24)),
        valid_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(0.62, 0.92, 1.0, 0.34),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
        invalid_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.32, 0.32, 0.28),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
    });
}

/// Extra ray length for the camera's orbit distance in third person.
pub(crate) fn cam_mode(orbit: &OrbitDist, mode: &CameraMode) -> f32 {
    if *mode == CameraMode::Third {
        orbit.0
    } else {
        0.0
    }
}

pub(crate) fn is_bucket(id: ItemId) -> bool {
    id == EMPTY_BUCKET || id == WATER_BUCKET || id == LAVA_BUCKET
}

/// The block under the screen centre: the ray runs from the camera along its
/// view axis (through the head in third person, since the camera orbits it),
/// and counts only if the block is within the player's reach.
/// Ignores liquid cells unless `held` is a bucket (`EMPTY_BUCKET`, `WATER_BUCKET`, `LAVA_BUCKET`).
pub(crate) fn target_block(
    q: &impl BlockQuery,
    cam: &Transform,
    body: &PlayerBody,
    extra: f32,
    held: Option<ItemId>,
) -> Option<Hit> {
    let include_liquid = held.is_some_and(is_bucket);
    raycast_pick(
        q,
        cam.translation.into(),
        (*cam.forward()).into(),
        RAY_MAX + extra,
        include_liquid,
    )
    .filter(|h| in_reach(body, h.block))
}

/// Computes the placement ghost cell, validity, and preview shape for `held`.
pub(crate) fn placement_preview_cell(
    q: &impl BlockQuery,
    body: &PlayerBody,
    hit: Hit,
    held: ItemId,
) -> Option<([i32; 3], bool, PlacementShape)> {
    let data = item_data(held);
    let c = [
        hit.block[0] + hit.normal[0],
        hit.block[1] + hit.normal[1],
        hit.block[2] + hit.normal[2],
    ];
    if data.class == ItemClass::Wall {
        let valid = in_reach(body, c) && q.block(c[0], c[1], c[2]) == AIR;
        return Some((c, valid, PlacementShape::Wall));
    }
    let tile = data.create_tile?;
    let free = q.block(c[0], c[1], c[2]) == AIR;
    let blocks_player = tile.is_solid() && overlaps_player(body, c);
    let supported = tile != TORCH
        || [[0, -1, 0], [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]]
            .iter()
            .any(|n| q.block(c[0] + n[0], c[1] + n[1], c[2] + n[2]).is_solid());
    let floor_ok = !matches!(tile, TABLE | CHAIR) || {
        let b = q.block(c[0], c[1] - 1, c[2]);
        b.is_solid() || b.is_platform()
    };
    let valid = free && !blocks_player && supported && floor_ok && in_reach(body, c);
    let shape = if tile == TORCH {
        PlacementShape::Torch
    } else {
        PlacementShape::Cube
    };
    Some((c, valid, shape))
}

/// Crisp outline on the targeted block + translucent placement ghost preview.
#[allow(clippy::too_many_arguments)]
fn cursor(
    mut commands: Commands,
    mut gizmos: Gizmos,
    cam: Single<&Transform, With<FlyCamera>>,
    stream: Res<ChunkStream>,
    player: Res<LocalPlayer>,
    mode: Res<CameraMode>,
    orbit: Res<OrbitDist>,
    inv: Option<Res<LocalInventory>>,
    ghost_assets: Option<Res<PlacementGhostAssets>>,
    ruler: Option<Res<crate::hud::inventory::RulerEnabled>>,
    build_grid: Option<Res<crate::hud::inventory::BuildGridEnabled>>,
    mut ruler_readout: Option<ResMut<crate::hud::inventory::RulerReadout>>,
    mut ghost_q: Query<
        (
            &mut Transform,
            &mut Mesh3d,
            &mut MeshMaterial3d<StandardMaterial>,
            &mut Visibility,
        ),
        (With<PlacementGhost>, Without<FlyCamera>),
    >,
) {
    let q = LoadedBlocks(stream.generated());
    let held = inv
        .as_ref()
        .and_then(|i| i.0.selected_stack())
        .map(|s| s.id);
    let hit = target_block(&q, &cam, &player.0.body, cam_mode(&orbit, &mode), held);
    if let Some(h) = hit {
        let c =
            Vec3::new(h.block[0] as f32, h.block[1] as f32, h.block[2] as f32) + Vec3::splat(0.5);
        // Crisp two-tone wireframe: dark outer frame + subtle bright inner edge.
        gizmos.cube(
            Transform::from_translation(c).with_scale(Vec3::splat(1.003)),
            Color::srgba(0.04, 0.04, 0.06, 0.95),
        );
        gizmos.cube(
            Transform::from_translation(c).with_scale(Vec3::splat(1.007)),
            Color::srgba(1.0, 1.0, 1.0, 0.42),
        );
    }

    let preview =
        hit.and_then(|h| held.and_then(|id| placement_preview_cell(&q, &player.0.body, h, id)));

    if ruler.is_some_and(|r| r.0) {
        if let Some(h) = hit {
            let p_blk = [
                player.0.body.pos[0].floor() as i32,
                player.0.body.pos[1].floor() as i32,
                player.0.body.pos[2].floor() as i32,
            ];
            let dx = h.block[0] - p_blk[0];
            let dy = h.block[1] - p_blk[1];
            let dz = h.block[2] - p_blk[2];
            let dist = ((dx * dx + dy * dy + dz * dz) as f32).sqrt();
            if let Some(ref mut ro) = ruler_readout {
                ro.0 = Some(([dx, dy, dz], dist));
            }
            let p_c = Vec3::new(
                p_blk[0] as f32 + 0.5,
                p_blk[1] as f32 + 0.5,
                p_blk[2] as f32 + 0.5,
            );
            let t_c = Vec3::new(
                h.block[0] as f32 + 0.5,
                h.block[1] as f32 + 0.5,
                h.block[2] as f32 + 0.5,
            );
            let p_x = Vec3::new(t_c.x, p_c.y, p_c.z);
            let p_xy = Vec3::new(t_c.x, t_c.y, p_c.z);
            gizmos.line(p_c, p_x, Color::srgba(1.0, 0.32, 0.32, 0.92));
            gizmos.line(p_x, p_xy, Color::srgba(0.32, 1.0, 0.45, 0.92));
            gizmos.line(p_xy, t_c, Color::srgba(0.35, 0.65, 1.0, 0.92));
            gizmos.line(p_c, t_c, Color::srgba(1.0, 0.88, 0.35, 0.75));
        } else if let Some(ref mut ro) = ruler_readout {
            ro.0 = None;
        }
    } else if let Some(ref mut ro) = ruler_readout
        && ro.0.is_some()
    {
        ro.0 = None;
    }

    if build_grid.is_some_and(|g| g.0)
        && let Some(h) = hit
    {
        let cell = preview.map(|(c, _, _)| c).unwrap_or([
            h.block[0] + h.normal[0],
            h.block[1] + h.normal[1],
            h.block[2] + h.normal[2],
        ]);
        let grid_col = Color::srgba(0.45, 0.92, 1.0, 0.42);
        let cell_c = Vec3::new(
            cell[0] as f32 + 0.5,
            cell[1] as f32 + 0.5,
            cell[2] as f32 + 0.5,
        );
        gizmos.cube(
            Transform::from_translation(cell_c).with_scale(Vec3::splat(1.004)),
            Color::srgba(0.95, 0.85, 0.30, 0.72),
        );
        // 3x3 1-block alignment grid centered on the targeted placement cell.
        let n = IVec3::from_array(h.normal);
        let (u, v) = if n.y != 0 {
            (Vec3::X, Vec3::Z)
        } else if n.x != 0 {
            (Vec3::Y, Vec3::Z)
        } else {
            (Vec3::X, Vec3::Y)
        };
        let face_offset = -Vec3::new(n.x as f32, n.y as f32, n.z as f32) * 0.495;
        let origin = cell_c + face_offset;
        for i in -1..=2 {
            let fi = i as f32 - 0.5;
            gizmos.line(
                origin + u * fi - v * 1.5,
                origin + u * fi + v * 1.5,
                grid_col,
            );
            gizmos.line(
                origin + v * fi - u * 1.5,
                origin + v * fi + u * 1.5,
                grid_col,
            );
        }
    }

    let Some(assets) = ghost_assets else {
        return;
    };
    let Ok((mut tf, mut mesh, mut mat, mut vis)) = ghost_q.single_mut() else {
        commands.spawn((
            PlacementGhost,
            Mesh3d(assets.cube_mesh.clone()),
            MeshMaterial3d(assets.valid_mat.clone()),
            Transform::IDENTITY,
            Visibility::Hidden,
            DespawnOnExit(AppState::InGame),
        ));
        return;
    };
    if let Some((cell, valid, shape)) = preview {
        let center = Vec3::new(cell[0] as f32, cell[1] as f32, cell[2] as f32) + Vec3::splat(0.5);
        tf.translation = match shape {
            PlacementShape::Torch => center - Vec3::Y * 0.14,
            _ => center,
        };
        mesh.0 = match shape {
            PlacementShape::Cube => assets.cube_mesh.clone(),
            PlacementShape::Wall => assets.wall_mesh.clone(),
            PlacementShape::Torch => assets.torch_mesh.clone(),
        };
        mat.0 = if valid {
            assets.valid_mat.clone()
        } else {
            assets.invalid_mat.clone()
        };
        *vis = Visibility::Inherited;
        if valid {
            gizmos.cube(
                Transform::from_translation(center).with_scale(Vec3::splat(1.002)),
                Color::srgba(0.65, 0.95, 1.0, 0.55),
            );
        }
    } else {
        *vis = Visibility::Hidden;
    }
}

fn pick<T: Clone>(v: &[T], rng: &mut u32) -> Option<T> {
    if v.is_empty() {
        return None;
    }
    *rng = rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    Some(v[(*rng >> 16) as usize % v.len()].clone())
}

/// WorldGen.tile break: stone, hard bricks, spikes, and ores ring (Tink), everything else digs.
fn kill_tink(tile: terravibe_sim::BlockId) -> bool {
    matches!(
        tile,
        STONE
            | IRON
            | COPPER
            | GOLD
            | SILVER
            | EBONSTONE
            | DEMONITE_ORE
            | HELLSTONE
            | OBSIDIAN
            | OBSIDIAN_BRICK
            | HELLSTONE_BRICK
            | BLUE_DUNGEON_BRICK
            | GREEN_DUNGEON_BRICK
            | PINK_DUNGEON_BRICK
            | SPIKES
            | HELLFORGE
    )
}

fn should_play_bow_draw_sfx(item: ItemId) -> bool {
    held_anim_kind(item) == HeldAnimKind::BowDraw
        && !matches!(
            item,
            MUSKET
                | FLINTLOCK_PISTOL
                | MINISHARK
                | HANDGUN
                | PHOENIX_BLASTER
                | CLOCKWORK_ASSAULT_RIFLE
        )
}

#[allow(clippy::too_many_arguments)]
fn on_use(
    mut commands: Commands,
    mut events: MessageReader<UseEvents>,
    audio: Res<UseAudio>,
    assets: Option<Res<CrackAssets>>,
    mut cracks: ResMut<Cracks>,
    mut swing: ResMut<Swing>,
    mut tracker: ResMut<HeldAnimTracker>,
    stream: Option<Res<ChunkStream>>,
    mut particles: Option<ResMut<ParticlePool>>,
    settings: Option<Res<Settings>>,
    focus: Option<Res<crate::audio::AudioFocusState>>,
    cam_tf: Option<Single<&GlobalTransform, With<FlyCamera>>>,
    time: Res<Time<Real>>,
    mut sfx: MessageWriter<SfxEvent>,
    mut rng: Local<u32>,
) {
    let now = time.elapsed_secs();
    let mut sounds: Vec<Handle<AudioSource>> = Vec::new();
    let mut spawn: Vec<([i32; 3], usize)> = Vec::new();
    let settings_val = settings.as_deref().copied().unwrap_or_default();
    for UseEvents(e) in events.read() {
        match e {
            UseEvent::Swing { item, prefix } => {
                swing.start = now;
                let spd = prefix_stats(*prefix).use_time;
                swing.secs = ((item_data(*item).use_animation as f32 * spd) / TICKS_PER_SEC)
                    .max(MIN_SWING_SECS);
                tracker.state.kick_t = 1.0;
                if should_play_bow_draw_sfx(*item) {
                    sfx.write(SfxEvent(SfxKind::Draw));
                }
                if let (Some((c1, c2, psize)), Some(pool), Some(cam)) = (
                    weapon_particle_tint(*item),
                    particles.as_deref_mut(),
                    cam_tf.as_deref(),
                ) {
                    let origin = cam.translation() + cam.forward() * 1.1;
                    let right = cam.right().as_vec3();
                    let up = cam.up().as_vec3();
                    for i in 0..6 {
                        let frac = (i as f32 / 5.0) - 0.5;
                        let pos = origin + right * (frac * 0.9) + up * (0.3 - frac.abs() * 0.5);
                        let vel = cam.forward() * 2.2 + right * (frac * 1.6) + Vec3::Y * 0.6;
                        let color = if i % 2 == 0 { c1 } else { c2 };
                        pool.spawn(pos, vel, color, psize, 0.32, 3.0, &settings_val);
                    }
                }
                sounds.extend(audio.swing.clone());
            }
            UseEvent::TileHit { pos, damage } => {
                sounds.extend(pick(&audio.dig, &mut rng));
                if let Some(stage) = crack_stage(*damage) {
                    spawn.push((*pos, stage));
                }
                if let Some(pool) = particles.as_deref_mut() {
                    let block = stream
                        .as_ref()
                        .map(|s| {
                            BlockQuery::block(&LoadedBlocks(s.generated()), pos[0], pos[1], pos[2])
                        })
                        .unwrap_or(STONE);
                    let color = particle_color(material_for_block(block));
                    let center = Vec3::new(
                        pos[0] as f32 + 0.5,
                        pos[1] as f32 + 0.5,
                        pos[2] as f32 + 0.5,
                    );
                    pool.spawn_burst(center, 5, color, 2.4, 0.08, 0.35, 11.0, &settings_val);
                }
            }
            UseEvent::TileBroken { pos, tile } => {
                let set = if kill_tink(*tile) {
                    &audio.tink
                } else {
                    &audio.dig
                };
                sounds.extend(pick(set, &mut rng));
                if let Some((en, _)) = cracks.0.remove(pos) {
                    commands.entity(en).despawn();
                }
                if let Some(pool) = particles.as_deref_mut() {
                    let color = particle_color(material_for_block(*tile));
                    let center = Vec3::new(
                        pos[0] as f32 + 0.5,
                        pos[1] as f32 + 0.5,
                        pos[2] as f32 + 0.5,
                    );
                    pool.spawn_burst(center, 10, color, 3.2, 0.10, 0.45, 11.0, &settings_val);
                }
            }
            UseEvent::Placed { .. } | UseEvent::WallPlaced { .. } => {
                tracker.state.kick_t = 1.0;
                sounds.extend(pick(&audio.dig, &mut rng))
            }
            UseEvent::Consume { .. } => {
                tracker.state.kick_t = 1.0;
            }
            UseEvent::Shoot { .. } | UseEvent::Cast { .. } => {
                tracker.state.kick_t = 1.0;
                sfx.write(SfxEvent(SfxKind::Release));
                if let (Some(pool), Some(cam)) = (particles.as_deref_mut(), cam_tf.as_deref()) {
                    let muzzle_pos = cam.translation() + cam.forward() * 0.65 + cam.right() * 0.22
                        - cam.up() * 0.15;
                    let (color, count, life) = if matches!(e, UseEvent::Cast { .. }) {
                        (Color::srgb(0.45, 0.85, 1.0), 6, 0.22)
                    } else {
                        (Color::srgb(1.0, 0.82, 0.35), 4, 0.15)
                    };
                    pool.spawn_burst(
                        muzzle_pos,
                        count,
                        color,
                        1.8,
                        0.06,
                        life,
                        0.0,
                        &settings_val,
                    );
                }
            }
            // Server-internal; never forwarded.
            UseEvent::ItemDrop { .. }
            | UseEvent::ShadowOrbBroken { .. }
            | UseEvent::Throw { .. }
            | UseEvent::Grapple { .. } => {}
        }
    }
    let sound_gain = if crate::audio::focus_muted(settings.as_deref(), focus.as_deref()) {
        0.0
    } else {
        crate::settings::volume_gain(settings_val.sound_volume)
    };
    if sound_gain > 0.0 {
        for h in sounds {
            commands.spawn((
                AudioPlayer(h),
                PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(sound_gain)),
            ));
        }
    }
    let Some(assets) = assets else { return };
    for (pos, stage) in spawn {
        let hash = pos[0].wrapping_mul(7) ^ pos[1].wrapping_mul(13) ^ pos[2].wrapping_mul(31);
        let variants = &assets.meshes[stage];
        let variant = hash.unsigned_abs() as usize % variants.len();
        if let Some((en, _)) = cracks.0.remove(&pos) {
            commands.entity(en).despawn();
        }
        let origin = Vec3::new(pos[0] as f32, pos[1] as f32, pos[2] as f32);
        let en = commands
            .spawn((
                Mesh3d(variants[variant].clone()),
                MeshMaterial3d(assets.material.clone()),
                // Lifted off the block so the overlay clears the raised relief.
                Transform::from_translation(origin - Vec3::splat(CRACK_LIFT))
                    .with_scale(Vec3::splat(1.0 + 2.0 * CRACK_LIFT)),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        cracks.0.insert(pos, (en, now));
    }
}

/// Demo only: keeps cracks alive so a late screenshot still shows them.
#[derive(Resource)]
struct CrackFreeze;

fn expire_cracks(
    mut commands: Commands,
    mut cracks: ResMut<Cracks>,
    time: Res<Time<Real>>,
    freeze: Option<Res<CrackFreeze>>,
) {
    if freeze.is_some() {
        return;
    }
    let now = time.elapsed_secs();
    cracks.0.retain(|_, (en, t)| {
        let keep = now - *t < CRACK_TTL;
        if !keep {
            commands.entity(*en).despawn();
        }
        keep
    });
}

type HeldQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Transform,
        &'static mut Mesh3d,
        &'static mut MeshMaterial3d<StandardMaterial>,
        &'static mut Visibility,
    ),
    With<HeldItem>,
>;

/// Updates the shared [`HeldAnimTracker`] from player motion, HP, slot changes, and swing timing.
pub(crate) fn update_held_anim_tracker(
    player: Res<LocalPlayer>,
    inv: Option<Res<LocalInventory>>,
    swing: Res<Swing>,
    time: Res<Time<Real>>,
    mut tracker: ResMut<HeldAnimTracker>,
    mut sfx: MessageWriter<SfxEvent>,
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    let selected = inv
        .as_ref()
        .and_then(|i| i.0.selected_stack())
        .map(|s| s.id);
    if selected != tracker.last_item {
        tracker.last_item = selected;
        tracker.state.equip_t = 0.0;
        if selected.is_some() {
            sfx.write(SfxEvent(SfxKind::Equip));
        }
    }
    tracker.state.equip_t = (tracker.state.equip_t + dt * 6.0).min(1.0);
    tracker.state.idle_t += dt;

    let body = &player.0.body;
    let horiz_speed = Vec2::new(body.vel[0], body.vel[2]).length();
    if body.on_ground && horiz_speed > 0.02 {
        tracker.state.walk_phase = (tracker.state.walk_phase + dt * 10.0) % std::f32::consts::TAU;
    } else {
        tracker.state.walk_phase = 0.0;
    }
    tracker.state.vel_y += (body.vel[1] - tracker.state.vel_y) * (dt * 14.0).min(1.0);

    let life = player.0.vitals.life;
    if tracker.last_hp.is_some_and(|prev| life < prev) {
        tracker.state.recoil_t = 1.0;
    }
    tracker.last_hp = Some(life);
    tracker.state.recoil_t = (tracker.state.recoil_t - dt * 6.0).max(0.0);
    tracker.state.kick_t = (tracker.state.kick_t - dt * 8.0).max(0.0);

    tracker.state.use_t = compute_swing_use_t(now, swing.start, swing.secs);
}

/// Keeps the held-item model on the camera in sync with the selected slot and
/// drives per-class first-person animations.
#[allow(clippy::too_many_arguments)]
fn held_item(
    mut commands: Commands,
    cam: Single<Entity, With<FlyCamera>>,
    mut held: HeldQuery,
    inv: Option<Res<LocalInventory>>,
    tracker: Res<HeldAnimTracker>,
    cmode: Res<CameraMode>,
    models: Option<Res<ItemModels>>,
    time: Res<Time<Real>>,
    mut particles: Option<ResMut<ParticlePool>>,
    settings: Option<Res<Settings>>,
    cam_tf: Query<&GlobalTransform, With<FlyCamera>>,
    mut ember_accum: Local<f32>,
) {
    let Some(inv) = inv else { return };
    let Ok((mut tf, mut mesh, mut mat, mut vis)) = held.single_mut() else {
        commands.entity(*cam).with_children(|c| {
            c.spawn((
                HeldItem,
                Transform::from_translation(HELD_POS),
                Visibility::Hidden,
                Mesh3d::default(),
                MeshMaterial3d::<StandardMaterial>::default(),
                DespawnOnExit(AppState::InGame),
            ));
        });
        return;
    };
    let Some(stack) = inv.0.selected_stack() else {
        *vis = Visibility::Hidden;
        return;
    };
    let id = stack.id.0;
    // The avatar holds the item in third person.
    if *cmode == CameraMode::Third {
        *vis = Visibility::Hidden;
        return;
    }
    let Some(m) = models.as_ref().and_then(|all| all.map.get(&id)) else {
        *vis = Visibility::Hidden;
        return;
    };
    mesh.0 = m.fp.clone();
    mat.0 = m.material.clone();
    tf.scale = Vec3::splat(m.fp_scale);
    *vis = Visibility::Inherited;

    let kind = held_anim_kind(stack.id);
    let anim_tf = compute_fp_transform(kind, &tracker.state);
    tf.translation = anim_tf.translation;
    tf.rotation = anim_tf.rotation;

    // Subtle idle/held embers for elemental/magic/torch items
    *ember_accum += time.delta_secs();
    if *ember_accum >= 0.18 {
        *ember_accum = 0.0;
        let s = settings.as_deref().copied().unwrap_or_default();
        if !s.reduced_motion
            && let Some((c1, _, psize)) = weapon_particle_tint(stack.id)
            && let Some(ref mut pool) = particles
            && let Ok(ctf) = cam_tf.single()
        {
            let t = time.elapsed_secs();
            let pos = ctf.translation()
                + ctf.forward() * 0.9
                + ctf.right() * (0.35 + (t * 7.0).sin() * 0.08)
                + ctf.up() * (-0.20 + (t * 9.0).cos() * 0.06);
            pool.spawn(
                pos,
                Vec3::new(0.0, 0.7, 0.0),
                c1,
                psize * 0.75,
                0.35,
                -0.8,
                &s,
            );
        }
    }
}

/// `TERRAVIBE_DEMO=dig`: drives the normal input path to dig a block, then
/// lets the screenshot go.
fn demo(
    mut commands: Commands,
    mut input: ResMut<SampledInput>,
    mut cam: Single<(&mut FlyCamera, &mut Transform)>,
    player: Option<Res<LocalPlayer>>,
    time: Res<Time<Real>>,
    mut t0: Local<Option<f32>>,
) {
    if player.is_none() {
        return;
    }
    let now = time.elapsed_secs();
    let t = now - *t0.get_or_insert(now);
    let (c, tf) = &mut *cam;
    c.pitch = -0.55;
    tf.rotation = Quat::from_euler(EulerRot::YXZ, c.yaw, c.pitch, 0.0);
    input.input.yaw = c.yaw;
    input.input.pitch = c.pitch;
    let crack = std::env::var(ENV_DEMO).is_ok_and(|v| v == "crack");
    if crack && t > 5.8 {
        // Stop mid-dig and hold still so the crack overlay is alive at capture.
        input.input.use_held = false;
        commands.insert_resource(CrackFreeze);
        commands.remove_resource::<DemoPending>();
        return;
    }
    input.input.slot = 1;
    input.input.use_held = (5.0..6.5).contains(&t);
    if t > 9.0 {
        commands.remove_resource::<DemoPending>();
    }
}

/// Tunnel floor for the lighting demo: eight blocks below the surface at
/// spawn, well inside the stone layer.
fn tunnel_floor() -> i32 {
    super::surface_y(0, 0).unwrap_or(64) - 8
}

/// Where the tunnel demo camera sits: just inside the near end, looking down
/// the tunnel at the torch. The tunnel runs along +z so the torch sprite faces
/// the camera.
fn tunnel_pose(floor: i32) -> (Vec3, Quat) {
    (
        Vec3::new(0.5, floor as f32 + 1.6, 1.5),
        Quat::from_euler(EulerRot::YXZ, std::f32::consts::PI, -0.06, 0.0),
    )
}

/// True once every chunk the carved box touches has arrived.
fn tunnel_chunks_ready(stream: &ChunkStream) -> bool {
    let floor = tunnel_floor();
    let lo = ChunkPos::of_block(-2, floor - 1, -1);
    let hi = ChunkPos::of_block(2, floor + 3, 30);
    (lo.x..=hi.x).all(|x| {
        (lo.y..=hi.y)
            .all(|y| (lo.z..=hi.z).all(|z| stream.generated().contains_key(&ChunkPos { x, y, z })))
    })
}

/// Carves a sealed 30-block tunnel and puts a torch in its near half, through
/// the same `set_block` path the server uses, so meshing and relighting run.
fn carve_tunnel(stream: &mut ChunkStream) {
    let floor = tunnel_floor();
    for z in -1..=30 {
        for y in (floor - 1)..=(floor + 3) {
            for x in -2..=2 {
                let inside = (0..=29).contains(&z)
                    && (floor..=(floor + 2)).contains(&y)
                    && (-1..=1).contains(&x);
                stream.set_block([x, y, z], if inside { AIR } else { STONE });
            }
        }
    }
    stream.set_block([0, floor, 5], TORCH);
}

/// `TERRAVIBE_DEMO=tunnel`: waits for the chunks, carves the tunnel, holds the
/// camera inside it, then lets the screenshot take over. The torch is placed
/// through `set_block`, which queues relight tasks for the chunks around it.
fn tunnel_demo(
    mut commands: Commands,
    mut stream: ResMut<ChunkStream>,
    mut cam: Single<&mut Transform, With<FlyCamera>>,
    time: Res<Time<Real>>,
    mut start: Local<Option<f32>>,
    mut carved: Local<Option<f32>>,
    welcomed: Option<Res<LocalPlayer>>,
) {
    // The spawn column (hence the tunnel depth) is only known after Welcome.
    if welcomed.is_none() {
        return;
    }
    let now = time.elapsed_secs();
    let t0 = *start.get_or_insert(now);
    let (pos, rot) = tunnel_pose(tunnel_floor());
    cam.translation = pos;
    cam.rotation = rot;

    if let Some(at) = *carved {
        if now - at > 1.0 {
            commands.remove_resource::<DemoPending>();
        }
        return;
    }
    // Carve once the box is loaded; after 8 s use whatever arrived rather
    // than leaving the screenshot pending forever.
    if !tunnel_chunks_ready(&stream) && now - t0 < 8.0 {
        return;
    }
    carve_tunnel(&mut stream);
    *carved = Some(now);
}

/// Nearest (to the spawn column) life crystal, else ore block, in the generated chunks.
use terravibe_sim::CHUNK_SIZE as CS;

fn find_vein_target(stream: &ChunkStream) -> Option<[i32; 3]> {
    let is_ore = |b| [COPPER, IRON, SILVER, GOLD].contains(&b);
    let mut best: Option<(bool, i64, [i32; 3])> = None;
    for (cp, chunk) in stream.generated() {
        for (lx, ly, lz) in
            (0..CS).flat_map(|x| (0..CS).flat_map(move |y| (0..CS).map(move |z| (x, y, z))))
        {
            let b = chunk.get(lx as usize, ly as usize, lz as usize);
            if b != LIFE_CRYSTAL && !is_ore(b) {
                continue;
            }
            let p = [cp.x * CS + lx, cp.y * CS + ly as i32, cp.z * CS + lz as i32];
            let d = (p[0] as i64).pow(2) + (p[2] as i64).pow(2);
            let key = (b != LIFE_CRYSTAL, d, p);
            if best.is_none_or(|k| (key.0, key.1) < (k.0, k.1)) {
                best = Some(key);
            }
        }
    }
    best.map(|k| k.2)
}

/// Carves a 3x3 shaft along +z ending at `t`, keeping ore and crystal cells.
fn carve_to_vein(stream: &mut ChunkStream, t: [i32; 3]) {
    for z in (t[2] - 9)..t[2] {
        for y in (t[1] - 1)..=(t[1] + 1) {
            for x in (t[0] - 1)..=(t[0] + 1) {
                let cp = ChunkPos::of_block(x, y, z);
                let keep = stream.generated().get(&cp).is_some_and(|c| {
                    let b = c.get(
                        x.rem_euclid(CS) as usize,
                        y.rem_euclid(CS) as usize,
                        z.rem_euclid(CS) as usize,
                    );
                    [COPPER, IRON, SILVER, GOLD, LIFE_CRYSTAL].contains(&b)
                });
                if !keep {
                    stream.set_block([x, y, z], AIR);
                }
            }
        }
    }
    stream.set_block([t[0], t[1] - 1, t[2] - 2], TORCH);
}

/// `TERRAVIBE_DEMO=progression`: finds the nearest life crystal or ore vein near
/// spawn, carves a shaft to it and points the camera at it (screenshot hook).
fn progression_demo(
    mut commands: Commands,
    mut stream: ResMut<ChunkStream>,
    mut cam: Single<&mut Transform, With<FlyCamera>>,
    time: Res<Time<Real>>,
    mut start: Local<Option<f32>>,
    mut done: Local<Option<f32>>,
) {
    let now = time.elapsed_secs();
    let t0 = *start.get_or_insert(now);
    if let Some(at) = *done {
        if now - at > 1.5 {
            commands.remove_resource::<DemoPending>();
        }
        return;
    }
    if now - t0 < 4.0 {
        return;
    }
    let target = find_vein_target(&stream);
    if target.is_none() && now - t0 < 10.0 {
        return;
    }
    let t = target.unwrap_or([0, tunnel_floor(), 12]);
    carve_to_vein(&mut stream, t);
    cam.translation = Vec3::new(t[0] as f32 + 0.5, t[1] as f32 + 0.5, t[2] as f32 - 4.5);
    cam.rotation = Quat::from_euler(EulerRot::YXZ, std::f32::consts::PI, 0.0, 0.0);
    info!("progression demo target {t:?} (found={})", target.is_some());
    *done = Some(now);
}

#[cfg(test)]
mod tests {
    use super::*;
    use terravibe_sim::{DIRT, Liquid};

    struct Slab(i32);

    impl BlockQuery for Slab {
        fn block(&self, x: i32, y: i32, z: i32) -> terravibe_sim::BlockId {
            if (x, y, z) == (self.0, 0, 0) {
                DIRT
            } else {
                AIR
            }
        }
    }

    #[test]
    fn target_rejected_beyond_reach() {
        // Camera on the player's eye line looking down +x.
        let body = PlayerBody::new([0.5, 0.0, 0.5]);
        let cam = Transform::from_xyz(0.5, 0.9, 0.5).looking_to(Vec3::X, Vec3::Y);
        assert!(target_block(&Slab(2), &cam, &body, 0.0, None).is_some());
        assert!(target_block(&Slab(7), &cam, &body, 0.0, None).is_none());
        // A camera pulled back along the same line still reaches the near block.
        let back = Transform::from_xyz(-3.5, 0.9, 0.5).looking_to(Vec3::X, Vec3::Y);
        assert!(target_block(&Slab(2), &back, &body, 4.0, None).is_some());
        assert!(target_block(&Slab(7), &back, &body, 4.0, None).is_none());
    }

    struct LiquidSlab(i32);

    impl BlockQuery for LiquidSlab {
        fn block(&self, _x: i32, _y: i32, _z: i32) -> terravibe_sim::BlockId {
            AIR
        }
        fn liquid(&self, x: i32, y: i32, z: i32) -> Option<Liquid> {
            if (x, y, z) == (self.0, 0, 0) {
                Some(Liquid::water(255))
            } else {
                None
            }
        }
    }

    #[test]
    fn raycast_picking_liquid_only_with_bucket() {
        let body = PlayerBody::new([0.5, 0.0, 0.5]);
        let cam = Transform::from_xyz(0.5, 0.0, 0.5).looking_to(Vec3::X, Vec3::Y);
        let slab = LiquidSlab(2);

        // Without held item or non-bucket item: ignores liquid
        assert!(target_block(&slab, &cam, &body, 0.0, None).is_none());
        assert!(target_block(&slab, &cam, &body, 0.0, Some(terravibe_sim::item::WOOD)).is_none());

        // With bucket items: hits liquid
        assert!(target_block(&slab, &cam, &body, 0.0, Some(EMPTY_BUCKET)).is_some());
        assert!(target_block(&slab, &cam, &body, 0.0, Some(WATER_BUCKET)).is_some());
        assert!(target_block(&slab, &cam, &body, 0.0, Some(LAVA_BUCKET)).is_some());
    }

    #[test]
    fn crack_sheet_layout_is_variant_column_stage_row() {
        // Main.tile crack rendering: Rectangle(crackStyle * 18, stage * 18, 16, 16).
        assert_eq!(crack_origin(2, 3), (54.0, 36.0));
        assert_eq!(crack_origin(3, 0), (0.0, 54.0));
        assert_eq!(crack_stage(80), Some(3));
    }

    #[test]
    fn crack_stage_for_damage() {
        // TileDrawing crack overlay: nothing below 20 damage, then one frame
        // (0..=3) per 20 points.
        assert_eq!(crack_stage(0), None);
        assert_eq!(crack_stage(19), None);
        assert_eq!(crack_stage(20), Some(0));
        assert_eq!(crack_stage(45), Some(1));
        assert_eq!(crack_stage(60), Some(2));
        assert_eq!(crack_stage(99), Some(3));
    }

    #[test]
    fn kill_tink_covers_late_game_ores_and_bricks_and_firearms_skip_bow_draw_sfx() {
        use terravibe_sim::DIRT;
        use terravibe_sim::item::{DEMON_BOW, MOLTEN_FURY, WOODEN_BOW};

        for hard_tile in [
            STONE,
            IRON,
            COPPER,
            GOLD,
            SILVER,
            EBONSTONE,
            DEMONITE_ORE,
            HELLSTONE,
            OBSIDIAN,
            OBSIDIAN_BRICK,
            HELLSTONE_BRICK,
            BLUE_DUNGEON_BRICK,
            GREEN_DUNGEON_BRICK,
            PINK_DUNGEON_BRICK,
            SPIKES,
            HELLFORGE,
        ] {
            assert!(kill_tink(hard_tile), "{hard_tile:?} should play tink sound");
        }
        assert!(!kill_tink(DIRT));

        assert!(should_play_bow_draw_sfx(WOODEN_BOW));
        assert!(should_play_bow_draw_sfx(DEMON_BOW));
        assert!(should_play_bow_draw_sfx(MOLTEN_FURY));
        assert!(!should_play_bow_draw_sfx(MUSKET));
        assert!(!should_play_bow_draw_sfx(FLINTLOCK_PISTOL));
        assert!(!should_play_bow_draw_sfx(MINISHARK));
        assert!(!should_play_bow_draw_sfx(HANDGUN));
        assert!(!should_play_bow_draw_sfx(PHOENIX_BLASTER));
        assert!(!should_play_bow_draw_sfx(CLOCKWORK_ASSAULT_RIFLE));
    }

    #[test]
    fn swing_use_t_progresses_continuously_and_floors_zero_animation() {
        // Even with secs == 0.0, MIN_SWING_SECS prevents immediate t >= 1.0.
        let t0 = compute_swing_use_t(10.0, 10.0, 0.0);
        assert_eq!(t0, Some(0.0));
        let t1 = compute_swing_use_t(10.0 + MIN_SWING_SECS * 0.5, 10.0, 0.0);
        assert!(t1.is_some_and(|v| v > 0.4 && v < 0.6));
        assert_eq!(
            compute_swing_use_t(10.0 + MIN_SWING_SECS + 0.01, 10.0, 0.0),
            None
        );

        // Continuous swings reset start and keep progressing without getting stuck.
        let second = compute_swing_use_t(10.25, 10.20, 0.25);
        assert!(second.is_some_and(|v| (v - 0.2).abs() < 1e-4));
    }

    #[test]
    fn placement_preview_cell_handles_blocks_walls_torches_and_overlap() {
        struct FloorWorld;
        impl BlockQuery for FloorWorld {
            fn block(&self, _x: i32, y: i32, _z: i32) -> terravibe_sim::BlockId {
                if y <= 0 { terravibe_sim::DIRT } else { AIR }
            }
        }
        let body = PlayerBody::new([5.5, 1.0, 5.5]);
        let hit = Hit {
            block: [3, 0, 5],
            normal: [0, 1, 0],
        };
        assert_eq!(
            placement_preview_cell(&FloorWorld, &body, hit, terravibe_sim::item::DIRT_BLOCK),
            Some(([3, 1, 5], true, PlacementShape::Cube))
        );
        assert_eq!(
            placement_preview_cell(&FloorWorld, &body, hit, terravibe_sim::item::TORCH),
            Some(([3, 1, 5], true, PlacementShape::Torch))
        );
        assert_eq!(
            placement_preview_cell(&FloorWorld, &body, hit, terravibe_sim::item::WOOD_WALL),
            Some(([3, 1, 5], true, PlacementShape::Wall))
        );
        // Overlapping the player invalidates solid block placement.
        let self_hit = Hit {
            block: [5, 0, 5],
            normal: [0, 1, 0],
        };
        assert_eq!(
            placement_preview_cell(
                &FloorWorld,
                &body,
                self_hit,
                terravibe_sim::item::DIRT_BLOCK
            ),
            Some(([5, 1, 5], false, PlacementShape::Cube))
        );
        // Non-placeable tool produces no preview.
        assert_eq!(
            placement_preview_cell(&FloorWorld, &body, hit, terravibe_sim::item::COPPER_PICKAXE),
            None
        );
    }
}
