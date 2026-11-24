# Wave 1: UI Overlays, Ruler & Trash Fixes Design

**Date:** 2026-11-25
**Scope:** Project (Server-authoritative Trash slot, Flexbox NPC/Guide Crafting/Shop/Reforge/Housing overlay, Terraria-style 6-boss Healthbar, 32-block Ruler with anchor pinning & 3D grid, and HUD/Menu text-block cleanliness audit).

## 1. Intent & Why

Playtesting revealed several high-friction UX bugs and basic-text-block overlays that break immersion and usability:
- **Trash slot is broken**: Clicking the trash slot in `inventory.rs` only clears the client's local cursor; the server never receives a message, so items are not actually discarded.
- **Guide Crafting & NPC overlay has overlapping text**: `town.rs` dumps all text into one monolithic `HudText` node while tabs, speaker title, Guide material slot, and recipe/shop icons are `PositionType::Absolute` overlays using hardcoded leading spaces, causing `"Guide — Crafting"` to collide with tab buttons and text to draw over text.
- **Boss Healthbar is basic and incomplete**: `boss_bar.rs` renders a plain 16px red bar with floating text and omits **King Slime** and **Queen Bee**.
- **Ruler is limited and collides with HUD**: `interact.rs` only raycasts within 5-block placement reach from player feet with 4 thin lines, and `RulerHudText` floats unstyled at `win[1] - 42.0` where it collides with the boss bar and context prompts.
- **Remaining UIs**: Several HUD/menu panels need a cleanliness pass to ensure no plain text-block dumps, clipped labels, or overlapping elements remain.

## 2. Approaches Considered

1. **Approach 1 (Selected): 4 Focused Sub-Project Waves (Wave 1 → 2 → 3 → 4)**
   - Wave 1: UI, Overlays, Ruler & Trash Fixes (this spec).
   - Wave 2: Weapon Animations, 3rd-Person/MP Grip & Core Feel (physics, liquids, magnetic floor pickups).
   - Wave 3: Armor Overhaul, Set-Bonus FX (Shadow/Molten/etc.) & Complete Pre-Hardmode Accessories with worn 3D details.
   - Wave 4: Full Enemy & Item `.vox` Model Audit against Terraria.
   - *Why selected:* Each wave is independently testable, avoids cross-system merge conflicts, and fixes the most painful playtest bugs first.
2. **Approach 2: Single Master Mega-Plan**
   - *Trade-off:* One upfront review, but too large to keep crisp and verify incrementally.
3. **Approach 3: Quick-Fix Only, Deferring Content/Art**
   - *Trade-off:* Faster to playable polish, but leaves accessory and art parity unfinished.

## 3. Technical Design

### 3.1 Server-Authoritative Trash Slot
- **Sim (`crates/sim/src/slots.rs`, `crates/sim/src/net.rs`)**:
  - Add `SlotRef::Trash` to `SlotRef`.
  - Add `ServerMsg::Trash(Option<ItemStack>)` and `ClientMsg::TrashShift { slot: u8 }`.
- **Server (`crates/server/src/lib.rs`)**:
  - Add `trash: Option<ItemStack>` to per-client state (cleared on disconnect/teardown, never persisted across sessions, matching Terraria).
  - `ClickSlot { at: SlotRef::Trash, kind }`:
    - `Left` with held stack → moves held stack into `c.trash` (destroying any previous `c.trash`) and clears `c.held`.
    - `Left` with empty cursor → moves `c.trash` into `c.held` and clears `c.trash`.
  - `TrashShift { slot }`: moves non-empty bag stack `slot` into `c.trash` (destroying previous `c.trash`), syncing `Inventory` and `ServerMsg::Trash`.
- **Client (`crates/client/src/net.rs`, `crates/client/src/hud/inventory.rs`)**:
  - Store `LocalTrash(pub Option<ItemStack>)` updated by `ServerMsg::Trash`.
  - Clicking `TRASH_CELL` sends `ClientMsg::ClickSlot { at: SlotRef::Trash, kind: ClickKind::Left }` (preserving `VALUABLE_TRASH_THRESHOLD_COPPER` = `100` copper double-click confirmation).
  - `Shift+Click` (or `Ctrl+Click`) on a non-favorited bag slot when chest, shop, and guide crafting are all closed sends `ClientMsg::TrashShift { slot }`.

