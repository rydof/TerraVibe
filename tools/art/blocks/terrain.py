"""dirt, stone, grass, mud, jungle grass, snow, ice.

Stone grids (`STONE0`, `STONE1`, `STONE_LEG`) are module level so the ore and
corruption scripts can reuse the same base instead of forking it.
"""
from palette import *
from tex import emit

# Dirt: clumped dark speckles, sparse light pebbles, one root-like dark streak.
DIRT_LEG = {"b": DIRT, "d": DIRT_D, "l": DIRT_L, "p": DIRT_P}
DIRT0 = [
    "bbbbbbbddbbbbbbb",
    "bbbbbblbdbddbbbb",
    "bbddbbbbbbbdbbbb",
    "bbddbbbbbbbbbbbb",
    "ddbbbbbpbbbbbbld",
    "dbbbbbbbpbbbbbdd",
    "bbbbbddbpbbbbbbb",
    "bbbbbbdpbbbbbbbb",
    "blbbbbbpbbbbbbbb",
    "bbbbbbbbpbbbddbb",
    "bbbbbbbblbbbddbb",
    "bbbbbbbbbbbbbbbb",
    "bbbddbbbbbbbbbbb",
    "bbbdbbbbbbbbblbb",
    "bbbbbbbdbddbbbbb",
    "bbbbbbbddbbbbbbb",
]
DIRT1 = [
    "bbddbbbbbbbbbbbb",
    "bbdbbbbbbbblbbbb",
    "bbbbbbbbbbbbddbb",
    "bbbbbbbbddbbbdbb",
    "bbbbbbbbddbbbbbb",
    "bbbblbbbbbbbbbbb",
    "bbblbbbbbbpbbblb",
    "bbbbbddbbbpbbbbb",
    "bbbbbddbbbbpbbbb",
    "bbddbbbbbbbpbbbb",
    "dddbbbbbbbbbbbbd",
    "dbbbbbblbbbbbbdd",
    "bbbbbbbddbbbbbbb",
    "bbbbbbbddbbbbbbb",
    "blbdbbbbbbbbbbbb",
    "bbddbbbbbbbbbbbb",
]
emit("dirt", DIRT0, DIRT_LEG)
emit("dirt_v1", DIRT1, DIRT_LEG)

# Stone: angular cracks 4-6 px long with light chips on the lump tops.
STONE_LEG = {"b": STONE, "d": STONE_D, "k": STONE_DD, "l": STONE_L}
STONE0 = [
    "bbbbbbbbbdbbbbbb",
    "blbbbbbbbkldbbbb",
    "bbkbbbblbbbbbbbb",
    "bbbdbbdbbbbbdbbb",
    "bbbdbbbbblbbbdbb",
    "bbbbkbbbbbbbbbbb",
    "dbbbdbbbbbbbbbkd",
    "dkblblbbbbbbbbbd",
    "bbbbbbbbdbbblbbb",
    "bblbbbkbbbbblbbb",
    "bbbbbbbddbbbbbbb",
    "bbbbddbbbbbbbdbb",
    "bbddbbbbbbdbbbbb",
    "bdbbbbbbbbbdbblb",
    "bbbbbbbbbbbbbbbb",
    "bbbbbbbbbdbbbbbb",
]
STONE1 = [
    "bbbbbbdbbbbbbbbb",
    "dlbbbbbdblbbbbdd",
    "ddbdbbbbbbbbbbbd",
    "bbbbkbbbbbkbblbb",
    "bbbbdblbbbbdbbbb",
    "bbbbbdbbbbbdbbbb",
    "bbbbbkbbbbbbdbbb",
    "bbblbbbbbbbbbbbb",
    "bbbbbbbblbbbbbbb",
    "bbdbbbbbbbblbbbd",
    "bbbddbbbbbbbbddb",
    "bbbbbdbbbbbbdbbb",
    "bbbbbbdbbbbbbbbb",
    "bbbblbbbbbbbbblb",
    "bbbbbbblbbbbbbbb",
    "bbbbbbdbbbbbbbbb",
]
emit("stone", STONE0, STONE_LEG)
emit("stone_v1", STONE1, STONE_LEG)

