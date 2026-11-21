//! Character appearance, hair styles and tinted player models.

use std::path::Path;
use std::sync::OnceLock;

use terravibe_sim::character::{Appearance, HairStyle};

use crate::art::mesh_data::MeshData;
use crate::art::vox::VoxModel;
use crate::art::voxmesh::mesh;

// Base palette colors from _player_base.py
const BASE_SKIN: [u8; 3] = [232, 169, 130];
const BASE_SKIN_D: [u8; 3] = [201, 134, 96];
const BASE_HAIR: [u8; 3] = [107, 68, 35];
const BASE_HAIR_D: [u8; 3] = [70, 41, 15];
const BASE_HAIR_L: [u8; 3] = [142, 95, 49];
const BASE_EYE_P: [u8; 3] = [42, 58, 140];
const BASE_SHIRT: [u8; 3] = [242, 242, 242];
const BASE_SHIRT_D: [u8; 3] = [197, 201, 214];
const BASE_PANTS: [u8; 3] = [61, 95, 196];
const BASE_PANTS_D: [u8; 3] = [43, 68, 150];

pub fn shade(c: [u8; 3], factor: f32) -> [u8; 3] {
    [
        (c[0] as f32 * factor).clamp(0.0, 255.0) as u8,
        (c[1] as f32 * factor).clamp(0.0, 255.0) as u8,
        (c[2] as f32 * factor).clamp(0.0, 255.0) as u8,
    ]
}

// ---------------------------------------------------------------------------
// Hair style voxel offsets on a 12x12x12 head box (x: 0..11, y: 0..11, z: 0..11)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HairTone {
    Base,
    Dark,
    Light,
}

#[allow(dead_code)]
const HAIR_BALD: &[[i8; 3]] = &[];

fn push_box(
    out: &mut Vec<([i8; 3], HairTone)>,
    x0: i8,
    y0: i8,
    z0: i8,
    x1: i8,
    y1: i8,
    z1: i8,
    tone: HairTone,
) {
    for x in x0..=x1 {
        for y in y0..=y1 {
            for z in z0..=z1 {
                if let Some(existing) = out.iter_mut().find(|(p, _)| *p == [x, y, z]) {
                    existing.1 = tone;
                } else {
                    out.push(([x, y, z], tone));
                }
            }
        }
    }
}

fn build_long_hair_shaded() -> Vec<([i8; 3], HairTone)> {
    let mut v = Vec::new();
    // Chamfered crown with highlight
    push_box(&mut v, 2, 11, 2, 9, 11, 9, HairTone::Base);
    push_box(&mut v, 3, 11, 3, 8, 11, 8, HairTone::Light);
    push_box(&mut v, 1, 10, 1, 10, 10, 10, HairTone::Base);
    // Asymmetric parted bangs
    push_box(&mut v, 2, 9, 11, 9, 10, 11, HairTone::Base);
    push_box(&mut v, 3, 10, 11, 6, 10, 11, HairTone::Light);
    push_box(&mut v, 2, 8, 11, 4, 8, 11, HairTone::Dark);
    push_box(&mut v, 8, 8, 11, 9, 8, 11, HairTone::Dark);
    // Layered side locks framing cheeks
    push_box(&mut v, 0, 3, 2, 0, 9, 8, HairTone::Dark);
    push_box(&mut v, 11, 3, 2, 11, 9, 8, HairTone::Dark);
    push_box(&mut v, 1, 2, 2, 1, 9, 6, HairTone::Base);
    push_box(&mut v, 10, 2, 2, 10, 9, 6, HairTone::Base);
    push_box(&mut v, 1, 5, 9, 1, 9, 10, HairTone::Base);
    push_box(&mut v, 10, 5, 9, 10, 9, 10, HairTone::Base);
    // Flowing tapered back hair down to y=1, z=0
    push_box(&mut v, 1, 4, 1, 10, 10, 1, HairTone::Base);
    push_box(&mut v, 2, 3, 0, 9, 10, 0, HairTone::Dark);
    push_box(&mut v, 3, 2, 0, 8, 2, 0, HairTone::Dark);
    push_box(&mut v, 4, 1, 0, 7, 1, 0, HairTone::Dark);
    v
}

