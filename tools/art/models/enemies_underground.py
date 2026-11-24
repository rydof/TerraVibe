"""Underground, dungeon, and underworld enemy 16-VPU rigs (14 rigs -> enemies family)."""
import os
import sys

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _HERE)
sys.path.insert(0, os.path.dirname(_HERE))

from palette import ASSETS, hexc
from vox import Model
from _rigtools import (
    humanoid_anims,
    jaw_chomp_anims,
    key,
    mirror_x,
    save_parts,
    slime_anims,
    wing_flap_anims,
    write_rig,
)


def _out(kind):
    return os.path.join(ASSETS, "models", kind)


def _humanoid_parts():
    return [
        ("torso", (6, 0, 3), (0, 8, 0), None),
        ("head", (6, 0, 6), (0, 12, 0), "torso"),
        ("arm_l", (2, 12, 2), (8, 12, 0), "torso"),
        ("arm_r", (2, 12, 2), (-8, 12, 0), "torso"),
        ("leg_l", (2.5, 12, 2.5), (2.5, 12, 0), None),
        ("leg_r", (2.5, 12, 2.5), (-2.5, 12, 0), None),
    ]


# ---- 1. skeleton -------------------------------------------------------------
def skeleton():
    BONE = hexc("#e5dec9")
    BONE_D = hexc("#b5ab92")
    BONE_L = hexc("#f5f0e1")
    SOCK = hexc("#1b1715")

    head = Model(12, 12, 12)
    head.box(2, 3, 2, 9, 10, 9, BONE)
    head.box(3, 11, 3, 8, 11, 8, BONE_L)
    head.box(3, 4, 10, 8, 9, 10, BONE)
    head.box(3, 1, 5, 8, 3, 10, BONE_D)  # jaw
    head.box(4, 1, 10, 7, 2, 10, BONE_L)  # teeth
    # Deep eye sockets & nasal cavity
    head.box(3, 6, 10, 4, 8, 10, SOCK)
    head.box(7, 6, 10, 8, 8, 10, SOCK)
    head.box(5, 4, 10, 6, 5, 10, SOCK)

    torso = Model(12, 12, 6)
    torso.box(5, 0, 2, 6, 11, 3, BONE_D)  # spine
    torso.box(3, 0, 1, 8, 2, 4, BONE)    # pelvis
    for ry in (4, 7, 10):                # 3-rib ribcage
        torso.box(2, ry, 1, 9, ry + 1, 4, BONE)
        torso.box(3, ry, 4, 4, ry + 1, 4, BONE_L)
        torso.box(7, ry, 4, 8, ry + 1, 4, BONE_L)

    arm_l = Model(4, 12, 4)
    arm_l.box(1, 1, 1, 2, 11, 2, BONE)
    arm_l.box(1, 5, 1, 2, 6, 2, BONE_D)
    arm_l.box(0, 0, 1, 3, 1, 2, BONE_L)
    arm_r = mirror_x(arm_l)

    leg_l = Model(5, 12, 5)
    leg_l.box(1, 2, 2, 3, 11, 3, BONE)
    leg_l.box(1, 5, 2, 3, 6, 3, BONE_D)
    leg_l.box(1, 0, 1, 3, 1, 4, BONE_L)
    leg_r = mirror_x(leg_l)

    save_parts("skeleton", {"head": head, "torso": torso, "arm_l": arm_l, "arm_r": arm_r, "leg_l": leg_l, "leg_r": leg_r})
    write_rig(_out("skeleton"), _humanoid_parts(), humanoid_anims(arm_base=15.0, arm_swing=35.0, leg_swing=38.0))


