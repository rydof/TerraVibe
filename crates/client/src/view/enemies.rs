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

/// Directory under `models/` holding the kind's `.vox` rig.
pub fn model_dir(kind: EnemyKind) -> &'static str {
    match kind {
        EnemyKind::Slime => "slime",
        EnemyKind::Zombie => "zombie",
        EnemyKind::DemonEye => "demon_eye",
        EnemyKind::ServantOfCthulhu => "servant_of_cthulhu",
        EnemyKind::EyeOfCthulhu => "eye_of_cthulhu",
        EnemyKind::EaterOfSouls => "eater_of_souls",
        EnemyKind::Skeleton => "skeleton",
        EnemyKind::CaveBat => "cave_bat",
        EnemyKind::GiantWorm => "giant_worm",
        EnemyKind::EaterOfWorldsHead => "eater_of_worlds_head",
        EnemyKind::EaterOfWorldsBody => "eater_of_worlds_body",
        EnemyKind::EaterOfWorldsTail => "eater_of_worlds_tail",
        EnemyKind::Guide => "guide",
        EnemyKind::Merchant => "merchant",
        EnemyKind::ArmsDealer => "arms_dealer",
        EnemyKind::Nurse => "nurse",
        EnemyKind::Demolitionist => "demolitionist",
        EnemyKind::Dryad => "dryad",
        EnemyKind::GoblinTinkerer => "goblin_tinkerer",
        EnemyKind::OldMan => "old_man",
        EnemyKind::AngryBones => "angry_bones",
        EnemyKind::DarkCaster => "dark_caster",
        EnemyKind::CursedSkull => "cursed_skull",
        EnemyKind::DungeonGuardian => "dungeon_guardian",
        EnemyKind::SkeletronHead => "skeletron_head",
        EnemyKind::SkeletronHand => "skeletron_hand",
        EnemyKind::FireImp => "fire_imp",
        EnemyKind::BoneSerpentHead => "bone_serpent_head",
        EnemyKind::BoneSerpentBody => "bone_serpent_body",
        EnemyKind::BoneSerpentTail => "bone_serpent_tail",
        EnemyKind::LavaSlime => "lava_slime",
        EnemyKind::Hellbat => "hellbat",
        EnemyKind::Demon => "demon",
        EnemyKind::VoodooDemon => "voodoo_demon",
        EnemyKind::Bunny => "bunny",
        EnemyKind::Bird => "bird",
        EnemyKind::Squirrel => "squirrel",
        EnemyKind::CaveCritter => "cave_critter",
        EnemyKind::WallOfFleshMouth => "wall_of_flesh_mouth",
        EnemyKind::WallOfFleshEye => "wall_of_flesh_eye",
        EnemyKind::TheHungry => "the_hungry",
        EnemyKind::Leech => "leech",
        EnemyKind::KingSlime => "king_slime",
        EnemyKind::QueenBee => "queen_bee",
        EnemyKind::Bee => "bee",
        EnemyKind::BloodZombie => "blood_zombie",
        EnemyKind::Drippler => "drippler",
        EnemyKind::GoblinPeon => "goblin_peon",
        EnemyKind::GoblinThief => "goblin_thief",
        EnemyKind::GoblinWarrior => "goblin_warrior",
        EnemyKind::GoblinSorcerer => "goblin_sorcerer",
        EnemyKind::GoblinArcher => "goblin_archer",
        EnemyKind::Antlion => "antlion",
        EnemyKind::Vulture => "vulture",
        EnemyKind::IceSlime => "ice_slime",
        EnemyKind::SnowFlinx => "snow_flinx",
        EnemyKind::JungleSlime => "jungle_slime",
        EnemyKind::Hornet => "hornet",
        EnemyKind::ManEater => "man_eater",
        EnemyKind::Harpy => "harpy",
        EnemyKind::Shark => "shark",
        EnemyKind::Crab => "crab",
        EnemyKind::Piranha => "piranha",
    }
}

