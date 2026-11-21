//! Greedy mesher for `.vox` models: one quad per merged same-colour face rectangle.

use super::mesh_data::MeshData;
use super::vox::VoxModel;

/// Voxels per world unit.
pub const VOXELS_PER_UNIT: f32 = 16.0;

fn srgb_to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Meshes `m` in game axes (x right, y up, z front; see `tools/art/vox.py`).
/// Positions are `(voxel corner - pivot) / VOXELS_PER_UNIT`, with `pivot` in voxel units.
pub fn mesh(m: &VoxModel, pivot: [f32; 3]) -> MeshData {
    let [fx, fy, fz] = m.size.map(|v| v as usize);
    // File (x, y, z) -> game (x, z, fy - 1 - y).
    let dim = [fx, fz, fy];
    let at = |x: usize, y: usize, z: usize| (z * dim[1] + y) * dim[0] + x;
    let mut grid = vec![0u8; dim[0] * dim[1] * dim[2]];
    for &[x, y, z, i] in &m.voxels {
        let (x, y, z) = (x as usize, y as usize, z as usize);
        if x < fx && y < fy && z < fz {
            grid[at(x, z, fy - 1 - y)] = i;
        }
    }
    let cell = |p: [i32; 3]| -> u8 {
        if (0..3).all(|a| p[a] >= 0 && (p[a] as usize) < dim[a]) {
            grid[at(p[0] as usize, p[1] as usize, p[2] as usize)]
        } else {
            0
        }
    };
    let mut out = MeshData::default();
    for d in 0..3 {
        // u x v = +axis d, so u then v runs counter-clockwise seen from +d.
        let (u, v) = ((d + 1) % 3, (d + 2) % 3);
        for positive in [true, false] {
            let sign = if positive { 1 } else { -1 };
            let mut normal = [0.0; 3];
            normal[d] = sign as f32;
            for layer in 0..dim[d] {
                let idx_at = |iu: usize, iv: usize| {
                    let mut p = [0i32; 3];
                    p[d] = layer as i32;
                    p[u] = iu as i32;
                    p[v] = iv as i32;
                    let i = cell(p);
                    p[d] += sign;
                    if i != 0 && cell(p) == 0 { i } else { 0 }
                };
                let mut mask: Vec<u8> = (0..dim[u] * dim[v])
                    .map(|k| idx_at(k % dim[u], k / dim[u]))
                    .collect();
                for v0 in 0..dim[v] {
                    for u0 in 0..dim[u] {
                        let i = mask[v0 * dim[u] + u0];
                        if i == 0 {
                            continue;
                        }
                        let mut w = 1;
                        while u0 + w < dim[u] && mask[v0 * dim[u] + u0 + w] == i {
                            w += 1;
                        }
                        let mut h = 1;
                        'grow: while v0 + h < dim[v] {
                            for k in 0..w {
                                if mask[(v0 + h) * dim[u] + u0 + k] != i {
                                    break 'grow;
                                }
                            }
                            h += 1;
                        }
                        for dv in 0..h {
                            for du in 0..w {
                                mask[(v0 + dv) * dim[u] + u0 + du] = 0;
                            }
                        }
                        let plane = (layer + usize::from(positive)) as f32;
                        let corner = |iu: usize, iv: usize| {
                            let mut p = [0.0; 3];
                            p[d] = plane;
                            p[u] = iu as f32;
                            p[v] = iv as f32;
                            std::array::from_fn(|a| (p[a] - pivot[a]) / VOXELS_PER_UNIT)
                        };
                        let mut quad = [
                            corner(u0, v0),
                            corner(u0 + w, v0),
                            corner(u0 + w, v0 + h),
                            corner(u0, v0 + h),
                        ];
                        if !positive {
                            quad.reverse();
                        }
                        let c = m.palette[i as usize];
                        let col = [
                            srgb_to_linear(c[0]),
                            srgb_to_linear(c[1]),
                            srgb_to_linear(c[2]),
                            c[3] as f32 / 255.0,
                        ];
                        out.push_quad_colored(quad, normal, [[0.0; 2]; 4], [col; 4]);
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(size: [u8; 3], voxels: Vec<[u8; 4]>) -> VoxModel {
        let mut palette = [[255u8; 4]; 256];
        palette[1] = [255, 0, 0, 255];
        palette[2] = [0, 0, 255, 255];
        VoxModel {
            size,
            voxels,
            palette,
        }
    }

    #[test]
    fn voxels_per_unit_is_16() {
        assert_eq!(VOXELS_PER_UNIT, 16.0);
    }

    #[test]
    fn full_block_model_spans_one_unit() {
        let mut v = Vec::new();
        for x in 0..16 {
            for y in 0..16 {
                for z in 0..16 {
                    v.push([x, y, z, 1]);
                }
            }
        }
        let m = mesh(&model([16, 16, 16], v), [8.0; 3]);
        for a in 0..3 {
            let lo = m
                .positions
                .iter()
                .map(|p| p[a])
                .fold(f32::INFINITY, f32::min);
            let hi = m
                .positions
                .iter()
                .map(|p| p[a])
                .fold(f32::NEG_INFINITY, f32::max);
            assert!((hi - lo - 1.0).abs() < 1e-6, "axis {a}: {lo}..{hi}");
            assert!(
                (lo + 0.5).abs() < 1e-6 && (hi - 0.5).abs() < 1e-6,
                "axis {a}"
            );
        }
    }

    #[test]
    fn greedy_box_is_six_quads() {
        let mut v = Vec::new();
        for x in 0..2 {
            for y in 0..2 {
                for z in 0..2 {
                    v.push([x, y, z, 1]);
                }
            }
        }
        let m = mesh(&model([2, 2, 2], v), [0.0; 3]);
        assert_eq!(m.quad_count(), 6);
        assert_eq!(m.positions.len(), 24);
        assert_eq!(m.colors.len(), 24);
        // 2 voxels = 2 / VOXELS_PER_UNIT units.
        let max = m.positions.iter().flatten().fold(0.0f32, |a, &b| a.max(b));
        assert!((max - 2.0 / VOXELS_PER_UNIT).abs() < 1e-6);
    }

    #[test]
    fn two_colors_split_faces() {
        let m = mesh(
            &model([2, 1, 1], vec![[0, 0, 0, 1], [1, 0, 0, 2]]),
            [0.0; 3],
        );
        // 2 end caps + 4 long sides split in two each.
        assert_eq!(m.quad_count(), 10);
    }

    /// A solid game-axis box `(x, y, z)`, stored in file order `[x, depth, height]`.
    fn slab(size: [u8; 3]) -> VoxModel {
        let (sx, sy, sz) = (size[0], size[1], size[2]);
        let mut voxels = Vec::new();
        for x in 0..sx {
            for y in 0..sy {
                for z in 0..sz {
                    voxels.push([x, sz - 1 - z, y, 1]);
                }
            }
        }
        model([sx, sz, sy], voxels)
    }

    #[test]
    fn greedy_merges_flat_slab() {
        let m = mesh(&slab([32, 32, 2]), [0.0; 3]);
        assert_eq!(m.quad_count(), 6);
        assert_eq!(m.positions.len(), 24);
    }

    #[test]
    fn enemy_models_triangle_budget() {
        for kind in ["slime", "zombie", "demon_eye", "eater_of_souls"] {
            let dir = crate::art::art_dir().join("models").join(kind);
            let r = crate::art::rig::try_load(&dir).unwrap_or_else(|e| panic!("{e}"));
            for p in &r.parts {
                let m =
                    crate::art::vox::try_load(&dir.join(&p.file)).unwrap_or_else(|e| panic!("{e}"));
                let tris = mesh(&m, p.pivot).quad_count() * 2;
                assert!(tris <= 6000, "{kind}/{}: {tris} triangles", p.file);
            }
        }
    }

    #[test]
    fn winding_matches_normals() {
        let m = mesh(&model([1, 1, 1], vec![[0, 0, 0, 1]]), [0.0; 3]);
        for q in 0..m.quad_count() {
            let p: Vec<[f32; 3]> = (0..3).map(|i| m.positions[q * 4 + i]).collect();
            let d = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
            let (a, b) = (d(p[1], p[0]), d(p[2], p[0]));
            let c = [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ];
            let n = m.normals[q * 4];
            assert!(c[0] * n[0] + c[1] * n[1] + c[2] * n[2] > 0.0, "quad {q}");
        }
    }
}
