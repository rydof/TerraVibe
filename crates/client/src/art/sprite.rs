//! Extrudes a 2D sprite into a voxel-ish mesh: one box per opaque pixel.

use terravibe_assets::RgbaImage;

use crate::art::mesh_data::MeshData;

/// (axis, sign, neighbour offset (dc, dr) in sprite space; `None` = always emit).
type FaceSpec = (usize, f32, Option<(i64, i64)>);

/// Pixels with alpha at or above this count as opaque.
const ALPHA_CUTOFF: u8 = 128;

/// One box per opaque pixel of `rect = (x, y, w, h)` in `img`, `thickness_px`
/// deep, each pixel `px_size` units wide. Faces shared by two opaque pixels
/// are skipped. The mesh is centred on x and z with its bottom edge at y = 0;
/// sprite row 0 is at the top. Every face samples its pixel's centre in `img`.
pub fn extrude_sprite(
    img: &RgbaImage,
    rect: (u32, u32, u32, u32),
    thickness_px: u32,
    px_size: f32,
) -> MeshData {
    let (rx, ry, rw, rh) = rect;
    let mut out = MeshData::default();
    let opaque = |c: i64, r: i64| {
        c >= 0
            && r >= 0
            && (c as u32) < rw
            && (r as u32) < rh
            && rx + (c as u32) < img.width
            && ry + (r as u32) < img.height
            && img.get(rx + c as u32, ry + r as u32)[3] >= ALPHA_CUTOFF
    };
    let s = px_size;
    let half_w = rw as f32 * s / 2.0;
    let half_d = thickness_px as f32 * s / 2.0;
    for r in 0..rh as i64 {
        for c in 0..rw as i64 {
            if !opaque(c, r) {
                continue;
            }
            let uv = [
                (rx as f32 + c as f32 + 0.5) / img.width as f32,
                (ry as f32 + r as f32 + 0.5) / img.height as f32,
            ];
            let lo = [
                c as f32 * s - half_w,
                (rh as i64 - 1 - r) as f32 * s,
                -half_d,
            ];
            let hi = [lo[0] + s, lo[1] + s, half_d];
            let faces: [FaceSpec; 6] = [
                (0, 1.0, Some((1, 0))),
                (0, -1.0, Some((-1, 0))),
                (1, 1.0, Some((0, -1))),
                (1, -1.0, Some((0, 1))),
                (2, 1.0, None),
                (2, -1.0, None),
            ];
            for (axis, sign, nb) in faces {
                if nb.is_some_and(|(dc, dr)| opaque(c + dc, r + dr)) {
                    continue;
                }
                box_face(&mut out, lo, hi, axis, sign, uv);
            }
        }
    }
    out
}

/// Emits the face of box `lo..hi` whose normal is `sign` along `axis`,
/// wound counter-clockwise seen from the normal side.
fn box_face(out: &mut MeshData, lo: [f32; 3], hi: [f32; 3], axis: usize, sign: f32, uv: [f32; 2]) {
    let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
    let plane = if sign > 0.0 { hi[axis] } else { lo[axis] };
    let corner = |ua: f32, ub: f32| {
        let mut p = [0.0; 3];
        p[axis] = plane;
        p[a] = ua;
        p[b] = ub;
        p
    };
    // (a, b, axis) is a right-handed cycle, so this order is CCW seen from +axis.
    let mut pts = [
        corner(lo[a], lo[b]),
        corner(hi[a], lo[b]),
        corner(hi[a], hi[b]),
        corner(lo[a], hi[b]),
    ];
    if sign < 0.0 {
        pts.swap(1, 3);
    }
    let mut normal = [0.0; 3];
    normal[axis] = sign;
    out.push_quad(pts, normal, [uv; 4]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extrude_two_pixels() {
        let mut img = RgbaImage::new(2, 1);
        img.pixels = vec![[255, 0, 0, 255]; 2];
        let m = extrude_sprite(&img, (0, 0, 2, 1), 1, 1.0);
        assert_eq!(m.quad_count(), 10); // two unit cubes sharing one face: 12 - 2
    }

    #[test]
    fn transparent_pixels_and_rect_are_respected() {
        let mut img = RgbaImage::new(3, 2);
        img.pixels = vec![[0, 0, 0, 0]; 6];
        img.pixels[0] = [9, 9, 9, 255]; // (0,0)
        img.pixels[5] = [9, 9, 9, 255]; // (2,1), outside the rect below
        let m = extrude_sprite(&img, (0, 0, 2, 2), 3, 0.5);
        assert_eq!(m.quad_count(), 6);
        // Row 0 is on top: the box spans y 0.5..1.0 and z -0.75..0.75.
        for p in &m.positions {
            assert!(p[1] >= 0.5 && p[1] <= 1.0 && p[2].abs() <= 0.75, "{p:?}");
        }
    }

    #[test]
    fn winding_matches_normal() {
        let mut img = RgbaImage::new(2, 2);
        img.pixels = vec![[1, 2, 3, 255]; 4];
        img.pixels[3] = [0; 4];
        let m = extrude_sprite(&img, (0, 0, 2, 2), 2, 1.0);
        for q in 0..m.quad_count() {
            let p = &m.positions[q * 4..q * 4 + 4];
            let e1: Vec<f32> = (0..3).map(|i| p[1][i] - p[0][i]).collect();
            let e2: Vec<f32> = (0..3).map(|i| p[2][i] - p[0][i]).collect();
            let c = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];
            let n = m.normals[q * 4];
            assert!(c[0] * n[0] + c[1] * n[1] + c[2] * n[2] > 0.0, "quad {q}");
        }
    }
}
