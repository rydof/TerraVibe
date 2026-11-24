# 16-VPU Voxel Models for All Remaining Entities

**Date:** 2026-11-24
**Scope:** Project (55 bespoke 16-VPU `.vox` rigs, 5 generator scripts, art validation registration, and full retirement of procedural cuboid enemy rigs in `crates/client/src/view/enemies.rs`).

## 1. Intent & Why

Every one of TerraVibe's 63 `EnemyKind`s should look like it belongs in the same hand-crafted 16-voxels-per-block (16-VPU) voxel world as `Slime`, `Zombie`, `DemonEye`, `EaterOfSouls`, and `EyeOfCthulhu`. Today, only 8 entities have their own `.vox` rig; 5 reuse another entity's rig (`LavaSlime`, `Nurse`, `Demolitionist`, `Dryad`, `GoblinTinkerer`), 32 are built out of flat untextured Bevy cuboids, and 18 Wave 5 enemies are bare single-box or 4-box stubs.

Giving every entity its own detailed 16-VPU `.vox` model and `rig.ron` eliminates placeholder visuals, removes ~1,000 lines of bespoke cuboid construction and `Proc*` animation code from `enemies.rs`, and brings 100% of entities under `tools/art/check_art.py`.

## 2. Goals, Non-Goals, and Anti-Goals

### Goals
- Author bespoke 16-VPU `.vox` models and `rig.ron` animations for all 55 remaining `EnemyKind`s (every entity that does not already have its own unique `assets/models/<kind>/` directory).
- Keep the existing 8 bespoke rigs (`slime`, `demon_eye`, `eater_of_souls`, `zombie`, `eye_of_cthulhu`, `guide`, `merchant`, `arms_dealer`) intact.
- Register all 55 new rigs in `tools/art/families.py` and pass `python3 -I tools/art/check_art.py`.
- Retire `spawn_procedural_enemy`, `has_procedural_rig`, and the procedural cuboid animation components (`BirdWing`, `ProcWing`, `ProcLimb`, `ProcJaw`, `ProcTendril`) in `crates/client/src/view/enemies.rs`, routing all 63 `EnemyKind`s through `EnemyRigs` + `spawn_rig_with`.
- Preserve gameplay-critical visual cues: slime translucency (`AlphaMode::Blend`), `Bird` folded-vs-flapping wings (`idle` vs `fly`), `SkeletronHead`/`DungeonGuardian` spin, `CursedSkull`/`FireImp` hover bob, hurt flash, telegraph scale/tint, and ambient particles.

### Non-Goals
- Changing enemy hitboxes, stats, AI, or spawning in `crates/sim`.
- Adding per-part emissive shaders or UV-mapped PNG textures to `.vox` rigs (the existing vertex-colored greedy mesher in `voxmesh.rs` + shared `StandardMaterial` in `rig_view.rs` is the target pipeline).
- Reworking the 8 already-bespoke `.vox` rigs.

