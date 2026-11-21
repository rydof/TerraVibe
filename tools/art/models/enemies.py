"""Original enemy and town NPC rigs, including the corruption's Eater of Souls.

Enemies are authored at 16 voxels per block (VPU) in game axes (x right, y up,
z front). The guide and merchant reuse the shared player base (`_player_base.py`),
so they get the same 32-voxel body; the merchant's hat raises its head model.
"""
import math
import os
import random

from _player_base import *
from _rigtools import key, surface, tapered, veins, write_rig


def out(kind):
    return os.path.join(ASSETS, "models", kind)


def recolor(m, mp):
    n = Model(*m.size)
    for k, c in m.v.items():
        n.v[k] = mp.get(c, c)
    return n


def save_parts(kind, parts):
    for name, m in parts.items():
        m.save(os.path.join(out(kind), name + ".vox"))


# ---- slime ------------------------------------------------------------------
# Pale translucent gel; the client tints it blue (or green) and blends it.
GEL_A = 190  # palette alpha ~0.75
GEL_TOP = hexc("#eaf3ff", GEL_A)
GEL_BOT = hexc("#b9cdec", GEL_A)
GEL_HI = hexc("#ffffff")  # opaque gloss, not blended
EYE_DARK = hexc("#0b1030")
NUCLEUS = hexc("#2b4a8f")
NUCLEUS_L = hexc("#5a86d8")


def slime():
    body = Model(24, 18, 20)
    body.ellipsoid(11.5, 0, 9.5, 12, 18, 10, GEL_TOP)
    for (x, y, z) in list(body.v):
        if y < 5:
            body.v[(x, y, z)] = GEL_BOT
    # Exactly three opaque gloss voxels, stepped down the upper front.
    for gx, gy in ((9, 14), (11, 15), (13, 16)):
        zs = [z for (x, y, z) in body.v if x == gx and y == gy]
        if zs:
            body.set(gx, gy, max(zs), GEL_HI)
    # Two 2x3x1 eye patches inside the gel, seen through the alpha.
    eyes = Model(18, 3, 1)
    for x0 in (2, 16):
        eyes.box(x0, 0, 0, x0 + 1, 2, 0, EYE_DARK)
    nucleus = Model(6, 6, 6)
    nucleus.sphere(2.5, 2.5, 2.5, 2.8, NUCLEUS)
    nucleus.sphere(2.5, 2.5, 2.5, 1.6, NUCLEUS_L)
    save_parts("slime", {"body": body, "eyes": eyes, "nucleus": nucleus})
    squash = (1.22, 0.74, 1.22)
    stretch = (0.88, 1.22, 0.88)
    write_rig(
        out("slime"),
        [
            ("body", (11, 0, 9), (0, 0, 0), None),
            ("eyes", (9, 0, 0), (0, 6, 7), "body"),
            ("nucleus", (3, 3, 3), (0, 6, 0), "body"),
        ],
        {
            "idle": (1.2, True, [
                key(0.0, "body", scale=(1, 1, 1)),
                key(0.6, "body", scale=(1.05, 0.92, 1.05)),
            ]),
            "hop": (0.8, True, [
                key(0.0, "body", scale=(1, 1, 1)),
                key(0.1, "body", scale=squash),
                key(0.3, "body", scale=stretch),
                key(0.8, "body", scale=(1, 1, 1)),
            ]),
        },
    )


# ---- demon eye ----------------------------------------------------------------
SCLERA = hexc("#efe6e0")
SCLERA_D = hexc("#d6c8c4")
VEIN = hexc("#c4333a")
IRIS = hexc("#a5381e")
IRIS_D = hexc("#7a2515")
PUPIL = hexc("#0a0a0a")
NERVE = hexc("#b3222a")
NERVE_L = hexc("#d9494f")
NERVE_D = hexc("#80161d")