### 3.2 Clean Flexbox NPC & Guide Crafting Overlay (`crates/client/src/hud/town.rs`)
- Replace the monolithic `TownLabel` + absolute icon overlays with structured Bevy flexbox containers styled with `ui_theme.rs` (`PANEL_BG_SOLID`, `PANEL_BORDER`, `GOLD_BORDER`, `BORDER_W`, `RADIUS_PANEL`).
- **Header Flex Row**: NPC name badge + Tab strip (`Talk`, `Crafting` for Guide, `Reforge` for Tinkerer, `Housing`) + `[X]` close button — zero overlap regardless of NPC name length.
- **Talk View**: Wrapped dialogue card + horizontal flex row of styled action buttons (`[Advice]`, `[Crafting]`, `[Heal — <cost>]`, `[Shop]`, `[Reforge]`, `[Close]`).
- **Guide Crafting View**: Two-column flex layout — left column has the 44×44 Material Slot + helper caption; right column has paginated/scrollable clickable Recipe Row cards (24×24 output icon, item name + output count, ingredient list with have/need coloring, station pill). Zero leading-space hacks, zero text-on-text overlap.
- **Shop View**: Scrollable clickable item rows with 24×24 item icon, rarity-colored name, stat-comparison badge, formatted `p/g/s/c` coin price, hover tooltip support, and `[1x] / [10x] / [All]` footer hints.
- **Reforge View (Goblin Tinkerer)**: Structured reforgeable item rows with icon, current prefix + name, reforge cost pill, clickable `[Reforge]` action, and live prefix-result banner.
- **Housing View**: Clean status badge (Valid / Invalid reason) + room bounds readout.

### 3.3 Terraria-Style Boss Healthbar (`crates/client/src/hud/boss_bar.rs`, `crates/client/src/art/ui_icons.rs`)
- Support **all 6 Pre-Hardmode bosses**: `KingSlime`, `EyeOfCthulhu`, `EaterOfWorlds`, `QueenBee`, `Skeletron`, `WallOfFlesh`.
- Add procedural 32×32 boss emblem icons in `crates/client/src/art/ui_icons.rs` (`BossEmblem` enum).
- Redesign `BossBarRoot` as a framed horizontal flex bar at bottom-center (`height: 28px`, gold outer border, dark recessed track, left 36×36 boss emblem badge, main fill bar + smooth trailing damage-drain ghost bar, crisp name + phase badge and `HP / Max (XX%)` label).
- Upgrade `BossTitleCard` intro banner with framed gold accents.

### 3.4 Working Terraria-Style Ruler (`crates/client/src/view/interact.rs`, `crates/client/src/hud/inventory.rs`, `crates/client/src/input_map.rs`)
- Extended 32-block raycast (`RULER_RAY_MAX = 32.0`) when `RulerEnabled(true)`.
- Anchor pinning resource `RulerAnchor(pub Option<[i32; 3]>)`: pressing `GameAction::RulerPin` (default `KeyCode::KeyR`) while Ruler is ON pins the aimed block (or clears if already pinned at that block / aiming at air).
- `RulerReadout` stores start block, end block, signed `ΔX/ΔY/ΔZ`, inclusive dimensions `W×H×D`, and Euclidean distance.
- 3D gizmos draw: a subtle 5×5 block-aligned grid on the aimed face, color-coded X/Y/Z measurement axes with 1-block tick marks, and highlighted start/end wireframe cubes.
- Replace unstyled floating `RulerHudText` at `win[1] - 42.0` with a styled obsidian-slate HUD pill docked near top-center below the compass (`top: 44px`), never colliding with the boss bar or context prompts.

### 3.5 UI Cleanliness Audit & Visual QA
- Audit `chest.rs`, `chat.rs`, `tooltip.rs`, `hud/inventory.rs` crafting sidebar, `menu/game.rs`, and `menu/options.rs` for any unstyled text blocks, overlapping text, or clipped labels.
- Verify via `cargo fmt --check`, `cargo check --workspace --all-targets` (0 warnings), `cargo test --workspace -- --test-threads=1`, and `./tools/qa-headless.sh` (`town_housing`, `tooltips_economy`, `combat_feedback`, `first_hour`, `menu_title`).

## 4. Global Constraints
- `crates/sim` and `crates/server` have zero Bevy, rendering, or audio dependencies.
- `Settings` and `InputBindings` remain `Copy`.
- Use `Option<Gizmos>` in Bevy systems called by unit tests.
- Zero copyrighted assets or local absolute paths.
