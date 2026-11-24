//! Procedural RGBA8 textures for HUD chrome, slot frames, resource icons, and ghost silhouettes.

use terravibe_assets::RgbaImage;

type Rgba = [u8; 4];

fn set_px(img: &mut RgbaImage, x: i32, y: i32, color: Rgba) {
    if x >= 0 && y >= 0 && (x as u32) < img.width && (y as u32) < img.height {
        img.pixels[y as usize * img.width as usize + x as usize] = color;
    }
}

fn blend_px(img: &mut RgbaImage, x: i32, y: i32, src: Rgba) {
    if x < 0 || y < 0 || (x as u32) >= img.width || (y as u32) >= img.height || src[3] == 0 {
        return;
    }
    let idx = y as usize * img.width as usize + x as usize;
    let dst = img.pixels[idx];
    let sa = src[3] as f32 / 255.0;
    let da = dst[3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 0.0 {
        return;
    }
    let mix = |s: u8, d: u8| -> u8 {
        let v = (s as f32 * sa + d as f32 * da * (1.0 - sa)) / out_a;
        v.round().clamp(0.0, 255.0) as u8
    };
    img.pixels[idx] = [
        mix(src[0], dst[0]),
        mix(src[1], dst[1]),
        mix(src[2], dst[2]),
        (out_a * 255.0).round().clamp(0.0, 255.0) as u8,
    ];
}

fn fill_rect(img: &mut RgbaImage, x0: i32, y0: i32, w: i32, h: i32, color: Rgba) {
    for y in y0..(y0 + h) {
        for x in x0..(x0 + w) {
            set_px(img, x, y, color);
        }
    }
}

fn draw_line(img: &mut RgbaImage, mut x0: i32, mut y0: i32, x1: i32, y1: i32, color: Rgba) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        set_px(img, x0, y0, color);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

/// Signed distance to a rounded rectangle centered at `(cx, cy)` with half-extents `(hx, hy)`
/// and corner radius `r`. Negative inside, ~0 on the boundary, positive outside.
fn rounded_rect_sdf(x: f32, y: f32, cx: f32, cy: f32, hx: f32, hy: f32, r: f32) -> f32 {
    let dx = (x - cx).abs() - (hx - r);
    let dy = (y - cy).abs() - (hy - r);
    let out_d = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt();
    let in_d = dx.max(dy).min(0.0);
    out_d + in_d - r
}

/// 48x48 obsidian-slate recessed slot well with 4px rounded corners, 1px hairline border,
/// subtle inner bevel, and bronze/gold border + corner/top accent when selected.
pub fn slot_frame(selected: bool) -> RgbaImage {
    let size = 48u32;
    let mut img = RgbaImage::new(size, size);
    let cx = 23.5f32;
    let cy = 23.5f32;
    let half = 23.0f32;
    let radius = 4.0f32;

    let fill: Rgba = if selected {
        [19, 25, 42, 236]
    } else {
        [10, 15, 26, 224]
    };
    let border: Rgba = if selected {
        [219, 179, 87, 235]
    } else {
        [92, 110, 148, 125]
    };

    for y in 0..size {
        for x in 0..size {
            let d = rounded_rect_sdf(x as f32, y as f32, cx, cy, half, half, radius);
            if d > 0.6 {
                continue;
            }
            if d >= -0.9 {
                let aa = (1.0 - (d + 0.2).max(0.0) / 0.8).clamp(0.0, 1.0);
                let mut c = border;
                c[3] = (c[3] as f32 * aa).round() as u8;
                set_px(&mut img, x as i32, y as i32, c);
            } else {
                let mut c = fill;
                // Subtle inner bevel: recessed top shadow, faint bottom catchlight.
                if y == 2 && x >= 4 && x <= 43 {
                    c = if selected {
                        [245, 205, 110, 180]
                    } else {
                        [5, 8, 15, 235]
                    };
                } else if y == 3 && selected && x >= 5 && x <= 42 {
                    c = [140, 112, 54, 95];
                } else if y == 45 && x >= 4 && x <= 43 {
                    c = if selected {
                        [180, 145, 70, 130]
                    } else {
                        [34, 44, 68, 170]
                    };
                } else if (x == 2 || x == 45) && y >= 4 && y <= 43 {
                    c = if selected {
                        [185, 150, 75, 110]
                    } else {
                        [18, 24, 38, 210]
                    };
                }
                set_px(&mut img, x as i32, y as i32, c);
            }
        }
    }

    if selected {
        // Subtle corner accent pips inside the 4px rounded corners.
        let pip: Rgba = [248, 214, 120, 230];
        for (px, py) in [(3, 3), (44, 3), (3, 44), (44, 44)] {
            set_px(&mut img, px, py, pip);
        }
    }

    img
}

/// Draw a filled shape with a 1px dark outline from a 24x24 signed/mask predicate.
fn render_24x24_outlined(
    inside: impl Fn(f32, f32) -> bool,
    shade: impl Fn(f32, f32) -> Rgba,
    outline: Rgba,
) -> RgbaImage {
    let mut img = RgbaImage::new(24, 24);
    let mut mask = [[false; 24]; 24];
    for y in 0..24 {
        for x in 0..24 {
            mask[y][x] = inside(x as f32 + 0.5, y as f32 + 0.5);
        }
    }
    for y in 0..24 {
        for x in 0..24 {
            if mask[y][x] {
                set_px(
                    &mut img,
                    x as i32,
                    y as i32,
                    shade(x as f32 + 0.5, y as f32 + 0.5),
                );
            } else {
                let adj = [(-1i32, 0), (1, 0), (0, -1), (0, 1)]
                    .iter()
                    .any(|&(dx, dy)| {
                        let nx = x as i32 + dx;
                        let ny = y as i32 + dy;
                        nx >= 0 && ny >= 0 && nx < 24 && ny < 24 && mask[ny as usize][nx as usize]
                    });
                if adj {
                    set_px(&mut img, x as i32, y as i32, outline);
                }
            }
        }
    }
    img
}

/// 24x24 crisp crimson-ruby heart icon.
pub fn heart_icon() -> RgbaImage {
    render_24x24_outlined(
        |x, y| {
            let nx = (x - 12.0) / 8.5;
            let ny = (11.5 - y) / 8.5;
            let v = nx * nx + ny * ny - 0.85;
            // Classic algebraic heart curve: (x^2 + y^2 - 0.85)^3 - x^2 * y^3 <= 0
            v * v * v - nx * nx * ny * ny * ny <= 0.0
        },
        |x, y| {
            let dx = x - 8.5;
            let dy = y - 7.5;
            if dx * dx + dy * dy < 6.0 {
                [255, 165, 175, 255]
            } else if x + y > 26.0 {
                [168, 24, 42, 255]
            } else {
                [224, 44, 66, 255]
            }
        },
        [36, 10, 18, 255],
    )
}

/// 24x24 crisp 5-pointed sapphire mana star icon.
pub fn mana_icon() -> RgbaImage {
    let mut verts = [(0.0f32, 0.0f32); 10];
    for (i, v) in verts.iter_mut().enumerate() {
        let angle = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
        let r = if i % 2 == 0 { 9.5 } else { 4.2 };
        *v = (12.0 + r * angle.cos(), 12.4 + r * angle.sin());
    }
    let point_in_star = |px: f32, py: f32| -> bool {
        let mut inside = false;
        let mut j = verts.len() - 1;
        for i in 0..verts.len() {
            let (xi, yi) = verts[i];
            let (xj, yj) = verts[j];
            if ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi) + xi) {
                inside = !inside;
            }
            j = i;
        }
        inside
    };
    render_24x24_outlined(
        point_in_star,
        |x, y| {
            if x < 11.5 && y < 11.5 {
                [150, 225, 255, 255]
            } else if x + y > 26.0 {
                [34, 92, 206, 255]
            } else {
                [56, 146, 250, 255]
            }
        },
        [14, 28, 72, 255],
    )
}

