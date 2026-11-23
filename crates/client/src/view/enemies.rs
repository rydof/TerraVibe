//! Streamed enemies: one shared extruded mesh per sprite frame, eased between
//! the server's 10 Hz snapshots.

use std::collections::HashMap;
use std::path::PathBuf;

use bevy::prelude::*;
use terravibe_sim::enemy::{EnemyId, EnemyKind};

use crate::art::art_dir;
use crate::art::rig::{self, Rig};
use crate::art::rig_view::{MeshCache, RigAnim, spawn_rig_with};
use crate::session::AppState;

/// Slime tint (RGB is the blue slime): `NPC.item defaults` type 1, `color = new Color(0, 80, 255, 100)` (gameplay spec).
const SLIME_TINT: Color = Color::srgba(0.45, 0.62, 1.0, 1.0);
/// Hurt flash colour and length (frames).
const HURT_TINT: Color = Color::srgb(1.0, 0.35, 0.35);
const HURT_FRAMES: u8 = 6;
/// Wind-up telegraph flash duration (seconds) and colour.
const TELEGRAPH_SECS: f32 = 0.25;
const TELEGRAPH_TINT: Color = Color::srgb(1.0, 0.85, 0.25);
/// Contact-enemy approach flash distance (blocks).
const APPROACH_FLASH_DIST: f32 = 3.5;
/// Seconds between `EnemyState` snapshots.
const SNAPSHOT_SECS: f32 = 0.1;
/// Horizontal travel below this (blocks) does not turn the enemy.
const TURN_MIN: f32 = 0.005;
/// Travel between snapshots (blocks) that counts as moving.
const MOVE_MIN: f32 = 0.02;

pub(crate) const ALL_ENEMY_KINDS: [EnemyKind; 63] = [
    EnemyKind::Slime,
    EnemyKind::DemonEye,
    EnemyKind::EaterOfSouls,
    EnemyKind::Zombie,
    EnemyKind::EyeOfCthulhu,
    EnemyKind::ServantOfCthulhu,
    EnemyKind::Guide,
    EnemyKind::Merchant,
    EnemyKind::ArmsDealer,
    EnemyKind::Nurse,
    EnemyKind::Demolitionist,
    EnemyKind::Dryad,
    EnemyKind::GoblinTinkerer,
    EnemyKind::Skeleton,
    EnemyKind::CaveBat,
    EnemyKind::GiantWorm,
    EnemyKind::EaterOfWorldsHead,
    EnemyKind::EaterOfWorldsBody,
    EnemyKind::EaterOfWorldsTail,
    EnemyKind::OldMan,
    EnemyKind::DungeonGuardian,
    EnemyKind::SkeletronHead,
    EnemyKind::SkeletronHand,
    EnemyKind::AngryBones,
    EnemyKind::DarkCaster,
    EnemyKind::CursedSkull,
    EnemyKind::FireImp,
    EnemyKind::BoneSerpentHead,
    EnemyKind::BoneSerpentBody,
    EnemyKind::BoneSerpentTail,
    EnemyKind::LavaSlime,
    EnemyKind::Hellbat,
    EnemyKind::Demon,
    EnemyKind::VoodooDemon,
    EnemyKind::Bunny,
    EnemyKind::Bird,
    EnemyKind::Squirrel,
    EnemyKind::CaveCritter,
    EnemyKind::WallOfFleshMouth,
    EnemyKind::WallOfFleshEye,
    EnemyKind::TheHungry,
    EnemyKind::Leech,
    EnemyKind::KingSlime,
    EnemyKind::QueenBee,
    EnemyKind::Bee,
    EnemyKind::BloodZombie,
    EnemyKind::Drippler,
    EnemyKind::GoblinPeon,
    EnemyKind::GoblinThief,
    EnemyKind::GoblinWarrior,
    EnemyKind::GoblinSorcerer,
    EnemyKind::GoblinArcher,
    EnemyKind::Antlion,
    EnemyKind::Vulture,
    EnemyKind::IceSlime,
    EnemyKind::SnowFlinx,
    EnemyKind::JungleSlime,
    EnemyKind::Hornet,
    EnemyKind::ManEater,
    EnemyKind::Harpy,
    EnemyKind::Shark,
    EnemyKind::Crab,
    EnemyKind::Piranha,
];

pub(crate) const KINDS: [EnemyKind; 63] = ALL_ENEMY_KINDS;

/// Whether `kind` is rendered with a bespoke multi-part procedural 3D rig
/// rather than a `.vox` rig directory.
pub(crate) fn has_procedural_rig(kind: EnemyKind) -> bool {
    !matches!(
        kind,
        EnemyKind::Slime
            | EnemyKind::LavaSlime
            | EnemyKind::DemonEye
            | EnemyKind::EaterOfSouls
            | EnemyKind::Zombie
            | EnemyKind::EyeOfCthulhu
            | EnemyKind::Guide
            | EnemyKind::Merchant
            | EnemyKind::ArmsDealer
            | EnemyKind::Nurse
            | EnemyKind::Demolitionist
            | EnemyKind::Dryad
            | EnemyKind::GoblinTinkerer
    )
}

/// Directory under `models/` holding the kind's `.vox` rig (for non-procedural kinds).
pub fn model_dir(kind: EnemyKind) -> &'static str {
    match kind {
        EnemyKind::Slime | EnemyKind::LavaSlime => "slime",
        EnemyKind::DemonEye => "demon_eye",
        EnemyKind::EaterOfSouls => "eater_of_souls",
        EnemyKind::Zombie => "zombie",
        EnemyKind::EyeOfCthulhu => "eye_of_cthulhu",
        EnemyKind::Guide | EnemyKind::Dryad => "guide",
        EnemyKind::Merchant | EnemyKind::Nurse | EnemyKind::GoblinTinkerer => "merchant",
        EnemyKind::ArmsDealer | EnemyKind::Demolitionist => "arms_dealer",
        _ => "procedural",
    }
}

/// Rig root scale: humanoids stand 2.2 blocks tall to fill their hitbox.
pub(crate) fn model_scale(kind: EnemyKind) -> f32 {
    match kind {
        EnemyKind::Zombie
        | EnemyKind::Guide
        | EnemyKind::Merchant
        | EnemyKind::ArmsDealer
        | EnemyKind::Nurse
        | EnemyKind::Demolitionist
        | EnemyKind::Dryad
        | EnemyKind::GoblinTinkerer
        | EnemyKind::Skeleton
        | EnemyKind::OldMan
        | EnemyKind::AngryBones
        | EnemyKind::DarkCaster => 1.1,
        EnemyKind::CursedSkull => 0.8,
        EnemyKind::DungeonGuardian => 2.0,
        EnemyKind::ServantOfCthulhu | EnemyKind::CaveBat => 0.5,
        EnemyKind::Hellbat => 0.6,
        EnemyKind::GiantWorm => 0.8,
        EnemyKind::EaterOfWorldsHead => 1.3,
        EnemyKind::EaterOfWorldsBody => 1.2,
        EnemyKind::EaterOfWorldsTail => 1.1,
        EnemyKind::Demon | EnemyKind::VoodooDemon => 1.3,
        EnemyKind::LavaSlime => 1.1,
        EnemyKind::BoneSerpentHead | EnemyKind::BoneSerpentBody | EnemyKind::BoneSerpentTail => 0.9,
        EnemyKind::FireImp => 1.0,
        EnemyKind::Bunny => 0.55,
        EnemyKind::Bird => 0.45,
        EnemyKind::Squirrel => 0.50,
        EnemyKind::CaveCritter => 0.45,
        _ => 1.0,
    }
}

pub(crate) fn tint_for(kind: EnemyKind) -> Color {
    match kind {
        EnemyKind::Slime => SLIME_TINT,
        EnemyKind::Skeleton => Color::srgb(0.9, 0.9, 0.85),
        EnemyKind::CaveBat => Color::srgb(0.55, 0.4, 0.55),
        EnemyKind::GiantWorm => Color::srgb(0.85, 0.55, 0.35),
        EnemyKind::EaterOfWorldsHead => Color::srgb(0.42, 0.18, 0.58),
        EnemyKind::EaterOfWorldsBody => Color::srgb(0.32, 0.12, 0.44),
        EnemyKind::EaterOfWorldsTail => Color::srgb(0.24, 0.08, 0.32),
        EnemyKind::OldMan => Color::srgb(0.45, 0.38, 0.32),
        EnemyKind::DungeonGuardian => Color::srgb(0.95, 0.95, 0.9),
        EnemyKind::LavaSlime => Color::srgb(1.0, 0.35, 0.1),
        EnemyKind::Hellbat => Color::srgb(0.5, 0.1, 0.15),
        EnemyKind::FireImp => Color::srgb(1.0, 0.55, 0.1),
        EnemyKind::Demon => Color::srgb(0.9, 0.15, 0.15),
        EnemyKind::VoodooDemon => Color::srgb(0.7, 0.15, 0.35),
        EnemyKind::BoneSerpentHead | EnemyKind::BoneSerpentBody | EnemyKind::BoneSerpentTail => {
            Color::srgb(0.92, 0.90, 0.82)
        }
        EnemyKind::Bunny => Color::srgb(0.95, 0.92, 0.88),
        EnemyKind::Bird => Color::srgb(0.35, 0.65, 0.95),
        EnemyKind::Squirrel => Color::srgb(0.72, 0.45, 0.25),
        EnemyKind::CaveCritter => Color::srgb(0.55, 0.52, 0.60),
        EnemyKind::WallOfFleshMouth | EnemyKind::WallOfFleshEye => Color::srgb(0.68, 0.16, 0.22),
        EnemyKind::TheHungry => Color::srgb(0.78, 0.20, 0.24),
        EnemyKind::Leech => Color::srgb(0.55, 0.14, 0.28),
        EnemyKind::KingSlime => Color::srgba(0.22, 0.58, 0.96, 0.85),
        EnemyKind::QueenBee => Color::srgb(0.95, 0.75, 0.15),
        EnemyKind::Bee => Color::srgb(0.96, 0.80, 0.18),
        EnemyKind::BloodZombie => Color::srgb(0.68, 0.16, 0.18),
        EnemyKind::Drippler => Color::srgb(0.75, 0.12, 0.20),
        EnemyKind::GoblinPeon
        | EnemyKind::GoblinThief
        | EnemyKind::GoblinWarrior
        | EnemyKind::GoblinSorcerer
        | EnemyKind::GoblinArcher => Color::srgb(0.42, 0.65, 0.34),
        EnemyKind::Antlion => Color::srgb(0.78, 0.62, 0.32),
        EnemyKind::Vulture => Color::srgb(0.45, 0.35, 0.28),
        EnemyKind::IceSlime => Color::srgba(0.60, 0.88, 1.0, 0.85),
        EnemyKind::SnowFlinx => Color::srgb(0.88, 0.90, 0.96),
        EnemyKind::JungleSlime => Color::srgba(0.28, 0.72, 0.24, 0.85),
        EnemyKind::Hornet => Color::srgb(0.85, 0.72, 0.14),
        EnemyKind::ManEater => Color::srgb(0.25, 0.62, 0.22),
        EnemyKind::Harpy => Color::srgb(0.48, 0.62, 0.92),
        EnemyKind::Shark => Color::srgb(0.42, 0.50, 0.62),
        EnemyKind::Crab => Color::srgb(0.88, 0.32, 0.18),
        EnemyKind::Piranha => Color::srgb(0.32, 0.58, 0.42),
        _ => Color::WHITE,
    }
}

#[derive(Clone, Default)]
pub struct EnemySounds {
    pub slime_hop: Option<Vec<u8>>,
}

impl EnemySounds {
    pub fn load(c: &terravibe_assets::TerrariaContent) -> Self {
        let slime_hop = c.sound("Sounds/Item_16").ok().map(|s| s.to_wav());
        Self { slime_hop }
    }
}

#[derive(Resource, Default)]
pub struct EnemyAudio {
    pub slime_hop: Option<Handle<AudioSource>>,
}

