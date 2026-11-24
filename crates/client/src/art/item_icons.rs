//! Hybrid 32x32 item icon renderer: isometric block cubes from `BlockArt` and
//! 3/4 shaded `.vox` projections with distinct variant details for tint-shared items.

use std::collections::HashMap;
use std::path::Path;

use terravibe_assets::RgbaImage;
use terravibe_sim::item::{
    ABEEMINATION, AGLET, AMAZON, ANKLET_OF_THE_WIND, BEE_BREASTPLATE, BEE_GREAVES, BEE_GUN,
    BEE_HEADGEAR, BEE_KEEPER, BEE_WAX, BEENADE, BLADE_OF_GRASS, BLUE_PHASEBLADE,
    CLOUD_IN_A_BALLOON, COPPER_BAR, COPPER_CHAINMAIL, COPPER_COIN, COPPER_GREAVES, COPPER_HELMET,
    DARK_LANCE, DEMON_BOW, EMPTY_BUCKET, FALLEN_STAR, FERAL_CLAWS, FLAMELASH, FLAMING_ARROW,
    FLOWER_OF_FIRE, GLOWING_MUSHROOM, GOBLIN_BATTLE_STANDARD, GOLD_BAR, GOLD_CHAINMAIL, GOLD_COIN,
    GOLD_GREAVES, GOLD_HELMET, GUIDE_VOODOO_DOLL, HARPOON, HEALING_POTION, HEART, HONEY_COMB, HOOK,
    IRON_BAR, IRON_CHAIN, IRON_CHAINMAIL, IRON_GREAVES, IRON_HELMET, IRONSKIN_POTION, ItemId,
    JUNGLE_HAT, JUNGLE_PANTS, JUNGLE_SHIRT, JUNGLE_SPORES, LAVA_BUCKET, LENS,
    LESSER_HEALING_POTION, LESSER_MANA_POTION, LIGHTNING_BOOTS, MANA_CRYSTAL, MANA_POTION,
    METEOR_HELMET, METEOR_LEGGINGS, METEOR_SUIT, METEORITE_BAR, MONEY_TROUGH, MUSHROOM,
    NECRO_BREASTPLATE, NECRO_GREAVES, NECRO_HELMET, NIGHTS_EDGE, NINJA_HOOD, NINJA_PANTS,
    NINJA_SHIRT, OBSIDIAN_SHIELD, OBSIDIAN_SKIN_POTION, PLATINUM_COIN, RECALL_POTION,
    REGENERATION_POTION, ROCKET_BOOTS, SHARK_TOOTH_NECKLACE, SHINE_POTION, SILVER_BAR,
    SILVER_CHAINMAIL, SILVER_COIN, SILVER_GREAVES, SILVER_HELMET, SLIME_CROWN, SLIME_HOOK,
    SLIMY_SADDLE, SPACE_GUN, SPECTRE_BOOTS, SPIKY_BALL, STAR_CANNON, STINGER, SUNFURY,
    SUSPICIOUS_LOOKING_EYE, SWIFTNESS_POTION, THORN_CHAKRAM, VINE, WAND_OF_SPARKING, WATER_BOLT,
    WATER_BUCKET, WATER_WALKING_POTION, WOOD_WALL, WORM_FOOD, all_item_ids,
};

use super::blocks::{BlockArt, TILE, WALL_ID};
use super::mesh_data::Face;
use super::vox::{self, VoxModel};
use crate::view::avatar::{HeldVisualKind, held_visual_for_item};
use crate::view::block_models;

const SIZE: u32 = 32;
const OUTLINE_COLOR: [u8; 4] = [12, 14, 20, 235];

