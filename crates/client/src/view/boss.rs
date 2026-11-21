//! The Eye of Cthulhu: a voxel rig following the server's per-tick `Boss`
//! state, eased between ticks and turned toward the local player.

use bevy::prelude::*;

use super::camera::CameraTrauma;
use super::particles::ParticlePool;
use super::player::LocalPlayer;
use crate::art::art_dir;
use crate::art::rig::{self, Rig};
use crate::art::rig_view::{MeshCache, RigAnim, RigPart, spawn_rig_with};
use crate::audio::{SfxEvent, SfxKind};
use crate::hud::boss_bar::BossState;
use crate::session::AppState;
use crate::settings::Settings;

const EASE_PER_SEC: f32 = 20.0;

/// `(iris visible, mouth visible)`: phase 2 swaps the eye for a toothy mouth.
#[allow(dead_code)]
pub(crate) fn eoc_parts_visible(phase: u8) -> (bool, bool) {
    (phase < 2, phase >= 2)
}

/// `(iris visible, mouth visible)` accounting for transformation spin states.
pub(crate) fn eoc_parts_visible_for(phase: u8, state: u8) -> (bool, bool) {
    let show_iris = state == 2 || (phase < 2 && state != 4);
    let show_mouth = state == 4 || (phase >= 2 && state != 2);
    (show_iris, show_mouth)
}

pub struct BossPlugin;

#[derive(Resource)]
struct BossRig {
    rig: Rig,
    dir: std::path::PathBuf,
}

#[derive(Resource, Default)]
pub(crate) struct BossFxState {
    pub prev_phase: u8,
    pub prev_state: u8,
    pub prev_life: i32,
    pub prev_pos: Vec3,
    pub dash_roar_cooldown: f32,
    pub particle_accum: f32,
    pub hurt_flash: f32,
    pub active: bool,
}

#[derive(Component)]
struct BossBody;

#[derive(Component)]
struct BossMaterial(Handle<StandardMaterial>);

impl Plugin for BossPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BossState>()
            .init_resource::<BossFxState>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    follow.before(crate::art::rig_view::animate_rigs),
                    chomp_mouth.after(crate::art::rig_view::animate_rigs),
                )
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

fn setup(mut commands: Commands) {
    let dir = art_dir().join("models/eye_of_cthulhu");
    match rig::try_load(&dir) {
        Ok(rig) => commands.insert_resource(BossRig { rig, dir }),
        Err(e) => warn!("{e}"),
    }
}

