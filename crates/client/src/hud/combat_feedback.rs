//! Pooled floating damage numbers, crosshair hit marker, and damage-direction indicator.

use bevy::prelude::*;
use terravibe_sim::net::{DamageKind, HitTargetKind};

use super::text::{HudText, HudTextColor, StackText};
use crate::audio::{SfxEvent, SfxKind};
use crate::session::AppState;
use crate::settings::Settings;
use crate::view::camera::FlyCamera;

/// Maximum concurrent floating damage numbers in the pool.
pub const MAX_DAMAGE_NUMBERS: usize = 32;
/// Rapid hits on the same target within this window merge into one number.
pub const DAMAGE_MERGE_WINDOW_SECS: f32 = 0.15;
/// Total lifetime of a floating damage number before it expires.
pub const DAMAGE_LIFETIME_SECS: f32 = 0.85;
/// Maximum distance (blocks) between hits to be considered the same target for merging.
const MERGE_DISTANCE_SQ: f32 = 1.5 * 1.5;
/// Maximum concurrent directional damage indicators around the crosshair.
const MAX_DIRECTION_INDICATORS: usize = 4;
/// Lifetime of a directional damage indicator in seconds.
const DIRECTION_INDICATOR_SECS: f32 = 0.75;
/// Radius (px) from screen center for directional damage indicators.
const DIRECTION_INDICATOR_RADIUS_PX: f32 = 64.0;

/// Client-side combat hit event forwarded from `ServerMsg::CombatHit`.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct CombatHitEvent {
    pub pos: Vec3,
    pub attacker_pos: Option<Vec3>,
    pub amount: i32,
    pub crit: bool,
    pub damage_kind: DamageKind,
    pub target_kind: HitTargetKind,
    pub killed: bool,
}

/// One active floating damage number entry.
#[derive(Clone, Debug)]
pub struct DamageNumberEntry {
    pub world_pos: Vec3,
    pub amount: i32,
    pub crit: bool,
    pub target_kind: HitTargetKind,
    pub age: f32,
    pub merge_timer: f32,
    pub vel: Vec3,
}

/// Pooled floating damage numbers, capped at [`MAX_DAMAGE_NUMBERS`].
#[derive(Resource, Default, Debug)]
pub struct DamageNumberPool {
    pub entries: Vec<DamageNumberEntry>,
}

impl DamageNumberPool {
    /// Spawns a new damage number or merges into a recent hit on the same target.
    /// Does nothing when `settings.damage_numbers` is `false` or `event.amount <= 0`.
    pub fn spawn_or_merge(&mut self, event: &CombatHitEvent, settings: &Settings) {
        if !settings.damage_numbers || event.amount <= 0 {
            return;
        }

        if let Some(existing) = self.entries.iter_mut().rev().find(|e| {
            e.target_kind == event.target_kind
                && e.merge_timer <= DAMAGE_MERGE_WINDOW_SECS
                && e.world_pos.distance_squared(event.pos) <= MERGE_DISTANCE_SQ
        }) {
            existing.amount += event.amount;
            existing.crit |= event.crit;
            existing.age = 0.0;
            existing.merge_timer = 0.0;
            existing.world_pos = event.pos + Vec3::new(0.0, 0.8, 0.0);
            return;
        }

        if self.entries.len() >= MAX_DAMAGE_NUMBERS {
            self.entries.remove(0);
        }

        let lateral = ((self.entries.len() % 5) as f32 - 2.0) * 0.35;
        self.entries.push(DamageNumberEntry {
            world_pos: event.pos + Vec3::new(0.0, 0.8, 0.0),
            amount: event.amount,
            crit: event.crit,
            target_kind: event.target_kind,
            age: 0.0,
            merge_timer: 0.0,
            vel: Vec3::new(lateral, 2.2, 0.0),
        });
    }

