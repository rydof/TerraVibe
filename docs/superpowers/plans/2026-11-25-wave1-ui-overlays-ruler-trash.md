# Wave 1: UI Overlays, Ruler & Trash Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the broken Trash slot, rebuild the NPC/Guide Crafting overlay with clean flexbox layout, upgrade the Boss Healthbar to Terraria style for all 6 bosses, make the 32-block Ruler genuinely usable, and audit remaining HUD/menu panels for text-block issues.

**Architecture:** Server-authoritative `SlotRef::Trash` and `ClientMsg::TrashShift` live in `terravibe_sim` and `terravibe_server`, synced to `LocalTrash` in `terravibe_client`. UI overlays (`town.rs`, `boss_bar.rs`, `inventory.rs`) use Bevy flexbox containers styled via `ui_theme.rs` with procedural 32×32 icons from `art/ui_icons.rs` and `art/item_icons.rs`. The 3D Ruler extends `interact.rs` raycasting to 32 blocks with `RulerAnchor` pinning and a top-center HUD pill.

**Tech Stack:** Rust (2024 edition), Bevy 0.20, `terravibe_sim`, `terravibe_server`, `terravibe_client`.

**Spec:** `docs/superpowers/specs/2026-11-25-wave1-ui-overlays-ruler-trash-design.md`

## Global Constraints
- `crates/sim` and `crates/server` have zero Bevy, rendering, or audio dependencies.
- `Settings` and `InputBindings` remain `Copy`.
- Use `Option<Gizmos>` in Bevy systems called by unit tests.
- Zero copyrighted assets or local absolute paths.
- Keep `cargo fmt --check`, `cargo check --workspace --all-targets` (0 warnings), and `cargo test --workspace -- --test-threads=1` passing.

## Review Focus
1. **Trash valuable-item safety**: Shift-trashing or clicking Trash with an item worth `>= 100` copper (`VALUABLE_TRASH_THRESHOLD_COPPER`) must require double-click confirmation on click, and favorited (`Alt+Click`) bag items must never be quick-trashed.
2. **Trash retrieval & overwrite**: Clicking Trash with an empty cursor retrieves the last trashed item; clicking Trash with a held item overwrites (destroys) the previous trash item; disconnecting/tearing down clears trash.
3. **Guide Crafting & NPC tab switching**: Switching between Guide `Talk`, `Crafting`, and `Housing` (or Tinkerer `Reforge`) must never leave stale rows, overlapping text, or stuck `GuideSlot` items when closed.
4. **Boss bar priority & 6-boss coverage**: When multiple bosses or segmented bosses (`EaterOfWorlds`) are active, HP sums/displays accurately for all 6 Pre-Hardmode bosses without panicking on zero `life_max`.
5. **Ruler anchor & raycast bounds**: Aiming at sky/air while unpinned shows `"Ruler: ON (aim at block)"`, while pinned shows `Anchor -> Aimed` or clears cleanly when `RulerEnabled` is toggled off.

---

### Task 1: Server-Authoritative Trash Slot (`sim` + `server`)

**Files:**
- Modify: `crates/sim/src/slots.rs`
- Modify: `crates/sim/src/net.rs`
- Modify: `crates/server/src/lib.rs`

**Interfaces:**
- Produces: `SlotRef::Trash`, `ClientMsg::TrashShift { slot: u8 }`, `ServerMsg::Trash(Option<ItemStack>)`.

- [ ] **Step 1: Write failing sim & server tests for `SlotRef::Trash` and `TrashShift`**

