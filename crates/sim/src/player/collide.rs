//! AABB-vs-voxel collision, one axis at a time.

use super::consts::{HALF_WIDTH, HEIGHT};
use crate::chunk::BlockQuery;

const EPS: f32 = 1e-5;
const MAX_SUBSTEP: f32 = 0.25;
const LO: [f32; 3] = [HALF_WIDTH, 0.0, HALF_WIDTH];
const HI: [f32; 3] = [HALF_WIDTH, HEIGHT, HALF_WIDTH];

/// True if the player box at `pos` (feet centre) overlaps a solid block.
pub fn overlaps(q: &impl BlockQuery, pos: [f32; 3]) -> bool {
    let r = |a: usize| {
        (
            (pos[a] - LO[a] + EPS).floor() as i32,
            (pos[a] + HI[a] - EPS).floor() as i32,
        )
    };
    let (x0, x1) = r(0);
    let (y0, y1) = r(1);
    let (z0, z1) = r(2);
    for x in x0..=x1 {
        for y in y0..=y1 {
            for z in z0..=z1 {
                if q.block(x, y, z).is_solid() {
                    return true;
                }
            }
        }
    }
    false
}

/// A one-way platform catches a falling body: some footprint cell at the new
/// feet height is a platform and the feet were at or above its top before the
/// step. Returns the platform top to land on. Terraria: `Collision.TileCollision`
/// (gameplay spec, `fallThrough` skips `TileID.Sets.Platforms`)
/// and the platform-top rule at gameplay spec/`:2083` (only feet above the
/// tile top collide).
fn platform_catch(q: &impl BlockQuery, from_y: f32, next: [f32; 3]) -> Option<f32> {
    let by = next[1].floor() as i32;
    let top = (by + 1) as f32;
    if from_y < top - EPS || next[1] >= top {
        return None;
    }
    let (x0, x1) = (
        (next[0] - HALF_WIDTH + EPS).floor() as i32,
        (next[0] + HALF_WIDTH - EPS).floor() as i32,
    );
    let (z0, z1) = (
        (next[2] - HALF_WIDTH + EPS).floor() as i32,
        (next[2] + HALF_WIDTH - EPS).floor() as i32,
    );
    (x0..=x1)
        .any(|x| (z0..=z1).any(|z| q.block(x, by, z).is_platform()))
        .then_some(top)
}

/// Move along `axis` by `delta` in sub-steps, stopping flush against solids.
/// Returns true if the move was blocked.
pub fn move_axis(q: &impl BlockQuery, pos: &mut [f32; 3], axis: usize, delta: f32) -> bool {
    move_axis_drop(q, pos, axis, delta, false)
}

/// [`move_axis`] where `drop` lets a falling body pass through platforms.
pub fn move_axis_drop(
    q: &impl BlockQuery,
    pos: &mut [f32; 3],
    axis: usize,
    delta: f32,
    drop: bool,
) -> bool {
    if delta == 0.0 {
        return false;
    }
    let n = (delta.abs() / MAX_SUBSTEP).ceil().max(1.0) as i32;
    let step = delta / n as f32;
    for _ in 0..n {
        let mut next = *pos;
        next[axis] += step;
        if axis == 1
            && step < 0.0
            && !drop
            && !overlaps(q, next)
            && let Some(top) = platform_catch(q, pos[1], next)
        {
            pos[1] = top;
            return true;
        }
        if overlaps(q, next) {
            if step > 0.0 {
                let e = next[axis] + HI[axis];
                pos[axis] = (e - EPS).floor() - HI[axis];
            } else {
                let e = next[axis] - LO[axis];
                pos[axis] = (e + EPS).floor() + 1.0 + LO[axis];
            }
            return true;
        }
        *pos = next;
    }
    false
}

/// Horizontal move along x or z with 1-block auto-step when `grounded`.
pub fn move_horizontal(
    q: &impl BlockQuery,
    pos: &mut [f32; 3],
    axis: usize,
    delta: f32,
    grounded: bool,
    drop: bool,
) -> bool {
    let before = *pos;
    if !move_axis(q, pos, axis, delta) {
        if grounded && !drop {
            step_onto_platform(q, pos);
        }
        return false;
    }
    if !grounded {
        return true;
    }
    let mut raised = before;
    raised[1] += 1.0;
    if overlaps(q, raised) {
        return true;
    }
    if !move_axis(q, &mut raised, axis, delta) && !overlaps(q, raised) {
        *pos = raised;
        return false;
    }
    true
}

/// Walking into a platform cell from the side lifts the feet onto its top
/// (Terraria lets players walk onto platforms like a one-tile step).
fn step_onto_platform(q: &impl BlockQuery, pos: &mut [f32; 3]) {
    let by = (pos[1] + EPS).floor() as i32;
    let top = (by + 1) as f32;
    if pos[1] >= top - EPS {
        return;
    }
    let (x0, x1) = (
        (pos[0] - HALF_WIDTH + EPS).floor() as i32,
        (pos[0] + HALF_WIDTH - EPS).floor() as i32,
    );
    let (z0, z1) = (
        (pos[2] - HALF_WIDTH + EPS).floor() as i32,
        (pos[2] + HALF_WIDTH - EPS).floor() as i32,
    );
    let on_platform = (x0..=x1).any(|x| (z0..=z1).any(|z| q.block(x, by, z).is_platform()));
    let raised = [pos[0], top, pos[2]];
    if on_platform && !overlaps(q, raised) {
        pos[1] = top;
    }
}
