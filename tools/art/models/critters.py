"""16-VPU bespoke voxel rigs for the 4 ambient critters:
bunny, bird, squirrel, cave_critter.
"""
from vox import Model
from palette import hexc
from _rigtools import out_dir, save_parts, mirror_x, write_rig, key


def build_bunny():
    fur = hexc("#f4f2ee")
    fur_hi = hexc("#ffffff")
    fur_sh = hexc("#d8d4cc")
    belly = hexc("#ebe6df")
    ear_pink = hexc("#f2a6b3")
    nose = hexc("#f08090")
    eye = hexc("#1e1a24")

    # Body: 10x8x12 (x: width, y: height, z: +front/-back)
    body = Model(10, 8, 12)
    body.ellipsoid(4.5, 3.5, 5.5, 4.5, 3.3, 5.0, fur)
    for (x, y, z) in list(body.v):
        if y >= 5:
            body.v[(x, y, z)] = fur_hi
        elif y <= 1:
            body.v[(x, y, z)] = belly
        elif z <= 2:
            body.v[(x, y, z)] = fur_sh
    # Cotton puff tail at back (z=0..2, y=3..5)
    body.ellipsoid(4.5, 4.0, 1.0, 1.8, 1.8, 1.5, fur_hi)

    # Head: 8x14x8 (includes tall ears)
    head = Model(8, 14, 8)
    head.ellipsoid(3.5, 3.2, 3.8, 3.5, 3.0, 3.6, fur)
    for (x, y, z) in list(head.v):
        if y >= 5 and y <= 6:
            head.v[(x, y, z)] = fur_hi
        elif y <= 1:
            head.v[(x, y, z)] = fur_sh
    # Tall ears (x=1..2 and x=5..6, y=6..13, z=1..3)
    for ex in (1, 5):
        head.box(ex, 6, 1, ex + 1, 13, 2, fur)
        head.box(ex, 7, 2, ex + 1, 12, 2, ear_pink)
        head.set(ex, 13, 1, fur_hi)
    # Eyes and pink nose
    head.set(0, 4, 5, eye)
    head.set(0, 3, 5, eye)
    head.set(7, 4, 5, eye)
    head.set(7, 3, 5, eye)
    head.box(3, 2, 7, 4, 3, 7, nose)

    # Front legs: 3x4x3
    leg_fl = Model(3, 4, 3)
    leg_fl.box(0, 0, 0, 2, 3, 2, fur)
    leg_fl.box(0, 0, 0, 2, 0, 2, fur_sh)
    leg_fr = mirror_x(leg_fl)

    # Back legs: 3x4x5 (longer hind feet)
    leg_bl = Model(3, 4, 5)
    leg_bl.box(0, 1, 0, 2, 3, 2, fur)
    leg_bl.box(0, 0, 0, 2, 0, 4, fur_sh)
    leg_br = mirror_x(leg_bl)

    save_parts(
        "bunny",
        {
            "body": body,
            "head": head,
            "leg_fl": leg_fl,
            "leg_fr": leg_fr,
            "leg_bl": leg_bl,
            "leg_br": leg_br,
        },
    )

    parts = [
        ("body", (5.0, 4.0, 6.0), (0.0, 0.0, 0.0), None),
        ("head", (4.0, 2.0, 4.0), (0.0, 3.5, 4.5), "body"),
        ("leg_fl", (1.5, 3.5, 1.5), (-3.0, -4.0, 3.5), "body"),
        ("leg_fr", (1.5, 3.5, 1.5), (3.0, -4.0, 3.5), "body"),
        ("leg_bl", (1.5, 3.5, 1.5), (-3.2, -4.0, -3.0), "body"),
        ("leg_br", (1.5, 3.5, 1.5), (3.2, -4.0, -3.0), "body"),
    ]
    anims = {
        "idle": (
            2.0,
            True,
            [
                key(0.0, "head", rot=(0, 0, 0)),
                key(1.0, "head", rot=(-6, 0, 0), off=(0, 0.3, 0)),
                key(2.0, "head", rot=(0, 0, 0)),
            ],
        ),
        "hop": (
            0.45,
            True,
            [
                key(0.0, "body", off=(0, 0, 0), rot=(0, 0, 0)),
                key(0.22, "body", off=(0, 3.0, 0), rot=(-12, 0, 0)),
                key(0.45, "body", off=(0, 0, 0), rot=(0, 0, 0)),
                key(0.0, "leg_fl", rot=(0, 0, 0)),
                key(0.22, "leg_fl", rot=(35, 0, 0)),
                key(0.45, "leg_fl", rot=(0, 0, 0)),
                key(0.0, "leg_fr", rot=(0, 0, 0)),
                key(0.22, "leg_fr", rot=(35, 0, 0)),
                key(0.45, "leg_fr", rot=(0, 0, 0)),
                key(0.0, "leg_bl", rot=(0, 0, 0)),
                key(0.22, "leg_bl", rot=(-40, 0, 0)),
                key(0.45, "leg_bl", rot=(0, 0, 0)),
                key(0.0, "leg_br", rot=(0, 0, 0)),
                key(0.22, "leg_br", rot=(-40, 0, 0)),
                key(0.45, "leg_br", rot=(0, 0, 0)),
            ],
        ),
    }
    write_rig(out_dir("bunny"), parts, anims)