/// Renders crisp 32x32 RGBA8 icons for all 225 registered items from `BlockArt`
/// and `assets/models/{items,blocks}/*.vox`.
pub fn render_all_item_icons(block_art: &BlockArt, models_dir: &Path) -> HashMap<u16, RgbaImage> {
    let mut item_vox_cache: HashMap<&'static str, Option<VoxModel>> = HashMap::new();
    let mut block_vox_cache: HashMap<&'static str, Option<VoxModel>> = HashMap::new();
    let mut out = HashMap::new();

    for id in all_item_ids() {
        let mut img = if id == WOOD_WALL {
            let layer = block_art
                .layer
                .get(&(WALL_ID, Face::PosX, 0))
                .copied()
                .unwrap_or(0);
            render_isometric_wall(&block_art.tiles[layer as usize])
        } else {
            match held_visual_for_item(id) {
                HeldVisualKind::Block(tile) => {
                    if block_art.has(tile) {
                        let top = &block_art.tiles
                            [block_art.face_layer(tile, Face::PosY, 0, 0, 0) as usize];
                        let left = &block_art.tiles
                            [block_art.face_layer(tile, Face::PosX, 0, 0, 0) as usize];
                        let right = &block_art.tiles
                            [block_art.face_layer(tile, Face::PosZ, 0, 0, 0) as usize];
                        render_isometric_cube(top, left, right)
                    } else if let Some(file) = block_models::model_file(tile) {
                        let model = block_vox_cache.entry(file).or_insert_with(|| {
                            vox::try_load(&models_dir.join("blocks").join(file)).ok()
                        });
                        model
                            .as_ref()
                            .map(|m| render_vox_icon(m, [1.0, 1.0, 1.0, 1.0]))
                            .unwrap_or_else(fallback_square)
                    } else {
                        fallback_square()
                    }
                }
                HeldVisualKind::Vox { file, tint } => {
                    let model = item_vox_cache.entry(file).or_insert_with(|| {
                        vox::try_load(&models_dir.join("items").join(file)).ok()
                    });
                    model
                        .as_ref()
                        .map(|m| render_vox_icon(m, tint))
                        .unwrap_or_else(fallback_square)
                }
            }
        };

        apply_variant_details(id, &mut img);
        add_outline(&mut img);
        out.insert(id.0, img);
    }

    out
}

fn set_px(img: &mut RgbaImage, x: i32, y: i32, color: [u8; 4]) {
    if x >= 0 && y >= 0 && (x as u32) < img.width && (y as u32) < img.height {
        img.pixels[y as usize * img.width as usize + x as usize] = color;
    }
}

fn blend_px(img: &mut RgbaImage, x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 || (x as u32) >= img.width || (y as u32) >= img.height {
        return;
    }
    let idx = y as usize * img.width as usize + x as usize;
    let dst = img.pixels[idx];
    let a = color[3] as f32 / 255.0;
    let inv = 1.0 - a;
    img.pixels[idx] = [
        (color[0] as f32 * a + dst[0] as f32 * inv).round() as u8,
        (color[1] as f32 * a + dst[1] as f32 * inv).round() as u8,
        (color[2] as f32 * a + dst[2] as f32 * inv).round() as u8,
        dst[3].max(color[3]),
    ];
}

fn shade_rgba(p: [u8; 4], factor: f32) -> [u8; 4] {
    [
        (p[0] as f32 * factor).clamp(0.0, 255.0).round() as u8,
        (p[1] as f32 * factor).clamp(0.0, 255.0).round() as u8,
        (p[2] as f32 * factor).clamp(0.0, 255.0).round() as u8,
        p[3],
    ]
}

fn sample_tile(tile: &[[u8; 4]], u: f32, v: f32) -> [u8; 4] {
    let tx = (u * TILE as f32).floor().clamp(0.0, (TILE - 1) as f32) as usize;
    let ty = (v * TILE as f32).floor().clamp(0.0, (TILE - 1) as f32) as usize;
    tile[ty * TILE as usize + tx]
}

fn render_isometric_cube(top: &[[u8; 4]], left: &[[u8; 4]], right: &[[u8; 4]]) -> RgbaImage {
    let mut img = RgbaImage::new(SIZE, SIZE);
    for py in 0..SIZE {
        for px in 0..SIZE {
            let x = px as f32 + 0.5;
            let y = py as f32 + 0.5;

            // Top face: T(16,4) -> RT(27,9.5) [u], T(16,4) -> LT(5,9.5) [v]
            let dx_t = x - 16.0;
            let dy_t = y - 4.0;
            let u_top = (dx_t / 11.0 + dy_t / 5.5) * 0.5;
            let v_top = (dy_t / 5.5 - dx_t / 11.0) * 0.5;
            if (0.0..1.0).contains(&u_top) && (0.0..1.0).contains(&v_top) {
                let p = sample_tile(top, u_top, v_top);
                if p[3] > 0 {
                    set_px(&mut img, px as i32, py as i32, shade_rgba(p, 1.0));
                    continue;
                }
            }

            // Left face: LT(5,9.5) -> C(16,15) [u], LT -> LB(5,21.5) [v]
            let u_left = (x - 5.0) / 11.0;
            let v_left = (y - 9.5 - u_left * 5.5) / 12.0;
            if (0.0..1.0).contains(&u_left) && (0.0..1.0).contains(&v_left) {
                let p = sample_tile(left, u_left, v_left);
                if p[3] > 0 {
                    set_px(&mut img, px as i32, py as i32, shade_rgba(p, 0.82));
                    continue;
                }
            }

            // Right face: C(16,15) -> RT(27,9.5) [u], C -> B(16,27) [v]
            let u_right = (x - 16.0) / 11.0;
            let v_right = (y - 15.0 + u_right * 5.5) / 12.0;
            if (0.0..1.0).contains(&u_right) && (0.0..1.0).contains(&v_right) {
                let p = sample_tile(right, u_right, v_right);
                if p[3] > 0 {
                    set_px(&mut img, px as i32, py as i32, shade_rgba(p, 0.66));
                }
            }
        }
    }
    img
}

