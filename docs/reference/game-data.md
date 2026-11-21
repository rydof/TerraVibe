# Game data and tooling reference

Facts that stay useful across milestones. Code is authoritative; update this file when a
milestone changes any of it.

## Crates

- `crates/sim`: world, blocks, items, crafting, player physics, enemies, bosses, town NPCs.
  No Bevy, rendering or audio.
- `crates/server`: authoritative tick loop (60 ticks/s), saves, world catalogue. No Bevy.
- `crates/client`: Bevy 0.20 app: rendering, HUD, menus, input, audio, QA driver.
- `crates/assets`: reads Terraria XNB/XWB files from the local pack or install (HUD icons,
  fonts, sounds only).

## Ids (Terraria's own numbering)

- Tiles: Dirt 0, Stone 1, Grass 2, Mushroom plant 3, Iron 6, Copper 7, Gold 8, Silver 9,
  Closed door 10, Open door 11, Life Crystal 12, Bottles 13, Tables 14, Chairs 15, Anvil 16,
  Furnace 17, Workbench 18, Platforms 19, Chest 21, Demonite ore 22, Corrupt grass 23, Ebonstone 25,
  Demon altar 26, Shadow Orb 31, Blue Dungeon Brick 41, Green Dungeon Brick 43, Pink Dungeon Brick 44,
  Spikes 48, Glass 54, Obsidian 56, Ash 57, Hellstone 58, Obsidian Brick 75, Hellstone Brick 76, Hellforge 77.
- Items: Iron Pickaxe 1, Dirt Block 2, Stone Block 3, Iron Broadsword 4, Mushroom 5, Torch 8, Wood 9, Iron Axe 10,
  ores 11–14 (Iron, Copper, Gold, Silver), bars 19–22 (Gold, Copper, Silver, Iron), Gel 23, Wooden Sword 24, Wooden Door 25,
  Lesser Healing Potion 28, Life Crystal 29, Bottle 31, Wooden Table 32, Furnace 33, Wooden Chair 34, Iron Anvil 35,
  Work Bench 36, Lens 38, Wooden Bow 39, Wooden Arrow 40, Flaming Arrow 41, Suspicious Looking Eye 43, Demon Bow 44,
  War Axe of the Night 45, Light's Bane 46, Chest 48, Band of Regeneration 49, Magic Mirror 50, Cloud in a Bottle 53,
  Hermes Boots 54, Enchanted Boomerang 55, Demonite Ore 56, Demonite Bar 57, Heart 58, Ebonstone Block 61, Vilethorn 64,
  Worm Food 70, coins 71–74 (Copper, Silver, Gold, Platinum), Fallen Star 75, armor 76–83 & 89–92 (Greaves, Chainmail, Helmets for Copper, Iron, Silver, Gold),
  Shadow Scale 86, Wood Wall 93, Wood Platform 94, Flintlock Pistol 95, Musket 96, Musket Ball 97, Minishark 98, Shadow Greaves 100, Shadow Scalemail 101, Shadow Helmet 102,
  Nightmare Pickaxe 103, The Breaker 104, Mana Crystal 109, Band of Starpower 111, Magic Missile 113, Shadow Orb 115, Flamarang 119,
  Molten Fury 120, Fiery Greatsword 121, Molten Pickaxe 122, Bone 154, Muramasa 155, Cobalt Shield 156, Aqua Scepter 157,
  Lucky Horseshoe 158, Shiny Red Balloon 159, Ball O' Hurt 162, Blue Moon 163, Handgun 164, Sand Block 169, Glass 170, Ash Block 172,
  Obsidian 173, Hellstone 174, Hellstone Bar 175, Mud Block 176, Obsidian Brick 192, Obsidian Skull 193, Wooden Hammer 196,
  Empty Bucket 205, Water Bucket 206, Lava Bucket 207, Hellstone Brick 214, Molten Hamaxe 217, Phoenix Blaster 219, Hellforge 221,
  Molten Helmet 231, Molten Breastplate 232, Molten Greaves 233, Guide Voodoo Doll 267, Spear 274, Obsidian Skin Potion 288,
  Golden Key 327, Shadow Key 329, Pwnhammer 367, Breaker Blade 426, Clockwork Assault Rifle 434, Sorcerer Emblem 489, Warrior Emblem 490,
  Ranger Emblem 491, Laser Rifle 514, Snow Block 593, Ice Block 664, Lava Charm 906, Wand of Sparking 3069, Sandstone Block 3271,
  Hardened Sand Block 3272, Valor 3317, Copper Axe 3506, Copper Shortsword 3507, Copper Broadsword 3508, Copper Pickaxe 3509,
  Silver Axe 3512, Silver Broadsword 3514, Silver Pickaxe 3515, Gold Axe 3518, Gold Broadsword 3520, Gold Pickaxe 3521, Firecracker 4912 (143 total).
