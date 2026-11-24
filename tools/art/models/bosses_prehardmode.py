"""Pre-Hardmode Bosses & Boss Minions (12 rigs, 16 VPU) for the `boss` family."""
import math
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.dirname(HERE))

from vox import Model
from palette import VPU, hexc
from _rigtools import key, out_dir, save_parts, slime_anims, write_rig


def ellipsoid(sx, sy, sz, col, cx=None, cy=None, cz=None, rx=None, ry=None, rz=None):
    m = Model(sx, sy, sz)
    cx = (sx - 1) / 2.0 if cx is None else cx
    cy = (sy - 1) / 2.0 if cy is None else cy
    cz = (sz - 1) / 2.0 if cz is None else cz
    rx = sx / 2.0 if rx is None else rx
    ry = sy / 2.0 if ry is None else ry
    rz = sz / 2.0 if rz is None else rz
    for x in range(sx):
        dx = (x - cx) / rx
        for y in range(sy):
            dy = (y - cy) / ry
            for z in range(sz):
                dz = (z - cz) / rz
                if dx * dx + dy * dy + dz * dz <= 1.0:
                    m.set(x, y, z, col)
    return m


def build_king_slime():
    gel = hexc("#2f8cf0be")
    gel_l = hexc("#64b4ffbe")
    gel_d = hexc("#1b5fb8be")
    body = Model(64, 52, 64)
    cx, cz = 31.5, 31.5
    for x in range(64):
        for y in range(52):
            for z in range(64):
                dx = (x - cx) / 31.0
                dz = (z - cz) / 31.0
                dy = (y - 6.0) / 44.0
                if y < 6:
                    r2 = dx * dx + dz * dz
                    if r2 <= 0.92:
                        body.set(x, y, z, gel_d)
                else:
                    if dx * dx + dy * dy * 1.15 + dz * dz <= 1.0:
                        c = gel_l if y > 38 else (gel_d if y < 12 else gel)
                        body.set(x, y, z, c)
    body.hollow(2)

    # Ninja silhouette inside
    ninja = Model(16, 28, 12)
    cloth = hexc("#1a1c2c")
    cloth_l = hexc("#29366f")
    skin = hexc("#e8b896")
    band = hexc("#b13e53")
    for x in range(4, 12):
        for y in range(0, 10):
            for z in range(4, 8):
                if x not in (7, 8) or y > 5:
                    ninja.set(x, y, z, cloth)
    for x in range(3, 13):
        for y in range(10, 20):
            for z in range(3, 9):
                ninja.set(x, y, z, cloth_l if y > 16 else cloth)
    for x in range(4, 12):
        for y in range(20, 28):
            for z in range(2, 10):
                c = skin if (22 <= y <= 24 and z >= 8 and 5 <= x <= 10) else (band if y == 25 else cloth)
                ninja.set(x, y, z, c)

    # Gold crown
    crown = Model(28, 14, 28)
    gold = hexc("#f4b41b")
    gold_d = hexc("#c48410")
    ruby = hexc("#d92b3a")
    saph = hexc("#2b7de9")
    for x in range(28):
        for z in range(28):
            r = math.hypot(x - 13.5, z - 13.5)
            if 9.5 <= r <= 13.0:
                for y in range(6):
                    crown.set(x, y, z, gold_d if y < 2 else gold)
                ang = math.atan2(z - 13.5, x - 13.5)
                if abs(math.sin(ang * 4)) > 0.75:
                    for y in range(6, 13):
                        crown.set(x, y, z, gold)
    for px, pz, gem in ((14, 1, ruby), (14, 26, ruby), (1, 14, saph), (26, 14, saph)):
        crown.set(px, 3, pz, gem)
        crown.set(px, 4, pz, gem)

    save_parts("king_slime", {"body": body, "ninja": ninja, "crown": crown})
    parts = [
        ("body", (32, 0, 32), (0, 0, 0), None),
        ("ninja", (8, 0, 6), (0, 12, 0), "body"),
        ("crown", (14, 0, 14), (0, 50, 0), "body"),
    ]
    write_rig(out_dir("king_slime"), parts, slime_anims("body", squash=(1.18, 0.78, 1.18), stretch=(0.90, 1.18, 0.90)))


