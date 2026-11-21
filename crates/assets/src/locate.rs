use std::path::{Path, PathBuf};

use crate::{
    AssetError, RgbaImage, SoundEffect, SpriteFont, decode_sound, decode_sprite_font,
    decode_texture, read_xnb,
};

pub const ENV_OVERRIDE: &str = "TERRAVIBE_TERRARIA_DIR";
pub const ENV_ASSETS: &str = "TERRAVIBE_ASSETS_DIR";

const MARKER: &str = "Images/Tiles_0.xnb";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Install,
    Pack,
}

/// A validated Terraria `Content` directory, or an extracted asset pack.
#[derive(Debug, Clone)]
pub struct TerrariaContent {
    root: PathBuf,
    source: Source,
}

impl TerrariaContent {
    /// Search order: explicit override, `$TERRAVIBE_TERRARIA_DIR`, Steam libraries.
    /// Invalid overrides are recorded in `tried` and the search continues.
    pub fn locate(override_dir: Option<PathBuf>) -> Result<Self, AssetError> {
        Self::locate_in(install_candidates(override_dir))
    }

    /// Search order: `pack_dir`, `$TERRAVIBE_ASSETS_DIR`, `<exe dir>/assets`, then the install
    /// (see [`TerrariaContent::locate`]). A pack dir is valid if it contains `pack.txt`.
    pub fn open(
        pack_dir: Option<PathBuf>,
        terraria_dir: Option<PathBuf>,
    ) -> Result<Self, AssetError> {
        let mut packs = Vec::new();
        packs.extend(pack_dir);
        if let Some(v) = std::env::var_os(ENV_ASSETS).filter(|v| !v.is_empty()) {
            packs.push(PathBuf::from(v));
        }
        if let Some(dir) = std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.join("assets")))
        {
            packs.push(dir);
        }
        Self::open_in(packs, install_candidates(terraria_dir))
    }

    fn open_in(packs: Vec<PathBuf>, installs: Vec<PathBuf>) -> Result<Self, AssetError> {
        let mut tried = Vec::new();
        for dir in packs {
            if dir.join(PACK_MARKER).is_file() {
                return Ok(Self {
                    root: dir,
                    source: Source::Pack,
                });
            }
            if !tried.contains(&dir) {
                tried.push(dir);
            }
        }
        match Self::locate_in(installs) {
            Ok(found) => Ok(found),
            Err(AssetError::NotFound { tried: t }) => {
                for p in t {
                    if !tried.contains(&p) {
                        tried.push(p);
                    }
                }
                Err(AssetError::NotFound { tried })
            }
            Err(e) => Err(e),
        }
    }

    /// Try each candidate (as given, then with `Content` appended).
    fn locate_in(candidates: Vec<PathBuf>) -> Result<Self, AssetError> {
        let mut tried = Vec::new();
        for cand in candidates {
            for dir in [cand.clone(), cand.join("Content")] {
                if dir.join(MARKER).is_file() {
                    return Ok(Self {
                        root: dir,
                        source: Source::Install,
                    });
                }
                if !tried.contains(&dir) {
                    tried.push(dir);
                }
            }
        }
        Err(AssetError::NotFound { tried })
    }

    pub fn is_pack(&self) -> bool {
        self.source == Source::Pack
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `name` like `"Images/Tiles_0"`; `.xnb` is appended.
    pub fn texture(&self, name: &str) -> Result<RgbaImage, AssetError> {
        if self.is_pack() {
            return crate::pack::read_png(&std::fs::read(self.root.join(format!("{name}.png")))?);
        }
        let bytes = std::fs::read(self.root.join(format!("{name}.xnb")))?;
        decode_texture(&read_xnb(&bytes)?)
    }

    pub fn sound(&self, name: &str) -> Result<SoundEffect, AssetError> {
        if self.is_pack() {
            return crate::pack::parse_wav(&std::fs::read(self.root.join(format!("{name}.wav")))?);
        }
        let bytes = std::fs::read(self.root.join(format!("{name}.xnb")))?;
        decode_sound(&read_xnb(&bytes)?)
    }

    /// `name` like `"Fonts/Mouse_Text"`; pack: `<name>.png` + `<name>.font`, install: `.xnb`.
    pub fn font(&self, name: &str) -> Result<SpriteFont, AssetError> {
        if self.is_pack() {
            let atlas =
                crate::pack::read_png(&std::fs::read(self.root.join(format!("{name}.png")))?)?;
            let text = std::fs::read_to_string(self.root.join(format!("{name}.font")))?;
            return crate::font::parse_font(&text, atlas);
        }
        let bytes = std::fs::read(self.root.join(format!("{name}.xnb")))?;
        decode_sprite_font(&read_xnb(&bytes)?)
    }

    /// Read music track bytes (WAV or OGG). In pack mode reads `Music/<name>.{wav,ogg}`.
    /// In install mode, decodes track from `Wave Bank.xwb`.
    pub fn music(&self, name: &str) -> Result<Vec<u8>, AssetError> {
        if self.is_pack() {
            let wav_path = self.root.join(format!("Music/{name}.wav"));
            if wav_path.is_file() {
                return Ok(std::fs::read(wav_path)?);
            }
            let ogg_path = self.root.join(format!("Music/{name}.ogg"));
            if ogg_path.is_file() {
                return Ok(std::fs::read(ogg_path)?);
            }
            return Err(AssetError::NotFound {
                tried: vec![wav_path, ogg_path],
            });
        }

        let bank_path = self.root.join("Wave Bank.xwb");
        if !bank_path.is_file() {
            return Err(AssetError::NotFound {
                tried: vec![bank_path],
            });
        }
        let track = crate::music::MUSIC_TRACKS
            .iter()
            .find(|t| t.id == name || t.file_stem == name)
            .ok_or_else(|| AssetError::Malformed(format!("unknown music track {name}")))?;

        let mut file = std::fs::File::open(&bank_path)?;
        let file_len = file.metadata()?.len();
        let entries = crate::read_wave_bank_index_from_reader(&mut file, file_len)?;
        let entry = entries.get(track.entry).ok_or_else(|| {
            AssetError::Malformed(format!("track entry {} out of range", track.entry))
        })?;

        use std::io::{Read, Seek, SeekFrom};
        file.seek(SeekFrom::Start(entry.offset as u64))?;
        let mut bytes = vec![0u8; entry.length as usize];
        file.read_exact(&mut bytes)?;

        match entry.format {
            crate::WaveFormat::Adpcm {
                channels,
                rate,
                block_align,
            } => {
                let samples = crate::decode_ms_adpcm(&bytes, channels, block_align)?;
                Ok(crate::pcm_to_wav(&samples, channels, rate))
            }
            crate::WaveFormat::Pcm {
                channels,
                rate,
                bits,
            } if bits == 16 => {
                let mut samples = Vec::with_capacity(bytes.len() / 2);
                for chunk in bytes.chunks_exact(2) {
                    samples.push(i16::from_le_bytes([chunk[0], chunk[1]]));
                }
                Ok(crate::pcm_to_wav(&samples, channels, rate))
            }
            _ => Err(AssetError::UnsupportedFormat(format!(
                "unsupported track format {:?}",
                entry.format
            ))),
        }
    }
}