- Buffs: Potion Sickness 21 (3600 ticks / 60 s), On Fire! 24 (420 ticks / 7 s, 4 hp/s DoT), Obsidian Skin (21,600 ticks / 6 min lava & hot-block immunity), Lava Ward (420 ticks / 7 s lava damage delay).
- NPCs: Eye of Cthulhu 4, Servant of Cthulhu 5, Eater of Souls 6, Giant Worm 10, Eater of Worlds Head 13,
  Eater of Worlds Body 14, Eater of Worlds Tail 15, Merchant 17, Arms Dealer 19, Skeleton 21, Guide 22, Fire Imp 24, Angry Bones 31,
  Dark Caster 32, Cursed Skull 34, Skeletron Head 35, Skeletron Hand 36, Old Man 37, Bone Serpent Head 39, Bone Serpent Body 40, Bone Serpent Tail 41, Cave Bat 49, Lava Slime 59, Hellbat 60, Demon 62, Voodoo Demon 66, Dungeon Guardian 68, Bunny, Bird, Squirrel, Cave Critter, Wall of Flesh Mouth 113, Wall of Flesh Eye 114, The Hungry 115, Leech 117.
- Recipes: `RecipeId` is append-only; never renumber (72 total, 0..=71). Empty Bucket 46 (3 Iron Bar at Anvil),
  Demonite Bar 47, War Axe of the Night 48, Light's Bane 49, Shadow Helmet 50, Shadow Scalemail 51,
  Shadow Greaves 52, Nightmare Pickaxe 53, Mana Crystal 54 (5 Fallen Stars), Wooden Hammer 55 (8 Wood at Workbench),
  The Breaker 56 (10 Demonite Bars at Anvil), Demon Bow 57 (8 Demonite Bars at Anvil), Hellstone Bar 58 (3 Hellstone + 1 Obsidian at Hellforge),
  Molten Helmet 59, Molten Breastplate 60, Molten Greaves 61, Molten Pickaxe 62, Molten Hamaxe 63, Fiery Greatsword 64,
  Molten Fury 65, Phoenix Blaster 66, Flamarang 67, Obsidian Skull 68 (20 Obsidian at Furnace), Obsidian Brick 69 (2 Obsidian at Furnace),
  Hellstone Brick 70 (1 Stone + 1 Hellstone at Furnace), Worm Food 71 (15 Shadow Scale + 15 Ebonstone Block at Demon Altar).

## Underworld (Milestone 9)

- World Generation & Structures:
  - Deepest band of the world: `y <= UNDERWORLD_TOP_Y` (`UNDERWORLD_TOP_Y = 24`).
  - Stone replaced with Ash (`ASH = BlockId(57)`).
  - Rich Hellstone veins (`HELLSTONE = BlockId(58)`) throughout ash terrain; mining requires pickaxe power >= 65 (Nightmare Pickaxe).
  - Voluminous lava lakes and bottom lava oceans (`LiquidKind::Lava`).
  - Ruined obsidian houses generated across the layer constructed from `OBSIDIAN_BRICK` (BlockId 75) and `HELLSTONE_BRICK` (BlockId 76), housing Hellforges (`HELLFORGE = BlockId(77)`) and locked Shadow Chests.
- Hazards & Protective Gear:
  - Lava contact deals 80 damage and applies burning.
  - Lava Charm (ItemId 906): accessory granting 420 ticks (7 s) delay before taking lava damage.
  - Obsidian Skin Potion (ItemId 288): consumable granting 21,600 ticks (6 min) complete immunity to lava damage and hot tile burn.
  - Hot tiles (Hellstone, Meteorite): walking on hot blocks inflicts burn DoT unless protected by `fire_walk`.
  - Obsidian Skull (ItemId 193): accessory preventing burn from Hellstone and Meteorite blocks (`fire_walk: true`).
  - Shadow Chests: unlocked using Shadow Key (ItemId 329); key is reusable and NOT consumed upon opening.
- Underworld Enemies & Spawning:
  - Depth-gated spawning: when player `y <= UNDERWORLD_TOP_Y`, natural spawner spawns exclusively from `UNDERWORLD_POOL`.
  - Fire Imp (`EnemyKind::FireImp`, ID 24; HP 70, dmg 30, def 16): teleports to solid ground and fires burning Fireball projectiles.
  - Demon (`EnemyKind::Demon`, ID 62; HP 120, dmg 34, def 8): flying bat/demon AI attacking the player.
  - Voodoo Demon (`EnemyKind::VoodooDemon`, ID 66; HP 140, dmg 32, def 10): flying demon carrying Guide Voodoo Doll; guaranteed 100% drop of Guide Voodoo Doll (ItemId 267).
  - Lava Slime (`EnemyKind::LavaSlime`, ID 59; HP 50, dmg 15, def 10): lava-immune hopping slime.
  - Hellbat (`EnemyKind::Hellbat`, ID 60; HP 46, dmg 35, def 8): fast erratic flying bat.
  - Bone Serpent: 6-segment burrowing worm (`BoneSerpentHead` ID 39, `BoneSerpentBody` ID 40, `BoneSerpentTail` ID 41; HP 300 each, moves through solid terrain toward player).
