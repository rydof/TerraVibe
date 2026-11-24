"""Single source of truth for the art checker: per family, which assets exist and their budgets.

Paths are relative to the assets directory. Later tasks only append to the lists:
  textures  - tile PNGs (TILE x TILE RGBA, palette colours only)
  models    - .vox files checked for size, budget, dilation and connectivity
  rigs      - rig.ron files; every `file: "x.vox"` must sit beside the rig
  detached  - model path -> voxels allowed in floating islands (declared eyes/particles)
"""
from palette import TILE

FAMILY_NAMES = ["terrain", "ores", "foliage", "enemies", "boss", "characters", "items", "furniture"]

MAX_VOXELS = {
    "terrain": 6000,
    "ores": 6000,
    # Measured 55,536-58,134 voxels for the 64x48x64 canopies; ~17% headroom.
    "foliage": 68000,
    "enemies": 20000,
    "boss": 90000,
    "characters": 14000,
    "items": 3000,
    "furniture": 12000,
}

TEXTURE_SIZE = (TILE, TILE)
MAX_AXIS = 255
MAX_UPSCALE_RATIO = 0.85
MAX_FREE_ISLAND = 3  # a detached island this small (or smaller) is tolerated everywhere

TERRAIN_TEXTURES = [
    "blocks/dirt.png",
    "blocks/dirt_v1.png",
    "blocks/stone.png",
    "blocks/stone_v1.png",
    "blocks/grass_top.png",
    "blocks/grass_top_v1.png",
    "blocks/grass_side.png",
    "blocks/grass_side_v1.png",
    "blocks/grass_bottom.png",
    "blocks/grass_bottom_v1.png",
    "blocks/mud.png",
    "blocks/jungle_grass_top.png",
    "blocks/jungle_grass_side.png",
    "blocks/jungle_grass_bottom.png",
    "blocks/snow.png",
    "blocks/ice.png",
    "blocks/sand.png",
    "blocks/sandstone.png",
    "blocks/hardened_sand.png",
]

ORES_TEXTURES = [
    "blocks/copper_ore.png",
    "blocks/iron_ore.png",
    "blocks/silver_ore.png",
    "blocks/gold_ore.png",
    "blocks/demonite_ore.png",
    "blocks/ebonstone.png",
    "blocks/corrupt_grass_top.png",
    "blocks/corrupt_grass_side.png",
    "blocks/corrupt_grass_bottom.png",
    "blocks/wood_block.png",
    "blocks/wood_wall.png",
    "blocks/trunk_top.png",
    "blocks/trunk_side.png",
    "blocks/trunk_bottom.png",
    "blocks/glass.png",
    "blocks/shadow_orb.png",
    "blocks/blue_dungeon_brick.png",
    "blocks/green_dungeon_brick.png",
    "blocks/pink_dungeon_brick.png",
    "blocks/spikes.png",
    "blocks/obsidian.png",
    "blocks/ash.png",
    "blocks/hellstone.png",
    "blocks/obsidian_brick.png",
    "blocks/hellstone_brick.png",
    "blocks/mushroom_grass_top.png",
    "blocks/mushroom_grass_side.png",
    "blocks/mushroom_grass_bottom.png",
    "blocks/cloud.png",
    "blocks/sunplate.png",
    "blocks/meteorite_ore.png",
    "blocks/hive.png",
]

FAMILIES = {
    name: {"textures": [], "models": [], "rigs": [], "detached": {}, "max_voxels": MAX_VOXELS[name]}
    for name in FAMILY_NAMES
}
FAMILIES["terrain"]["textures"] = TERRAIN_TEXTURES
FAMILIES["ores"]["textures"] = ORES_TEXTURES