/// Animation for a kind: slimes hop, walkers walk, else idle.
pub(crate) fn anim_for(kind: EnemyKind, moving: bool) -> &'static str {
    match (kind, moving) {
        (EnemyKind::Slime | EnemyKind::LavaSlime | EnemyKind::Bunny, true) => "hop",
        (
            EnemyKind::Zombie
            | EnemyKind::Guide
            | EnemyKind::Merchant
            | EnemyKind::ArmsDealer
            | EnemyKind::Nurse
            | EnemyKind::Demolitionist
            | EnemyKind::Dryad
            | EnemyKind::GoblinTinkerer
            | EnemyKind::Skeleton
            | EnemyKind::OldMan
            | EnemyKind::AngryBones
            | EnemyKind::DarkCaster,
            true,
        ) => "walk",
        _ => "idle",
    }
}

/// Server-to-view enemy events, written by `net::pump`.
#[derive(Message, Clone, Copy, Debug)]
pub enum EnemyMsg {
    Spawned {
        id: EnemyId,
        kind: EnemyKind,
        pos: Vec3,
    },
    Moved {
        id: EnemyId,
        pos: Vec3,
        vel: Vec3,
        hp: i32,
    },
    Despawned {
        id: EnemyId,
    },
    Telegraph {
        id: EnemyId,
        #[allow(dead_code)]
        kind: EnemyKind,
    },
}

/// Scale multiplier during a wind-up telegraph (`1.0` when idle).
pub(crate) fn telegraph_scale_factor(telegraph_t: f32) -> f32 {
    if telegraph_t <= 0.0 {
        1.0
    } else {
        1.0 + 0.18 * (telegraph_t / TELEGRAPH_SECS).clamp(0.0, 1.0)
    }
}

/// Whether `kind` deals contact damage and should flash when lunging/approaching.
pub(crate) fn is_contact_enemy(kind: EnemyKind) -> bool {
    !matches!(
        kind,
        EnemyKind::Guide
            | EnemyKind::Merchant
            | EnemyKind::ArmsDealer
            | EnemyKind::Nurse
            | EnemyKind::Demolitionist
            | EnemyKind::Dryad
            | EnemyKind::GoblinTinkerer
            | EnemyKind::OldMan
            | EnemyKind::FireImp
            | EnemyKind::DarkCaster
            | EnemyKind::Bunny
            | EnemyKind::Bird
            | EnemyKind::Squirrel
            | EnemyKind::CaveCritter
    )
}

/// Resolves the enemy's current tint from hurt, wind-up telegraph, or approach flash.
pub(crate) fn enemy_tint(
    base: Color,
    hurt: u8,
    telegraph_t: f32,
    approach_flash: bool,
    elapsed: f32,
) -> Color {
    if hurt > 0 {
        HURT_TINT
    } else if telegraph_t > 0.0 {
        TELEGRAPH_TINT
    } else if approach_flash && (elapsed * 16.0).sin() > 0.0 {
        Color::srgb(1.0, 0.55, 0.25)
    } else {
        base
    }
}

/// Kind-aware tint resolution: slimes keep their translucent base colour during
/// jump wind-up and approach, flashing only on hurt while preserving alpha.
pub(crate) fn enemy_tint_for(
    kind: EnemyKind,
    base: Color,
    hurt: u8,
    telegraph_t: f32,
    approach_flash: bool,
    elapsed: f32,
) -> Color {
    if matches!(kind, EnemyKind::Slime | EnemyKind::LavaSlime) {
        if hurt > 0 {
            HURT_TINT.with_alpha(base.alpha())
        } else {
            base
        }
    } else {
        enemy_tint(base, hurt, telegraph_t, approach_flash, elapsed)
    }
}

pub struct EnemiesPlugin {
    pub sounds: EnemySounds,
}

/// Rotation about Y that turns +Z toward the horizontal move `from -> to`;
/// `None` when it barely moved.
pub(crate) fn should_yaw_toward(kind: EnemyKind) -> bool {
    !matches!(
        kind,
        EnemyKind::WallOfFleshMouth | EnemyKind::WallOfFleshEye
    )
}

pub(crate) fn yaw_toward(from: Vec3, to: Vec3) -> Option<f32> {
    let (dx, dz) = (to.x - from.x, to.z - from.z);
    (dx.hypot(dz) > TURN_MIN).then(|| dx.atan2(dz))
}

#[derive(Component)]
#[allow(dead_code)] // `id` identifies the entity in inspector/debug output
pub struct ClientEnemy {
    pub id: EnemyId,
    pub kind: EnemyKind,
    pub hp: i32,
    pub moving: bool,
    pub vel: Vec3,
    pub hurt: u8,
    pub telegraph_t: f32,
}

impl ClientEnemy {
    #[allow(dead_code)]
    pub fn new(id: EnemyId, kind: EnemyKind) -> Self {
        Self {
            id,
            kind,
            hp: i32::MAX,
            moving: false,
            vel: Vec3::ZERO,
            hurt: 0,
            telegraph_t: 0.0,
        }
    }
}

/// The rig's own material and its resting colour.
#[derive(Component)]
struct EnemyMat {
    handle: Handle<StandardMaterial>,
    base: Color,
}

/// Marks a procedural bird's left (`-1.0`) or right (`1.0`) wing for fold/flap animation.
#[derive(Component)]
pub(crate) struct BirdWing {
    pub(crate) side: f32,
}

#[derive(Component)]
pub(crate) struct ProcWing {
    pub(crate) side: f32,
    pub(crate) rest: Vec3,
    pub(crate) base_rot: Quat,
    pub(crate) speed: f32,
    pub(crate) amp: f32,
}

#[derive(Component)]
pub(crate) struct ProcLimb {
    pub(crate) phase: f32,
    pub(crate) rest: Vec3,
    pub(crate) amp: f32,
}

#[derive(Component)]
pub(crate) struct ProcJaw {
    pub(crate) rest: Vec3,
    pub(crate) speed: f32,
    pub(crate) amp: f32,
}

#[derive(Component)]
pub(crate) struct ProcTendril {
    pub(crate) side: f32,
    pub(crate) rest: Vec3,
    pub(crate) base_rot: Quat,
    pub(crate) speed: f32,
    pub(crate) amp: f32,
}

/// Whether a bird is airborne/flying rather than grounded/walking.
pub(crate) fn is_bird_airborne(vel: Vec3, from: Vec3, to: Vec3) -> bool {
    vel.y.abs() > 0.005 || (to.y - from.y).abs() > 0.01 || vel.x.hypot(vel.z) > 0.04
}

#[derive(Component)]
pub struct EnemyInterp {
    pub from: Vec3,
    pub to: Vec3,
    pub t: f32,
}

impl EnemyInterp {
    pub(crate) fn advance(&mut self, dt: f32) {
        self.t = (self.t + dt / SNAPSHOT_SECS).min(1.0);
    }

    /// Smoothstep ease from `from` to `to`.
    pub(crate) fn pos(&self) -> Vec3 {
        let e = self.t * self.t * (3.0 - 2.0 * self.t);
        self.from.lerp(self.to, e)
    }
}

/// Rigs by kind, loaded at startup; the mesh cache is shared by every spawn.
#[derive(Resource)]
struct EnemyRigs {
    rigs: HashMap<EnemyKind, (Rig, PathBuf)>,
    cache: MeshCache,
}

#[derive(Resource, Default)]
struct EnemyEntities(HashMap<EnemyId, Entity>);