Add unit test `trash_slot_stores_retrieves_and_overwrites` in `crates/sim/src/slots.rs` and integration test `server_trash_slot_and_shift_trash` in `crates/server/src/lib.rs` asserting:
- `ClickSlot { at: SlotRef::Trash, kind: ClickKind::Left }` moves held item to `trash` and clears `held`, then second click with empty cursor retrieves `trash` back to `held`.
- `ClientMsg::TrashShift { slot: 0 }` moves bag slot 0 into `trash` and clears bag slot 0, emitting `ServerMsg::Trash(Some(...))`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p terravibe_sim trash_slot && cargo test -p terravibe_server server_trash`
Expected: FAIL with missing `SlotRef::Trash` / `TrashShift` / `ServerMsg::Trash`.

- [ ] **Step 3: Implement `SlotRef::Trash`, `ClientMsg::TrashShift`, `ServerMsg::Trash`, and server handlers**

- In `crates/sim/src/slots.rs`: add `SlotRef::Trash` variant (accepted for any `ItemStack`).
- In `crates/sim/src/net.rs`: add `ClientMsg::TrashShift { slot: u8 }` and `ServerMsg::Trash(Option<ItemStack>)`.
- In `crates/server/src/lib.rs`: add `trash: Option<ItemStack>` to client state (initialized `None`, cleared on teardown), handle `ClickSlot { at: SlotRef::Trash, kind }` and `ClientMsg::TrashShift { slot }`, and send `ServerMsg::Trash(c.trash)`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p terravibe_sim trash_slot && cargo test -p terravibe_server server_trash`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sim/src/slots.rs crates/sim/src/net.rs crates/server/src/lib.rs
git commit -m "feat(sim,server): add server-authoritative Trash slot and TrashShift"
```

---

### Task 2: Client Trash Wiring, Quick-Trash & Valuable Confirmation

**Files:**
- Modify: `crates/client/src/net.rs`
- Modify: `crates/client/src/hud/inventory.rs`

**Interfaces:**
- Consumes: `SlotRef::Trash`, `ClientMsg::TrashShift`, `ServerMsg::Trash` from Task 1.
- Produces: `LocalTrash(pub Option<ItemStack>)` resource in `crates/client/src/net.rs`.

- [ ] **Step 1: Write failing client tests for `LocalTrash` sync and bag `Shift+Click` quick-trash**

In `crates/client/src/hud/inventory.rs` tests:
- Assert `LocalTrash` updates on `ServerMsg::Trash`.
- Assert clicking `TRASH_CELL` with a cheap held item sends `ClientMsg::ClickSlot { at: SlotRef::Trash, kind: ClickKind::Left }`.
- Assert clicking `TRASH_CELL` with a valuable held item (`>= VALUABLE_TRASH_THRESHOLD_COPPER`) sets `pending_trash_confirm` on first click and sends `ClickSlot` on second click.
- Assert `Shift+Click` on a non-favorited bag slot when chest/shop/guide are closed sends `ClientMsg::TrashShift { slot }`, while favorited slots are ignored.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p terravibe_client trash`
Expected: FAIL.

- [ ] **Step 3: Implement `LocalTrash` and client Trash/Quick-Trash wiring**

- In `crates/client/src/net.rs`: add `LocalTrash(pub Option<ItemStack>)`, initialize it, handle `ServerMsg::Trash(t)`, and reset it in `session_teardown`.
- In `crates/client/src/hud/inventory.rs`:
  - Replace local-only `ui.trash` with `LocalTrash` for rendering the trash slot icon/count.
  - Send `ClientMsg::ClickSlot { at: SlotRef::Trash, kind: ClickKind::Left }` on confirmed trash click.
  - Route `Shift+Click` on non-favorited bag slots (when chest, shop, and guide crafting are closed) to `ClientMsg::TrashShift { slot }`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p terravibe_client trash`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/client/src/net.rs crates/client/src/hud/inventory.rs
git commit -m "feat(client): wire LocalTrash to server and support Shift+Click quick-trash"
```

---

### Task 3: Procedural Boss Emblems & All-6 Terraria Boss Healthbar

**Files:**
- Modify: `crates/client/src/art/ui_icons.rs`
- Modify: `crates/client/src/hud/boss_bar.rs`

**Interfaces:**
- Produces: `BossEmblem` enum and `boss_emblem_icon(emblem: BossEmblem) -> RgbaImage` in `crates/client/src/art/ui_icons.rs`.

