"""Shared player part builders (Terraria's default guy) and rig layout.

Parts are authored in game axes (x right, y up, z front) at 16 voxels per block;
the rig root is scaled so the whole figure is 1.8 blocks tall. The legs' top
four rows sit inside the torso, so the visible proportions are 8/12/12.
"""
from palette import *
from vox import Model

SKIN = hexc("#e8a982")
SKIN_D = hexc("#c98660")
HAIR = hexc("#6b4423")
HAIR_D = hexc("#46290f")
HAIR_L = hexc("#8e5f31")
SHIRT = hexc("#f2f2f2")
SHIRT_D = hexc("#c5c9d6")
PANTS = hexc("#3d5fc4")
PANTS_D = hexc("#2b4496")
SHOE = hexc("#5c3a1e")
SHOE_D = hexc("#3a2312")
EYE_W = hexc("#ffffff")
EYE_P = hexc("#2a3a8c")
MOUTH = hexc("#b0605a")

# Part sizes (x, y, z) and rest pivots (voxels, part-local), then the pivot's
# rest position in the root frame (x right, y up from the feet, z front).
SIZES = {
    "head": (12, 12, 12),
    "torso": (12, 12, 6),
    "arm_l": (4, 12, 4),
    "arm_r": (4, 12, 4),
    "leg_l": (5, 12, 5),
    "leg_r": (5, 12, 5),
}
PIVOTS = {
    "head": (6, 0, 6),
    "torso": (6, 0, 3),
    "arm_l": (2, 12, 2),
    "arm_r": (2, 12, 2),
    "leg_l": (2.5, 12, 2.5),
    "leg_r": (2.5, 12, 2.5),
}
# The character faces +z, so its right hand is at -x.
OFFSETS = {
    "head": (0, 20, 0),
    "torso": (0, 8, 0),
    "arm_l": (8, 20, 0),
    "arm_r": (-8, 20, 0),
    "leg_l": (2.5, 12, 0),
    "leg_r": (-2.5, 12, 0),
}
ROOT_SCALE = 0.9  # 32 voxels * 0.9 / 16 = 1.8 blocks, the sim's player height


def head():
    m = Model(*SIZES["head"])
    # Rounded head core: chamfer all 12 cube edges for an organic silhouette.
    m.box(1, 1, 1, 10, 10, 10, SKIN)
    m.box(2, 1, 11, 9, 10, 11, SKIN)   # face plane
    m.box(2, 1, 0, 9, 10, 0, SKIN_D)   # back of head
    m.box(0, 2, 2, 0, 9, 9, SKIN_D)    # shaded right side
    m.box(11, 2, 2, 11, 9, 9, SKIN)    # left side
    m.box(2, 11, 2, 9, 11, 9, SKIN)    # crown
    m.box(2, 0, 2, 9, 0, 9, SKIN_D)    # tapered chin/jawline

    # Sculpted hair shell: crown, layered back, sideburns, and asymmetric bangs.
    m.box(2, 11, 2, 9, 11, 9, HAIR)
    m.box(3, 11, 3, 8, 11, 8, HAIR_L)  # crown highlight
    m.box(1, 9, 1, 10, 10, 10, HAIR)
    m.box(2, 2, 0, 9, 10, 0, HAIR_D)   # back nape
    m.box(1, 3, 1, 10, 10, 1, HAIR)
    m.box(0, 5, 2, 0, 9, 7, HAIR_D)    # side locks
    m.box(11, 5, 2, 11, 9, 7, HAIR_D)
    m.box(1, 4, 2, 1, 9, 5, HAIR)
    m.box(10, 4, 2, 10, 9, 5, HAIR)

    # Ears peeking below the sideburns.
    m.box(0, 5, 6, 0, 6, 7, SKIN_D)
    m.box(11, 5, 6, 11, 6, 7, SKIN)

    # Front bangs with textured strands.
    m.box(2, 9, 11, 9, 10, 11, HAIR)
    m.box(3, 10, 11, 7, 10, 11, HAIR_L)
    m.box(2, 8, 11, 4, 8, 11, HAIR_D)
    m.box(7, 8, 11, 9, 8, 11, HAIR_D)
    m.set(5, 8, 11, HAIR)

    # Terraria-style 1x2 vertical eyes + brows + subtle nose/mouth.
    for ex in (3, 8):
        m.set(ex, 7, 11, EYE_W)
        m.set(ex, 6, 11, EYE_P)
    m.set(2, 7, 11, SKIN_D)
    m.set(9, 7, 11, SKIN_D)
    m.box(5, 5, 11, 6, 5, 11, SKIN_D)  # nose bridge
    m.box(5, 3, 11, 6, 3, 11, MOUTH)   # mouth
    m.box(2, 1, 11, 9, 1, 11, SKIN_D)  # jaw shadow
    return m


