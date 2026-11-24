# TerraVibe — Start here

Updated 2026-11-22. This is the single source of truth for what is done, what is next, and
the rules. Everything older lives in `docs/HANDOFF.md` and is history only.

## Current State & Resume Guide (2026-11-22)

### 1. Branch & Pushed Baseline
- **Active branch**: `dev` at `3ed1bc8` (latest pushed baseline: `origin/dev` `b7ead5f`, `origin/master` `9296d03`).
- **UI & Item Icon Rework — Phase 1 (Tasks 1–6 landed on `dev`: `ddd248c`, `ec01b66`, `2d924e9`, `f869201`, `3ed1bc8`)**:
  - Unified obsidian-slate design system (`crates/client/src/ui_theme.rs` re-exported via `hud/style.rs` & `menu/style.rs`), procedural HUD chrome & ghost/action icons (`art/ui_icons.rs`), 32×32 hybrid item icon renderer for all `225` items (`art/item_icons.rs`), upgraded `9×5` Creative Mode browser with all 6 boss summons & hover tooltips (`menu/creative.rs`), and full `hud/` + `menu/` UI polish with `0` legacy `Images/` or `ItemIconSource` matches.
  - Verified: `1,460` passed, `0` failed, `6` ignored (`terravibe_assets`: 51+6; `terravibe_client`: 593; `terravibe_server`: 208; `terravibe_sim`: 602); `cargo fmt --check`, `cargo check --workspace --all-targets` (0 warnings), `python3 -I tools/art/check_art.py` pass; `7/7` headless QA passed (`menu_title`, `first_hour`, `tooltips_economy`, `combat_feedback`, `town_housing`, `held_items`, `minimap`).
- **Pass P1 QA & Bench Audit (`3ed1bc8`, `Quadro T1000` Vulkan)**:
  - `13/13` headless QA scenarios passed (`+6` extra: `death_respawn`, `weather_audio`, `wall_of_flesh`, `accessibility_options`, `boss`, `molten_gear`).
  - `./tools/bench-headless.sh`: `avg_fps=88.4`, `p50_ms=10.86`, `p99_ms=16.97`, `over_16_7=36`, `quads=1,117,783`, `pass=false` (`p99_ms` 0.30ms over 16.67ms).
  - Top Pass P2 polish targets: `menu/diorama.rs` first-frame race, bespoke `.vox` models for iconic shared items, held-bow aim pose, chunk mesh/upload pacing (`p99_ms`), boss bar vs interaction prompt overlap, onboarding card auto-hide on combat/boss, and close-range flyer/enemy camera clipping.
- **Landed & pushed since `4e4177d`**:
  - Terraria-accurate vertical depth bands (Surface `y≈280..320`, Underground `y≈180..279`, Cavern `y≈64..179`, Underworld `y≈0..63`, deep lava `y<=90`, Underworld ruins `y=26..56`).
  - Guide Crafting slot (`SlotRef::Guide`, `GuideSlotShift`, `CloseGuideCrafting`, `ServerMsg::GuideSlot`, live recipe lookup UI) and fixed Welcome toast reappearing.
  - Animated molten lava & rippling water (`chunk_light.wesl`, step/bottom liquid faces, embers) and liquid physics (`is_solid()` flow, Underworld lava basins, chunk-load wake, buckets, `urgent_liquid` re-mesh & smart relighting).
  - Pre-allocated weather meshes and cave/roof rain occlusion (`SkyRoofCache` + underground rain/audio suppression).
  - Verified at `1,438` tests, `116.5 avg_fps`, `8/8` QA scenarios.

### 2. Active Effort: Full Pre-Hardmode Parity Sweep (up to Wall of Flesh) on `dev` — All 5 Waves Complete
- **Wave 1 (Committed on `dev`)**:
  - **9 new blocks**, **24 new items (`ITEMS` = `172`)**, **11 new recipes (`RECIPES` = `83`)**, Floating Islands (`sky_island.rs`), Glowing Mushroom Biome (`mushroom_biome.rs`), 8-slot HUD buffs, potions, Campfire/Heart Lantern auras, Grappling Hook, Starfury, explosives.
- **Wave 2 (Committed on `dev`)**:
  - **37 new items (`ITEMS` = `209`)**, **24 new recipes (`RECIPES` = `107`)**: `Night's Edge`, `Blade of Grass`, `Thorn Chakram`, `Amazon`, Jungle materials (`Jungle Spores`, `Stinger`, `Vine`) & Underground Jungle Shrines (`jungle_shrine.rs`); Meteorite crash crater + `Meteorite Bar` + `Meteor Armor` (0-mana `Space Gun`) + `Blue Phaseblade` + `Star Cannon`; `Water Bolt`, `Sunfury`, `Dark Lance`, `Flamelash`, `Flower of Fire`; `Necro`/`Jungle`/`Bee` Armor; `Rocket Boots` flight, `Spectre Boots`, `Obsidian Shield`, `Cloud in a Balloon`, `Lightning Boots`, `Aglet`, `Anklet of the Wind`, `Feral Claws`, `Shark Tooth Necklace` (+5 armor pen).
- **Wave 3 (Committed on `dev`)**:
  - Town NPCs & Economy (`Nurse` healing/debuff clear + `HealAtNurse`/`NurseHealed`, `Demolitionist`, `Dryad` + live Corruption % report, `Goblin Tinkerer` + Reforge UI tab + `ReforgeItem`/`ReforgeResult`), 7-NPC Housing tab layout.
- **Wave 4 (Committed on `dev`)**:
  - **12 new items (`ITEMS` = `221`)**, **2 new recipes (`RECIPES` = `109`)**: `King Slime` + `Slime Crown` + `Ninja Armor`/`Slime Hook`/`Slimy Saddle`; Underground Jungle **Bee Hives** (`bee_hive.rs`) + `LiquidKind::Honey` regen + `BEE_LARVA` + `Abeemination` + `Queen Bee` + `Beenades`/`Bee Gun`/`Bee Keeper`/`Honey Comb`/`Bee Wax` + homing `BeeShot`.
- **Wave 5 (Implemented & Verified on `dev`)**:
  - **4 new items (`ITEMS` = `225`)**, **1 new recipe (`RECIPES` = `110`)**, **18 new enemies (`KINDS` = `63`)**: `Blood Moon` + `BloodZombie`/`Drippler`/`MoneyTrough`; `Goblin Army` + `GoblinBattleStandard` + 5 Goblin enemies + `Harpoon`/`SpikyBall`; 11 biome enemies (`Antlion`, `Vulture`, `IceSlime`, `SnowFlinx`, `JungleSlime`, `Hornet`, `ManEater`, `Harpy`, `Shark`, `Crab`, `Piranha`).
  - Verified at `1,449` workspace tests (`0` failed), `cargo fmt --check`, `cargo check --workspace --all-targets` (0 warnings), `check_art.py` passing.

### 3. Resume Commands & Critical Invariants
- **Resume**: check `git status` and `git log -n 5 --oneline`, verify Wave 1 commit on `dev`, then execute Waves 2–5 sequentially or via `/swarm` `share` worktrees (always spawning `share` worktrees AFTER the previous wave commits!).
- **Invariants**:
  - `ITEMS.len()` tests in `item/mod.rs`, `contact_sheet.rs`, `tooltip.rs`, `avatar.rs`.
  - Nonzero sell values for non-coin items (`shop.rs`).
  - 3D `.vox`/block visual for every item; `python3 -I tools/art/check_art.py`.
  - `Option<Gizmos>` for Bevy systems in unit tests.
  - Never run concurrent Sway/Vulkan bench or QA runs; never open windows on `DP-1`.
  - Never commit `tools/token-audit.py`, `prompt.txt`, `metrics.json`, `run.log`, `bench_effects_*.png`, `assets/terraria-pack/`.
  - Zero copyrighted assets, decompile refs, or local absolute user paths.

## The project in one paragraph

A first-person/third-person 3D voxel recreation of Terraria in Rust + Bevy 0.20, with an
authoritative server even in single player. This era ends when a player can create a
character and world and defeat the **Wall of Flesh** by Terraria's main pre-Hardmode path.
Design: [roadmap](HANDOFF.md)
(read first), built on the [original design](HANDOFF.md).

## Where things are

