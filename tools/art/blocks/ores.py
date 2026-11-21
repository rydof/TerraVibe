"""Ores: Task 4 stone base plus 3-4 irregular ore clusters.

The stone grids come from terrain.py so ore edges tile seamlessly with plain
stone. Each cluster is 4-7 px: a base/dark body with one light highlight, and
no cluster touches a tile edge. The patch uses its own legend keys (`o`, `s`,
`h`) so it can never recolour the stone base.
"""
from palette import *
from terrain import STONE0, STONE1, STONE_LEG
from tex import emit, mirror, overlay

# Copper: four chunky clusters in the top-left, right, bottom-left and bottom-right.
COPPER_PATCH = [
    "................",
    "................",
    ".....h..........",
    "....oo..........",
    "...oso..........",
    "................",
    "................",
    ".........hoo....",
    ".........sos....",
    "................",
    "....h...........",
    "...oo...........",
    "...oso.....oosh.",
    "............os..",
    "................",
    "................",
]

# Iron: four wedge clusters hugging the corners, all clear of the edges.
IRON_PATCH = [
    "................",
    "................",
    "...h............",
    "..oos......h....",
    "..oo......oos...",
    "...........os...",
    "................",
    "................",
    "................",
    "....h...........",
    "...oos..........",
    "....os..........",
    "...........h....",
    "..........oos...",
    "...........os...",
    "................",
]

for name, base, patch, body, dark, hi in [
    ("copper_ore", STONE0, COPPER_PATCH, COPPER, COPPER_D, COPPER_L),
    ("iron_ore", STONE1, IRON_PATCH, IRON, IRON_D, IRON_L),
    ("silver_ore", STONE0, mirror(COPPER_PATCH), SILVER, SILVER_D, SILVER_L),
    ("gold_ore", STONE1, mirror(IRON_PATCH), GOLD, GOLD_D, GOLD_L),
]:
    emit(
        name,
        overlay(base, patch),
        {"o": body, "s": dark, "h": hi},
        base_legend=STONE_LEG,
    )
