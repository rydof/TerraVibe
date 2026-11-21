//! World items: the item's voxel model (half size, spinning) or, without one, a
//! flat icon card, bobbing above the spot the drop settled on. `DropSpawn` is sent once, so the client replays the sim's
//! fall locally (`tick_drop` on the loaded chunks) until the drop rests.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use bevy::prelude::*;
use terravibe_assets::RgbaImage;
use terravibe_sim::ChunkPos;
use terravibe_sim::drop::{Drop as SimDrop, DropId, tick_drop};
use terravibe_sim::inventory::ItemStack;
use terravibe_sim::item::ItemId;

use super::avatar::ItemModels;
use super::player::{LoadedBlocks, LocalPlayer};
use super::to_bevy_image;
use crate::session::AppState;
use crate::view::chunks::ChunkStream;

/// Icons draw at native size: 16 sprite pixels per block, like a tile.
const PX_SIZE: f32 = 1.0 / 16.0;
const BOB_AMPLITUDE: f32 = 0.1;
const BOB_RATE: f32 = 4.0;
/// Item models are drawn at half their held size.
const MODEL_SCALE: f32 = 0.5;
/// Spin speed (rad/s) and the height of a model's centre above the drop spot.
const SPIN_RATE: f32 = 2.0;
const MODEL_LIFT: f32 = 0.3;

/// How a drop is drawn.
#[derive(Debug, PartialEq)]
pub(crate) enum DropVisual {
    /// The item's `.vox` model.
    Model(PathBuf),
    /// A 3D block mesh (cube or placed block `.vox`).
    Block(terravibe_sim::BlockId),
    /// A quad textured with the 2D icon: fallback only for unknown items.
    Icon,
}

pub(crate) fn drop_visual(item: ItemId) -> DropVisual {
    if !terravibe_sim::item::item_exists(item) {
        return DropVisual::Icon;
    }
    match super::avatar::held_visual_for_item(item) {
        super::avatar::HeldVisualKind::Vox { file, .. } => {
            DropVisual::Model(crate::art::art_dir().join("models/items").join(file))
        }
        super::avatar::HeldVisualKind::Block(block) => DropVisual::Block(block),
    }
}

/// Spins a drop's model about the vertical axis with a per-drop phase offset.
#[derive(Component)]
struct DropSpin(f32);

/// Magnetic pickup glide toward the local player before despawning.
#[derive(Component)]
pub struct PickupGlide {
    pub start: Vec3,
    pub elapsed: f32,
    pub duration: f32,
}

const PICKUP_GLIDE_SECS: f32 = 0.14;
const PICKUP_GLIDE_MAX_DIST: f32 = 8.0;
const MAGNET_RANGE: f32 = 3.2;

/// Computes `(position, scale)` for a drop gliding magnetically into the player.
pub(crate) fn pickup_glide_pose(start: Vec3, target: Vec3, t: f32) -> (Vec3, f32) {
    let t = t.clamp(0.0, 1.0);
    let ease = t * t;
    (start.lerp(target, ease), (1.0 - 0.65 * ease).max(0.2))
}

/// Server-to-view drop events, written by `net::pump` (pickup = despawn).
#[derive(Message, Clone, Copy, Debug)]
pub enum DropMsg {
    Spawned {
        id: DropId,
        stack: ItemStack,
        pos: Vec3,
    },
    Despawned {
        id: DropId,
    },
}

pub struct DropsPlugin {
    pub icons: Arc<HashMap<u16, RgbaImage>>,
}

#[derive(Component)]
#[allow(dead_code)] // identifies the entity in debug output
pub struct ClientDrop {
    pub id: DropId,
    pub stack: ItemStack,
}

#[derive(Component)]
pub struct DropBob {
    pub base: Vec3,
    pub phase: f32,
}

/// Local copy of the falling drop, ticked at 60 Hz until it rests.
#[derive(Component)]
struct DropBody(SimDrop);

/// Rendered position of a bobbing drop at `t` seconds.
pub(crate) fn bob_pos(b: &DropBob, t: f32) -> Vec3 {
    b.base + Vec3::Y * BOB_AMPLITUDE * (BOB_RATE * t + b.phase).sin()
}