- Visuals & Audio:
  - Procedural voxel rigs and animations for all 6 Underworld enemies (demon wings, imp horns, serpent vertebrae segments).
  - Fireball projectile rendering with orange emissive core and flame trail.
  - Underworld ambient music track playing at `y <= UNDERWORLD_TOP_Y`.
  - Authored 3D `.vox` models for all 10 M9 items: Ash (172), Obsidian (173), Hellstone (174), Obsidian Brick (192), Obsidian Skull (193), Hellstone Brick (214), Hellforge (221), Guide Voodoo Doll (267), Obsidian Skin Potion (288), Lava Charm (906).

## Wall of Flesh (Milestone 11)

- Summon Mechanic:
  - Dropping a Guide Voodoo Doll (`ItemId(267)`) into Underworld lava (`y <= UNDERWORLD_TOP_Y`, i.e. `y <= 24`) while the Guide NPC is alive (`town_alive(TownKind::Guide)`).
  - The Guide dies immediately (`Guide` death announcement).
  - Wall of Flesh spawns at the nearer world X boundary and sweeps horizontally toward the opposite boundary across the Underworld (`y = 0..=24`).
- Boss Parts & Multi-Part AI:
  - 8,000 HP shared pool across `WallOfFleshMouth` (113) and two `WallOfFleshEye` (114) segments. Any damage to an Eye or Mouth reduces the shared health pool.
  - Sweep speed dynamically scales from `1.5 px/tick` (at full HP) up to `7.5 px/tick` as HP drops.
  - `WallOfFleshEye` (114): 50 contact damage, 0 defense. Periodically casts `ProjKind::EyeLaser` targeting nearest player; laser damage scales 22 / 26 / 30 and firing interval accelerates from 120 down to 50 ticks as HP drops.
  - `WallOfFleshMouth` (113): 50 contact damage, 12 defense. Spawns `Leech` (117) minions (capped at 3 concurrent).
  - `TheHungry` (115): 250 HP, 30 contact damage, 10 defense (scaling +10 dmg / +5 def per tier as HP drops); attached via tendrils to the vertical flesh wall, detaching when HP <= 80.
  - `Leech` (117): 60 HP, 26 contact damage, 2 defense; burrowing / homing worm minion spawned from mouth.
  - The Tongue & Horrified: Players behind the Wall of Flesh, > 95 blocks ahead, or leaving the Underworld (e.g. `y > 52`) are pulled back in front with 25 damage every 30 ticks; respawned players on the surface are safely ignored. Reaching the far world boundary inflicts inescapable crushing damage.
- Weapons, Emblems & Loot:
  - Guaranteed drops on defeat: 100% `PWNHAMMER` (367), exactly 1 class Emblem (33.3% each: `WARRIOR_EMBLEM` 490, `RANGER_EMBLEM` 491, `SORCERER_EMBLEM` 489), exactly 1 WoF Weapon (25% each: `BREAKER_BLADE` 426, `CLOCKWORK_ASSAULT_RIFLE` 434, `LASER_RIFLE` 514, `FIRECRACKER` 4912), 5-10 Gold coins, and 5-15 Lesser Healing Potions and Hearts. Clustered in a 3x3 box at mouth death position.
  - `PWNHAMMER` (367): hammer power 80%, 26 damage, 7.5 knockback, 26 use time, `auto_reuse`.
  - `WARRIOR_EMBLEM` (490): accessory, +15% melee damage multiplier.
  - `RANGER_EMBLEM` (491): accessory, +15% ranged damage multiplier.
  - `SORCERER_EMBLEM` (489): accessory, +15% magic damage multiplier.
  - `BREAKER_BLADE` (426): 70 melee damage, 8.0 knockback, 35 use time, 4% crit, 1.4x scale; deals 2.5x damage against targets with >= 90% HP.
  - `CLOCKWORK_ASSAULT_RIFLE` (434): 17 ranged damage, 0.0 knockback, 12 use time, 4% crit, 7.75 shoot speed, `auto_reuse`; 3-round burst firing 3 bullets for the cost of 1 ammo item.
  - `LASER_RIFLE` (514): 29 magic damage, 2.5 knockback, 12 use time, 4% crit, 14.0 shoot speed, 8 mana cost, `auto_reuse`; fires piercing `ProjKind::Laser` penetrating up to 3 enemies.
  - `FIRECRACKER` (4912): 40 melee damage, 2.0 knockback, 30 use time, 1.3x scale; 2.75x sweep reach, inflates enemy hitboxes, 50% On Fire! on primary target, 100% On Fire! + 75% splash damage to up to 2 nearby enemies.