# ---- 2. angry_bones ----------------------------------------------------------
def angry_bones():
    BONE = hexc("#d4c39e")
    BONE_D = hexc("#9c8a68")
    IRON = hexc("#636b75")
    IRON_D = hexc("#3f454d")
    RUST = hexc("#8a4b2a")
    SOCK = hexc("#1c1412")
    EYE = hexc("#ff5533")

    head = Model(12, 14, 12)
    head.box(2, 3, 2, 9, 9, 9, BONE)
    head.box(3, 1, 5, 8, 3, 10, BONE_D)
    head.box(3, 5, 10, 4, 7, 10, SOCK)
    head.box(7, 5, 10, 8, 7, 10, SOCK)
    head.set(3, 6, 10, EYE)
    head.set(8, 6, 10, EYE)
    # Dented iron dungeon helm
    head.box(1, 8, 1, 10, 12, 10, IRON)
    head.box(2, 12, 2, 9, 13, 9, IRON_D)
    head.box(5, 6, 10, 6, 11, 11, IRON)
    head.set(3, 11, 10, RUST)
    head.set(8, 10, 10, RUST)

    torso = Model(12, 12, 6)
    torso.box(5, 0, 2, 6, 11, 3, BONE_D)
    torso.box(3, 0, 1, 8, 2, 4, BONE)
    for ry in (4, 7, 10):
        torso.box(2, ry, 1, 9, ry + 1, 4, BONE)
    # Pauldrons
    torso.box(0, 9, 0, 2, 11, 5, IRON)
    torso.box(9, 9, 0, 11, 11, 5, IRON)
    torso.set(1, 11, 4, RUST)

    arm_l = Model(6, 12, 6)
    arm_l.box(2, 1, 2, 3, 11, 3, BONE)
    # Rusty buckler on arm_l
    arm_l.box(4, 1, 1, 5, 7, 5, IRON_D)
    arm_l.box(5, 2, 2, 5, 6, 4, RUST)

    arm_r = Model(4, 12, 4)
    arm_r.box(1, 1, 1, 2, 11, 2, BONE)

    leg_l = Model(5, 12, 5)
    leg_l.box(1, 2, 2, 3, 11, 3, BONE)
    leg_l.box(1, 0, 1, 3, 1, 4, IRON_D)
    leg_r = mirror_x(leg_l)

    parts = [
        ("torso", (6, 0, 3), (0, 8, 0), None),
        ("head", (6, 0, 6), (0, 12, 0), "torso"),
        ("arm_l", (3, 12, 3), (8, 12, 0), "torso"),
        ("arm_r", (2, 12, 2), (-8, 12, 0), "torso"),
        ("leg_l", (2.5, 12, 2.5), (2.5, 12, 0), None),
        ("leg_r", (2.5, 12, 2.5), (-2.5, 12, 0), None),
    ]
    save_parts("angry_bones", {"head": head, "torso": torso, "arm_l": arm_l, "arm_r": arm_r, "leg_l": leg_l, "leg_r": leg_r})
    write_rig(_out("angry_bones"), parts, humanoid_anims(arm_base=20.0, arm_swing=40.0, leg_swing=40.0))


# ---- 3. dark_caster ----------------------------------------------------------
def dark_caster():
    ROBE = hexc("#2a1e5c")
    ROBE_D = hexc("#191138")
    GOLD = hexc("#e0b23a")
    BONE = hexc("#dcd4be")
    SHADOW = hexc("#0d081a")
    VIOLET = hexc("#c855ff")
    STAFF = hexc("#4a3222")

    head = Model(12, 13, 12)
    head.box(1, 1, 1, 10, 11, 10, ROBE)
    head.box(2, 11, 2, 9, 12, 9, ROBE_D)
    head.box(2, 2, 9, 9, 9, 10, SHADOW)
    head.box(3, 3, 9, 8, 8, 9, BONE)
    head.box(3, 5, 10, 4, 6, 10, VIOLET)
    head.box(7, 5, 10, 8, 6, 10, VIOLET)
    head.box(2, 1, 11, 9, 1, 11, GOLD)

    torso = Model(12, 12, 6)
    torso.box(1, 0, 1, 10, 11, 4, ROBE)
    torso.box(5, 0, 4, 6, 11, 4, GOLD)
    torso.box(1, 0, 1, 10, 1, 4, ROBE_D)
    torso.set(5, 6, 5, GOLD)
    torso.set(6, 10, 5, GOLD)

    arm_l = Model(4, 12, 4)
    arm_l.box(0, 2, 0, 3, 11, 3, ROBE)
    arm_l.box(1, 0, 1, 2, 2, 2, BONE)

    arm_r = Model(6, 16, 6)
    arm_r.box(1, 4, 1, 4, 13, 4, ROBE)
    arm_r.box(2, 2, 2, 3, 4, 3, BONE)
    # Glowing arcane staff
    arm_r.box(2, 0, 4, 3, 13, 5, STAFF)
    arm_r.box(1, 13, 3, 4, 15, 5, VIOLET)

    leg_l = Model(5, 12, 5)
    leg_l.box(1, 2, 1, 3, 11, 3, ROBE_D)
    leg_l.box(1, 0, 1, 3, 1, 3, BONE)
    leg_r = mirror_x(leg_l)

    parts = [
        ("torso", (6, 0, 3), (0, 8, 0), None),
        ("head", (6, 0, 6), (0, 12, 0), "torso"),
        ("arm_l", (2, 12, 2), (8, 12, 0), "torso"),
        ("arm_r", (3, 13, 3), (-8, 12, 0), "torso"),
        ("leg_l", (2.5, 12, 2.5), (2.5, 12, 0), None),
        ("leg_r", (2.5, 12, 2.5), (-2.5, 12, 0), None),
    ]
    save_parts("dark_caster", {"head": head, "torso": torso, "arm_l": arm_l, "arm_r": arm_r, "leg_l": leg_l, "leg_r": leg_r})
    write_rig(_out("dark_caster"), parts, humanoid_anims(arm_base=30.0, arm_swing=20.0, leg_swing=25.0))


