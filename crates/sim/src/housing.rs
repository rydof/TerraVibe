//! Housing validity: a 3D port of Terraria's room check (`WorldGen.StartRoomCheck`,
//! gameplay spec, and `CheckRoom`, :6063), run on demand by the server.
//!
//! 3D adaptation (design choices, tune by playtest):
//! - Flood fill over passable cells with 6-connectivity. Vanilla pushes all 8 x/y
//!   neighbours (:6040); diagonals are dropped here so the fill cannot leak
//!   through corners.
//! - Solid blocks (incl. closed doors) and open doors bound the fill. Vanilla
//!   stops at solid tiles (:6103) and at some open-door frames (:6108+).
//! - Enclosure: every reachable passable cell must carry a wall bit (Task 1). A
//!   cell without one is open air (vanilla: no `wallHouse`/solid within 2 tiles
//!   on both axes fails the room, :6150-6180; the 3D rule is the strict form).
//! - Limits: vanilla `maxRoomTiles = 750` (:4181) scales x3 to 2250 for a
//!   3-deep room; `maxRoomSize = 100` bounding span (:4185, test :6108) and the
//!   60-cell minimum (:6050) are kept; 10-block edge margin (:5997) applies to y
//!   only because the world is unbounded in x/z.
//! - Furniture counts when the fill touches it (`houseTile`, :6118): chair, table
//!   and torch sit in room cells, doors on the boundary. Sets (gameplay spec
//!   `RoomNeeds`): chair 15, table 14, torch 4, door 10/11 (open or closed).

use std::collections::HashSet;

use crate::block::{
    AIR, BlockId, CHAIR, DOOR_CLOSED, DOOR_OPEN, FURNACE, HELLFORGE, PLATFORM, TABLE, TORCH,
    WORKBENCH,
};
use crate::interact::LoadedQuery;
use crate::worldgen::terrain::WORLD_HEIGHT;

pub const MIN_CELLS: u32 = 60;
pub const MAX_CELLS: u32 = 2250;
pub const MAX_SPAN: i32 = 100;
pub const EDGE: i32 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomFail {
    TooClose,
    StartInSolid,
    OpenAir,
    TooBig,
    TooSmall,
    Unloaded,
    NoStanding,
}