impl Plugin for EnemiesPlugin {
    fn build(&self, app: &mut App) {
        if let Some(ref bytes) = self.sounds.slime_hop {
            let handle = app
                .world_mut()
                .resource_mut::<Assets<AudioSource>>()
                .add(AudioSource {
                    bytes: bytes.clone().into(),
                });
            app.insert_resource(EnemyAudio {
                slime_hop: Some(handle),
            });
        } else {
            app.init_resource::<EnemyAudio>();
        }

        app.init_resource::<EnemyEntities>()
            .add_message::<EnemyMsg>()
            .add_systems(Startup, load_rigs)
            .add_systems(
                OnExit(AppState::InGame),
                |mut ents: ResMut<EnemyEntities>| {
                    ents.0.clear();
                },
            )
            .add_systems(
                Update,
                (apply_msgs, animate)
                    .chain()
                    .before(crate::art::rig_view::animate_rigs)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

fn load_rigs(mut commands: Commands) {
    let mut rigs = HashMap::new();
    for kind in KINDS {
        if has_procedural_rig(kind) {
            continue;
        }
        let dir = art_dir().join("models").join(model_dir(kind));
        match rig::try_load(&dir) {
            Ok(r) => {
                rigs.insert(kind, (r, dir));
            }
            Err(e) => warn!("{e}"),
        }
    }
    commands.insert_resource(EnemyRigs {
        rigs,
        cache: MeshCache::default(),
    });
}

fn proc_mat(
    materials: &mut Assets<StandardMaterial>,
    base_color: Color,
    emissive: Option<LinearRgba>,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color,
        emissive: emissive.unwrap_or(LinearRgba::BLACK),
        perceptual_roughness: 0.8,
        ..default()
    })
}

fn proc_part(
    p: &mut ChildSpawnerCommands,
    meshes: &mut Assets<Mesh>,
    size: [f32; 3],
    mat: Handle<StandardMaterial>,
    xyz: [f32; 3],
) {
    p.spawn((
        Mesh3d(meshes.add(Cuboid::new(size[0], size[1], size[2]))),
        MeshMaterial3d(mat),
        Transform::from_xyz(xyz[0], xyz[1], xyz[2]),
    ));
}

fn proc_part_rot(
    p: &mut ChildSpawnerCommands,
    meshes: &mut Assets<Mesh>,
    size: [f32; 3],
    mat: Handle<StandardMaterial>,
    xyz: [f32; 3],
    rot: Quat,
) {
    p.spawn((
        Mesh3d(meshes.add(Cuboid::new(size[0], size[1], size[2]))),
        MeshMaterial3d(mat),
        Transform::from_xyz(xyz[0], xyz[1], xyz[2]).with_rotation(rot),
    ));
}

#[allow(clippy::too_many_lines)]
fn spawn_procedural_enemy(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    id: EnemyId,
    kind: EnemyKind,
    pos: Vec3,
) -> Option<Entity> {
    if !has_procedural_rig(kind) {
        return None;
    }

    let spawn_with = |commands: &mut Commands,
                      meshes: &mut Assets<Mesh>,
                      root_size: [f32; 3],
                      root_mat: Handle<StandardMaterial>,
                      root_color: Color,
                      build: Box<dyn FnOnce(&mut ChildSpawnerCommands, &mut Assets<Mesh>)>|
     -> Entity {
        let root_mesh = meshes.add(Cuboid::new(root_size[0], root_size[1], root_size[2]));
        commands
            .spawn((
                Mesh3d(root_mesh),
                MeshMaterial3d(root_mat.clone()),
                Transform::from_translation(pos).with_scale(Vec3::splat(model_scale(kind))),
                ClientEnemy {
                    id,
                    kind,
                    hp: i32::MAX,
                    moving: false,
                    vel: Vec3::ZERO,
                    hurt: 0,
                    telegraph_t: 0.0,
                },
                EnemyInterp {
                    from: pos,
                    to: pos,
                    t: 1.0,
                },
                EnemyMat {
                    handle: root_mat,
                    base: root_color,
                },
                DespawnOnExit(AppState::InGame),
            ))
            .with_children(|p| build(p, meshes))
            .id()
    };

    let entity = match kind {
        EnemyKind::WallOfFleshMouth => {
            let base = Color::srgb(0.62, 0.14, 0.20);
            let flesh = proc_mat(materials, base, None);
            let dark = proc_mat(materials, Color::srgb(0.36, 0.06, 0.10), None);
            let sinew = proc_mat(materials, Color::srgb(0.48, 0.10, 0.16), None);
            let tooth = proc_mat(materials, Color::srgb(0.92, 0.88, 0.78), None);
            spawn_with(
                commands,
                meshes,
                [2.4, 24.0, 64.0],
                flesh,
                base,
                Box::new(move |p, m| {
                    for z_off in [-24.0, -16.0, -8.0, 8.0, 16.0, 24.0] {
                        proc_part(p, m, [2.85, 24.0, 1.3], dark.clone(), [0.0, 0.0, z_off]);
                    }
                    for y_off in [-8.0, -4.0, 4.0, 8.0] {
                        proc_part(p, m, [2.7, 0.7, 56.0], sinew.clone(), [0.1, y_off, 0.0]);
                    }
                    for &(y, z) in &[(-5.0, -5.0), (5.0, 5.0), (-3.0, 7.0), (3.0, -7.0)] {
                        p.spawn((
                            Mesh3d(m.add(Cuboid::new(1.8, 0.35, 0.35))),
                            MeshMaterial3d(sinew.clone()),
                            Transform::from_xyz(1.6, y, z),
                            ProcTendril {
                                side: y.signum(),
                                rest: Vec3::new(1.6, y, z),
                                base_rot: Quat::IDENTITY,
                                speed: 5.0,
                                amp: 0.28,
                            },
                        ));
                    }
                    // Gaping maw upper & lower jaws with chomping fang rows
                    proc_part(p, m, [2.6, 1.4, 3.6], dark.clone(), [0.0, 1.7, 0.0]);
                    p.spawn((
                        Mesh3d(m.add(Cuboid::new(2.6, 1.3, 3.6))),
                        MeshMaterial3d(dark),
                        Transform::from_xyz(0.0, -1.7, 0.0),
                        ProcJaw {
                            rest: Vec3::new(0.0, -1.7, 0.0),
                            speed: 8.5,
                            amp: 0.42,
                        },
                    ));
                    for zt in [-1.2, -0.4, 0.4, 1.2] {
                        proc_part(p, m, [0.55, 0.85, 0.40], tooth.clone(), [0.9, 0.85, zt]);
                        p.spawn((
                            Mesh3d(m.add(Cuboid::new(0.55, 0.85, 0.40))),
                            MeshMaterial3d(tooth.clone()),
                            Transform::from_xyz(0.9, -0.85, zt),
                            ProcJaw {
                                rest: Vec3::new(0.9, -0.85, zt),
                                speed: 8.5,
                                amp: 0.42,
                            },
                        ));
                    }
                }),
            )
        }
        EnemyKind::WallOfFleshEye => {
            let base = Color::srgb(0.94, 0.86, 0.84);
            let sclera = proc_mat(materials, base, None);
            let stalk = proc_mat(materials, Color::srgb(0.58, 0.14, 0.18), None);
            let vein = proc_mat(materials, Color::srgb(0.75, 0.18, 0.22), None);
            let iris = proc_mat(
                materials,
                Color::srgb(0.85, 0.12, 0.16),
                Some(LinearRgba::rgb(0.65, 0.06, 0.06)),
            );
            let pupil = proc_mat(materials, Color::srgb(0.05, 0.02, 0.03), None);
            spawn_with(
                commands,
                meshes,
                [2.2, 2.6, 2.6],
                sclera,
                base,
                Box::new(move |p, m| {
                    proc_part(p, m, [1.6, 1.1, 1.1], stalk, [-1.4, 0.0, 0.0]);
                    proc_part(p, m, [2.26, 0.22, 2.7], vein.clone(), [0.0, 0.7, 0.0]);
                    proc_part(p, m, [2.26, 0.22, 2.7], vein, [0.0, -0.7, 0.0]);
                    proc_part(p, m, [2.35, 1.5, 1.5], iris, [0.0, 0.0, 0.0]);
                    proc_part(p, m, [2.45, 0.7, 0.7], pupil, [0.0, 0.0, 0.0]);
                }),
            )
        }
        EnemyKind::TheHungry => {
            let base = tint_for(kind);
            let mat = proc_mat(materials, base, None);
            let dark = proc_mat(materials, Color::srgb(0.45, 0.08, 0.12), None);
            let tooth = proc_mat(materials, Color::srgb(0.92, 0.88, 0.78), None);
            spawn_with(
                commands,
                meshes,
                [1.1, 1.0, 1.2],
                mat,
                base,
                Box::new(move |p, m| {
                    proc_part(p, m, [0.30, 0.30, 1.2], dark, [0.0, 0.0, -1.0]);
                    p.spawn((
                        Mesh3d(m.add(Cuboid::new(0.95, 0.35, 0.65))),
                        MeshMaterial3d(tooth.clone()),
                        Transform::from_xyz(0.0, -0.35, 0.55),
                        ProcJaw {
                            rest: Vec3::new(0.0, -0.35, 0.55),
                            speed: 11.0,
                            amp: 0.35,
                        },
                    ));
                    proc_part(p, m, [0.25, 0.35, 0.25], tooth.clone(), [-0.3, 0.25, 0.62]);
                    proc_part(p, m, [0.25, 0.35, 0.25], tooth, [0.3, 0.25, 0.62]);
                }),
            )
        }
        EnemyKind::Leech => {
            let base = tint_for(kind);
            let mat = proc_mat(materials, base, None);
            let dark = proc_mat(materials, Color::srgb(0.32, 0.08, 0.16), None);
            let tooth = proc_mat(materials, Color::srgb(0.90, 0.85, 0.75), None);
            spawn_with(
                commands,
                meshes,
                [0.8, 0.7, 1.2],
                mat.clone(),
                base,
                Box::new(move |p, m| {
                    proc_part(p, m, [0.68, 0.58, 0.7], dark.clone(), [0.0, 0.0, -0.8]);
                    p.spawn((
                        Mesh3d(m.add(Cuboid::new(0.52, 0.45, 0.7))),
                        MeshMaterial3d(mat),
                        Transform::from_xyz(0.0, 0.0, -1.45),
                        ProcTendril {
                            side: 1.0,
                            rest: Vec3::new(0.0, 0.0, -1.45),
                            base_rot: Quat::IDENTITY,
                            speed: 9.0,
                            amp: 0.35,
                        },
                    ));
                    proc_part(p, m, [0.16, 0.16, 0.25], tooth.clone(), [-0.20, 0.0, 0.65]);
                    proc_part(p, m, [0.16, 0.16, 0.25], tooth, [0.20, 0.0, 0.65]);
                }),
            )
        }
        EnemyKind::KingSlime => {
            let base = tint_for(kind);
            let mat = proc_mat(materials, base, None);
            let gold = proc_mat(
                materials,
                Color::srgb(1.0, 0.84, 0.18),
                Some(LinearRgba::rgb(0.35, 0.25, 0.02)),
            );
            let ninja = proc_mat(materials, Color::srgb(0.14, 0.14, 0.20), None);
            spawn_with(
                commands,
                meshes,
                [4.2, 3.6, 4.2],
                mat,
                base,
                Box::new(move |p, m| {
                    // Ninja silhouette inside slime + golden crown on top
                    proc_part(p, m, [1.2, 1.8, 1.0], ninja, [0.0, -0.2, 0.0]);
                    proc_part(p, m, [1.8, 0.6, 1.8], gold.clone(), [0.0, 2.0, 0.0]);
                    proc_part(p, m, [0.35, 0.5, 0.35], gold.clone(), [-0.65, 2.45, -0.65]);
                    proc_part(p, m, [0.35, 0.5, 0.35], gold.clone(), [0.65, 2.45, -0.65]);
                    proc_part(p, m, [0.35, 0.5, 0.35], gold, [0.0, 2.55, 0.65]);
                }),
            )
        }
        EnemyKind::QueenBee => {
            let base = tint_for(kind);
            let yellow = proc_mat(materials, base, None);
            let stripe = proc_mat(materials, Color::srgb(0.16, 0.12, 0.08), None);
            let wing = proc_mat(materials, Color::srgba(0.88, 0.94, 1.0, 0.72), None);
            spawn_with(
                commands,
                meshes,
                [1.8, 1.6, 2.8],
                yellow,
                base,
                Box::new(move |p, m| {
                    proc_part(p, m, [1.9, 1.4, 0.5], stripe.clone(), [0.0, 0.0, -0.5]);
                    proc_part(p, m, [1.9, 1.4, 0.5], stripe.clone(), [0.0, 0.0, 0.5]);
                    proc_part(p, m, [0.35, 0.35, 0.9], stripe, [0.0, -0.2, -1.7]);
                    proc_part(p, m, [2.2, 0.12, 1.1], wing.clone(), [-1.4, 0.9, 0.0]);
                    proc_part(p, m, [2.2, 0.12, 1.1], wing, [1.4, 0.9, 0.0]);
                }),
            )
        }
        EnemyKind::Bee => {
            let base = tint_for(kind);
            let yellow = proc_mat(materials, base, None);
            let wing = proc_mat(materials, Color::srgba(0.90, 0.95, 1.0, 0.72), None);
            spawn_with(
                commands,
                meshes,
                [0.45, 0.4, 0.6],
                yellow,
                base,
                Box::new(move |p, m| {
                    proc_part(p, m, [0.55, 0.06, 0.3], wing.clone(), [-0.35, 0.25, 0.0]);
                    proc_part(p, m, [0.55, 0.06, 0.3], wing, [0.35, 0.25, 0.0]);
                }),
            )
        }
        EnemyKind::BloodZombie
        | EnemyKind::GoblinPeon
        | EnemyKind::GoblinThief
        | EnemyKind::GoblinWarrior
        | EnemyKind::GoblinSorcerer
        | EnemyKind::GoblinArcher
        | EnemyKind::SnowFlinx => {
            let base = tint_for(kind);
            let mat = proc_mat(materials, base, None);
            let dark = proc_mat(materials, Color::srgb(0.22, 0.20, 0.24), None);
            spawn_with(
                commands,
                meshes,
                [0.8, 1.1, 0.5],
                mat.clone(),
                base,
                Box::new(move |p, m| {
                    proc_part(p, m, [0.6, 0.6, 0.6], mat, [0.0, 0.85, 0.0]);
                    proc_part(p, m, [0.25, 0.75, 0.25], dark.clone(), [-0.25, -0.75, 0.0]);
                    proc_part(p, m, [0.25, 0.75, 0.25], dark, [0.25, -0.75, 0.0]);
                }),
            )
        }
        EnemyKind::IceSlime | EnemyKind::JungleSlime => {
            let base = tint_for(kind);
            let mat = proc_mat(materials, base, None);
            spawn_with(
                commands,
                meshes,
                [1.1, 0.8, 1.1],
                mat,
                base,
                Box::new(|_, _| {}),
            )
        }
        EnemyKind::Drippler
        | EnemyKind::Antlion
        | EnemyKind::Vulture
        | EnemyKind::Hornet
        | EnemyKind::ManEater
        | EnemyKind::Harpy
        | EnemyKind::Shark
        | EnemyKind::Crab
        | EnemyKind::Piranha => {
            let base = tint_for(kind);
            let mat = proc_mat(materials, base, None);
            let size = match kind {
                EnemyKind::Shark => [1.0, 0.7, 2.2],
                EnemyKind::Crab => [1.1, 0.6, 0.9],
                EnemyKind::Harpy => [0.9, 1.3, 0.7],
                _ => [0.9, 0.8, 1.0],
            };
            spawn_with(commands, meshes, size, mat, base, Box::new(|_, _| {}))
        }
        EnemyKind::SkeletronHead | EnemyKind::DungeonGuardian => {
            let base = Color::srgb(0.92, 0.90, 0.82);
            let skull = proc_mat(materials, base, None);
            let shade = proc_mat(materials, Color::srgb(0.76, 0.73, 0.66), None);
            let is_dg = kind == EnemyKind::DungeonGuardian;
            let socket = proc_mat(
                materials,
                Color::srgb(0.05, 0.04, 0.04),
                if is_dg {
                    Some(LinearRgba::rgb(0.95, 0.05, 0.05))
                } else {
                    Some(LinearRgba::rgb(0.45, 0.18, 0.02))
                },
            );
            let tooth = proc_mat(materials, Color::srgb(0.96, 0.94, 0.86), None);
            spawn_with(
                commands,
                meshes,
                [1.8, 1.5, 1.5],
                skull.clone(),
                base,
                Box::new(move |p, m| {
                    // Rounded cranium cap + brow ridge + cheekbones
                    proc_part(p, m, [1.5, 0.35, 1.25], skull.clone(), [0.0, 0.85, -0.05]);
                    proc_part(p, m, [1.85, 0.26, 0.30], shade.clone(), [0.0, 0.42, 0.72]);
                    proc_part(
                        p,
                        m,
                        [0.42, 0.34, 0.42],
                        shade.clone(),
                        [-0.62, -0.22, 0.56],
                    );
                    proc_part(p, m, [0.42, 0.34, 0.42], shade, [0.62, -0.22, 0.56]);
                    // Glowing eye sockets & nasal cavity
                    proc_part(
                        p,
                        m,
                        [0.36, 0.42, 0.16],
                        socket.clone(),
                        [-0.45, 0.14, 0.76],
                    );
                    proc_part(p, m, [0.36, 0.42, 0.16], socket.clone(), [0.45, 0.14, 0.76]);
                    proc_part(p, m, [0.22, 0.24, 0.14], socket, [0.0, -0.18, 0.76]);
                    // Articulated hinged jaw with teeth
                    p.spawn((
                        Mesh3d(m.add(Cuboid::new(1.22, 0.36, 0.72))),
                        MeshMaterial3d(skull),
                        Transform::from_xyz(0.0, -0.72, 0.28),
                        ProcJaw {
                            rest: Vec3::new(0.0, -0.72, 0.28),
                            speed: if is_dg { 14.0 } else { 7.5 },
                            amp: 0.28,
                        },
                    ));
                    for tx in [-0.38, -0.12, 0.12, 0.38] {
                        p.spawn((
                            Mesh3d(m.add(Cuboid::new(0.14, 0.20, 0.14))),
                            MeshMaterial3d(tooth.clone()),
                            Transform::from_xyz(tx, -0.50, 0.60),
                            ProcJaw {
                                rest: Vec3::new(tx, -0.50, 0.60),
                                speed: if is_dg { 14.0 } else { 7.5 },
                                amp: 0.28,
                            },
                        ));
                    }
                }),
            )
        }
        EnemyKind::SkeletronHand => {
            let base = Color::srgb(0.90, 0.88, 0.80);
            let hand = proc_mat(materials, base, None);
            let joint = proc_mat(materials, Color::srgb(0.74, 0.71, 0.63), None);
            spawn_with(
                commands,
                meshes,
                [0.85, 0.36, 0.85],
                hand.clone(),
                base,
                Box::new(move |p, m| {
                    // Forearm
                    proc_part(p, m, [0.42, 0.32, 0.85], joint.clone(), [0.0, 0.0, -0.75]);
                    // 4 curled jointed bone fingers
                    for fx in [-0.30, -0.10, 0.10, 0.30] {
                        proc_part_rot(
                            p,
                            m,
                            [0.14, 0.16, 0.48],
                            hand.clone(),
                            [fx, -0.02, 0.58],
                            Quat::from_rotation_x(0.22),
                        );
                        proc_part_rot(
                            p,
                            m,
                            [0.12, 0.14, 0.40],
                            joint.clone(),
                            [fx, -0.18, 0.92],
                            Quat::from_rotation_x(0.65),
                        );
                    }
                    // Thumb claw
                    proc_part_rot(
                        p,
                        m,
                        [0.36, 0.16, 0.16],
                        hand,
                        [-0.50, -0.06, 0.14],
                        Quat::from_rotation_y(-0.4),
                    );
                }),
            )
        }
        EnemyKind::Bunny => {
            let base = Color::srgb(0.96, 0.94, 0.92);
            let fur = proc_mat(materials, base, None);
            let ear_pink = proc_mat(materials, Color::srgb(0.95, 0.65, 0.72), None);
            let eye = proc_mat(materials, Color::srgb(0.08, 0.08, 0.10), None);
            spawn_with(
                commands,
                meshes,
                [0.72, 0.56, 0.92],
                fur.clone(),
                base,
                Box::new(move |p, m| {
                    // Head
                    proc_part(p, m, [0.60, 0.54, 0.54], fur.clone(), [0.0, 0.46, 0.42]);
                    // Tall ears + pink inner ear detail
                    proc_part_rot(
                        p,
                        m,
                        [0.16, 0.62, 0.14],
                        fur.clone(),
                        [-0.18, 0.97, 0.34],
                        Quat::from_rotation_x(-0.18),
                    );
                    proc_part_rot(
                        p,
                        m,
                        [0.16, 0.62, 0.14],
                        fur.clone(),
                        [0.18, 0.97, 0.34],
                        Quat::from_rotation_x(-0.18),
                    );
                    proc_part_rot(
                        p,
                        m,
                        [0.08, 0.44, 0.06],
                        ear_pink.clone(),
                        [-0.18, 0.95, 0.39],
                        Quat::from_rotation_x(-0.18),
                    );
                    proc_part_rot(
                        p,
                        m,
                        [0.08, 0.44, 0.06],
                        ear_pink.clone(),
                        [0.18, 0.95, 0.39],
                        Quat::from_rotation_x(-0.18),
                    );
                    // Pink nose & eyes
                    proc_part(p, m, [0.14, 0.12, 0.08], ear_pink, [0.0, 0.41, 0.71]);
                    proc_part(p, m, [0.10, 0.12, 0.08], eye.clone(), [-0.18, 0.53, 0.67]);
                    proc_part(p, m, [0.10, 0.12, 0.08], eye, [0.18, 0.53, 0.67]);
                    // Puff tail
                    proc_part(p, m, [0.30, 0.30, 0.30], fur.clone(), [0.0, 0.14, -0.56]);
                    // 2 bent hind haunches + hind feet
                    proc_part_rot(
                        p,
                        m,
                        [0.20, 0.32, 0.30],
                        fur.clone(),
                        [-0.30, -0.22, -0.24],
                        Quat::from_rotation_x(0.35),
                    );
                    proc_part_rot(
                        p,
                        m,
                        [0.20, 0.32, 0.30],
                        fur.clone(),
                        [0.30, -0.22, -0.24],
                        Quat::from_rotation_x(0.35),
                    );
                    proc_part(p, m, [0.18, 0.12, 0.34], fur.clone(), [-0.30, -0.42, -0.14]);
                    proc_part(p, m, [0.18, 0.12, 0.34], fur.clone(), [0.30, -0.42, -0.14]);
                    // 2 front paws
                    proc_part(p, m, [0.16, 0.28, 0.16], fur.clone(), [-0.22, -0.34, 0.30]);
                    proc_part(p, m, [0.16, 0.28, 0.16], fur, [0.22, -0.34, 0.30]);
                }),
            )
        }
        EnemyKind::Bird => {
            let base = Color::srgb(0.32, 0.62, 0.94);
            let feather = proc_mat(materials, base, None);
            let belly = proc_mat(materials, Color::srgb(0.88, 0.90, 0.95), None);
            let beak = proc_mat(materials, Color::srgb(0.96, 0.62, 0.12), None);
            let eye = proc_mat(materials, Color::srgb(0.06, 0.06, 0.08), None);
            spawn_with(
                commands,
                meshes,
                [0.62, 0.55, 0.85],
                feather.clone(),
                base,
                Box::new(move |p, m| {
                    proc_part(p, m, [0.46, 0.30, 0.60], belly, [0.0, -0.14, 0.05]);
                    proc_part(p, m, [0.44, 0.42, 0.44], feather.clone(), [0.0, 0.32, 0.38]);
                    // Orange beak
                    proc_part(p, m, [0.16, 0.14, 0.32], beak.clone(), [0.0, 0.28, 0.68]);
                    // Thin orange legs & feet
                    proc_part(p, m, [0.06, 0.22, 0.06], beak.clone(), [-0.14, -0.38, 0.02]);
                    proc_part(p, m, [0.06, 0.22, 0.06], beak.clone(), [0.14, -0.38, 0.02]);
                    proc_part(p, m, [0.10, 0.05, 0.16], beak.clone(), [-0.14, -0.49, 0.06]);
                    proc_part(p, m, [0.10, 0.05, 0.16], beak, [0.14, -0.49, 0.06]);
                    // Eyes
                    proc_part(p, m, [0.08, 0.10, 0.08], eye.clone(), [-0.21, 0.38, 0.46]);
                    proc_part(p, m, [0.08, 0.10, 0.08], eye, [0.21, 0.38, 0.46]);
                    // Left & right wings (spawned folded against sides; spread+flap when airborne)
                    for side in [-1.0_f32, 1.0_f32] {
                        p.spawn((
                            Mesh3d(m.add(Cuboid::new(0.60, 0.08, 0.42))),
                            MeshMaterial3d(feather.clone()),
                            Transform::from_xyz(side * 0.34, 0.02, -0.02)
                                .with_rotation(Quat::from_rotation_z(side * -1.48)),
                            BirdWing { side },
                        ));
                    }
                    // Tail
                    proc_part_rot(
                        p,
                        m,
                        [0.32, 0.10, 0.45],
                        feather,
                        [0.0, 0.12, -0.58],
                        Quat::from_rotation_x(0.35),
                    );
                }),
            )
        }
        EnemyKind::Squirrel => {
            let base = Color::srgb(0.68, 0.40, 0.20);
            let fur = proc_mat(materials, base, None);
            let belly = proc_mat(materials, Color::srgb(0.88, 0.74, 0.56), None);
            let eye = proc_mat(materials, Color::srgb(0.06, 0.06, 0.08), None);
            spawn_with(
                commands,
                meshes,
                [0.64, 0.56, 0.86],
                fur.clone(),
                base,
                Box::new(move |p, m| {
                    proc_part(p, m, [0.46, 0.32, 0.55], belly, [0.0, -0.10, 0.06]);
                    proc_part(p, m, [0.50, 0.46, 0.46], fur.clone(), [0.0, 0.38, 0.40]);
                    // Tufted ears
                    proc_part(p, m, [0.14, 0.24, 0.12], fur.clone(), [-0.18, 0.68, 0.36]);
                    proc_part(p, m, [0.14, 0.24, 0.12], fur.clone(), [0.18, 0.68, 0.36]);
                    proc_part(p, m, [0.08, 0.10, 0.08], eye.clone(), [-0.18, 0.44, 0.61]);
                    proc_part(p, m, [0.08, 0.10, 0.08], eye, [0.18, 0.44, 0.61]);
                    // 4 tiny voxel legs
                    proc_part(p, m, [0.12, 0.20, 0.12], fur.clone(), [-0.20, -0.34, 0.24]);
                    proc_part(p, m, [0.12, 0.20, 0.12], fur.clone(), [0.20, -0.34, 0.24]);
                    proc_part(p, m, [0.14, 0.20, 0.16], fur.clone(), [-0.22, -0.34, -0.24]);
                    proc_part(p, m, [0.14, 0.20, 0.16], fur.clone(), [0.22, -0.34, -0.24]);
                    // Bushy curved tail
                    proc_part_rot(
                        p,
                        m,
                        [0.34, 0.85, 0.34],
                        fur,
                        [0.0, 0.52, -0.58],
                        Quat::from_rotation_x(-0.45),
                    );
                }),
            )
        }
        EnemyKind::CaveCritter => {
            let base = Color::srgb(0.32, 0.34, 0.45);
            let shell = proc_mat(materials, base, None);
            let glow = proc_mat(
                materials,
                Color::srgb(0.35, 0.92, 0.78),
                Some(LinearRgba::rgb(0.2, 0.85, 0.7)),
            );
            let leg = proc_mat(materials, Color::srgb(0.16, 0.16, 0.22), None);
            spawn_with(
                commands,
                meshes,
                [0.72, 0.42, 0.92],
                shell,
                base,
                Box::new(move |p, m| {
                    // Glowing abdomen & eyes
                    proc_part(p, m, [0.56, 0.32, 0.44], glow.clone(), [0.0, 0.12, -0.30]);
                    proc_part(p, m, [0.10, 0.10, 0.10], glow.clone(), [-0.18, 0.10, 0.46]);
                    proc_part(p, m, [0.10, 0.10, 0.10], glow, [0.18, 0.10, 0.46]);
                    for z in [-0.28, 0.0, 0.28] {
                        proc_part(p, m, [0.28, 0.10, 0.10], leg.clone(), [-0.46, -0.14, z]);
                        proc_part(p, m, [0.28, 0.10, 0.10], leg.clone(), [0.46, -0.14, z]);
                    }
                }),
            )
        }
        EnemyKind::CaveBat | EnemyKind::Hellbat => {
            let is_hell = kind == EnemyKind::Hellbat;
            let base = if is_hell {
                Color::srgb(0.62, 0.12, 0.14)
            } else {
                Color::srgb(0.42, 0.30, 0.46)
            };
            let body = proc_mat(materials, base, None);
            let wing = proc_mat(
                materials,
                if is_hell {
                    Color::srgb(0.92, 0.36, 0.08)
                } else {
                    Color::srgb(0.28, 0.20, 0.32)
                },
                None,
            );
            let eye = proc_mat(
                materials,
                Color::srgb(1.0, 0.82, 0.2),
                Some(LinearRgba::rgb(0.8, 0.5, 0.05)),
            );
            spawn_with(
                commands,
                meshes,
                [0.65, 0.55, 0.65],
                body.clone(),
                base,
                Box::new(move |p, m| {
                    // Pointed ears
                    proc_part(p, m, [0.16, 0.32, 0.14], body.clone(), [-0.20, 0.40, 0.10]);
                    proc_part(p, m, [0.16, 0.32, 0.14], body, [0.20, 0.40, 0.10]);
                    // Glowing eyes
                    proc_part(p, m, [0.12, 0.12, 0.08], eye.clone(), [-0.16, 0.10, 0.34]);
                    proc_part(p, m, [0.12, 0.12, 0.08], eye, [0.16, 0.10, 0.34]);
                    // Angled flapping bat wings
                    for side in [-1.0_f32, 1.0_f32] {
                        let rest = Vec3::new(side * 0.78, 0.18, -0.05);
                        let base_rot = Quat::from_rotation_z(-side * 0.32);
                        p.spawn((
                            Mesh3d(m.add(Cuboid::new(0.95, 0.08, 0.55))),
                            MeshMaterial3d(wing.clone()),
                            Transform::from_translation(rest).with_rotation(base_rot),
                            ProcWing {
                                side,
                                rest,
                                base_rot,
                                speed: 18.0,
                                amp: 0.65,
                            },
                        ));
                    }
                }),
            )
        }
        EnemyKind::ServantOfCthulhu => {
            let base = Color::srgb(0.92, 0.88, 0.86);
            let sclera = proc_mat(materials, base, None);
            let iris = proc_mat(materials, Color::srgb(0.75, 0.14, 0.18), None);
            let pupil = proc_mat(materials, Color::srgb(0.06, 0.03, 0.04), None);
            let tendril = proc_mat(materials, Color::srgb(0.68, 0.16, 0.22), None);
            spawn_with(
                commands,
                meshes,
                [0.95, 0.95, 0.95],
                sclera,
                base,
                Box::new(move |p, m| {
                    proc_part(p, m, [0.60, 0.60, 0.16], iris, [0.0, 0.0, 0.46]);
                    proc_part(p, m, [0.28, 0.28, 0.18], pupil, [0.0, 0.0, 0.50]);
                    for &(x, y) in &[(-0.24, 0.20), (0.24, 0.20), (0.0, -0.24)] {
                        let rest = Vec3::new(x, y, -0.68);
                        p.spawn((
                            Mesh3d(m.add(Cuboid::new(0.14, 0.14, 0.65))),
                            MeshMaterial3d(tendril.clone()),
                            Transform::from_translation(rest),
                            ProcTendril {
                                side: if x == 0.0 { 1.0 } else { x.signum() },
                                rest,
                                base_rot: Quat::IDENTITY,
                                speed: 10.0,
                                amp: 0.35,
                            },
                        ));
                    }
                }),
            )
        }
        EnemyKind::GiantWorm => {
            let base = Color::srgb(0.76, 0.48, 0.36);
            let seg = proc_mat(materials, base, None);
            let ring = proc_mat(materials, Color::srgb(0.55, 0.32, 0.22), None);
            let fang = proc_mat(materials, Color::srgb(0.90, 0.86, 0.72), None);
            spawn_with(
                commands,
                meshes,
                [0.85, 0.85, 1.05],
                seg.clone(),
                base,
                Box::new(move |p, m| {
                    proc_part(p, m, [0.92, 0.92, 0.22], ring, [0.0, 0.0, 0.0]);
                    proc_part(p, m, [0.72, 0.72, 0.90], seg, [0.0, 0.0, -0.92]);
                    for side in [-1.0_f32, 1.0_f32] {
                        let rest = Vec3::new(side * 0.24, -0.10, 0.62);
                        p.spawn((
                            Mesh3d(m.add(Cuboid::new(0.14, 0.14, 0.32))),
                            MeshMaterial3d(fang.clone()),
                            Transform::from_translation(rest),
                            ProcTendril {
                                side,
                                rest,
                                base_rot: Quat::IDENTITY,
                                speed: 9.0,
                                amp: 0.25,
                            },
                        ));
                    }
                }),
            )
        }
        EnemyKind::EaterOfWorldsHead
        | EnemyKind::EaterOfWorldsBody
        | EnemyKind::EaterOfWorldsTail => {
            let base = tint_for(kind);
            let armor = proc_mat(materials, base, None);
            let carapace = proc_mat(materials, Color::srgb(0.20, 0.08, 0.28), None);
            let spine = proc_mat(materials, Color::srgb(0.58, 0.34, 0.72), None);
            let eye = proc_mat(
                materials,
                Color::srgb(0.85, 1.0, 0.35),
                Some(LinearRgba::rgb(0.5, 0.8, 0.1)),
            );
            let is_head = kind == EnemyKind::EaterOfWorldsHead;
            let is_tail = kind == EnemyKind::EaterOfWorldsTail;
            let size = if is_tail {
                [0.95, 0.95, 1.15]
            } else {
                [1.25, 1.25, 1.25]
            };
            spawn_with(
                commands,
                meshes,
                size,
                armor,
                base,
                Box::new(move |p, m| {
                    // Layered purple carapace & dorsal chitin ridges
                    proc_part(p, m, [1.35, 0.35, 0.95], carapace, [0.0, 0.50, 0.0]);
                    proc_part(p, m, [1.45, 0.22, 0.35], spine.clone(), [0.0, 0.65, 0.0]);
                    proc_part(p, m, [0.25, 0.30, 0.45], spine.clone(), [-0.70, 0.0, 0.0]);
                    proc_part(p, m, [0.25, 0.30, 0.45], spine, [0.70, 0.0, 0.0]);
                    // Glowing toxic yellow-green multi-eyes on every segment
                    proc_part(p, m, [0.14, 0.16, 0.18], eye.clone(), [-0.66, 0.24, 0.20]);
                    proc_part(p, m, [0.14, 0.16, 0.18], eye.clone(), [0.66, 0.24, 0.20]);
                    if is_head {
                        proc_part(p, m, [0.20, 0.20, 0.12], eye.clone(), [-0.34, 0.24, 0.64]);
                        proc_part(p, m, [0.20, 0.20, 0.12], eye.clone(), [0.34, 0.24, 0.64]);
                        proc_part(p, m, [0.16, 0.16, 0.12], eye.clone(), [0.0, 0.38, 0.64]);
                        // Snapping mandibles
                        for side in [-1.0_f32, 1.0_f32] {
                            let rest = Vec3::new(side * 0.42, -0.18, 0.78);
                            let base_rot = Quat::from_rotation_y(-side * 0.25);
                            p.spawn((
                                Mesh3d(m.add(Cuboid::new(0.18, 0.20, 0.56))),
                                MeshMaterial3d(eye.clone()),
                                Transform::from_translation(rest).with_rotation(base_rot),
                                ProcTendril {
                                    side,
                                    rest,
                                    base_rot,
                                    speed: 11.0,
                                    amp: 0.35,
                                },
                            ));
                        }
                    }
                }),
            )
        }
        EnemyKind::BoneSerpentHead | EnemyKind::BoneSerpentBody | EnemyKind::BoneSerpentTail => {
            let base = Color::srgb(0.92, 0.90, 0.82);
            let bone = proc_mat(materials, base, None);
            let socket = proc_mat(
                materials,
                Color::srgb(1.0, 0.32, 0.08),
                Some(LinearRgba::rgb(0.9, 0.25, 0.05)),
            );
            let is_head = kind == EnemyKind::BoneSerpentHead;
            let is_tail = kind == EnemyKind::BoneSerpentTail;
            let size = if is_tail {
                [0.65, 0.65, 1.10]
            } else {
                [1.05, 0.95, 1.15]
            };
            spawn_with(
                commands,
                meshes,
                size,
                bone.clone(),
                base,
                Box::new(move |p, m| {
                    proc_part(p, m, [1.30, 0.18, 0.24], bone.clone(), [0.0, 0.30, 0.0]);
                    if is_head {
                        proc_part(
                            p,
                            m,
                            [0.22, 0.22, 0.12],
                            socket.clone(),
                            [-0.30, 0.18, 0.58],
                        );
                        proc_part(p, m, [0.22, 0.22, 0.12], socket, [0.30, 0.18, 0.58]);
                        p.spawn((
                            Mesh3d(m.add(Cuboid::new(0.80, 0.26, 0.45))),
                            MeshMaterial3d(bone),
                            Transform::from_xyz(0.0, -0.45, 0.35),
                            ProcJaw {
                                rest: Vec3::new(0.0, -0.45, 0.35),
                                speed: 9.0,
                                amp: 0.25,
                            },
                        ));
                    }
                }),
            )
        }
        EnemyKind::Skeleton | EnemyKind::AngryBones => {
            let is_angry = kind == EnemyKind::AngryBones;
            let base = if is_angry {
                Color::srgb(0.84, 0.78, 0.68)
            } else {
                Color::srgb(0.92, 0.90, 0.85)
            };
            let bone = proc_mat(materials, base, None);
            let dark = proc_mat(materials, Color::srgb(0.08, 0.08, 0.08), None);
            let helm = proc_mat(
                materials,
                if is_angry {
                    Color::srgb(0.32, 0.36, 0.44)
                } else {
                    Color::srgb(0.70, 0.68, 0.62)
                },
                None,
            );
            spawn_with(
                commands,
                meshes,
                [0.24, 0.76, 0.22], // Spine core
                bone.clone(),
                base,
                Box::new(move |p, m| {
                    // Ribcage bars & pelvis
                    for ry in [0.24, 0.04, -0.14] {
                        proc_part(p, m, [0.64, 0.10, 0.36], bone.clone(), [0.0, ry, 0.0]);
                    }
                    proc_part(p, m, [0.54, 0.16, 0.32], bone.clone(), [0.0, -0.36, 0.0]);
                    // Sculpted skull
                    proc_part(p, m, [0.62, 0.62, 0.62], bone.clone(), [0.0, 0.72, 0.0]);
                    proc_part(p, m, [0.14, 0.14, 0.08], dark.clone(), [-0.16, 0.76, 0.32]);
                    proc_part(p, m, [0.14, 0.14, 0.08], dark, [0.16, 0.76, 0.32]);
                    if is_angry {
                        proc_part(p, m, [0.68, 0.22, 0.68], helm.clone(), [0.0, 0.96, 0.0]);
                        // Rusty buckler accent
                        proc_part(p, m, [0.12, 0.52, 0.52], helm, [-0.56, 0.05, 0.16]);
                    }
                    // Swinging bone arms & legs
                    for &(x, y, z, phase) in &[
                        (-0.46, 0.05, 0.12, 0.0),
                        (0.46, 0.05, 0.12, std::f32::consts::PI),
                        (-0.18, -0.72, 0.0, std::f32::consts::PI),
                        (0.18, -0.72, 0.0, 0.0),
                    ] {
                        let rest = Vec3::new(x, y, z);
                        p.spawn((
                            Mesh3d(m.add(Cuboid::new(0.20, 0.68, 0.20))),
                            MeshMaterial3d(bone.clone()),
                            Transform::from_translation(rest),
                            ProcLimb {
                                phase,
                                rest,
                                amp: 0.45,
                            },
                        ));
                    }
                }),
            )
        }
        EnemyKind::DarkCaster => {
            let base = Color::srgb(0.24, 0.16, 0.38);
            let robe = proc_mat(materials, base, None);
            let bone = proc_mat(materials, Color::srgb(0.90, 0.88, 0.80), None);
            let orb = proc_mat(
                materials,
                Color::srgb(0.65, 0.35, 1.0),
                Some(LinearRgba::rgb(0.5, 0.2, 0.9)),
            );
            spawn_with(
                commands,
                meshes,
                [0.72, 1.15, 0.50],
                robe.clone(),
                base,
                Box::new(move |p, m| {
                    // Hooded skull
                    proc_part(p, m, [0.58, 0.56, 0.56], bone, [0.0, 0.84, 0.04]);
                    proc_part(p, m, [0.68, 0.32, 0.66], robe.clone(), [0.0, 1.02, -0.02]);
                    // Glowing staff
                    proc_part(p, m, [0.12, 1.35, 0.12], robe, [0.52, 0.15, 0.35]);
                    proc_part(p, m, [0.26, 0.26, 0.26], orb, [0.52, 0.90, 0.35]);
                }),
            )
        }
        EnemyKind::CursedSkull => {
            let base = Color::srgb(0.86, 0.84, 0.76);
            let skull = proc_mat(materials, base, None);
            let glow = proc_mat(
                materials,
                Color::srgb(0.45, 0.85, 1.0),
                Some(LinearRgba::rgb(0.3, 0.7, 1.0)),
            );
            spawn_with(
                commands,
                meshes,
                [1.1, 1.0, 1.0],
                skull.clone(),
                base,
                Box::new(move |p, m| {
                    proc_part(p, m, [0.24, 0.24, 0.12], glow.clone(), [-0.26, 0.12, 0.52]);
                    proc_part(p, m, [0.24, 0.24, 0.12], glow.clone(), [0.26, 0.12, 0.52]);
                    proc_part(p, m, [0.76, 0.26, 0.45], skull, [0.0, -0.48, 0.18]);
                    proc_part(p, m, [0.95, 0.45, 0.45], glow, [0.0, 0.55, -0.15]);
                }),
            )
        }
        EnemyKind::OldMan => {
            let base = Color::srgb(0.46, 0.36, 0.30);
            let robe = proc_mat(materials, base, None);
            let skin = proc_mat(materials, Color::srgb(0.88, 0.70, 0.56), None);
            let beard = proc_mat(materials, Color::srgb(0.92, 0.92, 0.94), None);
            spawn_with(
                commands,
                meshes,
                [0.70, 1.10, 0.46],
                robe.clone(),
                base,
                Box::new(move |p, m| {
                    proc_part(p, m, [0.58, 0.58, 0.58], skin, [0.0, 0.84, 0.0]);
                    proc_part(p, m, [0.64, 0.24, 0.64], robe, [0.0, 1.06, -0.02]);
                    // White beard
                    proc_part(p, m, [0.44, 0.42, 0.16], beard, [0.0, 0.52, 0.32]);
                }),
            )
        }
        EnemyKind::FireImp => {
            let base = Color::srgb(0.92, 0.38, 0.10);
            let body = proc_mat(materials, base, Some(LinearRgba::rgb(0.4, 0.12, 0.02)));
            let horn = proc_mat(materials, Color::srgb(0.24, 0.10, 0.08), None);
            let eye = proc_mat(
                materials,
                Color::srgb(1.0, 0.92, 0.25),
                Some(LinearRgba::rgb(0.9, 0.7, 0.1)),
            );
            spawn_with(
                commands,
                meshes,
                [0.65, 0.78, 0.46],
                body.clone(),
                base,
                Box::new(move |p, m| {
                    proc_part(p, m, [0.56, 0.52, 0.52], body.clone(), [0.0, 0.65, 0.0]);
                    proc_part(p, m, [0.14, 0.30, 0.14], horn.clone(), [-0.22, 0.98, 0.0]);
                    proc_part(p, m, [0.14, 0.30, 0.14], horn, [0.22, 0.98, 0.0]);
                    proc_part(p, m, [0.12, 0.12, 0.08], eye.clone(), [-0.14, 0.70, 0.27]);
                    proc_part(p, m, [0.12, 0.12, 0.08], eye, [0.14, 0.70, 0.27]);
                    proc_part(p, m, [0.14, 0.14, 0.55], body, [0.0, -0.20, -0.45]);
                }),
            )
        }
        EnemyKind::Demon | EnemyKind::VoodooDemon => {
            let is_voodoo = kind == EnemyKind::VoodooDemon;
            let base = if is_voodoo {
                Color::srgb(0.65, 0.14, 0.32)
            } else {
                Color::srgb(0.84, 0.16, 0.16)
            };
            let body = proc_mat(materials, base, None);
            let wing = proc_mat(materials, Color::srgb(0.32, 0.06, 0.10), None);
            let horn = proc_mat(materials, Color::srgb(0.18, 0.16, 0.16), None);
            let eye = proc_mat(
                materials,
                Color::srgb(1.0, 0.86, 0.25),
                Some(LinearRgba::rgb(0.85, 0.6, 0.1)),
            );
            let doll = proc_mat(materials, Color::srgb(0.76, 0.58, 0.42), None);
            let doll_shirt = proc_mat(materials, Color::srgb(0.28, 0.55, 0.32), None);
            spawn_with(
                commands,
                meshes,
                [0.75, 0.95, 0.52],
                body.clone(),
                base,
                Box::new(move |p, m| {
                    proc_part(p, m, [0.62, 0.58, 0.58], body.clone(), [0.0, 0.76, 0.0]);
                    // Glowing eyes
                    proc_part(p, m, [0.12, 0.12, 0.08], eye.clone(), [-0.16, 0.80, 0.30]);
                    proc_part(p, m, [0.12, 0.12, 0.08], eye, [0.16, 0.80, 0.30]);
                    // Curved horns & barbed tail
                    proc_part_rot(
                        p,
                        m,
                        [0.16, 0.38, 0.16],
                        horn.clone(),
                        [-0.24, 1.14, 0.0],
                        Quat::from_rotation_z(0.25),
                    );
                    proc_part_rot(
                        p,
                        m,
                        [0.16, 0.38, 0.16],
                        horn,
                        [0.24, 1.14, 0.0],
                        Quat::from_rotation_z(-0.25),
                    );
                    proc_part(p, m, [0.14, 0.14, 0.65], body, [0.0, -0.25, -0.52]);
                    // Flapping leathery wings
                    for side in [-1.0_f32, 1.0_f32] {
                        let rest = Vec3::new(side * 0.75, 0.32, -0.22);
                        let base_rot = Quat::from_rotation_y(-side * 0.35);
                        p.spawn((
                            Mesh3d(m.add(Cuboid::new(0.95, 0.65, 0.10))),
                            MeshMaterial3d(wing.clone()),
                            Transform::from_translation(rest).with_rotation(base_rot),
                            ProcWing {
                                side,
                                rest,
                                base_rot,
                                speed: 12.0,
                                amp: 0.50,
                            },
                        ));
                    }
                    // Swaying Guide Voodoo Doll carried underneath by VoodooDemon
                    if is_voodoo {
                        p.spawn((
                            Mesh3d(m.add(Cuboid::new(0.28, 0.28, 0.28))),
                            MeshMaterial3d(doll),
                            Transform::from_xyz(0.0, -0.82, 0.05),
                            ProcTendril {
                                side: 1.0,
                                rest: Vec3::new(0.0, -0.82, 0.05),
                                base_rot: Quat::IDENTITY,
                                speed: 4.5,
                                amp: 0.25,
                            },
                        ));
                        p.spawn((
                            Mesh3d(m.add(Cuboid::new(0.30, 0.34, 0.22))),
                            MeshMaterial3d(doll_shirt),
                            Transform::from_xyz(0.0, -1.12, 0.05),
                            ProcTendril {
                                side: 1.0,
                                rest: Vec3::new(0.0, -1.12, 0.05),
                                base_rot: Quat::IDENTITY,
                                speed: 4.5,
                                amp: 0.25,
                            },
                        ));
                    }
                }),
            )
        }
        _ => return None,
    };

    Some(entity)
}

#[allow(clippy::too_many_arguments)]
fn apply_msgs(
    mut commands: Commands,
    mut msgs: MessageReader<EnemyMsg>,
    rigs: Option<ResMut<EnemyRigs>>,
    mut ents: ResMut<EnemyEntities>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut q: Query<(&mut EnemyInterp, &mut Transform, &mut ClientEnemy)>,
    enemy_audio: Option<Res<EnemyAudio>>,
    settings: Option<Res<crate::settings::Settings>>,
    focus: Option<Res<crate::audio::AudioFocusState>>,
) {
    let Some(mut rigs) = rigs else { return };
    let rigs = &mut *rigs;
    for m in msgs.read() {
        match *m {
            EnemyMsg::Spawned { id, kind, pos } => {
                if let Some(old) = ents.0.remove(&id) {
                    commands.entity(old).despawn();
                }
                if let Some(e) = spawn_procedural_enemy(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    id,
                    kind,
                    pos,
                ) {
                    ents.0.insert(id, e);
                    continue;
                }
                let Some((rig, dir)) = rigs.rigs.get(&kind) else {
                    continue;
                };
                let (e, handle) = spawn_rig_with(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    rig,
                    dir,
                    &mut rigs.cache,
                );
                let base = tint_for(kind);
                if kind == EnemyKind::Slime || kind == EnemyKind::LavaSlime {
                    if let Some(mut m) = materials.get_mut(&handle) {
                        m.alpha_mode = AlphaMode::Blend;
                    }
                }
                if let Some(mut m) = materials.get_mut(&handle) {
                    m.base_color = base;
                }
                commands.entity(e).insert((
                    ClientEnemy {
                        id,
                        kind,
                        hp: i32::MAX,
                        moving: false,
                        vel: Vec3::ZERO,
                        hurt: 0,
                        telegraph_t: 0.0,
                    },
                    EnemyInterp {
                        from: pos,
                        to: pos,
                        t: 1.0,
                    },
                    EnemyMat { handle, base },
                    Transform::from_translation(pos).with_scale(Vec3::splat(model_scale(kind))),
                    DespawnOnExit(AppState::InGame),
                ));
                ents.0.insert(id, e);
            }
            EnemyMsg::Moved { id, pos, vel, hp } => {
                let Some(&e) = ents.0.get(&id) else { continue };
                let Ok((mut interp, mut tf, mut en)) = q.get_mut(e) else {
                    continue;
                };
                let was_moving = en.moving;
                en.moving = pos.distance(interp.to) > MOVE_MIN;
                en.vel = vel;
                if en.kind == EnemyKind::Slime && !was_moving && en.moving {
                    if !crate::audio::focus_muted(settings.as_deref(), focus.as_deref()) {
                        if let Some(ref audio) = enemy_audio {
                            if let Some(ref h) = audio.slime_hop {
                                let gain = settings
                                    .as_ref()
                                    .map_or(0.75, |s| crate::settings::volume_gain(s.sound_volume));
                                if gain > 0.0 {
                                    commands.spawn((
                                        AudioPlayer(h.clone()),
                                        PlaybackSettings::DESPAWN
                                            .with_volume(bevy::audio::Volume::Linear(gain)),
                                    ));
                                }
                            }
                        }
                    }
                }
                if hp < en.hp && en.hp != i32::MAX {
                    en.hurt = HURT_FRAMES;
                }
                en.hp = hp;
                let from = interp.pos();
                if hp > 0
                    && should_yaw_toward(en.kind)
                    && let Some(yaw) = yaw_toward(interp.to, pos)
                {
                    tf.rotation = Quat::from_rotation_y(yaw);
                }
                *interp = EnemyInterp {
                    from,
                    to: pos,
                    t: 0.0,
                };
            }
            EnemyMsg::Despawned { id } => {
                if let Some(e) = ents.0.remove(&id) {
                    commands.entity(e).despawn();
                }
            }
            EnemyMsg::Telegraph { id, .. } => {
                if let Some(&e) = ents.0.get(&id)
                    && let Ok((_, _, mut en)) = q.get_mut(e)
                {
                    en.telegraph_t = TELEGRAPH_SECS;
                }
            }
        }
    }
}

fn animate(
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player: Option<Res<super::player::LocalPlayer>>,
    mut pool: Option<ResMut<super::particles::ParticlePool>>,
    settings: Option<Res<crate::settings::Settings>>,
    mut q: Query<(
        &mut ClientEnemy,
        &mut EnemyInterp,
        &mut Transform,
        Option<&mut RigAnim>,
        Option<&EnemyMat>,
        Option<&Children>,
    )>,
    mut wing_q: Query<(&BirdWing, &mut Transform), Without<ClientEnemy>>,
    mut proc_wing_q: Query<(&ProcWing, &mut Transform), (Without<ClientEnemy>, Without<BirdWing>)>,
    mut proc_limb_q: Query<
        (&ProcLimb, &mut Transform),
        (Without<ClientEnemy>, Without<BirdWing>, Without<ProcWing>),
    >,
    mut proc_jaw_q: Query<
        (&ProcJaw, &mut Transform),
        (
            Without<ClientEnemy>,
            Without<BirdWing>,
            Without<ProcWing>,
            Without<ProcLimb>,
        ),
    >,
    mut proc_tendril_q: Query<
        (&ProcTendril, &mut Transform),
        (
            Without<ClientEnemy>,
            Without<BirdWing>,
            Without<ProcWing>,
            Without<ProcLimb>,
            Without<ProcJaw>,
        ),
    >,
) {
    let dt = time.delta_secs();
    let elapsed = time.elapsed_secs();
    let player_pos = player.as_ref().map(|p| Vec3::from(p.0.body.pos));
    let s = settings.as_deref().copied().unwrap_or_default();
    for (mut e, mut interp, mut tf, mut anim, mat, children) in &mut q {
        interp.advance(dt);
        tf.translation = interp.pos();
        e.telegraph_t = (e.telegraph_t - dt).max(0.0);
        tf.scale = Vec3::splat(model_scale(e.kind) * telegraph_scale_factor(e.telegraph_t));
        if e.kind == EnemyKind::DungeonGuardian {
            tf.rotation = Quat::from_rotation_y(elapsed * 10.0);
        } else if e.kind == EnemyKind::SkeletronHead && e.vel.length() > 0.20 {
            // Spin animation / rotation when Head is in spin phase!
            tf.rotation = Quat::from_rotation_y(elapsed * 12.0);
        } else if matches!(e.kind, EnemyKind::CursedSkull | EnemyKind::FireImp) {
            tf.translation.y += (elapsed * 4.2 + e.id.0 as f32).sin() * 0.12;
        }
        if let Some(children) = children {
            if e.kind == EnemyKind::Bird {
                let airborne = is_bird_airborne(e.vel, interp.from, interp.to);
                let flap = (elapsed * 16.0).sin() * 0.55;
                for child in children.iter() {
                    if let Ok((wing, mut w_tf)) = wing_q.get_mut(child) {
                        if airborne {
                            w_tf.translation = Vec3::new(wing.side * 0.54, 0.14 + flap * 0.08, 0.0);
                            w_tf.rotation = Quat::from_rotation_z(wing.side * flap);
                        } else {
                            w_tf.translation = Vec3::new(wing.side * 0.34, 0.02, -0.02);
                            w_tf.rotation = Quat::from_rotation_z(wing.side * -1.48);
                        }
                    }
                }
            }
            for child in children.iter() {
                if let Ok((pw, mut c_tf)) = proc_wing_q.get_mut(child) {
                    let flap = (elapsed * pw.speed + e.id.0 as f32).sin() * pw.amp;
                    c_tf.translation = pw.rest + Vec3::Y * (flap * 0.08);
                    c_tf.rotation = pw.base_rot * Quat::from_rotation_z(pw.side * flap);
                } else if let Ok((pl, mut c_tf)) = proc_limb_q.get_mut(child) {
                    let swing = if e.moving {
                        (elapsed * 10.0 + pl.phase).sin() * pl.amp
                    } else {
                        (elapsed * 2.0 + pl.phase).sin() * 0.06
                    };
                    c_tf.translation = pl.rest;
                    c_tf.rotation = Quat::from_rotation_x(swing);
                } else if let Ok((pj, mut c_tf)) = proc_jaw_q.get_mut(child) {
                    let chomp = (elapsed * pj.speed + e.id.0 as f32).sin().abs() * pj.amp;
                    c_tf.translation = pj.rest - Vec3::Y * chomp;
                } else if let Ok((pt, mut c_tf)) = proc_tendril_q.get_mut(child) {
                    let sway = (elapsed * pt.speed + e.id.0 as f32).sin() * pt.amp;
                    c_tf.translation = pt.rest;
                    c_tf.rotation = pt.base_rot * Quat::from_rotation_y(pt.side * sway);
                }
            }
        }
        // Subtle ambient particles respecting ParticlePool cap & reduced_motion
        if !s.reduced_motion
            && let Some(ref mut p) = pool
            && p.entries.len() < super::particles::max_particles_for(&s).saturating_sub(8)
        {
            let phase = (elapsed * 60.0 + e.id.0 as f32 * 13.0) as u32;
            if phase.is_multiple_of(12) {
                let p_color = match e.kind {
                    EnemyKind::FireImp
                    | EnemyKind::Hellbat
                    | EnemyKind::Demon
                    | EnemyKind::VoodooDemon => Some(Color::srgb(1.0, 0.45, 0.1)),
                    EnemyKind::EaterOfWorldsHead
                    | EnemyKind::EaterOfWorldsBody
                    | EnemyKind::EaterOfWorldsTail => Some(Color::srgb(0.65, 0.95, 0.25)),
                    EnemyKind::DarkCaster | EnemyKind::CursedSkull | EnemyKind::BoneSerpentHead => {
                        Some(Color::srgb(0.45, 0.85, 1.0))
                    }
                    _ => None,
                };
                if let Some(color) = p_color {
                    p.spawn(
                        tf.translation + Vec3::new(((elapsed * 7.0).sin()) * 0.35, 0.3, 0.0),
                        Vec3::new(0.0, 0.8, 0.0),
                        color,
                        0.12,
                        0.35,
                        -1.5,
                        &s,
                    );
                }
            }
        }
        if let Some(ref mut anim) = anim {
            anim.play(anim_for(e.kind, e.moving));
        }
        let approach_flash = is_contact_enemy(e.kind)
            && e.hp > 0
            && player_pos.is_some_and(|pp| tf.translation.distance(pp) <= APPROACH_FLASH_DIST);
        if let Some(mat) = mat {
            let tint = enemy_tint_for(
                e.kind,
                mat.base,
                e.hurt,
                e.telegraph_t,
                approach_flash,
                elapsed,
            );
            if e.hurt > 0 {
                e.hurt -= 1;
            }
            if let Some(mut m) = materials.get_mut(&mat.handle)
                && m.base_color != tint
            {
                m.base_color = tint;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_enemy_kind_has_model() {
        for kind in ALL_ENEMY_KINDS {
            if !has_procedural_rig(kind) {
                let dir = art_dir().join("models").join(model_dir(kind));
                assert!(dir.is_dir(), "{kind:?}: {}", dir.display());
                assert!(dir.join("rig.ron").is_file(), "{kind:?} has no rig.ron");
            }
        }
        // Distinct families must not share misleading .vox rigs
        assert_ne!(model_dir(EnemyKind::Bunny), model_dir(EnemyKind::Slime));
        assert_ne!(model_dir(EnemyKind::Bird), model_dir(EnemyKind::DemonEye));
        assert_ne!(
            model_dir(EnemyKind::CaveBat),
            model_dir(EnemyKind::DemonEye)
        );
        assert_ne!(
            model_dir(EnemyKind::GiantWorm),
            model_dir(EnemyKind::EaterOfSouls)
        );
        assert_ne!(model_dir(EnemyKind::Skeleton), model_dir(EnemyKind::Zombie));
        assert_ne!(model_dir(EnemyKind::OldMan), model_dir(EnemyKind::Guide));
        assert_ne!(
            model_dir(EnemyKind::ArmsDealer),
            model_dir(EnemyKind::Merchant)
        );

        // Verify ALL 38 EnemyKinds actually spawn an entity when EnemyMsg::Spawned arrives
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .add_plugins(bevy::time::TimePlugin)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<AudioSource>()
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_state::<AppState>()
            .add_plugins(EnemiesPlugin {
                sounds: EnemySounds::default(),
            });
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        for (idx, &kind) in ALL_ENEMY_KINDS.iter().enumerate() {
            app.world_mut().write_message(EnemyMsg::Spawned {
                id: EnemyId((idx + 1) as u32),
                kind,
                pos: Vec3::new(idx as f32, 0.0, 0.0),
            });
        }
        app.update();

        let ents = app.world().resource::<EnemyEntities>();
        for (idx, &kind) in ALL_ENEMY_KINDS.iter().enumerate() {
            assert!(
                ents.0.contains_key(&EnemyId((idx + 1) as u32)),
                "{kind:?} failed to spawn an entity (invisible enemy!)"
            );
        }
    }

    #[test]
    fn eater_model_loads_and_flaps_at_idle() {
        assert!(KINDS.contains(&EnemyKind::EaterOfSouls));
        let dir = art_dir()
            .join("models")
            .join(model_dir(EnemyKind::EaterOfSouls));
        let r = rig::try_load(&dir).unwrap();
        for part in &r.parts {
            crate::art::vox::try_load(&dir.join(&part.file)).unwrap();
        }
        assert_eq!(anim_for(EnemyKind::EaterOfSouls, false), "idle");
        assert_eq!(anim_for(EnemyKind::EaterOfSouls, true), "idle");
        let idle = &r.anims["idle"];
        for wing in ["wing_l", "wing_r"] {
            assert!(r.parts.iter().any(|p| p.name == wing));
            assert_ne!(
                rig::sample(idle, wing, 0.0).0,
                rig::sample(idle, wing, idle.len / 2.0).0
            );
        }
    }

    #[test]
    fn slime_hop_squashes() {
        let r = rig::try_load(&art_dir().join("models/slime")).unwrap();
        let hop = &r.anims["hop"];
        assert!(rig::sample_scale(hop, "body", 0.05).y < 1.0);
        assert!(rig::sample_scale(hop, "body", 0.3).y > 1.0);
    }

    #[test]
    fn anim_picks_by_kind_and_motion() {
        assert_eq!(anim_for(EnemyKind::Slime, true), "hop");
        assert_eq!(anim_for(EnemyKind::Guide, true), "walk");
        assert_eq!(anim_for(EnemyKind::Guide, false), "idle");
        assert_eq!(anim_for(EnemyKind::DemonEye, true), "idle");
    }

    #[test]
    fn enemy_interp_reaches_target() {
        let mut i = EnemyInterp {
            from: Vec3::ZERO,
            to: Vec3::new(1.0, 2.0, 3.0),
            t: 0.0,
        };
        i.advance(0.05);
        assert!(i.pos().x > 0.0 && i.pos().x < 1.0);
        i.advance(0.2);
        assert_eq!(i.pos(), i.to);
    }

    #[test]
    fn enemy_yaw_faces_target() {
        let o = Vec3::ZERO;
        let yaw = yaw_toward(o, Vec3::X).unwrap();
        assert!((Quat::from_rotation_y(yaw) * Vec3::Z).distance(Vec3::X) < 1e-5);
        assert!(yaw_toward(o, Vec3::new(0.0, 5.0, 0.0)).is_none());
    }

    #[test]
    fn slime_hop_triggers_sound() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .add_plugins(bevy::time::TimePlugin)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<AudioSource>()
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_state::<AppState>()
            .add_plugins(EnemiesPlugin {
                sounds: EnemySounds {
                    slime_hop: Some(vec![0; 44]),
                },
            });

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        // Spawn a slime at origin
        app.world_mut().write_message(EnemyMsg::Spawned {
            id: EnemyId(1),
            kind: EnemyKind::Slime,
            pos: Vec3::ZERO,
        });
        app.update();

        // Move the slime significantly (distance > MOVE_MIN) -> triggers hop sound
        app.world_mut().write_message(EnemyMsg::Moved {
            id: EnemyId(1),
            pos: Vec3::new(1.0, 0.5, 0.0),
            vel: Vec3::ZERO,
            hp: 20,
        });
        app.update();

        // Assert an AudioPlayer entity was spawned for the hop
        let mut audio_q = app.world_mut().query::<&AudioPlayer>();
        assert_eq!(audio_q.iter(app.world()).count(), 1);
    }

    #[test]
    fn skeletron_head_and_hand_spawn_and_spin_rotation() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .add_plugins(bevy::time::TimePlugin)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<AudioSource>()
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_state::<AppState>()
            .add_plugins(EnemiesPlugin {
                sounds: EnemySounds::default(),
            });

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        // Spawn head and hand
        app.world_mut().write_message(EnemyMsg::Spawned {
            id: EnemyId(10),
            kind: EnemyKind::SkeletronHead,
            pos: Vec3::new(0.0, 50.0, 0.0),
        });
        app.world_mut().write_message(EnemyMsg::Spawned {
            id: EnemyId(11),
            kind: EnemyKind::SkeletronHand,
            pos: Vec3::new(-6.0, 48.0, 0.0),
        });
        app.update();

        let head_entity = {
            let ents = app.world().resource::<EnemyEntities>();
            assert!(ents.0.contains_key(&EnemyId(10)));
            assert!(ents.0.contains_key(&EnemyId(11)));
            ents.0[&EnemyId(10)]
        };

        // Move head with spin velocity (> 0.20)
        app.world_mut().write_message(EnemyMsg::Moved {
            id: EnemyId(10),
            pos: Vec3::new(0.0, 48.0, 0.0),
            vel: Vec3::new(0.0, -0.30, 0.0),
            hp: 4400,
        });
        app.update();

        let en = app
            .world()
            .entity(head_entity)
            .get::<ClientEnemy>()
            .unwrap();
        assert_eq!(en.kind, EnemyKind::SkeletronHead);
        assert!(en.vel.length() > 0.20, "spin velocity recorded");
    }

    #[test]
    fn all_underworld_kinds_have_models_and_scales() {
        let underworld_kinds = [
            EnemyKind::FireImp,
            EnemyKind::BoneSerpentHead,
            EnemyKind::BoneSerpentBody,
            EnemyKind::BoneSerpentTail,
            EnemyKind::LavaSlime,
            EnemyKind::Hellbat,
            EnemyKind::Demon,
            EnemyKind::VoodooDemon,
        ];
        for kind in underworld_kinds {
            assert!(KINDS.contains(&kind), "{kind:?} must be in KINDS");
            let dir = model_dir(kind);
            assert!(!dir.is_empty(), "{kind:?} model_dir must not be empty");
            let scale = model_scale(kind);
            assert!(scale > 0.0, "{kind:?} model_scale must be positive");
            let tint = tint_for(kind);
            assert_ne!(tint, Color::WHITE, "{kind:?} must have a non-default tint");
        }
        // Verify specific scales
        assert_eq!(model_scale(EnemyKind::FireImp), 1.0);
        assert_eq!(model_scale(EnemyKind::Demon), 1.3);
        assert_eq!(model_scale(EnemyKind::VoodooDemon), 1.3);
        assert_eq!(model_scale(EnemyKind::LavaSlime), 1.1);
        assert_eq!(model_scale(EnemyKind::Hellbat), 0.6);
        assert_eq!(model_scale(EnemyKind::BoneSerpentHead), 0.9);
        assert_eq!(model_scale(EnemyKind::BoneSerpentBody), 0.9);
        assert_eq!(model_scale(EnemyKind::BoneSerpentTail), 0.9);

        // Verify animations
        assert_eq!(anim_for(EnemyKind::LavaSlime, true), "hop");
        assert_eq!(anim_for(EnemyKind::LavaSlime, false), "idle");
        assert_eq!(anim_for(EnemyKind::FireImp, true), "idle");
        assert_eq!(anim_for(EnemyKind::Demon, true), "idle");
        assert_eq!(anim_for(EnemyKind::VoodooDemon, true), "idle");
        assert_eq!(anim_for(EnemyKind::Hellbat, true), "idle");
        assert_eq!(anim_for(EnemyKind::BoneSerpentHead, true), "idle");
    }

    #[test]
    fn bone_serpent_has_procedural_rig() {
        for kind in [
            EnemyKind::BoneSerpentHead,
            EnemyKind::BoneSerpentBody,
            EnemyKind::BoneSerpentTail,
        ] {
            assert!(has_procedural_rig(kind));
        }
    }

    #[test]
    fn telegraph_and_approach_flash_helpers() {
        assert_eq!(telegraph_scale_factor(0.0), 1.0);
        assert!(telegraph_scale_factor(TELEGRAPH_SECS) > 1.0);
        assert_eq!(
            enemy_tint(Color::WHITE, 0, TELEGRAPH_SECS, false, 0.0),
            TELEGRAPH_TINT
        );
        assert_eq!(
            enemy_tint(Color::WHITE, 2, TELEGRAPH_SECS, true, 0.0),
            HURT_TINT
        );
        assert!(is_contact_enemy(EnemyKind::Zombie));
        assert!(!is_contact_enemy(EnemyKind::Guide));
        assert_ne!(enemy_tint(Color::WHITE, 0, 0.0, true, 0.05), Color::WHITE);
    }

    #[test]
    fn critter_visual_mappings_and_non_contact() {
        for kind in [
            EnemyKind::Bunny,
            EnemyKind::Bird,
            EnemyKind::Squirrel,
            EnemyKind::CaveCritter,
        ] {
            assert!(KINDS.contains(&kind), "{kind:?} missing from KINDS");
            assert!(
                !is_contact_enemy(kind),
                "{kind:?} must not be contact enemy"
            );
            assert!(model_scale(kind) < 1.0, "{kind:?} should have small scale");
            assert!(
                has_procedural_rig(kind),
                "{kind:?} should have its own procedural rig"
            );
        }
        assert_eq!(anim_for(EnemyKind::Bunny, true), "hop");
        assert_eq!(anim_for(EnemyKind::Bird, true), "idle");
    }

    #[test]
    fn wof_parts_skip_yaw_toward_rotation() {
        assert!(!should_yaw_toward(EnemyKind::WallOfFleshMouth));
        assert!(!should_yaw_toward(EnemyKind::WallOfFleshEye));
        assert!(should_yaw_toward(EnemyKind::TheHungry));
        assert!(should_yaw_toward(EnemyKind::Leech));
    }

    #[test]
    fn slime_jump_color_stability() {
        let translucent_blue = Color::srgba(0.45, 0.62, 1.0, 0.65);
        let translucent_lava = Color::srgba(1.0, 0.35, 0.1, 0.72);
        for (kind, base) in [
            (EnemyKind::Slime, translucent_blue),
            (EnemyKind::LavaSlime, translucent_lava),
        ] {
            assert_eq!(
                enemy_tint_for(kind, base, 0, TELEGRAPH_SECS, false, 0.05),
                base,
                "{kind:?} must keep base colour during jump telegraph"
            );
            assert_eq!(
                enemy_tint_for(kind, base, 0, 0.0, true, 0.05),
                base,
                "{kind:?} must keep base colour during approach flash"
            );
            let hurt = enemy_tint_for(kind, base, 2, TELEGRAPH_SECS, true, 0.05);
            assert_eq!(hurt, HURT_TINT.with_alpha(base.alpha()));
            assert!((hurt.alpha() - base.alpha()).abs() < 1e-6);
        }

        // Verify in Bevy App: Slime & LavaSlime keep base_color on Telegraph while scaling up
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .add_plugins(bevy::time::TimePlugin)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<AudioSource>()
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_state::<AppState>()
            .add_plugins(EnemiesPlugin {
                sounds: EnemySounds::default(),
            });
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        for (idx, kind) in [EnemyKind::Slime, EnemyKind::LavaSlime].iter().enumerate() {
            let id = EnemyId(100 + idx as u32);
            app.world_mut().write_message(EnemyMsg::Spawned {
                id,
                kind: *kind,
                pos: Vec3::ZERO,
            });
        }
        app.update();

        for (idx, kind) in [EnemyKind::Slime, EnemyKind::LavaSlime].iter().enumerate() {
            let id = EnemyId(100 + idx as u32);
            app.world_mut()
                .write_message(EnemyMsg::Telegraph { id, kind: *kind });
        }
        app.update();

        let ents = app.world().resource::<EnemyEntities>();
        let mats = app.world().resource::<Assets<StandardMaterial>>();
        for (idx, kind) in [EnemyKind::Slime, EnemyKind::LavaSlime].iter().enumerate() {
            let e = ents.0[&EnemyId(100 + idx as u32)];
            let mat = app.world().get::<EnemyMat>(e).unwrap();
            let tf = app.world().get::<Transform>(e).unwrap();
            assert_eq!(mats.get(&mat.handle).unwrap().base_color, tint_for(*kind));
            assert!(tf.scale.y > model_scale(*kind));
        }
    }

    #[test]
    fn bunny_has_four_legs_and_raised_torso() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .add_plugins(bevy::time::TimePlugin)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<AudioSource>()
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_state::<AppState>()
            .add_plugins(EnemiesPlugin {
                sounds: EnemySounds::default(),
            });
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        app.world_mut().write_message(EnemyMsg::Spawned {
            id: EnemyId(201),
            kind: EnemyKind::Bunny,
            pos: Vec3::ZERO,
        });
        app.update();

        let e = app.world().resource::<EnemyEntities>().0[&EnemyId(201)];
        let children = app.world().get::<Children>(e).unwrap();
        let lower_parts: Vec<Vec3> = children
            .iter()
            .filter_map(|c| app.world().get::<Transform>(c))
            .map(|t| t.translation)
            .filter(|p| p.y < -0.20)
            .collect();
        assert!(
            lower_parts.len() >= 4,
            "Bunny should have at least 4 leg/foot parts below torso, got {}",
            lower_parts.len()
        );
        assert!(lower_parts.iter().any(|p| p.x < 0.0 && p.z > 0.0));
        assert!(lower_parts.iter().any(|p| p.x > 0.0 && p.z > 0.0));
        assert!(lower_parts.iter().any(|p| p.x < 0.0 && p.z < 0.0));
        assert!(lower_parts.iter().any(|p| p.x > 0.0 && p.z < 0.0));
    }

    #[test]
    fn bird_wings_fold_when_grounded_and_flap_when_airborne() {
        assert!(!is_bird_airborne(Vec3::ZERO, Vec3::ZERO, Vec3::ZERO));
        assert!(!is_bird_airborne(
            Vec3::new(0.03, 0.0, 0.0),
            Vec3::ZERO,
            Vec3::new(0.03, 0.0, 0.0)
        ));
        assert!(is_bird_airborne(
            Vec3::new(0.0, 0.2, 0.0),
            Vec3::ZERO,
            Vec3::new(0.0, 0.5, 0.0)
        ));

        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .add_plugins(bevy::time::TimePlugin)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<AudioSource>()
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_state::<AppState>()
            .add_plugins(EnemiesPlugin {
                sounds: EnemySounds::default(),
            });
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        app.world_mut().write_message(EnemyMsg::Spawned {
            id: EnemyId(202),
            kind: EnemyKind::Bird,
            pos: Vec3::ZERO,
        });
        app.update();

        let e = app.world().resource::<EnemyEntities>().0[&EnemyId(202)];
        let children: Vec<Entity> = app.world().get::<Children>(e).unwrap().iter().collect();
        let wings: Vec<Entity> = children
            .into_iter()
            .filter(|&c| app.world().get::<BirdWing>(c).is_some())
            .collect();
        assert_eq!(wings.len(), 2, "Bird should have 2 BirdWing children");

        // Grounded: wings folded tightly against the sides
        for &w in &wings {
            let tf = app.world().get::<Transform>(w).unwrap();
            let (_, _, z_angle) = tf.rotation.to_euler(EulerRot::XYZ);
            assert!(
                z_angle.abs() > 1.0,
                "grounded bird wing should be folded flat against side, got z_angle={z_angle}"
            );
            assert!(tf.translation.x.abs() < 0.40);
        }

        // Airborne: wings spread out and flap
        app.world_mut().write_message(EnemyMsg::Moved {
            id: EnemyId(202),
            pos: Vec3::new(0.0, 1.0, 0.0),
            vel: Vec3::new(0.2, 0.3, 0.0),
            hp: 5,
        });
        app.update();

        let mut angles_t1 = Vec::new();
        for &w in &wings {
            let tf = app.world().get::<Transform>(w).unwrap();
            let (_, _, z_angle) = tf.rotation.to_euler(EulerRot::XYZ);
            assert!(
                z_angle.abs() < 1.0,
                "airborne bird wing should spread out, got z_angle={z_angle}"
            );
            assert!(tf.translation.x.abs() > 0.45);
            angles_t1.push(tf.rotation);
        }

        // Advance time and verify wing rotation changes (flapping)
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(80));
        app.update();
        let angles_t2: Vec<Quat> = wings
            .iter()
            .map(|&w| app.world().get::<Transform>(w).unwrap().rotation)
            .collect();
        assert_ne!(angles_t1, angles_t2, "airborne bird wings should flap");
    }
}