fn render_isometric_wall(tex: &[[u8; 4]]) -> RgbaImage {
    let mut img = RgbaImage::new(SIZE, SIZE);
    for py in 0..SIZE {
        for px in 0..SIZE {
            let x = px as f32 + 0.5;
            let y = py as f32 + 0.5;

            // Thin isometric wall slab: front face + top rim + right edge
            let u_front = (x - 6.0) / 16.0;
            let v_front = (y - 7.0 - u_front * 5.0) / 15.0;
            if (0.0..1.0).contains(&u_front) && (0.0..1.0).contains(&v_front) {
                let p = sample_tile(tex, u_front, v_front);
                if p[3] > 0 {
                    set_px(&mut img, px as i32, py as i32, shade_rgba(p, 0.90));
                    continue;
                }
            }

            // Right edge (3px thick)
            let u_edge = (x - 22.0) / 4.0;
            let v_edge = (y - 12.0 + u_edge * 2.0) / 15.0;
            if (0.0..1.0).contains(&u_edge) && (0.0..1.0).contains(&v_edge) {
                let p = sample_tile(tex, 0.85, v_edge);
                if p[3] > 0 {
                    set_px(&mut img, px as i32, py as i32, shade_rgba(p, 0.64));
                    continue;
                }
            }

            // Top rim
            let dx = x - 10.0;
            let dy = y - 5.0;
            let u_top = dx / 16.0;
            let v_top = (dy - u_top * 5.0 + (x - 6.0) * 0.0) / 2.0;
            if (0.0..1.0).contains(&u_top) && (0.0..1.0).contains(&v_top) && x >= 6.0 && x <= 26.0 {
                let p = sample_tile(tex, u_top, 0.05);
                if p[3] > 0 {
                    set_px(&mut img, px as i32, py as i32, shade_rgba(p, 1.0));
                }
            }
        }
    }
    img
}

fn render_vox_icon(model: &VoxModel, tint: [f32; 4]) -> RgbaImage {
    let mut img = RgbaImage::new(SIZE, SIZE);
    if model.voxels.is_empty() {
        return fallback_square();
    }

    let sx = model.size[0].max(1) as usize;
    let sy = model.size[1].max(1) as usize;
    let sz = model.size[2].max(1) as usize;
    let mut occ = vec![false; sx * sy * sz];
    for &[vx, vy, vz, cidx] in &model.voxels {
        if model.palette[cidx as usize][3] > 0
            && (vx as usize) < sx
            && (vy as usize) < sy
            && (vz as usize) < sz
        {
            occ[(vz as usize * sy + vy as usize) * sx + vx as usize] = true;
        }
    }
    let is_occ = |x: i32, y: i32, z: i32| -> bool {
        if x < 0 || y < 0 || z < 0 || x >= sx as i32 || y >= sy as i32 || z >= sz as i32 {
            return false;
        }
        occ[(z as usize * sy + y as usize) * sx + x as usize]
    };

    // 3/4 isometric/angled projection
    let yaw: f32 = 0.58;
    let pitch: f32 = 0.40;
    let (cy, sy_ang) = (yaw.cos(), yaw.sin());
    let (cp, sp) = (pitch.cos(), pitch.sin());

    let project_raw = |x: f32, y: f32, z: f32| -> (f32, f32, f32) {
        let rx = x * cy - y * sy_ang;
        let ry = x * sy_ang + y * cy;
        let sx_p = rx;
        let sy_p = -(z * cp - ry * sp);
        let depth = ry * cp + z * sp;
        (sx_p, sy_p, depth)
    };

    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for &[vx, vy, vz, cidx] in &model.voxels {
        if model.palette[cidx as usize][3] == 0 {
            continue;
        }
        let (px, py, _) = project_raw(vx as f32, vy as f32, vz as f32);
        min_x = min_x.min(px);
        max_x = max_x.max(px);
        min_y = min_y.min(py);
        max_y = max_y.max(py);
    }
    if !min_x.is_finite() {
        return fallback_square();
    }

    let span = (max_x - min_x).max(max_y - min_y).max(1.0);
    let scale = (23.0 / span).min(3.2);
    let mid_x = (min_x + max_x) * 0.5;
    let mid_y = (min_y + max_y) * 0.5;
    let radius = (scale * 0.62).ceil().max(1.0) as i32;

    let mut z_buf = [f32::NEG_INFINITY; (SIZE * SIZE) as usize];
    for &[vx, vy, vz, cidx] in &model.voxels {
        let base = model.palette[cidx as usize];
        if base[3] == 0 {
            continue;
        }
        let (rx, ry, depth) = project_raw(vx as f32, vy as f32, vz as f32);
        let cx = 16.0 + (rx - mid_x) * scale;
        let cy_scr = 16.0 + (ry - mid_y) * scale;

        let mut shade: f32 = 0.74;
        if !is_occ(vx as i32, vy as i32, vz as i32 + 1) {
            shade += 0.20;
        }
        if !is_occ(vx as i32, vy as i32 - 1, vz as i32) {
            shade += 0.08;
        }
        if !is_occ(vx as i32 + 1, vy as i32, vz as i32) {
            shade += 0.05;
        }
        let shade = shade.clamp(0.55, 1.06);

        let col = [
            (base[0] as f32 * tint[0] * shade).clamp(0.0, 255.0).round() as u8,
            (base[1] as f32 * tint[1] * shade).clamp(0.0, 255.0).round() as u8,
            (base[2] as f32 * tint[2] * shade).clamp(0.0, 255.0).round() as u8,
            (base[3] as f32 * tint[3]).clamp(0.0, 255.0).round() as u8,
        ];

        let ix0 = (cx.round() as i32 - radius + 1).max(1);
        let ix1 = (cx.round() as i32 + radius - 1).min(SIZE as i32 - 2);
        let iy0 = (cy_scr.round() as i32 - radius + 1).max(1);
        let iy1 = (cy_scr.round() as i32 + radius - 1).min(SIZE as i32 - 2);
        for py in iy0..=iy1 {
            for px in ix0..=ix1 {
                let idx = py as usize * SIZE as usize + px as usize;
                if depth > z_buf[idx] {
                    z_buf[idx] = depth;
                    img.pixels[idx] = col;
                }
            }
        }
    }
    img
}