    /// Advances all active damage numbers and expires old ones.
    pub fn tick(&mut self, dt: f32) {
        for entry in &mut self.entries {
            entry.age += dt;
            entry.merge_timer += dt;
            entry.world_pos += entry.vel * dt;
            entry.vel.y -= 2.4 * dt;
        }
        self.entries.retain(|e| e.age < DAMAGE_LIFETIME_SECS);
    }
}

/// Crosshair hit marker state (brighter and larger on critical hits).
#[derive(Resource, Default, Debug)]
pub struct HitMarkerState {
    pub timer: f32,
    pub duration: f32,
    pub crit: bool,
}

impl HitMarkerState {
    pub fn trigger(&mut self, crit: bool) {
        self.crit = crit;
        self.duration = if crit { 0.22 } else { 0.14 };
        self.timer = self.duration;
    }

    pub fn tick(&mut self, dt: f32) {
        self.timer = (self.timer - dt).max(0.0);
    }

    pub fn alpha(&self) -> f32 {
        if self.duration <= 0.0 || self.timer <= 0.0 {
            0.0
        } else {
            (self.timer / self.duration).clamp(0.0, 1.0)
        }
    }

    pub fn scale(&self) -> f32 {
        if self.crit { 1.45 } else { 1.0 }
    }

    pub fn color(&self) -> Color {
        let a = self.alpha();
        if self.crit {
            Color::srgba(1.0, 0.88, 0.25, a)
        } else {
            Color::srgba(1.0, 1.0, 1.0, a * 0.9)
        }
    }
}

/// Active directional damage indicator entry.
#[derive(Clone, Debug)]
pub struct DamageDirectionEntry {
    pub attacker_pos: Vec3,
    pub timer: f32,
    pub duration: f32,
}

/// Tracks directional indicators around the crosshair when the player is hit.
#[derive(Resource, Default, Debug)]
pub struct DamageDirectionState {
    pub entries: Vec<DamageDirectionEntry>,
}

impl DamageDirectionState {
    pub fn trigger(&mut self, attacker_pos: Vec3) {
        if self.entries.len() >= MAX_DIRECTION_INDICATORS {
            self.entries.remove(0);
        }
        self.entries.push(DamageDirectionEntry {
            attacker_pos,
            timer: DIRECTION_INDICATOR_SECS,
            duration: DIRECTION_INDICATOR_SECS,
        });
    }

    pub fn tick(&mut self, dt: f32) {
        for entry in &mut self.entries {
            entry.timer -= dt;
        }
        self.entries.retain(|e| e.timer > 0.0);
    }
}

/// Computes the horizontal angle (radians in `[-PI, PI]`) from the camera's forward
/// direction toward `attacker_pos`: `0` = straight ahead, `+PI/2` = right,
/// `-PI/2` = left, `+-PI` = behind.
pub fn damage_direction_angle(cam_pos: Vec3, cam_yaw: f32, attacker_pos: Vec3) -> f32 {
    let to_attacker = Vec2::new(attacker_pos.x - cam_pos.x, attacker_pos.z - cam_pos.z);
    if to_attacker.length_squared() < 1e-6 {
        return 0.0;
    }
    let forward = Vec2::new(-cam_yaw.sin(), -cam_yaw.cos());
    let right = Vec2::new(cam_yaw.cos(), -cam_yaw.sin());
    let fwd_dot = to_attacker.dot(forward);
    let right_dot = to_attacker.dot(right);
    right_dot.atan2(fwd_dot)
}

#[derive(Component)]
pub struct DamageNumberSlot(usize);

#[derive(Component)]
pub struct HitMarkerRoot;

#[derive(Component)]
pub struct HitMarkerTick;

#[derive(Component)]
pub struct DamageDirectionSlot(usize);