fn build_spiky_hair_shaded() -> Vec<([i8; 3], HairTone)> {
    let mut v = Vec::new();
    // Chamfered crown base
    push_box(&mut v, 2, 11, 2, 9, 11, 9, HairTone::Base);
    push_box(&mut v, 1, 10, 1, 10, 10, 10, HairTone::Dark);
    // Choppy textured bangs
    push_box(&mut v, 2, 9, 11, 9, 10, 11, HairTone::Base);
    push_box(&mut v, 2, 8, 11, 3, 8, 11, HairTone::Dark);
    push_box(&mut v, 5, 8, 11, 6, 8, 11, HairTone::Base);
    push_box(&mut v, 8, 8, 11, 9, 8, 11, HairTone::Dark);
    // Sideburns
    push_box(&mut v, 0, 6, 4, 0, 9, 7, HairTone::Dark);
    push_box(&mut v, 11, 6, 4, 11, 9, 7, HairTone::Dark);
    // Back nape
    push_box(&mut v, 2, 5, 0, 9, 10, 0, HairTone::Dark);
    // 6 sculpted tapered spikes rising to y=12 (dark roots, light tips)
    for &(sx, sz) in &[(2, 2), (7, 2), (2, 7), (7, 7), (4, 4), (6, 8)] {
        push_box(&mut v, sx, 11, sz, sx + 1, 11, sz + 1, HairTone::Dark);
        push_box(&mut v, sx, 12, sz, sx, 12, sz, HairTone::Light);
    }
    v
}

fn build_mohawk_hair_shaded() -> Vec<([i8; 3], HairTone)> {
    let mut v = Vec::new();
    // Wider base along crown (x=4..=7, y=11, z=1..=10)
    push_box(&mut v, 4, 11, 1, 7, 11, 10, HairTone::Dark);
    // Central arched ridge (x=5..=6, peak y=12) with highlight crest
    push_box(&mut v, 5, 11, 1, 6, 12, 10, HairTone::Base);
    push_box(&mut v, 5, 12, 2, 6, 12, 9, HairTone::Light);
    // Front brow peak
    push_box(&mut v, 5, 10, 11, 6, 10, 11, HairTone::Base);
    // Taper down nape (z=0, y=3..=10)
    push_box(&mut v, 5, 6, 0, 6, 10, 0, HairTone::Base);
    push_box(&mut v, 5, 3, 0, 6, 5, 0, HairTone::Dark);
    v
}

fn build_ponytail_hair_shaded() -> Vec<([i8; 3], HairTone)> {
    let mut v = Vec::new();
    // Sleek chamfered crown + swept bangs/temples
    push_box(&mut v, 2, 11, 2, 9, 11, 9, HairTone::Base);
    push_box(&mut v, 3, 11, 3, 8, 11, 8, HairTone::Light);
    push_box(&mut v, 1, 10, 1, 10, 10, 10, HairTone::Base);
    push_box(&mut v, 2, 9, 11, 9, 10, 11, HairTone::Base);
    push_box(&mut v, 4, 10, 11, 8, 10, 11, HairTone::Light);
    push_box(&mut v, 0, 6, 3, 0, 9, 7, HairTone::Dark);
    push_box(&mut v, 11, 6, 3, 11, 9, 7, HairTone::Dark);
    // Back sweep into gathered hair band at z=0, y=6..=8, x=4..=7
    push_box(&mut v, 2, 7, 0, 9, 10, 0, HairTone::Base);
    push_box(&mut v, 4, 6, 0, 7, 8, 0, HairTone::Dark);
    // Curved tapered flowing ponytail at z=-1, y=1..=8
    push_box(&mut v, 5, 4, -1, 6, 8, -1, HairTone::Base);
    push_box(&mut v, 5, 6, -1, 6, 7, -1, HairTone::Light);
    push_box(&mut v, 5, 1, -1, 6, 3, -1, HairTone::Dark);
    v
}

