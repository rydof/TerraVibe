//! Meshes one 32^3 chunk: greedy-merged textured quads with `Texture2DArray`
//! layer indices in `UV_1` and per-vertex ambient occlusion in vertex colours.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::Mesh;
use terravibe_sim::{AIR, BlockId, CHUNK_SIZE, Chunk, ChunkPos, GLASS_BLOCK, World};

use super::blocks::BlockArt;
use super::is_model_block;
use super::mesh_data::{Face, MeshData};

const CS: usize = CHUNK_SIZE as usize;
const PAD_SIDE: usize = CS + 2;

/// Zero-clone 34³ block-id snapshot for greedy meshing on a worker thread.
pub struct ChunkMeshInput {
    pub pos: ChunkPos,
    /// `PAD_SIDE^3` block IDs, indexed `(lx + 1) + (ly + 1)*34 + (lz + 1)*34*34`.
    blocks: Box<[u16]>,
    /// Center chunk's wall bitset (`4096` `u64`s -> `512` `u64`s), if any.
    walls: Option<Box<[u64]>>,
    /// True if the center chunk contains any `GLASS_BLOCK` (needs `AlphaMode::Mask`).
    pub has_glass: bool,
}

impl ChunkMeshInput {
    #[inline]
    fn idx(px: usize, py: usize, pz: usize) -> usize {
        px + py * PAD_SIDE + pz * PAD_SIDE * PAD_SIDE
    }

    #[inline]
    fn block_local(&self, lx: i32, ly: i32, lz: i32) -> BlockId {
        BlockId(self.blocks[Self::idx((lx + 1) as usize, (ly + 1) as usize, (lz + 1) as usize)])
    }

    #[inline]
    fn wall_local(&self, lx: usize, ly: usize, lz: usize) -> bool {
        let Some(w) = self.walls.as_ref() else {
            return false;
        };
        let i = lx + ly * CS + lz * CS * CS;
        (w[i / 64] >> (i % 64)) & 1 == 1
    }

    /// Builds a `34^3` input buffer from the center chunk and its 6 face
    /// neighbours `[+X, -X, +Y, -Y, +Z, -Z]` without cloning any `Chunk`s.
    pub fn from_chunks(pos: ChunkPos, center: &Chunk, neighbors: [Option<&Chunk>; 6]) -> Self {
        let mut blocks = vec![AIR.0; PAD_SIDE * PAD_SIDE * PAD_SIDE].into_boxed_slice();
        let mut has_glass = false;

        for lz in 0..CS {
            for ly in 0..CS {
                for lx in 0..CS {
                    let b = center.get(lx, ly, lz);
                    if b == GLASS_BLOCK {
                        has_glass = true;
                    }
                    blocks[Self::idx(lx + 1, ly + 1, lz + 1)] = b.0;
                }
            }
        }

        // +X (nlx = 0 -> px = 33)
        if let Some(c) = neighbors[0] {
            for lz in 0..CS {
                for ly in 0..CS {
                    blocks[Self::idx(CS + 1, ly + 1, lz + 1)] = c.get(0, ly, lz).0;
                }
            }
        }
        // -X (nlx = 31 -> px = 0)
        if let Some(c) = neighbors[1] {
            for lz in 0..CS {
                for ly in 0..CS {
                    blocks[Self::idx(0, ly + 1, lz + 1)] = c.get(CS - 1, ly, lz).0;
                }
            }
        }
        // +Y (nly = 0 -> py = 33)
        if let Some(c) = neighbors[2] {
            for lz in 0..CS {
                for lx in 0..CS {
                    blocks[Self::idx(lx + 1, CS + 1, lz + 1)] = c.get(lx, 0, lz).0;
                }
            }
        }
        // -Y (nly = 31 -> py = 0)
        if let Some(c) = neighbors[3] {
            for lz in 0..CS {
                for lx in 0..CS {
                    blocks[Self::idx(lx + 1, 0, lz + 1)] = c.get(lx, CS - 1, lz).0;
                }
            }
        }
        // +Z (nlz = 0 -> pz = 33)
        if let Some(c) = neighbors[4] {
            for ly in 0..CS {
                for lx in 0..CS {
                    blocks[Self::idx(lx + 1, ly + 1, CS + 1)] = c.get(lx, ly, 0).0;
                }
            }
        }
        // -Z (nlz = 31 -> pz = 0)
        if let Some(c) = neighbors[5] {
            for ly in 0..CS {
                for lx in 0..CS {
                    blocks[Self::idx(lx + 1, ly + 1, 0)] = c.get(lx, ly, CS - 1).0;
                }
            }
        }

        let walls = if center.has_walls() {
            let mut w = vec![0u64; (CS * CS * CS) / 64].into_boxed_slice();
            for lz in 0..CS {
                for ly in 0..CS {
                    for lx in 0..CS {
                        if center.wall(lx, ly, lz) {
                            let i = lx + ly * CS + lz * CS * CS;
                            w[i / 64] |= 1u64 << (i % 64);
                        }
                    }
                }
            }
            Some(w)
        } else {
            None
        };

        Self {
            pos,
            blocks,
            walls,
            has_glass,
        }
    }
}