- Victory Screen & Pre-Hardmode Preservation:
  - Defeating the Wall of Flesh sets `facts.downed_wall_of_flesh = true`, broadcasts `ServerMsg::WallOfFleshDefeated` and `ServerMsg::WofGone`.
  - Client displays Victory modal overlay ("Wall of Flesh Defeated!") allowing players to "Continue Playing" in pre-Hardmode or "Save & Return to Title".
  - The world remains strictly pre-Hardmode: no Hardmode ores, no evil biome V spread, no Hardmode enemy spawns.
  - Guide advice updates: informs player about Voodoo Doll summon beforehand, and congratulates player on defeating the ancient guardian of the Underworld afterward.
- Item Visuals:
  - Authored 3D `.vox` models for all 8 M11 items: `367.vox` (Pwnhammer), `426.vox` (Breaker Blade), `434.vox` (Clockwork Assault Rifle), `489.vox` (Sorcerer Emblem), `490.vox` (Warrior Emblem), `491.vox` (Ranger Emblem), `514.vox` (Laser Rifle), `4912.vox` (Firecracker).
  - 3D procedural mesh rigs for `WallOfFleshMouth` (massive 64-block vertical flesh wall with protruding ribs and animated jaw), `WallOfFleshEye` (spherical eye with iris/pupil), `TheHungry`, `Leech`, and laser beam visuals (`ProjKind::Laser`, `ProjKind::EyeLaser`).

## Molten Gear (Milestone 10)

- Crafting Stations & Bar:
  - `HELLFORGE = BlockId(77)`: Underworld house station; also satisfies `FURNACE` recipes. `recipe_station_name` reports "Hellforge".
  - Hellstone Bar (ItemId 175; material, max stack 9999, sell 40 silver): 3 Hellstone (174) + 1 Obsidian (173) at a Hellforge. Bare ground and a Furnace both fail with `MissingStation`.
  - Picking up a Hellforge requires pickaxe power >= 65 (Nightmare or Deathbringer Pickaxe).
- Molten Armor & Set Bonus:
  - Molten Helmet (231): head, 8 defense, +7% melee critical chance.
  - Molten Breastplate (232): body, 9 defense, +7% melee damage.
  - Molten Greaves (233): legs, 8 defense, +7% melee speed (0.93x melee/tool use time).
  - Full set: 25 defense, +17% melee damage (7% breastplate + 10% set), +7% melee crit, +7% melee speed.
  - Full set grants `fire_walk()` (immunity to hot-block burning on `HELLSTONE` and `HELLSTONE_BRICK`) and immunity to the On Fire! debuff.
  - The vanilla fiery movement trail is deferred (cosmetic only).
- Molten Weapons & Tools (all Orange rarity, sell 54 silver):
  - Molten Pickaxe (122): 12 damage, knockback 2, use 23, pickaxe power 100, 10% On Fire! on melee hit.
  - Molten Hamaxe (217): 20 damage, knockback 7, use 27, axe power 150, hammer power 70, 20% On Fire! on melee hit.
  - Fiery Greatsword (121): 40 damage, knockback 6.5, use 40, 50% On Fire; the first hit of a swing explodes for 75% damage on up to two enemies within 5 blocks, always burning them.
  - Molten Fury (120): 31 damage, knockback 2, use 22, shoot speed 8.5; Wooden Arrows become Flaming Arrows (the arrow is still consumed once), other arrows behave normally.
  - Phoenix Blaster (219): 33 damage, knockback 2, use 11, shoot speed 13, semi-automatic (non-auto), consumes bullets.
  - Flamarang (119): 49 damage, knockback 8, use 14, bounces outward then pierces on return, 50% On Fire.
  - Tool weapons with damage > 0 (`ItemClass::Tool`, including Molten Pickaxe and Hamaxe) deal melee damage on swing hits alongside their tool powers.
- Recipes (all ten):
  - Hellstone Bar: 3 Hellstone + 1 Obsidian at a Hellforge.
  - Molten Helmet: 10 Hellstone Bars at an Anvil.
  - Molten Breastplate: 20 Hellstone Bars at an Anvil.
  - Molten Greaves: 15 Hellstone Bars at an Anvil.
  - Molten Pickaxe: 20 Hellstone Bars at an Anvil.
  - Molten Hamaxe: 15 Hellstone Bars at an Anvil.
  - Fiery Greatsword: 20 Hellstone Bars at an Anvil.
  - Molten Fury: 15 Hellstone Bars at an Anvil.
  - Phoenix Blaster: 1 Handgun (164) + 10 Hellstone Bars at an Anvil.
  - Flamarang: 1 Enchanted Boomerang (55) + 15 Hellstone Bars at an Anvil.