pub fn hair_voxels_shaded(style: HairStyle) -> Vec<([i8; 3], HairTone)> {
    match style {
        HairStyle::Bald => Vec::new(),
        HairStyle::Short => HAIR_SHORT.iter().map(|&p| (p, HairTone::Base)).collect(),
        HairStyle::Long => build_long_hair_shaded(),
        HairStyle::Spiky => build_spiky_hair_shaded(),
        HairStyle::Mohawk => build_mohawk_hair_shaded(),
        HairStyle::Ponytail => build_ponytail_hair_shaded(),
    }
}

#[allow(dead_code)]
pub fn hair_voxels(style: HairStyle) -> &'static [[i8; 3]] {
    static LONG: OnceLock<Vec<[i8; 3]>> = OnceLock::new();
    static SPIKY: OnceLock<Vec<[i8; 3]>> = OnceLock::new();
    static MOHAWK: OnceLock<Vec<[i8; 3]>> = OnceLock::new();
    static PONYTAIL: OnceLock<Vec<[i8; 3]>> = OnceLock::new();
    let extract = |s| hair_voxels_shaded(s).into_iter().map(|(p, _)| p).collect();
    match style {
        HairStyle::Bald => HAIR_BALD,
        HairStyle::Short => HAIR_SHORT,
        HairStyle::Long => LONG.get_or_init(|| extract(HairStyle::Long)),
        HairStyle::Spiky => SPIKY.get_or_init(|| extract(HairStyle::Spiky)),
        HairStyle::Mohawk => MOHAWK.get_or_init(|| extract(HairStyle::Mohawk)),
        HairStyle::Ponytail => PONYTAIL.get_or_init(|| extract(HairStyle::Ponytail)),
    }
}

#[allow(dead_code)]
const HAIR_SHORT: &[[i8; 3]] = &[
    // Crown cap (y = 11)
    [2, 11, 2],
    [3, 11, 2],
    [4, 11, 2],
    [5, 11, 2],
    [6, 11, 2],
    [7, 11, 2],
    [8, 11, 2],
    [9, 11, 2],
    [2, 11, 3],
    [3, 11, 3],
    [4, 11, 3],
    [5, 11, 3],
    [6, 11, 3],
    [7, 11, 3],
    [8, 11, 3],
    [9, 11, 3],
    [2, 11, 4],
    [3, 11, 4],
    [4, 11, 4],
    [5, 11, 4],
    [6, 11, 4],
    [7, 11, 4],
    [8, 11, 4],
    [9, 11, 4],
    [2, 11, 5],
    [3, 11, 5],
    [4, 11, 5],
    [5, 11, 5],
    [6, 11, 5],
    [7, 11, 5],
    [8, 11, 5],
    [9, 11, 5],
    [2, 11, 6],
    [3, 11, 6],
    [4, 11, 6],
    [5, 11, 6],
    [6, 11, 6],
    [7, 11, 6],
    [8, 11, 6],
    [9, 11, 6],
    [2, 11, 7],
    [3, 11, 7],
    [4, 11, 7],
    [5, 11, 7],
    [6, 11, 7],
    [7, 11, 7],
    [8, 11, 7],
    [9, 11, 7],
    [2, 11, 8],
    [3, 11, 8],
    [4, 11, 8],
    [5, 11, 8],
    [6, 11, 8],
    [7, 11, 8],
    [8, 11, 8],
    [9, 11, 8],
    [2, 11, 9],
    [3, 11, 9],
    [4, 11, 9],
    [5, 11, 9],
    [6, 11, 9],
    [7, 11, 9],
    [8, 11, 9],
    [9, 11, 9],
    // Front fringe (y = 10, z = 11)
    [3, 10, 11],
    [4, 10, 11],
    [5, 10, 11],
    [6, 10, 11],
    [7, 10, 11],
    [8, 10, 11],
];