fn fallback_square() -> RgbaImage {
    let mut img = RgbaImage::new(SIZE, SIZE);
    for y in 8..24 {
        for x in 8..24 {
            set_px(&mut img, x, y, [140, 148, 168, 255]);
        }
    }
    img
}

fn add_outline(img: &mut RgbaImage) {
    let orig = img.pixels.clone();
    let w = img.width as i32;
    let h = img.height as i32;
    let is_opaque = |x: i32, y: i32| -> bool {
        x >= 0 && y >= 0 && x < w && y < h && orig[(y * w + x) as usize][3] > 32
    };
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            if orig[idx][3] == 0
                && (is_opaque(x - 1, y)
                    || is_opaque(x + 1, y)
                    || is_opaque(x, y - 1)
                    || is_opaque(x, y + 1))
            {
                img.pixels[idx] = OUTLINE_COLOR;
            }
        }
    }
}

fn draw_coin(img: &mut RgbaImage, rim: [u8; 4], fill: [u8; 4], shine: [u8; 4]) {
    img.pixels.fill([0; 4]);
    let cx = 15.5_f32;
    let cy = 15.5_f32;
    for y in 6..26 {
        for x in 6..26 {
            let dx = (x as f32 - cx) / 7.5;
            let dy = (y as f32 - cy) / 9.0;
            let d = (dx * dx + dy * dy).sqrt();
            if d <= 1.0 {
                let col = if d > 0.76 {
                    rim
                } else if (x as f32 - 13.0).abs() < 1.5 && (y >= 10 && y <= 21) {
                    shine
                } else {
                    fill
                };
                set_px(img, x, y, col);
            }
        }
    }
}

fn draw_plus(img: &mut RgbaImage, cx: i32, cy: i32, color: [u8; 4]) {
    for d in -2..=2 {
        set_px(img, cx + d, cy, color);
        set_px(img, cx, cy + d, color);
    }
}

fn draw_dot(img: &mut RgbaImage, cx: i32, cy: i32, color: [u8; 4]) {
    for dy in -1_i32..=1 {
        for dx in -1_i32..=1 {
            if dx.abs() + dy.abs() <= 1 {
                set_px(img, cx + dx, cy + dy, color);
            }
        }
    }
}

