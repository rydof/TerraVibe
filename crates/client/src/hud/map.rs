//! Explored map, minimap overlays, compass strip, and map persistence.

use std::collections::HashMap;
use std::f32::consts::PI;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::ecs::system::SystemParam;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::ui::UiGlobalTransform;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use terravibe_server::save::atomic_write;
use terravibe_sim::block::BlockId;
use terravibe_sim::codec::{Reader, Writer};
use terravibe_sim::worldgen::corruption::corruption_at_for_size;
use terravibe_sim::worldgen::dungeon_entrance_pos;
use terravibe_sim::{CHUNK_SIZE, Chunk, WorldSize};

use crate::session::{AppState, SelectedWorld};
use crate::view::camera::FlyCamera;
use crate::view::chunks::ChunkStream;
use crate::view::enemies::{ClientEnemy, EnemyInterp};
use crate::view::player::LocalPlayer;

pub const MAP_MAGIC: &[u8; 4] = b"TVMP";
pub const MAP_VERSION: u8 = 1;
pub const MAP_CELL_SIZE: i32 = 4;
pub const EXPLORE_RADIUS: f32 = 48.0;
pub const FULL_MAP_TEX_W: u32 = 480;
pub const FULL_MAP_TEX_H: u32 = 270;
pub const OVERLAY_MAP_TEX_W: u32 = 320;
pub const OVERLAY_MAP_TEX_H: u32 = 180;
pub const FULL_ZOOM_DEFAULT: f32 = 4.5;
pub const OVERLAY_ZOOM: f32 = 4.5;
pub const OVERLAY_EXPLORED_ALPHA: u8 = 115;
pub const FULL_ZOOM_MIN: f32 = 1.0;
pub const FULL_ZOOM_MAX: f32 = 16.0;
pub const DEFAULT_WORLD_HALF_EXTENT: f32 = 1440.0;

/// Top-down explored map stored at 4-block resolution.
#[derive(Resource, Clone, Debug)]
pub struct ExploredMap {
    pub half_extent: i32,
    pub cells_across: usize,
    pub explored: Vec<bool>,
    pub colors: Vec<[u8; 3]>,
    pub dirty: bool,
}

impl ExploredMap {
    pub fn new(half_extent: i32) -> Self {
        let cells_across = ((half_extent * 2) / MAP_CELL_SIZE) as usize;
        let total = cells_across * cells_across;
        Self {
            half_extent,
            cells_across,
            explored: vec![false; total],
            colors: vec![[0, 0, 0]; total],
            dirty: false,
        }
    }

    #[inline]
    fn world_to_grid(&self, world_x: i32, world_z: i32) -> Option<(usize, usize)> {
        if world_x < -self.half_extent
            || world_x >= self.half_extent
            || world_z < -self.half_extent
            || world_z >= self.half_extent
        {
            return None;
        }
        let gx = (world_x + self.half_extent) / MAP_CELL_SIZE;
        let gz = (world_z + self.half_extent) / MAP_CELL_SIZE;
        if gx < 0 || gx >= self.cells_across as i32 || gz < 0 || gz >= self.cells_across as i32 {
            return None;
        }
        Some((gx as usize, gz as usize))
    }

    #[allow(dead_code)]
    pub fn set_cell(&mut self, cell_x: i32, cell_z: i32, explored: bool, color: [u8; 3]) {
        let world_x = cell_x * MAP_CELL_SIZE;
        let world_z = cell_z * MAP_CELL_SIZE;
        if let Some((gx, gz)) = self.world_to_grid(world_x, world_z) {
            let idx = gz * self.cells_across + gx;
            self.explored[idx] = explored;
            self.colors[idx] = color;
            self.dirty = true;
        }
    }

    pub fn is_explored(&self, world_x: i32, world_z: i32) -> bool {
        self.world_to_grid(world_x, world_z)
            .and_then(|(gx, gz)| {
                let idx = gz * self.cells_across + gx;
                self.explored.get(idx).copied()
            })
            .unwrap_or(false)
    }

    pub fn cell_color(&self, world_x: i32, world_z: i32) -> [u8; 3] {
        self.world_to_grid(world_x, world_z)
            .and_then(|(gx, gz)| {
                let idx = gz * self.cells_across + gx;
                self.colors.get(idx).copied()
            })
            .unwrap_or([0, 0, 0])
    }