- Authored 3D `.vox` models for all 10 M10 items (119, 120, 121, 122, 175, 217, 219, 231, 232, 233), including the Molten armor rig (helmet, breastplate, greaves).

## Dungeon and Skeletron (Milestone 8)

- Dungeon Generation & Structures:
  - Generated on outer third opposite the Jungle/Snow biome with surface entrance structure and subterranean multi-room brick chambers.
  - Tile IDs: `BLUE_DUNGEON_BRICK = BlockId(41)`, `GREEN_DUNGEON_BRICK = BlockId(43)`, `PINK_DUNGEON_BRICK = BlockId(44)`, `SPIKES = BlockId(48)`.
  - Spikes placed along room floors and ceilings; locked golden chests distributed throughout subterranean rooms.
- Old Man NPC & Dungeon Guardian:
  - Old Man (`EnemyKind::OldMan`, ID 37) stands at dungeon entrance; daytime interaction warns player; night-time interaction summons Skeletron and sacrifices Old Man.
  - Dungeon Guardian (`EnemyKind::DungeonGuardian`, ID 68; HP 9999, damage 9999, speed 20) spawns and charges players entering subterranean dungeon before Skeletron is defeated (`!downed_skeletron`).
- Skeletron Boss AI:
  - Multi-part boss: `SkeletronHead` (ID 35, HP 4400, dmg 32, def 10) + two independent `SkeletronHand` entities (ID 36, HP 600, dmg 20, def 14).
  - Hands hover flanking head and perform periodic lunges/slaps towards player.
  - Head alternates between hovering and high-speed spinning charge attack with visual rotation.
  - Defeating head despawns remaining hands, sets server flag `downed_skeletron = true`, drops loot, and permits safe dungeon entry.
- Post-Defeat Enemies & Chest Unlocking:
  - Post-defeat dungeon spawns: `AngryBones` (ID 31, HP 80, dmg 26, def 8), `DarkCaster` (ID 32, HP 50, dmg 20, def 2), `CursedSkull` (ID 34, HP 40, dmg 35, def 6).
  - Enemies drop `Bone` (154) and `GoldenKey` (327). One Dungeon chest per world deterministically guarantees `ShadowKey` (329); locked Dungeon chests never contain `GoldenKey`.
  - Locked golden dungeon chests require `GoldenKey` in player inventory; unlocks chest on interaction and consumes 1 key.
- Dungeon Weapons & Loot:
  - Muramasa (155): Fast auto-swinging melee katana emitting blue light.
  - Cobalt Shield (156): Accessory giving 1 defense and complete knockback immunity (`no_knockback: true`).
  - Aqua Scepter (157): Magic wand firing water stream projectile.
  - Handgun (164): Semi-auto pistol firing bullets with Musket Ball ammo.
  - Blue Moon (163): Heavy spiked flail projectile.
  - Magic Missile (113): Magic spark projectile dealing 35 magic damage.
  - Valor (3317): Dungeon melee weapon.
  - Shadow Key (329): Reusable key for underworld shadow chests.
  - Bone (154): Material / ammo.
- Authored 3D `.vox` drop models for all 10 M8 items (113, 154, 155, 156, 157, 163, 164, 327, 329, 3317).

## Eater of Worlds, Shadow Orbs & Demonite (Milestone 7)

- Shadow Orbs & Hammers:
  - `SHADOW_ORB = BlockId(31)` generated deep inside Corruption chasm pockets.
  - Requires hammer tool (`Wooden Hammer` ID 196, `The Breaker` ID 104) to damage and break.
  - Smashed orbs roll loot: 1st orb guarantees Musket (96) and 100 Musket Balls (97); subsequent orbs roll from Musket, Vilethorn (64), Ball O' Hurt (162), Shadow Orb (115), Band of Starpower (111).
  - Every 3rd orb broken summons Eater of Worlds.
- Eater of Worlds Boss AI & Worm Splitting:
  - Segmented 3D burrowing worm: Head (NPC 13, 150 HP, 22 dmg, 2 def), Body (NPC 14, 150 HP, 13 dmg, 4 def), Tail (NPC 15, 150 HP, 11 dmg, 8 def).
  - When intermediate body segments die, the worm splits into two independent worms (adjacent segments become new head and tail).
  - Segments drop Demonite Ore (56) and Shadow Scales (86); defeating the full worm sets `downed_eow`, drops extra boss healing potions/hearts, and blocks duplicate summons while alive (`EaterOfSouls` drops coins only).