# ---- 4. cursed_skull ---------------------------------------------------------
def cursed_skull():
    BONE = hexc("#c7bfa6")
    BONE_D = hexc("#8a816b")
    CYAN = hexc("#33f0ff")
    CYAN_L = hexc("#b5ffff")
    SOCK = hexc("#101a24")

    head = Model(14, 16, 14)
    head.box(2, 2, 2, 11, 10, 11, BONE)
    head.box(3, 10, 3, 10, 11, 10, BONE_D)
    head.box(3, 3, 12, 10, 9, 12, BONE)
    # Glowing cyan eye sockets
    head.box(3, 5, 12, 5, 7, 12, SOCK)
    head.box(8, 5, 12, 10, 7, 12, SOCK)
    head.set(4, 6, 12, CYAN)
    head.set(9, 6, 12, CYAN)
    head.box(6, 3, 12, 7, 4, 12, SOCK)
    # Spectral flames rising from crown
    head.box(4, 11, 4, 9, 13, 9, CYAN)
    head.box(5, 13, 5, 8, 15, 8, CYAN_L)

    jaw = Model(10, 4, 8)
    jaw.box(1, 0, 1, 8, 2, 7, BONE_D)
    jaw.box(2, 2, 6, 7, 3, 7, BONE)

    parts = [
        ("head", (7, 8, 7), (0, 8, 0), None),
        ("jaw", (5, 3, 2), (0, 2, 2), "head"),
    ]
    save_parts("cursed_skull", {"head": head, "jaw": jaw})
    write_rig(_out("cursed_skull"), parts, jaw_chomp_anims(jaw_part="jaw", period=0.5, drop=-2.0, rot_x=22.0, bob_part="head"))


# ---- 5. cave_bat -------------------------------------------------------------
def cave_bat():
    FUR = hexc("#5a4459")
    FUR_D = hexc("#3b2b3a")
    WING = hexc("#4a3649")
    WING_D = hexc("#2b1e2a")
    EYE = hexc("#ffbb22")
    FANG = hexc("#f2eee4")

    body = Model(10, 10, 10)
    body.box(2, 2, 2, 7, 7, 7, FUR)
    body.box(3, 1, 3, 6, 2, 6, FUR_D)
    # Pointed ears
    body.box(2, 8, 4, 3, 10, 5, FUR_D)
    body.box(6, 8, 4, 7, 10, 5, FUR_D)
    # Amber eyes & fangs
    body.set(3, 6, 8, EYE)
    body.set(6, 6, 8, EYE)
    body.set(3, 3, 8, FANG)
    body.set(6, 3, 8, FANG)

    wing_l = Model(10, 8, 2)
    wing_l.box(0, 2, 0, 8, 6, 1, WING)
    wing_l.box(0, 6, 0, 9, 7, 1, WING_D)
    wing_l.box(2, 1, 0, 4, 2, 1, WING)
    wing_l.box(6, 1, 0, 8, 2, 1, WING)
    wing_r = mirror_x(wing_l)

    parts = [
        ("body", (5, 5, 5), (0, 6, 0), None),
        ("wing_l", (0, 5, 1), (4, 6, 0), "body"),
        ("wing_r", (10, 5, 1), (-4, 6, 0), "body"),
    ]
    save_parts("cave_bat", {"body": body, "wing_l": wing_l, "wing_r": wing_r})
    write_rig(_out("cave_bat"), parts, wing_flap_anims(body_part="body", period=0.35, angle=40.0, bob=1.2))