- Work in the repository root on branch `master` (2026-10-27: the old wgpu/TypeScript
  implementation was removed and the Bevy project moved here from
  `.claude/worktrees/stage1-relief-spike`; `stage3-playable` is an older alias of the same
  history). Use `/usr/bin/git`.
- `docs/HANDOFF.md`: this file. `docs/reference/game-data.md`: ids, save format, flags,
  tools.
- `docs/HANDOFF.md`: designs still in force. `docs/HANDOFF.md`: the
  five Polish Program pass plans (implemented, kept until committed). Each new milestone adds its own spec and plan here.
- `docs/HANDOFF.md`: finished plans, old status reports, the previous handoff.
- `.superpowers/sdd/`: ignored per-task scratch. Only `reviewer-instructions.md` is kept;
  per-pass scratch dirs are deleted when a pass lands.

## Current state (2026-11-21: Batch 1 + Batch 2 + Swarm Polish/Perf/UI/Feel Waves 1–5 + Wave A/B/C Verified)

**Latest pushed commit:** `4e4177d` (menus + 3D character preview, slime/eye runaway & dawn fixes, 3D axes).
**Uncommitted working tree:** All 30 side-by-side Terraria Classic Pre-Hardmode parity items (`#1`–`#30` across Batches 1 & 2, WS1–WS4), the single-click block/wall/torch placement fix, adversarial review fixes (`Demon` non-teleport, end-to-end accessory prefixes `62..=80` with `TVCH` v4, and server test fixture alignment), and the multi-agent Swarm Polish/Perf/UI/Feel + Terraria UX parity overhaul (Waves 1–5 + Wave A/B/C) are implemented and verified (see [`2026-11-19-terraria-parity-audit-handoff.md`](HANDOFF.md)).

### Swarm Polish, Performance, UI & Camera Feel (Waves 1–5 + Wave A/B/C Verified)
- **Engine & GPU Performance**:
  - `crates/client/src/art/chunk_mesh.rs`: Greedy relief meshing (`mesh_chunk_greedy`) reduced rendered quads by 38% (`1,042,435` → `646,060`).
  - `crates/client/src/light/shaders/chunk_light.wesl`: WESL `@if(VERTEX_UVS_B)` and compile-time `@if(MAY_DISCARD)` with `textureSampleLevel` so `AlphaMode::Opaque` terrain keeps GPU Early-Z.
  - `crates/client/src/view/chunks.rs`, `light/engine.rs`, `view/block_models.rs`: Balanced `TaskPoolOptions`, moved `LightInput`/`ChunkMeshInput` packing and static block-model batching off the main thread, restored GPU-instanced tree canopies, and budgeted chunk finalizations + light uploads per frame.
  - `crates/client/src/view/sky.rs`: Inward CCW winding with back-face culling and `NotShadowCaster`/`NotShadowReceiver` on `SkyDome`.
- **HUD, Inventory, Menus, Critters & Camera Feel (Waves 1–5 + Wave A/B/C)**:
  - `crates/client/src/input_map.rs`, `gamepad.rs`, `net.rs`: Terraria-accurate keybinds (`M` FullscreenMap, `Tab` Minimap, `H` QuickHeal, `J` QuickMana, `B` QuickBuff, `Esc` Inventory, `T` Throw) and quick-potion hotkeys.
  - `crates/client/src/hud/inventory.rs`, `hud/town.rs`, `view/avatar.rs`, `view/interact.rs`: Vanity Armor (3) & Vanity Accessory (5) slots synced via `ServerMsg::Vanity`, Housing tab inside Inventory (`? Query` + clickable NPC banners), Ruler & Build Grid toggles with 3D `Gizmos`, and `Alt + Click` Favorite slot protection.
  - `crates/client/src/hud/map.rs`, `settings.rs`: 3-mode `Tab` map cycle (`Corner → Fullscreen Transparent Overlay → Off`), 100% edge-to-edge Fullscreen Map (`M`), and legacy `settings.cfg` migration.
  - `crates/sim/`, `crates/server/`, `crates/client/src/menu/creative.rs`: Killable critters (`Bunny`, `Bird`, `Squirrel`, `CaveCritter`), `Bug Net` (`ItemId(1991)`) catching/releasing + 5 `.vox` models (`148` total items, `90` item `.vox` models), Creative Mode spawner (`F2` / Pause Menu: `CreativeGive`, `FullHeal`, `GodMode`, `SetTime` without false dusk boss triggers + cursor ungrab), redesigned Inventory `[Housing]` tab, and slime/bunny/bird/squirrel AI/visual fixes.
  - `crates/sim/src/worldgen/`, `crates/sim/src/boss.rs`, `tools/art/models/`, `crates/client/src/view/`: Natural surface cave mouths (`in_surface_cave_mouth`) and richer surface trees/dressing; Terraria-accurate Eye of Cthulhu natural spawn eligibility (`life_max >= 200`, `defense >= 10`, `>= 3` town NPCs) and 2-stage spin transformation (`state 2 → 4`) with gore/roar/dash FX; remodeled `.vox` weapons/tools, all 6 armor sets, and EoC parts (`55` `.vox` files); rounded player head with 3-tone haircuts; overhauled procedural bosses & mobs (`ProcWing`, `ProcLimb`, `ProcJaw`, `ProcTendril`); weapon scaling, new held animations (`ShortswordStab`, `SpearThrust`, `GunFire`, `WhipSnap`), elemental swing arc particles, and metallic/emissive armor set-bonus FX.
  - `crates/client/src/menu/`: Title & Game Menu z-order, button padding, and Continue truncation fixes.
  - `crates/client/src/view/camera.rs`: Centered 3rd-person shoulder pivot (`SHOULDER_RIGHT = 0.0`, `SHOULDER_UP = 0.15`), occlusion pull-in damping, and sprint FOV.
  - **Git Branch Organization**: Release tags (`v0.1.0`, `v0.5.0`, `v0.8.0`, `v0.10.0`, `v0.11.0`, `v0.12.0`) and milestone merges live on `master` alongside the active `dev` integration branch.
- **Verified Metrics (`Quadro T1000`, Vulkan, 1920×1080)**:
  - **Default terrain bench**: `avg_fps=122.1`, `p50_ms=7.92`, `p99_ms=15.20`, `quads=653,404`, `over_16_7=11`, `startup_ms=2487`, `pass=true`.
  - **`--liquid-stress`**: `avg_fps=116.1`, `p50_ms=8.52`, `p99_ms=14.18`, `quads=625,882`, `over_16_7=3`, `pass=true`.
  - **`--effects`**: `pass=true`.
  - **Headless Visual QA**: `7/7` scenarios passed (`menu_title`, `first_hour`, `tooltips_economy`, `combat_feedback`, `town_housing`, `held_items`, `minimap`).
  - **Workspace Tests**: `1,427` passed (`577` client + `205` server + `588` sim + `57` assets), `0` failed, `6` ignored; `cargo fmt --check`, `cargo check --workspace --all-targets` (0 warnings), and `python3 -I tools/art/check_art.py` all pass (`148` items, `90` item `.vox` models).

Pre-Hardmode baseline built (`c1de20c` + `9d21a0a` + `c30e031` + `4e4177d`):

