//! Per-class held-item animations for first-person and third-person views.

use std::f32::consts::FRAC_PI_2;

use bevy::prelude::*;
use terravibe_sim::item::{
    BEE_GUN, CLOCKWORK_ASSAULT_RIFLE, COPPER_SHORTSWORD, DARK_LANCE, FIRECRACKER, FLINTLOCK_PISTOL,
    HANDGUN, ItemClass, ItemId, LASER_RIFLE, MINISHARK, MUSKET, PHOENIX_BLASTER, SPACE_GUN, SPEAR,
    STAR_CANNON, TORCH, item_data,
};

/// Held-item view-space rest anchor (in camera local space).
pub(crate) const HELD_POS: Vec3 = Vec3::new(0.42, -0.38, -0.8);

/// Minimum allowed camera-space Z distance (more negative = farther in front of camera).
pub(crate) const MIN_CAM_Z: f32 = -0.35;

/// Per-class held-item animation archetype.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HeldAnimKind {
    SwordSlash,
    ShortswordStab,
    SpearThrust,
    GunFire,
    WhipSnap,
    ToolMine,
    BowDraw,
    MagicCast,
    ConsumableRaise,
    BlockTap,
    TorchHold,
}

#[cfg(test)]
impl HeldAnimKind {
    pub const ALL: [Self; 11] = [
        Self::SwordSlash,
        Self::ShortswordStab,
        Self::SpearThrust,
        Self::GunFire,
        Self::WhipSnap,
        Self::ToolMine,
        Self::BowDraw,
        Self::MagicCast,
        Self::ConsumableRaise,
        Self::BlockTap,
        Self::TorchHold,
    ];
}

/// Runtime animation state sampled for both first-person and third-person held items.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeldAnimState {
    /// Equip transition progress in `[0.0, 1.0]` (`0.0` = lowered, `1.0` = settled).
    pub equip_t: f32,
    /// Elapsed idle time in seconds.
    pub idle_t: f32,
    /// Walk cycle phase in radians.
    pub walk_phase: f32,
    /// Vertical velocity in blocks/tick.
    pub vel_y: f32,
    /// Hit/damage recoil impulse in `[0.0, 1.0]`.
    pub recoil_t: f32,
    /// Normalized active-use progress in `0.0..1.0`, or `None` when idle.
    pub use_t: Option<f32>,
    /// Post-use kick impulse in `[0.0, 1.0]`.
    pub kick_t: f32,
}

impl Default for HeldAnimState {
    fn default() -> Self {
        Self {
            equip_t: 1.0,
            idle_t: 0.0,
            walk_phase: 0.0,
            vel_y: 0.0,
            recoil_t: 0.0,
            use_t: None,
            kick_t: 0.0,
        }
    }
}

/// Shared client-side tracker updated each frame from player movement, HP, and item use.
#[derive(Resource, Default)]
pub struct HeldAnimTracker {
    pub state: HeldAnimState,
    pub last_item: Option<ItemId>,
    pub last_hp: Option<i32>,
}

/// Maps an [`ItemId`] to its [`HeldAnimKind`].
pub fn held_anim_kind(id: ItemId) -> HeldAnimKind {
    match id {
        COPPER_SHORTSWORD => HeldAnimKind::ShortswordStab,
        SPEAR | DARK_LANCE => HeldAnimKind::SpearThrust,
        FIRECRACKER => HeldAnimKind::WhipSnap,
        FLINTLOCK_PISTOL
        | MUSKET
        | MINISHARK
        | HANDGUN
        | PHOENIX_BLASTER
        | CLOCKWORK_ASSAULT_RIFLE
        | SPACE_GUN
        | STAR_CANNON
        | BEE_GUN
        | LASER_RIFLE => HeldAnimKind::GunFire,
        _ => {
            let d = item_data(id);
            if d.name.contains("Whip") {
                HeldAnimKind::WhipSnap
            } else {
                held_anim_kind_for_class(d.class, id == TORCH)
            }
        }
    }
}