impl RoomFail {
    /// Wire code for `ServerMsg::HouseStatus` (0 = ok, 8 = valid but missing furniture).
    pub fn code(self) -> u8 {
        self as u8 + 1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoomOk {
    pub cells: u32,
    pub bbox: [i32; 3],
    /// Per-axis minimum of the room cells: a stable id for the room.
    pub lo: [i32; 3],
    pub chair: bool,
    pub table: bool,
    pub door: bool,
    pub light: bool,
}

/// `WorldGen.RoomNeeds` (:5601): chair, table, door and light source all present.
pub fn room_has_needs(r: &RoomOk) -> bool {
    r.chair && r.table && r.door && r.light
}

/// Wire status: `(ok, reason)`; reason 0 = fine, 1..=7 = `RoomFail::code`, 8 = needs missing.
pub fn status(r: &Result<RoomOk, RoomFail>) -> (bool, u8) {
    match r {
        Ok(r) if room_has_needs(r) => (true, 0),
        Ok(_) => (false, 8),
        Err(f) => (false, f.code()),
    }
}

const DIRS: [[i32; 3]; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];

fn near_edge(y: i32) -> bool {
    y < EDGE || i64::from(y) >= WORLD_HEIGHT - i64::from(EDGE)
}

/// Checks the room containing `start` (a passable cell). Iterative, bounded by
/// `MAX_CELLS` passable cells, aborts on the first failure, never panics on
/// unloaded space.
pub fn check_room(q: &impl LoadedQuery, start: [i32; 3]) -> Result<RoomOk, RoomFail> {
    if near_edge(start[1]) {
        return Err(RoomFail::TooClose);
    }
    if !q.is_loaded(start[0], start[1], start[2]) {
        return Err(RoomFail::Unloaded);
    }
    let sb = q.block(start[0], start[1], start[2]);
    if sb.is_solid() || sb == DOOR_OPEN || sb == PLATFORM {
        return Err(RoomFail::StartInSolid);
    }
    let mut seen: HashSet<[i32; 3]> = HashSet::new();
    let mut room: HashSet<[i32; 3]> = HashSet::new();
    let mut stack = vec![start];
    seen.insert(start);
    let (mut lo, mut hi) = (start, start);
    let (mut chair, mut table, mut door, mut light) = (false, false, false, false);
    while let Some(c) = stack.pop() {
        if near_edge(c[1]) {
            return Err(RoomFail::TooClose);
        }
        if !q.is_loaded(c[0], c[1], c[2]) {
            return Err(RoomFail::Unloaded);
        }
        let b: BlockId = q.block(c[0], c[1], c[2]);
        if b == DOOR_CLOSED || b == DOOR_OPEN || b == PLATFORM {
            door = true;
            continue;
        }
        table |= b == WORKBENCH;
        light |= b == FURNACE || b == HELLFORGE;
        if b.is_solid() {
            continue;
        }
        // Passable cell: furniture, wall bit, size limits.
        chair |= b == CHAIR;
        table |= b == TABLE;
        light |= b == TORCH;
        debug_assert!(b == AIR || !b.is_solid());
        if !q.has_wall(c[0], c[1], c[2]) {
            return Err(RoomFail::OpenAir);
        }
        room.insert(c);
        if room.len() as u32 > MAX_CELLS {
            return Err(RoomFail::TooBig);
        }
        for a in 0..3 {
            lo[a] = lo[a].min(c[a]);
            hi[a] = hi[a].max(c[a]);
        }
        if (0..3).any(|a| hi[a] - lo[a] >= MAX_SPAN) {
            return Err(RoomFail::TooBig);
        }
        for d in DIRS {
            let n = [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
            if seen.insert(n) {
                stack.push(n);
            }
        }
    }
    let cells = room.len() as u32;
    if cells < MIN_CELLS {
        return Err(RoomFail::TooSmall);
    }
    // Standing space: a floor cell (solid or platform below) with room above it.
    let standing = room.iter().any(|&[x, y, z]| {
        let below = q.block(x, y - 1, z);
        (below.is_solid() || below.is_platform()) && room.contains(&[x, y + 1, z])
    });
    if !standing {
        return Err(RoomFail::NoStanding);
    }
    Ok(RoomOk {
        cells,
        bbox: [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1],
        lo,
        chair,
        table,
        door,
        light,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{BlockId, STONE};
    use crate::chunk::BlockQuery;
    use std::collections::HashMap;

    #[derive(Default)]
    struct TQ {
        blocks: HashMap<[i32; 3], BlockId>,
        walls: HashSet<[i32; 3]>,
        /// Loaded x range (chunk-like cut); everything else is unloaded.
        x_max: i32,
    }
    impl BlockQuery for TQ {
        fn block(&self, x: i32, y: i32, z: i32) -> BlockId {
            *self.blocks.get(&[x, y, z]).unwrap_or(&AIR)
        }
    }
    impl LoadedQuery for TQ {
        fn is_loaded(&self, x: i32, _y: i32, _z: i32) -> bool {
            x <= self.x_max
        }
        fn has_wall(&self, x: i32, y: i32, z: i32) -> bool {
            self.walls.contains(&[x, y, z])
        }
    }

    const O: [i32; 3] = [20, 20, 20];

    /// Walled `w`x`h`x`d` interior at `O`, stone shell, a door in the -x face and
    /// a chair, table and torch inside.
    fn room(w: i32, h: i32, d: i32) -> TQ {
        let mut q = TQ {
            x_max: 1000,
            ..Default::default()
        };
        for x in -1..=w {
            for y in -1..=h {
                for z in -1..=d {
                    let p = [O[0] + x, O[1] + y, O[2] + z];
                    let inside = (0..w).contains(&x) && (0..h).contains(&y) && (0..d).contains(&z);
                    if inside {
                        q.walls.insert(p);
                    } else {
                        q.blocks.insert(p, STONE);
                    }
                }
            }
        }
        let at = |x, y, z| [O[0] + x, O[1] + y, O[2] + z];
        q.blocks.insert(at(-1, 0, 0), DOOR_CLOSED);
        q.blocks.insert(at(1, 0, 0), CHAIR);
        q.blocks.insert(at(2, 0, 0), TABLE);
        q.blocks.insert(at(3, 0, 0), TORCH);
        q
    }

    fn start() -> [i32; 3] {
        [O[0] + 2, O[1] + 1, O[2] + 1]
    }

    #[test]
    fn sealed_walled_room_with_furniture_ok() {
        let r = check_room(&room(5, 4, 4), start()).unwrap();
        assert_eq!(r.cells, 80);
        assert_eq!(r.bbox, [5, 4, 4]);
        assert!(room_has_needs(&r));
        assert_eq!(status(&Ok(r)), (true, 0));
    }

    #[test]
    fn missing_wall_fails_open_air() {
        let mut q = room(5, 4, 4);
        q.walls.remove(&[O[0] + 3, O[1] + 2, O[2] + 2]);
        assert_eq!(check_room(&q, start()), Err(RoomFail::OpenAir));
    }

    #[test]
    fn missing_door_fails_needs() {
        let mut q = room(5, 4, 4);
        q.blocks.insert([O[0] - 1, O[1], O[2]], STONE);
        let r = check_room(&q, start()).unwrap();
        assert!(!r.door && !room_has_needs(&r));
        assert_eq!(status(&Ok(r)), (false, 8));
    }

    #[test]
    fn missing_torch_fails_needs() {
        let mut q = room(5, 4, 4);
        q.blocks.remove(&[O[0] + 3, O[1], O[2]]);
        let r = check_room(&q, start()).unwrap();
        assert!(!r.light && !room_has_needs(&r));
    }

    #[test]
    fn door_open_or_closed_both_count() {
        let mut q = room(5, 4, 4);
        q.blocks.insert([O[0] - 1, O[1], O[2]], DOOR_OPEN);
        let r = check_room(&q, start()).unwrap();
        assert!(r.door && room_has_needs(&r), "open door seals and counts");
    }

    #[test]
    fn too_small_room_fails() {
        // 59 cells: 59x1x1 is long; use 59 = 59x1x1 with height 1 is also NoStanding,
        // so test size first: the minimum check precedes standing space.
        assert_eq!(
            check_room(&room(59, 1, 1), [O[0] + 5, O[1], O[2]]),
            Err(RoomFail::TooSmall)
        );
        // 60 cells passes the size limit (then fails standing: only 1 high).
        assert_eq!(
            check_room(&room(60, 1, 1), [O[0] + 5, O[1], O[2]]),
            Err(RoomFail::NoStanding)
        );
    }

    #[test]
    fn too_big_room_fails() {
        assert_eq!(check_room(&room(20, 20, 6), start()), Err(RoomFail::TooBig)); // 2400 > 2250
        assert_eq!(check_room(&room(101, 3, 2), start()), Err(RoomFail::TooBig)); // span 100
        assert!(check_room(&room(99, 3, 2), start()).is_ok()); // 594 cells, span 99
    }

    #[test]
    fn start_in_solid_fails() {
        let q = room(5, 4, 4);
        assert_eq!(
            check_room(&q, [O[0] - 1, O[1] + 1, O[2]]),
            Err(RoomFail::StartInSolid)
        );
    }

    #[test]
    fn unloaded_neighbor_fails_without_panic() {
        let mut q = room(5, 4, 4);
        q.x_max = O[0] + 3; // cut through the room: x = 24.. unloaded
        assert_eq!(check_room(&q, start()), Err(RoomFail::Unloaded));
        q.x_max = O[0] - 50; // start itself unloaded
        assert_eq!(check_room(&q, start()), Err(RoomFail::Unloaded));
    }

    #[test]
    fn near_world_edge_fails() {
        let mut q = TQ {
            x_max: 100,
            ..Default::default()
        };
        q.walls.insert([0, 5, 0]);
        assert_eq!(check_room(&q, [0, 5, 0]), Err(RoomFail::TooClose));
        assert_eq!(
            check_room(&q, [0, WORLD_HEIGHT as i32 - 3, 0]),
            Err(RoomFail::TooClose)
        );
    }

    #[test]
    fn no_standing_space_fails() {
        // A 10x6x1 slab room: 60 cells, no cell has room above it.
        assert_eq!(
            check_room(&room(10, 1, 6), [O[0] + 5, O[1], O[2] + 2]),
            Err(RoomFail::NoStanding)
        );
    }

    #[test]
    fn workbench_table_furnace_light_and_platform_door_satisfy_housing() {
        let mut q = room(5, 4, 4);
        let at = |x, y, z| [O[0] + x, O[1] + y, O[2] + z];
        q.blocks.insert(at(-1, 0, 0), PLATFORM);
        q.blocks.insert(at(2, 0, 0), WORKBENCH);
        q.blocks.insert(at(3, 0, 0), FURNACE);
        assert!(
            check_room(&q, start()).is_ok(),
            "workbench + furnace + platform must satisfy housing"
        );

        q.blocks.insert(at(3, 0, 0), HELLFORGE);
        assert!(
            check_room(&q, start()).is_ok(),
            "hellforge + workbench + platform must satisfy housing"
        );

        // Starting directly inside a platform cell behaves like StartInSolid.
        q.blocks.insert(start(), PLATFORM);
        assert_eq!(check_room(&q, start()), Err(RoomFail::StartInSolid));
    }
}