- Weapons & Progression:
  - Musket (96): Ranged weapon consuming Musket Ball ammo (97) from ammo slots.
  - Ball O' Hurt (162): Melee flail projectile extending and returning to player.
  - Vilethorn (64): Magic piercing spear passing through solid blocks.
  - Band of Starpower (111): Accessory giving +20 max mana.
  - Nightmare Pickaxe (103): 65% pickaxe power, capable of mining Demonite Ore and Hellstone.
  - Shadow Armor (Helmet 102, Scalemail 101, Greaves 100): 19 base defense, +15% movement speed set bonus when full set is equipped.
- Authored 3D `.vox` drop models for all 15 M7 items (56, 57, 64, 70, 86, 96, 97, 100, 101, 102, 104, 111, 115, 162, 196).

## Underground and Caverns (Milestone 6)

- Depth Bands:
  - `DepthBand::Surface`: `y > surface_y - 25`
  - `DepthBand::Underground`: `surface_y - 25 >= y > surface_y - 70`
  - `DepthBand::Cavern`: `y <= surface_y - 70`
- Structures & Loot:
  - Underground cabins: rectangular wood-plank rooms embedded in cave walls in Underground and Cavern layers with chests and furniture.
  - Underground & Cavern chest loot pools: Hermes Boots (54), Cloud in a Bottle (53), Band of Regeneration (49), Magic Mirror (50), Lucky Horseshoe (158), Shiny Red Balloon (159), Suspicious Looking Eye (43), Enchanted Boomerang (55), Spear (274); `max_stack == 1` chest items roll prefixes.
  - Breakable ambient pots dropping coins, torches, arrows, and bombs.
- Accessories Framework:
  - 5 active accessory slots (`SlotRef::Accessory(0..4)`), with prefixes (`62..=80`: Warding, Arcane, Lucky, Menacing, Quick, Violent, etc.) serialized in `Character` (`TVCH` v4) and replicated via `ServerMsg::Accessories`.
  - Hermes Boots (54): increases max run speed (+100%, 2.0x) when running continuously for >= 20 ticks.
  - Cloud in a Bottle (53): grants mid-air double jump.
  - Band of Regeneration (49): grants +1 HP/sec health regeneration.
  - Shiny Red Balloon (159): increases jump height by +30%.
  - Lucky Horseshoe (158): negates fall damage completely.
- Cave Enemies:
  - Cave Bat (NPC 49): 16 HP, 13 damage, 2 defense, erratic flyer AI.
  - Skeleton (NPC 21): 60 HP, 20 damage, 8 defense, walker AI.
  - Giant Worm (NPC 10): burrowing segmented digger AI moving through solid blocks.
  - Depth-gated spawning: cave enemies spawn only in Underground and Cavern layers.
- Weapons & 3D Drop Models:
  - Enchanted Boomerang (55): returning melee projectile curving back to player.
  - Spear (274): piercing directional thrust projectile extending forward and retracting.
  - 3D `.vox` models authored and registered for all M6 items (49, 50, 53, 54, 55, 158, 159, 274).

## Housing, Town NPCs and Magic (Stage 6)

- Housing: 3D enclosure check (flood fill over 6-connectivity air cells bounded by solid blocks/doors/platforms, requiring all interior air cells to have background walls; furniture required: chair, table/workbench, door/platform, light source/torch/furnace/hellforge; min volume 60 cells, max 2250 cells, max span 100). Checked off-tick (rate-limited to 2 s interval).
- Town NPCs: Guide present at world start; Merchant arrives when online players carry >= 5000 copper (50 silver, including cursor stack) and a vacant valid house exists; Arms Dealer (NPC 19) arrives when any player carries a gun or Musket Balls. Arrival and post-death respawns run on daytime, boss-free `SPAWN_CHECK_TICKS` checks. Wander within 12 blocks of home by day, sleep indoors at night.
- Shops: Merchant stocks Torch, Wooden Arrow, Lesser Healing Potion, Anvil, Copper Pickaxe, Copper Axe; Arms Dealer stocks Musket Ball (7c), Flintlock Pistol (5g), Minishark (35g). Shift-click inventory items to sell for 1/5 value.
- Corruption & Bosses: Chasms, ebonstone (>= 65% pickaxe power or explosives), corrupt grass, demonite ore veins, and demon altars. Eye of Cthulhu drops 30..=90 Demonite Ore; Eater of Worlds segments drop Demonite Ore and Shadow Scale, and full defeat sets `downed_eow`. Altars craft Suspicious Looking Eye (6 Lenses) and break with Pwnhammer (>= 80% hammer power).
- Demonite & Shadow gear: Nightmare Pickaxe (65% pickaxe power, mines demonite), War Axe of the Night, Light's Bane, Shadow armor set (defense 6+7+6 = 19).
- Mana & Magic: Base mana 20, max 200 via Mana Crystals (+20 max per crystal). Natural mana regeneration formula active with casting delay. Fallen stars fall at night and despawn at dawn; 5 stars craft a Mana Crystal. Wand of Sparking (found in surface chests, costs 2 mana, shoots spark projectile dealing 14 magic damage).
- HUD & Combat QoL:
  - Mana star column (blue stars representing 20 mana each).
  - Potion sickness buff icon (buff 21) with duration countdown.
  - Ammo counter beside bow weapons showing available arrows from preferred ammo slots.
  - Flaming arrows inflict On Fire debuff (4 hp/s DoT).
  - Eye of Cthulhu spawns Servants of Cthulhu (NPC 5) in phase 1 (cap 8).

