# 16-VPU Voxel Models for All Remaining Entities Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Author bespoke 16-VPU `.vox` models and `rig.ron` animations for all 55 remaining `EnemyKind`s, register them in `tools/art/families.py`, and retire all procedural cuboid enemy rendering in `crates/client/src/view/enemies.rs`.

**Architecture:** Five thematic Python generator scripts under `tools/art/models/` (auto-discovered by `tools/art/build.py`) share reusable rig/animation helpers in `_rigtools.py` and emit `.vox` + `rig.ron` assets validated by `tools/art/check_art.py`. In `crates/client/src/view/enemies.rs`, all 63 `EnemyKind`s route through `EnemyRigs` + `spawn_rig_with` with unique `model_dir` folders, `AlphaMode::Blend` for all 5 slimes, and `idle`/`fly` animation switching for `Bird`.

**Tech Stack:** Python 3 (`tools/art/`), MagicaVoxel `.vox` v150, RON (`rig.ron`), Rust / Bevy (`crates/client/src/view/enemies.rs`).

**Spec:** `docs/superpowers/specs/2026-11-24-enemy-voxel-models-design.md`

## Global Constraints
- Author all models at 16 voxels per block (`VPU = 16`) in game axes (`x` right, `y` up, `z` front).
- Every `.vox` file must have `max(sx, sy, sz) <= 255`, `<= 255` palette colors, non-empty, not a 2x dilation, no undeclared floating islands (`> 3` voxels), and stay within family voxel budgets (`characters`: 14,000, `enemies`: 20,000, `boss`: 90,000).
- Every one of the 63 `EnemyKind`s must have its own unique `assets/models/<kind>/` directory; do not modify the 8 existing bespoke rigs (`slime`, `demon_eye`, `eater_of_souls`, `zombie`, `eye_of_cthulhu`, `guide`, `merchant`, `arms_dealer`).
- Large parts (`KingSlime` dome, `SkeletronHead`/`DungeonGuardian` crania, `WallOfFleshMouth` wall slabs) must use `Model.hollow(2)` to stay within budgets.
- Do not change enemy hitboxes, stats, AI, or spawning in `crates/sim`.

## Review Focus
- **Duplicate `model_dir` or missing rig/part files across all 63 `EnemyKind`s:** Any alias or typo makes two entities share a model or spawn invisible; pinned by Task 7 uniqueness + spawn tests and `check_art.py` in Tasks 2–6.
- **Slime translucency and hurt-flash alpha preservation on all 5 slimes (`Slime`, `LavaSlime`, `IceSlime`, `JungleSlime`, `KingSlime`):** Missing `AlphaMode::Blend` or alpha-stripping hurt tint breaks gel visuals; pinned by Task 7 `slime_jump_color_stability` test.
- **`WallOfFleshMouth` 255-voxel axis limit and shared `.vox` part references:** Single-mesh authoring exceeds 255 voxels and mismatched `file:` names break `check_rig`; pinned by Task 1 `write_rig` test and Task 6 `check_art.py boss`.
- **2x-dilation or undeclared floating-island failures in `check_art.py`:** Coarse blocks or floating eyes/inclusions fail validation; pinned by `check_art.py` at the end of Tasks 2–6.
- **`Bird` grounded-vs-airborne wing animation switching:** Replacing `BirdWing` without switching `RigAnim` between `"idle"` (folded) and `"fly"` (spread+flapping) breaks bird behavior; pinned by Task 7 `bird_wings_fold_when_grounded_and_flap_when_airborne`.

---

### Task 1: Shared Rig & Animation Helpers in `_rigtools.py`

**Files:**
- Modify: `tools/art/models/_rigtools.py`
- Test: `tools/art/test_check_art.py`

