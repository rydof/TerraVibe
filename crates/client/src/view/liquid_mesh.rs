//! Mesh generation for liquid cells (water and lava).

use bevy::prelude::*;
use terravibe_sim::{CHUNK_SIZE, Chunk, ChunkPos, Liquid, LiquidKind};

use crate::art::mesh_data::MeshData;
use crate::view::to_bevy_mesh;

const UV_QUAD: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];

/// Meshes liquid cells in `chunk` at `pos`.
///
/// Returns `None` if the chunk has no liquid or if no liquid faces are exposed.
pub fn mesh_liquid(
    chunk: &Chunk,
    pos: ChunkPos,
    neighbours: impl Fn(i32, i32, i32) -> Option<Liquid>,
) -> Option<Mesh> {
    if !chunk.has_liquid() {
        return None;
    }

    let mut out = MeshData::default();
    let base_x = (pos.x * CHUNK_SIZE) as f32;
    let base_y = (pos.y * CHUNK_SIZE) as f32;
    let base_z = (pos.z * CHUNK_SIZE) as f32;

    for lz in 0..CHUNK_SIZE as usize {
        for ly in 0..CHUNK_SIZE as usize {
            for lx in 0..CHUNK_SIZE as usize {
                let Some(liq) = chunk.liquid(lx, ly, lz) else {
                    continue;
                };
                if liq.amount == 0 {
                    continue;
                }

                let color = match liq.kind {
                    LiquidKind::Water => {
                        LinearRgba::from(Color::srgba(0.18, 0.45, 0.88, 0.65)).to_f32_array()
                    }
                    LiquidKind::Lava => {
                        LinearRgba::from(Color::srgb(0.98, 0.45, 0.08)).to_f32_array()
                    }
                };

                let wx = pos.x * CHUNK_SIZE + lx as i32;
                let wy = pos.y * CHUNK_SIZE + ly as i32;
                let wz = pos.z * CHUNK_SIZE + lz as i32;

                let (above_liq, above_opaque) = if ly + 1 < CHUNK_SIZE as usize {
                    (
                        chunk.liquid(lx, ly + 1, lz),
                        chunk.get(lx, ly + 1, lz).is_opaque(),
                    )
                } else {
                    (neighbours(wx, wy + 1, wz), false)
                };

                let top = if above_liq.as_ref().map(|a| a.kind) == Some(liq.kind) {
                    1.0
                } else {
                    (liq.amount as f32 / 255.0).max(0.1)
                };

                let x = base_x + lx as f32;
                let y = base_y + ly as f32;
                let z = base_z + lz as f32;

                // Top face: generated only when cell above has no liquid and is not opaque
                if above_liq.is_none() && !above_opaque {
                    let corners = [
                        [x, y + top, z],
                        [x, y + top, z + 1.0],
                        [x + 1.0, y + top, z + 1.0],
                        [x + 1.0, y + top, z],
                    ];
                    out.push_quad_colored(corners, [0.0, 1.0, 0.0], UV_QUAD, [color; 4]);
                }

                // Side faces: generated only against non-liquid, non-opaque neighbours
                const SIDES: [(i32, i32, [f32; 3]); 4] = [
                    (1, 0, [1.0, 0.0, 0.0]),   // +X
                    (-1, 0, [-1.0, 0.0, 0.0]), // -X
                    (0, 1, [0.0, 0.0, 1.0]),   // +Z
                    (0, -1, [0.0, 0.0, -1.0]), // -Z
                ];

                for (dx, dz, norm) in SIDES {
                    let nx = lx as i32 + dx;
                    let nz = lz as i32 + dz;

                    let (n_liq, n_opaque) =
                        if (0..CHUNK_SIZE).contains(&nx) && (0..CHUNK_SIZE).contains(&nz) {
                            (
                                chunk.liquid(nx as usize, ly, nz as usize),
                                chunk.get(nx as usize, ly, nz as usize).is_opaque(),
                            )
                        } else {
                            (neighbours(wx + dx, wy, wz + dz), false)
                        };

                    if n_liq.is_none() && !n_opaque {
                        let corners = match (dx, dz) {
                            (1, 0) => [
                                [x + 1.0, y, z],
                                [x + 1.0, y + top, z],
                                [x + 1.0, y + top, z + 1.0],
                                [x + 1.0, y, z + 1.0],
                            ],
                            (-1, 0) => [
                                [x, y, z + 1.0],
                                [x, y + top, z + 1.0],
                                [x, y + top, z],
                                [x, y, z],
                            ],
                            (0, 1) => [
                                [x + 1.0, y, z + 1.0],
                                [x + 1.0, y + top, z + 1.0],
                                [x, y + top, z + 1.0],
                                [x, y, z + 1.0],
                            ],
                            (0, -1) => [
                                [x, y, z],
                                [x, y + top, z],
                                [x + 1.0, y + top, z],
                                [x + 1.0, y, z],
                            ],
                            _ => unreachable!(),
                        };
                        out.push_quad_colored(corners, norm, UV_QUAD, [color; 4]);
                    }
                }
            }
        }
    }

    to_bevy_mesh(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use terravibe_sim::{AIR, STONE};

    fn make_test_chunk() -> Chunk {
        Chunk::filled(AIR)
    }

    #[test]
    fn liquid_chunk_mesh_not_built_when_chunk_has_no_liquid() {
        let chunk = make_test_chunk();
        let mesh = mesh_liquid(&chunk, ChunkPos { x: 0, y: 0, z: 0 }, |_, _, _| None);
        assert!(mesh.is_none());
    }

    #[test]
    fn liquid_surface_height_scales_with_amount() {
        let mut c1 = make_test_chunk();
        c1.set_liquid(5, 5, 5, Some(Liquid::water(255)));
        let m1 = mesh_liquid(&c1, ChunkPos { x: 0, y: 0, z: 0 }, |_, _, _| None)
            .expect("should produce mesh");

        let mut c2 = make_test_chunk();
        c2.set_liquid(5, 5, 5, Some(Liquid::water(128)));
        let m2 = mesh_liquid(&c2, ChunkPos { x: 0, y: 0, z: 0 }, |_, _, _| None)
            .expect("should produce mesh");

        let mut c3 = make_test_chunk();
        c3.set_liquid(5, 5, 5, Some(Liquid::water(10)));
        let m3 = mesh_liquid(&c3, ChunkPos { x: 0, y: 0, z: 0 }, |_, _, _| None)
            .expect("should produce mesh");

        let max_y = |m: &Mesh| -> f32 {
            let pos = m
                .attribute(Mesh::ATTRIBUTE_POSITION)
                .unwrap()
                .as_float3()
                .unwrap();
            pos.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max)
        };

        let y1 = max_y(&m1);
        let y2 = max_y(&m2);
        let y3 = max_y(&m3);

        assert!(
            (y1 - 6.0).abs() < 1e-4,
            "amount 255 should have top at 6.0, got {y1}"
        );
        let expected_y2 = 5.0 + 128.0 / 255.0;
        assert!(
            (y2 - expected_y2).abs() < 1e-4,
            "amount 128 should have top at {expected_y2}, got {y2}"
        );
        let expected_y3 = 5.0 + 0.1;
        assert!(
            (y3 - expected_y3).abs() < 1e-4,
            "amount 10 should clamp to min 0.1 height, got {y3}"
        );
    }

    #[test]
    fn stacked_liquid_has_no_internal_faces() {
        let mut chunk = make_test_chunk();
        chunk.set_liquid(5, 5, 5, Some(Liquid::water(255)));
        chunk.set_liquid(5, 6, 5, Some(Liquid::water(255)));
        let mesh = mesh_liquid(&chunk, ChunkPos { x: 0, y: 0, z: 0 }, |_, _, _| None)
            .expect("should produce mesh");

        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .as_float3()
            .unwrap();
        let normals = mesh
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .unwrap()
            .as_float3()
            .unwrap();

        // There should be no horizontal face (normal (0, 1, 0) or (0, -1, 0)) at y = 6.0
        for (p, n) in positions.iter().zip(normals) {
            if (n[1].abs() - 1.0).abs() < 1e-4 {
                assert!(
                    (p[1] - 6.0).abs() > 1e-4,
                    "found internal horizontal face at y=6.0: {p:?}"
                );
            }
        }
    }

    #[test]
    fn liquid_top_face_only_below_air() {
        // 1. Water with AIR above -> top face is generated
        let mut c_air = make_test_chunk();
        c_air.set_liquid(5, 5, 5, Some(Liquid::water(255)));
        let m_air = mesh_liquid(&c_air, ChunkPos { x: 0, y: 0, z: 0 }, |_, _, _| None)
            .expect("mesh with air above");
        let normals_air = m_air
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .unwrap()
            .as_float3()
            .unwrap();
        assert!(
            normals_air.iter().any(|n| (n[1] - 1.0).abs() < 1e-4),
            "air above must generate top face"
        );

        // 2. Water with STONE (opaque) above -> no top face
        let mut c_stone = make_test_chunk();
        c_stone.set_liquid(5, 5, 5, Some(Liquid::water(255)));
        c_stone.set(5, 6, 5, STONE);
        let m_stone = mesh_liquid(&c_stone, ChunkPos { x: 0, y: 0, z: 0 }, |_, _, _| None)
            .expect("mesh with stone above");
        let normals_stone = m_stone
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .unwrap()
            .as_float3()
            .unwrap();
        assert!(
            !normals_stone.iter().any(|n| (n[1] - 1.0).abs() < 1e-4),
            "stone above must not generate top face"
        );

        // 3. Water with WATER above -> no top face
        let mut c_water = make_test_chunk();
        c_water.set_liquid(5, 5, 5, Some(Liquid::water(255)));
        c_water.set_liquid(5, 6, 5, Some(Liquid::water(255)));
        let m_water = mesh_liquid(&c_water, ChunkPos { x: 0, y: 0, z: 0 }, |_, _, _| None)
            .expect("mesh with water above");
        let normals_water = m_water
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .unwrap()
            .as_float3()
            .unwrap();
        let positions_water = m_water
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .as_float3()
            .unwrap();
        for (p, n) in positions_water.iter().zip(normals_water) {
            if (n[1] - 1.0).abs() < 1e-4 {
                assert!(
                    (p[1] - 6.0).abs() > 1e-4,
                    "submerged water cell must not have top face"
                );
            }
        }
    }

    #[test]
    fn side_faces_only_against_non_liquid() {
        // Water surrounded by stone on all 4 sides -> 0 side faces
        let mut c_surrounded = make_test_chunk();
        c_surrounded.set_liquid(5, 5, 5, Some(Liquid::water(255)));
        c_surrounded.set(4, 5, 5, STONE);
        c_surrounded.set(6, 5, 5, STONE);
        c_surrounded.set(5, 5, 4, STONE);
        c_surrounded.set(5, 5, 6, STONE);
        let m_surrounded =
            mesh_liquid(&c_surrounded, ChunkPos { x: 0, y: 0, z: 0 }, |_, _, _| None)
                .expect("mesh");
        let normals = m_surrounded
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .unwrap()
            .as_float3()
            .unwrap();
        // Only top face should exist (normal Y = 1.0)
        for n in normals {
            assert!(
                (n[1] - 1.0).abs() < 1e-4,
                "should only have top face, found normal {n:?}"
            );
        }

        // Water with air on +x side -> generates side face with normal (+1, 0, 0)
        let mut c_one_open = make_test_chunk();
        c_one_open.set_liquid(5, 5, 5, Some(Liquid::water(255)));
        c_one_open.set(4, 5, 5, STONE);
        c_one_open.set(5, 5, 4, STONE);
        c_one_open.set(5, 5, 6, STONE);
        let m_one_open =
            mesh_liquid(&c_one_open, ChunkPos { x: 0, y: 0, z: 0 }, |_, _, _| None).expect("mesh");
        let normals_open = m_one_open
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .unwrap()
            .as_float3()
            .unwrap();
        assert!(
            normals_open.iter().any(|n| (n[0] - 1.0).abs() < 1e-4),
            "+X side face must be generated"
        );
        assert!(
            !normals_open.iter().any(|n| (n[0] + 1.0).abs() < 1e-4),
            "-X side face must not be generated against stone"
        );

        // Water with water on +x side -> NO side face between them
        let mut c_two_water = make_test_chunk();
        c_two_water.set_liquid(5, 5, 5, Some(Liquid::water(255)));
        c_two_water.set_liquid(6, 5, 5, Some(Liquid::water(255)));
        c_two_water.set(4, 5, 5, STONE);
        c_two_water.set(5, 5, 4, STONE);
        c_two_water.set(5, 5, 6, STONE);
        c_two_water.set(7, 5, 5, STONE);
        c_two_water.set(6, 5, 4, STONE);
        c_two_water.set(6, 5, 6, STONE);
        let m_two_water =
            mesh_liquid(&c_two_water, ChunkPos { x: 0, y: 0, z: 0 }, |_, _, _| None).expect("mesh");
        let normals_two = m_two_water
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .unwrap()
            .as_float3()
            .unwrap();
        assert!(
            !normals_two.iter().any(|n| n[0].abs() > 0.5),
            "no side face along X between adjacent liquids"
        );
    }

    #[test]
    fn liquid_vertex_colors_match_spec() {
        use bevy::mesh::VertexAttributeValues;
        let mut cw = make_test_chunk();
        cw.set_liquid(0, 0, 0, Some(Liquid::water(255)));
        let mw = mesh_liquid(&cw, ChunkPos { x: 0, y: 0, z: 0 }, |_, _, _| None).unwrap();
        let VertexAttributeValues::Float32x4(cols_w) = mw.attribute(Mesh::ATTRIBUTE_COLOR).unwrap()
        else {
            panic!("expected Float32x4");
        };
        let expected_w = LinearRgba::from(Color::srgba(0.18, 0.45, 0.88, 0.65)).to_f32_array();
        for c in cols_w {
            for i in 0..4 {
                assert!((c[i] - expected_w[i]).abs() < 1e-4);
            }
        }

        let mut cl = make_test_chunk();
        cl.set_liquid(0, 0, 0, Some(Liquid::lava(255)));
        let ml = mesh_liquid(&cl, ChunkPos { x: 0, y: 0, z: 0 }, |_, _, _| None).unwrap();
        let VertexAttributeValues::Float32x4(cols_l) = ml.attribute(Mesh::ATTRIBUTE_COLOR).unwrap()
        else {
            panic!("expected Float32x4");
        };
        let expected_l = LinearRgba::from(Color::srgb(0.98, 0.45, 0.08)).to_f32_array();
        for c in cols_l {
            for i in 0..4 {
                assert!((c[i] - expected_l[i]).abs() < 1e-4);
            }
        }
    }
}