/// CPU-side greedy mesh with `UV_1` carrying `[layer, 0.0]` for `Texture2DArray`.
#[derive(Default)]
pub struct GreedyMeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub uv_bs: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

impl GreedyMeshData {
    pub fn quad_count(&self) -> usize {
        self.indices.len() / 6
    }

    fn push_quad(
        &mut self,
        corners: [[f32; 3]; 4],
        normal: [f32; 3],
        uvs: [[f32; 2]; 4],
        layer: u32,
        colors: [[f32; 4]; 4],
    ) {
        let base = self.positions.len() as u32;
        self.positions.extend_from_slice(&corners);
        self.normals.extend_from_slice(&[normal; 4]);
        self.uvs.extend_from_slice(&uvs);
        self.uv_bs.extend_from_slice(&[[layer as f32, 0.0]; 4]);
        self.colors.extend_from_slice(&colors);
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Converts CPU mesh data into Bevy meshes of at most `max_quads` quads each (`u16` indices).
    pub fn to_bevy_mesh_parts(self, max_quads: usize) -> Vec<Mesh> {
        assert!(max_quads * 4 <= usize::from(u16::MAX) + 1);
        let quads = self.quad_count();
        (0..quads)
            .step_by(max_quads)
            .map(|q0| {
                let q1 = (q0 + max_quads).min(quads);
                let (v0, v1) = (q0 * 4, q1 * 4);
                let base = v0 as u32;
                let indices = self.indices[q0 * 6..q1 * 6]
                    .iter()
                    .map(|&i| (i - base) as u16)
                    .collect();
                Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::RENDER_WORLD,
                )
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions[v0..v1].to_vec())
                .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals[v0..v1].to_vec())
                .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs[v0..v1].to_vec())
                .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, self.uv_bs[v0..v1].to_vec())
                .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors[v0..v1].to_vec())
                .with_inserted_indices(Indices::U16(indices))
            })
            .collect()
    }
}

/// Directional sunlight weight per face normal.
pub fn face_lighting_weight(face: Face) -> f32 {
    match face {
        Face::PosY => 1.0,
        Face::PosX => 0.90,
        Face::PosZ => 0.85,
        Face::NegZ => 0.75,
        Face::NegX => 0.70,
        Face::NegY => 0.55,
    }
}

/// Brightness per AO level (0 = open corner .. 3 = fully boxed in).
const AO: [f32; 4] = [1.0, 0.8, 0.65, 0.5];
/// Gap between a wall panel and the solid face it covers.
const WALL_INSET: f32 = 0.1;
/// Wall panel brightness.
const WALL_TINT: f32 = 0.6;

/// Whether `b` is drawn as a chunk cube.
fn is_cube(b: BlockId) -> bool {
    b.is_opaque() && !is_model_block(b)
}

/// Whether `b` hides the face of a neighbour (glass shows what is behind it).
fn hides(b: BlockId) -> bool {
    is_cube(b) && b != GLASS_BLOCK
}