def demon_eye():
    rng = random.Random(7)
    ball = Model(24, 24, 24)
    ball.sphere(11.5, 11.5, 11.5, 12, SCLERA)
    for p in list(ball.v):
        if p[1] < 9:
            ball.v[p] = SCLERA_D

    def iris_zone(p):
        return p[2] >= 20 and math.hypot(p[0] - 11.5, p[1] - 11.5) < 8.0

    veins(ball, rng, 22, 9, iris_zone)
    # A few deterministic surface veins guarantee the ball reads red even when
    # the random walk stays on the back.
    marked = 0
    for p in sorted(surface(ball, None)):
        if marked == 4:
            break
        if not iris_zone(p) and ball.v[p] in (SCLERA, SCLERA_D):
            ball.v[p] = VEIN
            marked += 1
    for p in list(ball.v):
        d = math.hypot(p[0] - 11.5, p[1] - 11.5)
        if p[2] >= 20 and d <= 7.0 and ball.v[p] in (SCLERA, SCLERA_D, VEIN):
            ball.v[p] = PUPIL if d <= 2.6 else IRIS if d <= 4.6 else IRIS_D
    t1 = tapered([10, 10, 8, 8, 6, 6, 4, 4], [NERVE, NERVE_L], 10, 16)
    t2 = tapered([6, 6, 5, 5, 4, 4, 3, 3], [NERVE_D, NERVE], 6, 16)
    save_parts("demon_eye", {"ball": ball, "tail1": t1, "tail2": t2})
    write_rig(
        out("demon_eye"),
        [
            ("ball", (12, 12, 12), (0, 16, 0), None),
            ("tail1", (5, 5, 16), (0, 0, -12), "ball"),
            ("tail2", (3, 3, 16), (0, 0, -16), "tail1"),
        ],
        {
            "idle": (1.0, True, [
                key(0.0, "tail1", (0, 22, 0)),
                key(0.5, "tail1", (0, -22, 0)),
                key(0.0, "tail2", (0, -35, 0)),
                key(0.5, "tail2", (0, 35, 0)),
                key(0.0, "ball", off=(0, 0, 0)),
                key(0.5, "ball", off=(0, 1.2, 0)),
            ]),
        },
    )