/// Spawns pooled HUD nodes for damage numbers, crosshair hit marker, and damage direction indicators.
pub fn setup(mut commands: Commands) {
    for i in 0..MAX_DAMAGE_NUMBERS {
        commands.spawn((
            DamageNumberSlot(i),
            StackText,
            HudText {
                text: String::new(),
                death: false,
            },
            HudTextColor(Color::WHITE),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..default()
            },
            Visibility::Hidden,
            GlobalZIndex(900),
            DespawnOnExit(AppState::InGame),
        ));
    }

    // Crosshair hit marker: 4 diagonal ticks centered at (50%, 50%).
    commands
        .spawn((
            HitMarkerRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(50.0),
                width: Val::Px(0.0),
                height: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Transform::from_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)),
            Visibility::Hidden,
            GlobalZIndex(950),
            DespawnOnExit(AppState::InGame),
        ))
        .with_children(|p| {
            let color = BackgroundColor(Color::WHITE);
            p.spawn((
                HitMarkerTick,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(-1.0),
                    top: Val::Px(-14.0),
                    width: Val::Px(2.0),
                    height: Val::Px(7.0),
                    ..default()
                },
                color,
            ));
            p.spawn((
                HitMarkerTick,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(-1.0),
                    top: Val::Px(7.0),
                    width: Val::Px(2.0),
                    height: Val::Px(7.0),
                    ..default()
                },
                color,
            ));
            p.spawn((
                HitMarkerTick,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(-14.0),
                    top: Val::Px(-1.0),
                    width: Val::Px(7.0),
                    height: Val::Px(2.0),
                    ..default()
                },
                color,
            ));
            p.spawn((
                HitMarkerTick,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(7.0),
                    top: Val::Px(-1.0),
                    width: Val::Px(7.0),
                    height: Val::Px(2.0),
                    ..default()
                },
                color,
            ));
        });

    // Directional damage indicators around crosshair.
    for i in 0..MAX_DIRECTION_INDICATORS {
        commands.spawn((
            DamageDirectionSlot(i),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(50.0),
                width: Val::Px(26.0),
                height: Val::Px(5.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.95, 0.15, 0.15, 0.0)),
            Visibility::Hidden,
            GlobalZIndex(940),
            DespawnOnExit(AppState::InGame),
        ));
    }
}

/// Consumes `CombatHitEvent` messages and updates feedback pools/states.
pub fn consume_combat_hits(
    mut events: MessageReader<CombatHitEvent>,
    settings: Option<Res<Settings>>,
    mut pool: ResMut<DamageNumberPool>,
    mut hit_marker: ResMut<HitMarkerState>,
    mut damage_dir: ResMut<DamageDirectionState>,
    mut sfx: Option<ResMut<Messages<SfxEvent>>>,
) {
    let settings = settings.as_deref().copied().unwrap_or_default();
    for ev in events.read() {
        pool.spawn_or_merge(ev, &settings);
        if ev.target_kind != HitTargetKind::Player {
            hit_marker.trigger(ev.crit);
            if let Some(ref mut bus) = sfx {
                let kind = if ev.killed {
                    SfxKind::Kill
                } else if ev.crit {
                    SfxKind::Crit
                } else {
                    SfxKind::Hit
                };
                bus.write(SfxEvent(kind));
            }
        } else if let Some(attacker_pos) = ev.attacker_pos {
            damage_dir.trigger(attacker_pos);
        }
    }
}