**Interfaces:**
- Consumes: `vox.Model`, `palette.ASSETS`, `palette.VPU`, `palette.hexc`, `check_art.check_rig`
- Produces:
  - `write_rig(out, parts, anims)` accepting both 4-tuples `(name, pivot, offset, parent)` and 5-tuples `(name, file, pivot, offset, parent)`
  - `out_dir(kind) -> str`
  - `save_parts(kind, parts: dict[str, Model]) -> None`
  - `recolor(m: Model, mp: dict) -> Model`
  - `mirror_x(m: Model) -> Model`
  - `humanoid_anims(arm_base=0.0, arm_swing=35.0, leg_swing=35.0, walk_len=0.6, idle_bob=0.8, walk_bob=1.0) -> dict`
  - `slime_anims(body_part="body", squash=(1.22, 0.74, 1.22), stretch=(0.88, 1.22, 0.88)) -> dict`
  - `wing_flap_anims(body_part="body", wing_l="wing_l", wing_r="wing_r", period=0.4, angle=35.0, bob=1.0) -> dict`
  - `jaw_chomp_anims(jaw_part="jaw", period=0.6, drop=-2.5, rot_x=18.0, bob_part=None) -> dict`
  - `swim_anims(tail_part="tail", period=0.5, yaw=28.0, bob_part="body") -> dict`

- [ ] **Step 1: Write failing unit tests in `tools/art/test_check_art.py`**
  Add a `RigTools(Base)` test class verifying:
  - `write_rig` with a 5-tuple `(name, file, pivot, offset, parent)` emits `name: "panel_a", file: "wall_slab.vox"` and passes `check_art.check_rig` when `wall_slab.vox` exists.
  - `write_rig` with a 4-tuple still emits `file: "<name>.vox"`.
  - `mirror_x` flips x-coordinates cleanly (`(x, y, z) -> (sx - 1 - x, y, z)`).

- [ ] **Step 2: Run test to verify it fails**
  Run: `python3 -I tools/art/test_check_art.py`
  Expected: FAIL (`AttributeError` / `ValueError` unpacking 5-tuple).

- [ ] **Step 3: Implement the helpers in `tools/art/models/_rigtools.py`**
  - Update `write_rig` to unpack 4-tuples or 5-tuples and auto-create `out` directory with `os.makedirs(out, exist_ok=True)`.
  - Add `out_dir`, `save_parts`, `recolor`, `mirror_x`, `humanoid_anims`, `slime_anims`, `wing_flap_anims`, `jaw_chomp_anims`, `swim_anims`.

- [ ] **Step 4: Run tests to verify pass**
  Run: `python3 -I tools/art/test_check_art.py`
  Expected: PASS (`OK`).

- [ ] **Step 5: Commit**
  ```bash
  git add tools/art/models/_rigtools.py tools/art/test_check_art.py
  git commit -m "feat(art): add shared rig and animation helpers to _rigtools"
  ```

---

### Task 2: Town NPCs (`npcs.py`) — 5 Rigs (`characters` family)

**Files:**
- Create: `tools/art/models/npcs.py`
- Modify: `tools/art/families.py` (append 5 NPC rigs to `FAMILIES["characters"]`)
- Generate: `assets/models/{nurse,demolitionist,dryad,goblin_tinkerer,old_man}/{head,torso,arm_l,arm_r,leg_l,leg_r}.vox` and `rig.ron`

**Interfaces:**
- Consumes: `_player_base.parts()`, `_player_base.PIVOTS`, `_player_base.OFFSETS`, `_rigtools.{out_dir, save_parts, recolor, write_rig, humanoid_anims}`
- Produces: 5 complete humanoid `.vox` rigs (`idle` + `walk` anims) registered in `FAMILIES["characters"]`.

- [ ] **Step 1: Register all 5 NPCs in `tools/art/families.py`**
  Add `nurse`, `demolitionist`, `dryad`, `goblin_tinkerer`, `old_man` (each with `PLAYER_PARTS` and `rig.ron`) to `FAMILIES["characters"]["models"]` and `FAMILIES["characters"]["rigs"]`.