/// Corner `(c, r)` (0 or 1 along the face's u and v axes) of the face of the
/// block at `o`.
fn corner(face: Face, o: [i32; 3], c: u32, r: u32) -> [f32; 3] {
    let (n, u, v) = (face.normal(), face.u_axis(), face.v_axis());
    std::array::from_fn(|a| {
        let along = |d: i32, t: u32| match d {
            1 => t as f32,
            -1 => 1.0 - t as f32,
            _ => 0.0,
        };
        o[a] as f32 + f32::from(n[a] > 0) + along(u[a], c) + along(v[a], r)
    })
}

/// Pushes one face quad; `ao` per corner in the order (0,0), (0,1), (1,1), (1,0).
fn push_face(
    out: &mut MeshData,
    face: Face,
    o: [i32; 3],
    uv: [f32; 4],
    col: [[f32; 4]; 4],
    ao: [u8; 4],
) {
    const CORNERS: [(u32, u32); 4] = [(0, 0), (0, 1), (1, 1), (1, 0)];
    let mut pos = CORNERS.map(|(c, r)| corner(face, o, c, r));
    let mut uvs = CORNERS.map(|(c, r)| {
        [
            uv[0] + c as f32 * (uv[2] - uv[0]),
            uv[1] + r as f32 * (uv[3] - uv[1]),
        ]
    });
    let mut col = col;
    // Split along the diagonal that avoids the AO seam.
    if ao[0] + ao[2] > ao[1] + ao[3] {
        pos.rotate_left(1);
        uvs.rotate_left(1);
        col.rotate_left(1);
    }
    let n = face.normal();
    out.push_quad_colored(pos, [n[0] as f32, n[1] as f32, n[2] as f32], uvs, col);
}

/// Pushes one greedy-meshed quad of size `(w, h)` with `Texture2DArray` `layer`.
#[allow(clippy::too_many_arguments)]
fn push_greedy_face(
    out: &mut GreedyMeshData,
    face: Face,
    o: [i32; 3],
    w: u32,
    h: u32,
    layer: u32,
    col: [[f32; 4]; 4],
    ao: [u8; 4],
) {
    let corners = [(0, 0), (0, h), (w, h), (w, 0)];
    let mut pos = corners.map(|(c, r)| corner(face, o, c, r));
    let mut uvs = corners.map(|(c, r)| [c as f32, r as f32]);
    let mut col = col;
    if ao[0] + ao[2] > ao[1] + ao[3] {
        pos.rotate_left(1);
        uvs.rotate_left(1);
        col.rotate_left(1);
    }
    let n = face.normal();
    out.push_quad(
        pos,
        [n[0] as f32, n[1] as f32, n[2] as f32],
        uvs,
        layer,
        col,
    );
}

#[derive(Copy, Clone, PartialEq, Eq)]
struct GreedyCell {
    block: u16,
    layer: u32,
    ao: [u8; 4],
}