- [ ] **Step 1: Write failing tests for all-6 boss detection and boss emblem icons**

- In `crates/client/src/art/ui_icons.rs`: test `boss_emblem_icon` returns 32×32 non-empty `RgbaImage` for all 6 `BossEmblem` variants (`KingSlime`, `EyeOfCthulhu`, `EaterOfWorlds`, `QueenBee`, `Skeletron`, `WallOfFlesh`).
- In `crates/client/src/hud/boss_bar.rs`: test `active_boss_info` detects `KingSlime` and `QueenBee` from `ClientEnemy` in addition to `EyeOfCthulhu`, `EaterOfWorlds`, `Skeletron`, and `WallOfFlesh`, and test `bar_fill(0, 0) == 0.0`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p terravibe_client boss_emblem && cargo test -p terravibe_client boss_bar`
Expected: FAIL.

- [ ] **Step 3: Implement `boss_emblem_icon` and redesign `boss_bar.rs`**

- In `crates/client/src/art/ui_icons.rs`: add `BossEmblem` enum and `boss_emblem_icon` generating distinct 32×32 pixel-art emblems for all 6 bosses.
- In `crates/client/src/hud/boss_bar.rs`:
  - Add `KingSlime` and `QueenBee` detection in `update`.
  - Rebuild `BossBarRoot` as a framed horizontal flex bar (`height: 28px`, `GOLD_BORDER`, dark track, 36×36 left emblem badge, main fill + trailing damage-drain ghost bar, crisp name + phase badge and `HP / Max (XX%)` label).
  - Frame `BossTitleCard` with `PANEL_BG_SOLID` and `GOLD_BORDER`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p terravibe_client boss_emblem && cargo test -p terravibe_client boss_bar`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/client/src/art/ui_icons.rs crates/client/src/hud/boss_bar.rs
git commit -m "feat(hud): upgrade Boss Healthbar with 6-boss support and procedural emblems"
```

---

### Task 4: Flexbox NPC & Guide Crafting / Shop / Reforge / Housing Overlay

**Files:**
- Modify: `crates/client/src/hud/town.rs`

**Interfaces:**
- Consumes: `LocalGuideSlot`, `ItemIcons`, `ui_theme.rs` tokens.

- [ ] **Step 1: Write failing layout & state tests for the rebuilt `town.rs` overlay**

- Test that `TownPanel` tab switching (`Talk`, `Crafting`, `Reforge`, `Housing`) updates active view visibility cleanly.
- Test `guide_crafting_recipes_for` returns matching recipes with ingredient counts and station names without leading-space padding hacks.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p terravibe_client town::tests`
Expected: FAIL.

- [ ] **Step 3: Rebuild `TownRoot` in `crates/client/src/hud/town.rs` using Bevy flexbox**

- Replace monolithic `TownLabel` + absolute icon overlays with:
  - **Header flex row**: NPC speaker badge + Tab strip (`Talk`, `Crafting`, `Reforge`, `Housing`) + `[X]` close button.
  - **Talk view**: Wrapped dialogue card + horizontal flex row of styled action buttons (`[Advice]`, `[Crafting]`, `[Heal]`, `[Shop]`, `[Reforge]`, `[Close]`).
  - **Guide Crafting view**: Two-column flex layout — left 44×44 Material Slot + caption; right paginated/scrollable clickable Recipe Row cards (24×24 output icon, item name + count, ingredient list with have/need coloring, station pill).
  - **Shop view**: Scrollable clickable item rows with 24×24 icon, rarity-colored name, stat-comparison badge, formatted `p/g/s/c` coin price, hover tooltip, and `[1x] / [10x] / [All]` footer.
  - **Reforge view**: Structured reforgeable item rows with icon, prefix + name, cost pill, `[Reforge]` button, and result banner.
  - **Housing view**: Clean status badge + room bounds readout.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p terravibe_client town::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/client/src/hud/town.rs
