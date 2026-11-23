//! Block texture atlas: 16x16 PNGs from `assets/blocks`, packed into 18x18 cells
//! (the tile plus a 1 px duplicated edge so grazing angles never sample a neighbour).

use std::collections::HashMap;
use std::path::Path;

use terravibe_assets::RgbaImage;
use terravibe_sim::{
    ASH, BLUE_DUNGEON_BRICK, BlockId, CLOUD_BLOCK, COPPER, CORRUPT_GRASS, DEMONITE_ORE, DIRT,
    EBONSTONE, GLASS_BLOCK, GOLD, GRASS, GREEN_DUNGEON_BRICK, HARDENED_SAND, HELLSTONE,
    HELLSTONE_BRICK, HIVE_BLOCK, ICE, IRON, JUNGLE_GRASS, METEORITE_ORE, MUD, MUSHROOM_GRASS,
    OBSIDIAN, OBSIDIAN_BRICK, PINK_DUNGEON_BRICK, SAND, SANDSTONE, SHADOW_ORB, SILVER, SNOW,
    SPIKES, STONE, SUNPLATE_BLOCK, TRUNK, WOOD_BLOCK,
};

use super::mesh_data::Face;

/// Block texture edge in pixels.
pub const TILE: u32 = 16;
const CELL: u32 = TILE + 2;
const COLUMNS: u32 = 16;
/// Pseudo block id of the wall texture.
pub const WALL_ID: u16 = 0xFFF0;
/// Variants a block can have per face group (`<name>_v1.png`, `_v2`, ...).
const MAX_VARIANTS: u8 = 4;

/// Every cube-meshed block with its file stem under `assets/blocks`.
pub const CUBE_BLOCKS: [(BlockId, &str); 35] = [
    (DIRT, "dirt"),
    (STONE, "stone"),
    (GRASS, "grass"),
    (COPPER, "copper_ore"),
    (IRON, "iron_ore"),
    (SILVER, "silver_ore"),
    (GOLD, "gold_ore"),
    (TRUNK, "trunk"),
    (SAND, "sand"),
    (MUD, "mud"),
    (JUNGLE_GRASS, "jungle_grass"),
    (SNOW, "snow"),
    (ICE, "ice"),
    (WOOD_BLOCK, "wood_block"),
    (SANDSTONE, "sandstone"),
    (HARDENED_SAND, "hardened_sand"),
    (GLASS_BLOCK, "glass"),
    (DEMONITE_ORE, "demonite_ore"),
    (CORRUPT_GRASS, "corrupt_grass"),
    (EBONSTONE, "ebonstone"),
    (SHADOW_ORB, "shadow_orb"),
    (BLUE_DUNGEON_BRICK, "blue_dungeon_brick"),
    (GREEN_DUNGEON_BRICK, "green_dungeon_brick"),
    (PINK_DUNGEON_BRICK, "pink_dungeon_brick"),
    (SPIKES, "spikes"),
    (OBSIDIAN, "obsidian"),
    (ASH, "ash"),
    (HELLSTONE, "hellstone"),
    (OBSIDIAN_BRICK, "obsidian_brick"),
    (HELLSTONE_BRICK, "hellstone_brick"),
    (MUSHROOM_GRASS, "mushroom_grass"),
    (CLOUD_BLOCK, "cloud"),
    (SUNPLATE_BLOCK, "sunplate"),
    (METEORITE_ORE, "meteorite_ore"),
    (HIVE_BLOCK, "hive"),
];

pub struct BlockArt {
    pub atlas: RgbaImage,
    /// `[u0, v0, u1, v1]` of the 16x16 interior, per (block id, face, variant).
    pub uv: HashMap<(u16, Face, u8), [f32; 4]>,
    /// Individual 16x16 RGBA8 tiles for `Texture2DArray` upload.
    pub tiles: Vec<Vec<[u8; 4]>>,
    /// Layer index in `tiles` per (block id, face, variant).
    pub layer: HashMap<(u16, Face, u8), u32>,
    variants: HashMap<(u16, Face), u8>,
}

fn group(face: Face) -> &'static str {
    match face {
        Face::PosY => "top",
        Face::NegY => "bottom",
        _ => "side",
    }
}

