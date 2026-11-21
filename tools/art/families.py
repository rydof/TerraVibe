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
]
FAMILIES["enemies"]["rigs"] = [
    "models/slime/rig.ron",
    "models/zombie/rig.ron",
    "models/demon_eye/rig.ron",
    "models/eater_of_souls/rig.ron",
]
# The two 6-voxel eye patches are detached by design (seen through the gel).
FAMILIES["enemies"]["detached"] = {"models/slime/eyes.vox": 12}

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
]
FAMILIES["boss"]["rigs"] = ["models/eye_of_cthulhu/rig.ron"]

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
)
FAMILIES["characters"]["rigs"] = [
    "models/player/rig.ron",
    "models/guide/rig.ron",
    "models/merchant/rig.ron",
    "models/arms_dealer/rig.ron",
]