# Grass top: blade tufts in base/light with dark shadows under the blades.
GRASS_LEG = {"g": GRASS, "d": GRASS_D, "l": GRASS_L}
GRASS_TOP0 = [
    "gggglgglggggdggg",
    "gggggggggggggggg",
    "ggggggglgggggggg",
    "gglggggglggggggg",
    "ggglggggdggggggg",
    "ggdggggggggglggg",
    "lgggggggggggglgl",
    "ggggggggggggdggg",
    "gggggggggggggggg",
    "gggglggggggggggg",
    "ggggglgggggggggg",
    "ggggdggggglggggg",
    "ggggggggggglgggg",
    "gggggggggggdgglg",
    "ggggggglgggggggg",
    "gggglgglggggdggg",
]
GRASS_TOP1 = [
    "gglggglgggggglgg",
    "gggggggggggggggg",
    "ggglgggggggggggg",
    "gggdgggggggggggg",
    "gggglgggglgggggg",
    "gggggggggdlggggg",
    "glgggggggggggggg",
    "gggggggggggggggg",
    "ggggggggggggglgg",
    "lggggggggggggdll",
    "gggggggggggggggg",
    "ggggglgggggggggg",
    "gggggdlggggggggg",
    "gggggggggglggggg",
    "ggggggggggglgggg",
    "gglggglgggggglgg",
]
emit("grass_top", GRASS_TOP0, GRASS_LEG)
emit("grass_top_v1", GRASS_TOP1, GRASS_LEG)

# Grass side: 3-5 px irregular fringe with hanging blades over dirt.
GRASS_SIDE_LEG = {"G": GRASS, "D": GRASS_D, "L": GRASS_L, **DIRT_LEG}
GRASS_SIDE0 = [
    "GGGDGGLGLGLDGGGG",
    "GGGGLDGLGGGDLGGG",
    "GGLGGLGGGGGLDGGG",
    "GGGDGGDGGDGGDGGG",
    "GGbbGGbbGbGbbGGG",
    "bbbbGbbbbGbbbGbb",
    "bbbbbbbbbGbbbbbb",
    "bbbddbbbbbblbbbb",
    "bbbddbblbbbbpbbb",
    "bbbbbbdbbbbbppbb",
    "bblbbbbbbddbbbbb",
    "bbbbbbbbbdbbbblb",
    "bbddbbbbpbbbbbbb",
    "bbdbbbbbbbblbbbb",
    "bbbbbbddbbbbblbb",
    "bbbbbbbbbbbbbbbb",
]
GRASS_SIDE1 = [
    "GGDGGLGDGGGLDGGG",
    "GGDGGGGDLGGGDGGG",
    "GDGLGGDGGGGDGLGG",
    "GGDGGDGGDGGDGGDG",
    "GbbGGbGbGGbbGGbG",
    "GbGbbbbGbbbGbbbG",
    "bbGbbbbGbbbbbbbb",
    "bbbblbbbbbddbbbb",
    "blbbbbpbbbddbbbb",
    "bbdbbbppbbbbbbbb",
    "bbbbddbbbbbbblbb",
    "bbbbdbbbblbbbbbb",
    "bbbpbbbbbbbbddbb",
    "bbbbbblbbbbbdbbb",
    "blbbbbbbddbbbbbb",
    "bbbbbbbbbbbbbbbb",
]
emit("grass_side", GRASS_SIDE0, GRASS_SIDE_LEG)
emit("grass_side_v1", GRASS_SIDE1, GRASS_SIDE_LEG)
emit("grass_bottom", DIRT0, DIRT_LEG)
emit("grass_bottom_v1", DIRT1, DIRT_LEG)

# Mud: wet soil clumps with a few lighter, drier pebbles.
MUD_LEG = {"m": MUD, "d": MUD_D, "l": MUD_L}
MUD0 = [
    "mmmmmddmmmmmmmmm",
    "mmmmmmdmlmmmmmmm",
    "mmmmmmmmmmmmddmm",
    "mmmddmmmlmmmmdmm",
    "mmmddmmmmmmmmmmm",
    "mmmmmmmmdmmmmmlm",
    "mmmmmmmmmmddmmmm",
    "mmlmmmmmmmddmmmm",
    "ddmmmmmmmmmmmmmd",
    "dmmmmmlmmmmmmmdd",
    "mmddmmmmmmmlmmmm",
    "mmmmmddmmmmmmmmm",
    "mmmmmddmmmmmlmmm",
    "mmmmmmmmmdmmlmmm",
    "mmlmmdmmmmmmmmmm",
    "mmmmmddmmmmmmmmm",
]
emit("mud", MUD0, MUD_LEG)