Built: menus (title with Continue button and seed display, characters create/delete/play with hair & color customization, worlds create/delete/play with size selector and backup recovery, grouped options with Display/Audio/Controls/Accessibility tabs, in-game Game Menu with controls reference and Guide advice, disabled multiplayer with disconnect retry); forest, desert, snow, jungle, corruption and ocean surface biomes; depth bands (Surface, Underground, Cavern, Underworld) with dense 3D cave networks; dynamic lighting, weather (rain, wind, ambient audio), and day/night; situational music with crossfade (all biomes, day/night, underground, underworld, corruption, and all 4 bosses); trees, mining, placing, walls; copper→gold ores, tools, armor; life crystals; crafting at stations (including Furnace recipes for Obsidian Skull, Obsidian/Hellstone Bricks, and Demon Altar Worm Food); chests, doors, furniture, underground cabins, ruined underworld houses; bow, arrows, potions, boomerangs, spears, guns with ammo, flails, magic thorn staff; 5 live accessory slots with core accessories, Cobalt Shield, Obsidian Skull, Lava Charm, Emblems; Guide and Merchant with housing, dialogue chain, and shop; slimes with hopping sounds, zombies, demon eyes, Eater of Souls, Cave Bats, Skeletons, Giant Worms; Eye of Cthulhu with Servants of Cthulhu; Eater of Worlds multi-segment 3D boss with splitting; Old Man & Dungeon Guardian; Skeletron 3D multi-part boss; Angry Bones, Dark Caster, Cursed Skull; Fire Imp, Demon, Voodoo Demon, Lava Slime, Hellbat, 6-segment Bone Serpent; locked golden dungeon chests and keys; locked shadow chests and reusable Shadow Key; saves with rotating `.bak` backup recovery; liquids (water, lava, obsidian, buckets, swimming, drowning, lava burn, Lava Charm protection, Obsidian Skin Potion immunity); finite world bounds (Small/Medium/Large); comprehensive accessibility (UI scale, colorblind palette, high-contrast crosshair, reduced motion), full key rebinding (including Throw `T`), and gamepad parity (D-pad hotkeys, prompts, confirmation); Molten armor & tools; Wall of Flesh multi-part boss, Guide Voodoo doll lava summon, class emblems, endgame weapons, and Victory modal overlay.

### End-to-End Polish & Completeness State (24-Finding Audit Resolved)
- **Boss Wipe & Multiplayer Safety**: Multiplayer WoF tongue pull bounded to Underworld depth (`y <= 52`) to avoid dragging Surface-respawned players; Skeletron and Eater of Worlds despawn cleanly on team wipe with automatic Old Man respawn at the Dungeon entrance if Skeletron remains undefeated.
- **Weapon & Tool Combat Mechanics**: Flails (`Blue Moon`) and `Valor` (`ProjKind::Yoyo` with `VALOR` `3317.vox` visuals) work cleanly; `Magic Missile` steers dynamically toward the player's aim point while held; `Aqua Scepter` fires arcing, 5-pierce `ProjKind::WaterStream` bolts; `Laser Rifle` pierces up to 3 targets; `Firecracker` features 2.75x sweep reach, 50% primary On Fire!, 75% splash explosion and 100% On Fire! on nearby enemies; Tool weapons (`ItemClass::Tool` with `damage > 0`) deal melee damage (Molten Pickaxe with 10% burn, Molten Hamaxe with 20% burn); `Magic Mirror` recalls to spawn with fall damage reset.
- **Crafting & Progression Recipes**: Added recipes 68..=71 for Obsidian Skull (20 Obsidian at Furnace), Obsidian Brick (2 Obsidian at Furnace, with Hellforge also satisfying Furnace), Hellstone Brick (1 Stone + 1 Hellstone at Furnace), and Worm Food (15 Shadow Scale + 15 Ebonstone Block at Demon Altar). Total recipes: 72; items: 143; enemy kinds: 38.
- **HUD & Status Indicators**: 4-slot buff/debuff row (`Potion Sickness`, `On Fire!`, `Obsidian Skin`, `Lava Ward`) with duration timers; Boss bar, title card, roar SFX, and situational boss music for all 4 bosses (`Eye of Cthulhu`, `Eater of Worlds`, `Skeletron`, `Wall of Flesh`); hotbar ammo count filtered by weapon ammo type; colorblind palette applied to hotbar name and pickup toasts.
- **Controls & Gamepad Parity**: Throw Item (`T`) rebindable without UI overlap; D-pad gamepad shortcuts for Throw (D-Left), Equip Armor (D-Down), Housing/Guide (D-Right), and Map (D-Up/Select); controller and keyboard confirmation on Death and Victory screens; cursor ungrab on player death.
- **Session Lifecycle & Audio**: `session_teardown` comprehensively resets all HUD and UI state to prevent state leaks across worlds; window focus loss muting (`mute_on_focus_loss`) respected by block use sounds and enemy hop SFX; firearms bypass bow draw SFX; `kill_tink` covers hard late-game blocks and ores.

### Polish Program State (Passes 1–5 Complete)
Approved spec: `docs/HANDOFF.md`

- **Pass 1: First hour & guidance** (`docs/HANDOFF.md`):
  - Death cause tracking, classic softcore coin drops, death modal & respawn protocol.
  - `WorldFacts` (`TVWF`), `CharacterFacts` v2, `LastPlayed` persistence.
  - Progression-driven Guide advice chain & pause menu advice line.
  - First-spawn onboarding card & contextual interaction prompts.
  - Compass strip, corner minimap (`M`), fullscreen map (`N`), explored map persistence (`map.bin`).
  - Loading stages, autosave indicator, saved toast, quit confirmation dialog.
  - Title Continue button, difficulty descriptions, explicit seed display; controls reference modal in pause menu.
  - `first_hour` QA scenario; `visual_audit` timeout fix.
- **Pass 2: Information & economy** (`docs/HANDOFF.md`):
  - Hover tooltips with item rarity colors, stats, sell values, and buff durations.
  - Coin denominations (`p/g/s/c`) and inventory cash total.
  - Shop buy/sell parity; item pickup toasts; `tooltips_economy` QA scenario.
- **Pass 3: Held items, combat & feedback** (`docs/HANDOFF.md`):
  - Held item 3D meshes with swing, thrust, and aiming animations.
  - Weapon swing trails, floating damage numbers, screen shake with reduced motion toggle, hit SFX; `held_items` and `combat_feedback` QA scenarios.
- **Pass 4: World life & atmosphere** (`docs/HANDOFF.md`):
  - Dynamic weather system (rain, wind, overcast sky); ambient audio and weather sounds.
  - Environmental ground dressing, ambient critters; independent audio bus architecture; `weather_audio` and `town_housing` QA scenarios.
- **Pass 5: Options, accessibility, stability & performance** (`docs/HANDOFF.md`):
  - **Key rebinding & controls**: `InputBindings` (`Copy`) with collision detection, interactive rebinding interface in Options tab, defaults reset.
  - **Gamepad support**: Core gamepad integration (left stick move, right stick look, South jump, RT attack, LT use/interact, North inventory, LB/RB hotbar, D-pad menu nav) with button glyph prompts.
  - **Grouped options**: Tabbed Options menu (`Display`, `Audio`, `Controls`, `Accessibility`). Quality presets (`Low`, `Medium`, `High`), UI scale (`0.75..=1.5`), HUD opacity (`0.2..=1.0`).
  - **Audio buses & focus mute**: Independent sliders for Music, SFX, Ambient, UI; `mute_on_focus_loss` automatically mutes audio on focus loss.
  - **Accessibility**: Colorblind item rarity palette, high-contrast crosshair, reduced motion toggle, configurable tooltip delay.
  - **Save safety & recovery**: Automatic `.bak` save rotation on atomic writes; corrupt primary file fallback loading `.bak`; version migration; "Load backup" button in Worlds menu.
  - **Stability & error recovery**: Panic hook writing crash dumps to `<saves>/crashes/crash_<timestamp>.log`; friendly disconnect screen with disconnect reason and `[Retry]` / `[Back to Menu]`.
  - **Performance & QA**: `PerfCounters` tracking particles, critters, chunk/map rebuild time; `QualityPreset` particle scaling; `bench_effects` stress benchmark; full QA scenario coverage (`death_respawn`, `minimap`, `accessibility_options`, `bench_effects`).

Milestone 11 additions (Wall of Flesh & End-of-Era Playthrough):
- Guide Voodoo Doll Lava Summon & Multi-Part Boss AI (`crates/sim/src/wof.rs`, `enemy.rs`, `crates/server/src/lib.rs`):
  - Dropping Guide Voodoo Doll (267) via `ClientMsg::DropItem` (throw key `T` or clicking outside open inventory) into Underworld lava (`y <= 24`) while Guide is alive kills the Guide and summons Wall of Flesh at nearer world X boundary sweeping across Underworld depth (`y = 0..=24`).
  - Multi-part boss consisting of `WallOfFleshMouth` (113, HP 8000, dmg 50, def 12), two `WallOfFleshEye` (114, HP 8000, dmg 50, def 0), attached `TheHungry` (115, HP 250, detaching at <= 80 HP), and spawned burrowing `Leech` (117, HP 60, cap 3).
  - Shared 8,000 HP pool synchronized across Mouth and Eyes; sweep speed dynamically accelerates from `1.5 px/tick` to `7.5 px/tick` as HP drops.
  - Eyes fire `ProjKind::EyeLaser` targeting players; laser damage scales 22 / 26 / 30 and firing rate accelerates as HP falls.
  - The Tongue & Horrified mechanics drag players who attempt to flee behind the Wall or out of the Underworld back in front with 25 damage every 30 ticks; far-edge crush kills remaining players when Wall reaches opposite edge.