- [ ] **Step 2: Run `check_art.py characters` to verify it fails on missing assets**
  Run: `python3 -I tools/art/check_art.py characters`
  Expected: FAIL (missing `.vox` / `rig.ron` files for the 5 NPCs).

- [ ] **Step 3: Implement `tools/art/models/npcs.py`**
  Author bespoke 16-VPU models for:
  - `nurse`: white medical dress/uniform with red cross on chest, white nurse cap with red cross, auburn hair bun, white shoes.
  - `demolitionist`: stocky dwarf torso/legs, bushy red-orange beard, miner's helmet with yellow headlamp, leather apron with red dynamite sticks strapped across chest.
  - `dryad`: emerald leaf/vine tunic, long dark-green hair, warm pale skin, vine-wrapped forearms & ankles.
  - `goblin_tinkerer`: green goblin skin, pointed ears protruding from head sides, brass aviator goggles on forehead, brown leather toolbelt/apron with pouches & wrench accent.
  - `old_man`: tattered brown-grey dungeon robe with deep hood, long white beard, shadowed weary eyes, knotted wooden staff held on `arm_r`.

- [ ] **Step 4: Generate and validate**
  Run: `python3 tools/art/models/npcs.py && python3 -I tools/art/check_art.py characters`
  Expected: `characters: ok (...)` with 0 errors.

- [ ] **Step 5: Commit**
  ```bash
  git add tools/art/models/npcs.py tools/art/families.py assets/models/nurse assets/models/demolitionist assets/models/dryad assets/models/goblin_tinkerer assets/models/old_man
  git commit -m "feat(art): add bespoke 16-VPU voxel rigs for 5 town NPCs"
  ```

---

### Task 3: Ambient Critters (`critters.py`) — 4 Rigs (`enemies` family)

**Files:**
- Create: `tools/art/models/critters.py`
- Modify: `tools/art/families.py` (append 4 critter rigs to `FAMILIES["enemies"]`)
- Generate: `assets/models/{bunny,bird,squirrel,cave_critter}/*.vox` and `rig.ron`

**Interfaces:**
- Consumes: `vox.Model`, `palette.hexc`, `_rigtools.{out_dir, save_parts, mirror_x, write_rig, key}`
- Produces:
  - `bunny`: parts `body`, `head`, `leg_fl`, `leg_fr`, `leg_bl`, `leg_br`; anims `idle`, `hop` (all 4 leg parts have rest offset `y < -3.5` voxels or clearly below torso so Task 7 leg test passes cleanly).
  - `bird`: parts `body`, `wing_l`, `wing_r`; anims `idle` (wings folded tight against sides, `rot_deg` z ≈ `±85°`) and `fly` (wings spread and flapping, `rot_deg` z oscillating around `±30°`).
  - `squirrel`: parts `body`, `tail`; anims `idle`, `walk`.
  - `cave_critter`: parts `body`, `legs_l`, `legs_r`; anims `idle`, `walk`.

- [ ] **Step 1: Register all 4 critters in `tools/art/families.py`**
  Append their `.vox` files and `rig.ron` files to `FAMILIES["enemies"]`.

- [ ] **Step 2: Run `check_art.py enemies` to verify failure on missing assets**
  Run: `python3 -I tools/art/check_art.py enemies`
  Expected: FAIL (missing critter assets).

- [ ] **Step 3: Implement `tools/art/models/critters.py`**
  Author detailed 16-VPU voxel models with multi-tone fur/feather/shell shading, eyes, ears/beaks/antennae, and keyframed animations matching the Interfaces block above.

- [ ] **Step 4: Generate and validate**
  Run: `python3 tools/art/models/critters.py && python3 -I tools/art/check_art.py enemies`
  Expected: `enemies: ok (...)` with 0 errors.