# Jungle grass: brighter tufts over mud, same fringe rules as grass.
JUNGLE_LEG = {"j": JUNGLE, "d": JUNGLE_D, "l": JUNGLE_L}
JUNGLE_TOP = [
    "jjjljjjdjjljjjjj",
    "jjjjjjjjjjjjjjjj",
    "jjljjjjjljjjjjjj",
    "jjjdjjjjdjjjjjjj",
    "jjjljjjjjjjjjjjj",
    "ljjjjjjjjjjjjjjl",
    "jjjjjdjjjjjjjljj",
    "jjjjddjjjjjjjjjj",
    "jjjjjjjljjdjjjjj",
    "jjjjjjjjjjjjjdjj",
    "jjjjljjjdjjjjjjj",
    "jjdjjjjjjjjjjjjj",
    "jjjjjljjjjljjjjj",
    "jjjdjjjjjjjjjjjj",
    "jjjjjjdjjjjjjljj",
    "jjjljjjdjjljjjjj",
]
emit("jungle_grass_top", JUNGLE_TOP, JUNGLE_LEG)

JUNGLE_SIDE_LEG = {"J": JUNGLE, "K": JUNGLE_D, "Q": JUNGLE_L, "m": MUD, "n": MUD_D, "o": MUD_L}
JUNGLE_SIDE = [
    "JJJKJJQKJJJKQJJJ",
    "JJJKJQJKJJJKJQJJ",
    "JQKJJJKJJJJKQJJJ",
    "JJJKJJKJJKJJKJJJ",
    "JmmJJmJmJJmmJJmJ",
    "JmJmmmmJmmmJmmmJ",
    "mmJmmmmJmmmmmmmm",
    "mmmommmmmnnmmmmm",
    "mmmmmmmmmnnmmmom",
    "mmmmnnmmmmmmommm",
    "mommmmmmmmmmnmmm",
    "mmmmmmnnmmmmmomm",
    "mmmmmmmmmmmmmmmm",
    "mmmmmmmmmmmmmmmm",
    "mmmmmmmmmmmmmmmm",
    "mmmmmmmmmmmmmmmm",
]
emit("jungle_grass_side", JUNGLE_SIDE, JUNGLE_SIDE_LEG)
emit("jungle_grass_bottom", MUD0, MUD_LEG)

# Snow: soft blue-shadow dimples and a few bright sparkles.
SNOW_LEG = {"w": SNOW, "d": SNOW_D, "l": SNOW_L}
SNOW0 = [
    "wwwwwwwwddwwwwww",
    "wwwwlwwwwdwwwwww",
    "wwwwwwwwwwddwwww",
    "wwlwwwwwwwddwwww",
    "wwwddwwwwwwwlwww",
    "wwwddwwwlwwwwwww",
    "ddwddwwwwwwlwwwd",
    "dwwwwwlwwwwwwwdd",
    "wwwwwwwwwlwwwddw",
    "wwwwlwwwwwwwwddw",
    "wlwwwwddwwwwwwww",
    "wwwwwwddwwwwlwww",
    "wwwlwwddwwwwwwww",
    "wwwwwwwwwdwwwwlw",
    "wwwwwlwwdwwwwwww",
    "wwwwwwwwddwwwwww",
]
emit("snow", SNOW0, SNOW_LEG)

# Ice: two diagonal highlight streaks over darker fracture lines.
ICE_LEG = {"i": ICE, "d": ICE_D, "l": ICE_L}
ICE0 = [
    "iiiiidiiiiiiiiii",
    "iiiiiidiiiiiiiii",
    "iiiiiiiiiiiliiii",
    "iiliiiiiiiiidiii",
    "iiiliiiiiiiiidii",
    "iiiiliiiiiiiidii",
    "iiiiiliiiiiiiiii",
    "idiiiiliiiiiiiii",
    "diiiiiiliiiiiidd",
    "ddiiliiiiiiiiiid",
    "iiiiiiiiiliiiiii",
    "iiiidiiiiiliiiii",
    "iiiiidiiiiiliiii",
    "iiiiidiiiiiiliii",
    "iiiiidiiiiiiilii",
    "iiiiidiiiiiiiiii",
]
emit("ice", ICE0, ICE_LEG)