# ---- 6. hellbat --------------------------------------------------------------
def hellbat():
    FUR = hexc("#5c1818")
    FUR_D = hexc("#2e0a0a")
    HORN = hexc("#1f1b1b")
    EMBER = hexc("#ff6a1a")
    EYE = hexc("#ffee33")

    body = Model(10, 11, 10)
    body.box(2, 2, 2, 7, 7, 7, FUR)
    body.box(3, 1, 3, 6, 2, 6, FUR_D)
    # Curved horns
    body.box(1, 8, 4, 2, 10, 5, HORN)
    body.box(7, 8, 4, 8, 10, 5, HORN)
    body.set(3, 6, 8, EYE)
    body.set(6, 6, 8, EYE)

    wing_l = Model(11, 8, 2)
    wing_l.box(0, 2, 0, 9, 6, 1, FUR_D)
    wing_l.box(0, 6, 0, 10, 7, 1, FUR)
    wing_l.box(1, 1, 0, 9, 1, 1, EMBER)  # fiery orange edge
    wing_r = mirror_x(wing_l)

    parts = [
        ("body", (5, 5, 5), (0, 6, 0), None),
        ("wing_l", (0, 5, 1), (4, 6, 0), "body"),
        ("wing_r", (11, 5, 1), (-4, 6, 0), "body"),
    ]
    save_parts("hellbat", {"body": body, "wing_l": wing_l, "wing_r": wing_r})
    write_rig(_out("hellbat"), parts, wing_flap_anims(body_part="body", period=0.32, angle=42.0, bob=1.4))


# ---- 7. giant_worm -----------------------------------------------------------
def giant_worm():
    DIRT = hexc("#7a5438")
    DIRT_D = hexc("#4f3421")
    RING = hexc("#9c6d49")
    FANG = hexc("#f0ead8")

    head = Model(12, 12, 14)
    head.box(2, 2, 2, 9, 9, 11, DIRT)
    head.box(3, 3, 0, 8, 8, 2, DIRT_D)
    # Snapping ivory mandibles
    head.box(2, 3, 11, 3, 5, 13, FANG)
    head.box(8, 3, 11, 9, 5, 13, FANG)
    head.set(3, 7, 11, DIRT_D)
    head.set(8, 7, 11, DIRT_D)

    body = Model(12, 12, 12)
    body.box(2, 2, 1, 9, 9, 10, DIRT)
    body.box(1, 3, 4, 10, 8, 7, RING)

    tail = Model(10, 10, 12)
    tail.box(2, 2, 4, 7, 7, 11, DIRT)
    tail.box(3, 3, 0, 6, 6, 4, DIRT_D)

    parts = [
        ("head", (6, 6, 7), (0, 6, 8), None),
        ("body", (6, 6, 6), (0, 6, -2), "head"),
        ("tail", (5, 5, 6), (0, 6, -12), "body"),
    ]
    anims = {
        "idle": (
            0.8,
            True,
            [
                key(0.0, "body", rot=(0, 15, 0)),
                key(0.4, "body", rot=(0, -15, 0)),
                key(0.8, "body", rot=(0, 15, 0)),
                key(0.0, "tail", rot=(0, -22, 0)),
                key(0.4, "tail", rot=(0, 22, 0)),
                key(0.8, "tail", rot=(0, -22, 0)),
            ],
        )
    }
    save_parts("giant_worm", {"head": head, "body": body, "tail": tail})
    write_rig(_out("giant_worm"), parts, anims)


