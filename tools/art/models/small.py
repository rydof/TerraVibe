"""Torch, life crystal, bottle and the surface herb mushroom.

All models use 16 voxels per block (1/16 voxel scale) with a bottom-centre pivot.
The torch, bottle and crystal fill one block; the mushroom is a small herb.
"""
import os

from palette import *
from vox import Model

OUT = os.path.join(ASSETS, "models", "blocks")
STICK = hexc("#8a5a2c")
STICK_D = hexc("#5e3b1c")
FLAME_OUT = hexc("#ff7d1a")
FLAME_MID = hexc("#ffb82e")
FLAME_CORE = hexc("#ffe56a")


def save(m, name):
    m.save(os.path.join(OUT, name + ".vox"))


# Torch: a 3x3 wooden handle, 16 voxels tall, topped with a 5-voxel flame in
# three colours (outer orange, mid amber, bright core).
m = Model(5, 16, 5)
m.box(1, 0, 1, 3, 9, 3, STICK)
m.box(1, 0, 1, 1, 9, 3, STICK_D)  # shaded left face
m.box(1, 10, 1, 3, 10, 3, STICK_D)  # charred rim under the flame
m.box(1, 11, 1, 3, 11, 3, FLAME_MID)
m.box(1, 12, 1, 3, 13, 3, FLAME_OUT)
m.box(2, 12, 2, 2, 14, 2, FLAME_CORE)
m.set(2, 15, 2, FLAME_CORE)
save(m, "torch")

# Life crystal: a pink-red faceted gem on a rocky base, four red shades.
RED_D = hexc("#a3122a")
RED = hexc("#e02848")
RED_L = hexc("#ff6a84")
RED_W = hexc("#ffc2cf")
m = Model(16, 16, 16)
m.box(3, 0, 3, 12, 1, 12, STONE_D)  # rocky base
rows = {  # y: (x0, x1) of the heart outline, two lobes at the top
    2: (7, 8), 3: (6, 9), 4: (5, 10), 5: (4, 11), 6: (4, 11),
    7: (4, 11), 8: (5, 10), 9: (5, 10), 10: (6, 9), 11: (7, 8),
}
for y, (x0, x1) in rows.items():
    z0, z1 = (6, 9) if y in (2, 11) else (5, 10) if y in (3, 10) else (4, 11)
    m.box(x0, y, z0, x1, y, z1, RED)
    m.box(x0, y, z0, x0, y, z1, RED_D)  # left facet
    m.box(x1, y, z0, x1, y, z1, RED_L)  # right facet
m.box(6, 11, 4, 7, 11, 4, RED_D)  # dip between the lobes
m.box(5, 5, 11, 5, 8, 11, RED_L)
m.set(5, 7, 11, RED_W)
m.set(6, 6, 11, RED_W)
save(m, "life_crystal")

# Bottle: clear glass body, narrow neck, cork. Three glass shades plus a glint.
GL = hexc("#bfe8f2")
GL_M = hexc("#8fc8d8")
GL_D = hexc("#5f9fb5")
CORK = hexc("#b98a5a")
m = Model(16, 16, 16)
m.cyl(7.5, 7.5, 4.0, 0, 9, GL)  # rounded body
m.cyl(7.5, 7.5, 2.6, 1, 8, GL_M)  # dim inside of an empty bottle
m.box(4, 0, 4, 4, 9, 11, GL_D)  # shaded left face
m.box(4, 0, 4, 11, 9, 4, GL_D)  # shaded back face
m.box(6, 10, 6, 9, 13, 9, GL)  # neck
m.box(6, 10, 6, 6, 13, 9, GL_D)
m.box(6, 14, 6, 9, 15, 9, CORK)  # cork
m.box(8, 3, 11, 9, 7, 11, hexc("#ffffff"))  # glint
save(m, "bottle")

