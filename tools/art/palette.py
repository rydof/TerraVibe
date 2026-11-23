"""Named Terraria-ish colours (sampled into palette_data.py) and output locations shared by the art scripts."""
import os

import palette_data as _d

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
ASSETS = os.path.join(ROOT, "assets")


def hexc(s, a=255):
    """'#rrggbb' -> (r, g, b, a)."""
    s = s.lstrip("#")
    return (int(s[0:2], 16), int(s[2:4], 16), int(s[4:6], 16), a)


def shade(c, k):
    """Scales rgb by k (clamped); keeps alpha."""
    return (*(max(0, min(255, round(v * k))) for v in c[:3]), c[3])


TILE = 16  # pixels per tile edge in the generated textures
VPU = 16  # voxels per world unit

# Sampled from the local Terraria pack by sample_palette.py; hex values only.
DIRT = hexc(_d.DIRT)
DIRT_D = hexc(_d.DIRT_D)
DIRT_L = hexc(_d.DIRT_L)
DIRT_P = hexc(_d.DIRT_P)
STONE = hexc(_d.STONE)
STONE_D = hexc(_d.STONE_D)
STONE_DD = hexc(_d.STONE_DD)
STONE_L = hexc(_d.STONE_L)
GRASS = hexc(_d.GRASS)
GRASS_D = hexc(_d.GRASS_D)
GRASS_L = hexc(_d.GRASS_L)
SAND = hexc(_d.SAND)
SAND_D = hexc(_d.SAND_D)
SAND_L = hexc(_d.SAND_L)
MUD = hexc(_d.MUD)
MUD_D = hexc(_d.MUD_D)
MUD_L = hexc(_d.MUD_L)
JUNGLE = hexc(_d.JUNGLE)
JUNGLE_D = hexc(_d.JUNGLE_D)
JUNGLE_L = hexc(_d.JUNGLE_L)
SNOW = hexc(_d.SNOW)
SNOW_D = hexc(_d.SNOW_D)
SNOW_L = hexc(_d.SNOW_L)
ICE = hexc(_d.ICE)
ICE_D = hexc(_d.ICE_D)
ICE_L = hexc(_d.ICE_L)
WOOD = hexc(_d.WOOD)
WOOD_D = hexc(_d.WOOD_D)
WOOD_L = hexc(_d.WOOD_L)
WOOD_G = hexc(_d.WOOD_G)
BARK = hexc(_d.BARK)
BARK_D = hexc(_d.BARK_D)
BARK_L = hexc(_d.BARK_L)
RING = hexc(_d.RING)
RING_D = hexc(_d.RING_D)
SANDSTONE = hexc(_d.SANDSTONE)
SANDSTONE_D = hexc(_d.SANDSTONE_D)
SANDSTONE_L = hexc(_d.SANDSTONE_L)
HSAND = hexc(_d.HSAND)
HSAND_D = hexc(_d.HSAND_D)
HSAND_L = hexc(_d.HSAND_L)
GLASS_F = hexc(_d.GLASS_F)
GLASS_G = hexc(_d.GLASS_G)
COPPER = hexc(_d.COPPER)
COPPER_L = hexc(_d.COPPER_L)
IRON = hexc(_d.IRON)
IRON_L = hexc(_d.IRON_L)
SILVER = hexc(_d.SILVER)
SILVER_L = hexc(_d.SILVER_L)
GOLD = hexc(_d.GOLD)
GOLD_L = hexc(_d.GOLD_L)
EBONSTONE = hexc(_d.EBONSTONE)
EBONSTONE_D = hexc(_d.EBONSTONE_D)
EBONSTONE_DD = hexc(_d.EBONSTONE_DD)
EBONSTONE_L = hexc(_d.EBONSTONE_L)
CORRUPT = hexc(_d.CORRUPT)
CORRUPT_D = hexc(_d.CORRUPT_D)
CORRUPT_L = hexc(_d.CORRUPT_L)
DEMONITE = hexc(_d.DEMONITE)
DEMONITE_L = hexc(_d.DEMONITE_L)
COPPER_D = hexc(_d.COPPER_D)
IRON_D = hexc(_d.IRON_D)
SILVER_D = hexc(_d.SILVER_D)
GOLD_D = hexc(_d.GOLD_D)
DEMONITE_D = hexc(_d.DEMONITE_D)


def mix(a, b, t):
    """Linear blend of two colours, t=0 -> a, t=1 -> b (alpha from a)."""
    return (*(round(x + (y - x) * t) for x, y in zip(a[:3], b[:3])), a[3])


# Corruption soil: surface dirt pulled toward the purple-black ebonstone family.
CDIRT_P = mix(DIRT_P, EBONSTONE_DD, 0.65)
CDIRT_D = mix(DIRT_D, EBONSTONE_DD, 0.6)
CDIRT = mix(DIRT, EBONSTONE_D, 0.6)


def ramp(name):
    """(dark, base, light) for a material family, e.g. ramp("DIRT")."""
    g = globals()
    base = g[name]
    return g.get(name + "_D", shade(base, 0.8)), base, g.get(name + "_L", shade(base, 1.2))


# Dungeon, Corruption orb, and Underworld block palette colours.
ORB_D = hexc("#3b1a5a")
ORB = hexc("#6b359c")
ORB_L = hexc("#a86be0")

DB_BLUE_D = hexc("#1f2e52")
DB_BLUE = hexc("#354e82")
DB_BLUE_L = hexc("#5472b0")

DB_GREEN_D = hexc("#1e422c")
DB_GREEN = hexc("#326946")
DB_GREEN_L = hexc("#4f9468")

DB_PINK_D = hexc("#522146")
DB_PINK = hexc("#823870")
DB_PINK_L = hexc("#b55aa0")

SPIKE_D = hexc("#4c515c")
SPIKE = hexc("#7e8494")
SPIKE_L = hexc("#c4cad6")

OBS_D = hexc("#140f1e")
OBS = hexc("#281f3d")
OBS_L = hexc("#48396b")

ASH_D = hexc("#2b292e")
ASH = hexc("#45424a")
ASH_L = hexc("#635f69")

HELL_D = hexc("#4a1814")
HELL = hexc("#8f2c1b")
HELL_L = hexc("#ff6e24")

OBS_BRICK_D = hexc("#181324")
OBS_BRICK = hexc("#32284a")
OBS_BRICK_L = hexc("#534378")

HELL_BRICK_D = hexc("#3b1614")
HELL_BRICK = hexc("#69251e")
HELL_BRICK_L = hexc("#d95326")

# Wave 1 Pre-Hardmode block/model palette colours.
MUSH_BLUE_D = hexc("#16347a")
MUSH_BLUE = hexc("#285ec4")
MUSH_BLUE_L = hexc("#5ca3ff")

CLOUD_D = hexc("#c7d8eb")
CLOUD = hexc("#ebf4fc")
CLOUD_L = hexc("#ffffff")

SUNPLATE_D = hexc("#8a5d1c")
SUNPLATE = hexc("#d69f38")
SUNPLATE_L = hexc("#ffe278")

METEOR_D = hexc("#331c1a")
METEOR = hexc("#633326")
METEOR_L = hexc("#ff7828")

HIVE_D = hexc("#804914")
HIVE = hexc("#bf7821")
HIVE_L = hexc("#ebaa42")
