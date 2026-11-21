"""Corruption blocks: ebonstone, corrupt grass (top/side/bottom), demonite ore."""
from palette import *
from tex import emit, mirror, overlay

# Ebonstone: purple-black stone, long angular cracks and a few lighter veins.
EB_LEG = {"b": EBONSTONE, "d": EBONSTONE_D, "k": EBONSTONE_DD, "l": EBONSTONE_L}
EBON0 = [
    "bbbbbbbbdbbbbbbb",
    "blbbbbbdbbbbblbb",
    "bbkbbbbdbbbbkbbb",
    "bbbdbbbdbbbbdbbb",
    "bbbbdbbkdbbbdbbb",
    "bbbbdbbbbdbbbbbb",
    "bbbbkdbbbbdbbbbb",
    "bbbbbbbbbbkdbbbb",
    "bllbbbbbbbbdbbbb",
    "bbbkbbbbbbbdbbbb",
    "bbbbdbbbbbbkdbbb",
    "bbbbdbbbbbbbdbbb",
    "bbbbkdbbbbbbdbbb",
    "bbbbbbdbbbbkdbbb",
    "blbbbbdbbbbbbbbb",
    "bbbbbbdbbbbbbbbb",
]
emit("ebonstone", EBON0, EB_LEG)

# Corrupt grass top: purple blades with spiky light tufts and dark shadows.
GL = {"g": CORRUPT, "d": CORRUPT_D, "l": CORRUPT_L}
emit(
    "corrupt_grass_top",
    [
        "gglggggggglggggg",
        "gglgggggggdggggg",
        "ggdgggggggggglgg",
        "ggggggggggggglgg",
        "gggggggggggggdgg",
        "gggglggggggggggg",
        "gggglggglggggggg",
        "ggggdggglggggggg",
        "ggggggggdggglggg",
        "glgggggggggglggg",
        "glggggggggggdggg",
        "gdggglgggggggggg",
        "ggggglgggggggglg",
        "gggggdgggggggglg",
        "ggggggggglggggdg",
        "gggggggggdgggggg",
    ],
    GL,
)

# Corrupt grass side: 3-5 px spiky fringe hanging over corrupt soil.
CD_LEG = {"b": CDIRT, "d": CDIRT_D, "p": CDIRT_P, "l": EBONSTONE_L}
GS = {"G": CORRUPT, "D": CORRUPT_D, "L": CORRUPT_L, **CD_LEG}
emit(
    "corrupt_grass_side",
    [
        "GGGLGGDGGGLGGDGG",
        "GGLGDGLGGGDLGGDG",
        "GGDGGDGGDGGDGGDG",
        "GGLGGDGGGGGLGGGG",
        "GGbGGbbGbGGbGGGb",
        "bbbGbbbbGbbbbGbb",
        "bbbDbbbbDbbbbDbb",
        "bbbbbbbbbbbbbbbb",
        "bbddbbbbbbblbbbb",
        "bbbddbbblbbbbpbb",
        "bbbbbbdbbbbbppbb",
        "bblbbbbbbddbbbbb",
        "bbbbbbbbbdbbbblb",
        "bbddbbbbpbbbbbbb",
        "bbdbbbbbbbblbbbb",
        "bbbbbbddbbbbbbbb",
    ],
    GS,
)

# Corrupt grass bottom: clumped corrupt soil with dark pits and pale grit.
emit(
    "corrupt_grass_bottom",
    [
        "bbbbbbbddbbbbbbb",
        "bbbbbblbbdbdbbbb",
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
    ],
    CD_LEG,
)

# Demonite: ebonstone with four bright violet gem clusters (base, dark, light).
GEMS = [
    "................",
    "................",
    "............h...",
    "...........ooo..",
    "....h.......o...",
    "...ooo..........",
    "....o...........",
    "................",
    "..........h.....",
    ".........ooo....",
    "..........os....",
    "..........o.....",
    "....h...........",
    "...ooo..........",
    "....os..........",
    "................",
]
emit(
    "demonite_ore",
    overlay(EBON0, GEMS),
    {"o": DEMONITE, "s": DEMONITE_D, "h": DEMONITE_L},
    base_legend=EB_LEG,
)