/// Applies item-specific pixel details/emblems so items sharing a `.vox` model
/// and/or tint remain visually distinct at 32x32.
fn apply_variant_details(id: ItemId, img: &mut RgbaImage) {
    match id {
        // Coins: distinct circular coin discs instead of bar ingots
        COPPER_COIN => draw_coin(
            img,
            [168, 86, 40, 255],
            [224, 126, 68, 255],
            [252, 188, 128, 255],
        ),
        SILVER_COIN => draw_coin(
            img,
            [140, 150, 166, 255],
            [208, 216, 228, 255],
            [248, 252, 255, 255],
        ),
        GOLD_COIN => draw_coin(
            img,
            [188, 136, 20, 255],
            [248, 204, 56, 255],
            [255, 245, 168, 255],
        ),
        PLATINUM_COIN => draw_coin(
            img,
            [148, 174, 204, 255],
            [220, 236, 252, 255],
            [255, 255, 255, 255],
        ),

        // Bars: metallic top specular band
        COPPER_BAR => {
            for x in 11..21 {
                blend_px(img, x, 13, [255, 190, 130, 190]);
            }
        }
        IRON_BAR => {
            for x in 11..21 {
                blend_px(img, x, 13, [215, 220, 228, 190]);
            }
        }
        SILVER_BAR => {
            for x in 11..21 {
                blend_px(img, x, 13, [245, 250, 255, 200]);
            }
        }
        GOLD_BAR => {
            for x in 11..21 {
                blend_px(img, x, 13, [255, 242, 140, 210]);
            }
        }
        METEORITE_BAR => {
            for x in 11..21 {
                blend_px(img, x, 13, [255, 120, 70, 200]);
            }
            draw_dot(img, 16, 17, [255, 165, 60, 255]);
        }

        // All 10 potions + Obsidian Skin
        LESSER_HEALING_POTION => {
            draw_plus(img, 16, 17, [255, 220, 225, 255]);
        }
        HEALING_POTION => {
            draw_plus(img, 16, 17, [255, 255, 255, 255]);
            for x in 13..19 {
                set_px(img, x, 9, [245, 200, 70, 255]);
            }
        }
        LESSER_MANA_POTION => {
            draw_dot(img, 16, 17, [180, 225, 255, 255]);
        }
        MANA_POTION => {
            draw_plus(img, 16, 17, [190, 240, 255, 255]);
            for x in 13..19 {
                set_px(img, x, 9, [210, 220, 240, 255]);
            }
        }
        REGENERATION_POTION => {
            for x in 12..21 {
                set_px(img, x, 16, [255, 170, 220, 255]);
            }
            draw_dot(img, 16, 18, [255, 230, 245, 255]);
        }
        SWIFTNESS_POTION => {
            set_px(img, 14, 18, [190, 255, 210, 255]);
            set_px(img, 15, 17, [190, 255, 210, 255]);
            set_px(img, 16, 16, [190, 255, 210, 255]);
            set_px(img, 17, 17, [190, 255, 210, 255]);
            set_px(img, 18, 18, [190, 255, 210, 255]);
        }
        IRONSKIN_POTION => {
            for y in 15..20 {
                for x in 14..=18 {
                    set_px(img, x, y, [210, 215, 225, 255]);
                }
            }
        }
        SHINE_POTION => {
            draw_plus(img, 16, 16, [255, 255, 190, 255]);
            set_px(img, 14, 14, [255, 255, 220, 255]);
            set_px(img, 18, 14, [255, 255, 220, 255]);
        }
        WATER_WALKING_POTION => {
            for x in 11..21 {
                let wy = if x % 2 == 0 { 16 } else { 17 };
                set_px(img, x, wy, [180, 248, 255, 255]);
            }
        }
        RECALL_POTION => {
            set_px(img, 16, 15, [220, 255, 255, 255]);
            set_px(img, 17, 16, [220, 255, 255, 255]);
            set_px(img, 16, 18, [220, 255, 255, 255]);
            set_px(img, 14, 17, [220, 255, 255, 255]);
        }
        OBSIDIAN_SKIN_POTION => {
            for x in 12..20 {
                set_px(img, x, 16, [195, 130, 255, 255]);
            }
        }

        // Armor variants sharing 100/101/102.vox
        METEOR_HELMET | METEOR_SUIT | METEOR_LEGGINGS => {
            for x in 13..19 {
                set_px(img, x, 14, [255, 140, 50, 255]);
            }
            draw_dot(img, 16, 17, [255, 210, 90, 255]);
        }
        BEE_HEADGEAR | BEE_BREASTPLATE | BEE_GREAVES => {
            for y in [12, 16, 20] {
                for x in 11..21 {
                    set_px(img, x, y, [250, 210, 40, 255]);
                    set_px(img, x, y + 1, [28, 24, 18, 255]);
                }
            }
        }
        NINJA_HOOD | NINJA_SHIRT | NINJA_PANTS => {
            for x in 11..21 {
                set_px(img, x, 13, [215, 45, 55, 255]);
            }
        }
        JUNGLE_HAT | JUNGLE_SHIRT | JUNGLE_PANTS => {
            draw_dot(img, 16, 13, [145, 245, 95, 255]);
            set_px(img, 14, 12, [110, 220, 75, 255]);
        }
        NECRO_HELMET | NECRO_BREASTPLATE | NECRO_GREAVES => {
            draw_dot(img, 16, 15, [245, 240, 232, 255]);
            set_px(img, 14, 18, [230, 225, 215, 255]);
            set_px(img, 18, 18, [230, 225, 215, 255]);
        }
        COPPER_HELMET | COPPER_CHAINMAIL | COPPER_GREAVES => {
            set_px(img, 16, 12, [255, 195, 130, 255]);
        }
        IRON_HELMET | IRON_CHAINMAIL | IRON_GREAVES => {
            set_px(img, 16, 12, [225, 230, 238, 255]);
        }
        SILVER_HELMET | SILVER_CHAINMAIL | SILVER_GREAVES => {
            set_px(img, 16, 12, [248, 252, 255, 255]);
        }
        GOLD_HELMET | GOLD_CHAINMAIL | GOLD_GREAVES => {
            set_px(img, 16, 12, [255, 242, 120, 255]);
        }

        // Iron Chain vs Hook (both 57.vox with same tint)
        IRON_CHAIN => {
            img.pixels.fill([0; 4]);
            for &(x, y) in &[
                (11, 20),
                (12, 19),
                (13, 18),
                (14, 17),
                (15, 16),
                (16, 15),
                (17, 14),
                (18, 13),
                (19, 12),
                (20, 11),
                (12, 20),
                (14, 18),
                (17, 15),
                (19, 13),
            ] {
                set_px(img, x, y, [185, 195, 210, 255]);
            }
            for &(x, y) in &[(13, 19), (16, 16), (18, 14)] {
                set_px(img, x, y, [120, 128, 142, 255]);
            }
        }
        HOOK | SLIME_HOOK => {
            let col = if id == SLIME_HOOK {
                [90, 210, 255, 255]
            } else {
                [210, 218, 232, 255]
            };
            for &(x, y) in &[
                (16, 8),
                (17, 9),
                (18, 11),
                (18, 13),
                (17, 15),
                (15, 16),
                (13, 15),
                (12, 13),
                (15, 9),
                (19, 10),
            ] {
                set_px(img, x, y, col);
                set_px(img, x + 1, y, col);
            }
        }

        // Aglet vs Anklet of the Wind (both 49.vox with identical tint)
        AGLET => {
            for y in 12..20 {
                set_px(img, 15, y, [245, 215, 80, 255]);
                set_px(img, 16, y, [220, 175, 50, 255]);
            }
        }
        ANKLET_OF_THE_WIND => {
            for &(x, y) in &[(11, 13), (12, 12), (14, 12), (19, 14), (20, 15), (18, 16)] {
                set_px(img, x, y, [160, 255, 220, 255]);
            }
            draw_dot(img, 16, 15, [110, 245, 255, 255]);
        }

        // Abeemination vs Bee Wax
        ABEEMINATION => {
            set_px(img, 13, 13, [25, 20, 18, 255]);
            set_px(img, 19, 13, [25, 20, 18, 255]);
            for x in 12..20 {
                set_px(img, x, 17, [35, 28, 20, 255]);
            }
            set_px(img, 10, 11, [235, 245, 255, 255]);
            set_px(img, 22, 11, [235, 245, 255, 255]);
        }
        BEE_WAX => {
            for &(x, y) in &[(14, 14), (18, 14), (16, 17), (14, 19), (18, 19)] {
                set_px(img, x, y, [255, 225, 85, 255]);
            }
        }

        // Remaining Boss Summons
        SUSPICIOUS_LOOKING_EYE => {
            draw_dot(img, 16, 15, [225, 35, 45, 255]);
            set_px(img, 16, 15, [20, 10, 15, 255]);
        }
        WORM_FOOD => {
            for x in 11..21 {
                set_px(img, x, 16, [135, 75, 195, 255]);
            }
            draw_dot(img, 15, 14, [110, 215, 85, 255]);
        }
        GUIDE_VOODOO_DOLL => {
            set_px(img, 14, 12, [20, 20, 20, 255]);
            set_px(img, 18, 12, [20, 20, 20, 255]);
            set_px(img, 16, 16, [230, 65, 65, 255]);
        }
        SLIME_CROWN => {
            for x in 12..20 {
                set_px(img, x, 11, [250, 205, 55, 255]);
            }
            set_px(img, 16, 10, [235, 45, 65, 255]);
            set_px(img, 13, 10, [75, 195, 255, 255]);
            set_px(img, 19, 10, [75, 195, 255, 255]);
        }
        GOBLIN_BATTLE_STANDARD => {
            for y in 9..22 {
                set_px(img, 13, y, [165, 115, 65, 255]);
            }
            for x in 14..21 {
                set_px(img, x, 12, [120, 225, 95, 255]);
                set_px(img, x, 15, [155, 85, 195, 255]);
            }
        }

        // Buckets
        EMPTY_BUCKET => {
            for x in 12..20 {
                set_px(img, x, 11, [50, 54, 62, 255]);
            }
        }
        WATER_BUCKET => {
            for x in 12..20 {
                set_px(img, x, 11, [70, 165, 255, 255]);
            }
            set_px(img, 19, 14, [120, 210, 255, 255]);
        }
        LAVA_BUCKET => {
            for x in 12..20 {
                set_px(img, x, 11, [255, 155, 35, 255]);
            }
            set_px(img, 16, 10, [255, 235, 95, 255]);
        }

        // Boots
        ROCKET_BOOTS => {
            draw_dot(img, 13, 21, [255, 130, 40, 255]);
            set_px(img, 13, 23, [255, 225, 80, 255]);
        }
        SPECTRE_BOOTS => {
            set_px(img, 12, 13, [210, 248, 255, 255]);
            set_px(img, 11, 12, [210, 248, 255, 255]);
            set_px(img, 12, 15, [160, 230, 255, 255]);
        }
        LIGHTNING_BOOTS => {
            set_px(img, 15, 13, [255, 245, 85, 255]);
            set_px(img, 16, 14, [255, 245, 85, 255]);
            set_px(img, 15, 15, [255, 245, 85, 255]);
            set_px(img, 16, 16, [255, 245, 85, 255]);
        }

        // Other accessories
        SHARK_TOOTH_NECKLACE => {
            set_px(img, 15, 17, [245, 245, 240, 255]);
            set_px(img, 16, 17, [245, 245, 240, 255]);
            set_px(img, 16, 18, [245, 245, 240, 255]);
        }
        HONEY_COMB => {
            draw_dot(img, 16, 16, [255, 195, 45, 255]);
        }
        OBSIDIAN_SHIELD => {
            draw_plus(img, 16, 15, [225, 185, 75, 255]);
        }
        FERAL_CLAWS => {
            for y in 11..16 {
                set_px(img, 13, y, [245, 235, 215, 255]);
                set_px(img, 16, y - 1, [245, 235, 215, 255]);
                set_px(img, 19, y, [245, 235, 215, 255]);
            }
        }
        CLOUD_IN_A_BALLOON => {
            for x in 13..20 {
                set_px(img, x, 19, [245, 250, 255, 255]);
                set_px(img, x, 20, [225, 238, 250, 255]);
            }
        }

        // 23.vox / 115.vox / 40.vox / 9.vox items
        MUSHROOM => {
            for x in 13..19 {
                set_px(img, x, 13, [235, 195, 155, 255]);
            }
        }
        GLOWING_MUSHROOM => {
            set_px(img, 14, 13, [120, 235, 255, 255]);
            set_px(img, 18, 14, [120, 235, 255, 255]);
        }
        HEART => {
            draw_dot(img, 14, 14, [255, 75, 95, 255]);
            draw_dot(img, 18, 14, [255, 75, 95, 255]);
            draw_dot(img, 16, 17, [255, 75, 95, 255]);
        }
        JUNGLE_SPORES => {
            set_px(img, 13, 13, [175, 255, 95, 255]);
            set_px(img, 19, 15, [175, 255, 95, 255]);
            set_px(img, 15, 19, [175, 255, 95, 255]);
        }
        SLIMY_SADDLE => {
            for x in 12..20 {
                set_px(img, x, 15, [250, 210, 75, 255]);
            }
        }
        MONEY_TROUGH => {
            set_px(img, 13, 11, [255, 165, 195, 255]);
            set_px(img, 19, 11, [255, 165, 195, 255]);
            for x in 14..19 {
                set_px(img, x, 13, [40, 25, 35, 255]);
            }
        }
        LENS => {
            draw_dot(img, 16, 15, [110, 185, 255, 255]);
            set_px(img, 16, 15, [20, 35, 65, 255]);
        }
        FALLEN_STAR => {
            draw_plus(img, 16, 15, [255, 250, 160, 255]);
        }
        MANA_CRYSTAL => {
            for y in 12..19 {
                set_px(img, 16, y, [145, 225, 255, 255]);
            }
        }
        FLAMING_ARROW => {
            draw_dot(img, 19, 11, [255, 195, 55, 255]);
        }
        STINGER => {
            set_px(img, 20, 10, [145, 245, 65, 255]);
        }
        VINE => {
            set_px(img, 13, 13, [85, 215, 85, 255]);
            set_px(img, 18, 17, [85, 215, 85, 255]);
        }

        // Tinted weapons
        BLADE_OF_GRASS => {
            set_px(img, 18, 12, [145, 255, 110, 255]);
        }
        NIGHTS_EDGE => {
            set_px(img, 18, 12, [195, 110, 255, 255]);
        }
        BEE_KEEPER => {
            set_px(img, 18, 12, [255, 225, 75, 255]);
        }
        DEMON_BOW => {
            set_px(img, 15, 15, [175, 105, 245, 255]);
        }
        WAND_OF_SPARKING => {
            draw_dot(img, 19, 11, [255, 185, 75, 255]);
        }
        WATER_BOLT => {
            draw_dot(img, 16, 15, [105, 205, 255, 255]);
        }
        FLOWER_OF_FIRE => {
            draw_dot(img, 19, 11, [255, 115, 45, 255]);
        }
        SPACE_GUN => {
            set_px(img, 18, 14, [130, 255, 110, 255]);
        }
        BEE_GUN => {
            set_px(img, 18, 14, [255, 215, 55, 255]);
        }
        BLUE_PHASEBLADE => {
            set_px(img, 18, 12, [130, 230, 255, 255]);
        }
        STAR_CANNON => {
            draw_plus(img, 16, 14, [255, 240, 120, 255]);
        }
        SUNFURY => {
            draw_dot(img, 18, 12, [255, 145, 45, 255]);
        }
        DARK_LANCE => {
            set_px(img, 19, 11, [165, 85, 225, 255]);
        }
        HARPOON => {
            set_px(img, 19, 11, [215, 225, 235, 255]);
        }
        FLAMELASH => {
            draw_dot(img, 19, 11, [255, 135, 45, 255]);
        }
        THORN_CHAKRAM => {
            draw_dot(img, 16, 15, [120, 245, 110, 255]);
        }
        AMAZON => {
            draw_dot(img, 16, 15, [110, 235, 125, 255]);
        }
        BEENADE => {
            set_px(img, 15, 15, [30, 24, 18, 255]);
            set_px(img, 17, 15, [255, 225, 65, 255]);
        }
        SPIKY_BALL => {
            set_px(img, 13, 13, [220, 225, 235, 255]);
            set_px(img, 19, 13, [220, 225, 235, 255]);
        }

        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::art::art_dir;
    use terravibe_sim::item::{
        ABEEMINATION, AGLET, ANKLET_OF_THE_WIND, BEE_HEADGEAR, BEE_WAX, COPPER_BAR, COPPER_COIN,
        GOLD_BAR, GOLD_COIN, HEALING_POTION, HOOK, IRON_CHAIN, LESSER_HEALING_POTION, MANA_POTION,
        METEOR_HELMET, REGENERATION_POTION, all_item_ids,
    };

    #[test]
    fn all_225_items_have_non_empty_icons() {
        let art = BlockArt::load(&art_dir());
        let map = render_all_item_icons(&art, &art_dir().join("models"));
        assert_eq!(map.len(), 225);
        for id in all_item_ids() {
            let img = map
                .get(&id.0)
                .unwrap_or_else(|| panic!("missing icon for item {}", id.0));
            assert_eq!((img.width, img.height), (32, 32));
            let opaque = img.pixels.iter().filter(|p| p[3] > 0).count();
            assert!(
                opaque > 16,
                "item {} has only {opaque} non-transparent pixels",
                id.0
            );
        }
    }

    #[test]
    fn tint_shared_potions_and_coins_are_distinct() {
        let art = BlockArt::load(&art_dir());
        let map = render_all_item_icons(&art, &art_dir().join("models"));
        let diff = |a: ItemId, b: ItemId| {
            assert_ne!(
                map[&a.0].pixels, map[&b.0].pixels,
                "items {} and {} should have distinct icons",
                a.0, b.0
            );
        };
        diff(HEALING_POTION, LESSER_HEALING_POTION);
        diff(HEALING_POTION, MANA_POTION);
        diff(HEALING_POTION, REGENERATION_POTION);
        diff(MANA_POTION, REGENERATION_POTION);
        diff(COPPER_COIN, GOLD_COIN);
        diff(COPPER_COIN, COPPER_BAR);
        diff(GOLD_COIN, GOLD_BAR);
        diff(COPPER_BAR, GOLD_BAR);
        diff(METEOR_HELMET, BEE_HEADGEAR);
        diff(IRON_CHAIN, HOOK);
        diff(AGLET, ANKLET_OF_THE_WIND);
        diff(ABEEMINATION, BEE_WAX);
    }
}