- [ ] **Step 5: Commit**
  ```bash
  git add tools/art/models/critters.py tools/art/families.py assets/models/bunny assets/models/bird assets/models/squirrel assets/models/cave_critter
  git commit -m "feat(art): add bespoke 16-VPU voxel rigs for 4 ambient critters"
  ```

---

### Task 4: Underground, Dungeon & Underworld Enemies (`enemies_underground.py`) — 14 Rigs (`enemies` family)

**Files:**
- Create: `tools/art/models/enemies_underground.py`
- Modify: `tools/art/families.py` (append 14 enemy rigs + any slime `detached` allowances to `FAMILIES["enemies"]`)
- Generate: `assets/models/{skeleton,angry_bones,dark_caster,cursed_skull,cave_bat,hellbat,giant_worm,bone_serpent_head,bone_serpent_body,bone_serpent_tail,lava_slime,fire_imp,demon,voodoo_demon}/*.vox` and `rig.ron`

**Interfaces:**
- Consumes: `vox.Model`, `palette.hexc`, `_rigtools.*`
- Produces: 14 rigs registered in `FAMILIES["enemies"]`:
  - Walkers (`idle`, `walk`): `skeleton`, `angry_bones`, `dark_caster`.
  - Hoppers (`idle`, `hop`): `lava_slime` (translucent molten orange-red gel `alpha=195`, charred base flecks, glowing magma core inside).
  - Flyers/Worms/Hoverers (`idle`): `cursed_skull` (with chattering `jaw`), `cave_bat` (`wing_l`/`wing_r`), `hellbat` (`wing_l`/`wing_r`), `giant_worm` (`head`, `body`, `tail` + mandibles), `bone_serpent_head` (`skull`, `jaw`), `bone_serpent_body` (`segment`), `bone_serpent_tail` (`tail`), `fire_imp` (`body`, `arm_l`, `arm_r`, `tail`), `demon` (`body`, `wing_l`, `wing_r`), `voodoo_demon` (`body`, `wing_l`, `wing_r`, `voodoo_doll`).

- [ ] **Step 1: Register all 14 rigs in `tools/art/families.py`**
  Append their `.vox` and `rig.ron` paths (and `detached` entry for `lava_slime` if it uses detached eyes/core) to `FAMILIES["enemies"]`.

- [ ] **Step 2: Run `check_art.py enemies` to verify failure on missing assets**
  Run: `python3 -I tools/art/check_art.py enemies`
  Expected: FAIL (missing 14 underground/underworld rigs).

- [ ] **Step 3: Implement `tools/art/models/enemies_underground.py`**
  Author detailed 16-VPU voxel models with distinct silhouettes, multi-tone shading, glowing eye colors, armor/staff/wing/jaw/doll parts, and keyframed animations.

- [ ] **Step 4: Generate and validate**
  Run: `python3 tools/art/models/enemies_underground.py && python3 -I tools/art/check_art.py enemies`
  Expected: `enemies: ok (...)` with 0 errors.

- [ ] **Step 5: Commit**
  ```bash
  git add tools/art/models/enemies_underground.py tools/art/families.py assets/models/skeleton assets/models/angry_bones assets/models/dark_caster assets/models/cursed_skull assets/models/cave_bat assets/models/hellbat assets/models/giant_worm assets/models/bone_serpent_head assets/models/bone_serpent_body assets/models/bone_serpent_tail assets/models/lava_slime assets/models/fire_imp assets/models/demon assets/models/voodoo_demon
  git commit -m "feat(art): add 14 underground, dungeon, and underworld 16-VPU enemy rigs"
  ```

---

### Task 5: Events & Biome Enemies (`enemies_biomes_events.py`) — 20 Rigs (`enemies` family)

**Files:**
- Create: `tools/art/models/enemies_biomes_events.py`
- Modify: `tools/art/families.py` (append 20 enemy rigs + `ice_slime`/`jungle_slime` `detached` allowances to `FAMILIES["enemies"]`)
- Generate: `assets/models/{servant_of_cthulhu,bee,blood_zombie,drippler,goblin_peon,goblin_thief,goblin_warrior,goblin_sorcerer,goblin_archer,antlion,vulture,ice_slime,snow_flinx,jungle_slime,hornet,man_eater,harpy,shark,crab,piranha}/*.vox` and `rig.ron`