## Saves and Backup Recovery

- Root: `TERRAVIBE_SAVES` or the user data dir; one folder per world.
- `world.bin` (magic, version, seed, clock tick), `chunks/<cx>_<cy>_<cz>.bin` (edited chunks
  only), `players/<name>.bin` (inventory, armor, life, position, potion delay), `chests.bin`, `chest_locks.bin` (`TVLK`), `facts.bin` (`TVWF` v3), `towns.bin`, `weather.bin`, `map.bin`.
- Writes go to `*.tmp` then rename. Autosave every 3600 ticks (60 s).
- Backup Rotation: Existing primary save files (`world.bin`, `players/<name>.bin`, `chests.bin`) are rotated to `<file>.bak` before atomic writes replace them (`rotate_backup`).
- Corrupt Recovery: `load_world_with_recovery` detects corrupt primary files and falls back automatically to `.bak` (`recovered_from_bak: true`).
- Backup UI: The Worlds menu detects corrupt worlds and presents an explicit "Load backup" button and recovery badge.
- Version Migration: `migrate_world_version` safely upgrades legacy snapshots to the current format.
- Crash Logs: Panic hooks record crash details to `<saves>/crashes/crash_<timestamp>.log`.
- Disconnect Screen: Network/session drops show an explicit disconnect reason with `[Retry]` and `[Back to Menu]` buttons.
- Not persisted: the boss, arrows.
- Characters live in separate character files in `characters/` (`TVCH` v4: appearance, inventory, armor, accessories, accessory prefixes, vitals, `CharacterFacts`, and `obsidian_skin_ticks`).

## Controls, Rebinding, and Gamepad

- Movement: `WASD` snappy 3D movement (full ground speed in ≤ 6 ticks / 0.1s; stops in ≤ 3 ticks; instant momentum redirection on turns; `block::ICE` retains slide). `Space` jump.
- Camera: third person by default, `V` first person, `F4` fly mode, mouse wheel zooms. Pointer lock reasserts on focus gain and left-click when no panel is open.
- Key Rebinding: All gameplay actions (`MoveForward`, `MoveBack`, `MoveLeft`, `MoveRight`, `Jump`, `Inventory`, `Minimap`, `FullscreenMap`, `Housing`, `BreakWall`, `EquipArmor`, `Throw`) route through `InputBindings` (`Copy`) with key-conflict detection. Persisted in `settings.cfg` and JSON; defaults can be reset from Options.
- Gamepad Support: Full gamepad integration (`GamepadPlugin`). Left stick for movement (deadzone-filtered), right stick for camera look/aim. South (A) jumps, RT attacks/mines, LT interacts/uses items/talks to NPCs, North (Y) toggles inventory, LB/RB cycles hotbar, D-pad / South / East navigates menus. Contextual button prompt glyphs (e.g. `[LT] Talk`, `[A] Jump`) display in HUD prompts.
- Inventory: `Esc` opens/closes the Terraria 58-slot inventory (50 bag slots with hotbar 0–9 on top row, 4 coin slots, 4 ammo slots, 3 armor + 5 accessory slots). Releasing/grabbing cursor is tied to inventory open/close. `Esc` closes inventory, chest, and shop in one press. While dead, `Esc` opens Game Menu directly.
- Game Menu: accessed via the "Settings" button in the bottom-right corner of the open inventory, containing Resume, Options, Controls reference, Save & Quit.
- Slot interaction:
  - `Left click`: empty hand picks stack; with held stack places into empty slot, merges matching items up to max stack, or swaps.
  - `Right click`: picks one item from slot onto held stack.
  - `Shift+left click`: on recipe row crafts max possible count; on bag slot while shop is open sells item to NPC; while chest is open quick-moves stack into chest.
  - Coin slots (50–53) accept only coins; ammo slots (54–57) accept only ammo; armor slots accept only matching armor pieces. Bows consume ammo from ammo slots before bag slots.
- Action suppression: while inventory, chest, or shop is open, left clicks never attack, mine, or place blocks (`use_held` false).

## Options, Accessibility, and Performance (Pass 5)