/// 24x24 crisp heater shield defense icon.
pub fn defense_shield_icon() -> RgbaImage {
    render_24x24_outlined(
        |x, y| {
            if !(4.0..=20.0).contains(&y) {
                return false;
            }
            let half_w = if y < 12.0 {
                7.2
            } else {
                let t = (y - 12.0) / 8.0;
                7.2 * (1.0 - t * t)
            };
            (x - 12.0).abs() <= half_w
        },
        |x, y| {
            if y < 6.2 {
                [219, 179, 87, 255]
            } else if x < 12.0 {
                [198, 212, 234, 255]
            } else {
                [112, 128, 158, 255]
            }
        },
        [20, 26, 40, 255],
    )
}

/// 24x24 crisp potion sickness debuff icon (flask + diagonal warning slash).
pub fn potion_sickness_icon() -> RgbaImage {
    let mut img = render_24x24_outlined(
        |x, y| {
            let cork = (10.0..=14.0).contains(&x) && (3.5..=5.5).contains(&y);
            let neck = (9.5..=14.5).contains(&x) && (5.5..=9.0).contains(&y);
            let dx = x - 12.0;
            let dy = y - 14.5;
            let bulb = dx * dx + dy * dy <= 34.0 && y <= 20.5;
            cork || neck || bulb
        },
        |x, y| {
            if y < 5.5 {
                [188, 138, 86, 255]
            } else if y < 12.0 {
                [200, 215, 232, 255]
            } else if x < 11.0 && y < 15.5 {
                [235, 130, 245, 255]
            } else {
                [172, 50, 192, 255]
            }
        },
        [28, 14, 36, 255],
    );
    // Crisp coral-red warning slash across the vial.
    draw_line(&mut img, 5, 19, 19, 5, [245, 86, 82, 255]);
    draw_line(&mut img, 5, 18, 18, 5, [255, 150, 145, 255]);
    img
}