const PACK_MARKER: &str = "pack.txt";

fn install_candidates(override_dir: Option<PathBuf>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    candidates.extend(override_dir);
    if let Some(v) = std::env::var_os(ENV_OVERRIDE).filter(|v| !v.is_empty()) {
        candidates.push(PathBuf::from(v));
    }
    candidates.extend(steam_candidates());
    candidates
}

fn steam_candidates() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = std::env::home_dir() {
        roots.push(home.join(".local/share/Steam"));
        roots.push(home.join(".steam/steam"));
        roots.push(home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"));
    }
    roots.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
    roots.push(PathBuf::from(r"C:\Program Files\Steam"));

    let mut libs: Vec<PathBuf> = Vec::new();
    for root in roots {
        let vdf = std::fs::read_to_string(root.join("steamapps/libraryfolders.vdf"))
            .map(|s| parse_library_folders(&s))
            .unwrap_or_default();
        for l in std::iter::once(root).chain(vdf) {
            if !libs.contains(&l) {
                libs.push(l);
            }
        }
    }
    libs.into_iter()
        .map(|l| l.join("steamapps/common/Terraria/Content"))
        .collect()
}

/// Every `"path"` value in Steam's `libraryfolders.vdf` (backslash escapes resolved).
pub fn parse_library_folders(vdf: &str) -> Vec<PathBuf> {
    let mut tokens = Vec::new();
    let mut chars = vdf.chars();
    while let Some(c) = chars.next() {
        if c != '"' {
            continue;
        }
        let mut s = String::new();
        while let Some(c) = chars.next() {
            match c {
                '"' => break,
                '\\' => match chars.next() {
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some(o) => s.push(o),
                    None => break,
                },
                o => s.push(o),
            }
        }
        tokens.push(s);
    }
    tokens
        .windows(2)
        .filter(|w| w[0] == "path")
        .map(|w| PathBuf::from(&w[1]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_vdf_paths() {
        let vdf = r#""libraryfolders" { "0" { "path" "/home/a/.local/share/Steam" } "1" { "path" "D:\\Games\\Steam" } }"#;
        assert_eq!(
            parse_library_folders(vdf),
            vec![
                PathBuf::from("/home/a/.local/share/Steam"),
                PathBuf::from("D:\\Games\\Steam")
            ]
        );
    }

    #[test]
    fn locate_reports_tried_paths() {
        let err = TerrariaContent::locate_in(vec!["/nonexistent/x".into()]).unwrap_err();
        let AssetError::NotFound { tried } = &err else {
            panic!("expected NotFound")
        };
        assert!(tried.iter().any(|p| p.starts_with("/nonexistent/x")));
        let msg = err.to_string();
        assert!(msg.contains("/nonexistent/x"));
        assert!(msg.contains("--terraria-dir"));
        assert!(msg.contains(ENV_OVERRIDE));
    }

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tv_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn pack_runs_without_install() {
        let d = tmp("pack_no_install");
        std::fs::write(d.join("pack.txt"), "x").unwrap();
        let c =
            TerrariaContent::open(Some(d.clone()), Some("/nonexistent/terraria".into())).unwrap();
        assert!(c.is_pack());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn open_prefers_pack() {
        let d = tmp("open_pack");
        std::fs::create_dir_all(d.join("Images")).unwrap();
        std::fs::write(d.join("pack.txt"), "terravibe asset pack v1\n").unwrap();
        std::fs::write(d.join("Images/Tiles_0.png"), b"x").unwrap();
        let c = TerrariaContent::open(Some(d.clone()), None).unwrap();
        assert!(c.is_pack());
        assert_eq!(c.root(), d);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn open_reports_pack_paths() {
        let err = TerrariaContent::open_in(
            vec!["/nonexistent/pack".into()],
            vec!["/nonexistent/inst".into()],
        )
        .unwrap_err();
        let AssetError::NotFound { tried } = &err else {
            panic!("expected NotFound")
        };
        assert!(tried.iter().any(|p| p.starts_with("/nonexistent/pack")));
        assert!(tried.iter().any(|p| p.starts_with("/nonexistent/inst")));
        let msg = err.to_string();
        assert!(msg.contains("--assets-dir"));
        assert!(msg.contains(ENV_ASSETS));
    }

    #[test]
    fn invalid_candidate_does_not_stop_search() {
        let base = std::env::temp_dir().join(format!("tv_locate_skip_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("Images")).unwrap();
        std::fs::write(base.join("Images/Tiles_0.xnb"), b"x").unwrap();
        let found =
            TerrariaContent::locate_in(vec!["/nonexistent/x".into(), base.clone()]).unwrap();
        assert_eq!(found.root(), base);
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn override_accepts_parent_dir() {
        let base = std::env::temp_dir().join(format!("tv_locate_parent_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("Content/Images")).unwrap();
        std::fs::write(base.join("Content/Images/Tiles_0.xnb"), b"x").unwrap();
        let found = TerrariaContent::locate(Some(base.clone())).unwrap();
        assert!(found.root().ends_with("Content"));
        std::fs::remove_dir_all(&base).unwrap();
    }
}