def build_bird():
    blue = hexc("#3b7dd8")
    blue_hi = hexc("#5ca0f2")
    blue_sh = hexc("#255399")
    cream = hexc("#f4e8d1")
    beak = hexc("#f2992e")
    foot = hexc("#d97724")
    eye = hexc("#14141c")

    # Body: 8x9x11
    body = Model(8, 9, 11)
    # Torso
    body.ellipsoid(3.5, 3.8, 4.5, 3.2, 2.8, 4.0, blue)
    # Head
    body.ellipsoid(3.5, 6.2, 7.2, 2.5, 2.3, 2.5, blue_hi)
    # Cream belly
    for (x, y, z) in list(body.v):
        if y <= 3 and z >= 3 and 2 <= x <= 5:
            body.v[(x, y, z)] = cream
        elif y >= 7:
            body.v[(x, y, z)] = blue_hi
    # Beak (+z)
    body.box(3, 5, 9, 4, 6, 10, beak)
    # Eyes
    body.set(1, 7, 8, eye)
    body.set(6, 7, 8, eye)
    # Tail feathers (-z)
    body.box(2, 3, 0, 5, 4, 2, blue_sh)
    # Orange feet
    body.box(2, 0, 4, 2, 1, 5, foot)
    body.box(5, 0, 4, 5, 1, 5, foot)

    # Wings: 6x2x6
    wing_l = Model(6, 2, 6)
    wing_l.box(0, 0, 1, 5, 1, 5, blue)
    wing_l.box(0, 0, 0, 4, 0, 1, blue_sh)
    wing_l.box(0, 1, 2, 3, 1, 5, blue_hi)
    wing_r = mirror_x(wing_l)

    save_parts("bird", {"body": body, "wing_l": wing_l, "wing_r": wing_r})

    parts = [
        ("body", (4.0, 4.5, 5.5), (0.0, 0.0, 0.0), None),
        ("wing_l", (5.5, 1.0, 3.0), (-3.2, 0.5, -0.5), "body"),
        ("wing_r", (0.5, 1.0, 3.0), (3.2, 0.5, -0.5), "body"),
    ]
    anims = {
        "idle": (
            2.0,
            True,
            [
                key(0.0, "wing_l", rot=(0, 0, 85)),
                key(0.0, "wing_r", rot=(0, 0, -85)),
                key(0.0, "body", off=(0, 0, 0)),
                key(1.0, "body", off=(0, 0.3, 0)),
                key(2.0, "body", off=(0, 0, 0)),
            ],
        ),
        "fly": (
            0.36,
            True,
            [
                key(0.0, "wing_l", rot=(0, 0, -10)),
                key(0.18, "wing_l", rot=(0, 0, 50)),
                key(0.36, "wing_l", rot=(0, 0, -10)),
                key(0.0, "wing_r", rot=(0, 0, 10)),
                key(0.18, "wing_r", rot=(0, 0, -50)),
                key(0.36, "wing_r", rot=(0, 0, 10)),
                key(0.0, "body", off=(0, 0, 0)),
                key(0.18, "body", off=(0, 1.0, 0)),
                key(0.36, "body", off=(0, 0, 0)),
            ],
        ),
    }
    write_rig(out_dir("bird"), parts, anims)


