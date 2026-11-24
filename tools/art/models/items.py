"""Held items at 1/16 voxels: pickaxes, axes, swords, the wooden bow and arrow,
and the torch.

Sprites in the xy plane, two voxels thick (z 0..1). The grip is at the
bottom-left and the tip at the top-right; the client mounts them in the hand or
in front of the first-person camera. Swords, pickaxes and axes are 19 voxels
(1.2 blocks) across. File names are the item ids (`crates/sim/src/item/mod.rs`).
"""
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from palette import *
from vox import Model

OUT = os.path.join(ASSETS, "models", "items")
N = 19  # 1.2 blocks at 16 voxels per block
HANDLE = hexc("#8a5a2c")
HANDLE_D = hexc("#5e3b1c")
HANDLE_L = hexc("#b07a40")
GRIP = hexc("#3c2a1a")  # the pommel and the leather wrap band
STRING = hexc("#f2ecd8")
FEATHER = hexc("#ffffff")
FEATHER_D = hexc("#c8ccd6")
FLAME_W = hexc("#fff3a0")
FLAME_Y = hexc("#ffcf33")
FLAME_O = hexc("#ff8a1e")
FLAME_R = hexc("#d9480f")

# (main, light, dark) per metal tier.
METAL = {
    "copper": (COPPER, COPPER_L, hexc("#9c4a18")),
    "iron": (hexc("#a9aebb"), hexc("#e3e7ee"), hexc("#6c7280")),
    "silver": (SILVER, SILVER_L, hexc("#8493ac")),
    "gold": (GOLD, GOLD_L, hexc("#b08a14")),
    "demonite": (DEMONITE, DEMONITE_L, DEMONITE_D),
    # M10 Molten tier: dark basalt body, molten yellow edge, deep magma bevel.
    "molten": (hexc("#332b34"), FLAME_Y, hexc("#7a1c06")),
}

# Item ids: gameplay spec via crates/sim/src/item/mod.rs.
PICK = {"copper": 3509, "iron": 1, "silver": 3515, "gold": 3521, "demonite": 103}
AXE = {"copper": 3506, "iron": 10, "silver": 3512, "gold": 3518, "demonite": 45}
BROAD = {"copper": 3508, "iron": 4, "silver": 3514, "gold": 3520, "demonite": 46}
SHORT = {"copper": 3507}
WOOD_SWORD, WOOD_BOW, WOOD_ARROW, TORCH = 24, 39, 40, 8


def px(m, x, y, c, z0=1, z1=2):
    """Core thickness: fills z in [z0, z1] (default inner 2 layers of a 4-voxel model)."""
    for z in range(z0, min(z1 + 1, m.size[2])):
        m.set(x, y, z, c)


def px_full(m, x, y, c):
    """Full 4-voxel sculpted depth (z = 0..3) for ridges, collars, crossguards, and pommels."""
    px(m, x, y, c, 0, m.size[2] - 1)


def save(m, item_id):
    m.save(os.path.join(OUT, f"{item_id}.vox"))


def handle(m, end):
    """Sculpted 3D wooden handle on the diagonal from the pommel (0,0) to (end,end)
    with a 4-voxel pommel cap, rounded grip spine, and leather wrap bands."""
    # Rounded 4-voxel pommel cap
    px_full(m, 0, 0, GRIP)
    px_full(m, 1, 1, GRIP)
    px(m, 1, 0, HANDLE_D)
    px(m, 0, 1, HANDLE_D)
    for i in range(2, end + 1):
        wrap = (i % 2 == 0)
        # Central spine is full 4 voxels thick; shaded sides taper to 2 voxels
        px_full(m, i, i, GRIP if wrap else HANDLE_L)
        px(m, i + 1, i, HANDLE_D)
        px(m, i, i + 1, HANDLE)


def pickaxe(tier, item_id):
    if tier == "demonite":
        nightmare_pickaxe()
        return
    if tier == "molten":
        molten_pickaxe()
        return
    main, light, dark = METAL[tier]
    m = Model(N, N, 4)
    handle(m, 9)
    # Heavy 4-voxel socket/collar joining head to handle
    for x in range(9, 12):
        for y in range(9, 12):
            px_full(m, x, y, dark)
    px_full(m, 10, 10, main)
    # Curved pickaxe head: central ridge is 4 voxels thick, beveled edges 2 voxels thick
    for x in range(6, 15):
        y_center = 20 - x
        px_full(m, x, y_center, main)
        px(m, x, y_center - 1, dark)
        px(m, x, y_center + 1, light)
    # Upper pick horn tapering to sharp point
    px_full(m, 5, 15, main)
    px(m, 5, 14, dark)
    px(m, 4, 15, light)
    px(m, 4, 14, dark)
    # Lower pick horn tapering to sharp point
    px_full(m, 15, 5, main)
    px(m, 14, 5, dark)
    px(m, 15, 4, light)
    px(m, 14, 4, dark)
    save(m, item_id)