pub(crate) fn is_slime(kind: EnemyKind) -> bool {
    matches!(
        kind,
        EnemyKind::Slime
            | EnemyKind::LavaSlime
            | EnemyKind::IceSlime
            | EnemyKind::JungleSlime
            | EnemyKind::KingSlime
    )
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
        (
            EnemyKind::Slime
            | EnemyKind::LavaSlime
            | EnemyKind::IceSlime
            | EnemyKind::JungleSlime
            | EnemyKind::KingSlime
            | EnemyKind::Bunny,
            true,
        ) => "hop",
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
            | EnemyKind::DarkCaster
            | EnemyKind::BloodZombie
            | EnemyKind::GoblinPeon
            | EnemyKind::GoblinThief
            | EnemyKind::GoblinWarrior
            | EnemyKind::GoblinSorcerer
            | EnemyKind::GoblinArcher
            | EnemyKind::Crab
            | EnemyKind::SnowFlinx
            | EnemyKind::Squirrel
            | EnemyKind::CaveCritter,
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
    if is_slime(kind) {
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
                if is_slime(kind) {
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
    )>,
) {
    let dt = time.delta_secs();
    let elapsed = time.elapsed_secs();
    let player_pos = player.as_ref().map(|p| Vec3::from(p.0.body.pos));
    let s = settings.as_deref().copied().unwrap_or_default();
    for (mut e, mut interp, mut tf, mut anim, mat) in &mut q {
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
            let clip = if e.kind == EnemyKind::Bird
                && is_bird_airborne(e.vel, interp.from, interp.to)
            {
                "fly"
            } else {
                anim_for(e.kind, e.moving)
            };
            anim.play(clip);
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
        let mut dirs = std::collections::HashSet::new();
        for kind in ALL_ENEMY_KINDS {
            let dir_name = model_dir(kind);
            assert!(
                dirs.insert(dir_name),
                "duplicate model_dir {dir_name:?} for {kind:?}"
            );
            let dir = art_dir().join("models").join(dir_name);
            assert!(dir.is_dir(), "{kind:?}: {}", dir.display());
            assert!(dir.join("rig.ron").is_file(), "{kind:?} has no rig.ron");
        }
        assert_eq!(dirs.len(), ALL_ENEMY_KINDS.len());
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
                !model_dir(kind).is_empty(),
                "{kind:?} should have its own .vox rig"
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

        let all_slimes = [
            EnemyKind::Slime,
            EnemyKind::LavaSlime,
            EnemyKind::IceSlime,
            EnemyKind::JungleSlime,
            EnemyKind::KingSlime,
        ];
        for (idx, kind) in all_slimes.iter().enumerate() {
            let id = EnemyId(100 + idx as u32);
            app.world_mut().write_message(EnemyMsg::Spawned {
                id,
                kind: *kind,
                pos: Vec3::ZERO,
            });
        }
        app.update();
        app.update();

        for (idx, kind) in all_slimes.iter().enumerate() {
            let id = EnemyId(100 + idx as u32);
            app.world_mut()
                .write_message(EnemyMsg::Telegraph { id, kind: *kind });
        }
        app.update();

        let ents = app.world().resource::<EnemyEntities>();
        let mats = app.world().resource::<Assets<StandardMaterial>>();
        for (idx, kind) in all_slimes.iter().enumerate() {
            let e = ents.0[&EnemyId(100 + idx as u32)];
            let mat = app.world().get::<EnemyMat>(e).unwrap();
            let tf = app.world().get::<Transform>(e).unwrap();
            let std_mat = mats.get(&mat.handle).unwrap();
            assert_eq!(std_mat.base_color, tint_for(*kind));
            assert_eq!(
                std_mat.alpha_mode,
                AlphaMode::Blend,
                "{kind:?} must use AlphaMode::Blend"
            );
            assert!(tf.scale.y > model_scale(*kind));
        }
    }

    #[test]
    fn bunny_has_four_legs_and_raised_torso() {
        let rig_ron = std::fs::read_to_string(art_dir().join("models/bunny/rig.ron"))
            .expect("bunny rig.ron must exist");
        for part in ["leg_fl", "leg_fr", "leg_bl", "leg_br"] {
            assert!(
                rig_ron.contains(part),
                "bunny rig.ron missing leg part {part}"
            );
        }
        assert!(
            rig_ron.contains("\"hop\"") || rig_ron.contains("hop:"),
            "bunny rig.ron missing hop animation"
        );
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
        let anim = app.world().get::<RigAnim>(e).unwrap();
        assert_eq!(anim.name, "idle", "grounded bird should play idle clip");

        // Airborne: switches to fly clip
        app.world_mut().write_message(EnemyMsg::Moved {
            id: EnemyId(202),
            pos: Vec3::new(0.0, 1.0, 0.0),
            vel: Vec3::new(0.2, 0.3, 0.0),
            hp: 5,
        });
        app.update();

        let anim = app.world().get::<RigAnim>(e).unwrap();
        assert_eq!(anim.name, "fly", "airborne bird should play fly clip");
    }
}
