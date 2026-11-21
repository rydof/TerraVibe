//! World catalogue: list, create and delete save directories by name.
//!
//! Reads only `world.bin` headers and mtimes — never chunks or players — so
//! listing stays cheap and works for damaged worlds too.

use std::cmp::Reverse;
use std::io;
use std::path::Path;
use std::time::SystemTime;

use crate::save::{atomic_write, decode_world_header, encode_world_header};
use terravibe_sim::WorldSize;

/// Clock of a new world: `Main.time = 13500` (8:15 AM), the same fresh-start
/// value the server uses.
const NEW_CLOCK_TICKS: u32 = 13_500;

/// One catalogue row: a save directory with a `world.bin` header.
#[derive(Debug)]
pub struct WorldInfo {
    pub name: String,
    pub seed: u32,
    pub clock_tick: u32,
    pub size: WorldSize,
    /// `world.bin` mtime: when the world was last saved.
    pub last_played: SystemTime,
    /// The header is unreadable: the row is shown, but cannot be played.
    pub damaged: bool,
}

/// Why a world name is unusable.
#[derive(Debug, PartialEq, Eq)]
pub enum NameError {
    Empty,
    TooLong,
    Illegal(char),
}

impl From<NameError> for io::Error {
    fn from(e: NameError) -> Self {
        io::Error::new(io::ErrorKind::InvalidInput, format!("{e:?}"))
    }
}

/// Single source of truth for world names: trimmed, 1–32 chars, ASCII
/// letters/digits/space/`_`/`-`. Path separators, `.`/`..`, control chars and
/// non-ASCII are refused, so a validated name is always a safe path component.
pub fn validate_name(name: &str) -> Result<&str, NameError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(NameError::Empty);
    }
    if name.chars().count() > 32 {
        return Err(NameError::TooLong);
    }
    for c in name.chars() {
        if !(c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '-')) {
            return Err(NameError::Illegal(c));
        }
    }
    Ok(name)
}

/// Every world under `root`: healthy rows by `last_played` descending, then
/// damaged rows by name ascending. A missing root or a directory without
/// `world.bin` is skipped; nothing but the header and its mtime is read.
pub fn list_worlds(root: &Path) -> io::Result<Vec<WorldInfo>> {
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        // No saves yet: an empty menu, not an error.
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut healthy = Vec::new();
    let mut damaged = Vec::new();
    for entry in entries.flatten() {
        let wb = entry.path().join("world.bin");
        let Ok(bytes) = std::fs::read(&wb) else {
            continue; // no world.bin: not a world
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let (seed, clock_tick, size, is_damaged) = match decode_world_header(&bytes) {
            Ok((seed, clock_tick, size)) => (seed, clock_tick, size, false),
            // Shown but not playable; still deletable.
            Err(_) => (0, 0, WorldSize::Small, true),
        };
        let row = WorldInfo {
            name,
            seed,
            clock_tick,
            size,
            last_played: mtime(&wb),
            damaged: is_damaged,
        };
        if is_damaged {
            damaged.push(row);
        } else {
            healthy.push(row);
        }
    }
    healthy.sort_by_key(|r| Reverse(r.last_played));
    damaged.sort_by(|a, b| a.name.cmp(&b.name));
    healthy.extend(damaged);
    Ok(healthy)
}

/// Creates `<root>/<name>/{chunks,players}` and a fresh `world.bin` header
/// (seed, clock 13_500, size); never generates chunks. Refuses a name differing from
/// an existing world only by case, and never escapes the root.
pub fn create_world(root: &Path, name: &str, seed: u32, size: WorldSize) -> io::Result<WorldInfo> {
    let name = validate_name(name)?;
    match std::fs::read_dir(root) {
        Ok(entries) => {
            for entry in entries.flatten() {
                if entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(name)
                {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        format!("world {name:?} already exists"),
                    ));
                }
            }
        }
        // A fresh root has no siblings to collide with.
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let dir = root.join(name);
    std::fs::create_dir_all(dir.join("chunks"))?;
    std::fs::create_dir_all(dir.join("players"))?;
    let wb = dir.join("world.bin");
    atomic_write(&wb, &encode_world_header(seed, NEW_CLOCK_TICKS, size))?;
    Ok(WorldInfo {
        name: name.into(),
        seed,
        clock_tick: NEW_CLOCK_TICKS,
        size,
        last_played: mtime(&wb),
        damaged: false,
    })
}

/// Removes `<root>/<name>`; deleting a missing world already did its job.
pub fn delete_world(root: &Path, name: &str) -> io::Result<()> {
    let name = validate_name(name)?;
    match std::fs::remove_dir_all(root.join(name)) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        res => res,
    }
}