    pub fn load_from(path: &Path, half_extent: i32) -> io::Result<Self> {
        let bytes = fs::read(path)?;
        if bytes.len() < 13 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "map file too small",
            ));
        }
        if &bytes[0..4] != MAP_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid map magic",
            ));
        }
        if bytes[4] != MAP_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsupported map version",
            ));
        }

        let mut r = Reader(&bytes[5..]);
        let saved_half = r
            .i32()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "corrupt half_extent"))?;
        let saved_across = r
            .u32()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "corrupt cells_across"))?
            as usize;

        let expected_across = ((half_extent * 2) / MAP_CELL_SIZE) as usize;
        if saved_half != half_extent || saved_across != expected_across {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "map bounds mismatch",
            ));
        }

        let total = expected_across * expected_across;
        let mask_len = (total + 7) / 8;
        let mask_bytes = r
            .take(mask_len)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "corrupt mask"))?;

        let mut explored = vec![false; total];
        for i in 0..total {
            let byte_idx = i / 8;
            let bit_idx = i % 8;
            explored[i] = (mask_bytes[byte_idx] & (1 << bit_idx)) != 0;
        }

        let mut colors = vec![[0, 0, 0]; total];
        for i in 0..total {
            if explored[i] {
                let rgb = r.take(3).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "corrupt color data")
                })?;
                colors[i] = [rgb[0], rgb[1], rgb[2]];
            }
        }

        Ok(Self {
            half_extent,
            cells_across: expected_across,
            explored,
            colors,
            dirty: false,
        })
    }

    pub fn save_to(&self, path: &Path) -> io::Result<()> {
        let total = self.cells_across * self.cells_across;
        let mask_len = (total + 7) / 8;
        let mut mask_bytes = vec![0u8; mask_len];

        for i in 0..total {
            if self.explored.get(i).copied().unwrap_or(false) {
                mask_bytes[i / 8] |= 1 << (i % 8);
            }
        }

        let mut w = Writer::default();
        w.bytes(MAP_MAGIC);
        w.u8(MAP_VERSION);
        w.i32(self.half_extent);
        w.u32(self.cells_across as u32);
        w.bytes(&mask_bytes);

        for i in 0..total {
            if self.explored.get(i).copied().unwrap_or(false) {
                let c = self.colors.get(i).copied().unwrap_or([0, 0, 0]);
                w.bytes(&c);
            }
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        atomic_write(path, &w.0)
    }

    #[cfg(test)]
    pub fn update_explored(&mut self, player_pos: [f32; 3], chunks: &ChunkStream) {
        let mut scratch = HashMap::new();
        self.update_explored_with_scratch(player_pos, chunks, &mut scratch);
    }

    pub fn update_explored_with_scratch(
        &mut self,
        player_pos: [f32; 3],
        chunks: &ChunkStream,
        col_chunks: &mut HashMap<(i32, i32), Vec<(i32, Arc<Chunk>)>>,
    ) {
        let px = player_pos[0];
        let pz = player_pos[2];
        let chunk_map = chunks.generated();
        if chunk_map.is_empty() {
            return;
        }

        let min_wx = ((px - EXPLORE_RADIUS).floor() as i32).max(-self.half_extent);
        let max_wx = ((px + EXPLORE_RADIUS).ceil() as i32).min(self.half_extent - 1);
        let min_wz = ((pz - EXPLORE_RADIUS).floor() as i32).max(-self.half_extent);
        let max_wz = ((pz + EXPLORE_RADIUS).ceil() as i32).min(self.half_extent - 1);

        let min_cx = min_wx.div_euclid(CHUNK_SIZE);
        let max_cx = max_wx.div_euclid(CHUNK_SIZE);
        let min_cz = min_wz.div_euclid(CHUNK_SIZE);
        let max_cz = max_wz.div_euclid(CHUNK_SIZE);

        // Reuse column scratch buffer, only indexing chunks near the player.
        for list in col_chunks.values_mut() {
            list.clear();
        }
        for (pos, chunk) in chunk_map {
            if pos.x >= min_cx && pos.x <= max_cx && pos.z >= min_cz && pos.z <= max_cz {
                col_chunks
                    .entry((pos.x, pos.z))
                    .or_default()
                    .push((pos.y, chunk.clone()));
            }
        }
        for list in col_chunks.values_mut() {
            if list.len() > 1 {
                list.sort_unstable_by_key(|(y, _)| std::cmp::Reverse(*y));
            }
        }

        let min_cell_x = min_wx.div_euclid(MAP_CELL_SIZE);
        let max_cell_x = max_wx.div_euclid(MAP_CELL_SIZE);
        let min_cell_z = min_wz.div_euclid(MAP_CELL_SIZE);
        let max_cell_z = max_wz.div_euclid(MAP_CELL_SIZE);

        let r_sq = EXPLORE_RADIUS * EXPLORE_RADIUS;

        for cell_z in min_cell_z..=max_cell_z {
            for cell_x in min_cell_x..=max_cell_x {
                let cell_min_x = cell_x * MAP_CELL_SIZE;
                let cell_min_z = cell_z * MAP_CELL_SIZE;

                // Distance from player to cell bounding box
                let nearest_x = px.clamp(cell_min_x as f32, (cell_min_x + MAP_CELL_SIZE) as f32);
                let nearest_z = pz.clamp(cell_min_z as f32, (cell_min_z + MAP_CELL_SIZE) as f32);
                let dist_sq =
                    (nearest_x - px) * (nearest_x - px) + (nearest_z - pz) * (nearest_z - pz);
                if dist_sq > r_sq {
                    continue;
                }

                // Check loaded chunks in this 4x4 cell
                let mut highest_y = i32::MIN;
                let mut highest_block = None;
                let mut has_loaded_chunk = false;

                for dx in 0..MAP_CELL_SIZE {
                    for dz in 0..MAP_CELL_SIZE {
                        let wx = cell_min_x + dx;
                        let wz = cell_min_z + dz;
                        let cx = wx.div_euclid(CHUNK_SIZE);
                        let cz = wz.div_euclid(CHUNK_SIZE);
                        let lx = wx.rem_euclid(CHUNK_SIZE) as usize;
                        let lz = wz.rem_euclid(CHUNK_SIZE) as usize;

                        if let Some(list) = col_chunks.get(&(cx, cz))
                            && !list.is_empty()
                        {
                            has_loaded_chunk = true;
                            for (cy, chunk) in list {
                                let base_y = *cy * CHUNK_SIZE;
                                if base_y + CHUNK_SIZE - 1 <= highest_y {
                                    break;
                                }
                                for ly in (0..CHUNK_SIZE as usize).rev() {
                                    let y = base_y + ly as i32;
                                    if y <= highest_y {
                                        break;
                                    }
                                    let b = chunk.get(lx, ly, lz);
                                    if b != terravibe_sim::block::AIR {
                                        highest_y = y;
                                        highest_block = Some(b);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }

                // Unvisited chunks remain hidden
                if !has_loaded_chunk {
                    continue;
                }

                let color = highest_block
                    .map(block_minimap_color)
                    .unwrap_or([20, 20, 30]);
                if let Some((gx, gz)) = self.world_to_grid(cell_min_x, cell_min_z) {
                    let idx = gz * self.cells_across + gx;
                    if !self.explored[idx] || self.colors[idx] != color {
                        self.explored[idx] = true;
                        self.colors[idx] = color;
                        self.dirty = true;
                    }
                }
            }
        }
    }
}

pub fn block_minimap_color(block: BlockId) -> [u8; 3] {
    match block {
        terravibe_sim::block::GRASS => [80, 180, 60],
        terravibe_sim::block::DIRT => [140, 90, 50],
        terravibe_sim::block::STONE => [128, 128, 128],
        terravibe_sim::block::IRON => [180, 140, 120],
        terravibe_sim::block::COPPER => [180, 100, 60],
        terravibe_sim::block::GOLD => [230, 200, 50],
        terravibe_sim::block::SILVER => [200, 200, 210],
        terravibe_sim::block::TORCH => [255, 200, 50],
        terravibe_sim::block::TRUNK | terravibe_sim::block::WOOD_BLOCK => [130, 90, 45],
        terravibe_sim::block::SAND => [220, 210, 130],
        terravibe_sim::block::MUD => [90, 60, 40],
        terravibe_sim::block::JUNGLE_GRASS => [110, 200, 30],
        terravibe_sim::block::CORRUPT_GRASS => [130, 80, 160],
        terravibe_sim::block::DEMONITE_ORE => [100, 70, 180],
        terravibe_sim::block::EBONSTONE => [75, 60, 90],
        terravibe_sim::block::DEMON_ALTAR => [140, 50, 160],
        terravibe_sim::block::SHADOW_ORB => [170, 40, 180],
        terravibe_sim::block::HELLSTONE => [180, 50, 30],
        terravibe_sim::block::SNOW => [240, 245, 250],
        terravibe_sim::block::ICE => [170, 210, 240],
        terravibe_sim::block::SANDSTONE | terravibe_sim::block::HARDENED_SAND => [190, 170, 100],
        terravibe_sim::block::WORKBENCH
        | terravibe_sim::block::TABLE
        | terravibe_sim::block::CHAIR
        | terravibe_sim::block::DOOR_CLOSED
        | terravibe_sim::block::DOOR_OPEN
        | terravibe_sim::block::PLATFORM => [150, 110, 60],
        terravibe_sim::block::ANVIL => [100, 100, 110],
        terravibe_sim::block::FURNACE => [130, 120, 110],
        terravibe_sim::block::CHEST => [190, 140, 40],
        terravibe_sim::block::LIFE_CRYSTAL => [240, 40, 80],
        terravibe_sim::block::MUSHROOM_PLANT => [60, 120, 230],
        terravibe_sim::block::BOTTLE => [200, 230, 240],
        terravibe_sim::block::GLASS_BLOCK => [210, 235, 245],
        terravibe_sim::block::OBSIDIAN => [40, 30, 60],
        terravibe_sim::block::BLUE_DUNGEON_BRICK => [60, 70, 180],
        terravibe_sim::block::GREEN_DUNGEON_BRICK => [60, 160, 80],
        terravibe_sim::block::PINK_DUNGEON_BRICK => [180, 80, 120],
        terravibe_sim::block::SPIKES => [150, 150, 160],
        terravibe_sim::block::ASH => [60, 60, 65],
        terravibe_sim::block::OBSIDIAN_BRICK => [50, 40, 70],
        terravibe_sim::block::HELLSTONE_BRICK => [190, 60, 40],
        terravibe_sim::block::HELLFORGE => [170, 80, 50],
        terravibe_sim::block::POT => [165, 105, 65],
        terravibe_sim::block::MOSS => [65, 145, 75],
        terravibe_sim::block::SHELF_MUSHROOM => [205, 145, 85],
        terravibe_sim::block::STALACTITE => [110, 110, 120],
        terravibe_sim::block::SMALL_ROCK => [145, 140, 135],
        _ => [128, 128, 128],
    }
}

/// Display modes for the HUD minimap / transparent overlay cycled by `Tab`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MapOverlayMode {
    #[default]
    Corner,
    FullscreenOverlay,
    Off,
}

impl MapOverlayMode {
    /// Cycles `Corner -> FullscreenOverlay -> Off -> Corner`.
    pub const fn next(self) -> Self {
        match self {
            Self::Corner => Self::FullscreenOverlay,
            Self::FullscreenOverlay => Self::Off,
            Self::Off => Self::Corner,
        }
    }
}

/// Map UI visibility and discovered world markers.
#[derive(Resource, Debug, Clone)]
pub struct MapUiState {
    pub overlay_mode: MapOverlayMode,
    pub fullscreen_visible: bool,
    #[allow(dead_code)]
    pub spawn_marker: [f32; 2],
    pub dungeon_marker: Option<[f32; 2]>,
    pub corruption_marker: Option<[f32; 2]>,
    pub underworld_marker: Option<[f32; 2]>,
    pub full_zoom: f32,
    pub full_center: [f32; 2],
    pub full_dragging: bool,
    pub last_cursor_pos: Option<Vec2>,
    pub cursor_world: Option<[i32; 2]>,
}

impl Default for MapUiState {
    fn default() -> Self {
        Self {
            overlay_mode: MapOverlayMode::Corner,
            fullscreen_visible: false,
            spawn_marker: [0.0, 0.0],
            dungeon_marker: None,
            corruption_marker: None,
            underworld_marker: None,
            full_zoom: FULL_ZOOM_DEFAULT,
            full_center: [0.0, 0.0],
            full_dragging: false,
            last_cursor_pos: None,
            cursor_world: None,
        }
    }
}

/// Returns the half-span (in world blocks) visible in the fullscreen map at `zoom`.
#[inline]
pub fn full_view_half_span(half_extent: f32, zoom: f32) -> f32 {
    let z = zoom.clamp(FULL_ZOOM_MIN, FULL_ZOOM_MAX);
    half_extent.max(1.0) / z
}

/// Clamps fullscreen map center and zoom so the viewport never pans outside `[-half_extent, half_extent]`.
pub fn clamp_full_view(center: [f32; 2], zoom: f32, half_extent: f32) -> ([f32; 2], f32) {
    let z = zoom.clamp(FULL_ZOOM_MIN, FULL_ZOOM_MAX);
    let max_offset = (half_extent - full_view_half_span(half_extent, z)).max(0.0);
    (
        [
            center[0].clamp(-max_offset, max_offset),
            center[1].clamp(-max_offset, max_offset),
        ],
        z,
    )
}

/// Maps a continuous texture coordinate `(fx, fy)` in `[0, tex_w] x [0, tex_h]` to world `(x, z)`.
pub fn pixel_to_world(
    fx: f32,
    fy: f32,
    tex_w: f32,
    tex_h: f32,
    center: [f32; 2],
    zoom: f32,
    half_extent: f32,
) -> [f32; 2] {
    let span_z = 2.0 * full_view_half_span(half_extent, zoom);
    let aspect = tex_w.max(1.0) / tex_h.max(1.0);
    let span_x = span_z * aspect;
    let u = fx / tex_w.max(1.0) - 0.5;
    let v = fy / tex_h.max(1.0) - 0.5;
    [center[0] + u * span_x, center[1] + v * span_z]
}

/// Maps world `(wx, wz)` to continuous texture coordinates `(fx, fy)` in `[0, tex_w] x [0, tex_h]`.
pub fn world_to_pixel(
    wx: f32,
    wz: f32,
    tex_w: f32,
    tex_h: f32,
    center: [f32; 2],
    zoom: f32,
    half_extent: f32,
) -> [f32; 2] {
    let span_z = 2.0 * full_view_half_span(half_extent, zoom);
    let aspect = tex_w.max(1.0) / tex_h.max(1.0);
    let span_x = span_z * aspect;
    let u = (wx - center[0]) / span_x + 0.5;
    let v = (wz - center[1]) / span_z + 0.5;
    [u * tex_w, v * tex_h]
}

/// Converts a window cursor position into normalized `[0.0, 1.0]` UV coordinates inside a UI rect.
pub fn cursor_to_map_uv(cursor: Vec2, rect_center: Vec2, rect_size: Vec2) -> Option<[f32; 2]> {
    if rect_size.x <= 0.0 || rect_size.y <= 0.0 {
        return None;
    }
    let top_left = rect_center - rect_size * 0.5;
    let rel = (cursor - top_left) / rect_size;
    if (0.0..=1.0).contains(&rel.x) && (0.0..=1.0).contains(&rel.y) {
        Some([rel.x, rel.y])
    } else {
        None
    }
}

/// Adjusts `center` when zooming from `old_zoom` to `new_zoom` so the world point at `uv` stays fixed.
pub fn zoom_around_uv(
    center: [f32; 2],
    old_zoom: f32,
    new_zoom: f32,
    uv: [f32; 2],
    tex_w: f32,
    tex_h: f32,
    half_extent: f32,
) -> ([f32; 2], f32) {
    let clamped_new = new_zoom.clamp(FULL_ZOOM_MIN, FULL_ZOOM_MAX);
    let u = uv[0] - 0.5;
    let v = uv[1] - 0.5;
    let aspect = tex_w.max(1.0) / tex_h.max(1.0);
    let old_span_z = 2.0 * full_view_half_span(half_extent, old_zoom);
    let old_span_x = old_span_z * aspect;
    let new_span_z = 2.0 * full_view_half_span(half_extent, clamped_new);
    let new_span_x = new_span_z * aspect;
    let wx = center[0] + u * old_span_x;
    let wz = center[1] + v * old_span_z;
    let new_center = [wx - u * new_span_x, wz - v * new_span_z];
    clamp_full_view(new_center, clamped_new, half_extent)
}

impl MapUiState {
    pub fn update_markers(&mut self, player_pos: [f32; 3], seed: u32, size: WorldSize) {
        let px = player_pos[0];
        let py = player_pos[1];
        let pz = player_pos[2];

        // Dungeon entrance marker: within 64 blocks of entrance
        if self.dungeon_marker.is_none() {
            let entrance = dungeon_entrance_pos(seed, size);
            let dx = entrance[0] - px;
            let dz = entrance[2] - pz;
            if dx * dx + dz * dz <= 64.0 * 64.0 {
                self.dungeon_marker = Some([entrance[0], entrance[2]]);
            }
        }

        // Corruption marker: within 64 blocks of corruption column
        if self.corruption_marker.is_none() {
            let wx = px.round() as i32;
            let wz = pz.round() as i32;
            if corruption_at_for_size(seed, wx, wz, size) > 0.0 {
                self.corruption_marker = Some([px, pz]);
            }
        }

        // Underworld marker: player descended to y <= 24
        if self.underworld_marker.is_none() && py <= 24.0 {
            self.underworld_marker = Some([px, pz]);
        }
    }
}

/// Computes the compass angle (radians in [-pi, pi]) from the camera forward direction.
pub fn heading_angle(forward: Vec3) -> f32 {
    let dir = Vec2::new(forward.x, -forward.z);
    dir.x.atan2(dir.y)
}

/// Computes horizontal offset in pixels for a marker with angle `marker_rad` on a strip of `strip_width`.
pub fn compass_offset(heading: f32, marker_rad: f32, strip_width: f32) -> Option<f32> {
    let mut diff = (marker_rad - heading).rem_euclid(2.0 * PI);
    if diff > PI {
        diff -= 2.0 * PI;
    }
    let fov = PI; // 180 degrees field on strip
    if diff.abs() <= fov / 2.0 {
        Some(diff / (fov / 2.0) * (strip_width / 2.0))
    } else {
        None
    }
}

pub const COMPASS_DIRECTIONS: [(&str, f32); 8] = [
    ("N", 0.0),
    ("NE", PI / 4.0),
    ("E", PI / 2.0),
    ("SE", 3.0 * PI / 4.0),
    ("S", PI),
    ("SW", -3.0 * PI / 4.0),
    ("W", -PI / 2.0),
    ("NW", -PI / 4.0),
];

#[derive(Component)]
pub struct CompassStripRoot;

#[derive(Component)]
pub struct CompassMarkerLabel(pub usize);

#[derive(Component)]
pub struct CornerMinimapRoot;

#[derive(Component)]
pub struct CornerMinimapImage;

#[derive(Component)]
pub struct MinimapInfoPill;

#[derive(Component)]
pub struct MinimapInfoText;

#[derive(Component)]
pub struct TransparentOverlayRoot;

#[derive(Component)]
pub struct FullscreenMapRoot;

#[derive(Component)]
pub struct FullscreenMapImage;

#[derive(Component)]
pub struct FullscreenMapTitleText;

#[derive(Component)]
pub struct FullscreenMapStatsText;

#[derive(Resource)]
pub struct MinimapAssets {
    pub corner_handle: Handle<Image>,
    pub overlay_handle: Handle<Image>,
    pub full_handle: Handle<Image>,
}

#[derive(SystemParam)]
pub struct FullscreenMapInputParams<'w, 's> {
    mouse: Option<Res<'w, ButtonInput<MouseButton>>>,
    wheel: Option<MessageReader<'w, 's, MouseWheel>>,
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    cursor_opts: Query<'w, 's, &'static mut CursorOptions, With<PrimaryWindow>>,
    full_img_q: Query<
        'w,
        's,
        (&'static ComputedNode, &'static UiGlobalTransform),
        With<FullscreenMapImage>,
    >,
    map: Option<Res<'w, ExploredMap>>,
    player: Option<Res<'w, LocalPlayer>>,
    selected_world: Option<Res<'w, SelectedWorld>>,
    time: Option<Res<'w, Time>>,
    game_menu: Option<Res<'w, crate::session::GameMenu>>,
}

fn world_half_extent_from(
    map: Option<&ExploredMap>,
    selected_world: Option<&SelectedWorld>,
) -> f32 {
    if let Some(m) = map {
        m.half_extent as f32
    } else if let Some(w) = selected_world {
        w.size.half_extent() as f32
    } else {
        DEFAULT_WORLD_HALF_EXTENT
    }
}

pub fn handle_map_input(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut map_ui: ResMut<MapUiState>,
    settings: Option<Res<crate::settings::Settings>>,
    mut params: FullscreenMapInputParams,
) {
    let bindings = settings
        .as_deref()
        .map_or_else(crate::input_map::InputBindings::default, |s| s.bindings);
    let minimap_pressed = keys.just_pressed(bindings.key(crate::input_map::GameAction::Minimap))
        || gamepads
            .iter()
            .any(|g| g.just_pressed(GamepadButton::DPadUp));
    if minimap_pressed {
        map_ui.overlay_mode = map_ui.overlay_mode.next();
    }

    let was_open = map_ui.fullscreen_visible;
    let fullmap_pressed = keys
        .just_pressed(bindings.key(crate::input_map::GameAction::FullscreenMap))
        || gamepads
            .iter()
            .any(|g| g.just_pressed(GamepadButton::Select));
    let mut closed_by_esc = false;
    if fullmap_pressed {
        map_ui.fullscreen_visible = !map_ui.fullscreen_visible;
    } else if map_ui.fullscreen_visible && keys.just_pressed(KeyCode::Escape) {
        map_ui.fullscreen_visible = false;
        closed_by_esc = true;
    }

    let half_extent =
        world_half_extent_from(params.map.as_deref(), params.selected_world.as_deref());
    let player_xz = params
        .player
        .as_deref()
        .map(|p| [p.0.body.pos[0], p.0.body.pos[2]]);

    // Handle open/close transitions (reset drag, center on player, cursor grab).
    if !was_open && map_ui.fullscreen_visible {
        map_ui.full_dragging = false;
        map_ui.last_cursor_pos = None;
        if let Some(p_xz) = player_xz {
            let (c, z) = clamp_full_view(p_xz, map_ui.full_zoom, half_extent);
            map_ui.full_center = c;
            map_ui.full_zoom = z;
        }
        if let Ok(mut opts) = params.cursor_opts.single_mut() {
            opts.grab_mode = CursorGrabMode::None;
            opts.visible = true;
        }
    } else if was_open && !map_ui.fullscreen_visible {
        map_ui.full_dragging = false;
        map_ui.last_cursor_pos = None;
        map_ui.cursor_world = None;
        let menu_open = params
            .game_menu
            .as_deref()
            .is_some_and(|gm| gm.panel.is_some());
        if !closed_by_esc
            && !menu_open
            && let Ok(mut opts) = params.cursor_opts.single_mut()
        {
            opts.grab_mode = CursorGrabMode::Locked;
            opts.visible = false;
        }
    }

    if !map_ui.fullscreen_visible {
        if let Some(ref mut wheel) = params.wheel {
            wheel.clear();
        }
        return;
    }

    // Re-center on player with `R`.
    if keys.just_pressed(KeyCode::KeyR)
        && let Some(p_xz) = player_xz
    {
        let (c, z) = clamp_full_view(p_xz, map_ui.full_zoom, half_extent);
        map_ui.full_center = c;
        map_ui.full_zoom = z;
    }

    // Cursor position & UV over the fullscreen map image.
    let cursor_pos = params
        .windows
        .single()
        .ok()
        .and_then(|w| w.cursor_position());
    let img_rect = params
        .full_img_q
        .single()
        .ok()
        .map(|(node, tf)| (tf.translation, node.size()));
    let cursor_uv = match (cursor_pos, img_rect) {
        (Some(c), Some((center, size))) => cursor_to_map_uv(c, center, size),
        _ => None,
    };

    // Scroll-wheel zoom (1.0x..16.0x), zooming toward the cursor when over the map.
    if let Some(ref mut wheel) = params.wheel {
        let mut scroll_steps = 0.0f32;
        for ev in wheel.read() {
            scroll_steps += match ev.unit {
                MouseScrollUnit::Line => ev.y,
                MouseScrollUnit::Pixel => ev.y / 60.0,
            };
        }
        if scroll_steps.abs() > f32::EPSILON {
            let old_zoom = map_ui.full_zoom;
            let new_zoom =
                (old_zoom * 1.18f32.powf(scroll_steps)).clamp(FULL_ZOOM_MIN, FULL_ZOOM_MAX);
            let uv = cursor_uv.unwrap_or([0.5, 0.5]);
            let (c, z) = zoom_around_uv(
                map_ui.full_center,
                old_zoom,
                new_zoom,
                uv,
                FULL_MAP_TEX_W as f32,
                FULL_MAP_TEX_H as f32,
                half_extent,
            );
            map_ui.full_center = c;
            map_ui.full_zoom = z;
        }
    }

    // Left-click drag panning.
    if let Some(ref mouse) = params.mouse {
        if mouse.just_pressed(MouseButton::Left) && cursor_pos.is_some() {
            if img_rect.is_none() || cursor_uv.is_some() {
                map_ui.full_dragging = true;
                map_ui.last_cursor_pos = cursor_pos;
            }
        } else if mouse.pressed(MouseButton::Left) && map_ui.full_dragging {
            if let (Some(curr), Some(prev)) = (cursor_pos, map_ui.last_cursor_pos) {
                let delta = curr - prev;
                let (img_w, img_h) = img_rect
                    .map_or((FULL_MAP_TEX_W as f32, FULL_MAP_TEX_H as f32), |(_, s)| {
                        (s.x.max(1.0), s.y.max(1.0))
                    });
                let span_z = 2.0 * full_view_half_span(half_extent, map_ui.full_zoom);
                let span_x = span_z * (FULL_MAP_TEX_W as f32 / FULL_MAP_TEX_H as f32);
                let next_center = [
                    map_ui.full_center[0] - (delta.x / img_w) * span_x,
                    map_ui.full_center[1] - (delta.y / img_h) * span_z,
                ];
                let (c, z) = clamp_full_view(next_center, map_ui.full_zoom, half_extent);
                map_ui.full_center = c;
                map_ui.full_zoom = z;
                map_ui.last_cursor_pos = Some(curr);
            }
        } else {
            map_ui.full_dragging = false;
            map_ui.last_cursor_pos = None;
        }
    }

    // WASD / Arrow panning.
    let mut pan_dir = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        pan_dir.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        pan_dir.x += 1.0;
    }
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        pan_dir.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        pan_dir.y += 1.0;
    }
    if pan_dir != Vec2::ZERO {
        let dt = params
            .time
            .as_deref()
            .map_or(1.0 / 60.0, |t| t.delta_secs());
        let dir = pan_dir.normalize();
        let speed = full_view_half_span(half_extent, map_ui.full_zoom) * 1.25;
        let next_center = [
            map_ui.full_center[0] + dir.x * speed * dt,
            map_ui.full_center[1] + dir.y * speed * dt,
        ];
        let (c, z) = clamp_full_view(next_center, map_ui.full_zoom, half_extent);
        map_ui.full_center = c;
        map_ui.full_zoom = z;
    }

    // Update hovered world (X, Z) coordinates.
    map_ui.cursor_world = cursor_uv.map(|uv| {
        let tex_w = FULL_MAP_TEX_W as f32;
        let tex_h = FULL_MAP_TEX_H as f32;
        let [wx, wz] = pixel_to_world(
            uv[0] * tex_w,
            uv[1] * tex_h,
            tex_w,
            tex_h,
            map_ui.full_center,
            map_ui.full_zoom,
            half_extent,
        );
        [wx.round() as i32, wz.round() as i32]
    });
}

pub fn setup_map_ui(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let corner_img = Image::new_fill(
        Extent3d {
            width: 100,
            height: 100,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[15, 15, 20, 240],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    let overlay_img = Image::new_fill(
        Extent3d {
            width: OVERLAY_MAP_TEX_W,
            height: OVERLAY_MAP_TEX_H,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    let full_img = Image::new_fill(
        Extent3d {
            width: FULL_MAP_TEX_W,
            height: FULL_MAP_TEX_H,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[8, 12, 22, 240],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );

    let corner_handle = images.add(corner_img);
    let overlay_handle = images.add(overlay_img);
    let full_handle = images.add(full_img);
    commands.insert_resource(MinimapAssets {
        corner_handle: corner_handle.clone(),
        overlay_handle: overlay_handle.clone(),
        full_handle: full_handle.clone(),
    });

    // Compass strip at top center
    commands
        .spawn((
            CompassStripRoot,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                left: Val::Percent(50.0),
                width: Val::Px(240.0),
                height: Val::Px(26.0),
                margin: UiRect {
                    left: Val::Px(-120.0),
                    ..default()
                },
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BTN)),
                overflow: Overflow::clip(),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BorderColor::all(super::style::GOLD_BORDER),
            BackgroundColor(super::style::PANEL_BG),
            GlobalZIndex(30),
            DespawnOnExit(AppState::InGame),
        ))
        .with_children(|parent| {
            // Center needle / tick mark
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(119.0),
                    top: Val::Px(0.0),
                    width: Val::Px(2.0),
                    height: Val::Px(24.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 0.85, 0.2, 0.9)),
            ));

            for (i, (dir_text, _)) in COMPASS_DIRECTIONS.iter().enumerate() {
                parent.spawn((
                    CompassMarkerLabel(i),
                    Text::new(*dir_text),
                    TextFont {
                        font_size: FontSize::Px(11.0),
                        ..default()
                    },
                    TextColor(if *dir_text == "N" {
                        Color::srgb(1.0, 0.3, 0.3)
                    } else {
                        Color::srgb(0.9, 0.9, 0.9)
                    }),
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(5.0),
                        left: Val::Px(120.0),
                        ..default()
                    },
                ));
            }
        });

    // Corner minimap at top right (below hearts/stars, clear of armor column)
    commands
        .spawn((
            CornerMinimapRoot,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(104.0),
                right: Val::Px(56.0),
                width: Val::Px(168.0),
                height: Val::Px(168.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_PANEL)),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(super::style::GOLD_BORDER),
            BackgroundColor(super::style::PANEL_BG_SOLID),
            GlobalZIndex(25),
            DespawnOnExit(AppState::InGame),
        ))
        .with_children(|parent| {
            parent.spawn((
                CornerMinimapImage,
                ImageNode::new(corner_handle),
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
            ));
            // Player center marker dot (168 / 2 - 3 = 81)
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(81.0),
                    top: Val::Px(81.0),
                    width: Val::Px(6.0),
                    height: Val::Px(6.0),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BorderColor::all(Color::BLACK),
                BackgroundColor(Color::srgb(1.0, 0.9, 0.2)),
            ));
        });

    // Biome · Depth · Time info pill beneath the corner minimap
    commands
        .spawn((
            MinimapInfoPill,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(276.0),
                right: Val::Px(56.0),
                min_width: Val::Px(168.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.5)),
                border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BTN)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(super::style::PANEL_BORDER),
            BackgroundColor(super::style::PANEL_BG_SOLID),
            GlobalZIndex(25),
            DespawnOnExit(AppState::InGame),
        ))
        .with_children(|parent| {
            parent.spawn((
                MinimapInfoText,
                Node::default(),
                super::text::StackText,
                super::text::HudText {
                    text: "Forest · Surface · Day".to_string(),
                    death: false,
                },
            ));
        });

    // Terraria-style fullscreen transparent map overlay (non-interactive, Tab 3-mode cycle)
    commands.spawn((
        TransparentOverlayRoot,
        ImageNode::new(overlay_handle),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            display: Display::None,
            ..default()
        },
        GlobalZIndex(20),
        DespawnOnExit(AppState::InGame),
    ));

    // Fullscreen Terraria-style edge-to-edge world map (M)
    commands
        .spawn((
            FullscreenMapRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(8.0 / 255.0, 12.0 / 255.0, 22.0 / 255.0, 0.96)),
            GlobalZIndex(1150),
            DespawnOnExit(AppState::InGame),
        ))
        .with_children(|parent| {
            // Edge-to-edge 16:9 widescreen map canvas
            parent.spawn((
                FullscreenMapImage,
                ImageNode::new(full_handle),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
            ));

            // Floating header bar above the map canvas: World Name, Zoom %, Cursor (X, Z)
            parent
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(14.0),
                        left: Val::Px(24.0),
                        right: Val::Px(24.0),
                        padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BTN)),
                        ..default()
                    },
                    BorderColor::all(super::style::GOLD_BORDER),
                    BackgroundColor(super::style::PANEL_BG_SOLID),
                    GlobalZIndex(1152),
                ))
                .with_children(|hdr| {
                    hdr.spawn((
                        FullscreenMapTitleText,
                        Node::default(),
                        super::text::StackText,
                        super::text::HudTextColor(super::style::GOLD_TEXT),
                        super::text::HudText {
                            text: "World Map".to_string(),
                            death: false,
                        },
                    ));
                    hdr.spawn((
                        FullscreenMapStatsText,
                        Node::default(),
                        super::text::StackText,
                        super::text::HudTextColor(Color::WHITE),
                        super::text::HudText {
                            text: "Zoom: 450%  ·  Cursor: (---, ---)".to_string(),
                            death: false,
                        },
                    ));
                });

            // Floating footer bar above the map canvas: controls legend
            parent
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        bottom: Val::Px(14.0),
                        left: Val::Px(0.0),
                        width: Val::Percent(100.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    GlobalZIndex(1152),
                ))
                .with_children(|wrap| {
                    wrap.spawn((
                        Node {
                            padding: UiRect::axes(Val::Px(16.0), Val::Px(6.0)),
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BTN)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BorderColor::all(super::style::PANEL_BORDER),
                        BackgroundColor(super::style::PANEL_BG_SOLID),
                    ))
                    .with_children(|ftr| {
                        ftr.spawn((
                            Node::default(),
                            super::text::StackText,
                            super::text::HudTextColor(Color::srgb(0.86, 0.90, 0.98)),
                            super::text::HudText {
                                text: "Scroll: Zoom · Drag/WASD: Pan · R: Center · M/Esc: Close"
                                    .to_string(),
                                death: false,
                            },
                        ));
                    });
                });
        });
}