- M11 Endgame Weapons, Emblems & Loot Tables (`crates/sim/src/item/mod.rs`, `player/mod.rs`, `drop.rs`, `interact.rs`):
  - Pwnhammer (367): 80% hammer power, 26 damage, 7.5 knockback, 26 use time, `auto_reuse`.
  - Class Emblems: Warrior Emblem (490, +15% melee damage), Ranger Emblem (491, +15% ranged damage), Sorcerer Emblem (489, +15% magic damage).
  - Breaker Blade (426): 70 melee damage, 8.0 knockback, 35 use time; deals 2.5x damage against targets at or above 90% health.
  - Clockwork Assault Rifle (434): 17 ranged damage, 12 use time, `auto_reuse`; fires 3-round burst consuming only 1 ammo item.
  - Laser Rifle (514): 29 magic damage, 12 use time, 8 mana cost, `auto_reuse`; fires piercing `ProjKind::Laser` through up to 2 enemies.
  - Firecracker (4912): 40 melee damage, 30 use time; 2.75x sweep reach, inflates hitboxes, 50% On Fire! on primary target, 100% On Fire! + 75% splash damage to up to 2 nearby enemies.
  - Guaranteed drops on defeat: 100% Pwnhammer, exactly 1 class emblem (1/3 each), exactly 1 WoF weapon (1/4 each), 5-10 Gold coins, 5-15 Lesser Healing Potions and Hearts clustered at mouth death location.
- 3D `.vox` Models & Client Visuals (`tools/art/models/items.py`, `crates/client/src/view/`):
  - Handcrafted 3D `.vox` models authored and registered for all 8 M11 items: Pwnhammer (367), Breaker Blade (426), Clockwork Assault Rifle (434), Sorcerer Emblem (489), Warrior Emblem (490), Ranger Emblem (491), Laser Rifle (514), Firecracker (4912). Total registered items: 141.
  - 3D procedural mesh rigs for `WallOfFleshMouth` (colossal 64-block vertical sheet spanning Underworld height + ribs + jaw), `WallOfFleshEye` (spherical eye with pupil), `TheHungry`, `Leech`, and laser beams (`ProjKind::Laser`, `ProjKind::EyeLaser`).
- Victory Modal, Protocol & Progression (`crates/client/src/hud/victory.rs`, `boss_bar.rs`, `crates/sim/src/facts.rs`):
  - Defeating Wall of Flesh sets `facts.downed_wall_of_flesh = true`, broadcasts `ServerMsg::WallOfFleshDefeated` and `ServerMsg::WofGone`.
  - Victory modal overlay ("Wall of Flesh Defeated!") offers "Continue Playing" in pre-Hardmode or "Save & Return to Title".
  - World remains strictly pre-Hardmode: no Hardmode ores, no evil biome V spread, no Hardmode enemy spawns.
  - Guide advice updates for Underworld Voodoo Doll summon beforehand and end-of-era congratulations afterward.
  - `progression_chain_end_of_era` end-to-end integration test and `wall_of_flesh` QA scenario.

Milestone 10 additions (Molten Gear):
- Crafting Stations & Bar:
  - `HELLFORGE = BlockId(77)`: existing Underworld house station, recognized by recipe check. `recipe_station_name` reports "Hellforge".
  - Hellstone Bar (ItemId 175; material, max stack 9999, sell 40 silver): 3 Hellstone (174) + 1 Obsidian (173) at a Hellforge.
  - Picking up a Hellforge requires pickaxe power >= 65 (Nightmare Pickaxe).
- Molten Armor & Set Bonus:
  - Molten Helmet (231): head, 8 defense, +7% melee critical chance.
  - Molten Breastplate (232): body, 9 defense, +7% melee damage.
  - Molten Greaves (233): legs, 8 defense, +7% melee speed.
  - Full set: 25 defense, +17% melee damage (7% breastplate + 10% set bonus), +7% melee crit, +7% melee speed.
  - Full set grants `fire_walk()` (immunity to hot-block burning on `HELLSTONE` and `HELLSTONE_BRICK`) and immunity to the On Fire! debuff.
- Molten Weapons & Tools:
  - Molten Pickaxe (122): 12 damage, knockback 2, use 23, pickaxe power 100, 10% On Fire! on hit.
  - Molten Hamaxe (217): 20 damage, knockback 7, use 27, axe power 150, hammer power 70, 20% On Fire! on hit.
  - Fiery Greatsword (121): 40 damage, knockback 6.5, use 40, 50% On Fire!; first swing hit explodes for 75% damage on up to two enemies within 5 blocks, burning them.
  - Molten Fury (120): 31 damage, knockback 2, use 22, shoot speed 8.5; transforms Wooden Arrows into Flaming Arrows.
  - Phoenix Blaster (219): 23 damage, knockback 2, use 10, shoot speed 13, semi-automatic (`auto_reuse`), consumes bullets.
  - Flamarang (119): 32 damage, knockback 8, use 14, pierces enemies, 50% On Fire!.
- 3D `.vox` Models & Assets:
  - Handcrafted 3D `.vox` models authored and registered for all 10 M10 items (119, 120, 121, 122, 175, 217, 219, 231, 232, 233), including the Molten armor model parts.

Milestone 9 additions (Underworld):
- Underworld Layer Generation (`crates/sim/src/worldgen/underworld.rs`, `terrain.rs`, `block.rs`):
  - Bottom layer spanning `y <= UNDERWORLD_TOP_Y` (`UNDERWORLD_TOP_Y = 24`).
  - Stone replaced with Ash (`ASH = BlockId(57)`), interspersed with Hellstone veins (`HELLSTONE = BlockId(58)`) requiring tier 65 pickaxes.
  - Voluminous lava lakes and bottom lava oceans.
  - Ruined house generation in cell grids using `OBSIDIAN_BRICK` (BlockId 75) and `HELLSTONE_BRICK` (BlockId 76), housing Hellforges (`HELLFORGE = BlockId(77)`) and locked Shadow Chests.
- Hazards & Protective Gear (`crates/sim/src/player/`, `crates/sim/src/interact.rs`):
  - Hot blocks (Hellstone, Meteorite) inflict burning DoT unless protected by `p.fire_walk()`.
  - Obsidian Skull (193): equipped accessory granting `fire_walk: true`, preventing burn damage from hot blocks.
  - Lava Charm (906): accessory delaying lava contact damage by 420 ticks (7 seconds).
  - Obsidian Skin Potion (288): consumable granting 21,600 ticks (6 minutes) complete immunity to lava damage and hot block burning.
  - Shadow Chest unlocking: right-clicking locked underworld chests with Shadow Key (329) unlocks them without consuming the key.
- Underworld Enemies & 3D Procedural Rigs (`crates/sim/src/enemy.rs`, `crates/client/src/view/enemies.rs`):
  - Depth-gated spawning: when player `y <= UNDERWORLD_TOP_Y`, natural spawner spawns exclusively from `UNDERWORLD_POOL`.
  - Fire Imp (`EnemyKind::FireImp`, ID 24; HP 70, dmg 30, def 16): teleports and casts Fireball projectiles.
  - Demon (`EnemyKind::Demon`, ID 62; HP 120, dmg 34, def 8): flying demon AI with procedural bat wings.
  - Voodoo Demon (`EnemyKind::VoodooDemon`, ID 66; HP 140, dmg 32, def 10): flying demon holding Guide Voodoo Doll; guaranteed 100% drop of Guide Voodoo Doll (267).
  - Lava Slime (`EnemyKind::LavaSlime`, ID 59; HP 50, dmg 15, def 10): lava-immune hopping slime with magma voxel core.
  - Hellbat (`EnemyKind::Hellbat`, ID 60; HP 46, dmg 35, def 8): fast erratic flying bat.
  - Bone Serpent (`BoneSerpentHead` ID 39, `BoneSerpentBody` ID 40, `BoneSerpentTail` ID 41; HP 300 each): 6-segment burrowing worm with procedural vertebrae segments that follows the player through solid terrain.
