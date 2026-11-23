"""Wave 1 Pre-Hardmode 16x16 block textures: mushroom grass, cloud, sunplate, meteorite, hive."""
from palette import *
from tex import emit

# Glowing Mushroom Grass (top/side/bottom) on Mud
emit(
    "mushroom_grass_top",
    [
        "bbbbdbbbblbbbbbb",
        "bbblbbbbbbbbdbbb",
        "bbbbbbbbbbblbbbb",
        "bllbbdbbbbbbbbbb",
        "bbbbbbbbblbbbbdb",
        "bbbbdbbbbbbbbbbb",
        "bbblbbbbbbdbblbb",
        "bbbbbbbbbbbbbbbb",
        "dbbbbbblbbbbbbdb",
        "bbbbdbbbbbblbbbb",
        "bbbbbbbllbbbbbbb",
        "bbblbbbbbbbbdbbb",
        "bbbbbbdbbbbbbbbb",
        "bllbbbbbbbblbbbb",
        "bbbbdbbbbbbbbbdb",
        "bbbbbbblbbbbbbbb",
    ],
    {"d": MUSH_BLUE_D, "b": MUSH_BLUE, "l": MUSH_BLUE_L},
)

emit(
    "mushroom_grass_side",
    [
        "llllbbbblllbbbbb",
        "bbbdbbbbbbbbdbbb",
        "bbbbbdbbbblbbbbb",
        "mddmbmdmmmddmbmm",
        "mmmmmmmdmmmmmmmm",
        "mmmdmmmmmmmdmmmm",
        "mmmmmdmmmmmmmmmm",
        "dmmmmmmmdmmmmmdm",
        "mmmmmmmmmmmdmmmm",
        "mmdmmmmmmmmmmmmm",
        "mmmmmdmmmmdmmmmm",
        "mmmmmmmmmmmmmdmm",
        "mmmdmmmmmdmmmmmm",
        "mmmmmmmmdmmmmmmd",
        "dmmmmmmmmmmmdmmm",
        "mmmmmdmmmmmmmmmm",
    ],
    {"d": MUSH_BLUE_D, "b": MUSH_BLUE, "l": MUSH_BLUE_L, "m": MUD},
)

emit(
    "mushroom_grass_bottom",
    [
        "mmmmmmmmmdmmmmmm",
        "mmdmmmmmmmmmdmmm",
        "mmmmmdmmmmmmmmmm",
        "mmmmmmmmmdmmmmmm",
        "dmmmmmmmmmmmmdmm",
        "mmmdmmmmmmmmmmmm",
        "mmmmmmmmdmmmmmmm",
        "mmmdmmmmmmmdmmmm",
        "mmmmmmmmmmmmmmmd",
        "mdmmmmmdmmmmmmmm",
        "mmmmmmmmmmmdmmmm",
        "mmmmmdmmmmmmmmmm",
        "dmmmmmmmmdmmmmmm",
        "mmmmmmmmmmmmdmmm",
        "mmdmmmmmmmmmmmmm",
        "mmmmmmmdmmmmmdmm",
    ],
    {"m": MUD, "d": MUD_D},
)

# Cloud: soft pillowy white-blue sky block
emit(
    "cloud",
    [
        "ddbbbbllllbbbbdd",
        "dbbbbllllllbbbbd",
        "bbbbllllllllbbbb",
        "bbllbbbbbbbbllbb",
        "bllbbbbbbbbbblbb",
        "bbbbbbbbbbbbbbbb",
        "bbbbddbbbbddbbbb",
        "dbbbbbbbbbbbbbbd",
        "dbllbbbbbbbbllbd",
        "bllllbbbbbbllllb",
        "bbbbbbbbbbbbbbbb",
        "bbddbbbbbbbbddbb",
        "dbbbddbbbbddbbbd",
        "ddbbbbbbbbbbbbdd",
        "dddbbbbddbbbbddd",
        "dddddddddddddddd",
    ],
    {"d": CLOUD_D, "b": CLOUD, "l": CLOUD_L},
)

# Sunplate: ornate golden-blue sky temple brick
emit(
    "sunplate",
    [
        "dddddddddddddddd",
        "dlllllllllllllld",
        "dlbbbbbbbbbbbbld",
        "dlbbddlllllldbbd",
        "dlbdllbbbbbbldbd",
        "dlbdlbllllbbldbd",
        "dlbdlblbbblbldbd",
        "dlbdlblbbblbldbd",
        "dlbdlbbllllbldbd",
        "dlbdlbbbbbbbldbd",
        "dlbbdlllllldbbbd",
        "dlbbbbbbbbbbbbld",
        "dlllllllllllllld",
        "dbbbddbbbbddbbbd",
        "dbbbbbbbbbbbbbbd",
        "dddddddddddddddd",
    ],
    {"d": SUNPLATE_D, "b": SUNPLATE, "l": SUNPLATE_L},
)

# Meteorite Ore: dark cratered rock with glowing fiery ember veins
emit(
    "meteorite_ore",
    [
        "ddddbbbbddddbddd",
        "ddbbllbbddbbbbdd",
        "dbllbbddbbbbdbbd",
        "ddbbddddbbllbbdd",
        "bbbbddbbllbbbbdd",
        "bbddbbbbddddbbbd",
        "ddddddbbllbbddbd",
        "ddbbddbbbbllbbdd",
        "dbllbbbbddbbbbdd",
        "bbbbddbbbbddbbbd",
        "bbddddbbllbbbbbd",
        "ddbbbbbbllbbdddd",
        "dbllbbddbbbbbbdd",
        "ddbbddddbbllbbdd",
        "ddbbbbbbbbbbbbbd",
        "dddddddddddddddd",
    ],
    {"d": METEOR_D, "b": METEOR, "l": METEOR_L},
)

# Hive: hexagonal amber honeycomb block
emit(
    "hive",
    [
        "ddddbbbbddddbbbb",
        "ddbbllllbbddbbll",
        "dbblbbbbbbdbblbb",
        "dbbbddddbbdbbbdd",
        "ddddbbbbddddbbbb",
        "bbddllllbbbbddll",
        "bbbbbbbbbbbbbbbb",
        "ddbbbbddddbbbbdd",
        "ddddbbbbddddbbbb",
        "ddbbllllbbddbbll",
        "dbblbbbbbbdbblbb",
        "dbbbddddbbdbbbdd",
        "ddddbbbbddddbbbb",
        "bbddllllbbbbddll",
        "bbbbbbbbbbbbbbbb",
        "dddddddddddddddd",
    ],
    {"d": HIVE_D, "b": HIVE, "l": HIVE_L},
)