fn decode(path: &Path) -> Result<Vec<[u8; 4]>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("missing art: {}: {e}", path.display()))?;
    let bad = |e: String| format!("bad art: {}: {e}", path.display());
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .map_err(|e| bad(e.to_string()))?;
    let mut buf = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or_else(|| bad("too large".into()))?
    ];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| bad(e.to_string()))?;
    if info.color_type != png::ColorType::Rgba
        || info.bit_depth != png::BitDepth::Eight
        || (info.width, info.height) != (TILE, TILE)
    {
        return Err(bad(format!(
            "want {TILE}x{TILE} RGBA8, got {}x{} {:?}",
            info.width, info.height, info.color_type
        )));
    }
    Ok(buf
        .chunks_exact(4)
        .map(|p| [p[0], p[1], p[2], p[3]])
        .collect())
}

impl BlockArt {
    /// Loads `dir/blocks/*.png`; the error names the offending file.
    pub fn try_load(dir: &Path) -> Result<BlockArt, String> {
        let blocks = dir.join("blocks");
        let mut entries: Vec<(u16, &str)> = CUBE_BLOCKS.iter().map(|&(b, n)| (b.0, n)).collect();
        entries.push((WALL_ID, "wood_wall"));
        let mut tiles: Vec<Vec<[u8; 4]>> = Vec::new();
        let mut slot_of: HashMap<String, usize> = HashMap::new();
        let mut slots: HashMap<(u16, Face, u8), usize> = HashMap::new();
        let mut variants = HashMap::new();
        for (id, name) in entries {
            for face in Face::ALL {
                let g = group(face);
                // `<name>_<group>` first, then the all-faces `<name>`.
                let stem = [format!("{name}_{g}"), name.to_string()]
                    .into_iter()
                    .find(|s| blocks.join(format!("{s}.png")).exists())
                    .ok_or_else(|| {
                        format!(
                            "missing art: {}",
                            blocks.join(format!("{name}_{g}.png")).display()
                        )
                    })?;
                let mut n = 0u8;
                while n < MAX_VARIANTS {
                    let file = if n == 0 {
                        format!("{stem}.png")
                    } else {
                        format!("{stem}_v{n}.png")
                    };
                    let path = blocks.join(&file);
                    if !path.exists() {
                        break;
                    }
                    let slot = match slot_of.get(&file) {
                        Some(&s) => s,
                        None => {
                            tiles.push(decode(&path)?);
                            slot_of.insert(file, tiles.len() - 1);
                            tiles.len() - 1
                        }
                    };
                    slots.insert((id, face, n), slot);
                    n += 1;
                }
                variants.insert((id, face), n);
            }
        }
        let rows = (tiles.len() as u32).div_ceil(COLUMNS);
        let mut atlas = RgbaImage::new(COLUMNS * CELL, rows * CELL);
        for (i, t) in tiles.iter().enumerate() {
            let (ox, oy) = (i as u32 % COLUMNS * CELL, i as u32 / COLUMNS * CELL);
            for py in 0..CELL {
                for px in 0..CELL {
                    let (tx, ty) = (
                        px.saturating_sub(1).min(TILE - 1),
                        py.saturating_sub(1).min(TILE - 1),
                    );
                    atlas.pixels[((oy + py) * atlas.width + ox + px) as usize] =
                        t[(ty * TILE + tx) as usize];
                }
            }
        }
        let (w, h) = (atlas.width as f32, atlas.height as f32);
        let layer = slots.iter().map(|(&k, &s)| (k, s as u32)).collect();
        let uv = slots
            .into_iter()
            .map(|(k, s)| {
                let (x, y) = (
                    (s as u32 % COLUMNS * CELL + 1) as f32,
                    (s as u32 / COLUMNS * CELL + 1) as f32,
                );
                (
                    k,
                    [x / w, y / h, (x + TILE as f32) / w, (y + TILE as f32) / h],
                )
            })
            .collect();
        Ok(BlockArt {
            atlas,
            uv,
            tiles,
            layer,
            variants,
        })
    }

    /// Like `try_load`, exiting with the message on failure.
    pub fn load(dir: &Path) -> BlockArt {
        Self::try_load(dir).unwrap_or_else(|e| crate::view::fatal(e))
    }

    pub fn has(&self, id: BlockId) -> bool {
        self.variants.contains_key(&(id.0, Face::PosY))
    }