fn in_rounded_head(gx: usize, gy: usize, gz: usize) -> bool {
    let core = (1..=10).contains(&gx) && (1..=10).contains(&gy) && (1..=10).contains(&gz);
    let face = (2..=9).contains(&gx) && (1..=10).contains(&gy) && gz == 11;
    let back = (2..=9).contains(&gx) && (1..=10).contains(&gy) && gz == 0;
    let right = gx == 0 && (2..=9).contains(&gy) && (2..=9).contains(&gz);
    let left = gx == 11 && (2..=9).contains(&gy) && (2..=9).contains(&gz);
    let crown = (2..=9).contains(&gx) && gy == 11 && (2..=9).contains(&gz);
    let chin = (2..=9).contains(&gx) && gy == 0 && (2..=9).contains(&gz);
    let ears = (gx == 0 || gx == 11) && (5..=6).contains(&gy) && (6..=7).contains(&gz);
    core || face || back || right || left || crown || chin || ears
}
/// Allocates a free palette slot (1..=255) if an existing slot wasn't found.
fn alloc_palette_slot(m: &mut VoxModel, rgb: [u8; 3]) -> u8 {
    let used: Vec<bool> = (0..256)
        .map(|idx| m.voxels.iter().any(|v| v[3] as usize == idx))
        .collect();
    let slot = (1..256usize).find(|&idx| !used[idx]).unwrap_or(255);
    m.palette[slot] = [rgb[0], rgb[1], rgb[2], 255];
    slot as u8
}