/// `world.bin` mtime: the catalogue's `last_played`.
fn mtime(path: &Path) -> SystemTime {
    path.metadata()
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Unique temp dir per call: tests never touch the real saves.
    fn tmp() -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let d = std::env::temp_dir().join(format!(
            "tv_worlds_{}_{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    /// Rewrites the header as a Task 1 save: version byte 1 (the save.rs pattern).
    fn downgrade_to_v1(p: &Path) {
        let mut b = std::fs::read(p).unwrap();
        b[4] = 1;
        std::fs::write(p, b).unwrap();
    }

    #[test]
    fn validate_name_accepts_spaces_dashes_underscores() {
        assert_eq!(validate_name("My World_2-x"), Ok("My World_2-x"));
    }

    #[test]
    fn validate_name_rejects_empty_too_long_separators_dotdot_controls_nonascii() {
        assert!(matches!(validate_name("  "), Err(NameError::Empty)));
        assert!(matches!(
            validate_name(&"a".repeat(33)),
            Err(NameError::TooLong)
        ));
        for bad in ["a/b", "a\\b", ".", "..", "a\u{7}b", "café"] {
            assert!(validate_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn create_world_rejects_case_insensitive_duplicate() {
        let root = tmp();
        create_world(&root, "My World", 7, WorldSize::Small).unwrap();
        assert_eq!(
            create_world(&root, "MY WORLD", 8, WorldSize::Small)
                .unwrap_err()
                .kind(),
            io::ErrorKind::AlreadyExists
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn create_world_keeps_spaces_and_writes_header() {
        let root = tmp();
        let info = create_world(&root, "My World", 7, WorldSize::Small).unwrap();
        assert!(root.join("My World").is_dir());
        let (seed, tick, size) =
            decode_world_header(&std::fs::read(root.join("My World/world.bin")).unwrap()).unwrap();
        assert_eq!((seed, tick, size), (7, 13_500, WorldSize::Small));
        assert_eq!(info.seed, 7);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn create_world_refuses_traversal() {
        let root = tmp();
        assert!(create_world(&root, "../evil", 1, WorldSize::Small).is_err());
        assert!(!root.parent().unwrap().join("evil").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn list_worlds_orders_by_last_played_desc_skips_and_marks_damaged() {
        let root = tmp();
        create_world(&root, "older", 1, WorldSize::Small).unwrap();
        create_world(&root, "newer", 2, WorldSize::Small).unwrap();
        std::fs::create_dir_all(root.join("noheader")).unwrap(); // skipped: no world.bin
        for name in ["zeta", "alpha"] {
            // damaged rows get the newest mtimes
            std::fs::create_dir_all(root.join(name)).unwrap();
            std::fs::write(root.join(format!("{name}/world.bin")), b"junk").unwrap();
        }
        let rows = list_worlds(&root).unwrap(); // healthy by last_played desc, then damaged by name asc
        assert_eq!(
            rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
            ["newer", "older", "alpha", "zeta"]
        );
        assert!(!rows[0].damaged && !rows[1].damaged && rows[2].damaged && rows[3].damaged);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn v1_header_lists_healthy() {
        // Rewrite header to version byte 1, like the existing save.rs v1 test.
        let root = tmp();
        create_world(&root, "old", 3, WorldSize::Small).unwrap();
        downgrade_to_v1(&root.join("old/world.bin"));
        let rows = list_worlds(&root).unwrap();
        assert!(!rows[0].damaged);
        assert_eq!(rows[0].seed, 3);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn delete_world_removes_only_that_directory() {
        let root = tmp();
        create_world(&root, "gone", 1, WorldSize::Small).unwrap();
        create_world(&root, "kept", 2, WorldSize::Small).unwrap();
        delete_world(&root, "gone").unwrap();
        delete_world(&root, "gone").unwrap(); // idempotent
        assert!(!root.join("gone").exists() && root.join("kept").is_dir());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn old_header_without_size_loads_as_small() {
        let root = tmp();
        std::fs::create_dir_all(root.join("legacy/chunks")).unwrap();
        std::fs::create_dir_all(root.join("legacy/players")).unwrap();
        // Write an old 13-byte header (magic + version 4 + seed + clock_tick, no size byte)
        let mut w = terravibe_sim::codec::Writer::default();
        w.bytes(b"TVWD");
        w.u8(4);
        w.u32(42);
        w.u32(13_500);
        std::fs::write(root.join("legacy/world.bin"), &w.0).unwrap();

        let rows = list_worlds(&root).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].damaged);
        assert_eq!(rows[0].seed, 42);
        assert_eq!(rows[0].size, terravibe_sim::WorldSize::Small);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn create_world_persists_size() {
        use terravibe_sim::WorldSize;
        let root = tmp();
        for (name, size) in [
            ("w_small", WorldSize::Small),
            ("w_med", WorldSize::Medium),
            ("w_large", WorldSize::Large),
        ] {
            let info = create_world(&root, name, 99, size).unwrap();
            assert_eq!(info.size, size);
        }
        let rows = list_worlds(&root).unwrap();
        for (name, size) in [
            ("w_small", WorldSize::Small),
            ("w_med", WorldSize::Medium),
            ("w_large", WorldSize::Large),
        ] {
            let found = rows.iter().find(|r| r.name == name).unwrap();
            assert_eq!(found.size, size);
        }
        let _ = std::fs::remove_dir_all(root);
    }
}