# Trees, the seven Task 6 models: canopies plus the torch, crystal, bottle and
# herb mushroom. Task 10 may reclassify the items/furniture later.
FAMILIES["foliage"]["models"] = [
    "models/blocks/canopy_0.vox",
    "models/blocks/canopy_1.vox",
    "models/blocks/canopy_2.vox",
    "models/blocks/torch.vox",
    "models/blocks/life_crystal.vox",
    "models/blocks/bottle.vox",
    "models/blocks/mushroom.vox",
    "models/blocks/pot.vox",
    "models/blocks/stalactite.vox",
    "models/blocks/moss.vox",
    "models/blocks/small_rock.vox",
    "models/blocks/shelf_mushroom.vox",
    "models/blocks/glowing_mushroom.vox",
    "models/blocks/campfire.vox",
    "models/blocks/heart_lantern.vox",
]

# The four hostile enemy rigs, authored at 1/16 voxels. The guide and merchant
# are Task 9's and are intentionally not listed here yet.
FAMILIES["enemies"]["models"] = [
    "models/slime/body.vox",
    "models/slime/eyes.vox",
    "models/slime/nucleus.vox",
    "models/zombie/torso.vox",
    "models/zombie/head.vox",
    "models/zombie/arm_l.vox",
    "models/zombie/arm_r.vox",
    "models/zombie/leg_l.vox",
    "models/zombie/leg_r.vox",
    "models/demon_eye/ball.vox",
    "models/demon_eye/tail1.vox",
    "models/demon_eye/tail2.vox",
    "models/eater_of_souls/head.vox",
    "models/eater_of_souls/wing_l.vox",
    "models/eater_of_souls/wing_r.vox",
    "models/bunny/body.vox",
    "models/bunny/head.vox",
    "models/bunny/leg_fl.vox",
    "models/bunny/leg_fr.vox",
    "models/bunny/leg_bl.vox",
    "models/bunny/leg_br.vox",
    "models/bird/body.vox",
    "models/bird/wing_l.vox",
    "models/bird/wing_r.vox",
    "models/squirrel/body.vox",
    "models/squirrel/tail.vox",
    "models/cave_critter/body.vox",
    "models/cave_critter/legs_l.vox",
    "models/cave_critter/legs_r.vox",
    "models/skeleton/head.vox",
    "models/skeleton/torso.vox",
    "models/skeleton/arm_l.vox",
    "models/skeleton/arm_r.vox",
    "models/skeleton/leg_l.vox",
    "models/skeleton/leg_r.vox",
    "models/angry_bones/head.vox",
    "models/angry_bones/torso.vox",
    "models/angry_bones/arm_l.vox",
    "models/angry_bones/arm_r.vox",
    "models/angry_bones/leg_l.vox",
    "models/angry_bones/leg_r.vox",
    "models/dark_caster/head.vox",
    "models/dark_caster/torso.vox",
    "models/dark_caster/arm_l.vox",
    "models/dark_caster/arm_r.vox",
    "models/dark_caster/leg_l.vox",
    "models/dark_caster/leg_r.vox",
    "models/cursed_skull/head.vox",
    "models/cursed_skull/jaw.vox",
    "models/cave_bat/body.vox",
    "models/cave_bat/wing_l.vox",
    "models/cave_bat/wing_r.vox",
    "models/hellbat/body.vox",
    "models/hellbat/wing_l.vox",
    "models/hellbat/wing_r.vox",
    "models/giant_worm/head.vox",
    "models/giant_worm/body.vox",
    "models/giant_worm/tail.vox",
    "models/bone_serpent_head/skull.vox",
    "models/bone_serpent_head/jaw.vox",
    "models/bone_serpent_body/segment.vox",
    "models/bone_serpent_tail/tail.vox",
    "models/lava_slime/body.vox",
    "models/lava_slime/eyes.vox",
    "models/lava_slime/core.vox",
    "models/fire_imp/body.vox",
    "models/fire_imp/arm_l.vox",
    "models/fire_imp/arm_r.vox",
    "models/fire_imp/tail.vox",
    "models/demon/body.vox",
    "models/demon/wing_l.vox",
    "models/demon/wing_r.vox",
    "models/voodoo_demon/body.vox",
    "models/voodoo_demon/wing_l.vox",
    "models/voodoo_demon/wing_r.vox",
    "models/voodoo_demon/voodoo_doll.vox",
    # Task 5: Events & Biome Enemies
    "models/servant_of_cthulhu/ball.vox",
    "models/servant_of_cthulhu/tail1.vox",
    "models/servant_of_cthulhu/tail2.vox",
    "models/bee/body.vox",
    "models/bee/wing_l.vox",
    "models/bee/wing_r.vox",
    "models/blood_zombie/torso.vox",
    "models/blood_zombie/head.vox",
    "models/blood_zombie/arm_l.vox",
    "models/blood_zombie/arm_r.vox",
    "models/blood_zombie/leg_l.vox",
    "models/blood_zombie/leg_r.vox",
    "models/drippler/body.vox",
    "models/drippler/tendril.vox",
    "models/goblin_peon/torso.vox",
    "models/goblin_peon/head.vox",
    "models/goblin_peon/arm_l.vox",
    "models/goblin_peon/arm_r.vox",
    "models/goblin_peon/leg_l.vox",
    "models/goblin_peon/leg_r.vox",
    "models/goblin_thief/torso.vox",
    "models/goblin_thief/head.vox",
    "models/goblin_thief/arm_l.vox",
    "models/goblin_thief/arm_r.vox",
    "models/goblin_thief/leg_l.vox",
    "models/goblin_thief/leg_r.vox",
    "models/goblin_warrior/torso.vox",
    "models/goblin_warrior/head.vox",
    "models/goblin_warrior/arm_l.vox",
    "models/goblin_warrior/arm_r.vox",
    "models/goblin_warrior/leg_l.vox",
    "models/goblin_warrior/leg_r.vox",
    "models/goblin_sorcerer/torso.vox",
    "models/goblin_sorcerer/head.vox",
    "models/goblin_sorcerer/arm_l.vox",
    "models/goblin_sorcerer/arm_r.vox",
    "models/goblin_sorcerer/leg_l.vox",
    "models/goblin_sorcerer/leg_r.vox",
    "models/goblin_archer/torso.vox",
    "models/goblin_archer/head.vox",
    "models/goblin_archer/arm_l.vox",
    "models/goblin_archer/arm_r.vox",
    "models/goblin_archer/leg_l.vox",
    "models/goblin_archer/leg_r.vox",
    "models/antlion/body.vox",
    "models/antlion/mandible_l.vox",
    "models/antlion/mandible_r.vox",
    "models/vulture/body.vox",
    "models/vulture/wing_l.vox",
    "models/vulture/wing_r.vox",
    "models/ice_slime/body.vox",
    "models/ice_slime/eyes.vox",
    "models/ice_slime/shards.vox",
    "models/snow_flinx/body.vox",
    "models/snow_flinx/ear_l.vox",
    "models/snow_flinx/ear_r.vox",
    "models/snow_flinx/leg_l.vox",
    "models/snow_flinx/leg_r.vox",
    "models/jungle_slime/body.vox",
    "models/jungle_slime/eyes.vox",
    "models/jungle_slime/spores.vox",
    "models/hornet/body.vox",
    "models/hornet/wing_l.vox",
    "models/hornet/wing_r.vox",
    "models/man_eater/stem.vox",
    "models/man_eater/head.vox",
    "models/man_eater/jaw.vox",
    "models/harpy/torso.vox",
    "models/harpy/head.vox",
    "models/harpy/wing_l.vox",
    "models/harpy/wing_r.vox",
    "models/harpy/leg_l.vox",
    "models/harpy/leg_r.vox",
    "models/shark/body.vox",
    "models/shark/tail.vox",
    "models/shark/jaw.vox",
    "models/crab/body.vox",
    "models/crab/claw_l.vox",
    "models/crab/claw_r.vox",
    "models/crab/leg_l.vox",
    "models/crab/leg_r.vox",
    "models/piranha/body.vox",
    "models/piranha/tail.vox",
    "models/piranha/jaw.vox",
]
FAMILIES["enemies"]["rigs"] = [
    "models/slime/rig.ron",
    "models/zombie/rig.ron",
    "models/demon_eye/rig.ron",
    "models/eater_of_souls/rig.ron",
    "models/bunny/rig.ron",
    "models/bird/rig.ron",
    "models/squirrel/rig.ron",
    "models/cave_critter/rig.ron",
    "models/skeleton/rig.ron",
    "models/angry_bones/rig.ron",
    "models/dark_caster/rig.ron",
    "models/cursed_skull/rig.ron",
    "models/cave_bat/rig.ron",
    "models/hellbat/rig.ron",
    "models/giant_worm/rig.ron",
    "models/bone_serpent_head/rig.ron",
    "models/bone_serpent_body/rig.ron",
    "models/bone_serpent_tail/rig.ron",
    "models/lava_slime/rig.ron",
    "models/fire_imp/rig.ron",
    "models/demon/rig.ron",
    "models/voodoo_demon/rig.ron",
    "models/servant_of_cthulhu/rig.ron",
    "models/bee/rig.ron",
    "models/blood_zombie/rig.ron",
    "models/drippler/rig.ron",
    "models/goblin_peon/rig.ron",
    "models/goblin_thief/rig.ron",
    "models/goblin_warrior/rig.ron",
    "models/goblin_sorcerer/rig.ron",
    "models/goblin_archer/rig.ron",
    "models/antlion/rig.ron",
    "models/vulture/rig.ron",
    "models/ice_slime/rig.ron",
    "models/snow_flinx/rig.ron",
    "models/jungle_slime/rig.ron",
    "models/hornet/rig.ron",
    "models/man_eater/rig.ron",
    "models/harpy/rig.ron",
    "models/shark/rig.ron",
    "models/crab/rig.ron",
    "models/piranha/rig.ron",
]
# The two 6-voxel eye patches are detached by design (seen through the gel).
FAMILIES["enemies"]["detached"] = {
    "models/slime/eyes.vox": 12,
    "models/lava_slime/eyes.vox": 12,
    "models/ice_slime/eyes.vox": 12,
    "models/jungle_slime/eyes.vox": 12,
}