/// Modifies the palette and voxels of `m` according to `app`.
pub fn tint_player_model(part: &str, m: &mut VoxModel, app: &Appearance) {
    let skin_main = app.skin;
    let skin_d = shade(app.skin, 0.78);
    let hair_main = app.hair_color;
    let hair_d = shade(app.hair_color, 0.70);
    let hair_l = shade(app.hair_color, 1.25);
    let eye_p = app.eyes;
    let shirt_main = app.shirt;
    let shirt_d = shade(app.shirt, 0.78);
    let pants_main = app.pants;
    let pants_d = shade(app.pants, 0.78);

    let mut skin_idx: Option<u8> = None;
    let mut skin_d_idx: Option<u8> = None;
    let mut hair_idx: Option<u8> = None;
    let mut hair_d_idx: Option<u8> = None;
    let mut hair_l_idx: Option<u8> = None;

    for (idx, pal) in m.palette.iter_mut().enumerate().skip(1) {
        let rgb = [pal[0], pal[1], pal[2]];
        if rgb == BASE_SKIN {
            skin_idx = Some(idx as u8);
            pal[0] = skin_main[0];
            pal[1] = skin_main[1];
            pal[2] = skin_main[2];
        } else if rgb == BASE_SKIN_D {
            skin_d_idx = Some(idx as u8);
            pal[0] = skin_d[0];
            pal[1] = skin_d[1];
            pal[2] = skin_d[2];
        } else if rgb == BASE_HAIR {
            hair_idx = Some(idx as u8);
            pal[0] = hair_main[0];
            pal[1] = hair_main[1];
            pal[2] = hair_main[2];
        } else if rgb == BASE_HAIR_D {
            hair_d_idx = Some(idx as u8);
            pal[0] = hair_d[0];
            pal[1] = hair_d[1];
            pal[2] = hair_d[2];
        } else if rgb == BASE_HAIR_L {
            hair_l_idx = Some(idx as u8);
            pal[0] = hair_l[0];
            pal[1] = hair_l[1];
            pal[2] = hair_l[2];
        } else if rgb == BASE_EYE_P {
            pal[0] = eye_p[0];
            pal[1] = eye_p[1];
            pal[2] = eye_p[2];
        } else if rgb == BASE_SHIRT {
            pal[0] = shirt_main[0];
            pal[1] = shirt_main[1];
            pal[2] = shirt_main[2];
        } else if rgb == BASE_SHIRT_D {
            pal[0] = shirt_d[0];
            pal[1] = shirt_d[1];
            pal[2] = shirt_d[2];
        } else if rgb == BASE_PANTS {
            pal[0] = pants_main[0];
            pal[1] = pants_main[1];
            pal[2] = pants_main[2];
        } else if rgb == BASE_PANTS_D {
            pal[0] = pants_d[0];
            pal[1] = pants_d[1];
            pal[2] = pants_d[2];
        }
    }

    if part == "head" && app.hair != HairStyle::Short {
        // Remove existing default hair voxels by their original 1-based palette index
        m.voxels.retain(|&[_, _, _, i]| {
            Some(i) != hair_idx && Some(i) != hair_d_idx && Some(i) != hair_l_idx
        });

        let skin_fill = skin_idx.unwrap_or_else(|| alloc_palette_slot(m, skin_main));
        let skin_d_fill = skin_d_idx.unwrap_or(skin_fill);
        let hair_fill = hair_idx.unwrap_or_else(|| alloc_palette_slot(m, hair_main));
        let hair_d_fill = hair_d_idx.unwrap_or_else(|| alloc_palette_slot(m, hair_d));
        let hair_l_fill = hair_l_idx.unwrap_or_else(|| alloc_palette_slot(m, hair_l));

        // Backfill the chamfered organic head silhouette so removing Short hair
        // never exposes hollow interior holes or fills the 12 chamfered edges.
        let mut occupied = [[[false; 12]; 12]; 12];
        let orig_fy = m.size[1] as usize;
        for &[vx, file_y, vy_game, _] in &m.voxels {
            let gx = vx as usize;
            let gy = vy_game as usize;
            let gz = orig_fy.saturating_sub(1 + file_y as usize);
            if gx < 12 && gy < 12 && gz < 12 {
                occupied[gx][gy][gz] = true;
            }
        }
        for gx in 0..12usize {
            for gy in 0..12usize {
                for gz in 0..12usize {
                    if !in_rounded_head(gx, gy, gz) || occupied[gx][gy][gz] {
                        continue;
                    }
                    let fill_idx = if gx == 0 || gy == 0 || gz == 0 {
                        skin_d_fill
                    } else {
                        skin_fill
                    };
                    let file_y = (orig_fy - 1 - gz) as u8;
                    m.voxels.push([gx as u8, file_y, gy as u8, fill_idx]);
                }
            }
        }

        // Expand m.size[1] if the style has negative gz (e.g. Ponytail at gz = -1).
        // Existing voxels keep their file_y; pivot[2] shifts by +z_shift in player_part_pivot_for.
        let style_voxels = hair_voxels_shaded(app.hair);
        let min_gz = style_voxels.iter().map(|(v, _)| v[2]).min().unwrap_or(0);
        let z_shift: u8 = if min_gz < 0 { (-min_gz) as u8 } else { 0 };
        m.size[1] = m.size[1].saturating_add(z_shift);

        let fy = m.size[1] as i32;
        for ([gx, gy, gz], tone) in style_voxels {
            let vx = gx.clamp(0, m.size[0] as i8 - 1) as u8;
            let vy_game = gy.max(0) as u8;
            if vy_game >= m.size[2] {
                m.size[2] = vy_game + 1;
            }
            let gz_shifted = gz as i32 + z_shift as i32;
            let file_y = (fy - 1 - gz_shifted.clamp(0, fy - 1)) as u8;
            let tone_idx = match tone {
                HairTone::Base => hair_fill,
                HairTone::Dark => hair_d_fill,
                HairTone::Light => hair_l_fill,
            };
            if let Some(existing) = m
                .voxels
                .iter_mut()
                .find(|v| v[0] == vx && v[1] == file_y && v[2] == vy_game)
            {
                existing[3] = tone_idx;
            } else {
                m.voxels.push([vx, file_y, vy_game, tone_idx]);
            }
        }
    }
}

pub fn player_part_pivot_for(part: &str, m: &VoxModel) -> [f32; 3] {
    let mut p = player_part_pivot(part);
    if part == "head" && m.size[1] > 12 {
        p[2] += (m.size[1] - 12) as f32;
    }
    p
}