**Interfaces:**
- Consumes: `vox.Model`, `palette.hexc`, `_rigtools.*`
- Produces: 20 rigs registered in `FAMILIES["enemies"]`:
  - Walkers (`idle`, `walk`): `blood_zombie`, `goblin_peon`, `goblin_thief`, `goblin_warrior`, `goblin_sorcerer`, `goblin_archer`, `snow_flinx`, `crab`.
  - Hoppers (`idle`, `hop`): `ice_slime` (translucent glacial cyan `alpha=190`, ice shards & eyes inside), `jungle_slime` (translucent emerald `alpha=190`, vines/spores & eyes inside).
  - Flyers/Swimmers/Stationary (`idle`): `servant_of_cthulhu`, `bee`, `drippler`, `antlion`, `vulture`, `hornet`, `man_eater`, `harpy`, `shark`, `piranha`.

- [ ] **Step 1: Register all 20 rigs in `tools/art/families.py`**
  Append their `.vox` and `rig.ron` paths (and any `detached` allowances for `ice_slime`/`jungle_slime` eyes/inclusions) to `FAMILIES["enemies"]`.

- [ ] **Step 2: Run `check_art.py enemies` to verify failure on missing assets**
  Run: `python3 -I tools/art/check_art.py enemies`
  Expected: FAIL (missing 20 event/biome rigs).

- [ ] **Step 3: Implement `tools/art/models/enemies_biomes_events.py`**
  Author detailed 16-VPU voxel models with distinct silhouettes (each of the 5 goblins gets its own armor/hood/weapon: club, dual daggers, horned helm + axe/shield, wizard hood + chaos staff, feathered cap + bow/quiver; aquatic enemies get sculpted fins/gills/jaws/tails; flying enemies get scalloped/feathered/translucent wings).

- [ ] **Step 4: Generate and validate**
  Run: `python3 tools/art/models/enemies_biomes_events.py && python3 -I tools/art/check_art.py enemies`
  Expected: `enemies: ok (...)` with 0 errors.

- [ ] **Step 5: Commit**
  ```bash
  git add tools/art/models/enemies_biomes_events.py tools/art/families.py assets/models/servant_of_cthulhu assets/models/bee assets/models/blood_zombie assets/models/drippler assets/models/goblin_peon assets/models/goblin_thief assets/models/goblin_warrior assets/models/goblin_sorcerer assets/models/goblin_archer assets/models/antlion assets/models/vulture assets/models/ice_slime assets/models/snow_flinx assets/models/jungle_slime assets/models/hornet assets/models/man_eater assets/models/harpy assets/models/shark assets/models/crab assets/models/piranha
  git commit -m "feat(art): add 20 event and biome 16-VPU enemy rigs"
  ```

---

### Task 6: Pre-Hardmode Bosses & Boss Minions (`bosses_prehardmode.py`) — 12 Rigs (`boss` family)

**Files:**
- Create: `tools/art/models/bosses_prehardmode.py`
- Modify: `tools/art/families.py` (append 12 boss rigs to `FAMILIES["boss"]`)
- Generate: `assets/models/{king_slime,queen_bee,eater_of_worlds_head,eater_of_worlds_body,eater_of_worlds_tail,skeletron_head,skeletron_hand,dungeon_guardian,wall_of_flesh_mouth,wall_of_flesh_eye,the_hungry,leech}/*.vox` and `rig.ron`