git commit -m "feat(hud): rebuild NPC & Guide Crafting overlay with clean flexbox layout"
```

---

### Task 5: Upgraded 32-Block Ruler with Anchor Pinning, 3D Grid/Ticks & Top Pill

**Files:**
- Modify: `crates/client/src/input_map.rs`
- Modify: `crates/client/src/hud/inventory.rs`
- Modify: `crates/client/src/view/interact.rs`

**Interfaces:**
- Produces: `GameAction::RulerPin` (default `KeyCode::KeyR`), `RulerAnchor(pub Option<[i32; 3]>)`, `RULER_RAY_MAX: f32 = 32.0`.

- [ ] **Step 1: Write failing tests for `GameAction::RulerPin`, `RulerAnchor`, and 32-block readout**

- In `crates/client/src/input_map.rs`: test `InputBindings::default().key(GameAction::RulerPin) == KeyCode::KeyR` and no keybind collisions.
- In `crates/client/src/view/interact.rs`: test `compute_ruler_readout(start, end)` returns signed `[dx, dy, dz]`, inclusive `[w, h, d]`, and Euclidean distance.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p terravibe_client ruler`
Expected: FAIL.

- [ ] **Step 3: Implement `RulerPin`, `RulerAnchor`, 32-block raycast, 3D grid/ticks, and top HUD pill**

- In `crates/client/src/input_map.rs`: add `GameAction::RulerPin` bound to `KeyCode::KeyR` (ensure `InputBindings` stays `Copy`).
- In `crates/client/src/view/interact.rs`:
  - Raycast up to `RULER_RAY_MAX = 32.0` when `RulerEnabled(true)`.
  - Toggle/clear `RulerAnchor` on `GameAction::RulerPin` press.
  - Draw 5×5 block-aligned grid on aimed face, color-coded X/Y/Z axes with 1-block tick marks, and start/end wireframe cubes via `Option<Gizmos>`.
- In `crates/client/src/hud/inventory.rs`: replace floating `RulerHudText` at `win[1] - 42.0` with a styled obsidian-slate HUD pill docked near top-center (`top: 44px`) showing `ΔX`, `ΔY`, `ΔZ`, `W×H×D`, and distance.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p terravibe_client ruler && cargo test -p terravibe_client input_map`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/client/src/input_map.rs crates/client/src/hud/inventory.rs crates/client/src/view/interact.rs
git commit -m "feat(ruler): extend raycast to 32 blocks with anchor pinning, 3D grid, and top pill"
```

---

### Task 6: Remaining HUD/Menu Text-Block Audit & Full Verification Gate

**Files:**
- Modify (as needed from audit): `crates/client/src/hud/chest.rs`, `crates/client/src/hud/chat.rs`, `crates/client/src/hud/tooltip.rs`, `crates/client/src/hud/inventory.rs`, `crates/client/src/menu/game.rs`, `crates/client/src/menu/options.rs`

- [ ] **Step 1: Audit remaining HUD & menu panels for unstyled text blocks, clipped labels, or overlaps**

Inspect `chest.rs`, `chat.rs`, `tooltip.rs`, `inventory.rs` crafting sidebar, `menu/game.rs`, and `menu/options.rs` and fix any plain unstyled text blocks or overlapping labels.

- [ ] **Step 2: Run full workspace fmt, check, and test suite**

Run: `cargo fmt --check && cargo check --workspace --all-targets && cargo test --workspace -- --test-threads=1`
Expected: 0 formatting diffs, 0 compiler warnings, all tests passing.

- [ ] **Step 3: Run headless visual QA scenarios**

Run: `./tools/qa-headless.sh` (or targeted scenarios `town_housing`, `tooltips_economy`, `combat_feedback`, `first_hour`, `menu_title`) and inspect screenshots to confirm zero overlapping text and clean UI styling.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "chore(ui): polish remaining HUD/menu panels and pass Wave 1 QA gate"
```