pub fn player_part_pivot(part: &str) -> [f32; 3] {
    match part {
        "head" => [6.0, 0.0, 6.0],
        "torso" => [6.0, 0.0, 3.0],
        "arm_l" | "arm_r" => [2.0, 12.0, 2.0],
        "leg_l" | "leg_r" => [2.5, 12.0, 2.5],
        _ => [0.0, 0.0, 0.0],
    }
}

/// Builds a customized player part mesh from its base asset and `app`.
pub fn build_player_part_mesh(part: &str, app: &Appearance, dir: &Path) -> Option<MeshData> {
    let path = dir.join(format!("{part}.vox"));
    let mut model = crate::art::vox::try_load(&path).ok()?;
    tint_player_model(part, &mut model, app);
    Some(mesh(&model, player_part_pivot_for(part, &model)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::art::art_dir;

    #[test]
    fn every_hair_style_has_voxels_except_bald() {
        for &style in &HairStyle::ALL {
            let voxels = hair_voxels(style);
            if style == HairStyle::Bald {
                assert!(voxels.is_empty(), "bald hair style must have no voxels");
            } else {
                assert!(
                    !voxels.is_empty(),
                    "{:?} hair style must have voxels",
                    style
                );
            }
        }
    }

    #[test]
    fn hair_voxels_stay_within_head_plus_one() {
        for &style in &HairStyle::ALL {
            for &[x, y, z] in hair_voxels(style) {
                assert!(
                    (-1..=12).contains(&x) && (-1..=12).contains(&y) && (-1..=12).contains(&z),
                    "{:?} voxel [{}, {}, {}] outside head box plus one",
                    style,
                    x,
                    y,
                    z
                );
            }
        }
    }

    #[test]
    fn appearance_colors_reach_model_materials() {
        let app = Appearance {
            hair: HairStyle::Short,
            skin: [210, 150, 110],
            hair_color: [50, 120, 240],
            eyes: [20, 180, 70],
            shirt: [230, 40, 90],
            pants: [40, 50, 60],
        };

        let dir = art_dir().join("models/player");
        if !dir.exists() {
            return;
        }

        let mut head = crate::art::vox::load(&dir.join("head.vox"));
        tint_player_model("head", &mut head, &app);

        let colors: Vec<[u8; 3]> = head.palette.iter().map(|p| [p[0], p[1], p[2]]).collect();
        assert!(colors.contains(&app.skin), "head must contain tinted skin");
        assert!(
            colors.contains(&app.hair_color),
            "head must contain tinted hair"
        );
        assert!(colors.contains(&app.eyes), "head must contain tinted eyes");
    }

    #[test]
    fn default_remote_players_still_render() {
        let app = Appearance::default();
        let dir = art_dir().join("models/player");
        if !dir.exists() {
            return;
        }

        for part in ["head", "torso", "arm_l", "arm_r", "leg_l", "leg_r"] {
            let mesh_data = build_player_part_mesh(part, &app, &dir);
            assert!(mesh_data.is_some(), "{part} failed to build mesh data");
            let m = mesh_data.unwrap();
            assert!(!m.positions.is_empty(), "{part} mesh is empty");
        }
    }

    #[test]
    fn every_hair_style_preserves_face_and_skin_shell_without_hollow_holes() {
        let dir = art_dir().join("models/player");
        if !dir.exists() {
            return;
        }
        for &style in &HairStyle::ALL {
            let app = Appearance {
                hair: style,
                skin: [215, 160, 125],
                hair_color: [60, 130, 220],
                eyes: [30, 190, 90],
                shirt: [200, 50, 50],
                pants: [40, 60, 150],
            };
            let mut head = crate::art::vox::load(&dir.join("head.vox"));
            tint_player_model("head", &mut head, &app);

            let fy = head.size[1] as usize;
            let z_shift = head.size[1].saturating_sub(12) as usize;
            let voxel_rgb = |gx: usize, gy: usize, gz_shifted: usize| -> Option<[u8; 3]> {
                let file_y = fy.checked_sub(1 + gz_shifted)?;
                head.voxels
                    .iter()
                    .find(|v| v[0] as usize == gx && v[1] as usize == file_y && v[2] as usize == gy)
                    .map(|v| {
                        let c = head.palette[v[3] as usize];
                        [c[0], c[1], c[2]]
                    })
            };

            // Face eyes (pupils at (3,6,11) and (8,6,11)) must remain eye-tinted
            assert_eq!(
                voxel_rgb(3, 6, 11 + z_shift),
                Some(app.eyes),
                "{style:?}: left pupil missing or wrong color"
            );
            assert_eq!(
                voxel_rgb(8, 6, 11 + z_shift),
                Some(app.eyes),
                "{style:?}: right pupil missing or wrong color"
            );

            // Crown (5,11,5), back (5,5,0), and sides (0,8,5) & (11,8,5) must be occupied (no hollow holes)
            for &(gx, gy, gz) in &[(5, 11, 5), (5, 5, 0), (0, 8, 5), (11, 8, 5)] {
                assert!(
                    voxel_rgb(gx, gy, gz + z_shift).is_some(),
                    "{style:?}: hollow head hole at ({gx}, {gy}, {gz})"
                );
            }

            // Bald must have skin on crown; non-Bald must have tinted hair voxels
            let has_hair = head.voxels.iter().any(|v| {
                [
                    head.palette[v[3] as usize][0],
                    head.palette[v[3] as usize][1],
                    head.palette[v[3] as usize][2],
                ] == app.hair_color
            });
            if style == HairStyle::Bald {
                assert_eq!(
                    voxel_rgb(5, 11, 5 + z_shift),
                    Some(app.skin),
                    "Bald crown must be skin"
                );
                assert!(!has_hair, "Bald must not have hair-colored voxels");
            } else {
                assert!(has_hair, "{style:?} must have hair-colored voxels");
            }

            // Ponytail must expand size[1] so z=-1 is not squashed into z=0
            if style == HairStyle::Ponytail {
                assert!(head.size[1] >= 13, "Ponytail must expand size[1] for z=-1");
                assert_eq!(
                    voxel_rgb(5, 5, 0),
                    Some(app.hair_color),
                    "Ponytail tail at gz=-1 (shifted 0) must be present"
                );
            }

            // Mesh must build cleanly with non-empty geometry
            let m = build_player_part_mesh("head", &app, &dir).expect("mesh");
            assert!(!m.positions.is_empty(), "{style:?} mesh empty");
        }
    }

    #[test]
    fn tint_handles_matching_skin_and_hair_color_without_deleting_skin() {
        let dir = art_dir().join("models/player");
        if !dir.exists() {
            return;
        }
        let same = [180, 140, 110];
        let app = Appearance {
            hair: HairStyle::Mohawk,
            skin: same,
            hair_color: same,
            eyes: [40, 80, 200],
            ..Appearance::default()
        };
        let mut head = crate::art::vox::load(&dir.join("head.vox"));
        tint_player_model("head", &mut head, &app);
        // Eyes and skin jaw must still be intact
        assert!(head.voxels.len() > 1000, "head voxels should not be wiped");
    }

    #[test]
    fn non_short_hairstyles_preserve_chamfered_head_edges() {
        let dir = art_dir().join("models/player");
        if !dir.exists() {
            return;
        }
        let app = Appearance {
            hair: HairStyle::Bald,
            ..Appearance::default()
        };
        let mut head = crate::art::vox::load(&dir.join("head.vox"));
        tint_player_model("head", &mut head, &app);

        let fy = head.size[1] as usize;
        let has_voxel = |gx: usize, gy: usize, gz: usize| -> bool {
            let file_y = fy - 1 - gz;
            head.voxels
                .iter()
                .any(|v| v[0] as usize == gx && v[1] as usize == file_y && v[2] as usize == gy)
        };
        for &(gx, gy, gz) in &[(0, 11, 5), (11, 11, 5), (0, 0, 5)] {
            assert!(
                !has_voxel(gx, gy, gz),
                "chamfered edge ({gx}, {gy}, {gz}) must remain empty for Bald"
            );
        }
    }
}