/// Greedy-meshes `input` using `Texture2DArray` layer indices and tiled UVs.
pub fn mesh_chunk_greedy(input: &ChunkMeshInput, art: &BlockArt) -> GreedyMeshData {
    let mut out = GreedyMeshData::default();
    let pos = input.pos;
    let base = [pos.x * CHUNK_SIZE, pos.y * CHUNK_SIZE, pos.z * CHUNK_SIZE];
    let mut mask: [Option<GreedyCell>; CS * CS] = [None; CS * CS];

    for face in Face::ALL {
        let (n, u, v) = (face.normal(), face.u_axis(), face.v_axis());
        let weight = face_lighting_weight(face);
        let axis_coord = |a: usize, d: usize, cu: usize, cv: usize| -> usize {
            if n[a] != 0 {
                d
            } else if u[a] == 1 {
                cu
            } else if u[a] == -1 {
                CS - 1 - cu
            } else if v[a] == 1 {
                cv
            } else {
                CS - 1 - cv
            }
        };

        for d in 0..CS {
            mask.fill(None);
            for cv in 0..CS {
                for cu in 0..CS {
                    let lx = axis_coord(0, d, cu, cv) as i32;
                    let ly = axis_coord(1, d, cu, cv) as i32;
                    let lz = axis_coord(2, d, cu, cv) as i32;
                    let b = input.block_local(lx, ly, lz);
                    if !is_cube(b) || !art.has(b) {
                        continue;
                    }
                    let out_cell = [lx + n[0], ly + n[1], lz + n[2]];
                    let nb = input.block_local(out_cell[0], out_cell[1], out_cell[2]);
                    if hides(nb) || (b == GLASS_BLOCK && nb == GLASS_BLOCK) {
                        continue;
                    }
                    let occ = |du: i32, dv: i32| {
                        hides(input.block_local(
                            out_cell[0] + du * u[0] + dv * v[0],
                            out_cell[1] + du * u[1] + dv * v[1],
                            out_cell[2] + du * u[2] + dv * v[2],
                        ))
                    };
                    let mut ao = [0u8; 4];
                    for (i, (du, dv)) in
                        [(-1, -1), (-1, 1), (1, 1), (1, -1)].into_iter().enumerate()
                    {
                        let (s1, s2, c) = (occ(du, 0), occ(0, dv), occ(du, dv));
                        ao[i] = if s1 && s2 {
                            3
                        } else {
                            u8::from(s1) + u8::from(s2) + u8::from(c)
                        };
                    }
                    let layer = art.face_layer(b, face, base[0] + lx, base[1] + ly, base[2] + lz);
                    mask[cv * CS + cu] = Some(GreedyCell {
                        block: b.0,
                        layer,
                        ao,
                    });
                }
            }

            // Greedy 2D rectangle merge on `mask`.
            for cv0 in 0..CS {
                let mut cu0 = 0;
                while cu0 < CS {
                    let Some(cell) = mask[cv0 * CS + cu0] else {
                        cu0 += 1;
                        continue;
                    };
                    let mut w = 1usize;
                    while cu0 + w < CS && mask[cv0 * CS + cu0 + w] == Some(cell) {
                        w += 1;
                    }
                    let mut h = 1usize;
                    'height: while cv0 + h < CS {
                        for du in 0..w {
                            if mask[(cv0 + h) * CS + cu0 + du] != Some(cell) {
                                break 'height;
                            }
                        }
                        h += 1;
                    }
                    for dv in 0..h {
                        for du in 0..w {
                            mask[(cv0 + dv) * CS + cu0 + du] = None;
                        }
                    }
                    let lx = axis_coord(0, d, cu0, cv0) as i32;
                    let ly = axis_coord(1, d, cu0, cv0) as i32;
                    let lz = axis_coord(2, d, cu0, cv0) as i32;
                    let col = cell.ao.map(|a| {
                        let k = AO[a as usize] * weight;
                        [k, k, k, 1.0]
                    });
                    push_greedy_face(
                        &mut out,
                        face,
                        [base[0] + lx, base[1] + ly, base[2] + lz],
                        w as u32,
                        h as u32,
                        cell.layer,
                        col,
                        cell.ao,
                    );
                    cu0 += w;
                }
            }
        }
    }

    if input.walls.is_some() {
        mesh_walls_greedy(input, art, &mut out);
    }
    out
}

fn mesh_walls_greedy(input: &ChunkMeshInput, art: &BlockArt, out: &mut GreedyMeshData) {
    let layer = art.wall_layer();
    let pos = input.pos;
    let base = [pos.x * CHUNK_SIZE, pos.y * CHUNK_SIZE, pos.z * CHUNK_SIZE];
    let tint = [[WALL_TINT, WALL_TINT, WALL_TINT, 1.0]; 4];
    for lz in 0..CS {
        for ly in 0..CS {
            for lx in 0..CS {
                if !input.wall_local(lx, ly, lz) {
                    continue;
                }
                if input
                    .block_local(lx as i32, ly as i32, lz as i32)
                    .is_opaque()
                {
                    continue;
                }
                for face in Face::ALL {
                    let n = face.normal();
                    let nbx = lx as i32 + n[0];
                    let nby = ly as i32 + n[1];
                    let nbz = lz as i32 + n[2];
                    if !is_cube(input.block_local(nbx, nby, nbz)) {
                        continue;
                    }
                    let start = out.positions.len();
                    push_greedy_face(
                        out,
                        face.opposite(),
                        [base[0] + nbx, base[1] + nby, base[2] + nbz],
                        1,
                        1,
                        layer,
                        tint,
                        [0; 4],
                    );
                    for p in &mut out.positions[start..] {
                        for a in 0..3 {
                            p[a] -= n[a] as f32 * WALL_INSET;
                        }
                    }
                }
            }
        }
    }
}

