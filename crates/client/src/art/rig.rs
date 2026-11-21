//! Rigid-part rigs: part pivots plus keyframed rotations and offsets.

use std::collections::HashMap;
use std::path::Path;

use bevy::math::{EulerRot, Quat, Vec3};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Part {
    pub name: String,
    /// `.vox` file next to `rig.ron`.
    pub file: String,
    /// Rotation pivot in the part's voxel units.
    pub pivot: [f32; 3],
    /// Rest position of the pivot, relative to the parent (or the rig root).
    pub offset: [f32; 3],
    #[serde(default)]
    pub parent: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Key {
    pub t: f32,
    pub part: String,
    pub rot_deg: [f32; 3],
    pub offset: [f32; 3],
    /// Squash and stretch; omitted keys leave the part unscaled.
    #[serde(default = "unit_scale")]
    pub scale: [f32; 3],
}

fn unit_scale() -> [f32; 3] {
    [1.0; 3]
}

#[derive(Clone, Debug, Deserialize)]
pub struct Anim {
    pub len: f32,
    pub looping: bool,
    pub keys: Vec<Key>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Rig {
    pub parts: Vec<Part>,
    #[serde(default)]
    pub anims: HashMap<String, Anim>,
}

fn pose(k: &Key) -> (Vec3, Vec3) {
    (Vec3::from(k.rot_deg), Vec3::from(k.offset))
}

fn quat(rot_deg: Vec3) -> Quat {
    Quat::from_euler(
        EulerRot::XYZ,
        rot_deg.x.to_radians(),
        rot_deg.y.to_radians(),
        rot_deg.z.to_radians(),
    )
}

/// Pose of `part` at time `t`: linear interpolation between its keys (holding
/// the first and last, or wrapping last -> first when the anim loops).
/// Identity when the part has no keys.
pub fn sample(anim: &Anim, part: &str, t: f32) -> (Quat, Vec3) {
    let mut keys: Vec<&Key> = anim.keys.iter().filter(|k| k.part == part).collect();
    keys.sort_by(|a, b| a.t.total_cmp(&b.t));
    let (Some(first), Some(last)) = (keys.first(), keys.last()) else {
        return (Quat::IDENTITY, Vec3::ZERO);
    };
    let t = if anim.looping && anim.len > 0.0 {
        t.rem_euclid(anim.len)
    } else {
        t
    };
    let blend = |a: &Key, b: &Key, f: f32| {
        let (ra, oa) = pose(a);
        let (rb, ob) = pose(b);
        (quat(ra.lerp(rb, f)), oa.lerp(ob, f))
    };
    if t <= first.t {
        // Looping anims blend from the last key over the wrap.
        if anim.looping && keys.len() > 1 && first.t > 0.0 {
            let span = anim.len - last.t + first.t;
            return blend(last, first, (anim.len - last.t + t) / span);
        }
        return blend(first, first, 0.0);
    }
    for w in keys.windows(2) {
        if t <= w[1].t {
            let f = (t - w[0].t) / (w[1].t - w[0].t).max(1e-6);
            return blend(w[0], w[1], f);
        }
    }
    if anim.looping && anim.len > last.t {
        let span = anim.len - last.t + first.t;
        return blend(last, first, ((t - last.t) / span).min(1.0));
    }
    blend(last, last, 0.0)
}

/// Scale of `part` at time `t`, interpolated like [`sample`]; ONE without keys.
pub fn sample_scale(anim: &Anim, part: &str, t: f32) -> Vec3 {
    let mut keys: Vec<&Key> = anim.keys.iter().filter(|k| k.part == part).collect();
    keys.sort_by(|a, b| a.t.total_cmp(&b.t));
    let (Some(first), Some(last)) = (keys.first(), keys.last()) else {
        return Vec3::ONE;
    };
    let t = if anim.looping && anim.len > 0.0 {
        t.rem_euclid(anim.len)
    } else {
        t
    };
    let s = |k: &Key| Vec3::from(k.scale);
    if t <= first.t {
        if anim.looping && keys.len() > 1 && first.t > 0.0 {
            let span = anim.len - last.t + first.t;
            return s(last).lerp(s(first), (anim.len - last.t + t) / span);
        }
        return s(first);
    }
    for w in keys.windows(2) {
        if t <= w[1].t {
            let f = (t - w[0].t) / (w[1].t - w[0].t).max(1e-6);
            return s(w[0]).lerp(s(w[1]), f);
        }
    }
    if anim.looping && anim.len > last.t {
        let span = anim.len - last.t + first.t;
        return s(last).lerp(s(first), ((t - last.t) / span).min(1.0));
    }
    s(last)
}

/// Non-fatal loader; the error names the file.
pub fn try_load(dir: &Path) -> Result<Rig, String> {
    let path = dir.join("rig.ron");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("missing art: {}: {e}", path.display()))?;
    ron::from_str(&text).map_err(|e| format!("bad art: {}: {e}", path.display()))
}

/// Reads `dir/rig.ron`, exiting with a message that names it on any error.
pub fn load(dir: &Path) -> Rig {
    try_load(dir).unwrap_or_else(|e| crate::view::fatal(e))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::art::vox::{self, VoxModel};

    fn key(t: f32, rot: f32) -> Key {
        Key {
            t,
            part: "arm".into(),
            rot_deg: [0.0, 0.0, rot],
            offset: [0.0, t, 0.0],
            scale: [1.0; 3],
        }
    }

    #[test]
    fn sample_interpolates_midpoint() {
        let a = Anim {
            len: 1.0,
            looping: false,
            keys: vec![key(0.0, 0.0), key(1.0, 90.0)],
        };
        let (q, o) = sample(&a, "arm", 0.5);
        let want = Quat::from_rotation_z(45f32.to_radians());
        assert!(q.angle_between(want) < 1e-4);
        assert!((o.y - 0.5).abs() < 1e-6);
    }

    #[test]
    fn sample_identity_without_keys() {
        let a = Anim {
            len: 1.0,
            looping: true,
            keys: vec![key(0.0, 10.0)],
        };
        assert_eq!(sample(&a, "leg", 0.3), (Quat::IDENTITY, Vec3::ZERO));
    }

    #[test]
    fn loads_committed_test_rig() {
        let r = try_load(&crate::art::art_dir().join("models/test")).unwrap();
        assert_eq!(r.parts[0].file, "cube.vox");
        assert!(r.anims.contains_key("spin"));
    }

    #[test]
    fn missing_rig_reports_path() {
        let e = try_load(Path::new("/nonexistent/x")).err().unwrap();
        assert!(e.contains("/nonexistent/x/rig.ron"), "{e}");
    }

    // ---- enemy art at 1/16 voxels -------------------------------------------

    const ENEMY_DIRS: [&str; 4] = ["slime", "zombie", "demon_eye", "eater_of_souls"];

    fn art(kind: &str) -> std::path::PathBuf {
        crate::art::art_dir().join("models").join(kind)
    }

    fn enemy_rig(kind: &str) -> Rig {
        try_load(&art(kind)).unwrap_or_else(|e| panic!("{e}"))
    }

    fn part_model(kind: &str, file: &str) -> VoxModel {
        vox::try_load(&art(kind).join(file)).unwrap_or_else(|e| panic!("{e}"))
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

    fn find<'a>(r: &'a Rig, name: &str) -> &'a Part {
        r.parts
            .iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("rig has no part {name}"))
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
        let p = find(r, name);
        near(p.pivot, pivot, &format!("{name} pivot"));
        near(p.offset, offset, &format!("{name} offset"));
        assert_eq!(p.parent.as_deref(), parent, "{name} parent");
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
    fn enemy_rigs_load_and_parts_exist() {
        for kind in ENEMY_DIRS.iter().copied().chain(["guide", "merchant"]) {
            let dir = art(kind);
            let r = try_load(&dir).unwrap_or_else(|e| panic!("{e}"));
            for p in &r.parts {
                assert!(
                    dir.join(&p.file).is_file(),
                    "{kind}: rig names missing part {}",
                    p.file
                );
            }
        }
        let expected: [(&str, &[&str]); 4] = [
            ("slime", &["body", "eyes", "nucleus"]),
            (
                "zombie",
                &["torso", "head", "arm_l", "arm_r", "leg_l", "leg_r"],
            ),
            ("demon_eye", &["ball", "tail1", "tail2"]),
            ("eater_of_souls", &["head", "wing_l", "wing_r"]),
        ];
        for (kind, names) in expected {
            let r = enemy_rig(kind);
            for name in names {
                assert!(
                    r.parts.iter().any(|p| p.name == *name),
                    "{kind}: no {name} part"
                );
            }
        }
    }

    #[test]
    fn enemy_rig_offsets_are_16_scale() {
        let slime = enemy_rig("slime");
        assert_part(&slime, "body", [11.0, 0.0, 9.0], [0.0; 3], None);
        assert_part(
            &slime,
            "eyes",
            [9.0, 0.0, 0.0],
            [0.0, 6.0, 7.0],
            Some("body"),
        );
        assert_part(
            &slime,
            "nucleus",
            [3.0, 3.0, 3.0],
            [0.0, 6.0, 0.0],
            Some("body"),
        );

        let zombie = enemy_rig("zombie");
        assert_part(&zombie, "torso", [4.0, 0.0, 3.0], [0.0, 8.0, 0.0], None);
        assert_part(&zombie, "head", [6.0, 0.0, 6.0], [0.0, 20.0, 0.0], None);
        for (name, x) in [("arm_l", 6.0), ("arm_r", -6.0)] {
            assert_part(&zombie, name, [2.0, 12.0, 2.0], [x, 20.0, 0.0], None);
        }
        for (name, x) in [("leg_l", 2.0), ("leg_r", -2.0)] {
            assert_part(&zombie, name, [2.0, 8.0, 2.0], [x, 8.0, 0.0], None);
        }

        let eye = enemy_rig("demon_eye");
        assert_part(&eye, "ball", [12.0, 12.0, 12.0], [0.0, 16.0, 0.0], None);
        assert_part(
            &eye,
            "tail1",
            [5.0, 5.0, 16.0],
            [0.0, 0.0, -12.0],
            Some("ball"),
        );
        assert_part(
            &eye,
            "tail2",
            [3.0, 3.0, 16.0],
            [0.0, 0.0, -16.0],
            Some("tail1"),
        );

        let eater = enemy_rig("eater_of_souls");
        assert_part(&eater, "head", [15.0, 14.0, 13.0], [0.0, 16.0, 0.0], None);
        assert_part(
            &eater,
            "wing_l",
            [22.0, 12.0, 4.0],
            [-10.0, 0.0, 0.0],
            Some("head"),
        );
        assert_part(
            &eater,
            "wing_r",
            [0.0, 12.0, 4.0],
            [10.0, 0.0, 0.0],
            Some("head"),
        );

        // Doubled animation offsets; rotations are unchanged.
        let ball = eye.anims["idle"]
            .keys
            .iter()
            .find(|k| k.part == "ball" && k.t == 0.5)
            .unwrap();
        near(ball.offset, [0.0, 1.2, 0.0], "demon eye ball bob");
        let head = eater.anims["idle"]
            .keys
            .iter()
            .find(|k| k.part == "head" && k.t == 0.3)
            .unwrap();
        near(head.offset, [0.0, 1.0, 0.0], "eater head bob");
        let wing = eater.anims["idle"]
            .keys
            .iter()
            .find(|k| k.part == "wing_l" && k.t == 0.3)
            .unwrap();
        near(wing.rot_deg, [0.0, 0.0, 35.0], "eater wing rotation");
        let idle = zombie.anims["idle"]
            .keys
            .iter()
            .find(|k| k.part == "torso" && k.t == 1.0)
            .unwrap();
        near(idle.offset, [0.0, 0.8, 0.0], "zombie idle bob");
        let walk = zombie.anims["walk"]
            .keys
            .iter()
            .find(|k| k.part == "torso" && k.t == 0.2)
            .unwrap();
        near(walk.offset, [0.0, 1.0, 0.0], "zombie walk bob");
        let arm = zombie.anims["idle"]
            .keys
            .iter()
            .find(|k| k.part == "arm_l" && k.t == 0.0)
            .unwrap();
        near(arm.rot_deg, [-80.0, 0.0, 0.0], "zombie shambling arms");
    }

    #[test]
    fn enemy_models_are_16_scale() {
        assert_eq!(game_dims(&part_model("slime", "body.vox")), (24, 18, 20));
        let eyes = part_model("slime", "eyes.vox");
        let (ex, ey, ez) = game_dims(&eyes);
        assert!(ex <= 18 && ey <= 3 && ez <= 1, "slime eyes {ex}x{ey}x{ez}");
        assert!(!eyes.voxels.is_empty(), "slime eyes empty");
        let nucleus = part_model("slime", "nucleus.vox");
        let (nx, ny, nz) = game_dims(&nucleus);
        assert!(nx <= 8 && ny <= 8 && nz <= 8, "nucleus {nx}x{ny}x{nz}");
        assert!(!nucleus.voxels.is_empty(), "nucleus empty");

        for (file, want) in [
            ("head.vox", (12, 12, 12)),
            ("torso.vox", (8, 12, 6)),
            ("arm_l.vox", (4, 12, 4)),
            ("arm_r.vox", (4, 12, 4)),
            ("leg_l.vox", (4, 8, 6)),
            ("leg_r.vox", (4, 8, 6)),
        ] {
            assert_eq!(
                game_dims(&part_model("zombie", file)),
                want,
                "zombie {file}"
            );
        }

        assert_eq!(
            game_dims(&part_model("demon_eye", "ball.vox")),
            (24, 24, 24)
        );
        assert_eq!(
            game_dims(&part_model("demon_eye", "tail1.vox")),
            (10, 10, 16)
        );
        assert_eq!(game_dims(&part_model("demon_eye", "tail2.vox")), (6, 6, 16));

        assert_eq!(
            game_dims(&part_model("eater_of_souls", "head.vox")),
            (32, 38, 28)
        );
        assert_eq!(
            game_dims(&part_model("eater_of_souls", "wing_l.vox")),
            (24, 24, 8)
        );
        assert_eq!(
            game_dims(&part_model("eater_of_souls", "wing_r.vox")),
            (24, 24, 8)
        );
    }

    #[test]
    fn slime_spec() {
        let body = game_voxels(&part_model("slime", "body.vox"));
        let gel: BTreeSet<[u8; 4]> = colours(&body).into_iter().filter(|c| c[3] < 255).collect();
        assert_eq!(gel.len(), 2, "slime body gel shades: {gel:?}");
        assert!(
            gel.iter().all(|c| c[3] == 190),
            "gel shades must be alpha 190: {gel:?}"
        );
        let gloss = body.iter().filter(|(_, c)| c[3] == 255).count();
        assert_eq!(gloss, 3, "slime gloss voxels");

        let eyes = game_voxels(&part_model("slime", "eyes.vox"));
        assert_eq!(eyes.len(), 12, "slime eye voxels");
        let dark: Vec<[u8; 3]> = eyes
            .iter()
            .filter(|(_, c)| (c[0] as u16 + c[1] as u16 + c[2] as u16) < 150)
            .map(|(p, _)| *p)
            .collect();
        assert_eq!(dark.len(), 12, "slime dark eye voxels");
        let mid = (dark.iter().map(|p| p[0]).min().unwrap()
            + dark.iter().map(|p| p[0]).max().unwrap())
            / 2;
        let (mut lo, mut hi) = (Vec::new(), Vec::new());
        for p in &dark {
            if p[0] < mid {
                lo.push(*p);
            } else if p[0] > mid {
                hi.push(*p);
            }
        }
        assert_eq!(lo.len() + hi.len(), 12, "two separated eye patches");
        for ps in [lo, hi] {
            assert_eq!(ps.len(), 6, "each eye patch has 6 voxels");
            assert_eq!(extent(&ps), (2, 3, 1), "eye patch is 2x3x1");
        }
    }

    #[test]
    fn zombie_spec() {
        const SHIRT: [[u8; 4]; 2] = [[0x5f, 0x7a, 0x8f, 255], [0x46, 0x59, 0x6b, 255]];
        const PANTS: [[u8; 4]; 2] = [[0x5a, 0x4f, 0x6e, 255], [0x40, 0x38, 0x50, 255]];
        const BONE: [u8; 4] = [0xde, 0xd2, 0xb2, 255];

        let torso = game_voxels(&part_model("zombie", "torso.vox"));
        let not_clothing = |c: &[u8; 4]| {
            // skin, skin shadow, bone, bone shadow, boot.
            c[..3] == [0x7d, 0x9d, 0x6a]
                || c[..3] == [0x5d, 0x7a, 0x52]
                || c[..3] == [0xde, 0xd2, 0xb2]
                || c[..3] == [0xb8, 0xa9, 0x8a]
                || c[..3] == [0x3d, 0x2f, 0x2a]
        };
        let clothing: BTreeSet<[u8; 4]> = colours(&torso)
            .into_iter()
            .filter(|c| !not_clothing(c))
            .collect();
        assert_eq!(clothing.len(), 4, "torso clothing shades: {clothing:?}");
        for c in SHIRT.iter().chain(PANTS.iter()) {
            assert!(clothing.contains(c), "torso misses clothing shade {c:?}");
        }
        let shirts = clothing.iter().filter(|c| c[1] > c[0]).count();
        let pants = clothing.len() - shirts;
        assert_eq!((shirts, pants), (2, 2), "two shirt and two pants shades");

        let bone: usize = ["torso", "head", "arm_l", "arm_r", "leg_l", "leg_r"]
            .iter()
            .map(|f| {
                game_voxels(&part_model("zombie", &format!("{f}.vox")))
                    .iter()
                    .filter(|(_, c)| *c == BONE)
                    .count()
            })
            .sum();
        assert!(bone >= 8, "zombie bone voxels: {bone}");

        let bottom = torso.iter().filter(|(p, _)| p[1] == 0).count();
        assert!(bottom < 8 * 6, "torso bottom row is not ragged");
    }

    #[test]
    fn demon_eye_spec() {
        let ball = game_voxels(&part_model("demon_eye", "ball.vox"));
        assert_eq!(
            game_dims(&part_model("demon_eye", "ball.vox")),
            (24, 24, 24)
        );
        assert!(
            ball.iter().any(|(_, c)| c[..3] == [0xc4, 0x33, 0x3a]),
            "demon eye has no VEIN voxels"
        );
        let mut iris: BTreeSet<[u8; 4]> = BTreeSet::new();
        for (p, c) in &ball {
            let d = ((p[0] as f32 - 11.5).powi(2) + (p[1] as f32 - 11.5).powi(2)).sqrt();
            let sclera = c[..3] == [0xef, 0xe6, 0xe0] || c[..3] == [0xd6, 0xc8, 0xc4];
            if p[2] >= 20 && d <= 7.0 && !sclera {
                iris.insert(*c);
            }
        }
        assert!(iris.len() >= 3, "front iris colours: {iris:?}");
        for tail in ["tail1.vox", "tail2.vox"] {
            assert!(!part_model("demon_eye", tail).voxels.is_empty(), "{tail}");
        }
    }

    #[test]
    fn eater_spec() {
        let head = game_voxels(&part_model("eater_of_souls", "head.vox"));
        assert_eq!(
            game_dims(&part_model("eater_of_souls", "head.vox")),
            (32, 38, 28)
        );
        let maw = head
            .iter()
            .filter(|(p, _)| (12..=19).contains(&p[0]) && (15..=22).contains(&p[1]) && p[2] >= 24)
            .count();
        assert_eq!(maw, 0, "front-centre maw is not empty");

        const BONE: [u8; 4] = [0xde, 0xd2, 0xb2, 255];
        let white: BTreeSet<[u8; 3]> = head
            .iter()
            .filter(|(_, c)| *c == BONE)
            .map(|(p, _)| *p)
            .collect();
        assert!(white.len() >= 8, "eater bone voxels: {}", white.len());
        let spikes = white
            .iter()
            .filter(|p| white.contains(&[p[0], p[1] + 1, p[2]]))
            .count();
        assert!(spikes >= 4, "eater 2-voxel teeth: {spikes}");

        let hides: BTreeSet<[u8; 4]> = colours(&head)
            .into_iter()
            .filter(|c| c[2] > c[1] && c[0] as u16 + c[1] as u16 + c[2] as u16 > 150)
            .collect();
        assert!(hides.len() >= 3, "eater hide shades: {hides:?}");
    }
}