def torso():
    m = Model(*SIZES["torso"])
    # Tapered chest & waist: 12-wide at shoulders/chest (y=7..10), 10-wide at waist (y=2..6),
    # chamfered front/back outer edges so it doesn't read as a flat brick.
    m.box(1, 0, 1, 10, 10, 4, SHIRT)
    m.box(1, 7, 0, 10, 10, 5, SHIRT)   # chest depth
    m.box(0, 7, 1, 11, 10, 4, SHIRT)   # broad shoulders
    m.box(2, 2, 0, 9, 6, 5, SHIRT)     # tapered waist
    m.box(1, 0, 0, 10, 2, 5, PANTS)    # hips
    m.box(1, 3, 0, 10, 3, 5, PANTS_D)  # belt line
    m.box(1, 11, 1, 10, 11, 4, SHIRT)  # rounded shoulder top

    # Shading & shirt folds.
    m.box(0, 7, 1, 0, 10, 4, SHIRT_D)
    m.box(1, 4, 1, 1, 6, 4, SHIRT_D)
    m.box(1, 0, 0, 1, 2, 5, PANTS_D)
    m.box(1, 0, 0, 10, 0, 5, PANTS_D)
    m.box(2, 4, 5, 9, 4, 5, SHIRT_D)   # tuck shadow above belt

    # Open V-neck collar.
    m.box(4, 10, 5, 7, 11, 5, SKIN)
    m.box(5, 9, 5, 6, 10, 5, SKIN)
    m.set(4, 10, 5, SKIN_D)
    m.set(5, 9, 5, SKIN_D)
    return m


def arm(outer_x):
    m = Model(*SIZES["arm_l"])
    inner_x = 3 - outer_x
    # Upper sleeve (y=5..10): rounded shoulder cap + rolled cuff.
    m.box(0, 5, 0, 3, 10, 3, SHIRT)
    m.box(1, 11, 1, 2, 11, 2, SHIRT)
    m.box(0, 5, 0, 3, 5, 3, SHIRT_D)  # sleeve cuff
    m.box(outer_x, 5, 0, outer_x, 10, 3, SHIRT_D)
    # Trim sharp sleeve corners.
    for x in (0, 3):
        for z in (0, 3):
            m.set(x, 10, z, None)

    # Slimmer forearm (y=2..4) and rounded hand (y=0..1).
    fx0, fx1 = (1, 3) if outer_x == 0 else (0, 2)
    m.box(fx0, 2, 1, fx1, 4, 2, SKIN)
    m.box(1, 2, 0, 2, 4, 3, SKIN)
    m.set(outer_x, 3, 1, SKIN_D)
    m.set(outer_x, 3, 2, SKIN_D)
    m.box(1, 0, 1, 2, 1, 2, SKIN)
    m.set(1 if outer_x == 0 else 2, 0, 1, SKIN_D)
    m.set(inner_x, 1, 2, SKIN)
    return m


def leg(outer_x):
    m = Model(*SIZES["leg_l"])
    # Thigh/trouser leg (y=4..11) with chamfered corners so legs separate cleanly.
    m.box(1, 4, 0, 3, 11, 4, PANTS)
    m.box(0, 4, 1, 4, 11, 3, PANTS)
    m.box(outer_x, 4, 1, outer_x, 11, 3, PANTS_D)

    # Tapered ankle (y=3).
    m.box(1, 3, 1, 3, 3, 3, PANTS_D)

    # Shaped boot (y=0..2): heel tucked, toe extending forward to z=4.
    m.box(1, 0, 1, 3, 2, 3, SHOE)
    m.box(0, 1, 1, 4, 2, 3, SHOE)
    m.box(1, 0, 4, 3, 1, 4, SHOE_D)  # forward boot toe
    m.box(1, 0, 1, 3, 0, 3, SHOE_D)  # sole
    m.box(outer_x, 1, 1, outer_x, 2, 3, SHOE_D)
    return m


def parts():
    return {
        "head": head(),
        "torso": torso(),
        "arm_l": arm(3),
        "arm_r": arm(0),
        "leg_l": leg(4),
        "leg_r": leg(0),
    }