**Interfaces:**
- Consumes: `vox.Model`, `palette.hexc`, `_rigtools.*` (specifically 5-tuple `write_rig` for `wall_of_flesh_mouth`)
- Produces: 12 rigs registered in `FAMILIES["boss"]`:
  - `king_slime`: hollowed translucent blue gel `body.vox` (`alpha=190`, `hollow(2)`), `ninja.vox`, `crown.vox`; anims `idle`, `hop`.
  - `queen_bee`: `body.vox`, `wing_l.vox`, `wing_r.vox`; anim `idle`.
  - `eater_of_worlds_head`: `head.vox`, `mandible_l.vox`, `mandible_r.vox`; anim `idle`.
  - `eater_of_worlds_body`: `body.vox`; anim `idle`.
  - `eater_of_worlds_tail`: `tail.vox`; anim `idle`.
  - `skeletron_head`: hollowed ivory `cranium.vox`, `jaw.vox`; anim `idle`.
  - `skeletron_hand`: `hand.vox`; anim `idle`.
  - `dungeon_guardian`: distinct weathered cracked-bone hollowed `cranium.vox` with blazing crimson sockets, fanged `jaw.vox`; anim `idle`.
  - `wall_of_flesh_mouth`: `wall_slab.vox` (hollowed 16-VPU crimson flesh/sinew/rib tile <= 128 voxels per axis, tiled across the 24x64-block span via 5-tuple `rig.ron` entries), `maw_upper.vox`, `maw_lower.vox` (chomping), `tendril.vox` (swaying); anim `idle`.
  - `wall_of_flesh_eye`: `eye.vox`, `stalk.vox`; anim `idle`.
  - `the_hungry`: `head.vox`, `jaw.vox`; anim `idle`.
  - `leech`: `head.vox`, `tail.vox`; anim `idle`.

- [ ] **Step 1: Register all 12 boss rigs in `tools/art/families.py`**
  Append their `.vox` and `rig.ron` paths to `FAMILIES["boss"]`.

- [ ] **Step 2: Run `check_art.py boss` to verify failure on missing assets**
  Run: `python3 -I tools/art/check_art.py boss`
  Expected: FAIL (missing 12 boss rigs).

- [ ] **Step 3: Implement `tools/art/models/bosses_prehardmode.py`**
  Author detailed 16-VPU boss models using `hollow(2)` on large shells and 5-tuple `write_rig` tiling on `wall_of_flesh_mouth`.

- [ ] **Step 4: Generate and validate**
  Run: `python3 tools/art/models/bosses_prehardmode.py && python3 -I tools/art/check_art.py boss`
  Expected: `boss: ok (...)` with 0 errors.

- [ ] **Step 5: Commit**
  ```bash
  git add tools/art/models/bosses_prehardmode.py tools/art/families.py assets/models/king_slime assets/models/queen_bee assets/models/eater_of_worlds_head assets/models/eater_of_worlds_body assets/models/eater_of_worlds_tail assets/models/skeletron_head assets/models/skeletron_hand assets/models/dungeon_guardian assets/models/wall_of_flesh_mouth assets/models/wall_of_flesh_eye assets/models/the_hungry assets/models/leech
  git commit -m "feat(art): add 12 pre-hardmode boss and minion 16-VPU voxel rigs"
  ```

---

### Task 7: Client Integration, Procedural Cuboid Retirement & Full Verification

**Files:**
- Modify: `crates/client/src/view/enemies.rs`

**Interfaces:**
- Consumes: All 63 `assets/models/<kind>/rig.ron` and `.vox` files from Tasks 2–6 + existing 8 rigs.
- Produces: Clean `.vox`-only `EnemiesPlugin` where every `EnemyKind` has a unique `model_dir(kind)`, all 5 slimes use `AlphaMode::Blend`, and `Bird` switches between `"idle"` and `"fly"`.