/// Maps an [`ItemClass`] (and whether it is a torch) to its [`HeldAnimKind`].
pub fn held_anim_kind_for_class(class: ItemClass, is_torch: bool) -> HeldAnimKind {
    if is_torch {
        return HeldAnimKind::TorchHold;
    }
    match class {
        ItemClass::Melee => HeldAnimKind::SwordSlash,
        ItemClass::Tool => HeldAnimKind::ToolMine,
        ItemClass::Ranged => HeldAnimKind::BowDraw,
        ItemClass::Magic => HeldAnimKind::MagicCast,
        ItemClass::Consumable => HeldAnimKind::ConsumableRaise,
        ItemClass::Block
        | ItemClass::Placeable
        | ItemClass::Wall
        | ItemClass::Ammo
        | ItemClass::Armor
        | ItemClass::Accessory
        | ItemClass::Other => HeldAnimKind::BlockTap,
    }
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn rest_pose(kind: HeldAnimKind) -> (Vec3, Quat) {
    match kind {
        HeldAnimKind::TorchHold => (
            HELD_POS + Vec3::new(-0.02, 0.04, -0.04),
            Quat::from_rotation_x(-0.20)
                * Quat::from_rotation_z(0.15)
                * Quat::from_rotation_y(-0.25),
        ),
        HeldAnimKind::BowDraw => (
            HELD_POS + Vec3::new(-0.02, 0.02, -0.02),
            Quat::from_rotation_y(1.45) * Quat::from_rotation_z(-0.10),
        ),
        _ => (
            HELD_POS,
            Quat::from_rotation_y(1.85)
                * Quat::from_rotation_z(-0.20)
                * Quat::from_rotation_x(-0.15),
        ),
    }
}

/// Computes the camera-local [`Transform`] for the first-person held item.
///
/// Guarantees `translation.z <= -0.35` across all kinds, states, and use phases.
pub fn compute_fp_transform(kind: HeldAnimKind, state: &HeldAnimState) -> Transform {
    let (base_pos, base_rot) = rest_pose(kind);
    let mut pos = base_pos;
    let mut rot = base_rot;

    // Equip rise from lowered off-screen position.
    let equip = smoothstep(state.equip_t);
    let unequip = 1.0 - equip;
    pos += Vec3::new(0.05 * unequip, -0.32 * unequip, 0.12 * unequip);
    rot *= Quat::from_rotation_x(-0.45 * unequip);

    // Subtle idle breathing & rotational sway.
    pos.x += (state.idle_t * 1.3).cos() * 0.005;
    pos.y += (state.idle_t * 2.0).sin() * 0.009;
    rot *= Quat::from_rotation_z((state.idle_t * 1.1).sin() * 0.025)
        * Quat::from_rotation_x((state.idle_t * 1.7).cos() * 0.015);

    // Walk figure-eight bob (zero when walk_phase == 0.0).
    pos.x += state.walk_phase.sin() * 0.020;
    pos.y -= (state.walk_phase * 2.0).sin().abs() * 0.018;

    // Vertical velocity lag.
    pos.y += (state.vel_y * -0.18).clamp(-0.08, 0.08);

    // Damage recoil jolt.
    let recoil = state.recoil_t.clamp(0.0, 1.0);
    pos += Vec3::new(0.03 * recoil, -0.05 * recoil, 0.08 * recoil);
    rot *= Quat::from_rotation_x(0.24 * recoil) * Quat::from_rotation_z(-0.18 * recoil);

    // Post-use kick impulse.
    let kick = state.kick_t.clamp(0.0, 1.0);
    pos += Vec3::new(0.0, -0.02 * kick, 0.06 * kick);
    rot *= Quat::from_rotation_x(0.14 * kick);

    // Per-class active use curve.
    if let Some(raw_t) = state.use_t {
        let t = raw_t.clamp(0.0, 1.0);
        let lunge = (t * std::f32::consts::PI).sin();
        let prog = smoothstep(t);
        match kind {
            HeldAnimKind::SwordSlash => {
                // Terraria useStyle 1: smooth overhead-back to forward-down cleave with follow-through.
                let ease = smoothstep(t);
                let ang = 1.65 - 3.40 * t;
                let yaw = 1.72 + 0.42 * ease;
                let roll = -0.18 + 0.48 * ease;
                rot = Quat::from_rotation_x(ang + 0.24 * recoil + 0.14 * kick - 0.45 * unequip)
                    * Quat::from_rotation_y(yaw)
                    * Quat::from_rotation_z(roll);
                let sweep_arc = (1.0 - ease) * 0.16 - ease * 0.10;
                pos += Vec3::new(-0.16 * lunge - 0.08 * ease, sweep_arc, -0.18 * lunge);
            }
            HeldAnimKind::ToolMine => {
                // Crisp 3D forward-downward pick/axe chop with smooth wrist follow-through.
                let ang = super::player::swing_angle(t) * 0.92;
                let yaw = 1.85 + 0.18 * prog;
                let roll = -0.20 + 0.34 * prog;
                rot = Quat::from_rotation_x(ang)
                    * Quat::from_rotation_y(yaw)
                    * Quat::from_rotation_z(roll);
                pos += Vec3::new(-0.09 * lunge, -0.05 * lunge, -0.17 * lunge);
            }
            HeldAnimKind::BowDraw => {
                // Draw back along sightline, then release.
                let draw = if t < 0.75 {
                    smoothstep(t / 0.75)
                } else {
                    1.0 - smoothstep((t - 0.75) / 0.25)
                };
                pos += Vec3::new(-0.08 * draw, 0.04 * draw, 0.12 * draw);
                rot *= Quat::from_rotation_z(0.18 * draw) * Quat::from_rotation_y(0.12 * draw);
            }
            HeldAnimKind::MagicCast => {
                // Thrust wand forward and up.
                pos += Vec3::new(-0.06 * lunge, 0.08 * lunge, -0.18 * lunge);
                rot *= Quat::from_rotation_x(-0.45 * lunge);
            }
            HeldAnimKind::ConsumableRaise => {
                // Raise item toward mouth/center.
                pos += Vec3::new(-0.22 * lunge, 0.14 * lunge, -0.06 * lunge);
                rot *= Quat::from_rotation_z(0.55 * lunge) * Quat::from_rotation_x(0.25 * lunge);
            }
            HeldAnimKind::BlockTap => {
                // Quick forward placement tap.
                pos += Vec3::new(-0.04 * lunge, -0.04 * lunge, -0.14 * lunge);
                rot *= Quat::from_rotation_x(-0.35 * lunge);
            }
            HeldAnimKind::TorchHold => {
                // Gentle forward wave.
                pos += Vec3::new(-0.03 * lunge, 0.04 * lunge, -0.08 * lunge);
                rot *= Quat::from_rotation_x(-0.22 * lunge);
            }
            HeldAnimKind::ShortswordStab => {
                let jab = (t * std::f32::consts::PI).sin();
                pos += Vec3::new(-0.04 * jab, 0.02 * jab, -0.26 * jab);
                rot *= Quat::from_rotation_x(-0.55 * jab);
            }
            HeldAnimKind::SpearThrust => {
                let thrust = (t * std::f32::consts::PI).sin();
                pos += Vec3::new(-0.06 * thrust, 0.04 * thrust, -0.34 * thrust);
                rot *= Quat::from_rotation_x(-0.68 * thrust);
            }
            HeldAnimKind::GunFire => {
                let kick_back = (1.0 - prog) * lunge;
                pos += Vec3::new(0.0, 0.03 * kick_back, 0.12 * kick_back);
                rot *= Quat::from_rotation_x(0.28 * kick_back);
            }
            HeldAnimKind::WhipSnap => {
                let snap = (t * std::f32::consts::PI).sin();
                pos += Vec3::new(-0.18 * snap, 0.10 * snap, -0.22 * snap);
                rot *= Quat::from_rotation_x(-0.85 * snap) * Quat::from_rotation_z(0.45 * snap);
            }
        }
    }

    // Hard non-clipping guard: never closer to camera than MIN_CAM_Z.
    pos.z = pos.z.min(MIN_CAM_Z);
    Transform {
        translation: pos,
        rotation: rot,
        scale: Vec3::ONE,
    }
}

/// Computes the `arm_r` local rotation for the third-person avatar.
pub fn compute_tp_arm_rot(kind: HeldAnimKind, state: &HeldAnimState) -> Quat {
    let equip = smoothstep(state.equip_t);
    let unequip_pitch = 0.45 * (1.0 - equip);
    let idle_bob = (state.idle_t * 2.0).sin() * 0.04;
    let walk_swing = state.walk_phase.sin() * 0.25;
    let recoil = state.recoil_t.clamp(0.0, 1.0) * 0.35;
    let kick = state.kick_t.clamp(0.0, 1.0) * 0.20;

    if let Some(raw_t) = state.use_t {
        let t = raw_t.clamp(0.0, 1.0);
        let prog = smoothstep(t);
        let lunge = (t * std::f32::consts::PI).sin();
        return match kind {
            HeldAnimKind::SwordSlash => {
                // Constant-speed overhead-back (-152 deg) down past waist (+37 deg).
                let pitch = -2.65 + 3.30 * t + recoil;
                Quat::from_rotation_x(pitch) * Quat::from_rotation_z(-0.22 * lunge)
            }
            HeldAnimKind::ToolMine => {
                let pitch = -2.15 + 1.60 * prog + recoil;
                Quat::from_rotation_x(pitch)
            }
            HeldAnimKind::BowDraw => {
                let pitch = -FRAC_PI_2 - 0.12 * lunge + recoil;
                Quat::from_rotation_x(pitch) * Quat::from_rotation_y(0.25 * lunge)
            }
            HeldAnimKind::MagicCast => {
                let pitch = -1.75 - 0.35 * lunge + recoil;
                Quat::from_rotation_x(pitch)
            }
            HeldAnimKind::ConsumableRaise => {
                let pitch = -1.65 - 0.55 * lunge + recoil;
                Quat::from_rotation_x(pitch) * Quat::from_rotation_z(0.35 * lunge)
            }
            HeldAnimKind::BlockTap => {
                let pitch = -1.35 - 0.40 * lunge + recoil;
                Quat::from_rotation_x(pitch)
            }
            HeldAnimKind::TorchHold => {
                let pitch = -1.25 - 0.30 * lunge + recoil;
                Quat::from_rotation_x(pitch)
            }
            HeldAnimKind::ShortswordStab => {
                let pitch = -1.45 - 0.35 * lunge + recoil;
                Quat::from_rotation_x(pitch)
            }
            HeldAnimKind::SpearThrust => {
                let pitch = -1.55 - 0.45 * lunge + recoil;
                Quat::from_rotation_x(pitch)
            }
            HeldAnimKind::GunFire => {
                let pitch = -FRAC_PI_2 + 0.22 * (1.0 - prog) * lunge + recoil;
                Quat::from_rotation_x(pitch)
            }
            HeldAnimKind::WhipSnap => {
                let pitch = -2.2 + 2.4 * prog + recoil;
                Quat::from_rotation_x(pitch) * Quat::from_rotation_z(-0.35 * lunge)
            }
        };
    }

    let base_pitch = match kind {
        HeldAnimKind::TorchHold => -1.20 + idle_bob + walk_swing * 0.35,
        HeldAnimKind::BowDraw => -FRAC_PI_2 + idle_bob * 0.5 + walk_swing * 0.25,
        HeldAnimKind::GunFire => -1.35 + idle_bob * 0.5 + walk_swing * 0.25,
        _ => walk_swing + idle_bob,
    };
    Quat::from_rotation_x(base_pitch + unequip_pitch + recoil + kick)
}

#[cfg(test)]
mod tests {
    use super::*;
    use terravibe_sim::item::all_item_ids;

    #[test]
    fn held_anim_kind_maps_all_item_classes() {
        assert_eq!(held_anim_kind(TORCH), HeldAnimKind::TorchHold);
        assert_eq!(
            held_anim_kind_for_class(ItemClass::Melee, false),
            HeldAnimKind::SwordSlash
        );
        assert_eq!(
            held_anim_kind_for_class(ItemClass::Tool, false),
            HeldAnimKind::ToolMine
        );
        assert_eq!(
            held_anim_kind_for_class(ItemClass::Ranged, false),
            HeldAnimKind::BowDraw
        );
        assert_eq!(
            held_anim_kind_for_class(ItemClass::Magic, false),
            HeldAnimKind::MagicCast
        );
        assert_eq!(
            held_anim_kind_for_class(ItemClass::Consumable, false),
            HeldAnimKind::ConsumableRaise
        );
        for class in [
            ItemClass::Block,
            ItemClass::Placeable,
            ItemClass::Wall,
            ItemClass::Ammo,
            ItemClass::Armor,
            ItemClass::Accessory,
            ItemClass::Other,
        ] {
            assert_eq!(
                held_anim_kind_for_class(class, false),
                HeldAnimKind::BlockTap
            );
        }
        for id in all_item_ids() {
            let _ = held_anim_kind(id);
            let _ = item_data(id).class;
        }
    }

    #[test]
    fn fp_transform_never_clips_camera_across_all_kinds_and_phases() {
        for kind in HeldAnimKind::ALL {
            let rest = compute_fp_transform(kind, &HeldAnimState::default());
            assert!(
                rest.translation.z <= MIN_CAM_Z,
                "{kind:?} rest z={} > {MIN_CAM_Z}",
                rest.translation.z
            );

            for i in 0..=20 {
                let t = i as f32 / 20.0;
                let state = HeldAnimState {
                    equip_t: (i % 5) as f32 / 4.0,
                    idle_t: t * 3.0,
                    walk_phase: t * std::f32::consts::TAU,
                    vel_y: (t - 0.5) * 0.8,
                    recoil_t: if i % 3 == 0 { 1.0 } else { 0.0 },
                    use_t: Some(t),
                    kick_t: 1.0 - t,
                };
                let tf = compute_fp_transform(kind, &state);
                assert!(
                    tf.translation.z <= MIN_CAM_Z,
                    "{kind:?} at t={t} clipped camera: z={}",
                    tf.translation.z
                );
            }
        }
    }

    #[test]
    fn equip_and_recoil_offset_transform() {
        let rest = compute_fp_transform(HeldAnimKind::SwordSlash, &HeldAnimState::default());
        let unequipped = compute_fp_transform(
            HeldAnimKind::SwordSlash,
            &HeldAnimState {
                equip_t: 0.0,
                ..Default::default()
            },
        );
        assert!(
            unequipped.translation.y < rest.translation.y - 0.1,
            "equip_t=0 should lower item: unequipped={} rest={}",
            unequipped.translation.y,
            rest.translation.y
        );

        let recoiled = compute_fp_transform(
            HeldAnimKind::SwordSlash,
            &HeldAnimState {
                recoil_t: 1.0,
                ..Default::default()
            },
        );
        assert!(
            recoiled.translation != rest.translation || recoiled.rotation != rest.rotation,
            "recoil_t=1 should offset transform from rest"
        );
    }

    #[test]
    fn tp_arm_rot_is_normalized_across_all_kinds() {
        for kind in HeldAnimKind::ALL {
            let idle = compute_tp_arm_rot(kind, &HeldAnimState::default());
            assert!(idle.is_normalized(), "{kind:?} idle quat not normalized");
            let active = compute_tp_arm_rot(
                kind,
                &HeldAnimState {
                    use_t: Some(0.5),
                    ..Default::default()
                },
            );
            assert!(
                active.is_normalized(),
                "{kind:?} active quat not normalized"
            );
        }
    }

    #[test]
    fn fp_tool_pose_points_forward_and_chops_downward() {
        let rest = compute_fp_transform(HeldAnimKind::ToolMine, &HeldAnimState::default());
        let forward_axis = rest.rotation * Vec3::X;
        assert!(
            forward_axis.z < -0.5,
            "tool rest pose should point forward into -Z, got {forward_axis:?}"
        );

        let start = compute_fp_transform(
            HeldAnimKind::ToolMine,
            &HeldAnimState {
                use_t: Some(0.05),
                ..Default::default()
            },
        );
        let end = compute_fp_transform(
            HeldAnimKind::ToolMine,
            &HeldAnimState {
                use_t: Some(0.95),
                ..Default::default()
            },
        );
        let start_dir = start.rotation * Vec3::X;
        let end_dir = end.rotation * Vec3::X;
        assert!(
            start_dir.y > end_dir.y + 0.5,
            "tool swing should chop downward from {start_dir:?} to {end_dir:?}"
        );
    }

    #[test]
    fn fp_broadsword_sweeps_linear_overhead_to_downward_arc() {
        let start = compute_fp_transform(
            HeldAnimKind::SwordSlash,
            &HeldAnimState {
                use_t: Some(0.0),
                ..Default::default()
            },
        );
        let mid = compute_fp_transform(
            HeldAnimKind::SwordSlash,
            &HeldAnimState {
                use_t: Some(0.5),
                ..Default::default()
            },
        );
        let end = compute_fp_transform(
            HeldAnimKind::SwordSlash,
            &HeldAnimState {
                use_t: Some(1.0),
                ..Default::default()
            },
        );
        let s = start.rotation * Vec3::X;
        let m = mid.rotation * Vec3::X;
        let e = end.rotation * Vec3::X;
        assert!(s.y > 0.75, "broadsword should start overhead, got {s:?}");
        assert!(
            m.z < -0.75,
            "broadsword should sweep through forward, got {m:?}"
        );
        assert!(e.y < -0.75, "broadsword should finish low, got {e:?}");
    }
}