# Held items at 1/16 voxels: one file per item id, named by `crates/sim/src/item/mod.rs`.
ITEM_FILES = [
    "1.vox",  # iron pickaxe
    "4.vox",  # iron broadsword
    "8.vox",  # torch
    "9.vox",  # wood
    "10.vox",  # iron axe
    "23.vox",  # gel
    "24.vox",  # wooden sword
    "39.vox",  # wooden bow
    "40.vox",  # wooden arrow
    "3506.vox",  # copper axe
    "3507.vox",  # copper short sword
    "3508.vox",  # copper broadsword
    "3509.vox",  # copper pickaxe
    "3512.vox",  # silver axe
    "3514.vox",  # silver broadsword
    "3515.vox",  # silver pickaxe
    "3518.vox",  # gold axe
    "3520.vox",  # gold broadsword
    "3521.vox",  # gold pickaxe
    "45.vox",  # war axe of the night
    "46.vox",  # light's bane
    "103.vox",  # nightmare pickaxe
    "49.vox",  # band of regeneration
    "50.vox",  # magic mirror
    "53.vox",  # cloud in a bottle
    "54.vox",  # hermes boots
    "55.vox",  # enchanted boomerang
    "158.vox",  # lucky horseshoe
    "159.vox",  # shiny red balloon
    "274.vox",  # spear
    "56.vox",  # demonite ore
    "57.vox",  # demonite bar
    "64.vox",  # vilethorn
    "70.vox",  # worm food
    "86.vox",  # shadow scale
    "95.vox",  # flintlock pistol
    "96.vox",  # musket
    "97.vox",  # musket ball
    "98.vox",  # minishark
    "100.vox",  # shadow greaves
    "101.vox",  # shadow scalemail
    "102.vox",  # shadow helmet
    "104.vox",  # the breaker
    "111.vox",  # band of starpower
    "115.vox",  # shadow orb
    "162.vox",  # ball o' hurt
    "196.vox",  # wooden hammer
    # M8 Dungeon items
    "113.vox",  # magic missile
    "154.vox",  # bone
    "155.vox",  # muramasa
    "156.vox",  # cobalt shield
    "157.vox",  # aqua scepter
    "163.vox",  # blue moon
    "164.vox",  # handgun
    "327.vox",  # golden key
    "329.vox",  # shadow key
    "3317.vox",  # valor
    # M9 Underworld items
    "172.vox",  # ash
    "173.vox",  # obsidian
    "174.vox",  # hellstone
    "192.vox",  # obsidian brick
    "193.vox",  # obsidian skull
    "214.vox",  # hellstone brick
    "221.vox",  # hellforge
    "267.vox",  # guide voodoo doll
    "288.vox",  # obsidian skin potion
    "906.vox",  # lava charm
    # M10 Molten gear
    "119.vox",  # flamarang
    "120.vox",  # molten fury
    "121.vox",  # fiery greatsword
    "122.vox",  # molten pickaxe
    "175.vox",  # hellstone bar
    "217.vox",  # molten hamaxe
    "219.vox",  # phoenix blaster
    "231.vox",  # molten helmet (drop)
    "232.vox",  # molten breastplate (drop)
    "233.vox",  # molten greaves (drop)
    # M11 Wall of Flesh drops
    "367.vox",   # pwnhammer
    "426.vox",   # breaker blade
    "434.vox",   # clockwork assault rifle
    "489.vox",   # sorcerer emblem
    "490.vox",   # warrior emblem
    "491.vox",   # ranger emblem
    "514.vox",   # laser rifle
    "4912.vox",  # firecracker
    # Critter catching & caught critters
    "1991.vox",  # bug net
    "2019.vox",  # bunny
    "2015.vox",  # bird
    "2018.vox",  # squirrel
    "2007.vox",  # cave critter
    # Wave 1 Pre-Hardmode items
    "65.vox",    # starfury
    "84.vox",    # grappling hook
    "166.vox",   # bomb
    "167.vox",   # dynamite
    "168.vox",   # grenade
    # Pass P2 iconic bespoke items
    "71.vox",    # copper coin
    "72.vox",    # silver coin
    "73.vox",    # gold coin
    "74.vox",    # platinum coin
    "188.vox",   # healing potion
    "189.vox",   # mana potion
    "273.vox",   # night's edge
    "127.vox",   # space gun
    "165.vox",   # water bolt
    "560.vox",   # slime crown
    "1133.vox",  # abeemination
    "361.vox",   # goblin battle standard
]
FAMILIES["items"]["models"] = [f"models/items/{f}" for f in ITEM_FILES]