- Audio & 3D Voxel Models (`crates/client/src/audio/`, `tools/art/models/items.py`, `crates/client/src/view/`):
  - Underworld ambient music track playing at `y <= UNDERWORLD_TOP_Y`.
  - Handcrafted 3D `.vox` models authored and registered for all 10 M9 items: Ash (172), Obsidian (173), Hellstone (174), Obsidian Brick (192), Obsidian Skull (193), Hellstone Brick (214), Hellforge (221), Guide Voodoo Doll (267), Obsidian Skin Potion (288), Lava Charm (906).

Milestone 8 additions (Dungeon and Skeletron):
- Dungeon Structure Worldgen (`crates/sim/src/block.rs`, `crates/sim/src/worldgen/dungeon.rs`, `terrain.rs`):
  - Placed on outer third of the world opposite Jungle/Snow with surface entrance building, doorway, staircase, and branching subterranean brick chambers.
  - Tile IDs: `BLUE_DUNGEON_BRICK = BlockId(41)`, `GREEN_DUNGEON_BRICK = BlockId(43)`, `PINK_DUNGEON_BRICK = BlockId(44)`, `SPIKES = BlockId(48)`.
  - Spikes placed along room floors and ceilings; locked golden chests distributed throughout subterranean rooms.
- Old Man NPC & Dungeon Guardian Defense (`crates/sim/src/enemy.rs`, `crates/server/src/lib.rs`):
  - Old Man (`EnemyKind::OldMan`, ID 37) stands at dungeon entrance; daytime interaction warns player; night-time interaction curses Old Man and summons Skeletron.
  - Dungeon Guardian (`EnemyKind::DungeonGuardian`, ID 68; HP 9999, damage 9999, speed 20) spawns and charges players entering subterranean dungeon before Skeletron is defeated (`!downed_skeletron`).
- Skeletron 3D Multi-part Boss AI (`crates/sim/src/enemy.rs`, `crates/client/src/view/enemies.rs`):
  - Multi-part boss consisting of `SkeletronHead` (ID 35, HP 4400, dmg 32, def 10) and two independent `SkeletronHand` entities (ID 36, HP 600, dmg 20, def 14).
  - Hands hover flanking head and execute periodic lunging/slapping attacks toward player.
  - Head alternates between hovering and high-speed spinning charge attack with continuous 3D rotation.
  - Defeating head despawns remaining hands, sets server flag `downed_skeletron = true`, drops boss loot, and permits safe subterranean dungeon entry.
- Post-Defeat Enemies & Golden Key Chest Unlocking (`crates/sim/src/drop.rs`, `crates/sim/src/interact.rs`, `crates/server/src/lib.rs`):
  - Post-defeat dungeon spawns: `AngryBones` (ID 31, walker), `DarkCaster` (ID 32, teleporting spellcaster), `CursedSkull` (ID 34, no-clip flying skull).
  - Enemies drop `Bone` (154) and `GoldenKey` (327).
  - Locked golden dungeon chests require `GoldenKey` in player inventory; right-click unlocks chest and consumes 1 key.
- Dungeon Weapons, Cobalt Shield & Handcrafted 3D Voxel Drops (`crates/sim/src/item/`, `tools/art/`):
  - Weapons & gear: `Muramasa` (155, fast auto-swing blue katana), `CobaltShield` (156, accessory giving 1 def and complete knockback immunity `no_knockback = true`), `AquaScepter` (157, magic water stream wand), `Handgun` (164, semi-auto pistol using Musket Balls), `BlueMoon` (163, heavy spiked flail), `MagicMissile` (113, controllable magic spark), `Valor` (3317, dungeon melee weapon), `ShadowKey` (329, reusable shadow chest key), `Bone` (154, material/ammo).
  - Handcrafted 3D `.vox` models authored in `tools/art/models/items.py` and registered for all 10 M8 items (113, 154, 155, 156, 157, 163, 164, 327, 329, 3317).