#[derive(Default)]
pub struct ExploreThrottle {
    last_pos: Option<[f32; 3]>,
    last_chunk_count: usize,
    timer: f32,
    scratch: HashMap<(i32, i32), Vec<(i32, Arc<Chunk>)>>,
}

#[allow(clippy::too_many_arguments)]
pub fn update_map_exploration(
    player: Option<Res<LocalPlayer>>,
    chunks: Option<Res<ChunkStream>>,
    mut map: Option<ResMut<ExploredMap>>,
    mut ui_state: ResMut<MapUiState>,
    selected_world: Option<Res<SelectedWorld>>,
    time: Option<Res<Time>>,
    mut throttle: Local<ExploreThrottle>,
) {
    let (Some(player), Some(chunks), Some(ref mut map)) = (player, chunks, map.as_deref_mut())
    else {
        return;
    };
    let pos = player.0.body.pos;
    let chunk_count = chunks.generated().len();
    let moved_far = throttle.last_pos.is_none_or(|lp| {
        let dx = pos[0] - lp[0];
        let dy = pos[1] - lp[1];
        let dz = pos[2] - lp[2];
        dx * dx + dy * dy + dz * dz >= 16.0
    });
    let chunks_changed = chunk_count != throttle.last_chunk_count;
    throttle.timer += time.as_ref().map_or(0.25, |t| t.delta_secs());

    if !moved_far && !(chunks_changed && throttle.timer >= 0.25) {
        return;
    }
    throttle.last_pos = Some(pos);
    throttle.last_chunk_count = chunk_count;
    throttle.timer = 0.0;

    map.update_explored_with_scratch(pos, &chunks, &mut throttle.scratch);

    let seed = selected_world
        .as_ref()
        .map_or(crate::view::SEED, |w| w.seed);
    let size = selected_world.as_ref().map_or(WorldSize::Small, |w| w.size);
    let mut probe = ui_state.clone();
    probe.update_markers(pos, seed, size);
    if probe.dungeon_marker != ui_state.dungeon_marker
        || probe.corruption_marker != ui_state.corruption_marker
        || probe.underworld_marker != ui_state.underworld_marker
    {
        ui_state.dungeon_marker = probe.dungeon_marker;
        ui_state.corruption_marker = probe.corruption_marker;
        ui_state.underworld_marker = probe.underworld_marker;
    }
}

