//! Character storage on disk and migration from legacy per-world player saves.

use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use terravibe_sim::character::{Appearance, Character, Difficulty};
use terravibe_sim::item::Prefix;

use crate::save::{PlayerSave, atomic_write, sanitize};

/// Summary of an available character on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterInfo {
    pub name: String,
    pub modified: SystemTime,
}

#[derive(Debug, PartialEq, Eq)]
pub enum StoreError {
    NotFound,
    Corrupt,
    Io(String),
}

impl From<io::Error> for StoreError {
    fn from(e: io::Error) -> Self {
        if e.kind() == io::ErrorKind::NotFound {
            StoreError::NotFound
        } else {
            StoreError::Io(e.to_string())
        }
    }
}

/// Root directory where character `.plr` files live:
/// `$XDG_DATA_HOME/terravibe/players`, falling back to `~/.local/share/terravibe/players`.
pub fn characters_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(|h| PathBuf::from(h).join(".local/share"))
                .unwrap_or_else(|| PathBuf::from("."))
        });
    base.join("terravibe/players")
}

/// Lists all valid characters under `root`, sorted newest first by last modified time.
/// Missing directories return an empty list; corrupt files are skipped.
pub fn list_characters(root: &Path) -> io::Result<Vec<CharacterInfo>> {
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut list = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if p.extension().is_some_and(|ext| ext == "plr") {
            if let Ok(bytes) = std::fs::read(&p) {
                if let Ok(c) = Character::from_bytes(&bytes) {
                    let modified = entry
                        .metadata()
                        .and_then(|m| m.modified())
                        .unwrap_or(SystemTime::UNIX_EPOCH);
                    list.push(CharacterInfo {
                        name: c.name,
                        modified,
                    });
                }
            }
        }
    }
    list.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(list)
}

/// Loads a character by name from `<root>/<sanitized>.plr`.
pub fn load_character(root: &Path, name: &str) -> Result<Character, StoreError> {
    let safe = sanitize(name, "player");
    let p = root.join(format!("{safe}.plr"));
    let bytes = std::fs::read(&p)?;
    Character::from_bytes(&bytes).map_err(|_| StoreError::Corrupt)
}

/// Atomically saves a character to `<root>/<sanitized>.plr`.
pub fn save_character(root: &Path, c: &Character) -> io::Result<()> {
    std::fs::create_dir_all(root)?;
    let safe = sanitize(&c.name, "player");
    let p = root.join(format!("{safe}.plr"));
    let bytes = c.to_bytes();
    atomic_write(&p, &bytes)
}

