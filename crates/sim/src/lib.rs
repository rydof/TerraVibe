//! Simulation core: blocks, chunks, world storage, and world generation.

pub mod block;
pub mod boss;
pub mod character;
pub mod chest;
pub mod chunk;
pub mod codec;
pub mod coin;
pub mod combat;
pub mod crafting;
pub mod drop;
pub mod enemy;
pub mod facts;
pub mod housing;
pub mod interact;
pub mod inventory;
pub mod item;
pub mod liquid;
pub mod liquid_cell;
pub mod mana;
pub mod net;
pub mod player;
pub mod projectile;
pub mod shop;
pub mod slots;
pub mod spawn;
pub mod star;
pub mod time;
pub mod town_npc;
pub mod weather;
pub mod wof;
pub mod world_bounds;
pub mod worldgen;

pub use block::*;
pub use character::{Appearance, Character, Difficulty, HairStyle};
pub use chunk::{BlockEdit, BlockQuery, CHUNK_SIZE, Chunk, ChunkPos, World};
pub use coin::coin_unit_value;
pub use facts::{CharacterFacts, WorldFacts};
pub use liquid_cell::{Liquid, LiquidKind};
pub use mana::{CRYSTAL_MANA, MANA_CAP, ManaState, START_MANA_MAX};
pub use player::{
    DeathCause, PlayerBody, PlayerInput, StepEvents, step_body, step_body_bounded,
    tick_player_bounded,
};
pub use world_bounds::{BEACH_WIDTH, OCEAN_WIDTH, SEA_LEVEL_Y, WorldBounds, WorldSize};
pub use worldgen::{
    WORLD_CHUNK_LAYERS, WORLD_HEIGHT, dungeon_entrance_pos, generate_chunk, generate_chunk_sized,
    is_dungeon_point,
};