def nightmare_pickaxe():
    """Terraria Nightmare Pickaxe (22x22x4): curved jagged demonite horns & glowing eye socket."""
    main, light, dark = METAL["demonite"]
    m = Model(22, 22, 4)
    handle(m, 11)
    for x in range(10, 14):
        for y in range(10, 14):
            px_full(m, x, y, dark)
    px_full(m, 12, 12, light)  # glowing violet eye socket
    px_full(m, 11, 11, EBONSTONE_DD)
    for d in range(-6, 7):
        cx = 12 + d
        cy = 12 - d + (abs(d) // 3)
        if 1 <= cx < 21 and 1 <= cy < 21:
            px_full(m, cx, cy, main)
            px(m, cx, cy + 1, light if abs(d) >= 3 else main)
            px(m, cx, cy - 1, dark)
            if abs(d) in (3, 5):  # jagged demonite barbs
                px(m, cx - (1 if d < 0 else 0), cy - 2, dark)
    # Sharp curved pick tips
    px_full(m, 5, 19, light)
    px(m, 4, 20, light)
    px_full(m, 19, 7, light)
    px(m, 20, 6, light)
    save(m, 103)


def molten_pickaxe():
    """Terraria Molten Pickaxe (22x22x4): blazing magma-rimmed basalt pick with curving fiery horns."""
    main, _, dark = METAL["molten"]
    m = Model(22, 22, 4)
    handle(m, 11)
    for x in range(10, 14):
        for y in range(10, 14):
            px_full(m, x, y, dark)
    px_full(m, 12, 12, FLAME_O)
    for d in range(-6, 7):
        cx = 12 + d
        cy = 12 - d + (abs(d) // 3)
        if 1 <= cx < 21 and 1 <= cy < 21:
            px_full(m, cx, cy, main if abs(d) < 4 else FLAME_R)
            px(m, cx, cy + 1, FLAME_Y if abs(d) >= 3 else FLAME_O)
            px(m, cx, cy - 1, dark)
    px_full(m, 5, 19, FLAME_Y)
    px(m, 4, 20, FLAME_W)
    px_full(m, 19, 7, FLAME_Y)
    px(m, 20, 6, FLAME_W)
    save(m, 122)


def axe(tier, item_id):
    """3D bearded battle-axe: long diagonal haft extending through a socketed collar
    with a back poll, flared crescent blade on the +X (forward) side, sharp 1-2 voxel
    cutting edge, and clear negative space under the beard."""
    if tier == "demonite":
        war_axe_of_the_night()
        return
    if tier == "molten":
        molten_hamaxe()
        return
    main, light, dark = METAL[tier]
    m = Model(N, N, 4)
    handle(m, 12)
    # Haft tip protruding cleanly above the socket
    px_full(m, 13, 13, HANDLE_L)
    # Narrow socket collar (10..12) with reinforced 4-voxel dark back poll
    for i in (10, 11, 12):
        px_full(m, i, i, dark)
    for (x, y) in ((9, 11), (9, 12), (10, 12), (10, 13)):
        px_full(m, x, y, dark)
    # Thick 4-voxel socket cheek tapering into the flared crescent blade (+X side)
    for (x, y) in ((11, 10), (12, 10), (12, 11), (13, 10), (13, 11)):
        px_full(m, x, y, main)
    px_full(m, 12, 12, dark)
    # Mid-blade cheek (2 voxels thick in Z)
    for (x, y) in (
        (13, 9),
        (14, 9), (14, 10), (14, 11), (14, 12),
        (15, 6), (15, 7), (15, 8), (15, 9), (15, 10), (15, 11), (15, 12),
    ):
        px(m, x, y, main)
    # Bearded lower hook (clear air gap at (10..13, 6..8) between beard and haft)
    px(m, 14, 5, dark)
    px(m, 15, 5, main)
    # Inner bevel (2 voxels thick) and sharp 1-voxel outer cutting edge (z=1)
    for y in range(5, 12):
        px(m, 16, y, light if 6 <= y <= 10 else main)
    for y in range(6, 11):
        m.set(17, y, 1, light)
    m.set(16, 12, 1, light)
    m.set(15, 13, 1, light)
    m.set(15, 4, 1, light)
    save(m, item_id)


def war_axe_of_the_night():
    """Terraria War Axe of the Night (22x22x4): jagged double-notch demonite war axe."""
    main, light, dark = METAL["demonite"]
    m = Model(22, 22, 4)
    handle(m, 14)
    px_full(m, 15, 15, dark)
    for i in (12, 13, 14):
        px_full(m, i, i, dark)
    px_full(m, 13, 13, light)  # glowing violet eye
    for x in range(13, 18):
        for y in range(9, 16):
            if (x == 17 and y in (11, 13)) or (x == 13 and y in (9, 10)):
                continue  # jagged double-notch silhouette
            px_full(m, x, y, main if x < 16 else light)
    for y in (8, 9, 10, 12, 14, 15, 16):
        m.set(18, y, 1, light)
    m.set(17, 16, 1, light)
    m.set(16, 17, 1, light)
    m.set(17, 8, 1, light)
    px_full(m, 11, 14, dark)
    px_full(m, 11, 15, main)
    save(m, 45)


def molten_hamaxe():
    """Terraria Molten Hamaxe (23x23x4): dual-headed fiery axe + glowing magma hammer."""
    main, _, dark = METAL["molten"]
    m = Model(23, 23, 4)
    handle(m, 15)
    px_full(m, 16, 16, dark)
    for i in (13, 14, 15):
        px_full(m, i, i, dark)
    px_full(m, 14, 14, FLAME_O)
    # Fiery axe blade on +X side
    for x in range(15, 19):
        for y in range(9, 17):
            px_full(m, x, y, main if x < 17 else FLAME_R)
    for y in range(8, 18):
        px(m, 19, y, FLAME_O)
        m.set(20, y, 1, FLAME_Y if 10 <= y <= 15 else FLAME_O)
    m.set(20, 13, 1, FLAME_W)
    # Glowing magma hammer poll on -X/+Y side
    for x in range(10, 14):
        for y in range(15, 19):
            px_full(m, x, y, main if x > 10 else FLAME_O)
    px_full(m, 9, 16, FLAME_Y)
    px_full(m, 9, 17, FLAME_W)
    save(m, 217)


def sword(tier, item_id, wide):
    if tier == "demonite":
        lights_bane()
        return
    if not wide:
        shortsword(tier, item_id)
        return
    save(sword_model(tier, wide), item_id)


def shortsword(tier, item_id):
    """Compact 14x14x4 stabbing shortsword with narrow pointed gladius blade and small crossguard."""
    main, light, dark = METAL[tier]
    m = Model(14, 14, 4)
    handle(m, 3)
    # Small compact crossguard at g=4
    for d in (-2, -1, 0, 1, 2):
        gx, gy = 4 + d, 4 - d
        if 0 <= gx < 14 and 0 <= gy < 14:
            px_full(m, gx, gy, dark if abs(d) == 2 else main)
    # Narrow gladius blade (5..12)
    for i in range(5, 12):
        px_full(m, i, i, light if i >= 9 else main)
        px(m, i - 1, i, light)
        px(m, i, i - 1, dark)
    px_full(m, 12, 12, light)
    px(m, 12, 13, light)
    px(m, 13, 12, light)
    px(m, 13, 13, light)
    save(m, item_id)


def sword_model(tier, wide):
    """Progressive Terraria broadswords: Copper 19, Iron 20, Silver 21, Gold 22."""
    sizes = {"copper": 19, "iron": 20, "silver": 21, "gold": 22}
    sz = sizes.get(tier, N)
    main, light, dark = METAL[tier]
    m = Model(sz, sz, 4)
    handle(m, 4)
    if tier == "silver":
        px_full(m, 0, 0, hexc("#4fb6ff"))  # sapphire pommel
    # Crossguard at g=5 (ornate winged guard with ruby gem for gold)
    g = 5
    span = 5 if tier == "gold" else 4
    for d in range(-span, span + 1):
        gx = g + d
        gy = g - d
        if 0 <= gx < sz and 0 <= gy < sz:
            is_end = abs(d) >= span - 1
            c = light if (tier == "gold" and is_end) else (dark if is_end else main)
            px_full(m, gx, gy, c)
            if tier == "gold" and is_end and 0 <= gy + 1 < sz:
                px_full(m, gx, gy + 1, light)
            elif 0 <= gy + 1 < sz:
                px(m, gx, gy + 1, light if d <= 0 else dark)
            if 0 <= gy - 1 < sz:
                px(m, gx, gy - 1, dark)
    if tier == "gold":
        px_full(m, 5, 5, hexc("#d8283c"))  # ruby gem in guard
    # Ricasso
    px_full(m, 5, 6, dark)
    px_full(m, 6, 5, dark)
    px_full(m, 6, 6, main)
    # Shaped fuller & beveled cutting edges
    tip = sz - 2
    for i in range(7, tip):
        fuller = dark if (8 <= i <= tip - 3) else main
        px_full(m, i, i, fuller)
        px(m, i - 1, i, main)
        px(m, i - 1, i + 1, light)
        px(m, i, i + 1, light)
        px(m, i + 1, i, dark)
        if wide:
            px(m, i + 1, i - 1, dark)
            px(m, i, i - 1, main)
    # Tapered point
    px(m, tip - 1, tip, light)
    px(m, tip, tip - 1, dark)
    px_full(m, tip, tip, main)
    px(m, tip, tip + 1, light)
    px(m, tip + 1, tip, light)
    px(m, tip + 1, tip + 1, light)
    return m


def lights_bane():
    """Terraria Light's Bane (24x24x4): jagged asymmetric demonite greatsword with amethyst vein."""
    main, light, dark = METAL["demonite"]
    m = Model(24, 24, 4)
    handle(m, 5)
    # Curved demonic twin-prong guard
    for d in range(-5, 6):
        gx, gy = 6 + d, 6 - d
        if 0 <= gx < 24 and 0 <= gy < 24:
            px_full(m, gx, gy, dark if abs(d) < 4 else main)
            if abs(d) >= 4 and gy + 1 < 24 and gx + 1 < 24:
                px_full(m, gx + 1, gy + 1, light)
    px_full(m, 6, 6, light)
    # Obsidian-purple blade with glowing amethyst central vein & jagged notches
    for i in range(7, 21):
        px_full(m, i, i, light if i % 2 == 0 else main)  # glowing amethyst vein
        px(m, i - 1, i, dark)
        if i not in (11, 15, 18):  # asymmetric wicked blade notches
            px(m, i - 1, i + 1, light)
            px(m, i, i + 1, main)
        px(m, i + 1, i, dark)
        if i not in (13, 17):
            px(m, i + 1, i - 1, EBONSTONE_DD)
            px(m, i, i - 1, dark)
    px_full(m, 21, 21, main)
    px(m, 21, 22, light)
    px(m, 22, 21, light)
    px(m, 22, 22, light)
    px(m, 23, 23, light)
    save(m, 46)


def wooden_sword():
    """Carved oak broadsword with 4-voxel crossguard and beveled wood blade."""
    m = Model(N, N, 4)
    handle(m, 4)
    g = 5
    for d in range(-4, 5):
        gx = g + d
        gy = g - d
        if 0 <= gx < N and 0 <= gy < N:
            is_end = abs(d) >= 3
            px_full(m, gx, gy, HANDLE_D if is_end else HANDLE)
            if 0 <= gy + 1 < N:
                px(m, gx, gy + 1, HANDLE_L if d <= 0 else HANDLE_D)
            if 0 <= gy - 1 < N:
                px(m, gx, gy - 1, HANDLE_D)
    px_full(m, 5, 6, HANDLE_D)
    px_full(m, 6, 5, HANDLE_D)
    px_full(m, 6, 6, HANDLE)
    for i in range(7, 17):
        px_full(m, i, i, HANDLE)
        px(m, i - 1, i, HANDLE)
        px(m, i - 1, i + 1, HANDLE_L)
        px(m, i, i + 1, HANDLE_L)
        px(m, i + 1, i, HANDLE_D)
        px(m, i + 1, i - 1, HANDLE_D)
        px(m, i, i - 1, HANDLE)
    px(m, 16, 17, HANDLE_L)
    px(m, 17, 16, HANDLE_D)
    px_full(m, 17, 17, HANDLE)
    px(m, 17, 18, HANDLE_L)
    px(m, 18, 17, HANDLE_L)
    px(m, 18, 18, HANDLE_L)
    save(m, WOOD_SWORD)


def wooden_bow():
    """Upright 3D bow: the limb bulges toward +x, the string runs at x = 0."""
    m = Model(9, N, 4)
    for y in range(N):
        off = round(5 * math.sin(math.pi * y / (N - 1)))
        c = GRIP if 8 <= y <= 10 else (HANDLE_L if y % 4 else HANDLE)
        px_full(m, 1 + off, y, c)
        px(m, 2 + off, y, HANDLE_D)
    for y in range(1, N - 1):
        m.set(0, y, 1, STRING)
        m.set(0, y, 2, STRING)
    px_full(m, 1, 0, GRIP)
    px_full(m, 1, N - 1, GRIP)
    save(m, WOOD_BOW)


def arrow():
    m = Model(N, N, 2)
    # Shaft on the diagonal, shaded on one side, with a steel head at the tip.
    for i in range(3, 14):
        px(m, i, i, HANDLE_L)
        px(m, i + 1, i, HANDLE)
    # Steel head: shaded base, bright edge toward the tip.
    steel, steel_d = hexc("#e3e7ee"), hexc("#8a8f9c")
    px(m, 14, 14, steel_d)
    px(m, 15, 14, steel_d)
    px(m, 14, 15, steel_d)
    px(m, 15, 15, steel)
    px(m, 16, 16, steel)
    px(m, 17, 17, hexc("#aeb3bf"))
    # Two-tone fletching at the nock.
    for x, y in ((1, 3), (3, 1), (0, 4), (4, 0)):
        px(m, x, y, FEATHER)
    px(m, 2, 2, FEATHER_D)
    save(m, WOOD_ARROW)


def torch():
    m = Model(6, 16, 2)
    for y in range(0, 9):
        px(m, 2, y, HANDLE_D)
        px(m, 3, y, HANDLE)
    px(m, 2, 8, HANDLE_L)
    px(m, 3, 8, HANDLE_L)
    # Flame: orange base, yellow body, white core, red licks at the edges.
    px(m, 2, 9, FLAME_O)
    px(m, 3, 9, FLAME_O)
    for x in (1, 2, 3, 4):
        px(m, x, 10, FLAME_Y)
    px(m, 1, 10, FLAME_R)
    px(m, 4, 10, FLAME_O)
    for x in (1, 4):
        px(m, x, 11, FLAME_O)
    for x in (2, 3):
        px(m, x, 11, FLAME_W)
    px(m, 2, 12, FLAME_W)
    px(m, 3, 12, FLAME_Y)
    px(m, 2, 13, FLAME_Y)
    px(m, 3, 13, FLAME_W)
    px(m, 3, 14, FLAME_Y)
    save(m, TORCH)


GEL_BASE = hexc("#1e88e5")
GEL_DARK = hexc("#0d47a1")
GEL_MID = hexc("#1565c0")
GEL_LIGHT = hexc("#42a5f5")
GEL_PALE = hexc("#90caf9")
GEL_HI = hexc("#ffffff")


def wood_drop():
    """3D carved wood log (12x6x6 voxels) with bark exterior, longitudinal fissures,
    a severed branch knot, and growth rings with dark pith at both ends."""
    m = Model(12, 6, 6)
    for x in range(12):
        for y in range(6):
            for z in range(6):
                # Cylindrical cross section: omit four 1x1 corner voxels
                if (y == 0 or y == 5) and (z == 0 or z == 5):
                    continue
                is_end = x == 0 or x == 11
                is_outer = y == 0 or y == 5 or z == 0 or z == 5
                if is_end:
                    if is_outer:
                        c = BARK_D if (y + z) % 2 == 0 else BARK
                    elif y in (2, 3) and z in (2, 3):
                        c = RING_D if (y == 2 and z == 2) else WOOD_D
                    else:
                        c = RING if (y + z) % 2 == 0 else WOOD_L
                else:
                    if is_outer:
                        if y == 5 and 4 <= x <= 6 and 2 <= z <= 3:
                            c = BARK_D if (x in (4, 6) or z == 2) else WOOD_D
                        elif (z == 0 and x % 3 == 0) or (y == 0 and (x + 1) % 4 == 0) or (z == 5 and x % 4 == 1):
                            c = BARK_D
                        elif (y == 5 and (x + 2) % 3 == 0) or (z == 5 and (x + 2) % 4 == 0):
                            c = BARK_L
                        else:
                            c = BARK
                    else:
                        c = WOOD if y in (2, 3) and z in (2, 3) else WOOD_D
                m.set(x, y, z, c)
    save(m, 9)


def gel_drop():
    """3D organic gel droplet (8x8x8 voxels) with rounded blob profile,
    translucent blue gradient, and a specular highlight on the upper shoulder."""
    m = Model(8, 8, 8)
    legend = {
        "d": GEL_DARK,
        "m": GEL_MID,
        "b": GEL_BASE,
        "l": GEL_LIGHT,
        "p": GEL_PALE,
        "h": GEL_HI,
    }
    layers = [
        # y = 0: bottom base
        [
            "........",
            "..dddd..",
            "..dddd..",
            "..dddd..",
            "..dddd..",
            "........",
            "........",
            "........",
        ],
        # y = 1: lower curve
        [
            "........",
            "..dddd..",
            ".dddddd.",
            ".dddddd.",
            ".dddddd.",
            ".dddddd.",
            "..dddd..",
            "........",
        ],
        # y = 2: wide belly lower
        [
            "..mmmm..",
            ".mmmmmm.",
            "mmmmmmmm",
            "mmmmmmmm",
            "mmmmmmmm",
            "mmmmmmmm",
            ".mmmmmm.",
            "..mmmm..",
        ],
        # y = 3: wide belly upper
        [
            "..bbbb..",
            ".bbbbbb.",
            "bbbbbbbb",
            "bbbbbbbb",
            "bbbbbbbb",
            "bbbbbbbb",
            ".bbbbbb.",
            "..bbbb..",
        ],
        # y = 4: tapering body
        [
            "........",
            "..bbbb..",
            ".bbbbbb.",
            ".bbbbbb.",
            ".bbbbbb.",
            ".bbbbbb.",
            "..bbbb..",
            "........",
        ],
        # y = 5: droplet slope with highlight
        [
            "........",
            "........",
            "..llll..",
            "..llhh..",
            "..llhh..",
            "..llll..",
            "........",
            "........",
        ],
        # y = 6: droplet crown
        [
            "........",
            "........",
            "...ll...",
            "..llh...",
            "...ll...",
            "........",
            "........",
            "........",
        ],
        # y = 7: droplet tip
        [
            "........",
            "........",
            "...pp...",
            "...pp...",
            "........",
            "........",
            "........",
            "........",
        ],
    ]
    m.layers(layers, legend)
    save(m, 23)


RED_D = hexc("#a3122a")
RED = hexc("#e02848")
RED_L = hexc("#ff6a84")
RED_W = hexc("#ffc2cf")


def band_of_regeneration():
    """Silver ring (10x10x6) set with a glowing ruby stone."""
    m = Model(10, 10, 6)
    for x in range(10):
        for y in range(10):
            d2 = (x - 4.5) ** 2 + (y - 4.5) ** 2
            if 2.2**2 <= d2 <= 4.6**2:
                for z in range(1, 5):
                    c = SILVER_L if (x + y > 9) else (SILVER_D if (x + y < 7) else SILVER)
                    m.set(x, y, z, c)
    for x in (3, 6):
        for z in (1, 4):
            m.set(x, 8, z, SILVER_L)
            m.set(x, 9, z, SILVER_D)
    m.box(4, 8, 2, 5, 9, 3, RED)
    m.set(4, 9, 3, RED_W)
    m.set(5, 9, 3, RED_L)
    m.set(4, 8, 2, RED_D)
    save(m, 49)


def magic_mirror():
    """Handheld silver framed oval mirror with glowing glass (11x18x3)."""
    m = Model(11, 18, 3)
    for y in range(0, 7):
        px(m, 5, y, SILVER)
        px(m, 4, y, SILVER_D)
        px(m, 6, y, SILVER_L)
    px(m, 5, 0, SILVER_L)
    for x in range(11):
        for y in range(7, 18):
            d_outer = ((x - 5.0) / 4.8) ** 2 + ((y - 12.0) / 5.2) ** 2
            d_inner = ((x - 5.0) / 3.4) ** 2 + ((y - 12.0) / 3.8) ** 2
            if d_outer <= 1.0:
                if d_inner <= 1.0:
                    glass_c = ICE_L if (x + y in (15, 16, 17) or (x == 4 and y == 14)) else ICE
                    m.set(x, y, 1, glass_c)
                else:
                    for z in range(3):
                        c = SILVER_L if (x > 5 or y > 14) else (SILVER_D if x < 5 else SILVER)
                        m.set(x, y, z, c)
    px(m, 5, 17, SILVER_L)
    save(m, 50)


def cloud_in_a_bottle():
    """Glass bottle containing a fluffy white cloud (10x14x10)."""
    m = Model(10, 14, 10)
    m.cyl(4.5, 4.5, 1.6, 12, 13, BARK)
    m.cyl(4.5, 4.5, 1.2, 13, 13, BARK_L)
    for y in range(10, 12):
        for x in range(10):
            for z in range(10):
                d2 = (x - 4.5) ** 2 + (z - 4.5) ** 2
                if 1.5**2 <= d2 <= 2.4**2:
                    m.set(x, y, z, GLASS_G)
    for x in range(10):
        for z in range(10):
            d2 = (x - 4.5) ** 2 + (z - 4.5) ** 2
            if d2 <= 4.2**2:
                m.set(x, 0, z, GLASS_G)
    for y in range(1, 10):
        for x in range(10):
            for z in range(10):
                d2 = (x - 4.5) ** 2 + (z - 4.5) ** 2
                if 3.2**2 <= d2 <= 4.2**2:
                    m.set(x, y, z, GLASS_F if (x + z) % 3 == 0 else GLASS_G)
    CLOUD_W = (255, 255, 255, 255)
    CLOUD_S = (220, 230, 245, 255)
    CLOUD_D = (185, 200, 225, 255)
    for x in range(2, 8):
        for y in range(1, 7):
            for z in range(2, 8):
                d1 = ((x - 4.5) ** 2 + (y - 3.5) ** 2 + (z - 4.5) ** 2) / (2.2**2)
                d2 = ((x - 3.5) ** 2 + (y - 4.2) ** 2 + (z - 4.0) ** 2) / (1.8**2)
                d3 = ((x - 5.2) ** 2 + (y - 3.2) ** 2 + (z - 5.0) ** 2) / (1.7**2)
                if min(d1, d2, d3) <= 1.0:
                    c = CLOUD_W if y >= 4 else (CLOUD_S if y >= 2 else CLOUD_D)
                    m.set(x, y, z, c)
    save(m, 53)


def hermes_boots():
    """Winged running boots with golden wings (12x12x8)."""
    m = Model(12, 12, 8)
    for x in range(2, 10):
        for z in range(2, 6):
            m.set(x, 0, z, BARK_D)
    for y in range(1, 8):
        for x in range(2, 6):
            for z in range(2, 6):
                m.set(x, y, z, BARK if (y < 6) else BARK_L)
    for y in range(1, 4):
        for x in range(6, 10):
            for z in range(2, 6):
                m.set(x, y, z, BARK)
    for z_wing, side in ((1, -1), (6, 1)):
        for i in range(5):
            wx = 3 - i
            wy = 5 + i
            if 0 <= wx < 12 and 0 <= wy < 12:
                m.set(wx, wy, z_wing, GOLD_L)
                m.set(wx, wy - 1, z_wing, GOLD)
                m.set(wx + 1, wy, z_wing, GOLD_D)
        m.set(0, 10, z_wing, GOLD_L)
        m.set(1, 10, z_wing, GOLD_L)
        m.set(1, 11, z_wing, GOLD_L)
    save(m, 54)


def enchanted_boomerang():
    """Curved blue crescent boomerang with glowing engravings (16x16x4)."""
    m = Model(16, 16, 4)
    for x in range(1, 15):
        for y in range(1, 15):
            in_arm1 = 1 <= y <= 4 and 1 <= x <= 14
            in_arm2 = 1 <= x <= 4 and 1 <= y <= 14
            if (x >= 13 and (y < 2 or y > 3)) or (y >= 13 and (x < 2 or x > 3)):
                continue
            if in_arm1 or in_arm2:
                base_c = GEL_BASE if (x + y) % 2 == 0 else GEL_MID
                if x == 1 or y == 1:
                    base_c = GEL_DARK
                elif x == 4 or y == 4:
                    base_c = GEL_LIGHT
                if 2 <= x <= 3 or 2 <= y <= 3:
                    px_full(m, x, y, base_c)
                else:
                    px(m, x, y, base_c)
    for i in (3, 5, 7, 9, 11):
        px(m, i, 2, GEL_HI)
        px(m, 2, i, GEL_HI)
    px(m, 2, 2, GOLD_L)
    px(m, 3, 3, GOLD)
    save(m, 55)


def lucky_horseshoe():
    """Curved golden horseshoe (12x12x3)."""
    m = Model(12, 12, 3)
    for x in range(12):
        for y in range(12):
            dx = x - 5.5
            dy = y - 3.5
            d_outer = (dx / 4.8) ** 2 + (dy / 5.2) ** 2
            d_inner = (dx / 2.4) ** 2 + (dy / 2.8) ** 2
            if y > 7 and abs(dx) < 2.5:
                continue
            if d_outer <= 1.0 and (d_inner >= 1.0 or y > 4):
                for z in range(3):
                    c = GOLD_L if (z == 2 or x >= 8) else (GOLD_D if (z == 0 or x <= 3) else GOLD)
                    m.set(x, y, z, c)
    for nx, ny in ((2, 5), (2, 7), (2, 9), (9, 5), (9, 7), (9, 9), (5, 1), (6, 1)):
        if m.get(nx, ny, 2) is not None:
            m.set(nx, ny, 2, GOLD_D)
    save(m, 158)


def shiny_red_balloon():
    """Glossy red spherical balloon with tie and string (11x19x11)."""
    m = Model(11, 19, 11)
    for x in range(11):
        for y in range(7, 19):
            for z in range(11):
                d = ((x - 5.0) / 4.5) ** 2 + ((y - 12.5) / 5.0) ** 2 + ((z - 5.0) / 4.5) ** 2
                if d <= 1.0:
                    if x >= 6 and y >= 14 and z >= 6:
                        c = RED_W if (x == 7 and y == 15 and z == 7) else RED_L
                    elif x <= 3 or y <= 9 or z <= 3:
                        c = RED_D
                    else:
                        c = RED
                    m.set(x, y, z, c)
    m.box(4, 6, 4, 6, 7, 6, RED_D)
    m.box(4, 5, 4, 6, 5, 6, RED_D)
    m.set(5, 4, 5, STRING)
    m.set(5, 3, 5, STRING)
    m.set(6, 2, 5, STRING)
    m.set(6, 1, 5, STRING)
    m.set(5, 0, 5, STRING)
    save(m, 159)


def spear():
    """Proper long polearm shaft with broad flanged steel spearhead (26x26x4)."""
    m = Model(26, 26, 4)
    handle(m, 16)
    # Iron socket collar & side flanges
    for x in range(16, 19):
        for y in range(16, 19):
            px_full(m, x, y, IRON_D)
    px_full(m, 17, 17, IRON)
    px_full(m, 16, 18, IRON_L)
    px_full(m, 18, 16, IRON_D)
    # Broad flanged spearhead (18..25)
    for i in range(18, 25):
        px_full(m, i, i, IRON_L)
        px(m, i + 1, i, IRON)
        px(m, i, i + 1, IRON_D)
        if 19 <= i <= 22:
            px(m, i + 2, i, IRON_D)
            px(m, i, i + 2, IRON_L)
    px(m, 25, 25, IRON_L)
    save(m, 274)


def demonite_ore_drop():
    """3D purple crystal flecked ore chunk (10x10x10)."""
    m = Model(10, 10, 10)
    for x in range(10):
        for y in range(10):
            for z in range(10):
                d = ((x - 4.5) / 3.8) ** 2 + ((y - 4.5) / 3.5) ** 2 + ((z - 4.5) / 3.8) ** 2
                if d <= 1.0:
                    c = EBONSTONE_DD if (x + y + z) % 3 == 0 else (EBONSTONE_D if (x + z) % 2 == 0 else STONE_DD)
                    m.set(x, y, z, c)
    crystals = [
        (8, 6, 5, DEMONITE_L), (8, 5, 5, DEMONITE), (7, 6, 6, DEMONITE_D),
        (5, 8, 6, DEMONITE_L), (5, 8, 5, DEMONITE), (4, 7, 6, DEMONITE_D),
        (5, 5, 8, DEMONITE_L), (5, 4, 8, DEMONITE), (6, 5, 7, DEMONITE_D),
        (2, 6, 5, DEMONITE_L), (2, 5, 5, DEMONITE), (3, 6, 4, DEMONITE_D),
        (5, 2, 4, DEMONITE_L), (5, 2, 5, DEMONITE),
        (6, 7, 2, DEMONITE_L), (6, 6, 2, DEMONITE),
    ]
    for x, y, z, c in crystals:
        m.set(x, y, z, c)
    save(m, 56)


def demonite_bar():
    """3D ingot of dark lustrous purple demonite metal (14x7x8)."""
    m = Model(14, 7, 8)
    for y in range(6):
        x_min = 1 + y // 2
        x_max = 12 - y // 2
        z_min = 1 + y // 2
        z_max = 6 - y // 2
        for x in range(x_min, x_max + 1):
            for z in range(z_min, z_max + 1):
                if y == 0:
                    c = EBONSTONE_DD
                elif y == 5:
                    if z == z_max or x == x_max:
                        c = DEMONITE_L
                    elif z == z_min or x == x_min:
                        c = DEMONITE_D
                    else:
                        c = DEMONITE
                else:
                    if z == z_max or (x == x_max and z > 2):
                        c = DEMONITE_L if y >= 3 else DEMONITE
                    elif z == z_min or x == x_min:
                        c = DEMONITE_D
                    else:
                        c = DEMONITE if (x + y + z) % 2 == 0 else DEMONITE_D
                m.set(x, y, z, c)
    save(m, 57)


def vilethorn():
    """Twisted purple thorny branch wand (20x20x3)."""
    m = Model(20, 20, 3)
    spine = [
        (1, 1), (2, 2), (3, 2), (4, 3), (4, 4), (5, 5), (6, 5), (7, 6),
        (8, 7), (8, 8), (9, 9), (10, 9), (11, 10), (12, 11), (12, 12),
        (13, 13), (14, 14), (15, 14), (16, 15), (17, 16), (17, 17), (18, 18),
    ]
    for x, y in spine:
        for z in (0, 1):
            c = EBONSTONE_DD if z == 0 else (EBONSTONE_D if (x + y) % 2 == 0 else CORRUPT_D)
            m.set(x, y, z, c)
    thorns = [
        ([(5, 6), (5, 7), (4, 8)], DEMONITE_L),
        ([(8, 6), (9, 5), (10, 5)], DEMONITE),
        ([(12, 13), (11, 14), (11, 15)], DEMONITE_L),
        ([(15, 13), (16, 12), (17, 12)], DEMONITE),
        ([(18, 19), (19, 19)], DEMONITE_L),
        ([(19, 18)], DEMONITE),
        ([(17, 18)], DEMONITE_L),
    ]
    for pts, col in thorns:
        for x, y in pts:
            m.set(x, y, 1, col)
            m.set(x, y, 0, DEMONITE_D)
    save(m, 64)


def worm_food():
    """Rotten chunk of meat dusted with vile powder (11x9x11)."""
    m = Model(11, 9, 11)
    for x in range(11):
        for y in range(8):
            for z in range(11):
                d = ((x - 5.0) / 4.4) ** 2 + ((y - 3.5) / 3.4) ** 2 + ((z - 5.0) / 4.4) ** 2
                if d <= 1.0:
                    c = MUD_D if y < 2 else (MUD if (x + y + z) % 2 == 0 else CDIRT_D)
                    m.set(x, y, z, c)
    for bx, by, bz in [(1, 3, 5), (1, 4, 5), (2, 3, 5), (2, 4, 5), (0, 3, 5)]:
        m.set(bx, by, bz, STONE_L if bx == 0 else STONE)
    for x in range(2, 9):
        for z in range(2, 9):
            if m.get(x, 6, z) is not None:
                m.set(x, 7, z, CORRUPT_L if (x + z) % 2 == 0 else DEMONITE_L)
            elif m.get(x, 5, z) is not None:
                m.set(x, 6, z, CORRUPT_L if (x + z) % 3 == 0 else JUNGLE_D)
    save(m, 70)


def shadow_scale():
    """Dark curved chitinous scale with longitudinal ridges (11x13x5)."""
    m = Model(11, 13, 5)
    for y in range(1, 13):
        half_w = min(4.5, 1.0 + y * 0.35) if y < 10 else (4.5 if y == 10 else (3.5 if y == 11 else 2.5))
        for x in range(11):
            dx = abs(x - 5.0)
            if dx <= half_w:
                z_curv = 2.0 + 1.6 * (1.0 - (dx / 5.0) ** 2)
                z = int(round(z_curv))
                for z_layer in (z - 1, z):
                    if 0 <= z_layer < 5:
                        if dx < 0.8:
                            c = DEMONITE_L if y >= 8 else DEMONITE
                        elif dx > half_w - 1.0:
                            c = EBONSTONE_DD
                        else:
                            c = DEMONITE if (x + y) % 2 == 0 else DEMONITE_D
                        m.set(x, y, z_layer, c)
    for px_val, py in [(2, 12), (5, 12), (8, 12)]:
        m.set(px_val, py, 2, DEMONITE_L)
        m.set(px_val, py, 3, DEMONITE)
    save(m, 86)


def musket():
    """Long-barrel wooden flintlock rifle with iron fittings (22x10x3)."""
    m = Model(22, 10, 3)
    stock = [
        (1, 1), (1, 2), (2, 2), (2, 3), (3, 2), (3, 3),
        (4, 3), (4, 4), (5, 3), (5, 4), (6, 4), (7, 4),
    ]
    for x, y in stock:
        m.set(x, y, 1, BARK if (x + y) % 2 == 0 else BARK_D)
        m.set(x, y, 0, BARK_D)
    for x in range(8, 18):
        m.set(x, 4, 1, BARK)
        m.set(x, 4, 0, BARK_D)
    for x in range(7, 22):
        m.set(x, 5, 1, IRON_L if x % 3 == 0 else IRON)
        m.set(x, 5, 0, IRON_D)
    for bx in (12, 17):
        m.set(bx, 4, 2, IRON_L)
        m.set(bx, 5, 2, IRON)
    m.set(6, 5, 2, IRON_D)
    m.set(7, 6, 2, IRON_L)
    m.set(7, 5, 2, IRON)
    m.set(5, 2, 1, IRON_D)
    m.set(6, 3, 1, IRON)
    m.set(21, 6, 1, IRON_L)
    save(m, 96)


def flintlock_pistol():
    """Compact walnut-and-iron flintlock pistol with flared muzzle and hammer (14x9x3)."""
    m = Model(14, 9, 3)
    # Curved wooden grip & stock
    for y in range(0, 5):
        gx = 1 + y // 2
        m.set(gx, y, 1, BARK if y % 2 == 0 else BARK_D)
        m.set(gx + 1, y, 1, BARK)
        m.set(gx, y, 0, BARK_D)
        m.set(gx + 1, y, 2, BARK_D)
    for x in range(4, 10):
        m.set(x, 4, 1, BARK)
        m.set(x, 4, 0, BARK_D)
    # Iron barrel + lockplate + flint hammer
    for x in range(4, 14):
        m.set(x, 5, 1, IRON_L if x >= 12 else IRON)
        m.set(x, 5, 0, IRON_D)
    m.set(5, 5, 2, GOLD_D)
    m.set(6, 5, 2, GOLD)
    m.set(4, 6, 1, IRON_D)
    m.set(3, 7, 1, IRON_L)  # hammer
    m.set(6, 3, 1, IRON_D)  # trigger guard
    m.set(7, 3, 1, IRON_D)
    m.set(13, 6, 1, IRON_L)  # front sight
    save(m, 95)


def minishark():
    """Terraria shark-shaped gatling gun with fins, toothy jaw, and rotary muzzle (26x13x5)."""
    m = Model(26, 13, 5)
    shark = hexc("#5a6e8a")
    shark_d = hexc("#3f4e66")
    shark_l = hexc("#8aa0bd")
    belly = hexc("#cbd4df")
    eye = hexc("#11141c")
    # Grip & trigger assembly
    m.box(4, 0, 2, 6, 3, 2, IRON_D)
    m.box(1, 3, 1, 3, 8, 3, shark_d)  # tail stock
    m.box(0, 2, 2, 1, 4, 2, shark)  # lower tail fin
    m.box(0, 7, 2, 1, 10, 2, shark_l)  # upper tail fin
    m.set(7, 2, 2, IRON)
    # Rounded grey shark body
    m.box(4, 4, 1, 15, 8, 3, shark)
    m.box(5, 5, 0, 14, 7, 4, shark)
    m.box(5, 4, 1, 14, 4, 3, belly)
    m.box(4, 8, 1, 15, 8, 3, shark_l)
    m.box(5, 5, 0, 14, 7, 0, shark_d)
    # Dorsal fin & pectoral fins
    m.box(7, 9, 2, 11, 9, 2, shark)
    m.box(8, 10, 2, 10, 10, 2, shark_l)
    m.box(8, 11, 2, 9, 11, 2, shark_l)
    m.set(8, 12, 2, shark_l)
    m.box(8, 3, 0, 10, 4, 0, shark_d)  # pectoral fins
    m.box(8, 3, 4, 10, 4, 4, shark_d)
    # Black eye
    m.set(13, 7, 4, eye)
    m.set(13, 7, 0, eye)
    # Open toothy jaw around rotary muzzle
    m.box(15, 7, 1, 17, 8, 3, shark)
    m.box(15, 3, 1, 17, 4, 3, belly)
    for tx in (15, 17):
        m.set(tx, 6, 3, FEATHER)
        m.set(tx, 6, 1, FEATHER)
        m.set(tx, 5, 3, FEATHER)
        m.set(tx, 5, 1, FEATHER)
    # Brass ammo feed box underneath
    m.box(9, 1, 2, 12, 3, 3, GOLD_D)
    m.set(10, 2, 3, GOLD)
    m.set(11, 2, 3, GOLD_L)
    # Multi-barrel brass/steel rotary muzzle
    for x in range(16, 26):
        m.set(x, 6, 2, IRON_L if x % 2 == 0 else IRON)
        m.set(x, 5, 2, IRON_D)
        m.set(x, 6, 1, IRON)
        m.set(x, 6, 3, IRON)
        m.set(x, 5, 1, IRON_D)
        m.set(x, 5, 3, IRON_D)
    m.box(19, 4, 1, 19, 7, 3, GOLD_D)  # brass barrel clamp
    m.box(23, 4, 1, 23, 7, 3, GOLD)
    save(m, 98)


def musket_ball():
    """Small grey metallic spherical round bullet (7x7x7)."""
    m = Model(7, 7, 7)
    for x in range(7):
        for y in range(7):
            for z in range(7):
                d2 = (x - 3.0) ** 2 + (y - 3.0) ** 2 + (z - 3.0) ** 2
                if d2 <= 2.8 ** 2:
                    if x >= 4 and y >= 4 and z >= 4:
                        c = SILVER_L if (x == 4 and y == 5 and z == 4) else IRON_L
                    elif x <= 2 or y <= 2 or z <= 2:
                        c = STONE_DD if (x == 1 and y == 1 and z == 1) else IRON_D
                    else:
                        c = IRON
                    m.set(x, y, z, c)
    save(m, 97)


def shadow_greaves():
    """Dark purple armored greaves with spiky ridges (12x12x8)."""
    m = Model(12, 12, 8)
    m.box(4, 2, 3, 7, 3, 4, EBONSTONE_DD)
    for x_base in (1, 7):
        m.box(x_base, 0, 1, x_base + 3, 0, 6, EBONSTONE_DD)
        m.box(x_base, 1, 1, x_base + 3, 3, 5, DEMONITE_D)
        m.set(x_base + 1, 1, 6, DEMONITE_L)
        m.set(x_base + 2, 1, 6, DEMONITE)
        m.box(x_base, 4, 2, x_base + 3, 8, 5, DEMONITE)
        for y in range(4, 9):
            m.set(x_base + 1, y, 5, DEMONITE_L)
        m.box(x_base, 9, 3, x_base + 3, 9, 5, DEMONITE_D)
        m.set(x_base + 1, 10, 5, DEMONITE)
        m.set(x_base + 2, 10, 5, DEMONITE_L)
        m.set(x_base + 1, 11, 5, DEMONITE_L)
    save(m, 100)


def shadow_scalemail():
    """Dark purple ribbed scale chestplate with spiky pauldrons (14x13x8)."""
    m = Model(14, 13, 8)
    m.box(3, 1, 2, 10, 9, 5, DEMONITE_D)
    for y in range(1, 10):
        for x in range(4, 10):
            c = DEMONITE_L if (x + y) % 2 == 0 else DEMONITE
            m.set(x, y, 6, c)
    for x_p, spike_x in [(1, 0), (10, 13)]:
        m.box(x_p, 8, 1, x_p + 2, 11, 6, DEMONITE)
        m.set(spike_x, 11, 4, DEMONITE_D)
        m.set(spike_x, 12, 4, DEMONITE_L)
        m.set(spike_x, 12, 3, DEMONITE)
    m.box(5, 10, 3, 8, 10, 4, DEMONITE_L)
    save(m, 101)


def shadow_helmet():
    """Horned spiky dark purple helmet (12x13x11)."""
    m = Model(12, 13, 11)
    for x in range(2, 10):
        for y in range(2, 9):
            for z in range(2, 10):
                d = ((x - 5.5) / 3.8) ** 2 + ((y - 5.0) / 3.8) ** 2 + ((z - 5.5) / 3.8) ** 2
                if d <= 1.0:
                    m.set(x, y, z, DEMONITE if (x + y + z) % 2 == 0 else DEMONITE_D)
    for x in range(3, 9):
        m.set(x, 4, 9, CORRUPT_L if x in (4, 7) else DEMONITE_L)
        m.set(x, 3, 9, EBONSTONE_DD)
        m.set(x, 5, 9, EBONSTONE_DD)
    for y in range(7, 11):
        for z in range(3, 8):
            m.set(5, y, z, DEMONITE_L)
            m.set(6, y, z, DEMONITE)
    for hx, dx in [(1, -1), (10, 1)]:
        horn = [
            (hx, 6, 6), (hx, 7, 5), (hx + dx, 8, 4),
            (hx + dx, 9, 3), (hx + dx, 10, 2), (hx + dx, 11, 1),
            (hx + dx, 12, 1),
        ]
        for x, y, z in horn:
            m.set(x, y, z, DEMONITE_L if y >= 11 else DEMONITE)
            m.set(x, y - 1, z, DEMONITE_D)
    save(m, 102)


def the_breaker():
    """Heavy brutal demonic war hammer on a dark shaft (20x20x4)."""
    m = Model(20, 20, 4)
    handle(m, 11)
    for x in range(10, 14):
        for y in range(10, 14):
            for z in (0, 1, 2, 3):
                m.set(x, y, z, DEMONITE_D)
    m.box(11, 13, 0, 17, 18, 3, DEMONITE)
    for x in range(12, 17):
        for y in range(14, 18):
            m.set(x, y, 3, DEMONITE_L if (x + y) % 2 == 0 else DEMONITE)
            m.set(x, y, 0, EBONSTONE_DD)
    for tooth in [(18, 14), (18, 16), (18, 18), (17, 19), (15, 19)]:
        for z in (1, 2):
            m.set(tooth[0], tooth[1], z, DEMONITE_L)
    for beak in [(10, 17), (9, 18), (8, 18)]:
        for z in (1, 2):
            m.set(beak[0], beak[1], z, DEMONITE_L if beak[0] == 8 else DEMONITE_D)
    save(m, 104)


def band_of_starpower():
    """Glowing celestial blue band set with a yellow star (10x10x6)."""
    m = Model(10, 10, 6)
    for x in range(10):
        for y in range(10):
            d2 = (x - 4.5) ** 2 + (y - 4.5) ** 2
            if 2.2 ** 2 <= d2 <= 4.6 ** 2:
                for z in range(1, 5):
                    c = ICE_L if (x + y > 9) else (ICE_D if (x + y < 7) else ICE)
                    m.set(x, y, z, c)
    for x in (3, 6):
        for z in (1, 4):
            m.set(x, 8, z, ICE_L)
            m.set(x, 9, z, ICE_D)
    m.box(4, 8, 2, 5, 9, 3, GOLD)
    m.set(4, 9, 3, FLAME_W)
    m.set(5, 9, 3, GOLD_L)
    m.set(4, 8, 2, GOLD_D)
    m.set(3, 9, 2, GOLD_L)
    m.set(6, 9, 2, GOLD_L)
    m.set(4, 9, 4, GOLD_L)
    save(m, 111)


def shadow_orb_item():
    """Miniature swirling dark purple orb (9x9x9)."""
    m = Model(9, 9, 9)
    for x in range(9):
        for y in range(9):
            for z in range(9):
                d2 = (x - 4.0) ** 2 + (y - 4.0) ** 2 + (z - 4.0) ** 2
                if d2 <= 3.8 ** 2:
                    swirl = (x * 2 + y * 3 + z * 5) % 7
                    if swirl in (0, 1):
                        c = CORRUPT_L if d2 >= 2.5 ** 2 else DEMONITE_L
                    elif swirl in (2, 3):
                        c = DEMONITE
                    elif swirl == 4:
                        c = DEMONITE_D
                    else:
                        c = EBONSTONE_DD
                    m.set(x, y, z, c)
    save(m, 115)


def ball_o_hurt():
    """Spiked iron flail ball with heavy chain link (12x14x12)."""
    m = Model(12, 14, 12)
    cx, cy, cz = 5.5, 5.0, 5.5
    for x in range(2, 10):
        for y in range(2, 9):
            for z in range(2, 10):
                d2 = (x - cx) ** 2 + (y - cy) ** 2 + (z - cz) ** 2
                if d2 <= 3.4 ** 2:
                    c = IRON if (x + y + z) % 2 == 0 else IRON_D
                    m.set(x, y, z, c)
    for y in range(0, 3):
        m.set(5, y, 5, IRON_L if y == 0 else IRON)
        m.set(6, y, 5, IRON)
        m.set(5, y, 6, IRON)
        m.set(6, y, 6, IRON_D)
    for x in range(0, 3):
        m.set(x, 5, 5, IRON_L if x == 0 else IRON)
        m.set(x, 5, 6, IRON_D)
    for x in range(9, 12):
        m.set(x, 5, 5, IRON_L if x == 11 else IRON)
        m.set(x, 5, 6, IRON_D)
    for z in range(0, 3):
        m.set(5, 5, z, IRON_L if z == 0 else IRON)
        m.set(6, 5, z, IRON_D)
    for z in range(9, 12):
        m.set(5, 5, z, IRON_L if z == 11 else IRON)
        m.set(6, 5, z, IRON_D)
    for cx_sign in (-1, 1):
        for cz_sign in (-1, 1):
            for cy_sign in (-1, 1):
                px_val = 5 if cx_sign == -1 else 6
                pz_val = 5 if cz_sign == -1 else 6
                py_val = 4 if cy_sign == -1 else 6
                for step in (1, 2):
                    sx = px_val + cx_sign * step
                    sy = py_val + cy_sign * step
                    sz = pz_val + cz_sign * step
                    if 0 <= sx < 12 and 0 <= sy < 14 and 0 <= sz < 12:
                        m.set(sx, sy, sz, IRON_L if step == 2 else IRON)
    m.box(5, 8, 5, 6, 9, 6, IRON_D)
    m.set(5, 10, 5, IRON_L)
    m.set(6, 10, 6, IRON)
    m.set(5, 11, 5, IRON)
    m.set(6, 11, 6, IRON_D)
    m.set(5, 12, 6, IRON_L)
    m.set(6, 12, 5, IRON)
    m.set(5, 13, 6, IRON)
    m.set(6, 13, 5, IRON_D)
    save(m, 162)


def wooden_hammer():
    """Traditional wooden mallet (19x19x4)."""
    m = Model(19, 19, 4)
    handle(m, 13)
    for i in range(-5, 6):
        hx = 13 + i
        hy = 13 - i
        for dx in (0, 1):
            for dy in (0, 1):
                for z in (0, 1, 2, 3):
                    x = hx + dx
                    y = hy + dy
                    if abs(i) == 5:
                        c = RING_D if (dx == 0 and z in (1, 2)) else RING
                    elif abs(i) == 4:
                        c = BARK_D
                    else:
                        c = WOOD_L if z == 3 else (WOOD_D if z == 0 else WOOD)
                    m.set(x, y, z, c)
    save(m, 196)


# M8 Dungeon & Loot Colours
COBALT_D = hexc("#103b82")
COBALT = hexc("#1d5ec9")
COBALT_L = hexc("#488af5")
COBALT_W = hexc("#a3c7fc")

BONE_L = hexc("#fbf9f1")
BONE = hexc("#eae4d3")
BONE_D = hexc("#c4bc9f")
BONE_DD = hexc("#8e8568")

AQUA_D = hexc("#0077b6")
AQUA = hexc("#00b4d8")
AQUA_L = hexc("#90e0ef")
AQUA_W = hexc("#caf0f8")


def magic_missile():
    """Slender wand with golden socket and glowing celestial blue orb (19x19x3)."""
    m = Model(19, 19, 3)
    # Pommel
    px(m, 1, 1, GOLD_D)
    px(m, 1, 2, GOLD)
    px(m, 2, 1, GOLD)
    px(m, 2, 2, GOLD_L)
    # Shaft
    for i in range(3, 10):
        c = GOLD if (i % 3 == 0) else COBALT_D
        for z in (0, 1):
            m.set(i, i, z, c)
            m.set(i + 1, i, z, HANDLE_D if i % 3 != 0 else GOLD_D)
            m.set(i, i + 1, z, COBALT if i % 3 != 0 else GOLD_L)
    # Golden socket / cradle
    for i in range(10, 12):
        for z in (0, 1, 2):
            m.set(i, i, z, GOLD)
            m.set(i + 1, i, z, GOLD_D)
            m.set(i, i + 1, z, GOLD_L)
    m.set(9, 12, 1, GOLD_L)
    m.set(10, 13, 1, GOLD_L)
    m.set(12, 9, 1, GOLD_D)
    m.set(13, 10, 1, GOLD_D)
    # Glowing orb
    for x in range(11, 17):
        for y in range(11, 17):
            for z in range(3):
                d2 = (x - 14.0) ** 2 + (y - 14.0) ** 2 + (z - 1.0) ** 2
                if d2 <= 2.6 ** 2:
                    if d2 <= 1.2 ** 2:
                        c = FLAME_W if (x + y + z) % 2 == 0 else GOLD_L
                    elif (x == 14 and y == 14) or d2 >= 2.2 ** 2:
                        c = COBALT_L if (x + y > 28) else COBALT
                    else:
                        c = ICE_L if z == 2 else ICE
                    m.set(x, y, z, c)
    # Gold orb trim
    m.set(14, 16, 1, GOLD_L)
    m.set(16, 14, 1, GOLD_L)
    m.set(12, 14, 1, GOLD_D)
    m.set(14, 12, 1, GOLD_D)
    save(m, 113)


def bone():
    """Carved ivory bone with rounded double condyles on both ends (16x16x4)."""
    m = Model(16, 16, 4)
    # Diagonal shaft from (3, 3) to (12, 12)
    for i in range(3, 13):
        for z in (1, 2):
            c = BONE_L if z == 2 else BONE
            m.set(i, i, z, c)
            m.set(i + 1, i, z, BONE_D)
            m.set(i, i + 1, z, BONE)
    # Lower condyles
    for (cx, cy) in [(2.0, 3.5), (3.5, 2.0), (1.5, 1.5)]:
        for x in range(math.floor(cx - 1.5), math.ceil(cx + 1.5) + 1):
            for y in range(math.floor(cy - 1.5), math.ceil(cy + 1.5) + 1):
                for z in range(4):
                    d2 = (x - cx) ** 2 + (y - cy) ** 2 + (z - 1.5) ** 2
                    if d2 <= 1.6 ** 2:
                        c = BONE_L if (x + y > 6 or z >= 2) else (BONE_DD if z == 0 else BONE_D)
                        m.set(x, y, z, c)
    # Upper condyles
    for (cx, cy) in [(11.0, 13.5), (13.5, 11.0), (13.5, 13.5)]:
        for x in range(math.floor(cx - 1.5), math.ceil(cx + 1.5) + 1):
            for y in range(math.floor(cy - 1.5), math.ceil(cy + 1.5) + 1):
                for z in range(4):
                    d2 = (x - cx) ** 2 + (y - cy) ** 2 + (z - 1.5) ** 2
                    if d2 <= 1.6 ** 2:
                        c = BONE_L if (x + y >= 25 or z >= 2) else (BONE_DD if z == 0 else BONE_D)
                        m.set(x, y, z, c)
    save(m, 154)


def muramasa():
    """Terraria Muramasa (26x26x4): sleek curved cobalt-blue katana with glowing cyan-white edge."""
    m = Model(26, 26, 4)
    # Pommel (kashira)
    px_full(m, 0, 0, GOLD)
    px(m, 1, 0, GOLD_D)
    px(m, 0, 1, GOLD_D)
    # Dark blue-wrapped long tsuka hilt (1..6)
    for i in range(1, 7):
        px_full(m, i, i, COBALT_D if i % 2 == 0 else hexc("#0a1128"))
        px(m, i + 1, i, hexc("#0a1128"))
        px(m, i, i + 1, COBALT_D)
    # Golden circular tsuba guard at (7, 7)
    for dx in (-2, -1, 0, 1, 2):
        for dy in (-2, -1, 0, 1, 2):
            if abs(dx) + abs(dy) <= 2:
                px_full(m, 7 + dx, 7 + dy, GOLD if abs(dx) + abs(dy) < 2 else GOLD_L)
    # Habaki collar
    px_full(m, 8, 8, GOLD)
    # Sleek curving single-edged blade (9..24)
    for i in range(9, 25):
        curve = round(1.4 * math.sin(math.pi * (i - 8) / 17.0))
        bx = i - curve
        by = i + curve
        if 0 <= bx < 26 and 0 <= by < 26:
            px_full(m, bx, by, COBALT)
            if bx + 1 < 26 and by - 1 >= 0:
                px(m, bx + 1, by - 1, COBALT_D)
            if bx - 1 >= 0 and by + 1 < 26:
                px(m, bx - 1, by + 1, ICE_L)  # glowing cyan-white razor edge
            if bx - 1 >= 0:
                px(m, bx - 1, by, COBALT_L)
    # Kissaki tip
    px_full(m, 25, 25, ICE_L)
    px(m, 24, 25, ICE_L)
    px(m, 25, 24, COBALT_L)
    save(m, 155)


def cobalt_shield():
    """Curved heater shield with cobalt blue field and ornate gold rim (14x16x4)."""
    m = Model(14, 16, 4)
    for y in range(16):
        if y >= 8:
            half_w = 6.0
        else:
            half_w = 1.0 + y * (5.0 / 8.0)
        for x in range(14):
            dx = abs(x - 6.5)
            if dx <= half_w:
                z_front = 2 if dx >= 4.5 else 3
                z_back = 0 if dx >= 4.5 else 1
                is_rim = (dx >= half_w - 1.1) or (y == 0) or (y >= 14)
                for z in range(z_back, z_front + 1):
                    if is_rim:
                        c = GOLD_L if (y >= 14 or dx >= 5.0) else (GOLD if z >= 2 else GOLD_D)
                    else:
                        is_cross = (5 <= x <= 7 and 6 <= y <= 12) or (4 <= x <= 8 and 8 <= y <= 10)
                        if is_cross and z == z_front:
                            c = GOLD_L if (x + y) % 2 == 0 else GOLD
                        else:
                            c = COBALT_L if z == z_front else (COBALT if z >= 2 else COBALT_D)
                    m.set(x, y, z, c)
    # Leather back straps
    for y in (7, 8):
        for x in (5, 6, 7):
            m.set(x, y, 0, HANDLE_D)
    save(m, 156)


def aqua_scepter():
    """Slender silver wand discharging a spiraling aqua stream (18x18x3)."""
    m = Model(18, 18, 3)
    # Handle
    for i in range(1, 10):
        for z in (0, 1):
            m.set(i, i, z, SILVER if i % 4 != 0 else GOLD)
            m.set(i + 1, i, z, SILVER_D if i % 4 != 0 else GOLD_D)
            m.set(i, i + 1, z, SILVER_L if i % 4 != 0 else GOLD_L)
    # Golden nozzle
    for x in (9, 10, 11):
        for y in (9, 10, 11):
            for z in (0, 1, 2):
                if x + y >= 19:
                    m.set(x, y, z, GOLD if (x + y) % 2 == 0 else GOLD_L)
    # Aqua crystal stream
    for i in range(11, 17):
        for z in (0, 1, 2):
            c = AQUA_W if z == 2 and i >= 14 else (AQUA_L if z == 2 else AQUA)
            m.set(i, i, z, c)
            m.set(i + 1, i, z, AQUA_D)
            m.set(i, i + 1, z, AQUA_L)
    splashes = [
        (13, 15, 1, AQUA_L), (12, 16, 1, AQUA_W), (13, 16, 2, AQUA_L),
        (15, 13, 1, AQUA_D), (16, 12, 1, AQUA), (16, 13, 0, AQUA_D),
        (15, 17, 1, AQUA_W), (16, 17, 2, AQUA_L), (17, 16, 1, AQUA_W),
        (17, 17, 1, AQUA_W), (16, 16, 2, AQUA_W), (17, 15, 0, AQUA_L),
    ]
    for sx, sy, sz, sc in splashes:
        m.set(sx, sy, sz, sc)
    save(m, 157)


def blue_moon():
    """Heavy flail ball of cobalt metal studded with glowing icy spikes (14x16x14)."""
    m = Model(14, 16, 14)
    cx, cy, cz = 6.5, 6.0, 6.5
    for x in range(14):
        for y in range(12):
            for z in range(14):
                d2 = (x - cx) ** 2 + (y - cy) ** 2 + (z - cz) ** 2
                if d2 <= 3.8 ** 2:
                    if d2 <= 2.2 ** 2:
                        c = COBALT_D if y < 5 else COBALT
                    else:
                        c = COBALT_L if (x + y + z) % 3 == 0 else (COBALT if y >= 5 else COBALT_D)
                    m.set(x, y, z, c)
    # Cardinal spikes
    for x, step in [(2, 1), (1, 2), (0, 3)]:
        m.set(x, 6, 6, ICE_L if step == 3 else ICE)
        m.set(x, 6, 7, ICE_L if step == 3 else ICE_D)
    for x, step in [(11, 1), (12, 2), (13, 3)]:
        m.set(x, 6, 6, ICE_L if step == 3 else ICE)
        m.set(x, 6, 7, ICE_L if step == 3 else ICE_D)
    for z, step in [(2, 1), (1, 2), (0, 3)]:
        m.set(6, 6, z, ICE_L if step == 3 else ICE)
        m.set(7, 6, z, ICE_L if step == 3 else ICE_D)
    for z, step in [(11, 1), (12, 2), (13, 3)]:
        m.set(6, 6, z, ICE_L if step == 3 else ICE)
        m.set(7, 6, z, ICE_L if step == 3 else ICE_D)
    for y, step in [(2, 1), (1, 2), (0, 3)]:
        m.set(6, y, 6, ICE_L if step == 3 else ICE)
        m.set(7, y, 7, ICE_L if step == 3 else ICE_D)
    # Diagonal corner spikes
    for s_x in (-1, 1):
        for s_z in (-1, 1):
            for s_y in (-1, 1):
                px_val = 5 if s_x == -1 else 8
                pz_val = 5 if s_z == -1 else 8
                py_val = 4 if s_y == -1 else 8
                for step in (1, 2):
                    sx = px_val + s_x * step
                    sy = py_val + s_y * step
                    sz = pz_val + s_z * step
                    if 0 <= sx < 14 and 0 <= sy < 16 and 0 <= sz < 14:
                        m.set(sx, sy, sz, ICE_L if step == 2 else ICE)
    # Top chain links
    m.box(6, 9, 6, 7, 10, 7, COBALT_D)
    for y in range(11, 16):
        if y % 2 == 1:
            m.set(6, y, 6, SILVER_L)
            m.set(7, y, 7, SILVER_D)
            m.set(6, y, 7, SILVER)
        else:
            m.set(7, y, 6, SILVER_L)
            m.set(6, y, 7, SILVER_D)
            m.set(7, y, 7, SILVER)
    save(m, 163)


def handgun():
    """Semi-automatic steel pistol with walnut grips and barrel bore (15x11x3)."""
    m = Model(15, 11, 3)
    # Grip (angled handle)
    for y in range(0, 6):
        gx = 2 + y // 2
        for z in (0, 1, 2):
            if z in (0, 2):
                c = BARK if (gx + y) % 2 == 0 else BARK_D
            else:
                c = IRON_D if y == 0 else IRON
            m.set(gx, y, z, c)
            m.set(gx + 1, y, z, c)
            if y <= 3:
                m.set(gx + 2, y, z, BARK_D if z in (0, 2) else IRON_D)
    # Trigger guard & trigger
    m.set(5, 5, 1, IRON_D)
    m.set(6, 4, 1, IRON)
    m.set(7, 4, 1, IRON_D)
    m.set(8, 5, 1, IRON)
    m.set(6, 5, 1, STONE_DD)  # trigger
    # Receiver / frame
    for x in range(3, 11):
        for z in (0, 1, 2):
            m.set(x, 6, z, IRON if (x + z) % 2 == 0 else IRON_D)
    # Slide
    for x in range(2, 14):
        for y in (7, 8):
            for z in (0, 1, 2):
                if y == 8:
                    c = IRON_L if z == 1 else IRON
                else:
                    c = IRON_L if x == 13 else (IRON_D if z == 0 else IRON)
                m.set(x, y, z, c)
    # Muzzle bore
    m.set(13, 7, 1, STONE_DD)
    # Ejection port
    m.set(6, 8, 2, STONE_DD)
    m.set(7, 8, 2, STONE_DD)
    # Rear sights / hammer
    m.set(2, 9, 0, IRON_D)
    m.set(2, 9, 2, IRON_D)
    m.set(12, 9, 1, IRON_L)  # front sight
    save(m, 164)


def golden_key():
    """Ornate golden skeleton key with clover bow and ward teeth (16x16x3)."""
    m = Model(16, 16, 3)
    # Bow (clover ring at bottom-left)
    for x in range(1, 7):
        for y in range(1, 7):
            d2 = (x - 3.5) ** 2 + (y - 3.5) ** 2
            if 1.2 ** 2 <= d2 <= 2.8 ** 2:
                for z in (0, 1, 2):
                    c = GOLD_L if (x + y > 7 or z == 2) else (GOLD_D if z == 0 else GOLD)
                    m.set(x, y, z, c)
    # Center clover cross in bow
    m.set(3, 3, 1, GOLD)
    m.set(4, 4, 1, GOLD_L)
    # Diagonal stem
    for i in range(5, 13):
        for z in (0, 1, 2):
            c = GOLD_L if z == 2 else (GOLD_D if z == 0 else GOLD)
            m.set(i, i, z, c)
            m.set(i + 1, i, z, GOLD_D)
            m.set(i, i + 1, z, GOLD_L)
    # Shaft collar ring
    m.set(11, 13, 1, GOLD_L)
    m.set(13, 11, 1, GOLD_D)
    # Bit (wards)
    bit = [
        (11, 14), (10, 15), (11, 15), (12, 14), (12, 15), (13, 14),
        (13, 13), (14, 14), (15, 13), (15, 12),
    ]
    for bx, by in bit:
        for z in (0, 1, 2):
            c = GOLD_L if z == 2 else (GOLD_D if z == 0 else GOLD)
            m.set(bx, by, z, c)
    save(m, 327)


def shadow_key():
    """Dark demonic key with purple crystalline eye and serrated bit (16x16x3)."""
    m = Model(16, 16, 3)
    # Demonic bow with horned lobes
    for x in range(1, 7):
        for y in range(1, 7):
            d2 = (x - 3.5) ** 2 + (y - 3.5) ** 2
            if 1.2 ** 2 <= d2 <= 2.8 ** 2:
                for z in (0, 1, 2):
                    c = DEMONITE_L if (x + y > 7 or z == 2) else (EBONSTONE_DD if z == 0 else DEMONITE_D)
                    m.set(x, y, z, c)
    # Glowing corrupt center eye in bow
    m.set(3, 3, 1, CORRUPT_L)
    m.set(4, 4, 1, DEMONITE_L)
    m.set(3, 4, 1, DEMONITE_L)
    m.set(4, 3, 1, CORRUPT_L)
    # Demonic barbed stem
    for i in range(5, 13):
        for z in (0, 1, 2):
            c = DEMONITE_L if z == 2 else (EBONSTONE_DD if z == 0 else DEMONITE)
            m.set(i, i, z, c)
            m.set(i + 1, i, z, DEMONITE_D)
            m.set(i, i + 1, z, DEMONITE_L)
        if i in (7, 10):
            m.set(i - 1, i + 1, 1, DEMONITE_L)
            m.set(i + 1, i - 1, 1, DEMONITE_D)
    # Serrated demonic bit
    bit = [
        (11, 14), (10, 15), (9, 15), (10, 14), (12, 14), (12, 15),
        (13, 15), (13, 13), (14, 14), (15, 13), (15, 14), (15, 15),
    ]
    for bx, by in bit:
        for z in (0, 1, 2):
            c = CORRUPT_L if (z == 2 and (bx + by) % 2 == 0) else (DEMONITE if z == 1 else EBONSTONE_DD)
            m.set(bx, by, z, c)
    save(m, 329)


def valor():
    """Cobalt blue and gold trimmed dungeon yoyo with central axle (11x11x7)."""
    m = Model(11, 11, 7)
    # Central axle connecting both halves
    m.box(4, 4, 1, 6, 6, 5, SILVER)
    m.box(5, 5, 2, 5, 5, 4, IRON)
    # Two yoyo discs: left (z 0..2) and right (z 4..6)
    for x in range(11):
        for y in range(11):
            d2 = (x - 5.0) ** 2 + (y - 5.0) ** 2
            if d2 <= 4.6 ** 2:
                is_rim = d2 >= 3.4 ** 2
                is_center = ((x in (4, 5, 6) and y == 5) or (y in (4, 5, 6) and x == 5))
                for z in (0, 1, 2, 4, 5, 6):
                    if is_rim:
                        c = GOLD_L if (x + y > 10 or z in (0, 6)) else (GOLD_D if (x + y < 8) else GOLD)
                    elif is_center and z in (0, 6):
                        c = FLAME_W if (x == 5 and y == 5) else GOLD_L
                    else:
                        c = COBALT_L if z in (0, 6) else (COBALT if (x + y) % 2 == 0 else COBALT_D)
                    m.set(x, y, z, c)
    save(m, 3317)


def ash_block():
    """Mottled grey-purple ash cube with lighter speckles (11x11x11)."""
    m = Model(11, 11, 11)
    ASH_DARK = hexc("#332f3b")
    ASH_BASE = hexc("#45414d")
    ASH_MID = hexc("#575261")
    ASH_LIGHT = hexc("#6d677a")
    ASH_SPECK = hexc("#8f899e")
    for x in range(11):
        for y in range(11):
            for z in range(11):
                # Chipping 8 extreme corners for a tumbled rock/ash block shape
                if (x in (0, 10) and y in (0, 10) and z in (0, 10)):
                    continue
                h = (x * 7 + y * 13 + z * 19 + (x ^ y ^ z) * 5) % 17
                if h in (0, 1):
                    c = ASH_SPECK
                elif h in (2, 3, 4):
                    c = ASH_LIGHT
                elif h in (5, 6, 7, 8):
                    c = ASH_MID
                elif h in (9, 10, 11, 12):
                    c = ASH_BASE
                else:
                    c = ASH_DARK
                m.set(x, y, z, c)
    save(m, 172)


def obsidian():
    """Faceted black-purple obsidian chunk with glassy highlights (11x11x11)."""
    m = Model(11, 11, 11)
    OBS_BLACK = hexc("#15111d")
    OBS_DARK = hexc("#211b2b")
    OBS_MID = hexc("#322942")
    OBS_PURPLE = hexc("#483b5e")
    OBS_SHINE = hexc("#736391")
    OBS_SPEC = hexc("#b5a7d6")
    for x in range(11):
        for y in range(11):
            for z in range(11):
                dx, dy, dz = x - 5.0, y - 5.0, z - 5.0
                if dx * dx + dy * dy + dz * dz > 5.2 ** 2:
                    continue
                if dx + dy + dz > 7.0:
                    continue
                if -dx + dy - dz > 6.8:
                    continue
                if dx - dy + dz > 6.8:
                    continue
                if -dx - dy - dz > 6.8:
                    continue
                c = OBS_BLACK if (x + y + z) % 3 == 0 else (OBS_DARK if (x ^ z) % 2 == 0 else OBS_MID)
                if abs(dx + dy + dz - 6.0) < 1.0 or abs(dx - dy + dz - 5.8) < 1.0:
                    c = OBS_PURPLE
                if x in (5, 6, 7) and y in (7, 8, 9) and z in (6, 7):
                    c = OBS_SPEC if (x == 6 and y == 8) else OBS_SHINE
                m.set(x, y, z, c)
    save(m, 173)


def hellstone():
    """Dark basalt cube with glowing orange-red molten vein streaks (11x11x11)."""
    m = Model(11, 11, 11)
    BASALT_D = hexc("#231e24")
    BASALT = hexc("#332b34")
    BASALT_L = hexc("#453b47")
    LAVA_DEEP = hexc("#b82e00")
    LAVA_RED = hexc("#e64a0a")
    LAVA_ORANGE = hexc("#ff8c1a")
    LAVA_HOT = hexc("#ffc740")
    for x in range(11):
        for y in range(11):
            for z in range(11):
                if (x in (0, 10) and y in (0, 10) and z in (0, 10)):
                    continue
                d1 = abs(y - (x + math.sin(z * 0.8) * 2.0))
                d2 = abs(y - ((10 - z) + math.cos(x * 0.9) * 2.0))
                is_vein1 = d1 < 1.2
                is_vein2 = d2 < 1.2
                is_outer = (x in (0, 10) or y in (0, 10) or z in (0, 10))
                if is_vein1 and is_vein2:
                    c = LAVA_HOT if is_outer else LAVA_ORANGE
                elif is_vein1:
                    c = LAVA_ORANGE if (x + z) % 2 == 0 else LAVA_RED
                elif is_vein2:
                    c = LAVA_RED if (y + z) % 2 == 0 else LAVA_DEEP
                else:
                    c = BASALT_L if (x + y + z) % 5 == 0 else (BASALT if (x * 3 + y + z) % 2 == 0 else BASALT_D)
                m.set(x, y, z, c)
    save(m, 174)


def obsidian_brick():
    """Two-course obsidian brick block with mortar grooves (12x12x12)."""
    m = Model(12, 12, 12)
    BRICK_DARK = hexc("#1a1522")
    BRICK_MID = hexc("#282033")
    BRICK_LGT = hexc("#3d324c")
    BRICK_HI = hexc("#544569")
    MORTAR = hexc("#0e0b12")
    for x in range(12):
        for y in range(12):
            for z in range(12):
                is_outer_x = (x == 0 or x == 11)
                is_outer_z = (z == 0 or z == 11)
                is_outer_side = is_outer_x or is_outer_z

                is_h_groove = (y == 5)
                if y <= 4:
                    is_v_groove = (x == 6 and is_outer_z) or (z == 6 and is_outer_x)
                elif 6 <= y <= 10:
                    is_v_groove = (x in (3, 9) and is_outer_z) or (z in (3, 9) and is_outer_x)
                else:
                    is_v_groove = False

                if y == 11:
                    is_top_groove = (x in (3, 9) or z in (3, 9))
                else:
                    is_top_groove = False

                if is_outer_side and (is_h_groove or is_v_groove):
                    continue
                if y == 11 and is_top_groove:
                    continue

                is_mortar_depth = is_h_groove
                if (x == 6 and 1 <= y <= 4 and (z in (1, 10))) or (z == 6 and 1 <= y <= 4 and (x in (1, 10))):
                    is_mortar_depth = True
                if (x in (3, 9) and 6 <= y <= 10 and (z in (1, 10))) or (z in (3, 9) and 6 <= y <= 10 and (x in (1, 10))):
                    is_mortar_depth = True

                if is_mortar_depth:
                    c = MORTAR
                else:
                    h = (x * 3 + y * 7 + z * 11) % 7
                    if (y in (4, 10) or y == 11):
                        c = BRICK_HI if h == 0 else BRICK_LGT
                    else:
                        c = BRICK_LGT if h in (1, 2) else (BRICK_MID if h in (3, 4, 5) else BRICK_DARK)
                m.set(x, y, z, c)
    save(m, 192)


def obsidian_skull():
    """Dark obsidian carved horned skull (13x14x11)."""
    m = Model(13, 14, 11)
    SK_VOID = hexc("#0c0912")
    SK_BLACK = hexc("#181222")
    SK_DARK = hexc("#281f36")
    SK_MID = hexc("#3d3052")
    SK_LIGHT = hexc("#594775")
    SK_HI = hexc("#7d66a1")

    for x in range(2, 11):
        for y in range(4, 12):
            for z in range(2, 9):
                dx = (x - 6.0) / 3.8
                dy = (y - 7.5) / 3.8
                dz = (z - 5.0) / 3.2
                if dx * dx + dy * dy + dz * dz <= 1.0:
                    c = SK_LIGHT if (y >= 10 and z >= 5) else (SK_MID if (x + y + z) % 2 == 0 else SK_DARK)
                    m.set(x, y, z, c)

    for x in range(4, 9):
        for y in range(1, 4):
            for z in range(4, 8):
                c = SK_DARK if y == 1 else (SK_MID if (x + z) % 2 == 0 else SK_BLACK)
                m.set(x, y, z, c)

    for x in range(4, 9):
        if x in (4, 6, 8):
            m.set(x, 2, 7, SK_LIGHT)
            m.set(x, 3, 7, SK_LIGHT)
        else:
            m.set(x, 2, 7, SK_VOID)
            m.set(x, 3, 7, SK_VOID)

    for y in (6, 7):
        for z in (7, 8):
            m.set(4, y, z, SK_VOID)
            m.set(8, y, z, SK_VOID)
    m.set(6, 5, 7, SK_VOID)

    for x in range(3, 10):
        if m.get(x, 8, 7) is not None:
            m.set(x, 8, 7, SK_HI if x in (4, 8) else SK_LIGHT)

    left_horn = [
        (2, 8, 4), (1, 9, 3), (1, 10, 3), (0, 11, 2), (0, 12, 1), (0, 13, 0)
    ]
    right_horn = [
        (10, 8, 4), (11, 9, 3), (11, 10, 3), (12, 11, 2), (12, 12, 1), (12, 13, 0)
    ]
    for horn in (left_horn, right_horn):
        for i, (hx, hy, hz) in enumerate(horn):
            col = SK_HI if i == len(horn) - 1 else (SK_LIGHT if i >= 3 else SK_MID)
            m.set(hx, hy, hz, col)
            if i <= 2:
                m.set(hx, hy, hz + 1, SK_DARK)

    save(m, 193)


def hellstone_brick():
    """Dark basalt brick with glowing fiery ember mortar lines (12x12x12)."""
    m = Model(12, 12, 12)
    BRICK_D = hexc("#1e1a20")
    BRICK_M = hexc("#2d2730")
    BRICK_L = hexc("#3d3542")
    LAVA_RED = hexc("#c4300a")
    LAVA_ORANGE = hexc("#f56e10")
    LAVA_HOT = hexc("#ffc430")

    for x in range(12):
        for y in range(12):
            for z in range(12):
                is_outer_x = (x == 0 or x == 11)
                is_outer_z = (z == 0 or z == 11)
                is_outer_side = is_outer_x or is_outer_z

                is_h_groove = (y == 5)
                if y <= 4:
                    is_v_groove = (x == 6 and is_outer_z) or (z == 6 and is_outer_x)
                elif 6 <= y <= 10:
                    is_v_groove = (x in (3, 9) and is_outer_z) or (z in (3, 9) and is_outer_x)
                else:
                    is_v_groove = False

                if y == 11:
                    is_top_groove = (x in (3, 9) or z in (3, 9))
                else:
                    is_top_groove = False

                if is_outer_side and (is_h_groove or is_v_groove):
                    continue
                if y == 11 and is_top_groove:
                    continue

                is_mortar_depth = is_h_groove
                if (x == 6 and 1 <= y <= 4 and (z in (1, 10))) or (z == 6 and 1 <= y <= 4 and (x in (1, 10))):
                    is_mortar_depth = True
                if (x in (3, 9) and 6 <= y <= 10 and (z in (1, 10))) or (z in (3, 9) and 6 <= y <= 10 and (x in (1, 10))):
                    is_mortar_depth = True

                if is_mortar_depth:
                    h = (x * 5 + y * 3 + z * 7) % 5
                    if h == 0:
                        c = LAVA_HOT
                    elif h in (1, 2):
                        c = LAVA_ORANGE
                    else:
                        c = LAVA_RED
                else:
                    h = (x * 7 + y * 11 + z * 3) % 6
                    c = BRICK_L if h == 0 else (BRICK_M if h in (1, 2, 3) else BRICK_D)
                m.set(x, y, z, c)
    save(m, 214)


def hellforge():
    """Squat forge with glowing orange mouth and anvil/chimney silhouette (15x14x13)."""
    m = Model(15, 14, 13)
    FORGE_D = hexc("#1a151e")
    FORGE_M = hexc("#2a2230")
    FORGE_L = hexc("#3d3245")
    HELL_ORE = hexc("#8c2c10")
    EMBER_D = hexc("#c4410f")
    EMBER = hexc("#ff9420")
    EMBER_L = hexc("#ffd84a")
    FLAME_W = hexc("#fff3a0")

    m.box(1, 0, 1, 13, 1, 11, FORGE_D)

    for x in range(2, 13):
        for y in range(2, 9):
            for z in range(2, 11):
                c = HELL_ORE if (x * y + z) % 7 == 0 else (FORGE_M if (x + y + z) % 2 == 0 else FORGE_L)
                m.set(x, y, z, c)

    m.box(0, 8, 5, 2, 9, 7, FORGE_M)
    m.box(0, 9, 5, 0, 9, 7, FORGE_L)

    for x in range(9, 13):
        for y in range(9, 14):
            for z in range(4, 9):
                if 10 <= x <= 11 and 5 <= z <= 6 and y >= 11:
                    continue
                c = FORGE_L if y == 13 else (HELL_ORE if (x + y) % 3 == 0 else FORGE_D)
                m.set(x, y, z, c)

    for x in range(4, 11):
        for y in range(3, 8):
            m.set(x, y, 10, FORGE_D)
    for x in range(5, 10):
        for y in range(4, 7):
            m.set(x, y, 9, EMBER_D)
            m.set(x, y, 10, EMBER)
    for x in range(6, 9):
        m.set(x, 4, 10, EMBER_L)
        m.set(x, 5, 10, FLAME_W)
        m.set(x, 4, 9, EMBER_L)

    save(m, 221)


def guide_voodoo_doll():
    """Straw figure with cloth wrap and red pin (9x15x7)."""
    m = Model(9, 15, 7)
    STRAW_D = hexc("#9c7e30")
    STRAW = hexc("#d4b359")
    STRAW_L = hexc("#ebd281")
    CLOTH_D = hexc("#beb7a8")
    CLOTH = hexc("#e8e2d5")
    PIN_RED = hexc("#e01b24")
    PIN_L = hexc("#ff5c5c")
    SILVER_SH = hexc("#d0d4dc")

    for y in range(0, 4):
        for z in range(2, 5):
            for x in (2, 3):
                m.set(x, y, z, STRAW_D if y == 0 else STRAW)
            for x in (5, 6):
                m.set(x, y, z, STRAW_D if y == 0 else STRAW)

    for x in range(2, 7):
        for y in range(4, 9):
            for z in range(2, 5):
                m.set(x, y, z, STRAW_L if (x + y) % 2 == 0 else STRAW)

    for y in range(4, 8):
        m.set(1, y, 3, STRAW)
        m.set(7, y, 3, STRAW)

    for x in range(2, 7):
        for y in (5, 6):
            for z in range(1, 6):
                if x in (2, 6) or z in (1, 5):
                    m.set(x, y, z, CLOTH_D if (x + y + z) % 2 == 0 else CLOTH)

    for x in range(3, 6):
        for z in range(2, 5):
            m.set(x, 9, z, STRAW_D)

    for x in range(2, 7):
        for y in range(10, 14):
            for z in range(1, 6):
                c = STRAW_L if y == 13 else (STRAW_D if (x in (2, 6) and z in (1, 5)) else STRAW)
                m.set(x, y, z, c)

    m.set(3, 11, 5, STRAW_D)
    m.set(5, 11, 5, STRAW_D)
    m.set(4, 14, 3, STRAW_L)
    m.set(3, 14, 3, STRAW)
    m.set(5, 14, 3, STRAW)

    m.set(5, 7, 5, SILVER_SH)
    m.set(5, 7, 6, SILVER_SH)
    m.set(5, 7, 7, PIN_RED)
    m.set(5, 8, 6, PIN_L)
    m.set(6, 7, 6, PIN_RED)

    save(m, 267)


def obsidian_skin_potion():
    """Round bottle with dark smoky obsidian liquid and orange cork (9x12x9)."""
    m = Model(9, 12, 9)
    CORK = hexc("#e66819")
    CORK_D = hexc("#b34b07")
    GLASS = hexc("#9ec2d6")
    GLASS_HI = hexc("#e0f2f7")
    SMOKE_D = hexc("#1a1520")
    SMOKE_M = hexc("#2d2438")
    SMOKE_L = hexc("#453754")
    EMBER_FLECK = hexc("#e8751a")

    for x in range(3, 6):
        for z in range(3, 6):
            if (x - 4.0) ** 2 + (z - 4.0) ** 2 <= 1.5 ** 2:
                m.set(x, 10, z, CORK)
                m.set(x, 11, z, CORK_D if (x == 4 and z == 4) else CORK)

    for x in range(3, 6):
        for z in range(3, 6):
            d2 = (x - 4.0) ** 2 + (z - 4.0) ** 2
            if 0.8 ** 2 <= d2 <= 1.8 ** 2:
                m.set(x, 9, z, GLASS_HI if (x == 3 and z == 4) else GLASS)
                m.set(x, 8, z, GLASS)

    for x in range(9):
        for y in range(8):
            for z in range(9):
                dx, dy, dz = x - 4.0, y - 4.0, z - 4.0
                r2 = dx * dx + dy * dy + dz * dz
                if r2 <= 3.8 ** 2:
                    is_shell = r2 >= 2.6 ** 2
                    if is_shell:
                        if x in (2, 3) and y in (5, 6) and z in (5, 6):
                            c = GLASS_HI
                        else:
                            c = GLASS
                    else:
                        if y <= 5:
                            h = (x * 3 + y * 7 + z * 11) % 11
                            if h == 0:
                                c = EMBER_FLECK
                            elif h in (1, 2, 3):
                                c = SMOKE_L
                            elif h in (4, 5, 6):
                                c = SMOKE_M
                            else:
                                c = SMOKE_D
                        else:
                            c = GLASS
                    m.set(x, y, z, c)

    save(m, 288)


def lava_charm():
    """Gold loop necklace with a glowing molten lava orb (11x15x7)."""
    m = Model(11, 15, 7)
    GOLD_D = hexc("#a07810")
    GOLD = hexc("#d4a017")
    GOLD_L = hexc("#f5cf47")
    LAVA_HOT = hexc("#ffc740")
    LAVA_ORANGE = hexc("#ff8c1a")
    LAVA_RED = hexc("#d9380f")
    OBS_CRUST = hexc("#241c2b")
    FLAME_W = hexc("#fff3a0")

    for x in range(4, 7):
        m.set(x, 14, 3, GOLD_L if x == 5 else GOLD)
    chain_l = [(3, 13, 3), (2, 12, 3), (2, 11, 3), (2, 10, 3), (3, 9, 3), (4, 8, 3)]
    for x, y, z in chain_l:
        m.set(x, y, z, GOLD_L if y % 2 == 0 else GOLD)
    chain_r = [(7, 13, 3), (8, 12, 3), (8, 11, 3), (8, 10, 3), (7, 9, 3), (6, 8, 3)]
    for x, y, z in chain_r:
        m.set(x, y, z, GOLD if y % 2 == 0 else GOLD_D)

    for x in range(3, 8):
        for z in range(2, 5):
            if (x in (3, 7) or z in (2, 4)):
                m.set(x, 7, z, GOLD_L if z == 4 else GOLD)

    m.set(5, 0, 3, GOLD_L)
    m.set(5, 1, 3, GOLD)

    for x in range(2, 9):
        for y in range(2, 7):
            for z in range(0, 7):
                dx = x - 5.0
                dy = y - 4.0
                dz = z - 3.0
                r2 = dx * dx + dy * dy + dz * dz
                if r2 <= 2.6 ** 2:
                    if r2 <= 1.2 ** 2:
                        c = FLAME_W if (x == 5 and y == 4) else LAVA_HOT
                    elif r2 <= 1.9 ** 2:
                        c = LAVA_ORANGE
                    else:
                        if (x + y + z) % 5 == 0:
                            c = OBS_CRUST
                        else:
                            c = LAVA_RED if (x + y) % 2 == 0 else LAVA_ORANGE
                    m.set(x, y, z, c)

    save(m, 906)


# M10 Molten gear: basalt bodies with magma seams. Same 1/16-voxel scheme as
# the tiers above; the ore and block art lives in the M9 section.
MOLT_D = hexc("#1c171d")  # cooled crust
MOLT_M = hexc("#332b34")  # basalt
MOLT_L = hexc("#4a4048")  # lit basalt


def flamarang():
    """Curved basalt crescent boomerang with a molten outer edge (16x16x2)."""
    m = Model(16, 16, 2)
    for x in range(1, 15):
        for y in range(1, 15):
            in_arm1 = 1 <= y <= 4 and 1 <= x <= 14
            in_arm2 = 1 <= x <= 4 and 1 <= y <= 14
            if (x >= 13 and (y < 2 or y > 3)) or (y >= 13 and (x < 2 or x > 3)):
                continue
            if in_arm1 or in_arm2:
                if x == 1 or y == 1:
                    c = FLAME_Y  # molten leading edge
                elif x == 4 or y == 4:
                    c = MOLT_D  # cooled inner bevel
                else:
                    c = MOLT_M if (x + y) % 2 == 0 else MOLT_L
                px(m, x, y, c)
    # Magma seams along the spine and an ember-hot elbow.
    for i in (6, 8, 10):
        px(m, i, 2, FLAME_O)
        px(m, 2, i, FLAME_O)
    px(m, 2, 2, FLAME_W)
    px(m, 3, 3, FLAME_O)
    save(m, 119)


def molten_fury():
    """Sweeping volcanic recurve bow (12x22x4) with blazing orange-yellow tips and basalt grip."""
    m = Model(12, 22, 4)
    for y in range(22):
        t = y / 21.0
        # Recurve S-curve profile
        off = round(6.5 * math.sin(math.pi * t) - 1.5 * math.sin(3 * math.pi * t))
        bx = max(1, min(9, 2 + off))
        if y in (0, 1, 20, 21):
            c, back = FLAME_Y, FLAME_O
        elif y in (2, 3, 18, 19):
            c, back = FLAME_O, FLAME_R
        elif 9 <= y <= 12:
            c, back = GRIP, MOLT_D
        else:
            c = MOLT_M if y % 2 else MOLT_L
            back = MOLT_D
        px_full(m, bx, y, c)
        px(m, bx + 1, y, back)
    for y in range(1, 21):
        m.set(1, y, 1, STRING)
        m.set(1, y, 2, STRING)
    px_full(m, 1, 0, FLAME_W)
    px_full(m, 1, 21, FLAME_W)
    save(m, 120)


def fiery_greatsword():
    """Terraria Fiery Greatsword / Volcano (28x28x4): massive jagged volcanic greatsword."""
    m = Model(28, 28, 4)
    handle(m, 5)
    # Horned demon-iron crossguard
    for d in range(-5, 6):
        gx, gy = 6 + d, 6 - d
        if 0 <= gx < 28 and 0 <= gy < 28:
            px_full(m, gx, gy, MOLT_D if abs(d) < 4 else FLAME_R)
            if abs(d) >= 4 and gx + 1 < 28 and gy + 1 < 28:
                px_full(m, gx + 1, gy + 1, FLAME_O)
    px_full(m, 6, 6, FLAME_O)
    # Wide charred basalt core, white-hot magma spine, blazing serrated flame-tooth edges
    for i in range(7, 25):
        px_full(m, i, i, FLAME_W if i % 3 == 0 else FLAME_Y)  # white-hot magma spine
        px(m, i - 1, i, MOLT_M)
        px(m, i, i - 1, MOLT_D)
        px(m, i - 1, i + 1, FLAME_O)
        px(m, i + 1, i - 1, FLAME_R)
        if i % 2 == 0 and i < 24:  # serrated flame-tooth edges
            px(m, i - 2, i + 1, FLAME_Y)
            px(m, i + 1, i - 2, FLAME_O)
    for x, y in ((24, 25), (25, 24), (25, 26), (26, 25)):
        px_full(m, x, y, FLAME_O)
    px_full(m, 25, 25, FLAME_Y)
    px_full(m, 26, 26, FLAME_W)
    px(m, 27, 27, FLAME_W)
    save(m, 121)


def hellstone_bar():
    """Molten ingot: basalt crust over a glowing magma core (14x7x8)."""
    m = Model(14, 7, 8)
    for y in range(6):
        x_min, x_max = 1 + y // 2, 12 - y // 2
        z_min, z_max = 1 + y // 2, 6 - y // 2
        for x in range(x_min, x_max + 1):
            for z in range(z_min, z_max + 1):
                seam = (x * 3 + z * 5) % 7 == 0
                edge = z == z_max or x == x_max
                if y == 0:
                    c = MOLT_D
                elif y == 5 and edge:
                    c = FLAME_Y if seam else FLAME_O
                elif edge:
                    c = FLAME_O if seam else MOLT_L
                elif z == z_min or x == x_min:
                    c = MOLT_D
                else:
                    c = FLAME_R if seam else (MOLT_M if (x + y + z) % 2 == 0 else MOLT_D)
                m.set(x, y, z, c)
    save(m, 175)


def phoenix_blaster():
    """Molten pistol with a vented barrel and white-hot muzzle (15x11x3)."""
    m = Model(15, 11, 3)
    # Grip (angled handle) with a magma inlay.
    for y in range(0, 6):
        gx = 2 + y // 2
        for z in (0, 1, 2):
            c = MOLT_M if z == 1 else MOLT_D
            m.set(gx, y, z, c)
            m.set(gx + 1, y, z, c)
            if y <= 3:
                m.set(gx + 2, y, z, MOLT_D if z != 1 else FLAME_R)
    # Trigger guard and trigger.
    m.set(5, 5, 1, MOLT_M)
    m.set(6, 4, 1, FLAME_R)
    m.set(7, 4, 1, MOLT_M)
    m.set(8, 5, 1, MOLT_M)
    m.set(6, 5, 1, MOLT_D)
    # Receiver.
    for x in range(3, 11):
        for z in (0, 1, 2):
            m.set(x, 6, z, MOLT_M if (x + z) % 2 == 0 else MOLT_D)
    # Slide with glowing vent seams.
    for x in range(2, 14):
        for y in (7, 8):
            for z in (0, 1, 2):
                if y == 8:
                    c = FLAME_O if x in (5, 8, 11) else (MOLT_L if z == 1 else MOLT_M)
                else:
                    c = FLAME_Y if x == 13 else (MOLT_D if z == 0 else MOLT_M)
                m.set(x, y, z, c)
    m.set(13, 7, 1, FLAME_W)  # muzzle glow
    m.set(6, 8, 2, FLAME_O)  # ejection port ember
    m.set(7, 8, 2, FLAME_O)
    m.set(2, 9, 0, MOLT_D)  # rear sight
    m.set(2, 9, 2, MOLT_D)
    m.set(12, 9, 1, FLAME_Y)  # front sight
    save(m, 219)


def molten_helmet_drop():
    """Horned basalt helm with a magma crest (12x13x11)."""
    m = Model(12, 13, 11)
    for x in range(2, 10):
        for y in range(2, 9):
            for z in range(2, 10):
                d = ((x - 5.5) / 3.8) ** 2 + ((y - 5.0) / 3.8) ** 2 + ((z - 5.5) / 3.8) ** 2
                if d <= 1.0:
                    m.set(x, y, z, MOLT_L if (x + y + z) % 2 == 0 else MOLT_M)
    # Glowing eye slit over the face and a molten brow band.
    for x in range(3, 9):
        m.set(x, 4, 9, FLAME_O if x in (4, 7) else FLAME_Y)
        m.set(x, 3, 9, MOLT_D)
        m.set(x, 5, 9, MOLT_D)
    for y in range(7, 11):
        for z in range(3, 8):
            m.set(5, y, z, FLAME_O if y >= 9 else FLAME_R)
            m.set(6, y, z, MOLT_M if y >= 9 else FLAME_O)
    for hx, dx in [(1, -1), (10, 1)]:
        horn = [
            (hx, 6, 6), (hx, 7, 5), (hx + dx, 8, 4),
            (hx + dx, 9, 3), (hx + dx, 10, 2), (hx + dx, 11, 1),
            (hx + dx, 12, 1),
        ]
        for x, y, z in horn:
            m.set(x, y, z, MOLT_L if y >= 11 else MOLT_M)
            m.set(x, y - 1, z, MOLT_D)
    save(m, 231)


def molten_breastplate_drop():
    """Basalt chestplate with a magma centre seam and pauldrons (14x13x8)."""
    m = Model(14, 13, 8)
    m.box(3, 1, 2, 10, 9, 5, MOLT_D)
    for y in range(1, 10):
        for x in range(4, 10):
            c = FLAME_O if x in (6, 7) else (MOLT_L if (x + y) % 2 == 0 else MOLT_M)
            m.set(x, y, 6, c)
    for x_p, spike_x in [(1, 0), (10, 13)]:
        m.box(x_p, 8, 1, x_p + 2, 11, 6, MOLT_M)
        m.set(spike_x, 11, 4, FLAME_R)
        m.set(spike_x, 12, 4, FLAME_Y)
        m.set(spike_x, 12, 3, FLAME_O)
    m.box(5, 10, 3, 8, 10, 4, FLAME_Y)  # molten collar
    save(m, 232)


def molten_greaves_drop():
    """Basalt greaves with glowing knee ridges (12x12x8)."""
    m = Model(12, 12, 8)
    m.box(4, 2, 3, 7, 3, 4, MOLT_D)
    for x_base in (1, 7):
        m.box(x_base, 0, 1, x_base + 3, 0, 6, MOLT_D)
        m.box(x_base, 1, 1, x_base + 3, 3, 5, MOLT_M)
        m.set(x_base + 1, 1, 6, FLAME_O)
        m.set(x_base + 2, 1, 6, MOLT_L)
        m.box(x_base, 4, 2, x_base + 3, 8, 5, MOLT_M)
        for y in range(4, 9):
            m.set(x_base + 1, y, 5, FLAME_Y if y % 2 == 0 else MOLT_L)
        m.box(x_base, 9, 3, x_base + 3, 9, 5, MOLT_L)
        m.set(x_base + 1, 10, 5, FLAME_O)
        m.set(x_base + 2, 10, 5, MOLT_M)
        m.set(x_base + 1, 11, 5, FLAME_Y)
    save(m, 233)


# M11 Wall of Flesh drops
PWN_PINK_D = hexc("#b03a7a")
PWN_PINK = hexc("#e86cb0")
PWN_PINK_L = hexc("#ffb8e0")
ARCANE_D = hexc("#5a189a")
ARCANE = hexc("#9d4edd")
ARCANE_L = hexc("#e0aaff")
EMERALD_D = hexc("#1b7a43")
EMERALD = hexc("#2ecc71")
EMERALD_L = hexc("#8affb8")


def pwnhammer():
    """Hallowed golden-pink warhammer (20x20x4)."""
    m = Model(20, 20, 4)
    handle(m, 14)
    for i in range(-5, 6):
        hx = 14 + i
        hy = 14 - i
        for dx in (0, 1):
            for dy in (0, 1):
                for z in (0, 1, 2, 3):
                    x, y = hx + dx, hy + dy
                    if abs(i) == 5:
                        c = PWN_PINK_L if z >= 2 else PWN_PINK_D
                    elif abs(i) == 4:
                        c = GOLD_L if z == 3 else GOLD_D
                    else:
                        c = PWN_PINK if z in (1, 2) else (GOLD_L if z == 3 else PWN_PINK_D)
                    m.set(x, y, z, c)
    save(m, 367)


def breaker_blade():
    """Colossal wide Terraria Buster-style greatsword (32x32x4)."""
    m = Model(32, 32, 4)
    handle(m, 7)
    g = 8
    for d in range(-6, 7):
        gx, gy = g + d, g - d
        if 0 <= gx < 32 and 0 <= gy < 32:
            px_full(m, gx, gy, GOLD_D if abs(d) >= 5 else GOLD)
            if gy + 1 < 32:
                px_full(m, gx, gy + 1, GOLD_L if d <= 0 else GOLD_D)
    # Massive broad steel slab (width 5-6 voxels) with dark iron fuller, gold studs & bright silver edge
    for k in range(9, 29):
        w = 3 if k < 25 else (2 if k < 27 else (1 if k < 28 else 0))
        for d in range(-w, w + 1):
            bx, by = k + d, k - d
            if 0 <= bx < 32 and 0 <= by < 32:
                if d == -w:
                    c = SILVER_L
                elif d == w:
                    c = IRON_D
                elif d == 0:
                    c = GOLD_L if k in (12, 16, 20, 24) else IRON_D
                else:
                    c = IRON
                if abs(d) <= 1:
                    px_full(m, bx, by, c)
                else:
                    px(m, bx, by, c)
    px_full(m, 29, 29, SILVER_L)
    px(m, 29, 30, SILVER_L)
    px(m, 30, 29, SILVER_L)
    px(m, 30, 30, SILVER_L)
    px(m, 31, 31, SILVER_L)
    save(m, 426)


def clockwork_assault_rifle():
    """Brass-and-steel clockwork burst rifle (22x10x3)."""
    m = Model(22, 10, 3)
    m.box(0, 2, 1, 4, 4, 1, WOOD_D)
    m.box(1, 3, 1, 4, 5, 1, WOOD)
    m.box(5, 3, 0, 11, 6, 2, GOLD_D)
    m.box(6, 4, 1, 10, 6, 2, GOLD)
    m.set(8, 5, 2, GOLD_L)
    m.set(7, 2, 1, WOOD_D)
    m.set(8, 1, 1, WOOD_D)
    m.box(12, 4, 1, 20, 5, 1, IRON_D)
    m.box(12, 5, 1, 21, 5, 1, SILVER)
    m.set(14, 6, 1, GOLD_L)
    save(m, 434)


def emblem_medallion(item_id, gem_d, gem, gem_l):
    """Golden class emblem medallion (12x12x3) with distinct center gem."""
    m = Model(12, 12, 3)
    for x in range(12):
        for y in range(12):
            dist = abs(x - 5.5) + abs(y - 5.5)
            if dist <= 5.5:
                if dist <= 2.2:
                    c = gem_l if (x + y >= 11) else (gem_d if (x + y <= 9) else gem)
                    m.set(x, y, 2, c)
                    m.set(x, y, 1, gem)
                    m.set(x, y, 0, gem_d)
                else:
                    c = GOLD_L if (x + y > 11) else (GOLD_D if (x + y < 10) else GOLD)
                    m.set(x, y, 1, c)
                    if dist <= 4.5:
                        m.set(x, y, 2, c)
    save(m, item_id)


def laser_rifle():
    """Dark-violet and magenta energy rifle (22x10x3)."""
    m = Model(22, 10, 3)
    m.box(0, 2, 1, 4, 5, 1, IRON_D)
    m.box(5, 3, 0, 12, 6, 2, ARCANE_D)
    m.box(6, 4, 1, 11, 5, 2, ARCANE)
    m.box(8, 4, 2, 10, 5, 2, ARCANE_L)
    m.set(7, 2, 1, IRON_D)
    m.set(8, 1, 1, IRON_D)
    m.box(13, 4, 1, 21, 5, 1, SILVER)
    m.box(14, 5, 1, 20, 5, 1, ARCANE_L)
    save(m, 514)


def firecracker():
    """Fiery crimson-and-gold whip (18x18x2)."""
    m = Model(18, 18, 2)
    handle(m, 5)
    px(m, 5, 5, GOLD_L)
    pts = [
        (6, 6), (7, 7), (7, 8), (8, 9), (8, 10), (9, 11), (10, 12),
        (11, 13), (12, 13), (13, 13), (14, 12), (15, 11), (16, 12),
        (16, 13), (16, 14), (15, 15), (15, 16),
    ]
    for idx, (x, y) in enumerate(pts):
        c = FLAME_Y if idx >= len(pts) - 3 else (FLAME_O if idx % 2 == 0 else RED_L)
        px(m, x, y, c)
    save(m, 4912)


def bug_net():
    """Wooden-handled bug net with iron hoop and woven mesh bag (19x19x4)."""
    m = Model(N, N, 4)
    handle(m, 9)
    rim_m, rim_l, rim_d = METAL["iron"]
    # Collar joining handle to hoop
    m.box(9, 9, 0, 11, 11, 3, rim_d)
    m.set(10, 10, 1, rim_m)
    m.set(10, 10, 2, rim_m)
    # Circular/oval metal hoop from (10,10) to (18,18)
    hoop_pts = [
        (10, 12), (10, 13), (10, 14),
        (11, 15), (11, 16),
        (12, 17), (13, 17), (14, 18), (15, 18),
        (16, 17), (17, 17), (17, 16), (18, 15), (18, 14),
        (17, 13), (17, 12), (16, 11), (15, 11),
        (14, 10), (13, 10), (12, 10), (11, 11),
    ]
    for x, y in hoop_pts:
        px_full(m, x, y, rim_l if (x + y) >= 29 else rim_m)
    # Woven net mesh filling the hoop and tapering back in z
    for x in range(11, 18):
        for y in range(11, 18):
            dx = x - 14.0
            dy = y - 14.0
            r2 = dx * dx + dy * dy
            if r2 <= 10.5:
                c = FEATHER if (x + y) % 2 == 0 else STRING
                m.set(x, y, 1, c)
                if r2 <= 6.0:
                    m.set(x, y, 0, FEATHER_D)
    save(m, 1991)


def bunny_item():
    """Handcrafted 3D white bunny critter item (12x13x6)."""
    m = Model(12, 13, 6)
    fur = FEATHER
    fur_d = FEATHER_D
    pink = hexc("#f4a4b8")
    eye = hexc("#1f1b24")
    # Body
    m.box(3, 1, 1, 8, 5, 4, fur)
    m.box(2, 0, 1, 8, 1, 4, fur_d)
    # Fluffy tail
    m.box(1, 2, 2, 2, 3, 3, fur)
    # Head
    m.box(6, 5, 1, 10, 8, 4, fur)
    # Nose & eyes
    m.set(11, 6, 2, pink)
    m.set(11, 6, 3, pink)
    m.set(9, 7, 1, eye)
    m.set(9, 7, 4, eye)
    # Long upright ears
    m.box(6, 9, 1, 7, 12, 2, fur)
    m.box(6, 9, 3, 7, 12, 4, fur)
    m.set(7, 10, 1, pink)
    m.set(7, 11, 1, pink)
    m.set(7, 10, 4, pink)
    m.set(7, 11, 4, pink)
    save(m, 2019)


def bird_item():
    """Handcrafted 3D perched blue songbird critter item (11x10x6)."""
    m = Model(11, 10, 6)
    blue = hexc("#3e84c6")
    blue_l = hexc("#68aee8")
    blue_d = hexc("#285a8e")
    eye = hexc("#1f1b24")
    # Feet
    m.set(4, 0, 1, FLAME_O)
    m.set(5, 0, 1, FLAME_O)
    m.set(4, 0, 4, FLAME_O)
    m.set(5, 0, 4, FLAME_O)
    # Body & cream breast
    m.box(3, 1, 1, 7, 4, 4, blue)
    m.box(5, 1, 2, 7, 3, 3, STRING)
    # Folded wings
    m.box(3, 2, 0, 6, 4, 0, blue_d)
    m.box(3, 2, 5, 6, 4, 5, blue_d)
    # Tail feathers
    m.box(0, 3, 2, 2, 4, 3, blue_d)
    m.set(0, 4, 2, blue)
    m.set(0, 4, 3, blue)
    # Head & crest
    m.box(5, 5, 1, 8, 7, 4, blue_l)
    m.set(6, 8, 2, blue_l)
    m.set(6, 8, 3, blue_l)
    # Eyes & beak
    m.set(7, 6, 1, eye)
    m.set(7, 6, 4, eye)
    m.box(9, 5, 2, 10, 6, 3, FLAME_Y)
    save(m, 2015)


def squirrel_item():
    """Handcrafted 3D chestnut squirrel critter item (13x12x6)."""
    m = Model(13, 12, 6)
    fur = HANDLE
    fur_l = HANDLE_L
    fur_d = HANDLE_D
    eye = hexc("#1f1b24")
    # Body & paws
    m.box(4, 0, 1, 9, 4, 4, fur)
    m.box(7, 1, 2, 9, 3, 3, STRING)
    m.set(10, 2, 2, fur_l)
    m.set(10, 2, 3, fur_l)
    # Bushy curled tail
    m.box(1, 2, 1, 3, 6, 4, fur_d)
    m.box(0, 5, 1, 3, 9, 4, fur)
    m.box(1, 8, 2, 4, 11, 3, fur_l)
    # Head & ears
    m.box(7, 5, 1, 11, 8, 4, fur_l)
    m.set(8, 9, 1, fur_d)
    m.set(8, 9, 4, fur_d)
    # Nose & eyes
    m.set(12, 6, 2, GRIP)
    m.set(12, 6, 3, GRIP)
    m.set(10, 7, 1, eye)
    m.set(10, 7, 4, eye)
    save(m, 2018)


def cave_critter_item():
    """Handcrafted 3D glowing cave beetle critter item (12x10x6)."""
    m = Model(12, 10, 6)
    shell_d = hexc("#3d3552")
    shell = hexc("#584d73")
    glow = hexc("#4be0e8")
    eye = hexc("#fff3a0")
    # Legs
    for x in (3, 5, 7):
        m.set(x, 0, 0, shell_d)
        m.set(x, 1, 0, shell_d)
        m.set(x, 0, 5, shell_d)
        m.set(x, 1, 5, shell_d)
    # Domed carapace
    m.box(2, 1, 1, 8, 4, 4, shell_d)
    m.box(3, 2, 1, 7, 5, 4, shell)
    m.box(4, 5, 2, 6, 5, 3, glow)
    m.set(3, 4, 1, glow)
    m.set(3, 4, 4, glow)
    # Head, glowing eyes & antennae
    m.box(8, 1, 1, 10, 3, 4, shell_d)
    m.set(10, 2, 1, eye)
    m.set(10, 2, 4, eye)
    m.set(11, 3, 1, glow)
    m.set(11, 4, 1, glow)
    m.set(11, 3, 4, glow)
    m.set(11, 4, 4, glow)
    save(m, 2007)


def coin_item(item_id, base, light, dark):
    """Sculpted circular coin disc with raised rim and center slot."""
    m = Model(11, 11, 4)
    cx, cy = 5, 5
    for y in range(11):
        for x in range(11):
            d2 = (x - cx) ** 2 + (y - cy) ** 2
            if d2 <= 22:
                if d2 >= 13:
                    px_full(m, x, y, light if x + y < 10 else dark)
                else:
                    px(m, x, y, base)
    # Center vertical slot / emblem
    for y in range(3, 8):
        px(m, cx, y, dark)
        m.set(cx, y, 2, light)
    save(m, item_id)


def potion_item(item_id, liquid_d, liquid, liquid_l, tall=False):
    """Glass potion flask with cork, neck, and colored liquid."""
    h = 15 if tall else 14
    m = Model(12, h, 4)
    cork = HANDLE
    cork_l = HANDLE_L
    # Cork & neck
    m.box(5, h - 2, 1, 6, h - 1, 2, cork)
    m.set(5, h - 1, 2, cork_l)
    m.box(4, h - 4, 1, 7, h - 3, 2, GLASS_F)
    m.box(5, h - 4, 1, 6, h - 3, 2, GLASS_G)
    # Bulbous body
    by_top = h - 5
    for y in range(1, by_top + 1):
        w = 4 if (y >= 2 and y <= by_top - 1) else 3
        for x in range(6 - w, 6 + w):
            is_edge = x == 6 - w or x == 6 + w - 1 or y == 1 or y == by_top
            if is_edge:
                px_full(m, x, y, GLASS_F)
            elif y <= by_top - 2:
                col = liquid_l if (x <= 4 and y >= 4) else (liquid_d if x >= 7 else liquid)
                px(m, x, y, col)
            else:
                px(m, x, y, GLASS_G)
    save(m, item_id)


def nights_edge():
    """Night's Edge: broad dark-obsidian & glowing violet notched greatsword."""
    m = Model(N, N, 4)
    blade_d = hexc("#2b1644")
    blade = hexc("#5a258c")
    edge = hexc("#b05cff")
    edge_l = hexc("#e2b3ff")
    guard = hexc("#3c1b59")
    # Pommel & grip
    px_full(m, 1, 1, guard)
    px_full(m, 2, 2, GRIP)
    px_full(m, 3, 3, GRIP)
    # Swept crossguard
    for dx, dy in [(-2, 2), (-1, 1), (0, 0), (1, -1), (2, -2)]:
        px_full(m, 4 + dx, 4 + dy, guard)
    px_full(m, 2, 7, edge)
    px_full(m, 7, 2, edge)
    # Broad curved/notched dark-violet blade
    for i in range(5, N - 1):
        px_full(m, i, i, blade_d)
        px(m, i - 1, i, blade)
        px(m, i, i - 1, blade)
        if i >= 7:
            px(m, i - 2, i, edge if i < N - 3 else edge_l)
            px(m, i, i - 2, edge if i < N - 3 else edge_l)
    px_full(m, N - 1, N - 1, edge_l)
    px(m, N - 2, N - 1, edge_l)
    px(m, N - 1, N - 2, edge_l)
    save(m, 273)


def space_gun():
    """Space Gun: meteorite-bronze sci-fi raygun with glowing cyan energy coils."""
    m = Model(16, 11, 4)
    body = COPPER
    body_d = COPPER_D
    body_l = COPPER_L
    glow = hexc("#33e6ff")
    glow_l = hexc("#b8f7ff")
    # Grip
    m.box(2, 1, 1, 4, 4, 2, body_d)
    m.box(3, 2, 1, 4, 4, 2, GRIP)
    # Receiver & barrel
    m.box(2, 5, 1, 12, 8, 2, body)
    m.box(3, 8, 1, 10, 8, 2, body_l)
    m.box(12, 6, 1, 14, 7, 2, body_d)
    # Glowing cyan energy coils & muzzle tip
    for x in (6, 8, 10):
        px_full(m, x, 6, glow)
        px_full(m, x, 7, glow_l)
    px_full(m, 15, 6, glow_l)
    px_full(m, 15, 7, glow)
    save(m, 127)


def water_bolt():
    """Water Bolt: sapphire spellbook with gold trim, pages, and water crest."""
    m = Model(12, 14, 4)
    pages = hexc("#efe6cf")
    # Book cover
    m.box(2, 1, 0, 10, 12, 3, DB_BLUE)
    m.box(2, 1, 0, 3, 12, 3, DB_BLUE_D)
    m.box(3, 11, 0, 10, 12, 3, DB_BLUE_L)
    # Exposed cream pages on right edge
    m.box(9, 2, 1, 10, 11, 2, pages)
    # Gold clasp & water-crest emblem on front face
    m.box(5, 5, 3, 7, 8, 3, hexc("#5cc8ff"))
    m.set(6, 9, 3, hexc("#b8ecff"))
    m.box(9, 6, 3, 10, 7, 3, GOLD)
    save(m, 165)


def slime_crown():
    """Slime Crown: golden 3-peaked crown set with blue gel jewel."""
    m = Model(13, 10, 4)
    gel = hexc("#3b9eff")
    gel_l = hexc("#9ed4ff")
    ruby = hexc("#e62e3e")
    # Crown band
    m.box(2, 2, 1, 10, 4, 2, GOLD)
    m.box(2, 2, 1, 10, 2, 2, GOLD_D)
    # Three peaks
    m.box(2, 5, 1, 3, 7, 2, GOLD_L)
    m.box(5, 5, 1, 7, 8, 2, GOLD_L)
    m.box(9, 5, 1, 10, 7, 2, GOLD_L)
    # Center slime jewel & side gems
    px_full(m, 6, 3, gel)
    m.set(6, 4, 2, gel_l)
    px_full(m, 3, 3, ruby)
    px_full(m, 9, 3, ruby)
    save(m, 560)


def abeemination():
    """Abeemination: honey-amber & black-striped Queen Bee summon with wings."""
    m = Model(14, 12, 4)
    amber = FLAME_Y
    amber_d = FLAME_O
    wing = GLASS_G
    wing_rim = GLASS_F
    # Wings
    m.box(2, 7, 1, 5, 10, 2, wing)
    m.box(8, 7, 1, 11, 10, 2, wing)
    m.set(2, 10, 1, wing_rim)
    m.set(11, 10, 1, wing_rim)
    # Striped bee-medallion body
    m.box(3, 2, 1, 10, 7, 2, amber)
    m.box(4, 1, 1, 9, 8, 2, amber_d)
    for x in (5, 8):
        m.box(x, 2, 0, x, 7, 3, GRIP)
    # Glowing eyes
    px_full(m, 4, 5, FLAME_R)
    px_full(m, 9, 5, FLAME_R)
    save(m, 1133)


def goblin_battle_standard():
    """Goblin Battle Standard: iron-tipped wooden pole with tattered crimson war banner."""
    m = Model(13, 18, 4)
    crimson = hexc("#c82828")
    crimson_d = hexc("#8a1818")
    # Pole
    m.box(2, 1, 1, 3, 16, 2, HANDLE)
    m.box(2, 16, 1, 3, 17, 2, IRON_L)
    # Crossbar
    m.box(2, 14, 1, 11, 15, 2, HANDLE_D)
    px_full(m, 11, 14, GOLD)
    # Crimson banner cloth with gold trim & tattered bottom
    m.box(4, 6, 1, 10, 13, 2, crimson)
    m.box(4, 12, 1, 10, 13, 2, GOLD)
    m.box(6, 8, 1, 8, 10, 2, crimson_d)
    m.box(4, 4, 1, 5, 5, 2, crimson_d)
    m.box(7, 5, 1, 8, 5, 2, crimson_d)
    m.box(9, 4, 1, 10, 5, 2, crimson_d)
    save(m, 361)


for tier in ("copper", "iron", "silver", "gold", "demonite"):
    pickaxe(tier, PICK[tier])
    axe(tier, AXE[tier])
    sword(tier, BROAD[tier], True)
sword("copper", SHORT["copper"], False)
wooden_sword()
wooden_bow()
arrow()
torch()
wood_drop()
gel_drop()
band_of_regeneration()
magic_mirror()
cloud_in_a_bottle()
hermes_boots()
enchanted_boomerang()
lucky_horseshoe()
shiny_red_balloon()
spear()
demonite_ore_drop()
demonite_bar()
vilethorn()
worm_food()
shadow_scale()
flintlock_pistol()
musket()
musket_ball()
minishark()
shadow_greaves()
shadow_scalemail()
shadow_helmet()
the_breaker()
band_of_starpower()
shadow_orb_item()
ball_o_hurt()
wooden_hammer()
magic_missile()
bone()
muramasa()
cobalt_shield()
aqua_scepter()
blue_moon()
handgun()
golden_key()
shadow_key()
valor()
ash_block()
obsidian()
hellstone()
obsidian_brick()
obsidian_skull()
hellstone_brick()
hellforge()
guide_voodoo_doll()
obsidian_skin_potion()
lava_charm()
# M10 Molten gear
flamarang()
molten_fury()
fiery_greatsword()
pickaxe("molten", 122)
axe("molten", 217)
hellstone_bar()
phoenix_blaster()
molten_helmet_drop()
molten_breastplate_drop()
molten_greaves_drop()
# M11 Wall of Flesh drops
pwnhammer()
breaker_blade()
clockwork_assault_rifle()
emblem_medallion(489, ARCANE_D, ARCANE, ARCANE_L)
emblem_medallion(490, RED_D, RED, RED_L)
emblem_medallion(491, EMERALD_D, EMERALD, EMERALD_L)
laser_rifle()
firecracker()
bug_net()
bunny_item()
bird_item()
squirrel_item()
cave_critter_item()
# Pass P2 iconic bespoke items
coin_item(71, COPPER, COPPER_L, COPPER_D)
coin_item(72, SILVER, SILVER_L, SILVER_D)
coin_item(73, GOLD, GOLD_L, GOLD_D)
coin_item(74, hexc("#dcebf7"), hexc("#ffffff"), hexc("#8fa8be"))
potion_item(188, hexc("#961824"), hexc("#e62e3e"), hexc("#ff8a95"), tall=False)
potion_item(189, hexc("#18459e"), hexc("#2a75f3"), hexc("#8cd2ff"), tall=True)
nights_edge()
space_gun()
water_bolt()
slime_crown()
abeemination()
goblin_battle_standard()