/// 24x24 coin slot ghost silhouette for tiers 0 (copper), 1 (silver), 2 (gold), 3 (platinum).
pub fn coin_ghost(tier: usize) -> RgbaImage {
    let mut img = RgbaImage::new(24, 24);
    let (base, rim, glint): (Rgba, Rgba, Rgba) = match tier {
        0 => (
            [188, 114, 72, 135],
            [140, 80, 46, 155],
            [228, 160, 115, 165],
        ),
        1 => (
            [178, 190, 206, 135],
            [125, 138, 156, 155],
            [225, 235, 248, 165],
        ),
        2 => (
            [224, 184, 74, 140],
            [170, 132, 42, 160],
            [250, 224, 128, 175],
        ),
        _ => (
            [202, 224, 242, 140],
            [142, 170, 194, 160],
            [242, 252, 255, 180],
        ),
    };

    for y in 0..24 {
        for x in 0..24 {
            let dx = x as f32 + 0.5 - 12.0;
            let dy = y as f32 + 0.5 - 12.0;
            let r = (dx * dx + dy * dy).sqrt();
            if r <= 7.5 {
                let c = if r >= 6.0 {
                    rim
                } else if dx + dy < -2.0 {
                    glint
                } else {
                    base
                };
                set_px(&mut img, x, y, c);
            }
        }
    }

    // Distinct tier pips inside the coin center so each tier is visually unique.
    match tier {
        0 => fill_rect(&mut img, 11, 9, 2, 6, rim),
        1 => {
            fill_rect(&mut img, 9, 9, 2, 6, rim);
            fill_rect(&mut img, 13, 9, 2, 6, rim);
        }
        2 => {
            fill_rect(&mut img, 8, 9, 2, 6, rim);
            fill_rect(&mut img, 11, 9, 2, 6, rim);
            fill_rect(&mut img, 14, 9, 2, 6, rim);
        }
        _ => {
            fill_rect(&mut img, 10, 10, 4, 4, glint);
            set_px(&mut img, 12, 8, rim);
            set_px(&mut img, 12, 15, rim);
        }
    }
    img
}

/// 24x24 ammo slot ghost silhouette (angled arrow).
pub fn ammo_ghost() -> RgbaImage {
    let mut img = RgbaImage::new(24, 24);
    let body: Rgba = [156, 170, 196, 140];
    let hi: Rgba = [200, 212, 232, 160];
    draw_line(&mut img, 6, 18, 17, 7, body);
    draw_line(&mut img, 6, 17, 16, 7, hi);
    // Arrowhead
    fill_rect(&mut img, 14, 6, 4, 2, hi);
    fill_rect(&mut img, 16, 6, 2, 4, hi);
    // Fletching
    draw_line(&mut img, 5, 15, 5, 19, body);
    draw_line(&mut img, 5, 19, 9, 19, body);
    img
}