# ---- 8. bone_serpent_head ----------------------------------------------------
def bone_serpent_head():
    BONE = hexc("#dfd6be")
    BONE_D = hexc("#9e9379")
    FIRE = hexc("#ff6a1a")
    SOCK = hexc("#1c110d")

    skull = Model(14, 14, 16)
    skull.box(2, 3, 2, 11, 10, 13, BONE)
    skull.box(3, 4, 13, 10, 8, 15, BONE_D)
    # Dorsal bone horns
    skull.box(2, 10, 2, 4, 13, 5, BONE_D)
    skull.box(9, 10, 2, 11, 13, 5, BONE_D)
    # Fiery orange eye sockets
    skull.box(3, 6, 12, 4, 8, 13, SOCK)
    skull.box(9, 6, 12, 10, 8, 13, SOCK)
    skull.set(3, 7, 13, FIRE)
    skull.set(10, 7, 13, FIRE)

    jaw = Model(10, 4, 12)
    jaw.box(1, 0, 1, 8, 2, 11, BONE_D)
    jaw.box(2, 2, 9, 7, 3, 11, BONE)

    parts = [
        ("skull", (7, 7, 8), (0, 7, 0), None),
        ("jaw", (5, 3, 2), (0, 2, 3), "skull"),
    ]
    save_parts("bone_serpent_head", {"skull": skull, "jaw": jaw})
    write_rig(_out("bone_serpent_head"), parts, jaw_chomp_anims(jaw_part="jaw", period=0.6, drop=-2.0, rot_x=20.0, bob_part="skull"))


# ---- 9. bone_serpent_body ----------------------------------------------------
def bone_serpent_body():
    BONE = hexc("#dfd6be")
    BONE_D = hexc("#9e9379")
    MAGMA = hexc("#ff5511")

    segment = Model(14, 14, 12)
    segment.box(3, 3, 1, 10, 10, 10, BONE)
    segment.box(5, 5, 0, 8, 8, 11, MAGMA)  # glowing magma-core spine
    # Curved ribcage spikes
    segment.box(1, 4, 4, 2, 9, 7, BONE_D)
    segment.box(11, 4, 4, 12, 9, 7, BONE_D)
    segment.box(6, 10, 4, 7, 13, 7, BONE_D)

    parts = [("segment", (7, 7, 6), (0, 7, 0), None)]
    anims = {
        "idle": (
            0.8,
            True,
            [
                key(0.0, "segment", rot=(0, -8, 0)),
                key(0.4, "segment", rot=(0, 8, 0)),
                key(0.8, "segment", rot=(0, -8, 0)),
            ],
        )
    }
    save_parts("bone_serpent_body", {"segment": segment})
    write_rig(_out("bone_serpent_body"), parts, anims)


# ---- 10. bone_serpent_tail ---------------------------------------------------
def bone_serpent_tail():
    BONE = hexc("#dfd6be")
    BONE_D = hexc("#9e9379")

    tail = Model(10, 10, 14)
    tail.box(2, 2, 8, 7, 7, 13, BONE)
    tail.box(3, 3, 4, 6, 6, 8, BONE_D)
    tail.box(4, 4, 0, 5, 5, 4, BONE)

    parts = [("tail", (5, 5, 7), (0, 5, 0), None)]
    anims = {
        "idle": (
            0.8,
            True,
            [
                key(0.0, "tail", rot=(0, 12, 0)),
                key(0.4, "tail", rot=(0, -12, 0)),
                key(0.8, "tail", rot=(0, 12, 0)),
            ],
        )
    }
    save_parts("bone_serpent_tail", {"tail": tail})
    write_rig(_out("bone_serpent_tail"), parts, anims)


