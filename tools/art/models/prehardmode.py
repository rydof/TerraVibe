"""Wave 1 Pre-Hardmode block and item .vox models."""
import os

from palette import *
from vox import Model

BLOCK_OUT = os.path.join(ASSETS, "models", "blocks")
ITEM_OUT = os.path.join(ASSETS, "models", "items")


def save_block(m, name):
    m.save(os.path.join(BLOCK_OUT, name + ".vox"))


def save_item(m, name):
    m.save(os.path.join(ITEM_OUT, name + ".vox"))


# --- Block Models ---

# Glowing Mushroom plant: pale blue-white stem with glowing cyan-blue cap
GM_STEM = hexc("#b5d6ff")
GM_CAP_D = hexc("#16347a")
GM_CAP = hexc("#2b6ef2")
GM_CAP_L = hexc("#70c4ff")
m = Model(9, 10, 9)
m.box(4, 0, 4, 4, 4, 4, GM_STEM)
m.box(2, 5, 2, 6, 6, 6, GM_CAP_D)
m.box(1, 6, 1, 7, 7, 7, GM_CAP)
for x, z in [(1, 1), (1, 7), (7, 1), (7, 7)]:
    m.set(x, 6, z, None)
    m.set(x, 7, z, None)
m.box(2, 8, 2, 6, 9, 6, GM_CAP_L)
for x, z in [(2, 2), (2, 6), (6, 2), (6, 6)]:
    m.set(x, 9, z, None)
save_block(m, "glowing_mushroom")

# Campfire: crossed logs over stones with bright orange-gold flames
LOG = hexc("#6e4221")
LOG_D = hexc("#472912")
FLAME_O = hexc("#ff6a14")
FLAME_M = hexc("#ffb326")
FLAME_C = hexc("#fff08a")
m = Model(14, 14, 14)
m.cyl(6.5, 6.5, 5.5, 0, 1, STONE_D)
m.cyl(6.5, 6.5, 3.8, 1, 1, None)
m.box(3, 1, 5, 10, 2, 8, LOG)
m.box(5, 1, 3, 8, 3, 10, LOG_D)
m.box(4, 3, 4, 9, 6, 9, FLAME_O)
m.box(5, 5, 5, 8, 9, 8, FLAME_M)
m.box(6, 7, 6, 7, 12, 7, FLAME_C)
m.set(6, 13, 6, FLAME_C)
save_block(m, "campfire")

# Heart Lantern: hanging iron chain + glowing ruby heart cage
HEART_D = hexc("#99112b")
HEART_M = hexc("#eb284b")
HEART_L = hexc("#ff8ca1")
CHAIN = hexc("#6d7585")
m = Model(12, 16, 12)
m.box(5, 12, 5, 6, 15, 6, CHAIN)
m.box(4, 11, 4, 7, 11, 7, IRON_D)
m.box(4, 2, 4, 7, 10, 7, HEART_M)
m.box(3, 4, 4, 8, 9, 7, HEART_M)
m.box(2, 6, 5, 9, 8, 6, HEART_L)
m.box(4, 2, 4, 5, 3, 7, HEART_D)
m.box(5, 1, 5, 6, 1, 6, IRON_D)
save_block(m, "heart_lantern")

# Tinkerer's Workshop: blue/gold inventor bench with gear & anvil detail
TW_WOOD = hexc("#7a4d28")
TW_WOOD_D = hexc("#4f3018")
TW_CLOTH = hexc("#2b5cb8")
TW_GOLD = hexc("#ebc346")
m = Model(16, 14, 14)
m.box(1, 0, 2, 3, 7, 4, TW_WOOD_D)
m.box(12, 0, 2, 14, 7, 4, TW_WOOD_D)
m.box(1, 0, 9, 3, 7, 11, TW_WOOD_D)
m.box(12, 0, 9, 14, 7, 11, TW_WOOD_D)
m.box(0, 8, 1, 15, 10, 12, TW_WOOD)
m.box(1, 10, 2, 7, 11, 10, TW_CLOTH)
m.box(9, 11, 4, 13, 13, 8, TW_GOLD)
m.box(10, 11, 5, 12, 12, 7, IRON_L)
save_block(m, "tinkerers_workshop")

# --- Item Models ---

# 65.vox: Starfury (pink-purple hilt, golden crossguard, starry blade)
SF_HILT = hexc("#c4459c")
SF_GUARD = hexc("#ffd040")
SF_BLADE = hexc("#f56eb3")
SF_EDGE = hexc("#ffccf2")
m = Model(16, 16, 2)
for i in range(4):
    m.box(1 + i, 1 + i, 0, 2 + i, 2 + i, 1, SF_HILT)
m.box(3, 5, 0, 6, 6, 1, SF_GUARD)
m.box(5, 3, 0, 6, 5, 1, SF_GUARD)
for i in range(6, 14):
    m.box(i, i, 0, i + 1, i + 1, 1, SF_BLADE)
    m.set(i, i + 1, 0, SF_EDGE)
    m.set(i, i + 1, 1, SF_EDGE)
m.box(14, 14, 0, 15, 15, 1, SF_GUARD)
save_item(m, "65")

# 84.vox: Grappling Hook (iron 3-prong claw + chain base)
m = Model(14, 14, 2)
for i in range(1, 9):
    m.box(i, i, 0, i + 1, i + 1, 1, IRON)
m.box(7, 9, 0, 11, 12, 1, IRON_L)
m.box(9, 7, 0, 12, 11, 1, IRON_L)
m.box(11, 11, 0, 13, 13, 1, IRON_L)
m.box(1, 1, 0, 3, 3, 1, CHAIN)
save_item(m, "84")

# 166.vox: Bomb (dark iron sphere with lit fuse)
m = Model(10, 12, 10)
m.cyl(4.5, 4.5, 4.0, 0, 7, STONE_D)
m.box(4, 8, 4, 5, 10, 5, LOG)
m.box(4, 10, 4, 5, 11, 5, FLAME_C)
save_item(m, "166")

# 167.vox: Dynamite (bundle of red sticks + fuse)
DYN_RED = hexc("#c92828")
DYN_BAND = hexc("#2b2b2e")
m = Model(8, 14, 8)
m.box(2, 0, 2, 5, 10, 5, DYN_RED)
m.box(2, 2, 2, 5, 3, 5, DYN_BAND)
m.box(2, 7, 2, 5, 8, 5, DYN_BAND)
m.box(3, 11, 3, 4, 12, 4, LOG)
m.set(4, 13, 4, FLAME_C)
save_item(m, "167")

# 168.vox: Grenade (olive ribbed body + pin)
GREN_G = hexc("#4a6b35")
GREN_L = hexc("#749c54")
m = Model(8, 10, 8)
m.cyl(3.5, 3.5, 3.2, 0, 6, GREN_G)
m.box(2, 3, 2, 5, 4, 5, GREN_L)
m.box(3, 7, 3, 4, 9, 4, IRON_L)
save_item(m, "168")