pub fn update_compass(
    cam_q: Query<(&FlyCamera, &Transform)>,
    mut label_q: Query<(&CompassMarkerLabel, &mut Node)>,
) {
    let Ok((_, tf)) = cam_q.single() else {
        return;
    };
    let heading = heading_angle(tf.forward().into());
    let strip_width = 240.0;

    for (label, mut node) in label_q.iter_mut() {
        let (_, marker_rad) = COMPASS_DIRECTIONS[label.0];
        if let Some(offset) = compass_offset(heading, marker_rad, strip_width) {
            node.display = Display::Flex;
            node.left = Val::Px((strip_width / 2.0) + offset - 6.0);
        } else {
            node.display = Display::None;
        }
    }
}

#[derive(Default)]
pub struct MinimapRenderCache {
    corner_cell: Option<(i32, i32)>,
    overlay_cell: Option<(i32, i32)>,
    overlay_enemy_sig: Option<u64>,
    full_cell: Option<(i32, i32)>,
    full_zoom_bits: Option<u32>,
    full_center_bits: Option<[u32; 2]>,
    full_enemy_sig: Option<u64>,
    corner_vis: Option<bool>,
    overlay_vis: Option<bool>,
    full_vis: Option<bool>,
}

#[derive(SystemParam)]
pub struct FullscreenMapRenderParams<'w, 's> {
    enemies: Query<
        'w,
        's,
        (
            &'static ClientEnemy,
            Option<&'static EnemyInterp>,
            Option<&'static Transform>,
        ),
    >,
    title_q: Query<
        'w,
        's,
        &'static mut super::text::HudText,
        (
            With<FullscreenMapTitleText>,
            Without<MinimapInfoText>,
            Without<FullscreenMapStatsText>,
        ),
    >,
    stats_q: Query<
        'w,
        's,
        &'static mut super::text::HudText,
        (
            With<FullscreenMapStatsText>,
            Without<MinimapInfoText>,
            Without<FullscreenMapTitleText>,
        ),
    >,
}