# ---- 11. lava_slime ----------------------------------------------------------
def lava_slime():
    GEL_A = 195
    GEL_TOP = hexc("#ff5e1a", GEL_A)
    GEL_BOT = hexc("#b8250a", GEL_A)
    CRUST = hexc("#261c1a")
    CORE = hexc("#fff275")
    CORE_O = hexc("#ffaa1d")
    EYE = hexc("#1f0c08")

    body = Model(24, 18, 20)
    body.ellipsoid(11.5, 0, 9.5, 12, 18, 10, GEL_TOP)
    for (x, y, z) in list(body.v):
        if y < 5:
            body.v[(x, y, z)] = GEL_BOT
    # Charred obsidian crust flecks on base
    for cx, cz in ((4, 6), (8, 3), (15, 4), (19, 8), (6, 14), (17, 13)):
        if (cx, 1, cz) in body.v:
            body.box(cx, 0, cz, min(cx + 1, 23), 2, min(cz + 1, 19), CRUST)

    eyes = Model(18, 3, 1)
    for x0 in (2, 16):
        eyes.box(x0, 0, 0, x0 + 1, 2, 0, EYE)

    core = Model(8, 8, 8)
    core.sphere(3.5, 3.5, 3.5, 3.5, CORE_O)
    core.sphere(3.5, 3.5, 3.5, 2.0, CORE)

    parts = [
        ("body", (12, 0, 10), (0, 0, 0), None),
        ("eyes", (9, 1.5, 0.5), (0, 8, 7), "body"),
        ("core", (4, 4, 4), (0, 7, 0), "body"),
    ]
    save_parts("lava_slime", {"body": body, "eyes": eyes, "core": core})
    write_rig(_out("lava_slime"), parts, slime_anims("body"))


# ---- 12. fire_imp ------------------------------------------------------------
def fire_imp():
    IMP = hexc("#d9381e")
    IMP_D = hexc("#8c1d0f")
    HORN = hexc("#241918")
    EYE = hexc("#ffee33")
    EMBER = hexc("#ff881a")

    body = Model(12, 16, 10)
    body.box(2, 2, 2, 9, 11, 7, IMP)
    body.box(3, 11, 2, 8, 15, 7, IMP)
    body.box(1, 13, 3, 2, 15, 5, HORN)
    body.box(9, 13, 3, 10, 15, 5, HORN)
    body.set(4, 13, 8, EYE)
    body.set(7, 13, 8, EYE)
    body.box(3, 0, 3, 4, 2, 6, IMP_D)
    body.box(7, 0, 3, 8, 2, 6, IMP_D)

    arm_l = Model(4, 10, 4)
    arm_l.box(1, 2, 1, 2, 9, 2, IMP)
    arm_l.box(0, 0, 0, 3, 2, 3, EMBER)
    arm_r = mirror_x(arm_l)

    tail = Model(4, 4, 10)
    tail.box(1, 1, 2, 2, 2, 9, IMP_D)
    tail.box(0, 0, 0, 3, 3, 2, EMBER)

    parts = [
        ("body", (6, 8, 5), (0, 8, 0), None),
        ("arm_l", (2, 9, 2), (7, 11, 0), "body"),
        ("arm_r", (2, 9, 2), (-7, 11, 0), "body"),
        ("tail", (2, 2, 9), (0, 4, -4), "body"),
    ]
    anims = {
        "idle": (
            0.8,
            True,
            [
                key(0.0, "body", off=(0, 0, 0)),
                key(0.4, "body", off=(0, 1.5, 0)),
                key(0.8, "body", off=(0, 0, 0)),
                key(0.0, "arm_l", rot=(20, 0, 0)),
                key(0.4, "arm_l", rot=(-10, 0, 0)),
                key(0.8, "arm_l", rot=(20, 0, 0)),
                key(0.0, "arm_r", rot=(-10, 0, 0)),
                key(0.4, "arm_r", rot=(20, 0, 0)),
                key(0.8, "arm_r", rot=(-10, 0, 0)),
                key(0.0, "tail", rot=(10, 15, 0)),
                key(0.4, "tail", rot=(10, -15, 0)),
                key(0.8, "tail", rot=(10, 15, 0)),
            ],
        )
    }
    save_parts("fire_imp", {"body": body, "arm_l": arm_l, "arm_r": arm_r, "tail": tail})
    write_rig(_out("fire_imp"), parts, anims)


