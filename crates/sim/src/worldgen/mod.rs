//! World generation.

pub mod biome;
pub mod chest_gen;
pub mod corruption;
pub mod crystal;
pub mod dressing;
pub mod dungeon;
pub mod liquid;
pub mod mushroom;
pub mod mushroom_biome;
mod noise;
pub mod ore;
pub mod sky_island;
pub mod terrain;
pub mod tree;
pub mod underworld;

pub use dungeon::{dungeon_entrance_pos, is_dungeon_point};
pub use terrain::{
    DepthBand, WORLD_CHUNK_LAYERS, WORLD_HEIGHT, depth_band, generate_chunk, generate_chunk_sized,
};