/// Deletes a character file `<root>/<sanitized>.plr`. Returns `Ok(())` if already missing.
pub fn delete_character(root: &Path, name: &str) -> io::Result<()> {
    let safe = sanitize(name, "player");
    let p = root.join(format!("{safe}.plr"));
    match std::fs::remove_file(p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Report of characters imported from legacy per-world saves.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    pub imported: Vec<(
        String, /* world */
        String, /* player */
        String, /* character */
    )>,
    pub skipped: usize,
}

/// Scans `<worlds_root>` for legacy `players/<name>.bin` saves and imports them
/// as characters under `<chars_root>`. Writes `<chars_root>/.migrated-v1` so it
/// executes only once. Original world saves remain untouched.
pub fn migrate_world_players(worlds_root: &Path, chars_root: &Path) -> io::Result<MigrationReport> {
    let marker = chars_root.join(".migrated-v1");
    if marker.exists() {
        return Ok(MigrationReport::default());
    }

    let entries = match std::fs::read_dir(worlds_root) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            std::fs::create_dir_all(chars_root)?;
            atomic_write(&marker, b"v1\n")?;
            return Ok(MigrationReport::default());
        }
        Err(e) => return Err(e),
    };

    std::fs::create_dir_all(chars_root)?;
    let mut report = MigrationReport::default();
    let mut known_names: HashSet<String> = list_characters(chars_root)?
        .into_iter()
        .map(|c| c.name)
        .collect();

    // Iterate over world directories
    for world_entry in entries.flatten() {
        let world_path = world_entry.path();
        if !world_path.is_dir() {
            continue;
        }
        let world_name = world_entry.file_name().to_string_lossy().into_owned();
        let players_dir = world_path.join("players");
        let player_entries = match std::fs::read_dir(players_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for pe in player_entries.flatten() {
            let p_path = pe.path();
            if p_path.extension().is_some_and(|ext| ext == "bin") {
                let p_raw_name = p_path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                let bytes = match std::fs::read(&p_path) {
                    Ok(b) => b,
                    Err(_) => {
                        report.skipped += 1;
                        continue;
                    }
                };
                let ps = match PlayerSave::from_bytes(&bytes) {
                    Ok(ps) => ps,
                    Err(_) => {
                        report.skipped += 1;
                        continue;
                    }
                };

                let base_name = sanitize(&p_raw_name, "player");
                let mut chosen = base_name.clone();

                if known_names.contains(&chosen) {
                    // Try world suffix first
                    let with_world = format!("{}-{}", base_name, sanitize(&world_name, "w"));
                    if !known_names.contains(&with_world) {
                        chosen = with_world;
                    } else {
                        // Suffix numbering
                        let mut suffix = 2;
                        loop {
                            let candidate = format!("{}-{}", base_name, suffix);
                            if !known_names.contains(&candidate) {
                                chosen = candidate;
                                break;
                            }
                            suffix += 1;
                        }
                    }
                }

                known_names.insert(chosen.clone());

                let character = Character {
                    name: chosen.clone(),
                    difficulty: Difficulty::Softcore,
                    appearance: Appearance::default(),
                    inventory: ps.inventory,
                    armor: ps.armor,
                    accessories: [None; 5],
                    accessory_prefixes: [Prefix(0); 5],
                    vanity_armor: [None; 3],
                    vanity_accessories: [None; 5],
                    vanity_accessory_prefixes: [Prefix(0); 5],
                    life_max: ps.life_max,
                    life: ps.life,
                    mana_max: 20,
                    mana: 20,
                    potion_delay: ps.potion_delay,
                    facts: Default::default(),
                    obsidian_skin_ticks: 0,
                };

                save_character(chars_root, &character)?;
                report
                    .imported
                    .push((world_name.clone(), p_raw_name, chosen));
            }
        }
    }

    atomic_write(&marker, b"v1\n")?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::{SaveDir, WorldSnapshot};
    use crate::worlds::create_world;
    use terravibe_sim::WorldSize;
    use terravibe_sim::inventory::Inventory;
    use terravibe_sim::item::{COPPER_PICKAXE, ItemId, Prefix, Rng};

    struct TempGuard(PathBuf);
    impl Drop for TempGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn tmp(tag: &str) -> (PathBuf, TempGuard) {
        let p = std::env::temp_dir().join(format!("tv_char_{}_{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        (p.clone(), TempGuard(p))
    }

    #[test]
    fn save_load_delete_roundtrip() {
        let (root, _g) = tmp("roundtrip");
        let c = Character::new("Adventurer", Appearance::default());
        save_character(&root, &c).expect("save");

        let loaded = load_character(&root, "Adventurer").expect("load");
        assert_eq!(c, loaded);

        delete_character(&root, "Adventurer").expect("delete");
        assert_eq!(
            load_character(&root, "Adventurer"),
            Err(StoreError::NotFound)
        );
    }

    #[test]
    fn list_skips_corrupt_and_orders_newest_first() {
        let (root, _g) = tmp("list");
        let c1 = Character::new("Alice", Appearance::default());
        save_character(&root, &c1).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(15));
        let c2 = Character::new("Bob", Appearance::default());
        save_character(&root, &c2).unwrap();

        // Write a corrupt file
        std::fs::write(root.join("corrupt.plr"), b"bad bytes").unwrap();

        let list = list_characters(&root).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "Bob");
        assert_eq!(list[1].name, "Alice");
    }

    #[test]
    fn delete_missing_is_ok() {
        let (root, _g) = tmp("del_missing");
        assert!(delete_character(&root, "non_existent").is_ok());
    }

    #[test]
    fn path_traversal_names_sanitized() {
        let (root, _g) = tmp("traversal");
        let mut c = Character::new("Hero", Appearance::default());
        c.name = "../evil".into();
        save_character(&root, &c).unwrap();
        assert!(root.join("evil.plr").exists());
        assert!(!root.parent().unwrap().join("evil.plr").exists());
    }

    #[test]
    fn migrate_keeps_inventory_armor_life_potion_delay() {
        let (root, _g) = tmp("migrate_basic");
        let worlds = root.join("worlds");
        let chars = root.join("chars");

        create_world(&worlds, "World1", 42, WorldSize::Small).unwrap();
        let save_dir = SaveDir::open(&worlds, "World1").unwrap();

        let mut inv = Inventory::empty();
        inv.slots[0] = Some(terravibe_sim::inventory::ItemStack {
            id: COPPER_PICKAXE,
            count: 1,
            prefix: Prefix(0),
        });
        let ps = PlayerSave {
            inventory: inv.clone(),
            armor: [Some(ItemId(10)), None, None],
            life_max: 120,
            life: 90,
            pos: [10.0, 20.0, 30.0],
            potion_delay: 45,
        };

        let snap = WorldSnapshot {
            seed: 42,
            clock_tick: 100,
            size: WorldSize::Small,
            chunks: vec![],
            players: vec![("player1".into(), ps.clone())],
            chests: vec![],
            towns: Default::default(),
            facts: Default::default(),
            weather: Default::default(),
            locked_chests: vec![],
            shadow_chests: vec![],
        };
        save_dir.save_world(&snap).unwrap();

        let rep = migrate_world_players(&worlds, &chars).unwrap();
        assert_eq!(rep.imported.len(), 1);
        assert_eq!(
            rep.imported[0],
            ("World1".into(), "player1".into(), "player1".into())
        );

        let c = load_character(&chars, "player1").unwrap();
        assert_eq!(c.name, "player1");
        assert_eq!(c.inventory, inv);
        assert_eq!(c.armor, [Some(ItemId(10)), None, None]);
        assert_eq!(c.life_max, 120);
        assert_eq!(c.life, 90);
        assert_eq!(c.potion_delay, 45);
        assert_eq!(c.mana_max, 20);
        assert_eq!(c.mana, 20);
    }

    #[test]
    fn migrate_leaves_world_saves_untouched() {
        let (root, _g) = tmp("migrate_untouched");
        let worlds = root.join("worlds");
        let chars = root.join("chars");

        create_world(&worlds, "World1", 42, WorldSize::Small).unwrap();
        let save_dir = SaveDir::open(&worlds, "World1").unwrap();
        let ps = PlayerSave {
            inventory: Inventory::starting(&mut Rng::new(1)),
            armor: [None; 3],
            life_max: 100,
            life: 100,
            pos: [0.0, 0.0, 0.0],
            potion_delay: 0,
        };
        save_dir
            .save_world(&WorldSnapshot {
                seed: 42,
                clock_tick: 0,
                size: WorldSize::Small,
                chunks: vec![],
                players: vec![("p".into(), ps)],
                chests: vec![],
                towns: Default::default(),
                facts: Default::default(),
                weather: Default::default(),
                locked_chests: vec![],
                shadow_chests: vec![],
            })
            .unwrap();

        let p_bin = worlds.join("World1/players/p.bin");
        let bytes_before = std::fs::read(&p_bin).unwrap();

        migrate_world_players(&worlds, &chars).unwrap();

        let bytes_after = std::fs::read(&p_bin).unwrap();
        assert_eq!(bytes_before, bytes_after);
    }

    #[test]
    fn migrate_name_collisions_get_suffix_no_overwrite() {
        let (root, _g) = tmp("migrate_collision");
        let worlds = root.join("worlds");
        let chars = root.join("chars");

        create_world(&worlds, "W1", 1, WorldSize::Small).unwrap();
        create_world(&worlds, "W2", 2, WorldSize::Small).unwrap();

        let ps = PlayerSave {
            inventory: Inventory::empty(),
            armor: [None; 3],
            life_max: 100,
            life: 100,
            pos: [0.0, 0.0, 0.0],
            potion_delay: 0,
        };

        SaveDir::open(&worlds, "W1")
            .unwrap()
            .save_world(&WorldSnapshot {
                seed: 1,
                clock_tick: 0,
                size: WorldSize::Small,
                chunks: vec![],
                players: vec![("same".into(), ps.clone())],
                chests: vec![],
                towns: Default::default(),
                facts: Default::default(),
                weather: Default::default(),
                locked_chests: vec![],
                shadow_chests: vec![],
            })
            .unwrap();

        SaveDir::open(&worlds, "W2")
            .unwrap()
            .save_world(&WorldSnapshot {
                seed: 2,
                clock_tick: 0,
                size: WorldSize::Small,
                chunks: vec![],
                players: vec![("same".into(), ps)],
                chests: vec![],
                towns: Default::default(),
                facts: Default::default(),
                weather: Default::default(),
                locked_chests: vec![],
                shadow_chests: vec![],
            })
            .unwrap();

        let rep = migrate_world_players(&worlds, &chars).unwrap();
        assert_eq!(rep.imported.len(), 2);
        let names: Vec<_> = rep.imported.iter().map(|(_, _, c)| c.as_str()).collect();
        assert!(names.contains(&"same"));
        assert!(
            names.contains(&"same-W2") || names.contains(&"same-W1") || names.contains(&"same-2")
        );
    }

    #[test]
    fn migrate_runs_once_marker() {
        let (root, _g) = tmp("migrate_once");
        let worlds = root.join("worlds");
        let chars = root.join("chars");

        create_world(&worlds, "W1", 1, WorldSize::Small).unwrap();
        SaveDir::open(&worlds, "W1")
            .unwrap()
            .save_world(&WorldSnapshot {
                seed: 1,
                clock_tick: 0,
                size: WorldSize::Small,
                chunks: vec![],
                players: vec![(
                    "p".into(),
                    PlayerSave {
                        inventory: Inventory::empty(),
                        armor: [None; 3],
                        life_max: 100,
                        life: 100,
                        pos: [0.0, 0.0, 0.0],
                        potion_delay: 0,
                    },
                )],
                chests: vec![],
                towns: Default::default(),
                facts: Default::default(),
                weather: Default::default(),
                locked_chests: vec![],
                shadow_chests: vec![],
            })
            .unwrap();

        let rep1 = migrate_world_players(&worlds, &chars).unwrap();
        assert_eq!(rep1.imported.len(), 1);

        // Delete the character from chars_root, but leave .migrated-v1
        delete_character(&chars, "p").unwrap();

        // Second run must be a no-op because marker exists
        let rep2 = migrate_world_players(&worlds, &chars).unwrap();
        assert_eq!(rep2.imported.len(), 0);
        assert_eq!(load_character(&chars, "p"), Err(StoreError::NotFound));
    }
}