# ---- 13. demon ---------------------------------------------------------------
def demon():
    SKIN = hexc("#a8242c")
    SKIN_D = hexc("#6e141a")
    HORN = hexc("#1c181a")
    EYE = hexc("#ffaa1d")
    WING = hexc("#4a1218")

    body = Model(14, 18, 10)
    body.box(2, 2, 2, 11, 13, 7, SKIN)
    body.box(3, 13, 2, 10, 17, 7, SKIN)
    body.box(1, 15, 3, 2, 17, 5, HORN)
    body.box(11, 15, 3, 12, 17, 5, HORN)
    body.set(4, 15, 8, EYE)
    body.set(9, 15, 8, EYE)
    # Ribbed chest
    for ry in (7, 9, 11):
        body.box(4, ry, 7, 9, ry, 8, SKIN_D)
    # Barbed tail
    body.box(6, 2, 0, 7, 4, 2, SKIN_D)

    wing_l = Model(14, 12, 2)
    wing_l.box(0, 3, 0, 12, 10, 1, WING)
    wing_l.box(0, 10, 0, 13, 11, 1, SKIN_D)
    wing_l.box(3, 1, 0, 5, 3, 1, WING)
    wing_l.box(8, 1, 0, 10, 3, 1, WING)
    wing_r = mirror_x(wing_l)

    parts = [
        ("body", (7, 9, 5), (0, 9, 0), None),
        ("wing_l", (0, 8, 1), (6, 11, -2), "body"),
        ("wing_r", (14, 8, 1), (-6, 11, -2), "body"),
    ]
    save_parts("demon", {"body": body, "wing_l": wing_l, "wing_r": wing_r})
    write_rig(_out("demon"), parts, wing_flap_anims(body_part="body", period=0.45, angle=35.0, bob=1.2))


# ---- 14. voodoo_demon --------------------------------------------------------
def voodoo_demon():
    SKIN = hexc("#8c1b4d")
    SKIN_D = hexc("#540e2d")
    HORN = hexc("#1c181a")
    EYE = hexc("#ffdd33")
    WING = hexc("#3d0c24")
    DOLL_SKIN = hexc("#e8a982")
    DOLL_HAIR = hexc("#6b4423")
    DOLL_SHIRT = hexc("#f2f2f2")

    body = Model(14, 18, 10)
    body.box(2, 2, 2, 11, 13, 7, SKIN)
    body.box(3, 13, 2, 10, 17, 7, SKIN)
    body.box(1, 15, 3, 2, 17, 5, HORN)
    body.box(11, 15, 3, 12, 17, 5, HORN)
    body.set(4, 15, 8, EYE)
    body.set(9, 15, 8, EYE)

    wing_l = Model(14, 12, 2)
    wing_l.box(0, 3, 0, 12, 10, 1, WING)
    wing_l.box(0, 10, 0, 13, 11, 1, SKIN_D)
    wing_r = mirror_x(wing_l)

    voodoo_doll = Model(6, 8, 4)
    voodoo_doll.box(1, 0, 1, 4, 4, 2, DOLL_SHIRT)
    voodoo_doll.box(1, 4, 1, 4, 6, 2, DOLL_SKIN)
    voodoo_doll.box(1, 6, 1, 4, 7, 2, DOLL_HAIR)
    voodoo_doll.set(2, 5, 3, HORN)
    voodoo_doll.set(3, 5, 3, HORN)

    parts = [
        ("body", (7, 9, 5), (0, 12, 0), None),
        ("wing_l", (0, 8, 1), (6, 14, -2), "body"),
        ("wing_r", (14, 8, 1), (-6, 14, -2), "body"),
        ("voodoo_doll", (3, 8, 2), (0, 2, 0), "body"),
    ]
    anims = wing_flap_anims(body_part="body", period=0.45, angle=35.0, bob=1.2)
    anims["idle"][2].extend([
        key(0.0, "voodoo_doll", rot=(0, 0, 12)),
        key(0.225, "voodoo_doll", rot=(0, 0, -12)),
        key(0.45, "voodoo_doll", rot=(0, 0, 12)),
    ])
    save_parts("voodoo_demon", {"body": body, "wing_l": wing_l, "wing_r": wing_r, "voodoo_doll": voodoo_doll})
    write_rig(_out("voodoo_demon"), parts, anims)


def build_all():
    skeleton()
    angry_bones()
    dark_caster()
    cursed_skull()
    cave_bat()
    hellbat()
    giant_worm()
    bone_serpent_head()
    bone_serpent_body()
    bone_serpent_tail()
    lava_slime()
    fire_imp()
    demon()
    voodoo_demon()


if __name__ == "__main__":
    build_all()