/// Blocks per icon pixel.
pub(crate) fn icon_px_size() -> f32 {
    PX_SIZE
}

/// Shared mesh and material per item id.
#[derive(Resource)]
pub(crate) struct DropVisuals {
    icons: Arc<HashMap<u16, RgbaImage>>,
    cache: HashMap<u16, (Handle<Mesh>, Handle<StandardMaterial>)>,
    warned: HashSet<u16>,
}

impl DropVisuals {
    fn new(icons: Arc<HashMap<u16, RgbaImage>>) -> Self {
        Self {
            icons,
            cache: HashMap::new(),
            warned: HashSet::new(),
        }
    }

    /// `None` (warning once per id) when the icon is missing or empty.
    pub(crate) fn get(
        &mut self,
        id: u16,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
        images: &mut Assets<Image>,
    ) -> Option<(Handle<Mesh>, Handle<StandardMaterial>)> {
        if let Some(c) = self.cache.get(&id) {
            return Some(c.clone());
        }
        let built = self.icons.get(&id).and_then(|img| {
            let (w, h) = (
                img.width as f32 * icon_px_size(),
                img.height as f32 * icon_px_size(),
            );
            // Bottom edge at y = 0, facing +z; 3D slab one voxel thick that spins.
            let mesh =
                Mesh::from(Cuboid::new(w, h, icon_px_size())).translated_by(Vec3::Y * h / 2.0);
            let emissive = if id == 75 {
                LinearRgba::rgb(2.0, 2.0, 0.5)
            } else {
                LinearRgba::NONE
            };
            let material = materials.add(StandardMaterial {
                base_color_texture: Some(images.add(to_bevy_image(img))),
                emissive: emissive.into(),
                alpha_mode: AlphaMode::Mask(0.5),
                perceptual_roughness: 1.0,
                cull_mode: None,
                double_sided: true,
                ..default()
            });
            Some((meshes.add(mesh), material))
        });
        match built {
            Some(c) => {
                self.cache.insert(id, c.clone());
                Some(c)
            }
            None => {
                if self.warned.insert(id) {
                    warn!("no drop icon for item {id}");
                }
                None
            }
        }
    }
}

#[derive(Resource, Default)]
struct DropEntities(HashMap<DropId, Entity>);