/// Advances combat feedback timers and syncs pooled HUD nodes.
#[allow(clippy::too_many_arguments)]
pub fn update_combat_feedback(
    time: Res<Time>,
    settings: Option<Res<Settings>>,
    mut pool: ResMut<DamageNumberPool>,
    mut hit_marker: ResMut<HitMarkerState>,
    mut damage_dir: ResMut<DamageDirectionState>,
    camera_q: Query<(&Camera, &GlobalTransform, &FlyCamera)>,
    mut dmg_slots: Query<(
        &DamageNumberSlot,
        &mut Node,
        &mut HudText,
        &mut HudTextColor,
        &mut Visibility,
    )>,
    mut marker_root: Query<
        (&mut Transform, &mut Visibility),
        (
            With<HitMarkerRoot>,
            Without<DamageNumberSlot>,
            Without<DamageDirectionSlot>,
        ),
    >,
    mut marker_ticks: Query<
        &mut BackgroundColor,
        (With<HitMarkerTick>, Without<DamageDirectionSlot>),
    >,
    mut dir_slots: Query<
        (
            &DamageDirectionSlot,
            &mut Node,
            &mut Transform,
            &mut BackgroundColor,
            &mut Visibility,
        ),
        (
            Without<HitMarkerRoot>,
            Without<DamageNumberSlot>,
            Without<HitMarkerTick>,
        ),
    >,
) {
    let dt = time.delta_secs();
    pool.tick(dt);
    hit_marker.tick(dt);
    damage_dir.tick(dt);

    let show_numbers = settings.as_deref().is_none_or(|s| s.damage_numbers);
    let cam = camera_q.iter().next();

    // Update floating damage number slots.
    for (slot, mut node, mut text, mut color, mut vis) in &mut dmg_slots {
        let Some(entry) = show_numbers.then(|| pool.entries.get(slot.0)).flatten() else {
            *vis = Visibility::Hidden;
            continue;
        };
        let Some((camera, cam_xf, _)) = cam else {
            *vis = Visibility::Hidden;
            continue;
        };
        let Ok(screen) = camera.world_to_viewport(cam_xf, entry.world_pos) else {
            *vis = Visibility::Hidden;
            continue;
        };

        let alpha = (1.0 - (entry.age / DAMAGE_LIFETIME_SECS).powi(2)).clamp(0.0, 1.0);
        let base_color = if entry.target_kind == HitTargetKind::Player {
            Color::srgba(1.0, 0.28, 0.28, alpha)
        } else if entry.crit {
            Color::srgba(1.0, 0.85, 0.22, alpha)
        } else {
            Color::srgba(1.0, 0.96, 0.90, alpha)
        };
        let label = if entry.crit {
            format!("{}!", entry.amount)
        } else {
            format!("{}", entry.amount)
        };
        if text.text != label {
            text.text = label;
        }
        if color.0 != base_color {
            color.0 = base_color;
        }
        node.left = Val::Px(screen.x - 10.0);
        node.top = Val::Px(screen.y - 12.0);
        *vis = Visibility::Inherited;
    }

    // Update crosshair hit marker.
    let marker_alpha = hit_marker.alpha();
    for (mut xf, mut vis) in &mut marker_root {
        if marker_alpha <= 0.0 {
            *vis = Visibility::Hidden;
        } else {
            *vis = Visibility::Inherited;
            *xf = Transform::from_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4))
                .with_scale(Vec3::splat(hit_marker.scale()));
        }
    }
    if marker_alpha > 0.0 {
        let c = hit_marker.color();
        for mut bg in &mut marker_ticks {
            bg.0 = c;
        }
    }

    // Update directional damage indicators around crosshair.
    for (slot, mut node, mut xf, mut bg, mut vis) in &mut dir_slots {
        let Some(entry) = damage_dir.entries.get(slot.0) else {
            *vis = Visibility::Hidden;
            continue;
        };
        let Some((_, cam_xf, fly_cam)) = cam else {
            *vis = Visibility::Hidden;
            continue;
        };
        let angle = damage_direction_angle(cam_xf.translation(), fly_cam.yaw, entry.attacker_pos);
        let dx = DIRECTION_INDICATOR_RADIUS_PX * angle.sin();
        let dy = -DIRECTION_INDICATOR_RADIUS_PX * angle.cos();
        let alpha = (entry.timer / entry.duration).clamp(0.0, 1.0) * 0.85;

        node.left = Val::Percent(50.0);
        node.top = Val::Percent(50.0);
        node.margin = UiRect::new(Val::Px(dx - 13.0), Val::Auto, Val::Px(dy - 2.5), Val::Auto);
        *xf = Transform::from_rotation(Quat::from_rotation_z(-angle));
        bg.0 = Color::srgba(0.95, 0.18, 0.18, alpha);
        *vis = Visibility::Inherited;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terravibe_sim::net::{DamageKind, HitTargetKind};

    use crate::settings::Settings;

    fn sample_hit(pos: Vec3, amount: i32, crit: bool) -> CombatHitEvent {
        CombatHitEvent {
            pos,
            attacker_pos: None,
            amount,
            crit,
            damage_kind: DamageKind::Melee,
            target_kind: HitTargetKind::Enemy,
            killed: false,
        }
    }

    #[test]
    fn damage_number_pool_caps_at_32_and_merges_rapid_ticks() {
        let settings = Settings::default();
        let mut pool = DamageNumberPool::default();

        // Two rapid hits on the same target within 0.15s should merge into one entry.
        pool.spawn_or_merge(
            &sample_hit(Vec3::new(10.0, 5.0, 10.0), 12, false),
            &settings,
        );
        pool.tick(0.05);
        pool.spawn_or_merge(&sample_hit(Vec3::new(10.1, 5.0, 10.0), 18, true), &settings);
        assert_eq!(pool.entries.len(), 1);
        assert_eq!(pool.entries[0].amount, 30);
        assert!(pool.entries[0].crit);

        // After > 0.15s, a new hit on the same target does not merge.
        pool.tick(0.20);
        pool.spawn_or_merge(&sample_hit(Vec3::new(10.0, 5.0, 10.0), 7, false), &settings);
        assert_eq!(pool.entries.len(), 2);

        // Flooding beyond MAX_DAMAGE_NUMBERS (32) caps at 32.
        for i in 0..50 {
            let pos = Vec3::new(100.0 + (i as f32) * 5.0, 5.0, 10.0);
            pool.spawn_or_merge(&sample_hit(pos, 5, false), &settings);
        }
        assert_eq!(pool.entries.len(), MAX_DAMAGE_NUMBERS);
    }

    #[test]
    fn damage_numbers_disabled_suppresses_spawns() {
        let mut settings = Settings::default();
        settings.damage_numbers = false;
        let mut pool = DamageNumberPool::default();

        pool.spawn_or_merge(&sample_hit(Vec3::new(1.0, 2.0, 3.0), 25, true), &settings);
        assert!(pool.entries.is_empty());
    }

    #[test]
    fn damage_direction_angle_points_toward_attacker() {
        let cam_pos = Vec3::ZERO;
        let cam_yaw = 0.0; // Facing -Z, right is +X

        // Ahead (-Z) -> ~0
        let ahead = damage_direction_angle(cam_pos, cam_yaw, Vec3::new(0.0, 0.0, -10.0));
        assert!(ahead.abs() < 1e-4, "expected 0 for ahead, got {ahead}");

        // Right (+X) -> +PI/2
        let right = damage_direction_angle(cam_pos, cam_yaw, Vec3::new(10.0, 0.0, 0.0));
        assert!(
            (right - std::f32::consts::FRAC_PI_2).abs() < 1e-4,
            "expected +PI/2 for right, got {right}"
        );

        // Left (-X) -> -PI/2
        let left = damage_direction_angle(cam_pos, cam_yaw, Vec3::new(-10.0, 0.0, 0.0));
        assert!(
            (left + std::f32::consts::FRAC_PI_2).abs() < 1e-4,
            "expected -PI/2 for left, got {left}"
        );

        // Behind (+Z) -> +-PI
        let behind = damage_direction_angle(cam_pos, cam_yaw, Vec3::new(0.0, 0.0, 10.0));
        assert!(
            (behind.abs() - std::f32::consts::PI).abs() < 1e-4,
            "expected +-PI for behind, got {behind}"
        );
    }
}