- Grouped Options UI: Tabbed settings dialog with 4 categories:
  - `Display`: Quality Preset (`Low`, `Medium`, `High`), UI Scale (`0.75..=1.5` scaling HUD and dialogs), HUD Opacity (`0.2..=1.0`).
  - `Audio`: Independent volume buses for Master, Music, Sound (SFX), Ambient, UI; `mute_on_focus_loss` automatically mutes audio when the window loses focus.
  - `Controls`: Interactive key rebinding with real-time conflict warnings and "Reset Defaults" action.
  - `Accessibility`: Colorblind item rarity palette, high-contrast crosshair toggle, reduced motion toggle (disables screen shake and camera jolts), configurable tooltip delay (`0.0..=1.0` s).
- Quality Presets & Performance: `QualityPreset` dynamically scales particle effect limits and weather density; `PerfCounters` track active particles, active critters, and map rebuild times.
- Headless Stress Benchmark: `bench_effects` scenario exercises maximum particle, weather, and critter loads.

## Liquids and world bounds (Milestone 3)

- Liquids: Water, Lava. Stored in chunks as amount (0–255) and kind bit. Tag 2 RLE codec.
- Flow budget: `MAX_CELL_UPDATES_PER_TICK = 4000` cells FIFO per tick. Lava flows at 1/3 speed (`LAVA_PERIOD = 3`).
- Contact: Lava adjacent to Water converts to Obsidian tile (min pickaxe power 40, iron pickaxe or better), clearing the lava cell; drops `OBSIDIAN_ITEM`.
- Buckets: Empty Bucket (3 Iron Bars at Anvil), Water Bucket, Lava Bucket. Pick up requires liquid amount >= 128; pouring places 255.
- Liquid physics: water movement scale 0.50, gravity scale 0.40, holding jump swims up (`SWIM_UP_SPEED = 2.0 * PX_TO_BLOCKS`). Entering water resets fall damage.
- Submerging: head submerged decrements breath (1 per 7 ticks, max 200). At 0 breath, drowning deals 2 damage every 7 ticks. Recovers +3 breath per tick in air.
- Lava damage: contact deals 80 damage (30-tick cooldown) and sets player On Fire (burn damage 1 per 30 ticks for 420 ticks). Submerging in water extinguishes fire.
- World Bounds: finite square worlds centred on origin. Small = 1440 half-extent, Medium = 2195, Large = 2880. Ocean ring width 170 blocks, beach ramp 40 blocks. Sea level Y = 40. Deep lava pool datum Y <= 24.

## Running and flags

- Play: `target/release/terravibe` from the repository root (opens the menu).
- `--world NAME` / `TERRAVIBE_WORLD`: load a world directly. `--save-dir` / `TERRAVIBE_SAVES`.
- `--input-debug`: logs 1 Hz diagnostics (winit backend, grab mode, window focus, motion deltas, cursor movement).
- `--assets-dir`, `TERRAVIBE_ASSETS_DIR`, `--terraria-dir`, `TERRAVIBE_TERRARIA_DIR`: asset
  sources (search order in `README.md`).
- `--bench`: 30 s fly-around benchmark, prints one `BENCH` line, exit 0 on pass.
- `--qa SCENARIO --qa-phase N --qa-out DIR`: scripted QA (use `tools/qa-headless.sh`). Registered scenarios:
  - Menu & audio: `menu_title`, `menu_characters`, `menu_worlds`, `menu_create`, `menu_options`, `menu_multiplayer`, `music`.
  - Core mechanics: `movement`, `combat`, `inventory`, `mining_crafting`, `housing_npc_shop`, `corruption_altar`, `boss`, `liquids`, `save_reload`.
  - Polish passes: `visual_audit`, `first_hour`, `tooltips_economy`, `combat_feedback`, `held_items`, `weather_audio`, `town_housing`.
  - Pass 5 options & stability: `death_respawn` (death cause, modal, coin drop, respawn protocol), `minimap` (compass strip, corner minimap, fullscreen map), `accessibility_options` (UI scale, colorblind palette, high contrast, focus mute), `bench_effects` (particle, weather, and critter stress benchmark).
  - Endgame progression (Milestones 10 & 11): `molten_gear` (Underworld HUD, gear stats, lava immunity), `wall_of_flesh` (Underworld boss encounter and end-of-era victory flow).
- `TERRAVIBE_DEMO=enemies|boss|npc|progression|dig|crack|tunnel|craft`: screenshot demos.
  `TERRAVIBE_SCREENSHOT`, `TERRAVIBE_CAMERA=x,y,z,yaw,pitch` (spectator pose),
  `TERRAVIBE_DEBUG`, `TERRAVIBE_ART` (art dir override).

## Tools

- `tools/install-terraria-assets.sh`: one-time local pack extraction to the ignored
  `assets/terraria-pack/`.
- `tools/bench-headless.sh`, `tools/qa-headless.sh SCENARIO --out DIR BIN`: run in a private
  headless Sway session; never touch the desktop.
- `python3 tools/art/build.py`: rebuild block textures, `.vox` models and rigs into `assets/`.