Milestone 7 additions (Eater of Worlds, Shadow Orbs & Demonite Progression):
- Shadow Orbs & Hammer Tool System (`crates/sim/src/block.rs`, `crates/sim/src/interact.rs`, `crates/sim/src/worldgen/corruption.rs`):
  - `SHADOW_ORB = BlockId(31)` generated in lower pockets of Corruption chasms during worldgen.
  - Tool stat `hammer: i32` added (`Wooden Hammer` ID 196, `The Breaker` ID 104); only hammer strikes damage `SHADOW_ORB`.
  - Breaking Shadow Orbs increments `shadow_orbs_broken` and drops from authentic loot pool:
    - 1st orb: 100% Musket (96) + 100 Musket Balls (97).
    - Subsequent orbs: random 1-of-5 (Musket + 100 Musket Balls, Vilethorn 64, Ball O' Hurt 162, Shadow Orb item 115, Band of Starpower 111).
  - 3rd orb broken (and every multiple of 3) triggers Eater of Worlds summon.
- Weapon Types & Accessory Effects (`crates/sim/src/projectile.rs`, `crates/sim/src/item/mod.rs`):
  - Guns & Bullets: `Musket` (96) consumes `Musket Ball` (97) from inventory ammo slots (54..58), firing high-speed bullet projectiles.
  - Flails: `Ball O' Hurt` (162) throws a spiked flail on a chain that damages enemies and retracts to player.
  - Magic Piercing: `Vilethorn` (64) fires an extending thorny staff projectile penetrating blocks and piercing multiple enemies.
  - Accessories: `Band of Starpower` (111) gives +20 max mana.
  - Boss Summon Consumable: `Worm Food` (70) summons Eater of Worlds when consumed inside Corruption biome.
- 3D Segmented Eater of Worlds Boss AI & Splitting (`crates/sim/src/enemy.rs`, `crates/server/src/lib.rs`):
  - 12-segment burrowing worm with `EaterOfWorldsHead` (NPC 13, 150 HP, 22 dmg, 2 def), `EaterOfWorldsBody` (NPC 14, 150 HP, 13 dmg, 4 def), and `EaterOfWorldsTail` (NPC 15, 150 HP, 11 dmg, 8 def) connected via parent/child indices.
  - Curving 3D burrowing movement steering toward player through blocks and air.
  - Authentic segment death splitting: when an intermediate body segment dies, adjacent segments turn into a new tail and a new head, splitting into independent living worms.
  - Each dying segment drops Demonite Ore (56) (1-3) and Shadow Scale (86) (1-2).
- Demonite / Shadow Armor Progression (`crates/sim/src/player/`, `crates/sim/src/crafting.rs`, `crates/sim/src/interact.rs`):
  - Nightmare Pickaxe (103, pick 65) capable of mining Demonite Ore and Hellstone tier blocks.
  - Full Shadow Armor set (Helmet 102, Scalemail 101, Greaves 100; 19 defense total) grants +15% run speed set bonus with shadow particle trail.
- 3D Voxel Drop Models (`tools/art/models/items.py`, `tools/art/families.py`, `crates/client/src/view/`):
  - Handcrafted 3D `.vox` drop models authored and registered for all 15 M7 items: Demonite Ore (56), Demonite Bar (57), Vilethorn (64), Worm Food (70), Shadow Scale (86), Musket (96), Musket Ball (97), Shadow Greaves (100), Shadow Scalemail (101), Shadow Helmet (102), The Breaker (104), Band of Starpower (111), Shadow Orb (115), Ball O' Hurt (162), Wooden Hammer (196).

Milestone 6 additions (Underground and Cavern Layers):
- Depth bands & 3D cave networks (`crates/sim/src/worldgen/terrain.rs`):
  - `DepthBand::Surface` (`y > surface_y - 25`), `DepthBand::Underground` (`surface_y - 25 >= y > surface_y - 70`), `DepthBand::Cavern` (`y <= surface_y - 70`).
  - Rich, connected 3D cave networks carved via 3D simplex noise with increased density in Caverns.
- Underground cabins & Cavern chest loot (`crates/sim/src/worldgen/chest_gen.rs`, `crates/sim/src/worldgen/terrain.rs`):
  - Underground wooden cabins embedded in cave walls in Underground and Cavern layers with chests and furniture.
  - Cavern chest loot tables: Hermes Boots (54), Cloud in a Bottle (53), Band of Regeneration (49), Magic Mirror (50), Enchanted Boomerang (55), Spear (274), plus bars, potions, and ammo. Ambient breakable pots drop coins, torches, arrows, and bombs.
- 5 Live Accessory Slots & Core Accessory Effects (`crates/sim/src/character.rs`, `crates/sim/src/player/`, `crates/server/`):
  - 5 active accessory slots in character data and UI, synced to server and client via `ServerMsg::Accessories`.
  - Hermes Boots: +100% (`2.0x`) move speed boost after 20 continuous running ticks.
  - Cloud in a Bottle: grants mid-air double jump.
  - Band of Regeneration: +1 HP/s regeneration.
  - Shiny Red Balloon: +30% jump height boost.
  - Lucky Horseshoe: complete immunity to fall damage.
- Cave Enemies (`crates/sim/src/enemy.rs`):
  - Cave Bat (ID 49): erratic flying AI, 16 HP, 13 dmg, 2 def.
  - Skeleton (ID 21): walker AI, 60 HP, 20 dmg, 8 def.
  - Giant Worm (ID 10): multi-segment burrower moving through solid blocks.
  - Depth-gated spawning: cave enemies spawn only in Underground and Cavern layers.
- Boomerangs & Spears with 3D Voxel Drop Models (`crates/sim/src/projectile.rs`, `tools/art/`):
  - Enchanted Boomerang (55): returning projectile curving back to player.
  - Spear (274): piercing directional thrust projectile extending and retracting.
  - Dedicated 3D `.vox` models authored in `tools/art/models/items.py` and registered for all M6 items (Hermes Boots 54, Cloud in a Bottle 53, Band of Regeneration 49, Magic Mirror 50, Shiny Red Balloon 159, Lucky Horseshoe 158, Enchanted Boomerang 55, Spear 274).

Milestone 5 additions (Stage 6: Housing, Town NPCs, Corruption, Mana and Magic):
- Housing 3D adaptation (`crates/sim/src/housing.rs`):
  - 6-connectivity iterative flood fill over passable air cells starting from an interior spot (no diagonal leaks across corners).
  - Every enclosed air cell must have its background wall bit set; opening to un-walled or unloaded air fails the room.
  - Enclosure bounds: solid walls, floor, ceiling, and doors (`DOOR_CLOSED` or `DOOR_OPEN`).
  - Size bounds: min 60 cells, max 2250 cells (scaled 3× from vanilla's 750 for 3D depth), bounding box span < 100.
  - Furniture requirements: chair, table, door, light source (torch). Must contain >= 1 column of standing space (2 air blocks above solid/platform floor).
  - Server rate limiting: room checks run off the per-tick path, scanned on demand and on a 2 s cadence (at most once per 2 s per house).
- Town NPCs & Shops (`crates/sim/src/town_npc.rs`, `crates/sim/src/shop.rs`):
  - Guide spawns at world start; Merchant arrives when any player has >= 5000 copper (50 silver) and a valid vacant house exists.
  - Pure state machine AI: wander within 12 blocks of home by day, sleep indoors at night.
  - Merchant shop: Torch, Wooden Arrow, Lesser Healing Potion, Iron Anvil, Copper Pickaxe, Copper Axe.
  - Coin stacking and payment logic (`coin.rs`): automatic denominations 100 copper -> 1 silver -> 1 gold -> 1 platinum; Shift-click bag slot to sell for 1/5 item value.
- Corruption Biome & Demonite (`crates/sim/src/worldgen/corruption.rs`):
  - Worldgen corruption pass (≥300 blocks from spawn): ebonstone, corrupt grass, deep chasms, demonite ore veins.
  - Demon Altars (tile 26, 3×2 footprint): unbreakable crafting station for Suspicious Looking Eye (6 Lenses).
  - Eater of Souls (NPC 6): flying enemy spawning in corruption, drops Shadow Scale (1–2 at 1/3) and Demonite Ore (1–2 at 1/10).
  - Demonite gear: Demonite Bars (3 ore at Furnace), Nightmare Pickaxe (minPick 65), War Axe of the Night, Light's Bane, and Shadow armor set (19 defense).
- Mana & Magic (`crates/sim/src/mana.rs`):
  - Base mana 20, max 200 via Mana Crystals (+20 per crystal); stored in character v1 file.
  - Authentic Terraria regen formula with post-cast delay and movement scaling.
  - Fallen Stars fall during night and vanish at dawn; 5 Fallen Stars craft 1 Mana Crystal without a station.
  - Wand of Sparking: found in surface chests, costs 2 mana, fires Spark projectiles dealing 14 magic damage.
- Combat & HUD QoL:
  - Mana star column HUD.
  - Potion Sickness debuff 21 HUD icon and countdown.
  - Held bow ammo stack counter.
  - Flaming Arrow hit inflicts On Fire debuff 24 (4 hp/s DoT for 7 s).
  - Eye of Cthulhu spawns Servants of Cthulhu (NPC 5) during phase 1 (cap 8).

Earlier skipped items completed in subsequent milestones:
- Rotten Chunk: Eater of Souls drops Shadow Scale directly in Stage 6 to permit pre-EoW crafting; Worm Food (70) craftable at Demon Altars with 15 Shadow Scale + 15 Ebonstone Block.
- Eater of Worlds & Worm Food: Fully implemented in Milestone 7.
- Hammers & Shadow Orbs: Fully implemented in Milestone 7 (Wooden Hammer 196, The Breaker 104; Pwnhammer 367 in M11).
- Shadow armor set bonus: +15% movement speed set bonus active when full set is equipped.

Verification: tests pass serially; release build warning-free; headless benchmark 182.2 FPS
avg, p50 5.37 ms, p99 8.36 ms, max 28.51 ms, over_16_7=2, pass=true on Quadro T1000; all QA scenarios pass headless.

Owner playtest findings (2026-10-27) status:
- [x] Camera does not turn at all with the laptop touchpad. -> Fixed in M1 Task 1.
- [x] Movement feels like ice. -> Fixed in M1 Task 2.
- [x] Art too blocky, colours unlike Terraria. -> Fixed in Milestone 2 (16×16 px textures, 1/16 block voxels, sampled Terraria palette).
- [x] No Terraria-style inventory; crafting should live inside it. -> Fixed in M1 Tasks 3–6 and polished in M2 Task 11.

Owner playtest findings from Milestone 1 (2026-10-27), scheduled for Milestones 2–4:
- [x] 1. Character and Guide currently only have 1 eye -> Fixed in M2 Task 9 (1/16 voxel 12³ heads with two 2×2 eyes with pupils).
- [x] 2. Hotbar scrolling via mouse wheel (bare wheel scrolls hotbar slots 0–9; Alt+wheel zooms camera) -> Fixed in M2 Task 11.
- [x] 3. Dropped items look flat as 2D sprites; need rotating 3D voxel models -> Fixed in M2 Task 10 (rotating 3D voxel drops).
- [x] 4. Guide / NPC dialogue chatbox needs styled Terraria border frame -> Fixed in M2 Task 11 (aged parchment + carved wooden border).
- [x] 5. Inventory backdrop is opaque (Terraria has translucent slots with world visible behind) -> Fixed in M2 Task 11.
- [x] 6. Armor, coin, and ammo slots need ghost silhouette icons -> Fixed in M2 Task 11.
- [x] 7. Swords need 16×16 voxel models, proper hand grip, and swing trail animations -> Fixed in M2 Task 10.
- [x] 8. Slimes missing hopping sound -> Fixed in M4 Task 8.

Resolved smaller issues:
- Shop sell on S removed; selling is now Shift-click bag slot; left click attacks suppressed while open.
- Decoupled combat demo from `qa-combat` world name.
- Fixed P1 double InGame transition bug in direct-in-game QA scenarios.
- Fixed server atomic save race by using unique temporary file names.
- Dead-code warnings removed from `hud::town`, `hud::chest`, `view/sky.rs`.
- Fixed celestial quad mesh vertex attribute usage panic on moon phase transitions.
- Milestone 2 art pipeline: `TILE = 16`, `VOXELS_PER_UNIT = 16.0`, `sample_palette.py` workflow, `check_art.py` validator covering all 8 families (140 assets).
- Tree stump preservation: tree chopping in `crates/sim/src/interact.rs` chops upwards from the targeted block, leaving trunk blocks below intact as a stump.
- Dropped Wood & Gel 3D models: Wood (`Item 9`) and Gel (`Item 23`) authored in `tools/art/models/items.py` with handcrafted 3D `.vox` models, rendering with 3D drop meshes instead of 2D icon cards.

## Open work (do this first)

**READY FOR FINAL 4-GATE VERIFICATION & OPTIONAL COMMIT/PUSH (2026-11-21):**
Working tree has all Batch 1 & Batch 2 (`WS1`–`WS4`) parity changes (`#1`–`#30`), the single-click block/wall/torch placement fix, and adversarial review fixes:
- `EnemyKind::Demon | EnemyKind::VoodooDemon` erroneous FireImp teleport removed (while `FireImp` keeps its 3-shot burst + teleport).
- Accessory prefixes (`62..=80`) preserved across `Player.accessory_prefixes` / `Character` (`TVCH` v4 with v1/v2/v3 backward compat) / `ServerMsg::Accessories` / `slots::click_with_prefixes`, applying defense/mana/crit/damage/move-speed/melee-speed bonuses on both server and client prediction, HUD defense, and inventory hover tooltips.
- `m9_e2e_underworld` (`fall_peak_y` reset after teleport) and legacy server test expectations (`(defense + 1) / 2`, `effective_mana_max()`, non-auto-reuse sword click pulsing, player knockback pinning, `Flamarang` 1-hit kill tolerance) updated and verified individually.

Remaining step:
- Run `cargo fmt --check`, `cargo check --workspace --all-targets`, `python3 -I tools/art/check_art.py`, and `cargo test --workspace -- --test-threads=1` after all edits settle, leaving `tools/token-audit.py`, `prompt.txt`, and `assets/terraria-pack/` untracked/unstaged, `sim`/`server` Bevy-free, and `Settings`/`InputBindings` `Copy`.

Summary of uncommitted implementation (`#1`–`#30` + single-click block fix + adversarial fixes):
- **Batch 1 (`#1`–`#4`, `#14`–`#16`, `#18`, `#25`)**: Head/hair 1-indexed `VoxModel.palette` + skin shell backfill + preview/in-game appearance, bespoke 3D rigs for enemy kinds, day/night dawn retreat / Skeletron enrage / neutral daytime Slimes / cave+Underworld spawn clearance / DemonScythe / FireImp / Bird AI.
- **Batch 2 WS1 (`#5`, `#6`, `#7`, `#12`, `#20`, `#21`, `#22`, `#23`, player `#29`)**: Client armor/accessory prediction sync, base vs `effective_mana_max()`, Hellstone Burning `30 HP/s`, silent `4 HP/s` `On Fire!` + Obsidian Skin, full HP/mana respawn, metal armor set bonuses + Shadow crit + un-gated Molten pieces + HUD defense, knockback/`Cobalt Shield` + fall `damage_taken`/`i_frames`, water surface `head_submerged`, `2.0x` Hermes Boots, `0.75` Cloud in a Bottle, duplicate accessory guard, `40` dmg Spikes, `vel[1]` regen check.
- **Batch 2 WS2 (`#8`, `#9`, `#13`, `#24`, `#26`–`#30` + single-click block placement fix)**: `(defense+1)/2`, melee reach, weapon/prefix tables (`1..=83` incl. accessory `62..=80` end-to-end across equip/unequip/slots/`TVCH` v4/HUD/tooltips) across crafting/drops/chests/projectiles, `FLINTLOCK_PISTOL = 95`, `MINISHARK = 98`, `MUSHROOM` consumable + 30s Potion Sickness, `5x` buy values, single-hit bullet i-frames (`pierce_cap == 1`), `ProjKind::WaterStream`, `ProjKind::Yoyo` (`VALOR`), steering `Magic Missile`, `Flamarang` outward bounce + return pierce, `MIN_PICK` (`OBSIDIAN` 55, `EBONSTONE` 65, Dungeon bricks 100), Pwnhammer Demon Altar break (`>= 80`), dead player respawn tick countdown, out-of-mana cast guard, single-click block/wall/torch placement fix (`auto_reuse: true` + `st.time = st.anim.max(use_time)`).
- **Batch 2 WS3 (`#10`, `#11`, `#17`, `#19`, `#28`–`#30`)**: `chest_locks.bin` (`TVLK`) + `chests_in_for_size` + `!contains_key` chunk reload guard, Underground Balloon/Horseshoe/Suspicious Eye + prefix rolls + deterministic Dungeon Shadow Key, no Golden Key inside locked Dungeon chests, Eye of Cthulhu Demonite drops (`30..=90`) & natural spawn gate, Eater of Worlds duplicate summon block + full defeat `downed_eow` (`TVWF` v3), `obsidian_skin_ticks` (`TVCH` v3) + `accessory_prefixes` (`TVCH` v4), `ArmsDealer` (`NPCID 19`) arrival & gun shop, town NPC day/boss-free respawn gating + cursor coin Merchant sum, housing Workbench/Furnace/Hellforge/Platform support, Obsidian Brick at Furnace + `"Bottle"` + Hellforge-as-Furnace.
- **Batch 2 WS4**: `.vox` assets `95.vox`, `98.vox`, and `models/arms_dealer/` rig + client wiring (`143` items, `38` enemy kinds) + reference doc sync.

State as of pushed commit `4e4177d`: All 11 roadmap milestones, Polish Passes 1–5, 24-finding audit, and Playtest Rounds 1–2 landed (`9d21a0a`, `c30e031`, `4e4177d`).

### Milestone 11 Summary
1. **Lava Summon & Multi-Part Boss AI**:
   - Guide Voodoo Doll (`ItemId(267)`) dropped into Underworld lava (`y <= 24`) kills live Guide and summons Wall of Flesh at nearer world X boundary.
   - Multi-part boss: `WallOfFleshMouth` (113), two `WallOfFleshEye` (114), `TheHungry` (115), and `Leech` (117).
   - Shared 8,000 HP pool across Mouth & Eyes; dynamic sweep acceleration (`1.5 px/tick` -> `7.5 px/tick`).
   - Eyes cast scaling `ProjKind::EyeLaser` (22/26/30 dmg, 120->50 tick cadence); Mouth spawns Leeches (cap 3).
   - The Tongue & Horrified pull players attempting to flee back with 25 damage every 30 ticks; far-edge crush on reaching opposite world boundary.
2. **Endgame Items, Emblems & Loot**:
   - 8 new items: Pwnhammer (367), Breaker Blade (426), Clockwork Assault Rifle (434), Sorcerer Emblem (489), Warrior Emblem (490), Ranger Emblem (491), Laser Rifle (514), Firecracker (4912). Total items: 141.
   - Guaranteed drops: 100% Pwnhammer, 1 class emblem, 1 WoF weapon, 5-10 Gold coins, 5-15 Lesser Healing Potions and Hearts.
3. **Victory UI, Protocol & Progression**:
   - WoF defeat flips `facts.downed_wall_of_flesh = true`, broadcasts `WallOfFleshDefeated` and `WofGone`.
   - Victory modal overlay ("Wall of Flesh Defeated!") offers "Continue Playing" in pre-Hardmode or "Save & Return to Title".
   - World remains strictly pre-Hardmode.
   - `progression_chain_end_of_era` end-to-end integration test and `wall_of_flesh` QA scenario.
4. **3D Assets & Client Rigs**:
   - Handcrafted 3D `.vox` models for all 8 items (`367.vox`, `426.vox`, `434.vox`, `489.vox`, `490.vox`, `491.vox`, `514.vox`, `4912.vox`).
   - 3D procedural mesh rigs for WoF Mouth (64-block vertical flesh wall), Eyes, The Hungry, Leech, and laser beams.
5. **Review & Verification**:
   - Whole-milestone review findings cleanly resolved and verified. All workspace gates pass.

## Roadmap checklist

Plans index, agent tiers and model choices:
[roadmap execution plan](archive/plans/2026-10-27-roadmap-execution.md). M1–M5 have
step-level plans; M6–M11 are outlines expanded at each milestone's start.

Each milestone gets its own spec and plan when started, ends with tests, release build,
headless benchmark, QA screenshots, review, an update of this file, and a stop for the
owner's playtest.

- [x] 1. Controls: touchpad camera, snappy movement, Terraria inventory with crafting on Esc;
      then gameplay QA (Tasks 10–11 of
      [menu-and-deep-qa plan](archive/plans/2026-10-25-menu-and-deep-qa.md)).
- [x] 2. Art overhaul: 16×16 blocks, 1/16 voxels, Terraria palette (Tasks 1–13 of
      [m2-art-overhaul plan](archive/plans/2026-10-27-m2-art-overhaul.md)).
- [x] 3. Liquids: water, lava, obsidian, buckets, swimming; finite world with edges and ocean ring
      (Tasks 1–11 of [m3-liquids plan](archive/plans/2026-10-27-m3-liquids.md)).
- [x] 4. Characters and music; Expert/Master and Mediumcore/Hardcore placeholders
      (Tasks 1–9 of [m4-characters-and-music plan](archive/plans/2026-10-27-m4-characters-and-music.md)).
- [x] 5. Finish Stage 6 (Tasks 8–12 of
      [stage 6 plan](archive/plans/2026-10-25-stage6-town-and-biomes.md)). Note: wire `mana_max`.
- [x] 6. Underground and cavern layers (Tasks 1–6 of
      [m6-underground-and-caverns plan](archive/plans/2026-11-08-m6-underground-and-caverns.md)).
- [x] 7. Eater of Worlds and demonite (Tasks 1–6 of
      [m7-eater-of-worlds plan](archive/plans/2026-11-09-m7-eater-of-worlds.md)).
- [x] 8. Dungeon and Skeletron (Tasks 1–6 of
      [m8-dungeon-and-skeletron plan](archive/plans/2026-11-09-m8-dungeon-and-skeletron.md)).
- [x] 9. Underworld (Tasks 1–6 of
      [m9-underworld plan](archive/plans/2026-11-10-m9-underworld.md)).
- [x] Polish Program (Passes 1–5):
      - [x] Pass 1: First hour and guidance ([polish-pass-1 plan](HANDOFF.md)).
      - [x] Pass 2: Information and economy ([polish-pass-2 plan](HANDOFF.md)).
      - [x] Pass 3: Held items, combat, and feedback ([polish-pass-3 plan](HANDOFF.md)).
      - [x] Pass 4: World and audio life ([polish-pass-4 plan](HANDOFF.md)).
      - [x] Pass 5: Options, accessibility, and stability ([polish-pass-5 plan](HANDOFF.md)).
- [x] 10. Molten gear (Tasks 1–6 of
      [m10-molten-gear plan](HANDOFF.md)).
- [x] 11. Wall of Flesh and the end-of-era playthrough (Tasks 1–6 of
      [m11-wall-of-flesh plan](HANDOFF.md)).

**Next action:** Commit and push Playtest Round 3 / Side-by-Side Terraria Parity + Swarm Polish/Perf/UI/Feel Waves 1–5 when instructed by the owner (excluding `tools/token-audit.py`, `prompt.txt`, and `assets/terraria-pack/`).

## Rules

- Keep token use low: narrow reads, filtered output, cheap subagents for routine work.
- Model hierarchy: follow the priority order in `docs/HANDOFF.md` §Model tiers (Strong: opus 5.5 → gemini-4-argon-high → deepseek v4.1 flash → gemini-3.8-flash-high → gpt-oss-120b; Standard: sonnet 5.5 → deepseek v4.1 flash → gemini-3.8-flash-medium → gpt-oss-120b; Cheap: deepseek v4.1 flash → haiku 5.5 → flint-40b/glm-5.3 → gemini-3.8-flash-low).
- Never open a game window on the owner's desktop for testing. Use `tools/bench-headless.sh`
  and `tools/qa-headless.sh` (private headless Sway). Manual play is the owner's, on the laptop
  panel eDP-1.
- Per task: test first, `cargo test --workspace -- --test-threads=1`, `cargo fmt --check`,
  no warnings, one commit.
- `sim` and `server` never depend on Bevy, rendering or audio. The server stays
  authoritative; clients use plain-data messages.
- Numbers come from the Terraria wiki or game, except tuned movement (roadmap §5.1).
- Never commit Terraria assets, music, decoded files or reference spec source. The local pack
  lives in ignored `assets/terraria-pack/`; fresh checkouts run
  `tools/install-terraria-assets.sh`.
- Push and merge: local branch `master` tracks remote `origin/master` (the default GitHub branch). No push or merge without the owner's instruction.
- Commits name the model that actually authored them.

### Key Technical Invariants and Guidelines

- **3D Dropped Items Invariant**: Every dropped item in the world must render as a 3D `.vox` model (`DropVisual::Model`). In-world dropped items must never render as flat 2D icon billboard cards. As new items are introduced in Milestones 6–11, every task adding items must include their corresponding 3D `.vox` drop models authored via `tools/art/models/items.py` and registered in `crates/client/src/view/avatar.rs` and `crates/client/src/view/drops.rs`.
- **Tree Stump Invariant**: Tree chopping in `crates/sim/src/interact.rs` operates only upwards from the targeted log height $y$ ($y' \ge y$), preserving all trunk blocks below ($y' < y$) as a stump.

## Agentic workflow (token-lean)

The goal is to deliver the most verified work for the fewest tokens. This workflow ran Polish
Passes 1–5 and was tuned along the way.

**The orchestrator (you) stays thin.**
- Don't read big source files yourself. Read the plan, hand tasks out, and read only the short reports that come back.
- Use subagent-driven development (the superpowers `subagent-driven-development` skill). Define two reusable subagent types once per session:
  - `implementer`: has write tools. Gets one plan task, writes the test first, implements, runs the scoped tests, and replies in ≤10 lines: files changed, tests run with pass/fail counts, any doubts.
  - `reviewer`: read-only. Gets the task text and the files changed, follows `.superpowers/sdd/reviewer-instructions.md`, and answers `Clean` or a numbered list of issues by severity.
- Give each brief the task text pasted from the plan, the exact file paths (watch for the plan-vs-code mismatches above), and the gate commands. Don't paste history.

**Match the model to the task.**
- `flash` for mechanical work: docs, warnings, renames, small fixes, single-file tasks, and every per-task review.
- `inherit`/`pro` for multi-system tasks (netcode, sim/client protocol, save format, gamepad/input).
- One `pro` whole-pass review at the end of each pass, not per task.

**Order and verification.**
- **Run implementers one at a time.** Parallel implementers in the same tree broke each other's builds and produced stale reports. You can run a reviewer in parallel only on a task that is already finished.
- Don't trust a "tests pass" claim unless it came from a full `cargo test --workspace` run after the last edit. In this session a reviewer passed Pass 5 on a stale gate report.
- Before you call a pass done, run one gate pass that starts after all edits are in.
- Batch the fixes from a review into one implementer call. Re-review with `flash`, scoped to the diff only.

**Stay within budget.**
- Keep briefs short and ask for short reports.
- Don't re-read files you have already summarized.
- Delete `.superpowers/sdd/<pass>/` scratch when a pass lands.
- If a subagent dies (DNS failure, 429 quota), retry once. If quota runs out, stop and write down the state here.

**Environment rules.**
- Never launch the game window. `tools/qa-headless.sh` GPU capture doesn't work in the agent environment, so skip it and rely on unit/ECS tests plus `tools/bench-headless.sh` where it works.
- `sim`/`server` stay Bevy-free.
- `Settings` and `InputBindings` must stay `Copy`.

## Prompt for a new agent

> Continue TerraVibe in `.` (branch `master`).
> Read `docs/HANDOFF.md` and `docs/HANDOFF.md`.
> Batch 1 + Batch 2 (`WS1`–`WS4` + adversarial review fixes + single-click block placement fix) are implemented in the working tree. Keep `sim`/`server` Bevy-free, `Settings`/`InputBindings` `Copy`, never stage `tools/token-audit.py`, `prompt.txt`, or `assets/terraria-pack/`, and never open a game window.
