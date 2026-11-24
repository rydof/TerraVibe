//! Original voxel art: `.vox` models, block textures and rigs authored by
//! `tools/art`, loaded from `assets/` at startup.

use std::path::PathBuf;

use terravibe_sim::{
    ANVIL, BOTTLE, BlockId, CAMPFIRE, CHAIR, CHEST, DEMON_ALTAR, DOOR_CLOSED, DOOR_OPEN, FURNACE,
    GLOWING_MUSHROOM_PLANT, HEART_LANTERN, HELLFORGE, LIFE_CRYSTAL, MOSS, MUSHROOM_PLANT, PLATFORM,
    POT, SHELF_MUSHROOM, SMALL_ROCK, STALACTITE, TABLE, TINKERERS_WORKSHOP, TORCH, WORKBENCH,
};

pub mod blocks;
pub mod chunk_mesh;
pub mod contact_sheet;
pub mod item_icons;
pub mod mesh_data;
pub mod rig;
pub mod rig_view;
#[allow(dead_code)]
pub mod sprite;
pub mod ui_icons;
pub mod vox;
pub mod voxmesh;

/// Env var overriding where the art lives.
const ENV_ART: &str = "TERRAVIBE_ART";

/// Directory holding `blocks/` and `models/`: `$TERRAVIBE_ART`, else the
/// repo's `assets/`.
pub fn art_dir() -> PathBuf {
    match std::env::var_os(ENV_ART) {
        Some(p) => PathBuf::from(p),
        None => PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets")),
    }
}

/// Path of the held-item voxel model for `item` when one is authored.
pub fn item_model(item: terravibe_sim::item::ItemId) -> Option<PathBuf> {
    let file = crate::view::avatar::item_model_file(item)?;
    let path = art_dir().join("models/items").join(file);
    path.is_file().then_some(path)
}

/// Blocks drawn as placed models instead of chunk cubes.
pub fn is_model_block(id: BlockId) -> bool {
    matches!(
        id,
        WORKBENCH
            | ANVIL
            | FURNACE
            | CHEST
            | DOOR_CLOSED
            | DOOR_OPEN
            | TABLE
            | CHAIR
            | PLATFORM
            | TORCH
            | LIFE_CRYSTAL
            | BOTTLE
            | MUSHROOM_PLANT
            | DEMON_ALTAR
            | HELLFORGE
            | POT
            | STALACTITE
            | MOSS
            | SMALL_ROCK
            | SHELF_MUSHROOM
            | HEART_LANTERN
            | GLOWING_MUSHROOM_PLANT
            | TINKERERS_WORKSHOP
            | CAMPFIRE
    )
}