#[inline]
fn set_rgba_pixel(data: &mut [u8], tex_w: i32, tex_h: i32, x: i32, y: i32, rgba: [u8; 4]) {
    if x < 0 || x >= tex_w || y < 0 || y >= tex_h {
        return;
    }
    let idx = ((y * tex_w + x) * 4) as usize;
    if idx + 3 < data.len() {
        data[idx] = rgba[0];
        data[idx + 1] = rgba[1];
        data[idx + 2] = rgba[2];
        data[idx + 3] = rgba[3];
    }
}

/// Draws a crisp diamond marker with a 1px dark outline onto an RGBA texture.
#[allow(clippy::too_many_arguments)]
fn draw_map_diamond(
    data: &mut [u8],
    tex_w: i32,
    tex_h: i32,
    cx: i32,
    cy: i32,
    radius: i32,
    fill: [u8; 3],
    outline: [u8; 3],
) {
    for dy in -(radius + 1)..=(radius + 1) {
        for dx in -(radius + 1)..=(radius + 1) {
            let manhattan = dx.abs() + dy.abs();
            if manhattan <= radius {
                set_rgba_pixel(
                    data,
                    tex_w,
                    tex_h,
                    cx + dx,
                    cy + dy,
                    [fill[0], fill[1], fill[2], 255],
                );
            } else if manhattan == radius + 1 {
                set_rgba_pixel(
                    data,
                    tex_w,
                    tex_h,
                    cx + dx,
                    cy + dy,
                    [outline[0], outline[1], outline[2], 255],
                );
            }
        }
    }
}

/// Draws a crisp player crosshair/diamond marker onto an RGBA texture.
fn draw_player_marker(data: &mut [u8], tex_w: i32, tex_h: i32, cx: i32, cy: i32) {
    draw_map_diamond(data, tex_w, tex_h, cx, cy, 4, [255, 235, 50], [10, 12, 20]);
    for d in -6i32..=6 {
        if d.abs() >= 4 {
            set_rgba_pixel(data, tex_w, tex_h, cx + d, cy, [255, 245, 140, 255]);
            set_rgba_pixel(data, tex_w, tex_h, cx, cy + d, [255, 245, 140, 255]);
        }
    }
}

struct MapTextureStyle {
    tex_w: u32,
    tex_h: u32,
    center: [f32; 2],
    zoom: f32,
    unexplored_rgba: [u8; 4],
    out_of_bounds_rgba: [u8; 4],
    explored_alpha: u8,
}

