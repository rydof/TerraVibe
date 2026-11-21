//! Face axes and a CPU-side mesh buffer.

/// One of the six axis-aligned cube faces.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Face {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

impl Face {
    pub const ALL: [Face; 6] = [
        Face::PosX,
        Face::NegX,
        Face::PosY,
        Face::NegY,
        Face::PosZ,
        Face::NegZ,
    ];

    pub fn opposite(self) -> Face {
        match self {
            Face::PosX => Face::NegX,
            Face::NegX => Face::PosX,
            Face::PosY => Face::NegY,
            Face::NegY => Face::PosY,
            Face::PosZ => Face::NegZ,
            Face::NegZ => Face::PosZ,
        }
    }

    /// Outward unit normal.
    pub fn normal(self) -> [i32; 3] {
        match self {
            Face::PosX => [1, 0, 0],
            Face::NegX => [-1, 0, 0],
            Face::PosY => [0, 1, 0],
            Face::NegY => [0, -1, 0],
            Face::PosZ => [0, 0, 1],
            Face::NegZ => [0, 0, -1],
        }
    }

    /// Sprite column direction. Chosen so that, seen from outside the face,
    /// `u` points right and `v` points down (`u x v = -normal`), so sprites
    /// are never mirrored.
    pub fn u_axis(self) -> [i32; 3] {
        match self {
            Face::PosX => [0, 0, -1],
            Face::NegX => [0, 0, 1],
            Face::PosY | Face::NegY | Face::PosZ => [1, 0, 0],
            Face::NegZ => [-1, 0, 0],
        }
    }

    /// Sprite row direction. Side faces use `-Y`, so sprite row 0 is on top.
    pub fn v_axis(self) -> [i32; 3] {
        match self {
            Face::PosY => [0, 0, 1],
            Face::NegY => [0, 0, -1],
            _ => [0, -1, 0],
        }
    }
}

/// A rectangle in texture space.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct UvRect {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
}

/// Triangle-list mesh data ready to upload.
#[derive(Default)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    /// Linear RGBA per vertex; empty for meshes without vertex colours.
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

impl MeshData {
    /// Appends a quad. Corners go counter-clockwise when viewed from the
    /// side the normal points to; emitted as triangles (0,1,2) and (0,2,3).
    pub fn push_quad(&mut self, corners: [[f32; 3]; 4], normal: [f32; 3], uvs: [[f32; 2]; 4]) {
        let base = self.positions.len() as u32;
        self.positions.extend_from_slice(&corners);
        self.normals.extend_from_slice(&[normal; 4]);
        self.uvs.extend_from_slice(&uvs);
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// `push_quad` plus one vertex colour per corner. A mesh uses either this
    /// or `push_quad` throughout, never both.
    pub fn push_quad_colored(
        &mut self,
        corners: [[f32; 3]; 4],
        normal: [f32; 3],
        uvs: [[f32; 2]; 4],
        colors: [[f32; 4]; 4],
    ) {
        self.push_quad(corners, normal, uvs);
        self.colors.extend_from_slice(&colors);
    }

    pub fn quad_count(&self) -> usize {
        self.indices.len() / 6
    }
}

/// One flat quad covering the whole of `face` of the unit block at
/// `block_origin`, sampling all of `uv` (never mirrored, row 0 on top).
pub fn flat_face(face: Face, block_origin: [f32; 3], uv: UvRect, out: &mut MeshData) {
    let (u, v, n) = (face.u_axis(), face.v_axis(), face.normal());
    // Grid corner: the face-square corner from which +u and +v stay inside.
    let corner: [f32; 3] = std::array::from_fn(|i| {
        block_origin[i]
            + if u[i] < 0 || v[i] < 0 || n[i] > 0 {
                1.0
            } else {
                0.0
            }
    });
    let at = |c: f32, r: f32| -> [f32; 3] {
        std::array::from_fn(|i| corner[i] + u[i] as f32 * c + v[i] as f32 * r)
    };
    // u x v = -normal, so (0,0),(0,1),(1,1),(1,0) is counter-clockwise from outside.
    let cr = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];
    let pts = cr.map(|[c, r]| at(c, r));
    let uvs = cr.map(|[c, r]| [uv.u0 + (uv.u1 - uv.u0) * c, uv.v0 + (uv.v1 - uv.v0) * r]);
    out.push_quad(pts, n.map(|x| x as f32), uvs);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cross(a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }

    #[test]
    fn side_v_axis_points_down() {
        assert_eq!(Face::PosX.v_axis(), [0, -1, 0]);
    }

    #[test]
    fn axes_are_unmirrored() {
        for f in Face::ALL {
            let n = f.normal();
            assert_eq!(
                cross(f.u_axis(), f.v_axis()),
                [-n[0], -n[1], -n[2]],
                "{f:?}"
            );
        }
    }

    #[test]
    fn push_quad_counts() {
        let mut m = MeshData::default();
        m.push_quad([[0.0; 3]; 4], [0.0, 1.0, 0.0], [[0.0; 2]; 4]);
        m.push_quad([[1.0; 3]; 4], [0.0, 1.0, 0.0], [[0.0; 2]; 4]);
        assert_eq!(m.quad_count(), 2);
        assert_eq!(m.positions.len(), 8);
        assert_eq!(&m.indices[6..], &[4, 5, 6, 4, 6, 7]);
    }
}