- [ ] **Step 1: Update `crates/client/src/view/enemies.rs` tests first (TDD)**
  Update/add tests in `mod tests`:
  - `every_enemy_kind_has_model`: assert all 63 `EnemyKind`s have **unique** `model_dir(kind)` (via `HashSet`, `len() == ALL_ENEMY_KINDS.len()`), valid `rig.ron` and `.vox` files, and spawn an entity in `EnemiesPlugin`.
  - Remove `bone_serpent_has_procedural_rig`; update `critter_visual_mappings_and_non_contact` to verify unique `.vox` rigs instead of `has_procedural_rig`.
  - Update `all_underworld_kinds_have_models_and_scales` to check valid `rig.ron` + positive scale + animations (removing `assert_ne!(tint, Color::WHITE)` since bespoke `.vox` rigs carry their own palette).
  - Update `slime_jump_color_stability` to cover all 5 slimes (`Slime`, `LavaSlime`, `IceSlime`, `JungleSlime`, `KingSlime`) preserving base color/alpha on telegraph and flashing hurt with preserved alpha, plus `AlphaMode::Blend` on their spawned `StandardMaterial`.
  - Update `bunny_has_four_legs_and_raised_torso` to verify `bunny`'s `rig.ron` has 4 distinct leg parts (`leg_fl`, `leg_fr`, `leg_bl`, `leg_br`) and `"hop"` animation.
  - Update `bird_wings_fold_when_grounded_and_flap_when_airborne` to verify `bird`'s `RigAnim` plays `"idle"` when grounded (folded wings in `rig::sample`) and switches to `"fly"` when airborne (spread & flapping wings in `rig::sample`).

- [ ] **Step 2: Run client enemy tests to confirm the updated tests fail before implementation**
  Run: `cargo test -p terravibe-client view::enemies`
  Expected: FAIL (procedural rigs still active, `model_dir` not unique).

- [ ] **Step 3: Refactor `crates/client/src/view/enemies.rs`**
  - Delete `has_procedural_rig`, `spawn_procedural_enemy`, `proc_mat`, `proc_part`, `proc_part_rot`, `BirdWing`, `ProcWing`, `ProcLimb`, `ProcJaw`, `ProcTendril`, and the procedural child query loops in `animate`.
  - Map all 63 `EnemyKind`s in `model_dir(kind)` to their unique snake_case folder names.
  - Add `is_slime(kind)` helper matching `Slime | LavaSlime | IceSlime | JungleSlime | KingSlime`; use it for `AlphaMode::Blend` in `apply_msgs` and alpha-preserving hurt flash in `enemy_tint_for`.
  - Set `tint_for(kind)` to `SLIME_TINT` for `EnemyKind::Slime` and `Color::WHITE` for all other kinds.
  - Set `model_scale(kind)` appropriately (`1.1` for humanoids matching `Zombie`/`Guide`, `1.0` for natively sized 16-VPU rigs, preserving existing scale expectations tested by `all_underworld_kinds_have_models_and_scales` / `critter_visual_mappings_and_non_contact` where appropriate or aligning model dimensions to match).
  - Update `anim_for(kind, moving)` to return `"hop"` for all 5 slimes + `Bunny`, `"walk"` for all walking humanoids/goblins/skeletons/NPCs/`Crab`/`SnowFlinx`/`Squirrel`/`CaveCritter`, and in `animate` override `Bird` with `"fly"` when `is_bird_airborne(e.vel, interp.from, interp.to)` else `"idle"`.

- [ ] **Step 4: Run full verification suite**
  Run:
  ```bash
  python3 tools/art/build.py && python3 -I tools/art/check_art.py && python3 -I tools/art/test_check_art.py && cargo test --workspace && cargo clippy --workspace --all-targets
  ```
  Expected:
  - `build.py` exits 0
  - `check_art.py` reports `ok` for all families
  - `test_check_art.py` reports `OK`
  - `cargo test --workspace` passes all tests
  - `cargo clippy --workspace --all-targets` exits 0 with no new warnings

- [ ] **Step 5: Commit**
  ```bash
  git add crates/client/src/view/enemies.rs
  git commit -m "feat(view): route all 63 enemies through bespoke 16-VPU voxel rigs"
  ```