/// Meshes `pos` with vertex positions in world coordinates.
pub fn mesh_chunk(world: &World, pos: ChunkPos, art: &BlockArt) -> MeshData {
    let mut out = MeshData::default();
    let base = [pos.x * CHUNK_SIZE, pos.y * CHUNK_SIZE, pos.z * CHUNK_SIZE];
    for lx in 0..CHUNK_SIZE {
        for ly in 0..CHUNK_SIZE {
            for lz in 0..CHUNK_SIZE {
                let (x, y, z) = (base[0] + lx, base[1] + ly, base[2] + lz);
                let b = world.block(x, y, z);
                if !is_cube(b) || !art.has(b) {
                    continue;
                }
                for face in Face::ALL {
                    let n = face.normal();
                    let out_cell = [x + n[0], y + n[1], z + n[2]];
                    let nb = world.block(out_cell[0], out_cell[1], out_cell[2]);
                    if hides(nb) || (b == GLASS_BLOCK && nb == GLASS_BLOCK) {
                        continue;
                    }
                    let (u, v) = (face.u_axis(), face.v_axis());
                    let occ = |du: i32, dv: i32| {
                        let p: [i32; 3] =
                            std::array::from_fn(|a| out_cell[a] + du * u[a] + dv * v[a]);
                        hides(world.block(p[0], p[1], p[2]))
                    };
                    let mut ao = [0u8; 4];
                    for (i, (du, dv)) in
                        [(-1, -1), (-1, 1), (1, 1), (1, -1)].into_iter().enumerate()
                    {
                        let (s1, s2, c) = (occ(du, 0), occ(0, dv), occ(du, dv));
                        ao[i] = if s1 && s2 {
                            3
                        } else {
                            u8::from(s1) + u8::from(s2) + u8::from(c)
                        };
                    }
                    let weight = face_lighting_weight(face);
                    let col = ao.map(|a| {
                        let k = AO[a as usize] * weight;
                        [k, k, k, 1.0]
                    });
                    push_face(
                        &mut out,
                        face,
                        [x, y, z],
                        art.face_uv(b, face, x, y, z),
                        col,
                        ao,
                    );
                }
            }
        }
    }
    if world.chunk(pos).is_some_and(|c| c.has_walls()) {
        mesh_walls(world, pos, art, &mut out);
    }
    out
}