    /// Interior UV rect of `face` of the block at `(x, y, z)`; the variant is
    /// a position hash. Panics for a block without art (check `has` first).
    pub fn face_uv(&self, id: BlockId, face: Face, x: i32, y: i32, z: i32) -> [f32; 4] {
        let n = self.variants[&(id.0, face)];
        self.uv[&(id.0, face, pick(x, y, z, face, n))]
    }

    /// Tile array layer index of `face` of the block at `(x, y, z)`.
    pub fn face_layer(&self, id: BlockId, face: Face, x: i32, y: i32, z: i32) -> u32 {
        let n = self.variants[&(id.0, face)];
        self.layer[&(id.0, face, pick(x, y, z, face, n))]
    }

    /// The wall panel texture.
    pub fn wall_uv(&self) -> [f32; 4] {
        self.uv[&(WALL_ID, Face::PosX, 0)]
    }

    /// Tile array layer index of the wall panel texture.
    pub fn wall_layer(&self) -> u32 {
        self.layer[&(WALL_ID, Face::PosX, 0)]
    }
}

/// Stable per-face hash in `0..n`.
fn pick(x: i32, y: i32, z: i32, face: Face, n: u8) -> u8 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1)
        ^ (y as u32).wrapping_mul(0x85EB_CA77)
        ^ (z as u32).wrapping_mul(0xC2B2_AE3D)
        ^ (face as u32).wrapping_mul(0x27D4_EB2F);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h % n.max(1) as u32) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn art() -> &'static BlockArt {
        use std::sync::OnceLock;
        static ART: OnceLock<BlockArt> = OnceLock::new();
        ART.get_or_init(|| BlockArt::try_load(&super::super::art_dir()).unwrap())
    }

    /// Writes a solid `w`x`h` RGBA PNG.
    fn write_png(path: &Path, w: u32, h: u32, px: [u8; 4]) {
        let file = std::fs::File::create(path).unwrap();
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let data: Vec<u8> = (0..w * h).flat_map(|_| px).collect();
        enc.write_header().unwrap().write_image_data(&data).unwrap();
    }

    /// A synthetic art dir where every block has one `size`x`size` tile.
    fn fixture(tag: &str, size: u32, px: [u8; 4]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("tv_blocks_{tag}_{}", std::process::id()));
        let blocks = dir.join("blocks");
        std::fs::create_dir_all(&blocks).unwrap();
        for (_, name) in CUBE_BLOCKS {
            write_png(&blocks.join(format!("{name}.png")), size, size, px);
        }
        write_png(&blocks.join("wood_wall.png"), size, size, px);
        dir
    }

    #[test]
    fn atlas_cells_are_18px_with_edge_padding() {
        let dir = fixture("cells", 16, [255, 0, 0, 255]);
        let a = BlockArt::try_load(&dir).unwrap();
        assert_eq!((TILE, CELL), (16, 18));
        assert_eq!((a.atlas.width, a.atlas.height % 18), (16 * 18, 0));
        // Cell 0 is the whole 18x18 block: red tile plus its duplicated ring.
        for y in 0..18 {
            for x in 0..18 {
                assert_eq!(
                    a.atlas.pixels[(y * a.atlas.width + x) as usize],
                    [255, 0, 0, 255]
                );
            }
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_8x8_png_naming_path_and_size() {
        let dir = fixture("reject", 8, [1, 2, 3, 255]);
        let e = BlockArt::try_load(&dir).err().unwrap();
        assert!(
            e.contains(dir.to_str().unwrap()) && e.contains(".png"),
            "{e}"
        );
        assert!(e.contains("16x16") && e.contains("8x8"), "{e}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn face_uv_inside_interior_of_cell() {
        let dir = fixture("uv", 16, [9, 9, 9, 255]);
        let a = BlockArt::try_load(&dir).unwrap();
        let (w, h) = (a.atlas.width as f32, a.atlas.height as f32);
        let r = a.face_uv(DIRT, Face::PosY, 0, 0, 0);
        // Slot 0: the interior starts 1 px into the 18 px cell and spans 16 px.
        let want = [1.0 / w, 1.0 / h, 17.0 / w, 17.0 / h];
        for (g, w) in r.iter().zip(want) {
            assert!((g - w).abs() < 1e-6, "{r:?} vs {want:?}");
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn atlas_tiles_are_edge_padded() {
        let a = art();
        let (w, h) = (a.atlas.width as f32, a.atlas.height as f32);
        let px = |x: i32, y: i32| a.atlas.pixels[(y as u32 * a.atlas.width + x as u32) as usize];
        for r in a.uv.values() {
            let (x0, y0) = ((r[0] * w).round() as i32, (r[1] * h).round() as i32);
            let (x1, y1) = (x0 + TILE as i32 - 1, y0 + TILE as i32 - 1);
            for i in 0..TILE as i32 {
                assert_eq!(px(x0 - 1, y0 + i), px(x0, y0 + i));
                assert_eq!(px(x1 + 1, y0 + i), px(x1, y0 + i));
                assert_eq!(px(x0 + i, y0 - 1), px(x0 + i, y0));
                assert_eq!(px(x0 + i, y1 + 1), px(x0 + i, y1));
            }
        }
    }

    #[test]
    fn every_cube_block_has_art() {
        let a = art();
        for (id, name) in CUBE_BLOCKS {
            assert!(a.has(id), "{name}");
            for f in Face::ALL {
                let r = a.face_uv(id, f, 3, 4, 5);
                assert!(r[0] < r[2] && r[1] < r[3], "{name} {f:?}");
            }
        }
        assert!(a.wall_uv()[0] < a.wall_uv()[2]);
    }

    #[test]
    fn variants_vary_and_are_stable() {
        let a = art();
        let uvs: std::collections::HashSet<[u32; 4]> = (0..64)
            .map(|x| a.face_uv(DIRT, Face::PosY, x, 0, 0).map(f32::to_bits))
            .collect();
        assert_eq!(uvs.len(), 2);
        assert_eq!(
            a.face_uv(GRASS, Face::PosX, 9, 9, 9),
            a.face_uv(GRASS, Face::PosX, 9, 9, 9)
        );
    }

    #[test]
    fn missing_art_reports_path() {
        let e = BlockArt::try_load(Path::new("/nonexistent/art"))
            .err()
            .unwrap();
        assert!(e.contains("/nonexistent/art/blocks/"), "{e}");
    }

    #[test]
    fn every_non_air_block_has_cube_or_model_art() {
        use terravibe_sim::*;
        let a = art();
        let all_blocks = [
            DIRT,
            STONE,
            GRASS,
            IRON,
            COPPER,
            GOLD,
            SILVER,
            TORCH,
            TRUNK,
            SAND,
            MUD,
            JUNGLE_GRASS,
            DEMONITE_ORE,
            CORRUPT_GRASS,
            EBONSTONE,
            DEMON_ALTAR,
            SHADOW_ORB,
            HELLSTONE,
            SNOW,
            ICE,
            WOOD_BLOCK,
            SANDSTONE,
            HARDENED_SAND,
            WORKBENCH,
            ANVIL,
            FURNACE,
            CHEST,
            LIFE_CRYSTAL,
            DOOR_CLOSED,
            DOOR_OPEN,
            TABLE,
            CHAIR,
            PLATFORM,
            MUSHROOM_PLANT,
            BOTTLE,
            GLASS_BLOCK,
            OBSIDIAN,
            BLUE_DUNGEON_BRICK,
            GREEN_DUNGEON_BRICK,
            PINK_DUNGEON_BRICK,
            SPIKES,
            ASH,
            OBSIDIAN_BRICK,
            HELLSTONE_BRICK,
            HELLFORGE,
            POT,
            STALACTITE,
            MOSS,
            SMALL_ROCK,
            SHELF_MUSHROOM,
        ];
        for id in all_blocks {
            assert_ne!(id, AIR);
            let is_cube = a.has(id);
            let is_model = crate::art::is_model_block(id);
            let model_path = crate::view::block_models::model_file(id);
            assert!(
                is_cube || (is_model && model_path.is_some()),
                "BlockId({}) has neither cube art nor model art",
                id.0
            );
            if let Some(f) = model_path {
                assert!(
                    is_model,
                    "BlockId({}) has model_file but !is_model_block",
                    id.0
                );
                let p = crate::art::art_dir().join("models/blocks").join(f);
                assert!(p.is_file(), "missing model file {}", p.display());
            }
        }
    }
}
