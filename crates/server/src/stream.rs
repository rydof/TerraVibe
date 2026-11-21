//! Per-connection chunk interest.

use std::collections::HashSet;

use terravibe_sim::net::{DESPAWN_MARGIN, VIEW_RADIUS_BLOCKS};
use terravibe_sim::{CHUNK_SIZE, ChunkPos};

/// Chunk layers (y chunk coordinates) the world occupies.
pub const LAYERS: std::ops::Range<i32> = 0..terravibe_sim::WORLD_CHUNK_LAYERS;

#[derive(Default)]
pub struct Interest {
    pub centre: Option<[f32; 3]>,
    /// Chunks already sent to this connection.
    pub sent: HashSet<ChunkPos>,
}

fn horizontal_dist(centre: [f32; 3], p: ChunkPos) -> f32 {
    let half = CHUNK_SIZE as f32 / 2.0;
    let cx = (p.x * CHUNK_SIZE) as f32 + half;
    let cz = (p.z * CHUNK_SIZE) as f32 + half;
    (cx - centre[0]).hypot(cz - centre[2])
}

/// Distance used for ordering: includes Y so the player's own layer comes first.
fn order_dist(centre: [f32; 3], p: ChunkPos) -> f32 {
    let cy = (p.y * CHUNK_SIZE) as f32 + CHUNK_SIZE as f32 / 2.0;
    horizontal_dist(centre, p).hypot(cy - centre[1])
}

impl Interest {
    /// Wanted chunks, nearest first.
    pub fn wanted(&self) -> Vec<ChunkPos> {
        let Some(c) = self.centre else {
            return Vec::new();
        };
        let cs = CHUNK_SIZE as f32;
        let r = (VIEW_RADIUS_BLOCKS / cs).ceil() as i32 + 1;
        let (ox, oz) = ((c[0] / cs).floor() as i32, (c[2] / cs).floor() as i32);
        let mut out = Vec::new();
        for x in ox - r..=ox + r {
            for z in oz - r..=oz + r {
                for y in terravibe_sim::net::vertical_layers_for_y(c[1]) {
                    let p = ChunkPos { x, y, z };
                    let d = horizontal_dist(c, p);
                    if d <= VIEW_RADIUS_BLOCKS {
                        out.push((order_dist(c, p), p));
                    }
                }
            }
        }
        out.sort_by(|a, b| a.0.total_cmp(&b.0));
        out.into_iter().map(|(_, p)| p).collect()
    }

    /// True if the chunk is beyond the horizontal or vertical unload distance.
    pub fn too_far(&self, p: ChunkPos) -> bool {
        match self.centre {
            Some(c) => {
                let cy = (c[1] / CHUNK_SIZE as f32).floor() as i32;
                horizontal_dist(c, p) > VIEW_RADIUS_BLOCKS + DESPAWN_MARGIN
                    || (p.y - cy).abs() > terravibe_sim::net::VERTICAL_CHUNK_RADIUS + 1
            }
            None => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_layer_is_streamed_before_other_layers() {
        let cs = CHUNK_SIZE as f32;
        // Player deep underground, in layer 2.
        let i = Interest {
            centre: Some([cs * 0.5, cs * 2.5, cs * 0.5]),
            ..Default::default()
        };
        let w = i.wanted();
        assert_eq!(w[0], ChunkPos { x: 0, y: 2, z: 0 });
        let c = i.centre.unwrap();
        assert!(
            w.windows(2)
                .all(|x| order_dist(c, x[0]) <= order_dist(c, x[1]))
        );
        // A neighbour in the same layer precedes the same column in other layers.
        let pos = |q: ChunkPos| w.iter().position(|p| *p == q).unwrap();
        assert!(pos(ChunkPos { x: 0, y: 2, z: 0 }) < pos(ChunkPos { x: 0, y: 0, z: 0 }));
        assert!(pos(ChunkPos { x: 1, y: 2, z: 0 }) < pos(ChunkPos { x: 1, y: 0, z: 0 }));
    }
}