/// Wall panels: for each walled air cell, a quad on every face toward a cube
/// neighbour, pushed `WALL_INSET` into the cell. Runs only for chunks that
/// have walls. Walls do not block sky light (they sit in rooms that already
/// have a solid roof).
fn mesh_walls(world: &World, pos: ChunkPos, art: &BlockArt, out: &mut MeshData) {
    let uv = art.wall_uv();
    let c = world.chunk(pos).unwrap();
    let base = [pos.x * CHUNK_SIZE, pos.y * CHUNK_SIZE, pos.z * CHUNK_SIZE];
    let tint = [[WALL_TINT, WALL_TINT, WALL_TINT, 1.0]; 4];
    for lz in 0..CHUNK_SIZE as usize {
        for ly in 0..CHUNK_SIZE as usize {
            for lx in 0..CHUNK_SIZE as usize {
                if !c.wall(lx, ly, lz) {
                    continue;
                }
                let cell = [
                    base[0] + lx as i32,
                    base[1] + ly as i32,
                    base[2] + lz as i32,
                ];
                if world.block(cell[0], cell[1], cell[2]).is_opaque() {
                    continue;
                }
                for face in Face::ALL {
                    let n = face.normal();
                    let nb = [cell[0] + n[0], cell[1] + n[1], cell[2] + n[2]];
                    if !is_cube(world.block(nb[0], nb[1], nb[2])) {
                        continue;
                    }
                    // The neighbour's face toward this cell, nudged into the cell.
                    let start = out.positions.len();
                    push_face(out, face.opposite(), nb, uv, tint, [0; 4]);
                    for p in &mut out.positions[start..] {
                        for a in 0..3 {
                            p[a] -= n[a] as f32 * WALL_INSET;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terravibe_sim::{AIR, Chunk, DIRT, TORCH, TRUNK, WORKBENCH};

    fn art() -> &'static BlockArt {
        use std::sync::OnceLock;
        static ART: OnceLock<BlockArt> = OnceLock::new();
        ART.get_or_init(|| BlockArt::try_load(&super::super::art_dir()).unwrap())
    }

    const C0: ChunkPos = ChunkPos { x: 0, y: 0, z: 0 };

    fn world_of(id: BlockId, blocks: &[(i32, i32, i32)]) -> World {
        let mut w = World::new();
        for &(x, y, z) in blocks {
            let cp = ChunkPos::of_block(x, y, z);
            if w.chunk(cp).is_none() {
                w.insert(cp, Chunk::filled(AIR));
            }
            let mut c = w.chunk(cp).unwrap().clone();
            let m = |v: i32| v.rem_euclid(CHUNK_SIZE) as usize;
            c.set(m(x), m(y), m(z), id);
            w.insert(cp, c);
        }
        w
    }

    fn world_with(blocks: &[(i32, i32, i32)]) -> World {
        world_of(DIRT, blocks)
    }

    #[test]
    fn face_normal_lighting_weights_ordering() {
        let top = face_lighting_weight(Face::PosY);
        let sun_x = face_lighting_weight(Face::PosX);
        let sun_z = face_lighting_weight(Face::PosZ);
        let opp_x = face_lighting_weight(Face::NegX);
        let opp_z = face_lighting_weight(Face::NegZ);
        let bottom = face_lighting_weight(Face::NegY);

        assert_eq!(top, 1.0, "top face must have full sun lighting 1.0");
        assert!(
            (0.85..=0.95).contains(&sun_x),
            "sun-facing x side must be highlighted (0.85..=0.95), got {sun_x}"
        );
        assert!(
            (0.85..=0.95).contains(&sun_z),
            "sun-facing z side must be highlighted (0.85..=0.95), got {sun_z}"
        );
        assert!(
            (0.65..=0.75).contains(&opp_x),
            "opposite x side must be ambient shaded (0.65..=0.75), got {opp_x}"
        );
        assert!(
            (0.65..=0.75).contains(&opp_z),
            "opposite z side must be ambient shaded (0.65..=0.75), got {opp_z}"
        );
        assert!(
            (0.5..=0.6).contains(&bottom),
            "bottom face must be shaded (0.5..=0.6), got {bottom}"
        );

        assert!(top > sun_x && sun_x > opp_x && opp_x > bottom);
        assert!(top > sun_z && sun_z > opp_z && opp_z > bottom);
    }

    #[test]
    fn single_block_six_quads() {
        let m = mesh_chunk(&world_with(&[(0, 0, 0)]), C0, art());
        assert_eq!(m.quad_count(), 6);
        assert_eq!(m.colors.len(), m.positions.len());
    }

    #[test]
    fn adjacent_blocks_cull_shared_face() {
        let m = mesh_chunk(&world_with(&[(0, 0, 0), (1, 0, 0)]), C0, art());
        assert_eq!(m.quad_count(), 10);
    }

    #[test]
    fn no_face_across_chunk_border() {
        let w = world_with(&[(31, 0, 0), (32, 0, 0)]);
        assert_eq!(mesh_chunk(&w, C0, art()).quad_count(), 5);
    }

    #[test]
    fn lone_block_has_no_occlusion() {
        let m = mesh_chunk(&world_with(&[(0, 0, 0)]), C0, art());
        for face in Face::ALL {
            let expected = face_lighting_weight(face);
            let n = face.normal().map(|v| v as f32);
            let quads_matching: Vec<_> = (0..m.quad_count())
                .filter(|&q| m.normals[q * 4] == n)
                .collect();
            assert_eq!(quads_matching.len(), 1, "face {face:?} should have 1 quad");
            let q = quads_matching[0];
            for v in 0..4 {
                assert!(
                    (m.colors[q * 4 + v][0] - expected).abs() < 1e-4,
                    "face {face:?} vertex color should be {expected}, got {}",
                    m.colors[q * 4 + v][0]
                );
            }
        }
    }

    #[test]
    fn ao_darkens_concave_corner() {
        // Floor block with a wall block beside and above it: its top face is concave there.
        let m = mesh_chunk(&world_with(&[(0, 0, 0), (1, 1, 0)]), C0, art());
        assert!(m.colors.iter().any(|c| c[0] < 1.0));
    }

    #[test]
    fn ao_reads_across_chunk_border() {
        // The occluder at x=32 lives in the next chunk; the block at x=31 must still see it.
        let m = mesh_chunk(&world_with(&[(31, 1, 0), (32, 2, 0)]), C0, art());
        assert_eq!(m.quad_count(), 6);
        assert!(m.colors.iter().any(|c| c[0] < 1.0));
    }

    #[test]
    fn model_blocks_not_meshed() {
        assert_eq!(
            mesh_chunk(&world_of(WORKBENCH, &[(0, 0, 0)]), C0, art()).quad_count(),
            0
        );
        let mut w = world_with(&[(0, 0, 0)]);
        let mut c = w.chunk(C0).unwrap().clone();
        c.set(1, 0, 0, TORCH);
        w.insert(C0, c);
        assert_eq!(mesh_chunk(&w, C0, art()).quad_count(), 6);
    }

    #[test]
    fn trunk_and_cube_blocks_mesh() {
        for (id, name) in super::super::blocks::CUBE_BLOCKS {
            let m = mesh_chunk(&world_of(id, &[(0, 0, 0)]), C0, art());
            assert_eq!(m.quad_count(), 6, "{name}");
        }
        let two = world_of(TRUNK, &[(0, 0, 0), (0, 1, 0)]);
        assert_eq!(mesh_chunk(&two, C0, art()).quad_count(), 10);
    }

    #[test]
    fn walls_add_panels_only_where_walled_and_backed() {
        let mut w = world_with(&[(0, 0, 0)]);
        let base = mesh_chunk(&w, C0, art()).quad_count();
        let mut c = w.chunk(C0).unwrap().clone();
        c.set_wall(1, 0, 0, true);
        w.insert(C0, c);
        let m = mesh_chunk(&w, C0, art());
        assert_eq!(m.quad_count(), base + 1);
        let q = m.quad_count() - 1;
        assert!(
            m.positions[q * 4..q * 4 + 4]
                .iter()
                .all(|p| (p[0] - 1.1).abs() < 1e-5)
        );
        assert_eq!(m.normals[q * 4], [1.0, 0.0, 0.0]);
        assert!(m.colors[q * 4][0] < 1.0);
        let mut f = world_with(&[(0, 0, 0)]);
        let mut c = f.chunk(C0).unwrap().clone();
        c.set_wall(9, 9, 9, true);
        f.insert(C0, c);
        assert_eq!(mesh_chunk(&f, C0, art()).quad_count(), base);
    }

    #[test]
    fn greedy_merges_adjacent_uniform_blocks() {
        let mut c = Chunk::filled(AIR);
        for x in 0..4 {
            for z in 0..4 {
                c.set(x, 0, z, terravibe_sim::WOOD_BLOCK);
            }
        }
        let input = ChunkMeshInput::from_chunks(C0, &c, [None; 6]);
        let g = mesh_chunk_greedy(&input, art());
        // A 4x1x4 slab of uniform blocks merges into 1 quad per face = 6 quads total!
        assert_eq!(g.quad_count(), 6);
        assert_eq!(g.uv_bs.len(), g.positions.len());
        // Top/bottom faces have 4x4 tiled UVs.
        assert!(g.uvs.iter().any(|uv| uv[0] == 4.0 && uv[1] == 4.0));
    }

    #[test]
    fn winding_matches_normals_and_stays_in_unit_cube() {
        let m = mesh_chunk(&world_with(&[(0, 0, 0)]), C0, art());
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
        assert!(
            m.positions
                .iter()
                .flatten()
                .all(|&v| (0.0..=1.0).contains(&v))
        );
    }
}
