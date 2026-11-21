"""Dungeon, Shadow Orb, Spikes, and Underworld 16x16 block textures."""
from palette import *
from tex import emit

# Shadow Orb: glowing purple sphere etched into a dark frame.
emit(
    "shadow_orb",
    [
        "dddddddddddddddd",
        "ddddbbbbbbbbdddd",
        "dddbbbbllbbbbddd",
        "ddbbrbllllbrbbdd",
        "dbbblllllllbbbdd",
        "dbblllllllllbbbd",
        "dbbllbblllbbllbd",
        "dbbllbblllbbllbd",
        "dbblllllllllllbd",
        "dbbbllllllllbbbd",
        "ddbbrllllllbrbdd",
        "dddbbbbllbbbbddd",
        "ddddbbbbbbbbdddd",
        "dddddbbbbbbddddd",
        "dddddddddddddddd",
        "dddddddddddddddd",
    ],
    {"d": ORB_D, "b": ORB, "l": ORB_L, "r": DEMONITE_L},
)

# Shared brick mortar layout for dungeon / obsidian / hellstone bricks.
BRICK_ROWS = [
    "dddddddddddddddd",
    "dlllllllldllllll",
    "dlbbbbbbbdlbbbbb",
    "dlbbbbbbbdlbbbbb",
    "dlbbbbbbbdlbbbbb",
    "dddddddddddddddd",
    "llldlllllllldlll",
    "bbbdlbbbbbbbdlbb",
    "bbbdlbbbbbbbdlbb",
    "bbbdlbbbbbbbdlbb",
    "dddddddddddddddd",
    "dlllllllldllllll",
    "dlbbbbbbbdlbbbbb",
    "dlbbbbbbbdlbbbbb",
    "dlbbbbbbbdlbbbbb",
    "dddddddddddddddd",
]

emit("blue_dungeon_brick", BRICK_ROWS, {"d": DB_BLUE_D, "b": DB_BLUE, "l": DB_BLUE_L})
emit("green_dungeon_brick", BRICK_ROWS, {"d": DB_GREEN_D, "b": DB_GREEN, "l": DB_GREEN_L})
emit("pink_dungeon_brick", BRICK_ROWS, {"d": DB_PINK_D, "b": DB_PINK, "l": DB_PINK_L})
emit("obsidian_brick", BRICK_ROWS, {"d": OBS_BRICK_D, "b": OBS_BRICK, "l": OBS_BRICK_L})
emit("hellstone_brick", BRICK_ROWS, {"d": HELL_BRICK_D, "b": HELL_BRICK, "l": HELL_BRICK_L})

# Spikes: metallic jagged spikes on a dark iron base.
emit(
    "spikes",
    [
        "dddddddddddddddd",
        "ddldddddddddlddd",
        "ddlbdddddddldddd",
        "ddbbdddddddlbddd",
        "ddbbdldddddbbddd",
        "ddbbdlbdddblbddd",
        "dbbbdbbdddblbbdd",
        "dbbbdbbdddblbbdd",
        "dbbbbbbdldbbbbdd",
        "dbbbbbbdlbbbbbdd",
        "bbbbbbbdbbbbbbdd",
        "bbbbbbbdbbbbbbbd",
        "bbbbbbbbbbbbbbbd",
        "dddddddddddddddd",
        "bbbbbbbbbbbbbbbb",
        "dddddddddddddddd",
    ],
    {"d": SPIKE_D, "b": SPIKE, "l": SPIKE_L},
)

# Obsidian: dark volcanic glass with sharp diagonal highlights.
emit(
    "obsidian",
    [
        "ddddbbbbbbdddddd",
        "dddbbbllbbbddddd",
        "ddbbbllbbbbbbddd",
        "dbbbllbbbbbbbbdd",
        "dbbbbbbbbddbbbdd",
        "bbbbbbbbddddbbbd",
        "bbbddbbbbddbbbbd",
        "bbddddbbbbbbbbbd",
        "bddddddbbbbllbbd",
        "ddddbbbbbbllbbbd",
        "dddbbbbbbllbbbdd",
        "ddbbbbddllbbbbdd",
        "dbbbbddddbbbbddd",
        "bbbbddddddbbbddd",
        "bbbddddddbbbbddd",
        "dddddddddddddddd",
    ],
    {"d": OBS_D, "b": OBS, "l": OBS_L},
)

# Ash: speckled dark grey volcanic ash.
emit(
    "ash",
    [
        "bbbbdbbbblbbbbdb",
        "bllbbbbddbbbbbbb",
        "bbbbdbbbbbblbbdb",
        "ddbbbbbbbbbbbbbb",
        "bbbbbblbbdbbbbdd",
        "bdbbbbbbbbbllbbb",
        "bbbbddbbbbbbbbdb",
        "bbbbbbbblbbbbbbb",
        "blbbbbbbbbddbbbb",
        "bbbbdbbbbbbbbbbl",
        "bbbbbbblbbdbbbbb",
        "ddbbbbbbbbbbddbb",
        "bbbbdbbbllbbbbbb",
        "bbblbbbbbbbdbbbb",
        "bbbbbbddbbbbbbbl",
        "dbbbbbbbbbdbbbbb",
    ],
    {"d": ASH_D, "b": ASH, "l": ASH_L},
)

# Hellstone: dark underworld rock veined with glowing magma.
emit(
    "hellstone",
    [
        "ddddbbbbddddbbbb",
        "dddbbllbbddbbddd",
        "dbbllbbllbbbdddd",
        "dbllbbbbllbbbbdd",
        "ddbbbbddllbbllbd",
        "ddddddddbbbllbbd",
        "bbddbbbbbbllbbdd",
        "bllbbbbbbllbbddd",
        "bbllbbbbbbbbdddd",
        "bbbllbbbbddbbddd",
        "ddddllbbddddbbbd",
        "dddbllbbbbbbllbd",
        "ddbbllbbbbbllbbd",
        "dbbbllbbbbllbbbd",
        "ddddbbbbdddddddd",
        "dddddddddddddddd",
    ],
    {"d": HELL_D, "b": HELL, "l": HELL_L},
)