fn spawn_gore_burst(
    pool: &mut ParticlePool,
    origin: Vec3,
    count: usize,
    speed: f32,
    settings: &Settings,
) {
    for i in 0..count {
        let a = (i as f32 / count.max(1) as f32) * std::f32::consts::TAU;
        let elev = ((i % 7) as f32 - 3.0) * 0.28;
        let dir = Vec3::new(a.cos(), elev, a.sin()).normalize_or_zero();
        let vel = dir * speed * (0.6 + ((i % 5) as f32) * 0.16);
        let color = match i % 3 {
            0 => Color::srgb(0.82, 0.10, 0.14),
            1 => Color::srgb(0.58, 0.08, 0.12),
            _ => Color::srgb(0.92, 0.86, 0.72),
        };
        pool.spawn(
            origin + dir * 0.6,
            vel,
            color,
            0.18 + ((i % 3) as f32) * 0.05,
            0.55 + ((i % 4) as f32) * 0.12,
            14.0,
            settings,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn follow(
    mut commands: Commands,
    time: Res<Time>,
    state: Res<BossState>,
    mut fx: ResMut<BossFxState>,
    player: Option<Res<LocalPlayer>>,
    assets: Option<Res<BossRig>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut pool: Option<ResMut<ParticlePool>>,
    mut trauma: Option<ResMut<CameraTrauma>>,
    mut sfx: Option<MessageWriter<SfxEvent>>,
    settings: Option<Res<Settings>>,
    mut q: Query<
        (
            Entity,
            &mut Transform,
            Option<&mut RigAnim>,
            Option<&BossMaterial>,
        ),
        With<BossBody>,
    >,
    mut parts: Query<(&RigPart, &mut Visibility)>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    let s = settings.as_deref().copied().unwrap_or_default();

    let Some(b) = state.0 else {
        if fx.active {
            if fx.prev_life <= 200 {
                if let Some(ref mut p) = pool {
                    spawn_gore_burst(p, fx.prev_pos + Vec3::Y * 2.5, 48, 11.0, &s);
                }
                if let Some(ref mut bus) = sfx {
                    bus.write(SfxEvent(SfxKind::Kill));
                }
                if let Some(ref mut tr) = trauma {
                    tr.add(0.6);
                }
            }
            *fx = BossFxState::default();
        }
        for (e, _, _, _) in &q {
            commands.entity(e).despawn();
        }
        return;
    };

    let target = Vec3::from(b.pos);
    fx.prev_pos = target;

    if !fx.active {
        fx.active = true;
        fx.prev_phase = b.phase;
        fx.prev_state = b.state;
        fx.prev_life = b.life;
        if let Some(ref mut tr) = trauma {
            tr.add(0.35);
        }
    }

    if b.life < fx.prev_life {
        fx.hurt_flash = 0.14;
    }
    fx.hurt_flash = (fx.hurt_flash - dt).max(0.0);
    fx.dash_roar_cooldown = (fx.dash_roar_cooldown - dt).max(0.0);

    // Transformation Stage 1 entry (state == 2)
    if b.state == 2 && fx.prev_state != 2 {
        if let Some(ref mut bus) = sfx {
            bus.write(SfxEvent(SfxKind::BossRoar));
        }
    }
    // Transformation Stage 2 entry (state == 4)
    if b.state == 4 && fx.prev_state != 4 {
        if let Some(ref mut bus) = sfx {
            bus.write(SfxEvent(SfxKind::BossRoar));
            bus.write(SfxEvent(SfxKind::BossTransform));
        }
        if let Some(ref mut tr) = trauma {
            tr.add(0.75);
        }
        if let Some(ref mut p) = pool {
            spawn_gore_burst(p, target + Vec3::Y * 2.5, 36, 10.0, &s);
        }
    }
    // Phase 2 Dash entry (state == 1)
    if b.phase >= 2 && b.state == 1 && fx.prev_state != 1 && fx.dash_roar_cooldown <= 0.0 {
        if let Some(ref mut bus) = sfx {
            bus.write(SfxEvent(SfxKind::BossDash));
        }
        fx.dash_roar_cooldown = 0.35;
    }

    fx.prev_phase = b.phase;
    fx.prev_state = b.state;
    fx.prev_life = b.life;

    // Continuous gore particles during transformation spin and Phase 2 dash
    let center = target + Vec3::Y * 2.5;
    if b.state == 2 {
        fx.particle_accum += dt * 36.0;
        let count = fx.particle_accum.floor() as usize;
        fx.particle_accum -= count as f32;
        if count > 0
            && let Some(ref mut p) = pool
        {
            spawn_gore_burst(p, center, count.min(6), 6.5, &s);
        }
    } else if b.phase >= 2 && b.state == 1 {
        fx.particle_accum += dt * 48.0;
        let count = fx.particle_accum.floor() as usize;
        fx.particle_accum -= count as f32;
        if count > 0
            && let Some(ref mut p) = pool
        {
            for i in 0..count.min(6) {
                let jitter = Vec3::new(
                    ((t * 17.0 + i as f32).sin()) * 1.1,
                    ((t * 23.0 + i as f32).cos()) * 1.1,
                    ((t * 31.0 + i as f32).sin()) * 1.1,
                );
                let color = if i % 2 == 0 {
                    Color::srgb(0.85, 0.10, 0.14)
                } else {
                    Color::srgb(0.95, 0.45, 0.12)
                };
                p.spawn(
                    center + jitter,
                    -Vec3::from(b.vel) * 0.25 + jitter * 1.5,
                    color,
                    0.18,
                    0.45,
                    8.0,
                    &s,
                );
            }
        }
    } else {
        fx.particle_accum = 0.0;
    }

    let Some(assets) = assets else { return };

    // The server's position is the feet centre; the rig's ball sits at body height.
    let Ok((root, mut tf, rig_anim, boss_mat)) = q.single_mut() else {
        let mut cache = MeshCache::default();
        let (root, mat) = spawn_rig_with(
            &mut commands,
            &mut meshes,
            &mut materials,
            &assets.rig,
            &assets.dir,
            &mut cache,
        );
        commands.entity(root).insert((
            BossBody,
            BossMaterial(mat),
            Transform::from_translation(target),
            DespawnOnExit(AppState::InGame),
        ));
        return;
    };

    let k = 1.0 - (-EASE_PER_SEC * dt).exp();
    let mut eased_pos = tf.translation.lerp(target, k);
    if b.state == 2 {
        let shudder = Vec3::new(
            (t * 73.0).sin() * 0.18,
            (t * 89.0).cos() * 0.18,
            (t * 67.0).sin() * 0.18,
        );
        eased_pos += shudder;
    }
    tf.translation = eased_pos;

    // Face the local player (full pitch + yaw in Phase 2, yaw + lean in Phase 1, rapid spin in state 2/4).
    let v = Vec3::from(b.vel);
    let to_player = player.map_or(Vec3::Z, |p| Vec3::from(p.0.body.pos) - tf.translation);
    let yaw = to_player.x.atan2(to_player.z);
    let pitch = if b.phase >= 2 {
        let horiz = (to_player.x * to_player.x + to_player.z * to_player.z)
            .sqrt()
            .max(0.01);
        (-to_player.y).atan2(horiz).clamp(-1.1, 1.1)
    } else {
        0.0
    };
    let lean = (-v.x * 1.5).clamp(-0.5, 0.5);
    let want = if b.state == 2 || b.state == 4 {
        Quat::from_rotation_y(yaw + b.spin) * Quat::from_rotation_z(lean)
    } else {
        Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch) * Quat::from_rotation_z(lean)
    };
    if b.state == 2 || b.state == 4 {
        tf.rotation = want;
    } else {
        tf.rotation = tf.rotation.slerp(want, k);
    }

    // Scale: transformation pulse or Phase 2 dash velocity stretch
    let speed = v.length();
    if b.state == 2 {
        let pulse = 1.0 + 0.08 * (t * 28.0).sin();
        tf.scale = Vec3::splat(pulse);
    } else if b.phase >= 2 && b.state == 1 && speed > 1.0 {
        let stretch = (1.0 + (speed / 45.0).clamp(0.0, 0.25)).min(1.25);
        tf.scale = Vec3::new(1.0 / stretch.sqrt(), 1.0 / stretch.sqrt(), stretch);
    } else {
        tf.scale = tf.scale.lerp(Vec3::ONE, k);
    }

    // Speed up tendril RigAnim playback (2.2x) in Phase 2 / dashing
    if (b.phase >= 2 || b.state == 1)
        && let Some(mut anim) = rig_anim
    {
        anim.t += dt * 1.2;
    }

    // Pulsing red emissive tint in Phase 2 + hurt flash
    if let Some(BossMaterial(mat_handle)) = boss_mat
        && let Some(mut mat) = materials.get_mut(mat_handle)
    {
        if fx.hurt_flash > 0.0 {
            mat.base_color = Color::srgb(1.0, 0.4, 0.4);
            mat.emissive = LinearRgba::new(0.8, 0.1, 0.1, 1.0);
        } else if b.phase >= 2 {
            let pulse = 0.25 + 0.20 * (t * 8.0).sin().abs();
            mat.base_color = Color::WHITE;
            mat.emissive = LinearRgba::new(pulse, 0.02, 0.02, 1.0);
        } else {
            mat.base_color = Color::WHITE;
            mat.emissive = LinearRgba::BLACK;
        }
    }

    let (iris, mouth) = eoc_parts_visible_for(b.phase, b.state);
    for (p, mut vis) in &mut parts {
        if p.root != root {
            continue;
        }
        let show = match p.name.as_str() {
            "iris" => iris,
            "mouth" => mouth,
            _ => continue,
        };
        let want = if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
    }
}

fn chomp_mouth(
    time: Res<Time>,
    state: Res<BossState>,
    q: Query<Entity, With<BossBody>>,
    mut parts: Query<(&RigPart, &mut Transform)>,
) {
    let Some(b) = state.0 else { return };
    if b.phase < 2 && b.state != 4 {
        return;
    }
    let Ok(root) = q.single() else { return };
    let chomp = 1.0 + 0.16 * (time.elapsed_secs() * 14.0).sin().abs();
    for (p, mut tf) in &mut parts {
        if p.root == root && p.name == "mouth" {
            tf.scale = Vec3::new(1.0, chomp, 1.0 + (chomp - 1.0) * 0.5);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

    use super::*;
    use crate::art::vox::{self, VoxModel};

    const SCLERA: [u8; 4] = [0xf0, 0xdc, 0xd2, 255];
    const SCLERA_D: [u8; 4] = [0xd8, 0xbc, 0xb4, 255];
    const VEIN: [u8; 4] = [0xc4, 0x33, 0x3a, 255];
    const IRIS_L: [u8; 4] = [0xe0, 0x40, 0x2f, 255];
    const IRIS: [u8; 4] = [0xb8, 0x1f, 0x24, 255];
    const IRIS_D: [u8; 4] = [0x8a, 0x12, 0x18, 255];
    const PUPIL: [u8; 4] = [0x0a, 0x0a, 0x0a, 255];
    const FLESH: [u8; 4] = [0x7a, 0x1f, 0x2a, 255];
    const FLESH_D: [u8; 4] = [0x4a, 0x0d, 0x12, 255];
    const TOOTH: [u8; 4] = [0xf2, 0xec, 0xd8, 255];
    const TOOTH_D: [u8; 4] = [0xcb, 0xc3, 0xa8, 255];

    fn boss_dir() -> std::path::PathBuf {
        art_dir().join("models/eye_of_cthulhu")
    }

    fn boss_rig() -> Rig {
        rig::try_load(&boss_dir()).unwrap_or_else(|e| panic!("{e}"))
    }

    fn part_model(name: &str) -> VoxModel {
        vox::try_load(&boss_dir().join(format!("{name}.vox"))).unwrap_or_else(|e| panic!("{e}"))
    }

    /// Game-axis dims from the file-order size `[x, depth, height]`.
    fn game_dims(m: &VoxModel) -> (u8, u8, u8) {
        (m.size[0], m.size[2], m.size[1])
    }

    /// Voxels in game axes `(x right, y up, z front)` with their RGBA colour.
    fn game_voxels(m: &VoxModel) -> Vec<([u8; 3], [u8; 4])> {
        let depth = m.size[1];
        m.voxels
            .iter()
            .map(|v| ([v[0], v[2], depth - 1 - v[1]], m.palette[v[3] as usize]))
            .collect()
    }

    fn colours(vs: &[([u8; 3], [u8; 4])]) -> BTreeSet<[u8; 4]> {
        vs.iter().map(|(_, c)| *c).collect()
    }

    fn positions(vs: &[([u8; 3], [u8; 4])], keep: &[[u8; 4]]) -> Vec<[u8; 3]> {
        vs.iter()
            .filter(|(_, c)| keep.contains(c))
            .map(|(p, _)| *p)
            .collect()
    }

    fn near(actual: [f32; 3], want: [f32; 3], what: &str) {
        for a in 0..3 {
            assert!(
                (actual[a] - want[a]).abs() < 1e-3,
                "{what}: {:?} != {want:?}",
                actual
            );
        }
    }

    fn assert_part(r: &Rig, name: &str, pivot: [f32; 3], offset: [f32; 3], parent: Option<&str>) {
        let p = r
            .parts
            .iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("rig has no part {name}"));
        near(p.pivot, pivot, &format!("{name} pivot"));
        near(p.offset, offset, &format!("{name} offset"));
        assert_eq!(p.parent.as_deref(), parent, "{name} parent");
    }

    fn neighbours(p: [u8; 3]) -> impl Iterator<Item = [u8; 3]> {
        const D: [(i16, i16, i16); 6] = [
            (1, 0, 0),
            (-1, 0, 0),
            (0, 1, 0),
            (0, -1, 0),
            (0, 0, 1),
            (0, 0, -1),
        ];
        D.into_iter().filter_map(move |d| {
            let c = [
                i16::from(p[0]) + d.0,
                i16::from(p[1]) + d.1,
                i16::from(p[2]) + d.2,
            ];
            if c.iter().all(|v| (0..=255).contains(v)) {
                Some([c[0] as u8, c[1] as u8, c[2] as u8])
            } else {
                None
            }
        })
    }

    /// 26-connected components of `ps`, in no particular order.
    fn components(ps: &[[u8; 3]]) -> Vec<Vec<[u8; 3]>> {
        let mut left: HashSet<[u8; 3]> = ps.iter().copied().collect();
        let mut out = Vec::new();
        while let Some(&seed) = left.iter().next() {
            left.remove(&seed);
            let mut comp = vec![seed];
            let mut stack = vec![seed];
            while let Some(p) = stack.pop() {
                for dx in -1i16..=1 {
                    for dy in -1i16..=1 {
                        for dz in -1i16..=1 {
                            let x = i16::from(p[0]) + dx;
                            let y = i16::from(p[1]) + dy;
                            let z = i16::from(p[2]) + dz;
                            if !(0..=255).contains(&x)
                                || !(0..=255).contains(&y)
                                || !(0..=255).contains(&z)
                            {
                                continue;
                            }
                            let q = [x as u8, y as u8, z as u8];
                            if left.remove(&q) {
                                comp.push(q);
                                stack.push(q);
                            }
                        }
                    }
                }
            }
            out.push(comp);
        }
        out
    }

    fn extent(ps: &[[u8; 3]]) -> (u8, u8, u8) {
        let span = |a: usize| {
            let lo = ps.iter().map(|p| p[a]).min().unwrap();
            let hi = ps.iter().map(|p| p[a]).max().unwrap();
            hi - lo + 1
        };
        (span(0), span(1), span(2))
    }

    #[test]
    fn eoc_phase2_shows_mouth() {
        assert_eq!(eoc_parts_visible(1), (true, false));
        assert_eq!(eoc_parts_visible(2), (false, true));
    }

    #[test]
    fn boss_rig_parts_exist() {
        let dir = boss_dir();
        let r = boss_rig();
        for p in &r.parts {
            assert!(
                dir.join(&p.file).is_file(),
                "rig names missing part {}",
                p.file
            );
        }
        for n in [
            "ball", "iris", "mouth", "tendril1", "tendril2", "tendril3", "tendril4",
        ] {
            assert!(r.parts.iter().any(|p| p.name == n), "rig has no {n} part");
        }
        assert_eq!(game_dims(&part_model("ball")), (64, 64, 64));
    }

    #[test]
    fn boss_rig_offsets_are_16_scale() {
        let r = boss_rig();
        assert_part(&r, "ball", [32.0, 32.0, 32.0], [0.0, 55.0, 0.0], None);
        assert_part(
            &r,
            "iris",
            [20.5, 20.5, 0.0],
            [0.0, 0.0, 31.0],
            Some("ball"),
        );
        assert_part(
            &r,
            "mouth",
            [27.5, 27.5, 0.0],
            [0.0, 0.0, 30.0],
            Some("ball"),
        );
        for (name, x, y) in [
            ("tendril1", 12.0, 12.0),
            ("tendril2", -12.0, 12.0),
            ("tendril3", 12.0, -12.0),
            ("tendril4", -12.0, -12.0),
        ] {
            assert_part(&r, name, [4.0, 4.0, 36.0], [x, y, -20.0], Some("ball"));
        }
    }

    #[test]
    fn boss_models_are_16_scale() {
        assert_eq!(game_dims(&part_model("ball")), (64, 64, 64));
        assert_eq!(game_dims(&part_model("iris")), (42, 42, 4));
        assert_eq!(game_dims(&part_model("mouth")), (56, 56, 12));
        for n in ["tendril1", "tendril2", "tendril3", "tendril4"] {
            assert_eq!(game_dims(&part_model(n)), (8, 8, 36), "{n}");
        }
    }

    #[test]
    fn boss_ball_spec() {
        let vs = game_voxels(&part_model("ball"));
        let cols = colours(&vs);
        assert_eq!(cols.len(), 3, "ball colours: {cols:?}");
        for c in [SCLERA, SCLERA_D, VEIN] {
            assert!(cols.contains(&c), "ball misses colour {c:?}");
        }

        let veins = positions(&vs, &[VEIN]);
        let comps = components(&veins);
        assert!(
            comps.len() >= 20,
            "distinct 1-voxel vein paths: {}",
            comps.len()
        );

        let set: HashSet<[u8; 3]> = vs.iter().map(|(p, _)| *p).collect();
        let vein_set: HashSet<[u8; 3]> = veins.iter().copied().collect();
        for &p in &vein_set {
            let block = (0..2).all(|dx| {
                (0..2).all(|dy| {
                    (0..2).all(|dz| vein_set.contains(&[p[0] + dx, p[1] + dy, p[2] + dz]))
                })
            });
            assert!(!block, "2x2x2 all-VEIN block at {p:?}");
        }

        assert!(
            (10_000..=40_000).contains(&vs.len()),
            "ball voxels: {}",
            vs.len()
        );

        // The ball is a 2-voxel-thick shell: the visible surface survives and
        // no voxel sits more than two steps from air. (A 2-voxel shell always
        // has an inner layer whose six neighbours are filled, so "every voxel
        // touches air" can only hold for a 1-voxel shell.)
        let mut depth: HashMap<[u8; 3], u32> = HashMap::new();
        let mut queue = VecDeque::new();
        for &p in &set {
            if neighbours(p).any(|q| !set.contains(&q)) {
                depth.insert(p, 1);
                queue.push_back(p);
            }
        }
        while let Some(p) = queue.pop_front() {
            let d = depth[&p];
            for q in neighbours(p) {
                if set.contains(&q) && !depth.contains_key(&q) {
                    depth.insert(q, d + 1);
                    queue.push_back(q);
                }
            }
        }
        assert_eq!(depth.len(), set.len(), "ball has unreachable voxels");
        assert_eq!(
            *depth.values().max().unwrap(),
            2,
            "ball shell is not 2 voxels thick"
        );
    }

    #[test]
    fn boss_iris_spec() {
        let vs = game_voxels(&part_model("iris"));
        let cols = colours(&vs);
        assert_eq!(cols.len(), 4, "iris colours: {cols:?}");
        for c in [PUPIL, IRIS_L, IRIS, IRIS_D] {
            assert!(cols.contains(&c), "iris misses colour {c:?}");
        }
        let pupil = positions(&vs, &[PUPIL]);
        let (px, py, _) = extent(&pupil);
        assert_eq!((px, py), (12, 12), "pupil extent {px}x{py}");
    }

    #[test]
    fn boss_mouth_spec() {
        let vs = game_voxels(&part_model("mouth"));
        let cols = colours(&vs);
        assert!(cols.contains(&FLESH), "mouth misses FLESH");
        assert!(cols.contains(&FLESH_D), "mouth misses FLESH_D");
        assert!(
            cols.contains(&TOOTH) && cols.contains(&TOOTH_D),
            "tooth shades"
        );

        let teeth = positions(&vs, &[TOOTH, TOOTH_D]);
        assert_eq!(components(&teeth).len(), 12, "tooth components");

        // The throat is open: the centre column of the front layers is empty.
        let set: HashSet<[u8; 3]> = vs.iter().map(|(p, _)| *p).collect();
        for x in 27..=28 {
            for y in 27..=28 {
                for z in 0..=3 {
                    assert!(
                        !set.contains(&[x, y, z]),
                        "centre column filled at {x},{y},{z}"
                    );
                }
            }
        }
    }

    #[test]
    fn boss_rig_animation_unchanged() {
        let r = boss_rig();
        let idle = &r.anims["idle"];
        assert!((idle.len - 1.6).abs() < 1e-6);
        assert!(idle.looping);
        assert_eq!(idle.keys.len(), 12);
        let want = [
            ("tendril1", 0.0, [32.0, -8.0, 0.0]),
            ("tendril1", 0.8, [12.0, -36.0, 0.0]),
            ("tendril1", 1.6, [32.0, -8.0, 0.0]),
            ("tendril2", 0.0, [12.0, 8.0, 0.0]),
            ("tendril2", 0.8, [32.0, 36.0, 0.0]),
            ("tendril2", 1.6, [12.0, 8.0, 0.0]),
            ("tendril3", 0.0, [-12.0, -8.0, 0.0]),
            ("tendril3", 0.8, [-32.0, -36.0, 0.0]),
            ("tendril3", 1.6, [-12.0, -8.0, 0.0]),
            ("tendril4", 0.0, [-32.0, 8.0, 0.0]),
            ("tendril4", 0.8, [-12.0, 36.0, 0.0]),
            ("tendril4", 1.6, [-32.0, 8.0, 0.0]),
        ];
        for (part, t, rot) in want {
            let k = idle
                .keys
                .iter()
                .find(|k| k.part == part && (k.t - t).abs() < 1e-6)
                .unwrap_or_else(|| panic!("idle has no {part} key at {t}"));
            near(k.rot_deg, rot, &format!("{part} at {t}"));
            near(k.offset, [0.0; 3], &format!("{part} offset at {t}"));
        }
    }
}