/// 24x24 trash slot ghost silhouette (bin with lid and ribs).
pub fn trash_ghost() -> RgbaImage {
    let mut img = RgbaImage::new(24, 24);
    let col: Rgba = [180, 132, 138, 145];
    let dim: Rgba = [125, 88, 94, 150];
    // Handle & lid
    fill_rect(&mut img, 10, 5, 4, 2, col);
    fill_rect(&mut img, 6, 7, 12, 2, col);
    // Bin body
    fill_rect(&mut img, 7, 10, 10, 9, col);
    // Inner ribs
    fill_rect(&mut img, 9, 11, 1, 6, dim);
    fill_rect(&mut img, 12, 11, 1, 6, dim);
    fill_rect(&mut img, 15, 11, 1, 6, dim);
    img
}

/// 24x24 armor slot ghost silhouette: 0 = helmet, 1 = chestplate, 2 = greaves.
pub fn armor_ghost(slot: usize) -> RgbaImage {
    let mut img = RgbaImage::new(24, 24);
    let col: Rgba = [156, 170, 196, 140];
    let dim: Rgba = [100, 114, 140, 150];
    match slot {
        0 => {
            // Helmet dome + visor slit
            fill_rect(&mut img, 8, 5, 8, 3, col);
            fill_rect(&mut img, 6, 8, 12, 8, col);
            fill_rect(&mut img, 7, 16, 10, 2, col);
            fill_rect(&mut img, 8, 11, 8, 2, dim);
        }
        1 => {
            // Chestplate with shoulder pauldrons
            fill_rect(&mut img, 5, 6, 4, 5, col);
            fill_rect(&mut img, 15, 6, 4, 5, col);
            fill_rect(&mut img, 8, 7, 8, 11, col);
            fill_rect(&mut img, 10, 7, 4, 2, dim);
        }
        _ => {
            // Greaves / boots
            fill_rect(&mut img, 7, 6, 10, 3, col);
            fill_rect(&mut img, 7, 9, 4, 8, col);
            fill_rect(&mut img, 13, 9, 4, 8, col);
            fill_rect(&mut img, 6, 17, 5, 2, col);
            fill_rect(&mut img, 13, 17, 5, 2, col);
        }
    }
    img
}

/// 24x24 accessory slot ghost silhouette (ring / pendant).
pub fn accessory_ghost() -> RgbaImage {
    let mut img = RgbaImage::new(24, 24);
    let ring: Rgba = [182, 168, 124, 145];
    let gem: Rgba = [120, 195, 225, 165];
    for y in 0..24 {
        for x in 0..24 {
            let dx = x as f32 + 0.5 - 12.0;
            let dy = y as f32 + 0.5 - 13.0;
            let r = (dx * dx + dy * dy).sqrt();
            if (4.5..=6.8).contains(&r) {
                set_px(&mut img, x, y, ring);
            }
        }
    }
    fill_rect(&mut img, 10, 5, 4, 4, gem);
    img
}

/// 24x24 crafting hammer button icon.
pub fn hammer_icon() -> RgbaImage {
    let mut img = RgbaImage::new(24, 24);
    let handle: Rgba = [176, 126, 76, 255];
    let steel: Rgba = [194, 206, 224, 255];
    let edge: Rgba = [92, 104, 128, 255];
    // Diagonal wooden handle
    for i in 0..10 {
        blend_px(&mut img, 6 + i, 18 - i, handle);
        blend_px(&mut img, 7 + i, 18 - i, handle);
    }
    // Hammer head
    fill_rect(&mut img, 10, 5, 9, 4, steel);
    fill_rect(&mut img, 11, 9, 7, 1, edge);
    fill_rect(&mut img, 17, 4, 2, 6, steel);
    img
}