fn render_world_map_texture(
    img: &mut Image,
    map: &ExploredMap,
    ui_state: &MapUiState,
    player_xz: [f32; 2],
    enemies: &Query<(&ClientEnemy, Option<&EnemyInterp>, Option<&Transform>)>,
    style: MapTextureStyle,
) {
    let tex_w = style.tex_w as i32;
    let tex_h = style.tex_h as i32;
    let tex_wf = style.tex_w as f32;
    let tex_hf = style.tex_h as f32;
    let half_extent = map.half_extent as f32;
    let center = style.center;
    let zoom = style.zoom.clamp(FULL_ZOOM_MIN, FULL_ZOOM_MAX);

    let Some(ref mut data) = img.data else {
        return;
    };

    for fy in 0..tex_h {
        for fx in 0..tex_w {
            let [wx, wz] = pixel_to_world(
                fx as f32 + 0.5,
                fy as f32 + 0.5,
                tex_wf,
                tex_hf,
                center,
                zoom,
                half_extent,
            );
            let ix = wx.floor() as i32;
            let iz = wz.floor() as i32;
            let rgba = if ix < -map.half_extent
                || ix >= map.half_extent
                || iz < -map.half_extent
                || iz >= map.half_extent
            {
                style.out_of_bounds_rgba
            } else if map.is_explored(ix, iz) {
                let c = map.cell_color(ix, iz);
                [c[0], c[1], c[2], style.explored_alpha]
            } else {
                style.unexplored_rgba
            };
            set_rgba_pixel(data, tex_w, tex_h, fx, fy, rgba);
        }
    }

    let stamp = |data: &mut [u8], pos: [f32; 2], radius: i32, fill: [u8; 3], outline: [u8; 3]| {
        let [px_f, py_f] =
            world_to_pixel(pos[0], pos[1], tex_wf, tex_hf, center, zoom, half_extent);
        let mx = px_f.round() as i32;
        let my = py_f.round() as i32;
        if (0..tex_w).contains(&mx) && (0..tex_h).contains(&my) {
            draw_map_diamond(data, tex_w, tex_h, mx, my, radius, fill, outline);
        }
    };

    // Static / discovered world markers
    stamp(data, ui_state.spawn_marker, 3, [60, 235, 90], [8, 12, 20]);
    if let Some(m) = ui_state.dungeon_marker {
        stamp(data, m, 3, [70, 140, 255], [8, 12, 20]);
    }
    if let Some(m) = ui_state.corruption_marker {
        stamp(data, m, 3, [190, 80, 255], [8, 12, 20]);
    }
    if let Some(m) = ui_state.underworld_marker {
        stamp(data, m, 3, [255, 95, 40], [8, 12, 20]);
    }

    // Live Town NPCs and Bosses
    for (enemy, interp, tf) in enemies.iter() {
        let is_town = enemy.kind.is_friendly();
        let is_boss = enemy.kind.is_boss();
        if !is_town && !is_boss {
            continue;
        }
        let pos = if let Some(i) = interp {
            i.pos()
        } else if let Some(t) = tf {
            t.translation
        } else {
            continue;
        };
        if is_boss {
            stamp(data, [pos.x, pos.z], 4, [255, 45, 75], [255, 230, 235]);
        } else {
            stamp(data, [pos.x, pos.z], 3, [80, 230, 255], [8, 12, 20]);
        }
    }

    // Player marker on top
    let [ppx, ppy] = world_to_pixel(
        player_xz[0],
        player_xz[1],
        tex_wf,
        tex_hf,
        center,
        zoom,
        half_extent,
    );
    let pmx = ppx.round() as i32;
    let pmy = ppy.round() as i32;
    if (0..tex_w).contains(&pmx) && (0..tex_h).contains(&pmy) {
        draw_player_marker(data, tex_w, tex_h, pmx, pmy);
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn update_minimap_rendering(
    ui_state: Res<MapUiState>,
    inv_open: Option<Res<super::inventory::InventoryOpen>>,
    map: Option<Res<ExploredMap>>,
    player: Option<Res<LocalPlayer>>,
    selected_world: Option<Res<SelectedWorld>>,
    clock: Option<Res<crate::net::ClientClock>>,
    assets: Option<Res<MinimapAssets>>,
    mut images: ResMut<Assets<Image>>,
    mut corner_root_q: Query<
        &mut Node,
        (
            With<CornerMinimapRoot>,
            Without<TransparentOverlayRoot>,
            Without<FullscreenMapRoot>,
            Without<MinimapInfoPill>,
        ),
    >,
    mut overlay_root_q: Query<
        &mut Node,
        (
            With<TransparentOverlayRoot>,
            Without<CornerMinimapRoot>,
            Without<FullscreenMapRoot>,
            Without<MinimapInfoPill>,
        ),
    >,
    mut pill_root_q: Query<
        &mut Node,
        (
            With<MinimapInfoPill>,
            Without<CornerMinimapRoot>,
            Without<TransparentOverlayRoot>,
            Without<FullscreenMapRoot>,
        ),
    >,
    mut pill_text_q: Query<
        &mut super::text::HudText,
        (
            With<MinimapInfoText>,
            Without<FullscreenMapTitleText>,
            Without<FullscreenMapStatsText>,
        ),
    >,
    mut full_root_q: Query<
        &mut Node,
        (
            With<FullscreenMapRoot>,
            Without<CornerMinimapRoot>,
            Without<TransparentOverlayRoot>,
            Without<MinimapInfoPill>,
        ),
    >,
    mut full_params: FullscreenMapRenderParams,
    mut perf: Option<ResMut<crate::qa::PerfCounters>>,
    mut cache: Local<MinimapRenderCache>,
) {
    let inv_is_open = inv_open.as_ref().is_some_and(|o| o.0);
    let show_corner = ui_state.overlay_mode == MapOverlayMode::Corner && !inv_is_open;
    let show_overlay = ui_state.overlay_mode == MapOverlayMode::FullscreenOverlay
        && !inv_is_open
        && !ui_state.fullscreen_visible;

    if cache.corner_vis != Some(show_corner) {
        cache.corner_vis = Some(show_corner);
        let want = if show_corner {
            Display::Flex
        } else {
            Display::None
        };
        for mut node in corner_root_q.iter_mut() {
            if node.display != want {
                node.display = want;
            }
        }
        for mut node in pill_root_q.iter_mut() {
            if node.display != want {
                node.display = want;
            }
        }
    }

    if cache.overlay_vis != Some(show_overlay) {
        cache.overlay_vis = Some(show_overlay);
        let want = if show_overlay {
            Display::Flex
        } else {
            Display::None
        };
        for mut node in overlay_root_q.iter_mut() {
            if node.display != want {
                node.display = want;
            }
        }
    }

    if cache.full_vis != Some(ui_state.fullscreen_visible) {
        cache.full_vis = Some(ui_state.fullscreen_visible);
        let want = if ui_state.fullscreen_visible {
            Display::Flex
        } else {
            Display::None
        };
        for mut node in full_root_q.iter_mut() {
            if node.display != want {
                node.display = want;
            }
        }
    }

    let (Some(map), Some(player), Some(assets)) = (map, player, assets) else {
        return;
    };

    if show_corner && let Ok(mut info_text) = pill_text_q.single_mut() {
        let p = player.0.body.pos;
        let (wx, wy, wz) = (
            p[0].floor() as i32,
            p[1].floor() as i32,
            p[2].floor() as i32,
        );
        let seed = selected_world
            .as_ref()
            .map_or(crate::view::SEED, |w| w.seed);
        let size = selected_world.as_ref().map_or(WorldSize::Small, |w| w.size);
        let biome_str = if wy <= terravibe_sim::worldgen::liquid::LAVA_DEPTH_Y {
            "Underworld"
        } else if corruption_at_for_size(seed, wx, wz, size) > 0.0 {
            "Corruption"
        } else {
            let w = terravibe_sim::worldgen::biome::biome_weights_for_size(seed, wx, wz, size);
            match terravibe_sim::worldgen::biome::dominant(&w) {
                terravibe_sim::worldgen::biome::Biome::Forest => "Forest",
                terravibe_sim::worldgen::biome::Biome::Desert => "Desert",
                terravibe_sim::worldgen::biome::Biome::Snow => "Snow",
                terravibe_sim::worldgen::biome::Biome::Jungle => "Jungle",
            }
        };
        let surface = terravibe_sim::worldgen::terrain::surface_height_for_size(seed, wx, wz, size);
        let rel_depth = surface - wy;
        let depth_str = if rel_depth <= 5 {
            "Surface".to_string()
        } else {
            format!("-{rel_depth}m")
        };
        let time_str = if clock.as_ref().is_none_or(|c| {
            terravibe_sim::time::WorldClock {
                tick: c.effective_tick(),
            }
            .is_day()
        }) {
            "Day"
        } else {
            "Night"
        };
        let want = format!("{biome_str} · {depth_str} · {time_str}");
        if info_text.text != want {
            info_text.text = want;
        }
    }

    if ui_state.fullscreen_visible {
        let world_name = selected_world
            .as_ref()
            .map_or("TerraVibe World", |w| w.name.as_str());
        if let Ok(mut title_text) = full_params.title_q.single_mut()
            && title_text.text != world_name
        {
            title_text.text = world_name.to_string();
        }
        if let Ok(mut stats_text) = full_params.stats_q.single_mut() {
            let zoom_pct = (ui_state.full_zoom * 100.0).round() as i32;
            let coords = match ui_state.cursor_world {
                Some([cx, cz]) => format!("({cx}, {cz})"),
                None => "(---, ---)".to_string(),
            };
            let want = format!("Zoom: {zoom_pct}%  ·  Cursor: {coords}");
            if stats_text.text != want {
                stats_text.text = want;
            }
        }
    }

    let mut enemy_sig = 0u64;
    if ui_state.fullscreen_visible || show_overlay {
        for (enemy, interp, tf) in full_params.enemies.iter() {
            if !enemy.kind.is_friendly() && !enemy.kind.is_boss() {
                continue;
            }
            let pos = if let Some(i) = interp {
                i.pos()
            } else if let Some(t) = tf {
                t.translation
            } else {
                continue;
            };
            enemy_sig = enemy_sig.rotate_left(7)
                ^ (enemy.id.0 as u64)
                ^ ((pos.x.round() as i64 as u64) << 16)
                ^ ((pos.z.round() as i64 as u64) << 32);
        }
    }

    let px = player.0.body.pos[0];
    let pz = player.0.body.pos[2];
    let center_cell_x = px.div_euclid(MAP_CELL_SIZE as f32) as i32;
    let center_cell_z = pz.div_euclid(MAP_CELL_SIZE as f32) as i32;
    let zoom_bits = ui_state.full_zoom.to_bits();
    let center_bits = [
        ui_state.full_center[0].to_bits(),
        ui_state.full_center[1].to_bits(),
    ];
    let map_or_ui_changed = map.is_changed() || ui_state.is_changed();
    let need_corner = show_corner
        && (map_or_ui_changed || cache.corner_cell != Some((center_cell_x, center_cell_z)));
    let need_overlay = show_overlay
        && (map_or_ui_changed
            || cache.overlay_cell != Some((center_cell_x, center_cell_z))
            || cache.overlay_enemy_sig != Some(enemy_sig));
    let need_full = ui_state.fullscreen_visible
        && (map_or_ui_changed
            || cache.full_cell != Some((center_cell_x, center_cell_z))
            || cache.full_zoom_bits != Some(zoom_bits)
            || cache.full_center_bits != Some(center_bits)
            || cache.full_enemy_sig != Some(enemy_sig));

    let rebuild_start = (need_corner || need_overlay || need_full).then(std::time::Instant::now);

    // Update corner minimap texture (100x100 cells centered on player) only when dirty
    if need_corner && let Some(mut corner_img) = images.get_mut(&assets.corner_handle) {
        cache.corner_cell = Some((center_cell_x, center_cell_z));

        for cy in 0..100 {
            for cx in 0..100 {
                let cell_x = center_cell_x + cx - 50;
                let cell_z = center_cell_z + cy - 50;
                let world_x = cell_x * MAP_CELL_SIZE;
                let world_z = cell_z * MAP_CELL_SIZE;

                let (r, g, b, a) = if map.is_explored(world_x, world_z) {
                    let c = map.cell_color(world_x, world_z);
                    (c[0], c[1], c[2], 255)
                } else {
                    (15, 15, 22, 220)
                };

                let idx = ((cy * 100 + cx) * 4) as usize;
                if let Some(ref mut data) = corner_img.data
                    && idx + 3 < data.len()
                {
                    data[idx] = r;
                    data[idx + 1] = g;
                    data[idx + 2] = b;
                    data[idx + 3] = a;
                }
            }
        }
    }

    // Update Terraria-style transparent overlay texture (320x180, unexplored alpha=0, explored alpha=115)
    if need_overlay && let Some(mut overlay_img) = images.get_mut(&assets.overlay_handle) {
        cache.overlay_cell = Some((center_cell_x, center_cell_z));
        cache.overlay_enemy_sig = Some(enemy_sig);
        render_world_map_texture(
            &mut overlay_img,
            &map,
            &ui_state,
            [px, pz],
            &full_params.enemies,
            MapTextureStyle {
                tex_w: OVERLAY_MAP_TEX_W,
                tex_h: OVERLAY_MAP_TEX_H,
                center: [px, pz],
                zoom: OVERLAY_ZOOM,
                unexplored_rgba: [0, 0, 0, 0],
                out_of_bounds_rgba: [0, 0, 0, 0],
                explored_alpha: OVERLAY_EXPLORED_ALPHA,
            },
        );
    }

    // Update fullscreen map texture (480x270 live explored map with zoom/pan/markers) only when dirty
    if need_full && let Some(mut full_img) = images.get_mut(&assets.full_handle) {
        cache.full_cell = Some((center_cell_x, center_cell_z));
        cache.full_zoom_bits = Some(zoom_bits);
        cache.full_center_bits = Some(center_bits);
        cache.full_enemy_sig = Some(enemy_sig);
        let half_extent = map.half_extent as f32;
        let (center, zoom) = clamp_full_view(ui_state.full_center, ui_state.full_zoom, half_extent);
        render_world_map_texture(
            &mut full_img,
            &map,
            &ui_state,
            [px, pz],
            &full_params.enemies,
            MapTextureStyle {
                tex_w: FULL_MAP_TEX_W,
                tex_h: FULL_MAP_TEX_H,
                center,
                zoom,
                unexplored_rgba: [10, 14, 24, 240],
                out_of_bounds_rgba: [6, 9, 18, 255],
                explored_alpha: 255,
            },
        );
    }

    if let (Some(t0), Some(perf)) = (rebuild_start, perf.as_deref_mut()) {
        perf.map_rebuild_ms = t0.elapsed().as_secs_f32() * 1000.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terravibe_sim::ChunkPos;

    fn tmp() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "tv-map-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn test_hud_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<MouseWheel>()
            .init_resource::<MapUiState>()
            .add_systems(Update, handle_map_input.run_if(in_state(AppState::InGame)));
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();
        app
    }

    fn press_key(app: &mut App, key: KeyCode) {
        if let Some(mut keys) = app.world_mut().get_resource_mut::<ButtonInput<KeyCode>>() {
            keys.press(key);
        }
    }

    fn set_highest_block(chunks: &mut ChunkStream, x: i32, z: i32, block: BlockId) {
        let y = 60;
        let pos = ChunkPos::of_block(x, y, z);
        if !chunks.generated().contains_key(&pos) {
            chunks.insert_chunk(pos, Arc::new(Chunk::filled(terravibe_sim::block::AIR)));
        }
        chunks.set_block([x, y, z], block);
    }

    #[test]
    fn column_within_48_blocks_revealed_and_outside_hidden() {
        let mut map = ExploredMap::new(1440); // Small world: half 1440, 720x720 cells
        let player_pos = [0.0, 50.0, 0.0];
        let mut chunks = ChunkStream::default();
        // Fill column at (20, 20) with GRASS and (80, 80) with STONE
        set_highest_block(&mut chunks, 20, 20, terravibe_sim::block::GRASS);
        set_highest_block(&mut chunks, 80, 80, terravibe_sim::block::STONE);

        map.update_explored(player_pos, &chunks);

        // (20, 20) distance ~ 28.3 <= 48 -> revealed
        assert!(map.is_explored(20, 20));
        assert_eq!(
            map.cell_color(20, 20),
            block_minimap_color(terravibe_sim::block::GRASS)
        );

        // (80, 80) distance ~ 113.1 > 48 -> remains hidden
        assert!(!map.is_explored(80, 80));
    }

    #[test]
    fn map_persistence_roundtrip() {
        let tmp = tmp();
        let mut map = ExploredMap::new(1440);
        map.set_cell(10, 10, true, [12, 34, 56]);

        let path = tmp.join("map.bin");
        map.save_to(&path).unwrap();

        let loaded = ExploredMap::load_from(&path, 1440).unwrap();
        assert!(loaded.is_explored(10 * 4, 10 * 4));
        assert_eq!(loaded.cell_color(10 * 4, 10 * 4), [12, 34, 56]);
    }

    #[test]
    fn tab_cycles_three_map_overlay_modes_and_m_toggles_fullscreen() {
        let mut app = test_hud_app();
        app.init_resource::<MapUiState>();
        assert_eq!(
            app.world().resource::<MapUiState>().overlay_mode,
            MapOverlayMode::Corner
        );
        assert!(!app.world().resource::<MapUiState>().fullscreen_visible);

        // Tab: Corner -> FullscreenOverlay
        press_key(&mut app, KeyCode::Tab);
        app.update();
        assert_eq!(
            app.world().resource::<MapUiState>().overlay_mode,
            MapOverlayMode::FullscreenOverlay
        );
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.release(KeyCode::Tab);
            keys.clear();
        }

        // Tab: FullscreenOverlay -> Off
        press_key(&mut app, KeyCode::Tab);
        app.update();
        assert_eq!(
            app.world().resource::<MapUiState>().overlay_mode,
            MapOverlayMode::Off
        );
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.release(KeyCode::Tab);
            keys.clear();
        }

        // Tab: Off -> Corner
        press_key(&mut app, KeyCode::Tab);
        app.update();
        assert_eq!(
            app.world().resource::<MapUiState>().overlay_mode,
            MapOverlayMode::Corner
        );

        // M: toggles fullscreen map
        press_key(&mut app, KeyCode::KeyM);
        app.update();
        assert!(app.world().resource::<MapUiState>().fullscreen_visible);
    }

    #[test]
    fn markers_revealed_within_64_blocks() {
        let mut ui_state = MapUiState::default();
        let seed = 42;
        let size = WorldSize::Small;
        let entrance = dungeon_entrance_pos(seed, size);

        // Initially only spawn marker is set
        assert_eq!(ui_state.spawn_marker, [0.0, 0.0]);
        assert_eq!(ui_state.dungeon_marker, None);
        assert_eq!(ui_state.corruption_marker, None);
        assert_eq!(ui_state.underworld_marker, None);

        // Far away from dungeon
        ui_state.update_markers([0.0, 50.0, 0.0], seed, size);
        assert_eq!(ui_state.dungeon_marker, None);

        // Within 64 blocks of dungeon entrance
        ui_state.update_markers(
            [entrance[0] + 10.0, entrance[1], entrance[2] + 10.0],
            seed,
            size,
        );
        assert!(ui_state.dungeon_marker.is_some());

        // Descend to Underworld: y <= 24
        ui_state.update_markers([0.0, 20.0, 0.0], seed, size);
        assert!(ui_state.underworld_marker.is_some());
    }

    #[test]
    fn compass_heading_and_directions() {
        // Forward pointing -Z (North)
        let heading_north = heading_angle(Vec3::NEG_Z);
        assert!(heading_north.abs() < 1e-5);
        let offset = compass_offset(heading_north, 0.0, 240.0).unwrap();
        assert!(offset.abs() < 1e-4);

        // Forward pointing +X (East)
        let heading_east = heading_angle(Vec3::X);
        assert!((heading_east - PI / 2.0).abs() < 1e-5);

        // Opposite direction (South) should be outside 180 FOV when facing North
        let offset_south = compass_offset(heading_north, PI, 240.0);
        // At exactly PI it's at the boundary or outside
        assert!(offset_south.is_none() || offset_south.unwrap().abs() >= 120.0);
    }

    #[test]
    fn unvisited_chunks_remain_hidden() {
        let mut map = ExploredMap::new(1440);
        let chunks = ChunkStream::default(); // empty
        map.update_explored([0.0, 50.0, 0.0], &chunks);
        assert!(!map.is_explored(0, 0));
        assert_eq!(map.cell_color(0, 0), [0, 0, 0]);
    }

    #[test]
    fn fullscreen_zoom_and_pan_clamping() {
        let half = 1440.0;

        // Zoom is clamped to [1.0, 16.0]
        let (c1, z1) = clamp_full_view([500.0, -500.0], 0.25, half);
        assert!((z1 - 1.0).abs() < 1e-5);
        assert_eq!(c1, [0.0, 0.0]); // At 1.0x, entire world fits -> center is [0, 0]

        let (c2, z2) = clamp_full_view([5000.0, -5000.0], 20.0, half);
        assert!((z2 - FULL_ZOOM_MAX).abs() < 1e-5);
        let max_off = half - half / FULL_ZOOM_MAX; // 1350.0
        assert!((c2[0] - max_off).abs() < 1e-4);
        assert!((c2[1] + max_off).abs() < 1e-4);

        // At 2.0x, max_offset = 720.0
        let (c3, z3) = clamp_full_view([400.0, -900.0], 2.0, half);
        assert!((z3 - 2.0).abs() < 1e-5);
        assert!((c3[0] - 400.0).abs() < 1e-4);
        assert!((c3[1] + 720.0).abs() < 1e-4);
    }

    #[test]
    fn fullscreen_coordinate_mapping_roundtrip() {
        let half = 1440.0;
        let tex_w = FULL_MAP_TEX_W as f32;
        let tex_h = FULL_MAP_TEX_H as f32;
        let center = [240.0, -180.0];
        let zoom = 4.5;

        // Center pixel maps to `center`
        let w_center = pixel_to_world(tex_w * 0.5, tex_h * 0.5, tex_w, tex_h, center, zoom, half);
        assert!((w_center[0] - center[0]).abs() < 1e-4);
        assert!((w_center[1] - center[1]).abs() < 1e-4);

        // Arbitrary world point roundtrips through world_to_pixel -> pixel_to_world
        let wx = 315.0;
        let wz = -95.0;
        let px = world_to_pixel(wx, wz, tex_w, tex_h, center, zoom, half);
        let back = pixel_to_world(px[0], px[1], tex_w, tex_h, center, zoom, half);
        assert!((back[0] - wx).abs() < 1e-3);
        assert!((back[1] - wz).abs() < 1e-3);

        // Zooming around a UV keeps that world coordinate pinned under the UV
        let uv = [0.75, 0.25];
        let before = pixel_to_world(
            uv[0] * tex_w,
            uv[1] * tex_h,
            tex_w,
            tex_h,
            center,
            2.0,
            half,
        );
        let (new_c, new_z) = zoom_around_uv(center, 2.0, 4.5, uv, tex_w, tex_h, half);
        let after = pixel_to_world(
            uv[0] * tex_w,
            uv[1] * tex_h,
            tex_w,
            tex_h,
            new_c,
            new_z,
            half,
        );
        assert!((after[0] - before[0]).abs() < 1e-3);
        assert!((after[1] - before[1]).abs() < 1e-3);
    }

    #[test]
    fn fullscreen_map_esc_scroll_pan_and_recenter() {
        let mut app = test_hud_app();
        app.add_message::<MouseWheel>()
            .init_resource::<ButtonInput<MouseButton>>();

        // Default full_zoom is 4.5
        assert!((app.world().resource::<MapUiState>().full_zoom - FULL_ZOOM_DEFAULT).abs() < 1e-5);

        // Open fullscreen map with M
        press_key(&mut app, KeyCode::KeyM);
        app.update();
        assert!(app.world().resource::<MapUiState>().fullscreen_visible);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();

        // Scroll up to zoom in
        app.world_mut().write_message(MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: 3.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
        app.update();
        let zoom_after_scroll = app.world().resource::<MapUiState>().full_zoom;
        assert!(zoom_after_scroll > FULL_ZOOM_DEFAULT && zoom_after_scroll <= FULL_ZOOM_MAX);

        // Pan right with D
        press_key(&mut app, KeyCode::KeyD);
        app.update();
        assert!(app.world().resource::<MapUiState>().full_center[0] > 0.0);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();

        // Close with Esc
        press_key(&mut app, KeyCode::Escape);
        app.update();
        assert!(!app.world().resource::<MapUiState>().fullscreen_visible);
    }

    #[test]
    fn transparent_overlay_has_zero_alpha_unexplored_and_115_explored() {
        let mut map = ExploredMap::new(1440);
        // Mark a cell at world (40, 40) explored with green color.
        map.set_cell(10, 10, true, [80, 180, 60]);

        let mut img = Image::new_fill(
            Extent3d {
                width: OVERLAY_MAP_TEX_W,
                height: OVERLAY_MAP_TEX_H,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[0, 0, 0, 0],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::MAIN_WORLD,
        );

        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let mut q_state = app
            .world_mut()
            .query::<(&ClientEnemy, Option<&EnemyInterp>, Option<&Transform>)>();
        let ui_state = MapUiState::default();
        let empty_enemies = q_state.query(app.world());

        render_world_map_texture(
            &mut img,
            &map,
            &ui_state,
            [0.0, 0.0],
            &empty_enemies,
            MapTextureStyle {
                tex_w: OVERLAY_MAP_TEX_W,
                tex_h: OVERLAY_MAP_TEX_H,
                center: [0.0, 0.0],
                zoom: OVERLAY_ZOOM,
                unexplored_rgba: [0, 0, 0, 0],
                out_of_bounds_rgba: [0, 0, 0, 0],
                explored_alpha: OVERLAY_EXPLORED_ALPHA,
            },
        );

        let data = img.data.as_ref().unwrap();
        // Top-left corner (far from origin and explored cell) must be 100% transparent (alpha == 0).
        assert_eq!(&data[0..4], &[0, 0, 0, 0]);

        // Pixel corresponding to explored world (42, 42) must have alpha == 115.
        let [px, py] = world_to_pixel(
            42.0,
            42.0,
            OVERLAY_MAP_TEX_W as f32,
            OVERLAY_MAP_TEX_H as f32,
            [0.0, 0.0],
            OVERLAY_ZOOM,
            1440.0,
        );
        let idx = ((py as usize) * OVERLAY_MAP_TEX_W as usize + (px as usize)) * 4;
        assert_eq!(data[idx + 3], OVERLAY_EXPLORED_ALPHA);
    }

    #[test]
    fn widescreen_fullscreen_map_and_overlay_layout() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<Image>>()
            .add_systems(Startup, setup_map_ui);
        app.update();

        let assets = app.world().resource::<MinimapAssets>();
        let images = app.world().resource::<Assets<Image>>();
        let full_img = images.get(&assets.full_handle).unwrap();
        assert_eq!(full_img.width(), FULL_MAP_TEX_W);
        assert_eq!(full_img.height(), FULL_MAP_TEX_H);

        let overlay_img = images.get(&assets.overlay_handle).unwrap();
        assert_eq!(overlay_img.width(), OVERLAY_MAP_TEX_W);
        assert_eq!(overlay_img.height(), OVERLAY_MAP_TEX_H);

        let mut full_root_q = app
            .world_mut()
            .query_filtered::<(&Node, &GlobalZIndex), With<FullscreenMapRoot>>();
        let (full_node, full_z) = full_root_q.single(app.world()).unwrap();
        assert_eq!(full_node.width, Val::Percent(100.0));
        assert_eq!(full_node.height, Val::Percent(100.0));
        assert_eq!(full_z.0, 1150);

        let mut overlay_root_q = app
            .world_mut()
            .query_filtered::<(&Node, &GlobalZIndex), With<TransparentOverlayRoot>>();
        let (overlay_node, overlay_z) = overlay_root_q.single(app.world()).unwrap();
        assert_eq!(overlay_node.width, Val::Percent(100.0));
        assert_eq!(overlay_node.height, Val::Percent(100.0));
        assert_eq!(overlay_z.0, 20);
    }
}