# Surface herb mushroom: a thin pale stem under a red cap with white specks,
# 6 voxels tall and flush on the grass (no pedestal).
CAP = hexc("#c0392b")
CAP_L = hexc("#e74c3c")
CAP_D = hexc("#8e2419")
STEM = hexc("#e6d7b0")
STEM_D = hexc("#c2ae7d")
SPECK = hexc("#fff8f0")
m = Model(7, 6, 7)
m.box(3, 0, 3, 3, 2, 3, STEM)  # 1x1 stem, bottom flush at y = 0
m.box(3, 0, 3, 3, 2, 3, STEM_D)
m.set(3, 2, 3, STEM)
m.box(1, 3, 1, 5, 3, 5, CAP_D)  # dark underside of the cap
m.box(0, 4, 0, 6, 4, 6, CAP)
for x, z in [(0, 0), (0, 6), (6, 0), (6, 6)]:
    m.set(x, 4, z, None)  # rounded cap corners
m.box(1, 5, 1, 5, 5, 5, CAP_L)  # domed top
for x, z in [(0, 0), (0, 6), (6, 0), (6, 6)]:
    m.set(x, 5, z, None)
for x, y, z in [(2, 5, 3), (4, 5, 2), (3, 4, 5), (1, 4, 4), (5, 4, 1), (3, 5, 4)]:
    m.set(x, y, z, SPECK)
save(m, "mushroom")

# Pot: terracotta clay pot with rounded belly, narrow neck and flared rim.
CLAY_D = hexc("#7a3e26")
CLAY = hexc("#a85836")
CLAY_L = hexc("#cf784f")
m = Model(12, 12, 12)
m.cyl(5.5, 5.5, 3.2, 0, 1, CLAY_D)
m.cyl(5.5, 5.5, 4.6, 2, 6, CLAY)
m.cyl(5.5, 5.5, 3.8, 7, 7, CLAY_D)
m.cyl(5.5, 5.5, 2.5, 8, 9, CLAY)
m.cyl(5.5, 5.5, 3.4, 10, 11, CLAY_L)
m.cyl(5.5, 5.5, 2.0, 10, 11, None)  # open mouth
m.box(3, 4, 9, 8, 5, 10, CLAY_L)  # front highlight band
save(m, "pot")

# Stalactite: tapered hanging stone icicle from ceiling (y=15 down to y=2).
m = Model(9, 16, 9)
m.cyl(4.0, 4.0, 3.5, 12, 15, STONE_D)
m.cyl(4.0, 4.0, 2.6, 9, 11, STONE)
m.cyl(4.0, 4.0, 1.8, 6, 8, STONE)
m.cyl(4.0, 4.0, 1.2, 4, 5, STONE_L)
m.box(4, 2, 4, 4, 3, 4, STONE_L)
save(m, "stalactite")

# Moss: low lush green moss tufts on cave floor.
m = Model(14, 4, 14)
m.box(1, 0, 1, 12, 1, 12, GRASS_D)
m.box(2, 1, 2, 11, 2, 11, GRASS)
m.box(3, 2, 4, 6, 3, 7, GRASS_L)
m.box(8, 2, 3, 10, 3, 6, GRASS_L)
m.box(5, 2, 8, 9, 3, 10, GRASS_L)
save(m, "moss")

# Small rock: irregular sculpted cave rock pile.
m = Model(12, 7, 12)
m.box(2, 0, 2, 9, 2, 9, STONE_D)
m.box(3, 2, 3, 8, 4, 8, STONE)
m.box(4, 4, 4, 7, 5, 6, STONE_L)
m.box(1, 0, 5, 3, 1, 8, STONE_DD)
m.box(8, 0, 3, 10, 2, 6, STONE)
save(m, "small_rock")

# Shelf mushroom: glowing cyan/blue cave mushroom cluster.
SM_D = hexc("#1b4f72")
SM = hexc("#2e86c1")
SM_L = hexc("#76d7ea")
m = Model(12, 10, 12)
m.box(5, 0, 5, 6, 4, 6, STEM_D)
m.box(1, 4, 2, 7, 5, 8, SM)
m.box(2, 5, 3, 6, 6, 7, SM_L)
m.box(5, 2, 7, 6, 3, 8, STEM)
m.box(4, 2, 6, 10, 3, 10, SM_D)
m.box(5, 3, 7, 9, 3, 9, SM_L)
save(m, "shelf_mushroom")