/// 24x24 quick-stack button icon (chest box with downward/into arrow).
pub fn quick_stack_icon() -> RgbaImage {
    let mut img = RgbaImage::new(24, 24);
    let box_col: Rgba = [198, 164, 96, 255];
    let box_dark: Rgba = [122, 92, 48, 255];
    let arrow: Rgba = [240, 238, 230, 255];
    // Tray / chest base
    fill_rect(&mut img, 5, 13, 14, 6, box_col);
    fill_rect(&mut img, 7, 15, 10, 3, box_dark);
    // Arrow pointing down into tray
    fill_rect(&mut img, 11, 4, 2, 7, arrow);
    fill_rect(&mut img, 9, 9, 6, 2, arrow);
    fill_rect(&mut img, 10, 11, 4, 1, arrow);
    img
}

/// 24x24 sort button icon (3 descending bars + down arrow).
pub fn sort_icon() -> RgbaImage {
    let mut img = RgbaImage::new(24, 24);
    let bar: Rgba = [226, 228, 234, 255];
    let accent: Rgba = [219, 179, 87, 255];
    // 3 descending horizontal bars
    fill_rect(&mut img, 4, 6, 10, 2, bar);
    fill_rect(&mut img, 4, 11, 7, 2, bar);
    fill_rect(&mut img, 4, 16, 4, 2, bar);
    // Down arrow on right
    fill_rect(&mut img, 17, 6, 2, 10, accent);
    fill_rect(&mut img, 15, 14, 6, 2, accent);
    fill_rect(&mut img, 16, 16, 4, 2, accent);
    img
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opaque_count(img: &RgbaImage) -> usize {
        img.pixels.iter().filter(|p| p[3] > 0).count()
    }

    #[test]
    fn slot_frames_have_exact_dimensions_and_differ() {
        let idle = slot_frame(false);
        let selected = slot_frame(true);
        assert_eq!((idle.width, idle.height), (48, 48));
        assert_eq!((selected.width, selected.height), (48, 48));
        assert!(opaque_count(&idle) > 0);
        assert!(opaque_count(&selected) > 0);
        assert_ne!(idle.pixels, selected.pixels);
    }

    #[test]
    fn hud_and_ghost_icons_have_24x24_dimensions_and_opaque_pixels() {
        let icons: Vec<(&str, RgbaImage)> = vec![
            ("heart_icon", heart_icon()),
            ("mana_icon", mana_icon()),
            ("defense_shield_icon", defense_shield_icon()),
            ("potion_sickness_icon", potion_sickness_icon()),
            ("coin_ghost(0)", coin_ghost(0)),
            ("coin_ghost(1)", coin_ghost(1)),
            ("coin_ghost(2)", coin_ghost(2)),
            ("coin_ghost(3)", coin_ghost(3)),
            ("ammo_ghost", ammo_ghost()),
            ("trash_ghost", trash_ghost()),
            ("armor_ghost(0)", armor_ghost(0)),
            ("armor_ghost(1)", armor_ghost(1)),
            ("armor_ghost(2)", armor_ghost(2)),
            ("accessory_ghost", accessory_ghost()),
            ("hammer_icon", hammer_icon()),
            ("quick_stack_icon", quick_stack_icon()),
            ("sort_icon", sort_icon()),
        ];

        for (name, img) in &icons {
            assert_eq!((img.width, img.height), (24, 24), "{name} must be 24x24");
            assert!(
                opaque_count(img) > 0,
                "{name} must have non-zero opaque pixels"
            );
        }
    }

    #[test]
    fn coin_tiers_and_armor_slots_are_distinct() {
        let coins: Vec<RgbaImage> = (0..4).map(coin_ghost).collect();
        for i in 0..coins.len() {
            for j in (i + 1)..coins.len() {
                assert_ne!(
                    coins[i].pixels, coins[j].pixels,
                    "coin_ghost({i}) and coin_ghost({j}) must be distinct"
                );
            }
        }

        let armor: Vec<RgbaImage> = (0..3).map(armor_ghost).collect();
        for i in 0..armor.len() {
            for j in (i + 1)..armor.len() {
                assert_ne!(
                    armor[i].pixels, armor[j].pixels,
                    "armor_ghost({i}) and armor_ghost({j}) must be distinct"
                );
            }
        }
    }
}