# Furniture at 1/16 voxels: the one-block cells of `block_models.rs` and the
# demon altar's 3x2x1 group.
FURNITURE_FILES = [
    "workbench.vox",
    "anvil.vox",
    "furnace.vox",
    "chest.vox",
    "door_closed.vox",
    "door_open.vox",
    "table.vox",
    "chair.vox",
    "platform.vox",
    "demon_altar.vox",
    "hellforge.vox",
    "tinkerers_workshop.vox",
]
FAMILIES["furniture"]["models"] = [f"models/blocks/{f}" for f in FURNITURE_FILES]

# The Eye of Cthulhu at 1/16 voxels: a 4-block ball, the phase 1 iris, the
# phase 2 mouth and four optic-nerve tendrils, all on one rig.
FAMILIES["boss"]["models"] = [
    "models/eye_of_cthulhu/ball.vox",
    "models/eye_of_cthulhu/iris.vox",
    "models/eye_of_cthulhu/mouth.vox",
    "models/eye_of_cthulhu/tendril1.vox",
    "models/eye_of_cthulhu/tendril2.vox",
    "models/eye_of_cthulhu/tendril3.vox",
    "models/eye_of_cthulhu/tendril4.vox",
    "models/king_slime/body.vox",
    "models/king_slime/ninja.vox",
    "models/king_slime/crown.vox",
    "models/queen_bee/body.vox",
    "models/queen_bee/wing_l.vox",
    "models/queen_bee/wing_r.vox",
    "models/eater_of_worlds_head/head.vox",
    "models/eater_of_worlds_head/mandible_l.vox",
    "models/eater_of_worlds_head/mandible_r.vox",
    "models/eater_of_worlds_body/body.vox",
    "models/eater_of_worlds_tail/tail.vox",
    "models/skeletron_head/cranium.vox",
    "models/skeletron_head/jaw.vox",
    "models/skeletron_hand/hand.vox",
    "models/dungeon_guardian/cranium.vox",
    "models/dungeon_guardian/jaw.vox",
    "models/wall_of_flesh_mouth/wall_slab.vox",
    "models/wall_of_flesh_mouth/maw_upper.vox",
    "models/wall_of_flesh_mouth/maw_lower.vox",
    "models/wall_of_flesh_mouth/tendril.vox",
    "models/wall_of_flesh_eye/eye.vox",
    "models/wall_of_flesh_eye/stalk.vox",
    "models/the_hungry/head.vox",
    "models/the_hungry/jaw.vox",
    "models/leech/head.vox",
    "models/leech/tail.vox",
]
FAMILIES["boss"]["rigs"] = [
    "models/eye_of_cthulhu/rig.ron",
    "models/king_slime/rig.ron",
    "models/queen_bee/rig.ron",
    "models/eater_of_worlds_head/rig.ron",
    "models/eater_of_worlds_body/rig.ron",
    "models/eater_of_worlds_tail/rig.ron",
    "models/skeletron_head/rig.ron",
    "models/skeletron_hand/rig.ron",
    "models/dungeon_guardian/rig.ron",
    "models/wall_of_flesh_mouth/rig.ron",
    "models/wall_of_flesh_eye/rig.ron",
    "models/the_hungry/rig.ron",
    "models/leech/rig.ron",
]