# ---- eater of souls -----------------------------------------------------------
def eater_of_souls():
    # Corrupted flying worm head: banded violet hide, an open front maw ringed
    # with white teeth, amber eyes, and scalloped bat membranes.
    hide, shade = hexc("#79518f"), hexc("#432c5e")
    ridge, membrane = hexc("#a27bb3"), hexc("#563477")
    bone = hexc("#ded2b2")
    dark, eye = hexc("#1b142c"), hexc("#efba52")
    head = Model(32, 38, 28)
    # Segmented barrel: rounded at the back, broad and nearly flat at the front.
    for z in range(28):
        if z == 0:
            f = 0.72
        elif z == 1:
            f = 0.84
        elif z == 2:
            f = 0.92
        elif z <= 19:
            f = 1.0
        else:
            f = 1.0 - 0.02 * (z - 19)
        rx, ry = 15.0 * f, 17.0 * f
        band = (hide, shade, ridge)[(z // 4) % 3]
        for x in range(32):
            for y in range(38):
                if ((x - 15.5) / rx) ** 2 + ((y - 19) / ry) ** 2 <= 1.0:
                    head.set(x, y, z, band)
    # Open maw: a cylindrical cavity through the front, with a dark throat wall.
    for z in range(17, 28):
        for x in range(32):
            for y in range(38):
                if math.hypot(x - 15.5, y - 19) <= 7.0:
                    head.set(x, y, z, None)
    for x in range(32):
        for y in range(38):
            if math.hypot(x - 15.5, y - 19) <= 6.0:
                head.set(x, y, 16, dark)
    # Eight white 2-voxel teeth around the rim of the maw.
    for k in range(8):
        a = k * math.pi / 4
        tx = round(15.5 + 8.6 * math.cos(a))
        ty = round(19 + 8.6 * math.sin(a))
        head.box(tx, ty, 27, tx, ty + 1, 27, bone)
    # Amber eyes above the maw.
    for ex in (9, 20):
        head.box(ex, 27, 27, ex + 2, 29, 27, dark)
        head.box(ex + 1, 28, 27, ex + 1, 28, 27, eye)

    wing_r = Model(24, 24, 8)
    for x in range(24):
        top = round(8 + min(x, 12) * 0.9 - max(0, x - 12) * 0.8)
        # Three pointed fingers with concave membrane edges between them.
        bottom = max(0, top - round(4 + 12 * abs(math.cos(x * math.pi / 9))))
        z = max(0, 4 - x // 3)
        wing_r.box(x, bottom, z, x, top, z + 1, membrane)
        wing_r.box(x, top, z, x, top, z + 1, ridge)
        if x in (0, 10, 20):
            wing_r.box(x, bottom, z + 1, x, top, z + 1, shade)
    wing_l = Model(*wing_r.size)
    for (x, y, z), c in wing_r.v.items():
        wing_l.set(23 - x, y, z, c)
    save_parts("eater_of_souls", {"head": head, "wing_l": wing_l, "wing_r": wing_r})
    write_rig(
        out("eater_of_souls"),
        [
            ("head", (15, 14, 13), (0, 16, 0), None),
            ("wing_l", (22, 12, 4), (-10, 0, 0), "head"),
            ("wing_r", (0, 12, 4), (10, 0, 0), "head"),
        ],
        {"idle": (0.6, True, [
            key(0.0, "wing_l", (0, 0, -30)),
            key(0.3, "wing_l", (0, 0, 35)),
            key(0.0, "wing_r", (0, 0, 30)),
            key(0.3, "wing_r", (0, 0, -35)),
            key(0.0, "head", off=(0, 0, 0)),
            key(0.3, "head", off=(0, 1.0, 0)),
        ])},
    )


# ---- humanoids ------------------------------------------------------------------
def humanoid_anims(arm_base=0.0, arm_swing=35.0, leg_swing=35.0, walk_len=0.6,
                   idle_bob=0.8, walk_bob=1.0):
    bobbed = ("torso", "head", "arm_l", "arm_r")
    idle = []
    for t, dy in ((0.0, 0.0), (1.0, idle_bob)):
        idle += [key(t, p, (arm_base, 0, 0) if p.startswith("arm") else (0, 0, 0), (0, dy, 0)) for p in bobbed]
    walk = []
    n = walk_len / 4
    for i, (s, dy) in enumerate(((1, 0.0), (0, walk_bob), (-1, 0.0), (0, walk_bob))):
        t = round(i * n, 3)
        walk += [key(t, "torso", off=(0, dy, 0)), key(t, "head", off=(0, dy, 0))]
        walk += [
            key(t, "arm_l", (arm_base + arm_swing * s, 0, 0), (0, dy, 0)),
            key(t, "arm_r", (arm_base - arm_swing * s, 0, 0), (0, dy, 0)),
            key(t, "leg_l", (-leg_swing * s, 0, 0)),
            key(t, "leg_r", (leg_swing * s, 0, 0)),
        ]
    return {"idle": (2.0, True, idle), "walk": (walk_len, True, walk)}


def write_humanoid(kind, ps, anims):
    save_parts(kind, ps)
    names = ("torso", "head", "arm_l", "arm_r", "leg_l", "leg_r")
    write_rig(out(kind), [(n, PIVOTS[n], OFFSETS[n], None) for n in names], anims)


def zombie():
    ZS, ZS_D = hexc("#7d9d6a"), hexc("#5d7a52")
    HAIR, HAIR_D, HAIR_L = hexc("#4a3b2a"), hexc("#2f251a"), hexc("#5e4b36")
    SHIRT, SHIRT_D = hexc("#5f7a8f"), hexc("#46596b")
    PANTS, PANTS_D = hexc("#5a4f6e"), hexc("#403850")
    BONE, BONE_D = hexc("#ded2b2"), hexc("#b8a98a")
    EYE_Y, EYE_R = hexc("#e8e8b0"), hexc("#8a1b1b")
    SHOE = hexc("#3d2f2a")

    head = Model(12, 12, 12)
    head.box(0, 0, 0, 11, 11, 11, ZS)
    head.box(0, 0, 0, 0, 11, 11, ZS_D)
    for x in (0, 11):
        for z in (0, 11):
            for y in range(12):
                head.set(x, y, z, None)  # rounded silhouette
    head.box(0, 9, 0, 11, 11, 11, HAIR)  # matted crown
    head.box(0, 8, 0, 11, 8, 2, HAIR_D)
    head.box(2, 9, 11, 9, 10, 11, HAIR_D)
    head.box(4, 10, 11, 7, 10, 11, HAIR_L)
    head.box(0, 9, 3, 0, 11, 9, HAIR)
    head.box(11, 9, 3, 11, 11, 9, HAIR_L)
    # Sunken red/yellow eyes and a lipless mouth with one tooth.
    head.box(2, 6, 11, 3, 7, 11, EYE_Y)
    head.box(8, 6, 11, 9, 7, 11, EYE_Y)
    head.set(3, 7, 11, EYE_R)
    head.set(8, 6, 11, EYE_R)
    head.box(4, 4, 11, 7, 4, 11, HAIR_D)
    head.box(5, 4, 11, 6, 4, 11, BONE)
    # Exposed skull on the temple and cheek.
    head.box(0, 6, 4, 0, 9, 8, BONE)
    head.set(1, 6, 11, BONE_D)

    torso = Model(8, 12, 6)
    torso.box(0, 0, 0, 7, 11, 5, SHIRT)
    torso.box(0, 0, 0, 7, 3, 5, PANTS)
    torso.box(0, 4, 0, 1, 11, 5, SHIRT_D)
    torso.box(0, 0, 0, 1, 3, 5, PANTS_D)
    torso.box(0, 4, 0, 7, 11, 0, SHIRT_D)
    torso.box(0, 0, 0, 7, 3, 0, PANTS_D)
    # Torn shirt exposing ribs and a strip of belly.
    torso.box(2, 5, 5, 5, 5, 5, BONE)
    torso.box(2, 7, 5, 5, 7, 5, BONE)
    torso.set(3, 6, 5, BONE_D)
    torso.box(3, 1, 5, 4, 2, 5, ZS)
    # Ragged hem: the bottom row is missing cells and two corners are torn away.
    for x, z in ((0, 0), (1, 3), (3, 0), (5, 5), (6, 2), (7, 4)):
        torso.set(x, 0, z, None)
    for x in (0, 6):
        for z in range(6):
            torso.set(x, 0, z, None)

    def arm(outer):
        m = Model(4, 12, 4)
        m.box(0, 7, 0, 3, 11, 3, SHIRT)
        m.box(outer, 7, 0, outer, 11, 3, SHIRT_D)
        m.box(0, 7, 0, 3, 7, 0, SHIRT_D)
        m.box(0, 2, 0, 3, 6, 3, ZS)
        m.box(outer, 2, 0, outer, 6, 3, ZS_D)
        m.box(0, 0, 0, 3, 1, 3, ZS)
        # Torn sleeve with bone showing at the elbow.
        m.set(1, 7, 0, None)
        m.set(2, 7, 3, None)
        m.box(1, 5, 3, 2, 6, 3, BONE)
        return m

    def leg(outer):
        m = Model(4, 8, 6)
        m.box(0, 3, 0, 3, 7, 4, PANTS)
        m.box(outer, 3, 0, outer, 7, 4, PANTS_D)
        m.box(0, 3, 0, 3, 3, 0, PANTS_D)
        m.box(0, 0, 0, 3, 2, 4, ZS)  # bare shin
        m.box(0, 0, 0, 3, 0, 5, SHOE)  # boot with a toe
        m.box(1, 1, 4, 2, 2, 4, BONE)  # exposed ankle bone
        m.set(0, 3, 0, None)
        m.set(3, 3, 4, None)
        return m

    save_parts("zombie", {
        "head": head,
        "torso": torso,
        "arm_l": arm(3),
        "arm_r": arm(0),
        "leg_l": leg(3),
        "leg_r": leg(0),
    })
    write_rig(
        out("zombie"),
        [
            ("torso", (4, 0, 3), (0, 8, 0), None),
            ("head", (6, 0, 6), (0, 20, 0), None),
            ("arm_l", (2, 12, 2), (6, 20, 0), None),
            ("arm_r", (2, 12, 2), (-6, 20, 0), None),
            ("leg_l", (2, 8, 2), (2, 8, 0), None),
            ("leg_r", (2, 8, 2), (-2, 8, 0), None),
        ],
        humanoid_anims(arm_base=-80, arm_swing=8, leg_swing=25, walk_len=0.8,
                       idle_bob=0.8, walk_bob=1.0),
    )


def guide():
    HAIR_G, HAIR_GD, HAIR_GL = hexc("#b8874b"), hexc("#8a5f2e"), hexc("#d4a463")
    STRAP = hexc("#6a4522")
    mp = {
        HAIR: HAIR_G, HAIR_D: HAIR_GD, HAIR_L: HAIR_GL,
        SHIRT: hexc("#5f8a3a"), SHIRT_D: hexc("#476b2a"),
        PANTS: hexc("#8a6a3c"), PANTS_D: hexc("#6a4e2a"),
        SHOE: hexc("#4a3018"),
    }
    ps = {n: recolor(m, mp) for n, m in parts().items()}
    t = ps["torso"]
    for k in range(5):
        t.set(3 + k, 5 + k, 5, STRAP)  # diagonal leather strap
        t.set(7 - k, 5 + k, 0, STRAP)  # back quiver strap
    t.box(4, 3, 5, 7, 3, 5, STRAP)     # belt buckle
    write_humanoid("guide", ps, humanoid_anims())


def merchant():
    COAT, COAT_D = hexc("#58698a"), hexc("#434f69")
    GREY, GREY_D = hexc("#c9c9d0"), hexc("#9c9ca8")
    BEARD, BEARD_L = hexc("#f4f4f4"), hexc("#d6d6dc")
    HAT, HAT_D = hexc("#8a5a2e"), hexc("#684220")
    GOLD = hexc("#d8b038")
    mp = {
        HAIR: GREY, HAIR_D: GREY_D, HAIR_L: BEARD,
        SHIRT: COAT, SHIRT_D: COAT_D,
        PANTS: hexc("#6e6e7a"), PANTS_D: hexc("#52525e"),
        SHOE: hexc("#3a2a1e"),
    }
    ps = {n: recolor(m, mp) for n, m in parts().items()}
    base = ps["head"]
    head = Model(12, 20, 12)
    head.v.update(base.v)
    # Sculpted white beard over chin & cheeks.
    head.box(2, 1, 11, 9, 4, 11, BEARD)
    head.box(3, 0, 11, 8, 1, 11, BEARD_L)
    head.box(4, 5, 11, 7, 5, 11, BEARD)  # mustache
    head.box(1, 2, 9, 1, 5, 10, BEARD_L)
    head.box(10, 2, 9, 10, 5, 10, BEARD_L)
    # Tapered wizard/merchant hat with chamfered brim and gold band.
    head.box(1, 11, 1, 10, 11, 10, HAT_D)
    head.box(0, 12, 1, 11, 12, 10, HAT)
    head.box(1, 12, 0, 10, 12, 11, HAT)
    head.box(2, 13, 2, 9, 15, 9, HAT)
    head.box(2, 13, 2, 9, 13, 9, GOLD)  # hat band
    head.box(3, 16, 3, 8, 18, 8, HAT)
    head.box(4, 19, 4, 7, 19, 7, HAT_D)
    ps["head"] = head
    t = ps["torso"]
    t.box(2, 0, 0, 9, 3, 5, COAT_D)  # long coat tail
    t.set(5, 5, 5, GOLD)              # coat buttons
    t.set(5, 7, 5, GOLD)
    t.box(3, 4, 0, 8, 8, 0, HAT)      # merchant pack on back
    write_humanoid("merchant", ps, humanoid_anims())


def arms_dealer():
    SKIN_A, SKIN_AD = hexc("#9c6342"), hexc("#7a4a30")
    HAIR_A, HAIR_AD, HAIR_AL = hexc("#1f1b24"), hexc("#121016"), hexc("#352f3d")
    COAT, COAT_D = hexc("#4a4f5e"), hexc("#313542")
    BAND = hexc("#c89d28")
    SHADES = hexc("#11131a")
    mp = {
        SKIN: SKIN_A, SKIN_D: SKIN_AD,
        HAIR: HAIR_A, HAIR_D: HAIR_AD, HAIR_L: HAIR_AL,
        SHIRT: COAT, SHIRT_D: COAT_D,
        PANTS: hexc("#2b2930"), PANTS_D: hexc("#1c1b21"),
        SHOE: hexc("#241812"), SHOE_D: hexc("#140d0a"),
    }
    ps = {n: recolor(m, mp) for n, m in parts().items()}
    # Cool dark shades across the eyes.
    h = ps["head"]
    h.box(2, 6, 11, 4, 7, 11, SHADES)
    h.box(7, 6, 11, 9, 7, 11, SHADES)
    h.box(5, 7, 11, 6, 7, 11, SHADES)
    t = ps["torso"]
    t.box(2, 0, 0, 9, 3, 5, COAT_D)
    for k in range(5):
        t.set(3 + k, 4 + k, 5, BAND)
    write_humanoid("arms_dealer", ps, humanoid_anims())


slime()
demon_eye()
eater_of_souls()
zombie()
guide()
merchant()
arms_dealer()