def build_squirrel():
    fur = hexc("#b5622e")
    fur_hi = hexc("#d47e45")
    fur_sh = hexc("#82411a")
    cream = hexc("#f0dec4")
    nose = hexc("#3a2218")
    eye = hexc("#161218")

    # Body: 8x9x11
    body = Model(8, 9, 11)
    body.ellipsoid(3.5, 3.2, 4.5, 3.2, 2.6, 3.8, fur)
    # Head
    body.ellipsoid(3.5, 6.2, 7.5, 2.8, 2.4, 2.6, fur_hi)
    # Cream belly
    for (x, y, z) in list(body.v):
        if y <= 2 and z >= 3 and 2 <= x <= 5:
            body.v[(x, y, z)] = cream
    # Tufted ears
    body.box(1, 8, 6, 2, 8, 7, fur_sh)
    body.box(5, 8, 6, 6, 8, 7, fur_sh)
    # Eyes & nose
    body.set(1, 6, 9, eye)
    body.set(6, 6, 9, eye)
    body.set(3, 5, 10, nose)
    body.set(4, 5, 10, nose)
    # 4 tiny paws
    body.box(1, 0, 6, 2, 1, 7, fur_sh)
    body.box(5, 0, 6, 6, 1, 7, fur_sh)
    body.box(1, 0, 2, 2, 1, 3, fur_sh)
    body.box(5, 0, 2, 6, 1, 3, fur_sh)

    # Bushy curved tail: 4x10x7
    tail = Model(4, 10, 7)
    tail.box(1, 0, 4, 2, 3, 6, fur_sh)
    tail.box(0, 2, 2, 3, 6, 5, fur)
    tail.box(0, 6, 0, 3, 9, 4, fur_hi)

    save_parts("squirrel", {"body": body, "tail": tail})

    parts = [
        ("body", (4.0, 4.5, 5.5), (0.0, 0.0, 0.0), None),
        ("tail", (2.0, 1.0, 5.5), (0.0, 0.5, -4.0), "body"),
    ]
    anims = {
        "idle": (
            1.6,
            True,
            [
                key(0.0, "tail", rot=(0, 0, 0)),
                key(0.8, "tail", rot=(-10, 0, 0)),
                key(1.6, "tail", rot=(0, 0, 0)),
            ],
        ),
        "walk": (
            0.4,
            True,
            [
                key(0.0, "body", off=(0, 0, 0), rot=(0, 0, 0)),
                key(0.2, "body", off=(0, 1.0, 0), rot=(-6, 0, 0)),
                key(0.4, "body", off=(0, 0, 0), rot=(0, 0, 0)),
                key(0.0, "tail", rot=(-5, -6, 0)),
                key(0.2, "tail", rot=(10, 6, 0)),
                key(0.4, "tail", rot=(-5, -6, 0)),
            ],
        ),
    }
    write_rig(out_dir("squirrel"), parts, anims)


def build_cave_critter():
    shell = hexc("#3a4b6e")
    shell_hi = hexc("#546b99")
    shell_sh = hexc("#25304a")
    glow = hexc("#2ef2ff")
    glow_dim = hexc("#189eb8")
    leg_col = hexc("#2a3652")

    # Body: 10x6x14
    body = Model(10, 6, 14)
    body.ellipsoid(4.5, 2.8, 7.0, 4.2, 2.3, 6.2, shell)
    for (x, y, z) in list(body.v):
        if y >= 4:
            body.v[(x, y, z)] = shell_hi
        elif y <= 1:
            body.v[(x, y, z)] = shell_sh
    # Bioluminescent cyan abdomen tip (-z)
    body.ellipsoid(4.5, 2.8, 2.0, 3.0, 1.8, 2.0, glow_dim)
    body.ellipsoid(4.5, 2.8, 1.5, 2.0, 1.2, 1.2, glow)
    # Glowing cyan eyes (+z)
    body.set(2, 3, 12, glow)
    body.set(7, 3, 12, glow)
    # Antennae
    body.set(3, 4, 13, shell_hi)
    body.set(6, 4, 13, shell_hi)

    # 3 jointed legs on left side: 4x4x10 (connected by inner mounting bar)
    legs_l = Model(4, 4, 10)
    legs_l.box(3, 3, 1, 3, 3, 8, leg_col)
    for lz in (1, 4, 8):
        legs_l.box(1, 2, lz, 3, 3, lz, leg_col)
        legs_l.box(0, 0, lz, 1, 2, lz, shell_sh)
    legs_r = mirror_x(legs_l)

    save_parts("cave_critter", {"body": body, "legs_l": legs_l, "legs_r": legs_r})

    parts = [
        ("body", (5.0, 3.0, 7.0), (0.0, 0.0, 0.0), None),
        ("legs_l", (3.5, 3.0, 5.0), (-4.5, -1.5, 0.0), "body"),
        ("legs_r", (0.5, 3.0, 5.0), (4.5, -1.5, 0.0), "body"),
    ]
    anims = {
        "idle": (
            2.0,
            True,
            [
                key(0.0, "body", off=(0, 0, 0)),
                key(1.0, "body", off=(0, 0.3, 0)),
                key(2.0, "body", off=(0, 0, 0)),
            ],
        ),
        "walk": (
            0.32,
            True,
            [
                key(0.0, "legs_l", rot=(12, 0, 0), off=(0, 0.4, 0)),
                key(0.16, "legs_l", rot=(-12, 0, 0), off=(0, 0, 0)),
                key(0.32, "legs_l", rot=(12, 0, 0), off=(0, 0.4, 0)),
                key(0.0, "legs_r", rot=(-12, 0, 0), off=(0, 0, 0)),
                key(0.16, "legs_r", rot=(12, 0, 0), off=(0, 0.4, 0)),
                key(0.32, "legs_r", rot=(-12, 0, 0), off=(0, 0, 0)),
            ],
        ),
    }
    write_rig(out_dir("cave_critter"), parts, anims)


def main():
    build_bunny()
    build_bird()
    build_squirrel()
    build_cave_critter()


if __name__ == "__main__":
    main()