# The player at 1/16 voxels, the six armor tiers, and the two town NPCs. The
# guide and merchant reuse the player part layout; the merchant's head model is
# taller to fit the hat.
PLAYER_PARTS = ("head", "torso", "arm_l", "arm_r", "leg_l", "leg_r")
ARMOR_PARTS = ("head", "body", "arm_l", "arm_r", "leg_l", "leg_r")
ARMOR_TIERS = ("copper", "iron", "silver", "gold", "shadow", "molten")
FAMILIES["characters"]["models"] = (
    [f"models/player/{p}.vox" for p in PLAYER_PARTS]
    + [f"models/armor/{t}_{p}.vox" for t in ARMOR_TIERS for p in ARMOR_PARTS]
    + [f"models/guide/{p}.vox" for p in PLAYER_PARTS]
    + [f"models/merchant/{p}.vox" for p in PLAYER_PARTS]
    + [f"models/arms_dealer/{p}.vox" for p in PLAYER_PARTS]
    + [f"models/nurse/{p}.vox" for p in PLAYER_PARTS]
    + [f"models/demolitionist/{p}.vox" for p in PLAYER_PARTS]
    + [f"models/dryad/{p}.vox" for p in PLAYER_PARTS]
    + [f"models/goblin_tinkerer/{p}.vox" for p in PLAYER_PARTS]
    + [f"models/old_man/{p}.vox" for p in PLAYER_PARTS]
)
FAMILIES["characters"]["rigs"] = [
    "models/player/rig.ron",
    "models/guide/rig.ron",
    "models/merchant/rig.ron",
    "models/arms_dealer/rig.ron",
    "models/nurse/rig.ron",
    "models/demolitionist/rig.ron",
    "models/dryad/rig.ron",
    "models/goblin_tinkerer/rig.ron",
    "models/old_man/rig.ron",
]