### Anti-Goals
- Reusing another entity's `.vox` directory or merely color-tinting a shared model (every one of the 63 `EnemyKind`s must have its own unique `assets/models/<kind>/` directory).
- Producing coarse 2x-dilated blocks or flat single-color cuboids disguised as `.vox` files (must pass `check_art.py`'s dilation, island, and voxel budget checks, with multi-tone shading, sculpted silhouettes, eyes, clothing/armor/anatomy details).
- Breaking existing unit tests or leaving invisible/unspawnable enemies.

## 3. Technical Constraints

- **16 VPU standard:** All models are authored at 16 voxels per world block in game axes (`x` right, `y` up, `z` front), matching `VOXELS_PER_UNIT = 16.0` in `crates/client/src/art/voxmesh.rs`.
- **`.vox` axis limit (255) & voxel budgets:** Each `.vox` part must have `max(sx, sy, sz) <= 255`, <= 255 palette colors, non-empty, not a 2x dilation, and fit within its family budget in `tools/art/families.py` (`enemies`: 20,000, `boss`: 90,000, `characters`: 14,000).
- **Large boss tiling (`WallOfFleshMouth`):** At 16 VPU, the Wall of Flesh spans 64 blocks along Z (~1,024 voxels) and 24 blocks along Y (~384 voxels), exceeding `.vox`'s 255-voxel axis cap. Extend `write_rig` in `tools/art/models/_rigtools.py` to accept an optional explicit `file` per part so `wall_of_flesh_mouth/rig.ron` can compose sculpted, hollowed 16-VPU flesh panels (`wall_slab.vox`), rib/sinew ridges, animated tendrils (`tendril.vox`), and upper/lower chomping maw jaws (`maw_upper.vox`, `maw_lower.vox`) using shared `.vox` files cached by `MeshCache`.
- **Palette & tinting:** Because each new entity has its own full-color `.vox` palette, `tint_for(kind)` in `enemies.rs` should return `Color::WHITE` for all bespoke `.vox` entities except `Slime` (which keeps `SLIME_TINT` for its pale translucent gel). All 5 slimes (`Slime`, `LavaSlime`, `IceSlime`, `JungleSlime`, `KingSlime`) must set `AlphaMode::Blend` on their `StandardMaterial` and preserve alpha during hurt flashes.
- **Hollowing large parts:** Large boss/body parts (e.g., `KingSlime` gel shell, `SkeletronHead` cranium, `WallOfFleshMouth` wall slabs) should use `Model.hollow(2)` (like `eye_of_cthulhu/ball.vox`) to stay well within voxel budgets and keep greedy meshing fast.

## 4. Approaches Considered

1. **Approach A (Selected): Full migration in one coordinated pass**
   - Split the 55 generators across 5 thematic Python scripts in `tools/art/models/` (auto-discovered by `tools/art/build.py`), register all 55 in `tools/art/families.py`, and remove `spawn_procedural_enemy` in `enemies.rs`.
   - *Why selected:* The `.vox` rig pipeline (`build.py`, `check_art.py`, `rig_view.rs`) is already mature; doing a single clean migration avoids carrying dual cuboid/voxel rendering paths and delivers a uniform 63-entity roster immediately.
2. **Approach B: Phased rollout in 3 passes**
   - Wave 5 placeholders + NPCs first, then Bosses, then Underground/Critters.
   - *Trade-off:* Faster intermediate review, but prolongs dual rendering paths and triples plan/verify overhead.
3. **Approach C: Monolithic `enemies.py` + hybrid Rust procedural animations**
   - *Rejected:* Bloats `enemies.py` into a 5,000+ line file and splits animation ownership awkwardly between `rig.ron` and Rust.

## 5. Architecture & Entity Breakdown (55 New Rigs)

### Generator Scripts (`tools/art/models/`)
All scripts are executed automatically by `tools/art/build.py`. Shared helpers live in `_rigtools.py` (optional `file` override in `write_rig`, reusable animation builders for humanoid walk/idle, slime hop/idle, winged flap/hover, worm undulate, quadruped hop/walk, swimming tail-wag).

#### 1. `tools/art/models/npcs.py` (5 rigs -> `characters` family)
Built on `_player_base.py` proportions (`head`, `torso`, `arm_l`, `arm_r`, `leg_l`, `leg_r`, `idle` + `walk` anims):
- `nurse`: White medical uniform with red cross emblem, nurse cap with red cross, auburn hair bun, white shoes.
- `demolitionist`: Stocky dwarf silhouette (wider torso/shorter legs), bushy red-orange beard, miner's helmet with headlamp, leather apron with strapped dynamite sticks.
- `dryad`: Nature-spirit tunic of layered emerald leaves/vines, long dark-green flowing hair, pale warm skin, vine-wrapped forearms and ankles.
- `goblin_tinkerer`: Green goblin skin, pointed ears, brass aviator goggles on forehead, brown leather toolbelt/apron with pouches, dark trousers.
- `old_man`: Hooded tattered brown/grey dungeon robe, long white beard, shadowed face with weary eyes, knotted wooden staff/robe sleeves.

#### 2. `tools/art/models/critters.py` (4 rigs -> `enemies` family)
- `bunny`: Rounded white fur body, sculpted head with pink nose and dark eyes, tall pink-lined ears, cotton puff tail, 4 distinct leg parts (`leg_fl`, `leg_fr`, `leg_bl`, `leg_br`) animated in `idle` and `hop`.
- `bird`: Bluebird body with cream belly, sculpted head + orange beak + eyes, orange feet, tail feathers, `wing_l` + `wing_r`. Two animations: `idle` (wings folded tight against sides) and `fly` (wings spread and flapping).
- `squirrel`: Warm russet fur body with cream belly, rounded head with tufted ears, 4 tiny paws, sculpted curved bushy tail (`tail` bobs in `idle`/`walk`).
- `cave_critter`: Segmented slate-blue cave beetle carapace, bright cyan bioluminescent abdomen & eyes, 6 jointed legs (`leg_l`, `leg_r` scuttling in `idle`/`walk`).

#### 3. `tools/art/models/enemies_underground.py` (14 rigs -> `enemies` family)
- `skeleton`: Sculpted skull with deep eye sockets & jaw, exposed spine + 3-rib ribcage + pelvis, bone arms & legs (`idle`, `walk`).
- `angry_bones`: Weathered tan-bone skeleton wearing a dented iron dungeon helm, pauldrons, and a rusty buckler on `arm_l` (`idle`, `walk`).
- `dark_caster`: Deep indigo hooded dungeon robe with gold trim, shadowed skull face with glowing violet eyes, skeletal hands holding a glowing arcane staff (`idle`, `walk`).
- `cursed_skull`: Floating weathered cranium with cyan spectral flames rising from the crown, glowing cyan eye sockets, hinged chattering `jaw` (`idle`).
- `cave_bat`: Fuzzy purple-brown bat body with pointed ears, fangs, bright amber eyes, scalloped leathery `wing_l`/`wing_r` (`idle` flapping).
- `hellbat`: Crimson-black ember bat with curved horns, glowing molten eyes, fiery orange-edged scalloped `wing_l`/`wing_r` (`idle` flapping).
- `giant_worm`: Segmented earthy-brown burrowing head + body ring + tapered tail with snapping ivory mandibles (`idle` undulation).
- `bone_serpent_head`: Underworld skeletal dragon-serpent skull with fiery orange eye sockets, dorsal bone horns, hinged chomping `jaw` (`idle`).
- `bone_serpent_body`: Articulated bone vertebra segment with curved ribcage spikes and glowing magma-core spine (`idle`).
- `bone_serpent_tail`: Tapered skeletal tail vertebrae ending in a sharp bone spike (`idle`).
- `lava_slime`: Translucent molten orange-red gel shell (`alpha ~195`), charred obsidian crust flecks on base, glowing yellow-white magma core inside (`idle`, `hop`).
- `fire_imp`: Blazing crimson-orange imp humanoid with curved dark horns, glowing yellow eyes, barbed tail, ember hands (`idle`).
- `demon`: Broad crimson underworld demon with curved obsidian horns, glowing amber eyes, ribbed chest, barbed tail, large scalloped `wing_l`/`wing_r` (`idle` flapping).
- `voodoo_demon`: Deep magenta-crimson demon with horns, glowing eyes, flapping `wing_l`/`wing_r`, and a sculpted swinging `voodoo_doll` (miniature Guide doll) dangling below (`idle`).

#### 4. `tools/art/models/enemies_biomes_events.py` (20 rigs -> `enemies` family)
- `servant_of_cthulhu`: Miniature bloodshot eyeball with red iris, black pupil, veined sclera, and 2 swaying optic-nerve tendrils (`idle`).
- `bee`: Small striped yellow-and-black fuzzy bee body, stinger, compound eyes, translucent fluttering `wing_l`/`wing_r` (`idle`).
- `blood_zombie`: Crimson-drenched rotting zombie with torn dark rags, exposed bloody ribs/skull, glowing red eyes, outstretched arms (`idle`, `walk`).
- `drippler`: Floating crimson fleshy mass studded with multiple bloodshot eyes, dripping gore tendrils hanging below (`idle`).
- `goblin_peon`: Green-skinned goblin with pointed ears, ragged brown tunic, spiked wooden club on `arm_r` (`idle`, `walk`).
- `goblin_thief`: Hunched agile goblin with dark hood/bandana, leather vest, dual iron daggers (`idle`, `walk`).
- `goblin_warrior`: Heavily armored goblin with horned iron helmet, spiked shoulder plates, battleaxe + shield (`idle`, `walk`).
- `goblin_sorcerer`: Purple-robed goblin mage with pointed wizard hood, glowing magenta chaos orb staff (`idle`, `walk`).
- `goblin_archer`: Green-cloaked goblin ranger with feathered cap, back quiver, wooden bow on `arm_l` (`idle`, `walk`).
- `antlion`: Sandy-ochre armored insect body half-buried posture, segmented thorax, large serrated snapping `mandible_l`/`mandible_r`, compound eyes (`idle`).
- `vulture`: Scruffy dark-brown feathered body, hunched bald pink neck/head with hooked ivory beak, taloned feet, broad `wing_l`/`wing_r` (`idle` / flapping).
- `ice_slime`: Translucent glacial-cyan gel shell (`alpha ~190`), frosted base, embedded crystalline ice shards & dark eyes inside (`idle`, `hop`).
- `snow_flinx`: Round fluffy white-and-ice-blue furball creature with small pink ears, dark eyes, stubby feet that waddle (`idle`, `walk`).
- `jungle_slime`: Translucent emerald-green gel shell (`alpha ~190`), mossy base, suspended jungle vines/amber spores & eyes inside (`idle`, `hop`).
- `hornet`: Elongated banded gold-and-black wasp thorax/abdomen, sharp stinger, antennae, translucent buzzing `wing_l`/`wing_r` (`idle`).
- `man_eater`: Snapping carnivorous jungle plant head with serrated crimson/green petals, white fang rows, leafy collar, coiled vine stalk (`idle` chomping).
- `harpy`: Sky-biome winged humanoid with azure feather hair, cerulean/white feathered `wing_l`/`wing_r`, taloned legs, sky-blue tunic (`idle` flapping).
- `shark`: Streamlined slate-grey ocean shark with pale belly, dorsal/pectoral fins, gill slits, toothy jaw, animated sweeping `tail` (`idle`).
- `crab`: Red-orange plated crab carapace, eyestalks, animated snapping `claw_l`/`claw_r`, 6 jointed legs (`idle`, `walk`).
- `piranha`: Deep-green/teal fish body with crimson belly, fierce underbite with sharp white teeth, yellow eye, rapid-wagging `tail` (`idle`).

#### 5. `tools/art/models/bosses_prehardmode.py` (12 rigs -> `boss` family)
- `king_slime`: Large hollowed translucent royal-blue gel dome (`alpha ~190`, ~64x56x64 voxels), detailed dark-clad ninja suspended inside (`ninja.vox`), ornate gold-and-ruby `crown.vox` perched on top (`idle`, `hop`).
- `queen_bee`: Large sculpted golden-and-black striped bee queen with armored thorax, crown-like head crest, fierce red eyes, segmented abdomen + stinger, large translucent veined `wing_l`/`wing_r` (`idle` rapid flap).
- `eater_of_worlds_head`: Massive armored violet-purple corruption worm head with layered chitin plates, 5 glowing toxic-green eyes, animated snapping `mandible_l`/`mandible_r` (`idle`).
- `eater_of_worlds_body`: Segmented violet chitin body ring with dorsal ridge spikes and glowing green side eyes (`idle`).
- `eater_of_worlds_tail`: Tapered violet armored tail segment with spiked tip and glowing side eyes (`idle`).
- `skeletron_head`: Large hollowed ivory-bone cranium (~32x30x28 voxels) with brow ridge, cheekbones, glowing amber-orange deep eye sockets, nasal cavity, articulated chomping `jaw` with teeth (`idle`).
- `skeletron_hand`: Sculpted skeletal forearm + palm + 4 curled jointed bone fingers + thumb claw (`idle` flexing).
- `dungeon_guardian`: Distinct weathered ancient-bone skull rig (cracked darker-ivory patina, blazing crimson eye sockets, wider fanged `jaw`) so it does not share `skeletron_head` (`idle`).
- `wall_of_flesh_mouth`: Modular 16-VPU rig composed of tiled hollowed crimson flesh/sinew/rib `wall_slab.vox` panels spanning the wall height & width, swaying `tendril.vox` appendages, and gaping `maw_upper.vox` + animated chomping `maw_lower.vox` with fang rows (`idle`).
- `wall_of_flesh_eye`: Large bloodshot sculpted eyeball on a fleshy optic stalk with veined sclera, glowing crimson iris, black pupil (`idle`).
- `the_hungry`: Tethered fleshy crimson-and-sinew snap-maw with dark stalk and animated chomping `jaw` ringed with fangs (`idle`).
- `leech`: Segmented burgundy-violet underworld worm with ridged rings, white mouth fangs, and undulating `tail` (`idle`).

### Client Integration (`crates/client/src/view/enemies.rs`)
- Remove `has_procedural_rig`, `spawn_procedural_enemy`, `proc_mat`, `proc_part`, `proc_part_rot`, `BirdWing`, `ProcWing`, `ProcLimb`, `ProcJaw`, `ProcTendril`, and the procedural child queries in `animate`.
- Update `model_dir(kind)` to map all 63 `EnemyKind`s to their own unique snake_case folder under `assets/models/`.
- Update `model_scale(kind)` so each new `.vox` rig matches its gameplay hitbox proportions (most new rigs authored at native 16-VPU size can use `1.0`, while humanoids keep `1.1` matching `Zombie`/`Guide`).
- Update `tint_for(kind)` to return `SLIME_TINT` for `Slime` and `Color::WHITE` for all other bespoke `.vox` entities; apply `AlphaMode::Blend` and alpha-preserving hurt flash (`enemy_tint_for`) to all 5 slimes (`Slime`, `LavaSlime`, `IceSlime`, `JungleSlime`, `KingSlime`).
- Update `anim_for(kind, moving)` to select `"hop"` for all slimes (`Slime`, `LavaSlime`, `IceSlime`, `JungleSlime`, `KingSlime`) and `Bunny`, `"walk"` for all walking humanoids/goblins/skeletons/NPCs/critters (`Crab`, `SnowFlinx`, `Squirrel`, `CaveCritter`, `BloodZombie`, `Goblin*`, `Skeleton`, `AngryBones`, `DarkCaster`, `OldMan`, `Nurse`, `Demolitionist`, `Dryad`, `GoblinTinkerer`), and `"fly"` for `Bird` when `is_bird_airborne(...)` is true (else `"idle"`).
- Update unit tests in `crates/client/src/view/enemies.rs` so they verify:
  - All 63 `EnemyKind`s have a **unique** `model_dir`, valid `rig.ron`, valid `.vox` files, and spawn successfully.
  - `Bunny` rig has 4 leg parts and `hop` animation.
  - `Bird` rig has `idle` (folded wings) and `fly` (spread flapping wings) and switches animations when airborne.
  - All 5 slimes preserve alpha and base color on telegraph/approach and flash on hurt.

## 6. Verification Plan
1. Run `python3 tools/art/build.py` — must exit 0 and generate all 55 new `assets/models/<kind>/` directories.
2. Run `python3 -I tools/art/check_art.py` — must report `ok` for all families (`enemies`, `boss`, `characters`, etc.) with 0 errors.
3. Run `cargo test -p terravibe-client view::enemies` and `cargo test --workspace` — all tests must pass.
4. Run `cargo clippy --workspace --all-targets` — no new warnings.

## 7. Left to the Builder
- Exact per-voxel color hex values, shading gradients, and voxel coordinates within each model's silhouette and family voxel budget.
- Exact keyframe angles/timings in each `rig.ron` (as long as animation names match `anim_for` and motion reads clearly).
- Exact `detached` allowances in `tools/art/families.py` for models with intentionally floating interior details (e.g., slime eyes/cores/ice shards/ninja inside translucent gel).