def build_queen_bee():
    gold = hexc("#f4b41b")
    dark = hexc("#241b16")
    amber = hexc("#d97a16")
    eye_c = hexc("#d92b3a")
    wing_c = hexc("#d8f0ffb4")

    body = Model(32, 28, 48)
    for x in range(32):
        for y in range(28):
            for z in range(48):
                dx = (x - 15.5) / 14.0
                dy = (y - 13.5) / 12.5
                dz = (z - 23.5) / 22.5
                if dx * dx + dy * dy + dz * dz <= 1.0:
                    stripe = (z // 6) % 2 == 0
                    c = dark if stripe else (amber if y < 10 else gold)
                    if z > 40 and abs(x - 15.5) > 5 and 12 <= y <= 18:
                        c = eye_c
                    body.set(x, y, z, c)
    # Stinger
    for z in range(0, 5):
        r = z * 0.6
        for x in range(13, 19):
            for y in range(11, 17):
                if math.hypot(x - 15.5, y - 13.5) <= r:
                    body.set(x, y, z, dark)

    wing_l = Model(28, 2, 16)
    for x in range(28):
        for z in range(16):
            if ((x - 14) / 14.0) ** 2 + ((z - 7.5) / 7.5) ** 2 <= 1.0:
                wing_l.set(x, 0, z, wing_c)
                wing_l.set(x, 1, z, wing_c)
    wing_r = Model(28, 2, 16)
    for (x, y, z), c in wing_l.v.items():
        wing_r.set(27 - x, y, z, c)

    save_parts("queen_bee", {"body": body, "wing_l": wing_l, "wing_r": wing_r})
    parts = [
        ("body", (16, 14, 24), (0, 0, 0), None),
        ("wing_l", (0, 1, 8), (14, 10, 4), "body"),
        ("wing_r", (28, 1, 8), (-14, 10, 4), "body"),
    ]
    anims = {
        "idle": (
            0.24,
            True,
            [
                key(0.0, "wing_l", rot=(0, 0, 28)),
                key(0.12, "wing_l", rot=(0, 0, -28)),
                key(0.24, "wing_l", rot=(0, 0, 28)),
                key(0.0, "wing_r", rot=(0, 0, -28)),
                key(0.12, "wing_r", rot=(0, 0, 28)),
                key(0.24, "wing_r", rot=(0, 0, -28)),
            ],
        )
    }
    write_rig(out_dir("queen_bee"), parts, anims)


def build_eater_of_worlds():
    chitin = hexc("#5a2d7a")
    chitin_d = hexc("#3a1b52")
    pustule = hexc("#8cd943")
    eye_c = hexc("#c8e636")
    fang = hexc("#e8e0c8")

    # Head + mandibles
    head = ellipsoid(32, 32, 32, chitin)
    rng = random.Random(101)
    for x in range(32):
        for y in range(32):
            for z in range(32):
                if (x, y, z) in head.v:
                    if z > 24 and (abs(x - 10) <= 2 or abs(x - 21) <= 2) and 14 <= y <= 20:
                        head.set(x, y, z, eye_c)
                    elif rng.random() < 0.08:
                        head.set(x, y, z, pustule if rng.random() < 0.4 else chitin_d)

    mand_l = Model(8, 8, 14)
    for z in range(14):
        w = max(1, int(3 - z * 0.18))
        for x in range(4 - w, 4 + w):
            for y in range(4 - w, 4 + w):
                mand_l.set(x, y, z, fang if z > 9 else chitin_d)
    mand_r = Model(8, 8, 14)
    for (x, y, z), c in mand_l.v.items():
        mand_r.set(7 - x, y, z, c)

    save_parts("eater_of_worlds_head", {"head": head, "mandible_l": mand_l, "mandible_r": mand_r})
    write_rig(
        out_dir("eater_of_worlds_head"),
        [
            ("head", (16, 16, 16), (0, 0, 0), None),
            ("mandible_l", (4, 4, 0), (-10, -4, 15), "head"),
            ("mandible_r", (4, 4, 0), (10, -4, 15), "head"),
        ],
        {
            "idle": (
                0.8,
                True,
                [
                    key(0.0, "mandible_l", rot=(0, 12, 0)),
                    key(0.4, "mandible_l", rot=(0, -18, 0)),
                    key(0.8, "mandible_l", rot=(0, 12, 0)),
                    key(0.0, "mandible_r", rot=(0, -12, 0)),
                    key(0.4, "mandible_r", rot=(0, 18, 0)),
                    key(0.8, "mandible_r", rot=(0, -12, 0)),
                ],
            )
        },
    )

    # Body
    body = ellipsoid(30, 30, 30, chitin)
    for (x, y, z) in list(body.v):
        if rng.random() < 0.10:
            body.set(x, y, z, pustule if rng.random() < 0.35 else chitin_d)
    save_parts("eater_of_worlds_body", {"body": body})
    write_rig(
        out_dir("eater_of_worlds_body"),
        [("body", (15, 15, 15), (0, 0, 0), None)],
        {"idle": (1.0, True, [key(0.0, "body", rot=(0, 0, 0)), key(0.5, "body", rot=(0, 0, 6)), key(1.0, "body", rot=(0, 0, 0))])},
    )

    # Tail
    tail = Model(26, 26, 32)
    for z in range(32):
        r = 12.0 * (1.0 - z / 34.0)
        for x in range(26):
            for y in range(26):
                if math.hypot(x - 12.5, y - 12.5) <= r:
                    tail.set(x, y, z, chitin_d if z > 24 else chitin)
    save_parts("eater_of_worlds_tail", {"tail": tail})
    write_rig(
        out_dir("eater_of_worlds_tail"),
        [("tail", (13, 13, 16), (0, 0, 0), None)],
        {"idle": (1.0, True, [key(0.0, "tail", rot=(0, 0, 0)), key(0.5, "tail", rot=(0, 0, -8)), key(1.0, "tail", rot=(0, 0, 0))])},
    )


def _build_skull(kind, bone, bone_d, socket_c, fang_c, crack=False):
    cranium = ellipsoid(56, 52, 56, bone)
    # Hollow 2-voxel shell
    cranium.hollow(2)
    # Carve eye sockets and nose
    for x in range(56):
        for y in range(52):
            for z in range(36, 56):
                if (math.hypot(x - 17, y - 28) <= 7.0 or math.hypot(x - 38, y - 28) <= 7.0):
                    cranium.v.pop((x, y, z), None)
                    if z == 46:
                        cranium.set(x, y, z, socket_c)
                if abs(x - 27.5) <= 2.5 and 14 <= y <= 21 and z >= 46:
                    cranium.v.pop((x, y, z), None)
    if crack:
        rng = random.Random(77)
        for (x, y, z) in list(cranium.v):
            if rng.random() < 0.06:
                cranium.set(x, y, z, bone_d)

    jaw = Model(36, 14, 32)
    for x in range(36):
        for y in range(14):
            for z in range(32):
                if (x < 6 or x >= 30 or z >= 24) and y < 10:
                    jaw.set(x, y, z, bone_d if y < 3 else bone)
    for tx in range(8, 29, 4):
        for y in range(10, 14):
            jaw.set(tx, y, 28, fang_c)
            jaw.set(tx, y, 29, fang_c)

    save_parts(kind, {"cranium": cranium, "jaw": jaw})
    parts = [
        ("cranium", (28, 26, 28), (0, 0, 0), None),
        ("jaw", (18, 10, 16), (0, -22, 6), "cranium"),
    ]
    anims = {
        "idle": (
            1.6,
            True,
            [
                key(0.0, "jaw", rot=(0, 0, 0)),
                key(0.8, "jaw", rot=(14, 0, 0)),
                key(1.6, "jaw", rot=(0, 0, 0)),
            ],
        )
    }
    write_rig(out_dir(kind), parts, anims)


def build_skeletron_and_guardian():
    _build_skull(
        "skeletron_head",
        bone=hexc("#ebe3cf"),
        bone_d=hexc("#c8bea6"),
        socket_c=hexc("#140e0c"),
        fang_c=hexc("#f5f0e2"),
        crack=False,
    )
    _build_skull(
        "dungeon_guardian",
        bone=hexc("#c4baa4"),
        bone_d=hexc("#6e6556"),
        socket_c=hexc("#e62424"),
        fang_c=hexc("#e8dfc8"),
        crack=True,
    )

    # Skeletron hand
    hand = Model(28, 36, 16)
    bone = hexc("#ebe3cf")
    bone_d = hexc("#c8bea6")
    for x in range(6, 22):
        for y in range(12, 28):
            for z in range(4, 12):
                hand.set(x, y, z, bone)
    for fx in (6, 10, 14, 18):
        for y in range(0, 12):
            for z in range(5, 11):
                hand.set(fx, y, z, bone_d if y < 4 else bone)
                hand.set(fx + 1, y, z, bone_d if y < 4 else bone)
    save_parts("skeletron_hand", {"hand": hand})
    write_rig(
        out_dir("skeletron_hand"),
        [("hand", (14, 18, 8), (0, 0, 0), None)],
        {"idle": (1.2, True, [key(0.0, "hand", rot=(0, 0, -6)), key(0.6, "hand", rot=(0, 0, 6)), key(1.2, "hand", rot=(0, 0, -6))])},
    )


def build_wall_of_flesh():
    flesh = hexc("#9e222e")
    flesh_d = hexc("#631018")
    sinew = hexc("#c8464e")
    rib = hexc("#dcd2be")
    tooth = hexc("#f2ecd8")
    throat = hexc("#1a0508")
    eye_w = hexc("#f0dcd2")
    iris = hexc("#c42026")
    pupil = hexc("#0a0a0a")

    # wall_slab.vox: 96 x 128 x 24 (<= 128 per axis), hollowed(1), tiled 4x8 for 24x64 blocks (384x1024 voxels)
    slab = Model(96, 128, 24)
    rng = random.Random(666)
    for x in range(96):
        for y in range(128):
            for z in range(24):
                if z < 20 or rng.random() < 0.85:
                    c = flesh
                    if (y % 32) in (14, 15, 16) and z > 16 and (x % 24) < 16:
                        c = rib
                    elif rng.random() < 0.15:
                        c = sinew if rng.random() < 0.5 else flesh_d
                    slab.set(x, y, z, c)
    slab.hollow(1)

    maw_u = Model(48, 24, 32)
    for x in range(48):
        for y in range(24):
            for z in range(32):
                if y >= 6:
                    maw_u.set(x, y, z, throat if z < 16 else flesh)
                elif z >= 22 and x % 6 < 3:
                    maw_u.set(x, y, z, tooth)
    maw_l = Model(48, 24, 32)
    for x in range(48):
        for y in range(24):
            for z in range(32):
                if y < 18:
                    maw_l.set(x, y, z, throat if z < 16 else flesh_d)
                elif z >= 22 and (x + 3) % 6 < 3:
                    maw_l.set(x, y, z, tooth)

    tendril = Model(9, 9, 32)
    for z in range(32):
        cx = 4.0 + math.sin(z * 0.25) * 1.2
        r = 2.2 - z * 0.03
        for x in range(9):
            for y in range(9):
                if math.hypot(x - cx, y - 4.0) <= r:
                    tendril.set(x, y, z, sinew if z % 3 == 0 else flesh_d)

    save_parts(
        "wall_of_flesh_mouth",
        {"wall_slab": slab, "maw_upper": maw_u, "maw_lower": maw_l, "tendril": tendril},
    )
    parts = [
        ("maw_upper", "maw_upper.vox", (24, 0, 16), (0, 12, 16), None),
        ("maw_lower", "maw_lower.vox", (24, 24, 16), (0, -12, 16), None),
        ("tendril", "tendril.vox", (4, 4, 0), (0, 0, 20), None),
    ]
    for ix in range(4):
        for iy in range(8):
            ox = (ix - 1.5) * 96
            oy = (iy - 3.5) * 128
            parts.append((f"wall_slab_{ix}_{iy}", "wall_slab.vox", (48, 64, 16), (ox, oy, -16), None))
    anims = {
        "idle": (
            0.9,
            True,
            [
                key(0.0, "maw_upper", rot=(-12, 0, 0)),
                key(0.45, "maw_upper", rot=(8, 0, 0)),
                key(0.9, "maw_upper", rot=(-12, 0, 0)),
                key(0.0, "maw_lower", rot=(12, 0, 0)),
                key(0.45, "maw_lower", rot=(-8, 0, 0)),
                key(0.9, "maw_lower", rot=(12, 0, 0)),
                key(0.0, "tendril", rot=(0, -15, 0)),
                key(0.45, "tendril", rot=(0, 15, 0)),
                key(0.9, "tendril", rot=(0, -15, 0)),
            ],
        )
    }
    write_rig(out_dir("wall_of_flesh_mouth"), parts, anims)

    # Wall of Flesh Eye
    eye = ellipsoid(40, 40, 40, eye_w)
    for x in range(40):
        for y in range(40):
            for z in range(32, 40):
                if (x, y, z) in eye.v:
                    d = math.hypot(x - 19.5, y - 19.5)
                    if d < 5.0:
                        eye.set(x, y, z, pupil)
                    elif d < 11.0:
                        eye.set(x, y, z, iris)
    stalk = Model(13, 13, 28)
    for z in range(28):
        cx = 6.0 + math.sin(z * 0.2) * 1.0
        r = 4.2 - z * 0.04
        for x in range(13):
            for y in range(13):
                if math.hypot(x - cx, y - 6.0) <= r:
                    stalk.set(x, y, z, sinew if z % 3 == 0 else flesh)
    save_parts("wall_of_flesh_eye", {"eye": eye, "stalk": stalk})
    write_rig(
        out_dir("wall_of_flesh_eye"),
        [("stalk", (6, 6, 0), (0, 0, 0), None), ("eye", (20, 20, 20), (0, 0, 28), "stalk")],
        {"idle": (1.4, True, [key(0.0, "eye", rot=(0, -14, 0)), key(0.7, "eye", rot=(0, 14, 0)), key(1.4, "eye", rot=(0, -14, 0))])},
    )

    # The Hungry
    h_head = ellipsoid(22, 18, 22, flesh)
    h_jaw = Model(18, 8, 18)
    for x in range(18):
        for y in range(8):
            for z in range(18):
                if y < 5:
                    h_jaw.set(x, y, z, flesh_d)
                elif z >= 14 and x % 4 < 2:
                    h_jaw.set(x, y, z, tooth)
    save_parts("the_hungry", {"head": h_head, "jaw": h_jaw})
    write_rig(
        out_dir("the_hungry"),
        [("head", (11, 9, 11), (0, 0, 0), None), ("jaw", (9, 6, 9), (0, -8, 2), "head")],
        {"idle": (0.6, True, [key(0.0, "jaw", rot=(0, 0, 0)), key(0.3, "jaw", rot=(24, 0, 0)), key(0.6, "jaw", rot=(0, 0, 0))])},
    )

    # Leech
    l_head = ellipsoid(16, 16, 20, flesh_d)
    for x in range(5, 11):
        for y in range(5, 11):
            l_head.set(x, y, 19, tooth if (x + y) % 2 == 0 else throat)
    l_tail = Model(12, 12, 20)
    for z in range(20):
        r = 5.5 * (1.0 - z / 22.0)
        for x in range(12):
            for y in range(12):
                if math.hypot(x - 5.5, y - 5.5) <= r:
                    l_tail.set(x, y, z, flesh)
    save_parts("leech", {"head": l_head, "tail": l_tail})
    write_rig(
        out_dir("leech"),
        [("head", (8, 8, 10), (0, 0, 0), None), ("tail", (6, 6, 0), (0, 0, -10), "head")],
        {"idle": (0.8, True, [key(0.0, "tail", rot=(0, 15, 0)), key(0.4, "tail", rot=(0, -15, 0)), key(0.8, "tail", rot=(0, 15, 0))])},
    )


def main():
    build_king_slime()
    build_queen_bee()
    build_eater_of_worlds()
    build_skeletron_and_guardian()
    build_wall_of_flesh()


if __name__ == "__main__":
    main()