impl Plugin for DropsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DropVisuals::new(self.icons.clone()))
            .init_resource::<DropEntities>()
            .add_message::<DropMsg>()
            .add_systems(
                OnExit(AppState::InGame),
                |mut ents: ResMut<DropEntities>| {
                    ents.0.clear();
                },
            )
            .add_systems(
                Update,
                (apply_msgs, bob, spin, glide_pickups)
                    .chain()
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(FixedUpdate, fall.run_if(in_state(AppState::InGame)));
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_msgs(
    mut commands: Commands,
    mut msgs: MessageReader<DropMsg>,
    mut visuals: ResMut<DropVisuals>,
    models: Option<Res<ItemModels>>,
    mut ents: ResMut<DropEntities>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    player: Option<Res<LocalPlayer>>,
    tf_q: Query<&Transform>,
) {
    for m in msgs.read() {
        match *m {
            DropMsg::Spawned { id, stack, pos } => {
                if let Some(old) = ents.0.remove(&id) {
                    commands.entity(old).despawn();
                }
                let model = match drop_visual(stack.id) {
                    DropVisual::Model(_) | DropVisual::Block(_) => models.as_ref().and_then(|m| {
                        m.map
                            .get(&stack.id.0)
                            .map(|i| (i.fp.clone(), i.material.clone()))
                    }),
                    DropVisual::Icon => None,
                };
                let (mesh, material, child) = match model {
                    Some((mesh, material)) => (
                        mesh,
                        material,
                        Transform::from_xyz(0.0, MODEL_LIFT, 0.0)
                            .with_scale(Vec3::splat(MODEL_SCALE)),
                    ),
                    None => {
                        let Some((mesh, material)) =
                            visuals.get(stack.id.0, &mut meshes, &mut materials, &mut images)
                        else {
                            continue;
                        };
                        (mesh, material, Transform::default())
                    }
                };
                let e = commands
                    .spawn((
                        ClientDrop { id, stack },
                        DropBob {
                            base: pos,
                            phase: id.0 as f32,
                        },
                        DropBody(SimDrop::new(id, stack, pos.to_array())),
                        Transform::from_translation(pos),
                        Visibility::default(),
                        DespawnOnExit(AppState::InGame),
                    ))
                    .with_children(|p| {
                        p.spawn((
                            Mesh3d(mesh),
                            MeshMaterial3d(material),
                            child,
                            DropSpin(id.0 as f32 * 0.73),
                        ));
                    })
                    .id();
                ents.0.insert(id, e);
            }
            DropMsg::Despawned { id } => {
                if let Some(e) = ents.0.remove(&id) {
                    let near_player =
                        player
                            .as_ref()
                            .zip(tf_q.get(e).ok())
                            .is_some_and(|(p, tf)| {
                                let ppos = Vec3::from(p.0.body.pos) + Vec3::Y * 0.9;
                                tf.translation.distance_squared(ppos)
                                    <= PICKUP_GLIDE_MAX_DIST * PICKUP_GLIDE_MAX_DIST
                            });
                    if near_player && let Ok(tf) = tf_q.get(e) {
                        commands
                            .entity(e)
                            .remove::<(DropBob, DropBody)>()
                            .insert(PickupGlide {
                                start: tf.translation,
                                elapsed: 0.0,
                                duration: PICKUP_GLIDE_SECS,
                            });
                    } else {
                        commands.entity(e).despawn();
                    }
                }
            }
        }
    }
}

/// Replays the sim fall; a drop over unloaded chunks waits (they read as solid).
fn fall(stream: Res<ChunkStream>, mut q: Query<(&mut DropBody, &mut DropBob)>) {
    let q_blocks = LoadedBlocks(stream.generated());
    for (mut body, mut bob) in &mut q {
        if body.0.on_ground {
            continue;
        }
        let p = body.0.pos;
        let cp = ChunkPos::of_block(
            p[0].floor() as i32,
            p[1].floor() as i32,
            p[2].floor() as i32,
        );
        if !stream.generated().contains_key(&cp) {
            continue;
        }
        tick_drop(&q_blocks, &mut body.0);
        bob.base = Vec3::from(body.0.pos);
    }
}

fn bob(
    time: Res<Time>,
    player: Option<Res<LocalPlayer>>,
    mut q: Query<(&DropBob, &mut Transform), Without<PickupGlide>>,
) {
    let t = time.elapsed_secs();
    let alpha = 1.0 - (-18.0 * time.delta_secs().max(0.0)).exp();
    let player_torso = player
        .as_ref()
        .map(|p| Vec3::from(p.0.body.pos) + Vec3::Y * 0.9);
    for (b, mut tf) in &mut q {
        let mut target = bob_pos(b, t);
        if let Some(ppos) = player_torso {
            let dist = target.distance(ppos);
            if dist < MAGNET_RANGE && dist > 1e-4 {
                let pull = ((MAGNET_RANGE - dist) / MAGNET_RANGE).powi(2) * 0.45;
                target = target.lerp(ppos, pull);
            }
        }
        tf.translation = tf.translation.lerp(target, alpha);
    }
}

fn spin(time: Res<Time>, mut q: Query<(&DropSpin, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (s, mut tf) in &mut q {
        let tilt = (t * 1.7 + s.0).sin() * 0.06;
        tf.rotation = Quat::from_rotation_y(SPIN_RATE * t + s.0) * Quat::from_rotation_z(tilt);
    }
}

fn glide_pickups(
    mut commands: Commands,
    time: Res<Time>,
    player: Option<Res<LocalPlayer>>,
    mut q: Query<(Entity, &mut PickupGlide, &mut Transform)>,
) {
    let dt = time.delta_secs().max(0.0);
    let target = player
        .as_ref()
        .map(|p| Vec3::from(p.0.body.pos) + Vec3::Y * 0.9);
    for (e, mut glide, mut tf) in &mut q {
        glide.elapsed += dt;
        let t = (glide.elapsed / glide.duration.max(1e-4)).clamp(0.0, 1.0);
        if let Some(dst) = target {
            let (pos, scale) = pickup_glide_pose(glide.start, dst, t);
            tf.translation = pos;
            tf.scale = Vec3::splat(scale);
        }
        if t >= 1.0 {
            commands.entity(e).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn icon() -> RgbaImage {
        RgbaImage {
            width: 2,
            height: 2,
            pixels: vec![[255, 0, 0, 255]; 4],
        }
    }

    #[test]
    fn drop_cache_reuses_mesh_per_item() {
        let mut v = DropVisuals::new(Arc::new(HashMap::from([(7, icon()), (8, icon())])));
        let (mut m, mut mat, mut im) = (Assets::default(), Assets::default(), Assets::default());
        let a = v.get(7, &mut m, &mut mat, &mut im).unwrap();
        let b = v.get(7, &mut m, &mut mat, &mut im).unwrap();
        let c = v.get(8, &mut m, &mut mat, &mut im).unwrap();
        assert_eq!(a.0, b.0);
        assert_eq!(a.1, b.1);
        assert_ne!(a.0, c.0);
        assert_eq!(m.len(), 2);
        assert!(v.get(99, &mut m, &mut mat, &mut im).is_none());
    }

    #[test]
    fn icon_drop_is_a_three_dimensional_volume() {
        // An icon with no model is extruded into a thin voxel slab that spins
        // like the models do; a flat card would have only 4 vertices.
        let mut v = DropVisuals::new(Arc::new(HashMap::from([(7, icon())])));
        let (mut m, mut mat, mut im) = (Assets::default(), Assets::default(), Assets::default());
        let (mesh, _) = v.get(7, &mut m, &mut mat, &mut im).unwrap();
        let count = m.get(&mesh).unwrap().count_vertices();
        assert!(
            count > 4 * 2,
            "icon drop has {count} vertices, a flat card has 4"
        );
    }

    #[test]
    fn unknown_item_uses_icon_card() {
        use terravibe_sim::item::{DIRT_BLOCK, WOODEN_ARROW};
        assert_eq!(drop_visual(ItemId(9999)), DropVisual::Icon);
        assert_eq!(
            drop_visual(DIRT_BLOCK),
            DropVisual::Block(terravibe_sim::DIRT)
        );
        assert!(matches!(drop_visual(WOODEN_ARROW), DropVisual::Model(_)));
    }

    #[test]
    fn drop_bob_stays_within_half_block() {
        let b = DropBob {
            base: Vec3::new(1.0, 10.0, 2.0),
            phase: 3.0,
        };
        for i in 0..1000 {
            let p = bob_pos(&b, i as f32 * 0.01);
            assert!((p.y - 10.0).abs() <= 0.5);
            assert_eq!((p.x, p.z), (1.0, 2.0));
        }
    }

    #[test]
    fn drop_icon_scale_matches_tile() {
        // A 16 px tile is one block, so an icon pixel is 1/16 block.
        assert_eq!(16.0 * icon_px_size(), 1.0);
    }

    #[test]
    fn wood_and_gel_drops_have_3d_models() {
        use terravibe_sim::item::{GEL, WOOD};
        assert!(matches!(drop_visual(WOOD), DropVisual::Model(_)));
        assert!(matches!(drop_visual(GEL), DropVisual::Model(_)));
    }

    #[test]
    fn all_m6_items_have_3d_vox_models() {
        use terravibe_sim::item::{
            BAND_OF_REGENERATION, CLOUD_IN_A_BOTTLE, ENCHANTED_BOOMERANG, HERMES_BOOTS,
            LUCKY_HORSESHOE, MAGIC_MIRROR, SHINY_RED_BALLOON, SPEAR,
        };
        let m6_items = [
            BAND_OF_REGENERATION,
            MAGIC_MIRROR,
            CLOUD_IN_A_BOTTLE,
            HERMES_BOOTS,
            ENCHANTED_BOOMERANG,
            LUCKY_HORSESHOE,
            SHINY_RED_BALLOON,
            SPEAR,
        ];
        for item in m6_items {
            assert!(
                matches!(drop_visual(item), DropVisual::Model(_)),
                "item {:?} ({}) should have a 3D drop model",
                terravibe_sim::item::item_data(item).name,
                item.0
            );
        }
    }

    #[test]
    fn all_m7_items_have_3d_vox_models() {
        use terravibe_sim::item::{
            BALL_O_HURT, BAND_OF_STARPOWER, DEMONITE_BAR, DEMONITE_ORE_ITEM, MUSKET, MUSKET_BALL,
            SHADOW_GREAVES, SHADOW_HELMET, SHADOW_ORB_ITEM, SHADOW_SCALE, SHADOW_SCALEMAIL,
            THE_BREAKER, VILETHORN, WOODEN_HAMMER, WORM_FOOD,
        };
        let m7_items = [
            DEMONITE_ORE_ITEM,
            DEMONITE_BAR,
            VILETHORN,
            WORM_FOOD,
            SHADOW_SCALE,
            MUSKET,
            MUSKET_BALL,
            SHADOW_GREAVES,
            SHADOW_SCALEMAIL,
            SHADOW_HELMET,
            THE_BREAKER,
            BAND_OF_STARPOWER,
            SHADOW_ORB_ITEM,
            BALL_O_HURT,
            WOODEN_HAMMER,
        ];
        for item in m7_items {
            assert!(
                matches!(drop_visual(item), DropVisual::Model(_)),
                "item {:?} ({}) should have a 3D drop model",
                terravibe_sim::item::item_data(item).name,
                item.0
            );
        }
    }

    #[test]
    fn all_m8_items_have_3d_vox_models() {
        use terravibe_sim::item::{
            AQUA_SCEPTER, BLUE_MOON, BONE, COBALT_SHIELD, GOLDEN_KEY, HANDGUN, MAGIC_MISSILE,
            MURAMASA, SHADOW_KEY, VALOR,
        };
        let m8_items = [
            MAGIC_MISSILE,
            BONE,
            MURAMASA,
            COBALT_SHIELD,
            AQUA_SCEPTER,
            BLUE_MOON,
            HANDGUN,
            GOLDEN_KEY,
            SHADOW_KEY,
            VALOR,
        ];
        for item in m8_items {
            assert!(
                matches!(drop_visual(item), DropVisual::Model(_)),
                "item {:?} ({}) should have a 3D drop model",
                terravibe_sim::item::item_data(item).name,
                item.0
            );
        }
    }

    #[test]
    fn arms_dealer_guns_have_3d_vox_models() {
        use terravibe_sim::item::{FLINTLOCK_PISTOL, MINISHARK};
        for item in [FLINTLOCK_PISTOL, MINISHARK] {
            match drop_visual(item) {
                DropVisual::Model(path) => assert!(
                    path.is_file(),
                    "item {:?} ({}) maps to missing model {}",
                    terravibe_sim::item::item_data(item).name,
                    item.0,
                    path.display()
                ),
                other => panic!(
                    "item {:?} ({}) should have a 3D drop model, got {other:?}",
                    terravibe_sim::item::item_data(item).name,
                    item.0
                ),
            }
        }
    }

    #[test]
    fn all_m9_items_have_3d_vox_models() {
        use terravibe_sim::item::{
            ASH_BLOCK, GUIDE_VOODOO_DOLL, HELLFORGE_ITEM, HELLSTONE_BRICK_ITEM, HELLSTONE_ITEM,
            LAVA_CHARM, OBSIDIAN_BRICK_ITEM, OBSIDIAN_ITEM, OBSIDIAN_SKIN_POTION, OBSIDIAN_SKULL,
        };
        let m9_items = [
            ASH_BLOCK,
            OBSIDIAN_ITEM,
            HELLSTONE_ITEM,
            OBSIDIAN_BRICK_ITEM,
            OBSIDIAN_SKULL,
            HELLSTONE_BRICK_ITEM,
            HELLFORGE_ITEM,
            GUIDE_VOODOO_DOLL,
            OBSIDIAN_SKIN_POTION,
            LAVA_CHARM,
        ];
        for item in m9_items {
            assert!(
                matches!(drop_visual(item), DropVisual::Model(_)),
                "item {:?} ({}) should have a 3D drop model",
                terravibe_sim::item::item_data(item).name,
                item.0
            );
        }
    }

    #[test]
    fn all_m10_items_have_3d_vox_models() {
        use terravibe_sim::item::{
            FIERY_GREATSWORD, FLAMARANG, HELLSTONE_BAR, MOLTEN_BREASTPLATE, MOLTEN_FURY,
            MOLTEN_GREAVES, MOLTEN_HAMAXE, MOLTEN_HELMET, MOLTEN_PICKAXE, PHOENIX_BLASTER,
        };
        let m10_items = [
            FLAMARANG,
            MOLTEN_FURY,
            FIERY_GREATSWORD,
            MOLTEN_PICKAXE,
            HELLSTONE_BAR,
            MOLTEN_HAMAXE,
            PHOENIX_BLASTER,
            MOLTEN_HELMET,
            MOLTEN_BREASTPLATE,
            MOLTEN_GREAVES,
        ];
        for item in m10_items {
            match drop_visual(item) {
                DropVisual::Model(path) => assert!(
                    path.is_file(),
                    "item {:?} ({}) maps to missing model {}",
                    terravibe_sim::item::item_data(item).name,
                    item.0,
                    path.display()
                ),
                other => panic!(
                    "item {:?} ({}) should have a 3D drop model, got {other:?}",
                    terravibe_sim::item::item_data(item).name,
                    item.0
                ),
            }
        }
    }

    #[test]
    fn all_m11_items_have_3d_vox_models() {
        use terravibe_sim::item::{
            BREAKER_BLADE, CLOCKWORK_ASSAULT_RIFLE, FIRECRACKER, LASER_RIFLE, PWNHAMMER,
            RANGER_EMBLEM, SORCERER_EMBLEM, WARRIOR_EMBLEM,
        };
        let m11_items = [
            PWNHAMMER,
            BREAKER_BLADE,
            CLOCKWORK_ASSAULT_RIFLE,
            SORCERER_EMBLEM,
            WARRIOR_EMBLEM,
            RANGER_EMBLEM,
            LASER_RIFLE,
            FIRECRACKER,
        ];
        for item in m11_items {
            match drop_visual(item) {
                DropVisual::Model(path) => assert!(
                    path.is_file(),
                    "item {:?} ({}) maps to missing model {}",
                    terravibe_sim::item::item_data(item).name,
                    item.0,
                    path.display()
                ),
                other => panic!(
                    "item {:?} ({}) should have a 3D drop model, got {other:?}",
                    terravibe_sim::item::item_data(item).name,
                    item.0
                ),
            }
        }
    }

    #[test]
    fn pickup_glide_eases_toward_player_and_shrinks() {
        let start = Vec3::new(0.0, 0.0, 0.0);
        let target = Vec3::new(4.0, 2.0, 0.0);
        let (p0, s0) = pickup_glide_pose(start, target, 0.0);
        let (p_mid, s_mid) = pickup_glide_pose(start, target, 0.5);
        let (p1, s1) = pickup_glide_pose(start, target, 1.0);
        assert_eq!(p0, start);
        assert_eq!(s0, 1.0);
        assert!(
            p_mid.x > 0.0 && p_mid.x < 2.0,
            "ease-in at t=0.5: {p_mid:?}"
        );
        assert!(s_mid < 1.0 && s_mid > s1);
        assert_eq!(p1, target);
        assert!(s1 < 0.5);
    }

    #[test]
    fn critter_and_bug_net_items_have_3d_vox_models() {
        use terravibe_sim::item::{
            BIRD_ITEM, BUG_NET, BUNNY_ITEM, CAVE_CRITTER_ITEM, SQUIRREL_ITEM,
        };
        for item in [
            BUG_NET,
            BUNNY_ITEM,
            BIRD_ITEM,
            SQUIRREL_ITEM,
            CAVE_CRITTER_ITEM,
        ] {
            match drop_visual(item) {
                DropVisual::Model(path) => assert!(
                    path.is_file(),
                    "item {:?} ({}) maps to missing model {}",
                    terravibe_sim::item::item_data(item).name,
                    item.0,
                    path.display()
                ),
                other => panic!(
                    "item {:?} ({}) should have a 3D drop model, got {other:?}",
                    terravibe_sim::item::item_data(item).name,
                    item.0
                ),
            }
        }
    }
}
