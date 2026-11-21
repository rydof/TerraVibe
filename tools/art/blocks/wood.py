"""trunk, wood block (planks), wood wall."""
from palette import *
from tex import emit

# Trunk side: vertical bark fissures (dark) with light ridges beside them.
BARK_LEG = {"b": BARK, "d": BARK_D, "l": BARK_L}
emit(
    "trunk_side",
    [
        "bbbdbbblbbbdbbbb",
        "blbdbbblbbbdblbb",
        "blbdbdbbbbbdblbb",
        "bbbdbbbbblbdbdbb",
        "bbbdbbbbblbdbdbb",
        "bbbbbbbbblbdbdbb",
        "blbbbdbbbbbdbbbb",
        "blbbbdbbbbbbdbbb",
        "bbbbbdbbblbbdbbb",
        "bbbbbdbbblbbdbbb",
        "bbbbbdbbbbbbdbbb",
        "bdbbbdbbblbbbbbb",
        "bdbbbdbbblbbbbbb",
        "bbbbbbbbbbbbbdbb",
        "blbbbdbbbbbbbdbb",
        "blbbbdbbbbbbbdbb",
    ],
    BARK_LEG,
)

# Trunk top/bottom: three concentric growth rings around a dark pith.
RINGS = [
    "bbbbbbbbbbbbbbbb",
    "brrrrrrrrrrrrrrb",
    "brbbbbbbbbbbbbbr",
    "brbbbbbbbbbbbbbr",
    "brbrrrrrrrrrbrbr",
    "brbrbbbbbbbrbrbr",
    "brbrbrrrrrbrbrbr",
    "brbrbrddbrbrbrbr",
    "brbrbrddbrbrbrbr",
    "brbrbrrrrrbrbrbr",
    "brbrbbbbbbbrbrbr",
    "brbrrrrrrrrrbrbr",
    "brbbbbbbbbbbbbbr",
    "brbbbbbbbbbbbbbr",
    "brrrrrrrrrrrrrrb",
    "bbbbbbbbbbbbbbbb",
]
RING_LEG = {"b": BARK, "r": RING, "d": RING_D}
emit("trunk_top", RINGS, RING_LEG)
emit("trunk_bottom", RINGS, {"b": BARK_D, "r": RING, "d": RING_D})

# Wood block: 4 px vertical planks, grain flecks and dark nail dots.
WOOD_LEG = {"w": WOOD, "d": WOOD_D, "l": WOOD_L, "g": WOOD_G, "n": BARK_D}
emit(
    "wood_block",
    [
        "wwwgwwwgwwwgwwwg",
        "wlwglwwgwlwglwwg",
        "wwwgwwdgwwwgwwdg",
        "wnwgwwwgwnwgwwwg",
        "wwwgwwwgwwwgwwwg",
        "wdwglwwgwdwglwwg",
        "wwwgwwwgwwwgwwwg",
        "wwwgwnwgwwwgwnwg",
        "wlwgwwwgwlwgwwwg",
        "wwwgwwdgwwwgwwdg",
        "wwwgwwwgwwwgwwwg",
        "wnwgwlwgwnwgwlwg",
        "wwwgwwwgwwwgwwwg",
        "wdwgwwwgwdwgwwwg",
        "wwwgwwdgwwwgwwdg",
        "wwwgwwwgwwwgwwwg",
    ],
    WOOD_LEG,
)

# Wood wall: the same plank grid in the darker wood ramp.
WALL_LEG = {"w": WOOD_D, "d": WOOD_G, "l": WOOD, "g": BARK_D, "n": WOOD_G}
emit(
    "wood_wall",
    [
        "wwwgwwwgwwwgwwwg",
        "wlwglwwgwlwglwwg",
        "wwwgwwdgwwwgwwdg",
        "wwwgwwwgwwwgwwwg",
        "wdwgwlwgwdwgwlwg",
        "wwwgwwwgwwwgwwwg",
        "wnwgwwwgwnwgwwwg",
        "wwwgwwdgwwwgwwdg",
        "wlwglwwgwlwglwwg",
        "wwwgwwwgwwwgwwwg",
        "wwwgwnwgwwwgwnwg",
        "wdwgwwwgwdwgwwwg",
        "wwwgwlwgwwwgwlwg",
        "wwwgwwwgwwwgwwwg",
        "wnwgwwdgwnwgwwdg",
        "wwwgwwwgwwwgwwwg",
    ],
    WALL_LEG,
)
