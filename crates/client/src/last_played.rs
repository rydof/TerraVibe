//! Most recently played character and world tracking for quick continuation.

use std::fs;
use std::io;
use std::path::Path;

use bevy::prelude::*;
use terravibe_server::save::atomic_write;
use terravibe_sim::codec::{Reader, Writer};

pub const LAST_PLAYED_MAGIC: &[u8; 4] = b"TVLP";
pub const LAST_PLAYED_VERSION: u8 = 1;

#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct LastPlayed {
    pub last_character: Option<String>,
    pub last_world: Option<String>,
}

impl LastPlayed {
    pub fn load(root: &Path) -> Self {
        let path = root.join("last_played.bin");
        let bytes = match fs::read(path) {
            Ok(b) => b,
            Err(_) => return Self::default(),
        };

        if bytes.len() < 5 || &bytes[0..4] != LAST_PLAYED_MAGIC || bytes[4] != LAST_PLAYED_VERSION {
            return Self::default();
        }

        let mut r = Reader(&bytes[5..]);
        let read_opt_str = |r: &mut Reader| -> Option<String> {
            let has_val = r.u8().ok()? != 0;
            if !has_val {
                return None;
            }
            let len = r.u16().ok()? as usize;
            let slice = r.take(len).ok()?;
            std::str::from_utf8(slice).ok().map(|s| s.to_string())
        };

        let last_character = read_opt_str(&mut r);
        let last_world = read_opt_str(&mut r);

        Self {
            last_character,
            last_world,
        }
    }

    pub fn save(&self, root: &Path) -> io::Result<()> {
        let mut w = Writer::default();
        w.bytes(LAST_PLAYED_MAGIC);
        w.u8(LAST_PLAYED_VERSION);

        let mut write_opt_str = |opt: &Option<String>| {
            if let Some(s) = opt {
                w.u8(1);
                let bytes = s.as_bytes();
                w.u16(bytes.len() as u16);
                w.bytes(bytes);
            } else {
                w.u8(0);
            }
        };

        write_opt_str(&self.last_character);
        write_opt_str(&self.last_world);

        fs::create_dir_all(root)?;
        atomic_write(&root.join("last_played.bin"), &w.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct TempDirGuard(PathBuf);
    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn test_dir(tag: &str) -> (PathBuf, TempDirGuard) {
        let p = std::env::temp_dir().join(format!("tv_lp_{}_{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("create temp dir");
        (p.clone(), TempDirGuard(p))
    }

    #[test]
    fn last_played_save_and_load_roundtrip() {
        let (root, _guard) = test_dir("roundtrip");
        let lp = LastPlayed {
            last_character: Some("Hero".into()),
            last_world: Some("Terra".into()),
        };
        lp.save(&root).unwrap();
        let loaded = LastPlayed::load(&root);
        assert_eq!(loaded, lp);
    }

    #[test]
    fn last_played_defaults_when_missing_or_corrupt() {
        let (root, _guard) = test_dir("corrupt");
        assert_eq!(LastPlayed::load(&root), LastPlayed::default());

        std::fs::write(root.join("last_played.bin"), b"junk").unwrap();
        assert_eq!(LastPlayed::load(&root), LastPlayed::default());
    }

    #[test]
    fn last_played_handles_nones() {
        let (root, _guard) = test_dir("nones");
        let lp = LastPlayed {
            last_character: None,
            last_world: Some("Terra".into()),
        };
        lp.save(&root).unwrap();
        assert_eq!(LastPlayed::load(&root), lp);
    }
}
