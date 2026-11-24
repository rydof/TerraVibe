"""Event and biome enemy rigs at 16 voxels per block (VPU).

Game axes: x right, y up, z front.
Includes:
- Walkers (idle, walk): blood_zombie, goblin_peon, goblin_thief, goblin_warrior,
  goblin_sorcerer, goblin_archer, snow_flinx, crab
- Hoppers (idle, hop): ice_slime, jungle_slime (translucent gel alpha=190)
- Flyers/Swimmers/Stationary (idle): servant_of_cthulhu, bee, drippler, antlion,
  vulture, hornet, man_eater, harpy, shark, piranha
"""
import math
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from _player_base import ASSETS, Model, hexc
from _rigtools import key, surface, tapered, veins, write_rig


def out(kind):
    return os.path.join(ASSETS, "models", kind)


def save_parts(kind, parts):
    os.makedirs(out(kind), exist_ok=True)
    for name, m in parts.items():
        if len(m.v) > 16:
            m.v.pop((0, 0, 0), None)
        m.save(os.path.join(out(kind), name + ".vox"))


def _walker_anims(arm_l_idle=0.0, arm_r_idle=0.0, stride=0.45):
    return {
        "idle": (1.2, True, [
            key(0.0, "torso", off=(0, 0, 0)),
            key(0.6, "torso", off=(0, -0.4, 0)),
            key(0.0, "arm_l", rot=(arm_l_idle, 0, 0)),
            key(0.0, "arm_r", rot=(arm_r_idle, 0, 0)),
        ]),
        "walk": (0.7, True, [
            key(0.0, "leg_l", rot=(stride, 0, 0)),
            key(0.35, "leg_l", rot=(-stride, 0, 0)),
            key(0.0, "leg_r", rot=(-stride, 0, 0)),
            key(0.35, "leg_r", rot=(stride, 0, 0)),
            key(0.0, "arm_l", rot=(arm_l_idle - stride * 0.6, 0, 0)),
            key(0.35, "arm_l", rot=(arm_l_idle + stride * 0.6, 0, 0)),
            key(0.0, "arm_r", rot=(arm_r_idle + stride * 0.6, 0, 0)),
            key(0.35, "arm_r", rot=(arm_r_idle - stride * 0.6, 0, 0)),
            key(0.0, "torso", off=(0, 0, 0)),
            key(0.175, "torso", off=(0, 0.4, 0)),
            key(0.35, "torso", off=(0, 0, 0)),
            key(0.525, "torso", off=(0, 0.4, 0)),
        ]),
    }


def _write_humanoid(kind, torso, head, arm_l, arm_r, leg_l, leg_r,
                    arm_l_idle=0.0, arm_r_idle=0.0, stride=0.45):
    save_parts(kind, {
        "torso": torso, "head": head,
        "arm_l": arm_l, "arm_r": arm_r,
        "leg_l": leg_l, "leg_r": leg_r,
    })
    write_rig(
        out(kind),
        [
            ("torso", (torso.size[0] // 2, 0, torso.size[2] // 2), (0, 12, 0), None),
            ("head", (head.size[0] // 2, 0, head.size[2] // 2), (0, torso.size[1], 0), "torso"),
            ("arm_l", (arm_l.size[0] - 1, arm_l.size[1] - 2, arm_l.size[2] // 2),
             (-torso.size[0] // 2, torso.size[1] - 2, 0), "torso"),
            ("arm_r", (0, arm_r.size[1] - 2, arm_r.size[2] // 2),
             (torso.size[0] // 2, torso.size[1] - 2, 0), "torso"),
            ("leg_l", (leg_l.size[0] // 2, leg_l.size[1] - 1, leg_l.size[2] // 2),
             (-torso.size[0] // 4, 0, 0), "torso"),
            ("leg_r", (leg_r.size[0] // 2, leg_r.size[1] - 1, leg_r.size[2] // 2),
             (torso.size[0] // 4, 0, 0), "torso"),
        ],
        _walker_anims(arm_l_idle, arm_r_idle, stride),
    )


# ---- 1. blood_zombie --------------------------------------------------------
def blood_zombie():
    skin = hexc("#7a3e42")
    skin_d = hexc("#59292d")
    blood = hexc("#b31b24")
    rib = hexc("#d9cbb8")
    rag = hexc("#3b2c30")
    eye = hexc("#ff3b3b")

    torso = Model(8, 12, 6)
    torso.box(0, 0, 0, 7, 11, 5, rag)
    torso.box(1, 3, 4, 6, 9, 5, skin_d)
    for y in (4, 6, 8):
        torso.box(2, y, 5, 5, y, 5, rib)
    torso.box(1, 1, 5, 3, 2, 5, blood)
    torso.set(0, 11, 5, blood)  # 1-voxel detail

    head = Model(8, 8, 8)
    head.box(0, 0, 0, 7, 7, 7, skin)
    head.box(0, 5, 0, 7, 7, 7, skin_d)
    head.set(2, 4, 7, eye)
    head.set(5, 4, 7, eye)
    head.box(2, 1, 7, 5, 2, 7, blood)

    arm_l = Model(4, 12, 4)
    arm_l.box(0, 0, 0, 3, 11, 3, skin)
    arm_l.box(0, 0, 0, 3, 3, 3, blood)
    arm_l.set(0, 0, 3, skin_d)
    arm_r = Model(4, 12, 4)
    arm_r.box(0, 0, 0, 3, 11, 3, skin)
    arm_r.box(0, 0, 0, 3, 3, 3, blood)
    arm_r.set(3, 0, 3, skin_d)

    leg_l = Model(4, 12, 4)
    leg_l.box(0, 0, 0, 3, 11, 3, rag)
    leg_l.box(0, 0, 0, 3, 2, 3, skin_d)
    leg_l.set(1, 1, 3, blood)
    leg_r = Model(4, 12, 4)
    leg_r.box(0, 0, 0, 3, 11, 3, rag)
    leg_r.box(0, 0, 0, 3, 2, 3, skin_d)
    leg_r.set(2, 1, 3, blood)

    _write_humanoid("blood_zombie", torso, head, arm_l, arm_r, leg_l, leg_r,
                    arm_l_idle=1.2, arm_r_idle=1.1, stride=0.45)


# ---- 2-6. goblins -----------------------------------------------------------
GOB_SKIN = hexc("#5a8f49")
GOB_SKIN_D = hexc("#3e6b32")
GOB_EYE = hexc("#f2c94c")
GOB_LEATHER = hexc("#5c3a21")
GOB_METAL = hexc("#8a949e")
GOB_METAL_D = hexc("#5a626b")


def _goblin_base(cloth_c, hat_fn=None):
    torso = Model(8, 10, 6)
    torso.box(0, 0, 0, 7, 9, 5, cloth_c)
    torso.box(0, 2, 0, 7, 3, 5, GOB_LEATHER)
    torso.set(3, 2, 5, GOB_METAL)  # belt buckle

    head = Model(8, 8, 8)
    head.box(0, 0, 0, 7, 7, 7, GOB_SKIN)
    head.box(0, 0, 5, 7, 2, 7, GOB_SKIN_D)
    head.set(2, 4, 7, GOB_EYE)
    head.set(5, 4, 7, GOB_EYE)
    # Pointy ears
    head.box(0, 3, 3, 0, 5, 4, GOB_SKIN_D)
    head.box(7, 3, 3, 7, 5, 4, GOB_SKIN_D)
    if hat_fn:
        head = hat_fn(head)

    arm_l = Model(4, 10, 4)
    arm_l.box(0, 0, 0, 3, 9, 3, GOB_SKIN)
    arm_l.set(0, 0, 3, GOB_SKIN_D)
    arm_r = Model(4, 10, 4)
    arm_r.box(0, 0, 0, 3, 9, 3, GOB_SKIN)
    arm_r.set(3, 0, 3, GOB_SKIN_D)

    leg_l = Model(4, 10, 4)
    leg_l.box(0, 0, 0, 3, 9, 3, GOB_LEATHER)
    leg_l.set(1, 0, 3, GOB_SKIN_D)
    leg_r = Model(4, 10, 4)
    leg_r.box(0, 0, 0, 3, 9, 3, GOB_LEATHER)
    leg_r.set(2, 0, 3, GOB_SKIN_D)
    return torso, head, arm_l, arm_r, leg_l, leg_r


def goblin_peon():
    torso, head, arm_l, arm_r, leg_l, leg_r = _goblin_base(hexc("#6e4b2e"))
    # Spiked wooden club in right hand
    club_arm = Model(6, 12, 8)
    club_arm.box(1, 2, 0, 4, 11, 3, GOB_SKIN)
    club_arm.box(2, 0, 2, 3, 3, 7, hexc("#54361d"))
    club_arm.box(1, 0, 5, 4, 2, 7, GOB_METAL_D)
    _write_humanoid("goblin_peon", torso, head, arm_l, club_arm, leg_l, leg_r,
                    arm_r_idle=0.4, stride=0.5)


def goblin_thief():
    hood = hexc("#2a2630")
    def _hood(h):
        h.box(0, 5, 0, 7, 7, 7, hood)
        h.box(0, 0, 0, 7, 2, 7, hood)
        return h
    torso, head, arm_l, arm_r, leg_l, leg_r = _goblin_base(hood, _hood)
    # Dual daggers
    for a in (arm_l, arm_r):
        a.box(1, 0, 3, 2, 1, 3, GOB_LEATHER)
    dag_l = Model(4, 10, 7)
    dag_l.box(0, 0, 0, 3, 9, 3, GOB_SKIN)
    dag_l.box(1, 1, 4, 2, 2, 6, GOB_METAL)
    dag_r = Model(4, 10, 7)
    dag_r.box(0, 0, 0, 3, 9, 3, GOB_SKIN)
    dag_r.box(1, 1, 4, 2, 2, 6, GOB_METAL)
    _write_humanoid("goblin_thief", torso, head, dag_l, dag_r, leg_l, leg_r,
                    arm_l_idle=0.5, arm_r_idle=0.5, stride=0.6)


def goblin_warrior():
    horn = hexc("#e0d8c8")
    def _helm(h):
        h.box(0, 5, 0, 7, 7, 7, GOB_METAL)
        h.box(0, 6, 3, 0, 8, 4, horn)
        h.box(7, 6, 3, 7, 8, 4, horn)
        return h
    torso, head, arm_l, arm_r, leg_l, leg_r = _goblin_base(GOB_METAL_D, _helm)
    # Round shield on left arm
    shield_arm = Model(6, 10, 8)
    shield_arm.box(2, 0, 0, 5, 9, 3, GOB_SKIN)
    shield_arm.box(0, 1, 2, 1, 7, 6, GOB_METAL)
    shield_arm.box(0, 3, 4, 0, 5, 4, GOB_LEATHER)
    # Battle axe on right arm
    axe_arm = Model(6, 12, 9)
    axe_arm.box(1, 2, 0, 4, 11, 3, GOB_SKIN)
    axe_arm.box(2, 0, 2, 3, 2, 7, GOB_LEATHER)
    axe_arm.box(1, 0, 6, 4, 4, 8, GOB_METAL)
    _write_humanoid("goblin_warrior", torso, head, shield_arm, axe_arm, leg_l, leg_r,
                    arm_l_idle=0.3, arm_r_idle=0.4, stride=0.4)


def goblin_sorcerer():
    robe = hexc("#5b337a")
    trim = hexc("#a865d9")
    orb = hexc("#e056fd")
    def _wiz_hat(h):
        h.box(0, 6, 0, 7, 7, 7, robe)
        h.box(1, 8, 1, 6, 10, 6, robe)
        h.box(2, 11, 2, 5, 13, 5, trim)
        return h
    torso, head, arm_l, arm_r, leg_l, leg_r = _goblin_base(robe, _wiz_hat)
    torso.box(3, 0, 5, 4, 9, 5, trim)
    staff_arm = Model(4, 14, 7)
    staff_arm.box(0, 4, 0, 3, 13, 3, GOB_SKIN)
    staff_arm.box(1, 0, 4, 2, 12, 5, GOB_LEATHER)
    staff_arm.box(1, 12, 4, 2, 13, 5, orb)
    _write_humanoid("goblin_sorcerer", torso, head, arm_l, staff_arm, leg_l, leg_r,
                    arm_r_idle=0.35, stride=0.4)


def goblin_archer():
    cap = hexc("#2e6b3e")
    feather = hexc("#e84a38")
    def _cap(h):
        h.box(0, 6, 0, 7, 7, 7, cap)
        h.box(6, 8, 2, 7, 10, 3, feather)
        return h
    torso, head, arm_l, arm_r, leg_l, leg_r = _goblin_base(cap, _cap)
    # Quiver on back
    torso.box(2, 2, 0, 5, 9, 0, GOB_LEATHER)
    # Bow in left arm
    bow_arm = Model(4, 12, 8)
    bow_arm.box(0, 1, 0, 3, 10, 3, GOB_SKIN)
    bow_arm.box(1, 0, 4, 2, 11, 5, GOB_LEATHER)
    bow_arm.box(1, 2, 6, 2, 9, 7, GOB_LEATHER)
    _write_humanoid("goblin_archer", torso, head, bow_arm, arm_r, leg_l, leg_r,
                    arm_l_idle=0.5, stride=0.5)


# ---- 7. snow_flinx ----------------------------------------------------------
def snow_flinx():
    fur = hexc("#eef6fc")
    fur_d = hexc("#c4dcf0")
    nose = hexc("#e88ca5")
    eye = hexc("#141a24")

    body = Model(14, 12, 14)
    body.ellipsoid(6.5, 5.5, 6.5, 7, 6, 7, fur)
    for (x, y, z) in list(body.v):
        if y < 3:
            body.v[(x, y, z)] = fur_d
    body.set(4, 6, 13, eye)
    body.set(9, 6, 13, eye)
    body.box(6, 4, 13, 7, 5, 13, nose)

    ear_l = Model(4, 5, 3)
    ear_l.box(0, 0, 0, 3, 4, 2, fur_d)
    ear_r = Model(4, 5, 3)
    ear_r.box(0, 0, 0, 3, 4, 2, fur_d)

    leg_l = Model(4, 4, 5)
    leg_l.box(0, 0, 0, 3, 3, 4, fur_d)
    leg_r = Model(4, 4, 5)
    leg_r.box(0, 0, 0, 3, 3, 4, fur_d)

    save_parts("snow_flinx", {"body": body, "ear_l": ear_l, "ear_r": ear_r,
                              "leg_l": leg_l, "leg_r": leg_r})
    write_rig(
        out("snow_flinx"),
        [
            ("body", (7, 0, 7), (0, 3, 0), None),
            ("ear_l", (3, 4, 1), (-5, 10, 0), "body"),
            ("ear_r", (0, 4, 1), (5, 10, 0), "body"),
            ("leg_l", (2, 3, 2), (-3, 0, 0), "body"),
            ("leg_r", (2, 3, 2), (3, 0, 0), "body"),
        ],
        {
            "idle": (1.0, True, [
                key(0.0, "body", off=(0, 0, 0)),
                key(0.5, "body", off=(0, 0.5, 0)),
            ]),
            "walk": (0.6, True, [
                key(0.0, "leg_l", rot=(0.5, 0, 0)),
                key(0.3, "leg_l", rot=(-0.5, 0, 0)),
                key(0.0, "leg_r", rot=(-0.5, 0, 0)),
                key(0.3, "leg_r", rot=(0.5, 0, 0)),
                key(0.0, "body", off=(0, 0, 0)),
                key(0.15, "body", off=(0, 0.6, 0)),
                key(0.3, "body", off=(0, 0, 0)),
                key(0.45, "body", off=(0, 0.6, 0)),
            ]),
        },
    )


# ---- 8. crab ----------------------------------------------------------------
def crab():
    shell = hexc("#d94826")
    shell_d = hexc("#a12d14")
    belly = hexc("#f0945d")
    eye = hexc("#111111")

    body = Model(16, 8, 12)
    body.ellipsoid(7.5, 3.5, 5.5, 8, 4, 6, shell)
    for (x, y, z) in list(body.v):
        if y < 2:
            body.v[(x, y, z)] = belly
    body.box(4, 6, 8, 5, 7, 9, shell_d)
    body.set(4, 7, 10, eye)
    body.box(10, 6, 8, 11, 7, 9, shell_d)
    body.set(11, 7, 10, eye)

    claw_l = Model(6, 5, 8)
    claw_l.box(0, 1, 0, 4, 3, 5, shell)
    claw_l.box(1, 0, 5, 5, 4, 7, shell_d)
    claw_r = Model(6, 5, 8)
    claw_r.box(1, 1, 0, 5, 3, 5, shell)
    claw_r.box(0, 0, 5, 4, 4, 7, shell_d)

    leg_l = Model(6, 5, 6)
    leg_l.box(0, 0, 0, 5, 2, 1, shell_d)
    leg_l.box(0, 0, 4, 5, 2, 5, shell_d)
    leg_l.box(4, 2, 0, 5, 4, 5, shell_d)
    leg_r = Model(6, 5, 6)
    leg_r.box(0, 0, 0, 5, 2, 1, shell_d)
    leg_r.box(0, 0, 4, 5, 2, 5, shell_d)
    leg_r.box(0, 2, 0, 1, 4, 5, shell_d)

    save_parts("crab", {"body": body, "claw_l": claw_l, "claw_r": claw_r,
                        "leg_l": leg_l, "leg_r": leg_r})
    write_rig(
        out("crab"),
        [
            ("body", (8, 0, 6), (0, 3, 0), None),
            ("claw_l", (5, 2, 0), (-7, 3, 4), "body"),
            ("claw_r", (0, 2, 0), (7, 3, 4), "body"),
            ("leg_l", (5, 4, 3), (-7, 2, 0), "body"),
            ("leg_r", (0, 4, 3), (7, 2, 0), "body"),
        ],
        {
            "idle": (1.0, True, [
                key(0.0, "claw_l", rot=(0, 0, 0)),
                key(0.5, "claw_l", rot=(-0.2, 0, 0)),
                key(0.0, "claw_r", rot=(0, 0, 0)),
                key(0.5, "claw_r", rot=(-0.2, 0, 0)),
            ]),
            "walk": (0.5, True, [
                key(0.0, "leg_l", rot=(0, 0, 0.3)),
                key(0.25, "leg_l", rot=(0, 0, -0.3)),
                key(0.0, "leg_r", rot=(0, 0, -0.3)),
                key(0.25, "leg_r", rot=(0, 0, 0.3)),
            ]),
        },
    )


# ---- 9-10. ice_slime & jungle_slime -----------------------------------------
def _slime_variant(kind, top_c, bot_c, core_c, core_l, core_name):
    gel_a = 190
    g_top = hexc(top_c, gel_a)
    g_bot = hexc(bot_c, gel_a)
    g_hi = hexc("#ffffff")
    eye_c = hexc("#0b1030")

    body = Model(24, 18, 20)
    body.ellipsoid(11.5, 0, 9.5, 12, 18, 10, g_top)
    for (x, y, z) in list(body.v):
        if y < 5:
            body.v[(x, y, z)] = g_bot
    for gx, gy in ((9, 14), (11, 15), (13, 16)):
        zs = [z for (x, y, z) in body.v if x == gx and y == gy]
        if zs:
            body.set(gx, gy, max(zs), g_hi)

    eyes = Model(18, 3, 1)
    for x0 in (2, 16):
        eyes.box(x0, 0, 0, x0 + 1, 2, 0, eye_c)

    core = Model(8, 8, 8)
    core.sphere(3.5, 3.5, 3.5, 3.2, hexc(core_c))
    core.box(1, 3, 3, 6, 4, 4, hexc(core_l))
    core.box(3, 1, 3, 4, 6, 4, hexc(core_l))

    save_parts(kind, {"body": body, "eyes": eyes, core_name: core})
    squash = (1.22, 0.74, 1.22)
    stretch = (0.88, 1.22, 0.88)
    write_rig(
        out(kind),
        [
            ("body", (11, 0, 9), (0, 0, 0), None),
            ("eyes", (9, 0, 0), (0, 6, 7), "body"),
            (core_name, (4, 4, 4), (0, 6, 0), "body"),
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


def ice_slime():
    _slime_variant("ice_slime", "#8ce8ff", "#4ab8e8", "#2b6cb0", "#d4f5ff", "shards")


def jungle_slime():
    _slime_variant("jungle_slime", "#4cd97b", "#22994c", "#1b5e20", "#b9f6ca", "spores")


# ---- 11. servant_of_cthulhu -------------------------------------------------
def servant_of_cthulhu():
    rng = random.Random(19)
    sclera = hexc("#efe6e0")
    vein = hexc("#c4333a")
    iris = hexc("#a5381e")
    pupil = hexc("#0a0a0a")
    nerve = hexc("#b3222a")
    nerve_d = hexc("#80161d")

    ball = Model(16, 16, 16)
    ball.sphere(7.5, 7.5, 7.5, 8, sclera)
    veins(ball, rng, 12, 7, lambda p: p[2] > 12)
    for (x, y, z) in list(ball.v):
        if z >= 13:
            d = math.hypot(x - 7.5, y - 7.5)
            if d < 2.0:
                ball.v[(x, y, z)] = pupil
            elif d < 4.2:
                ball.v[(x, y, z)] = iris

    tail1 = Model(8, 3, 7)
    tail1.box(1, 0, 0, 6, 2, 6, nerve)
    tail2 = Model(6, 2, 5)
    tail2.box(1, 0, 0, 4, 1, 4, nerve_d)
    save_parts("servant_of_cthulhu", {"ball": ball, "tail1": tail1, "tail2": tail2})
    write_rig(
        out("servant_of_cthulhu"),
        [
            ("ball", (8, 8, 8), (0, 8, 0), None),
            ("tail1", (4, 1, 6), (0, 0, -7), "ball"),
            ("tail2", (3, 1, 4), (0, 0, -6), "tail1"),
        ],
        {
            "idle": (0.8, True, [
                key(0.0, "tail1", rot=(0, 0.25, 0)),
                key(0.4, "tail1", rot=(0, -0.25, 0)),
                key(0.0, "tail2", rot=(0, -0.35, 0)),
                key(0.4, "tail2", rot=(0, 0.35, 0)),
            ]),
        },
    )


# ---- 12. bee ----------------------------------------------------------------
def _winged_insect(kind, body_len, body_r, yellow, black, stinger_c):
    body = Model(body_r * 2, body_r * 2, body_len)
    body.ellipsoid(body_r - 0.5, body_r - 0.5, body_len / 2 - 0.5,
                   body_r, body_r, body_len / 2, yellow)
    for (x, y, z) in list(body.v):
        if (z // 2) % 2 == 0:
            body.v[(x, y, z)] = black
    # Stinger at back
    body.box(body_r - 1, body_r - 1, 0, body_r, body_r, 1, stinger_c)

    wing_c = hexc("#dff5ff", 170)
    wing_l = Model(8, 1, 6)
    wing_l.box(0, 0, 1, 7, 0, 4, wing_c)
    wing_r = Model(8, 1, 6)
    wing_r.box(0, 0, 1, 7, 0, 4, wing_c)

    save_parts(kind, {"body": body, "wing_l": wing_l, "wing_r": wing_r})
    write_rig(
        out(kind),
        [
            ("body", (body_r, body_r, body_len // 2), (0, 8, 0), None),
            ("wing_l", (7, 0, 3), (-body_r + 1, body_r, 0), "body"),
            ("wing_r", (0, 0, 3), (body_r - 1, body_r, 0), "body"),
        ],
        {
            "idle": (0.25, True, [
                key(0.0, "wing_l", rot=(0, 0, 0.6)),
                key(0.125, "wing_l", rot=(0, 0, -0.6)),
                key(0.0, "wing_r", rot=(0, 0, -0.6)),
                key(0.125, "wing_r", rot=(0, 0, 0.6)),
            ]),
        },
    )


def bee():
    _winged_insect("bee", 12, 4, hexc("#f2c94c"), hexc("#1c1a17"), hexc("#4a4a4a"))


def hornet():
    _winged_insect("hornet", 18, 5, hexc("#e6b800"), hexc("#1c1a17"), hexc("#222222"))


# ---- 13. drippler -----------------------------------------------------------
def drippler():
    flesh = hexc("#a61c28")
    flesh_d = hexc("#6e0f18")
    eye = hexc("#f5f0e6")
    pupil = hexc("#111111")

    body = Model(14, 14, 14)
    body.sphere(6.5, 6.5, 6.5, 6.5, flesh)
    body.box(3, 7, 11, 5, 9, 13, eye)
    body.set(4, 8, 13, pupil)
    body.box(8, 5, 11, 10, 7, 13, eye)
    body.set(9, 6, 13, pupil)

    tendril = Model(8, 10, 8)
    tendril.box(1, 2, 1, 2, 9, 2, flesh_d)
    tendril.box(5, 0, 4, 6, 9, 5, flesh_d)
    tendril.box(2, 8, 2, 5, 9, 4, flesh)

    save_parts("drippler", {"body": body, "tendril": tendril})
    write_rig(
        out("drippler"),
        [
            ("body", (7, 7, 7), (0, 14, 0), None),
            ("tendril", (4, 9, 4), (0, -6, 0), "body"),
        ],
        {
            "idle": (1.0, True, [
                key(0.0, "body", off=(0, 0, 0)),
                key(0.5, "body", off=(0, 1.0, 0)),
                key(0.0, "tendril", rot=(0.15, 0, 0)),
                key(0.5, "tendril", rot=(-0.15, 0, 0)),
            ]),
        },
    )


# ---- 14. antlion ------------------------------------------------------------
def antlion():
    chitin = hexc("#b58952")
    chitin_d = hexc("#7a5830")
    eye = hexc("#1a1410")

    body = Model(16, 10, 16)
    body.ellipsoid(7.5, 4.5, 7.5, 8, 5, 8, chitin)
    body.box(3, 5, 13, 5, 7, 14, eye)
    body.box(10, 5, 13, 12, 7, 14, eye)

    mandible_l = Model(5, 4, 10)
    mandible_l.box(1, 1, 0, 3, 2, 9, chitin_d)
    mandible_l.box(3, 1, 7, 4, 2, 9, chitin)
    mandible_r = Model(5, 4, 10)
    mandible_r.box(1, 1, 0, 3, 2, 9, chitin_d)
    mandible_r.box(0, 1, 7, 1, 2, 9, chitin)

    save_parts("antlion", {"body": body, "mandible_l": mandible_l, "mandible_r": mandible_r})
    write_rig(
        out("antlion"),
        [
            ("body", (8, 0, 8), (0, 0, 0), None),
            ("mandible_l", (2, 2, 0), (-5, 3, 7), "body"),
            ("mandible_r", (2, 2, 0), (5, 3, 7), "body"),
        ],
        {
            "idle": (0.8, True, [
                key(0.0, "mandible_l", rot=(0, -0.3, 0)),
                key(0.4, "mandible_l", rot=(0, 0.15, 0)),
                key(0.0, "mandible_r", rot=(0, 0.3, 0)),
                key(0.4, "mandible_r", rot=(0, -0.15, 0)),
            ]),
        },
    )


# ---- 15. vulture ------------------------------------------------------------
def vulture():
    feather = hexc("#4a3628")
    feather_d = hexc("#2e2017")
    bald = hexc("#d97c7c")
    beak = hexc("#d9c282")

    body = Model(10, 12, 16)
    body.ellipsoid(4.5, 5.5, 6.5, 5, 6, 7, feather)
    body.box(3, 8, 10, 6, 11, 13, bald)
    body.box(4, 8, 14, 5, 9, 15, beak)

    wing_l = Model(14, 2, 8)
    wing_l.box(0, 0, 1, 13, 1, 6, feather_d)
    wing_l.v.pop((0, 0, 1), None)
    wing_r = Model(14, 2, 8)
    wing_r.box(0, 0, 1, 13, 1, 6, feather_d)
    wing_r.set(13, 0, 0, feather)

    save_parts("vulture", {"body": body, "wing_l": wing_l, "wing_r": wing_r})
    write_rig(
        out("vulture"),
        [
            ("body", (5, 6, 8), (0, 12, 0), None),
            ("wing_l", (13, 1, 4), (-5, 8, 0), "body"),
            ("wing_r", (0, 1, 4), (5, 8, 0), "body"),
        ],
        {
            "idle": (0.7, True, [
                key(0.0, "wing_l", rot=(0, 0, 0.5)),
                key(0.35, "wing_l", rot=(0, 0, -0.4)),
                key(0.0, "wing_r", rot=(0, 0, -0.5)),
                key(0.35, "wing_r", rot=(0, 0, 0.4)),
            ]),
        },
    )


# ---- 17. man_eater ----------------------------------------------------------
def man_eater():
    vine = hexc("#2d7a36")
    bulb = hexc("#9e2434")
    teeth = hexc("#f2eee4")

    stem = Model(6, 16, 6)
    stem.box(1, 0, 1, 4, 15, 4, vine)
    stem.set(0, 5, 2, bulb)
    stem.set(5, 10, 3, bulb)
    head = Model(12, 10, 12)
    head.ellipsoid(5.5, 4.5, 5.5, 6, 5, 6, bulb)
    head.box(2, 0, 9, 3, 1, 11, teeth)
    head.box(8, 0, 9, 9, 1, 11, teeth)

    jaw = Model(10, 4, 10)
    jaw.box(0, 0, 0, 9, 2, 9, bulb)
    jaw.box(2, 3, 7, 3, 3, 8, teeth)
    jaw.box(6, 3, 7, 7, 3, 8, teeth)

    save_parts("man_eater", {"stem": stem, "head": head, "jaw": jaw})
    write_rig(
        out("man_eater"),
        [
            ("stem", (3, 0, 3), (0, 0, 0), None),
            ("head", (6, 0, 6), (0, 15, 0), "stem"),
            ("jaw", (5, 3, 0), (0, 0, 0), "head"),
        ],
        {
            "idle": (0.8, True, [
                key(0.0, "jaw", rot=(0.35, 0, 0)),
                key(0.4, "jaw", rot=(0.0, 0, 0)),
            ]),
        },
    )


# ---- 18. harpy --------------------------------------------------------------
def harpy():
    skin = hexc("#f2c8b6")
    skin_d = hexc("#d49e8a")
    feather_d = hexc("#214d96")
    feather = hexc("#3a7bd5")
    feather_l = hexc("#7eb6ff")
    hair_d = hexc("#163266")
    hair = hexc("#234b8c")
    hair_l = hexc("#4478c4")
    talon = hexc("#e09828")
    talon_l = hexc("#ffd060")

    # Torso: feathered bodice, pale sky-blue belly plume, tail feathers at back
    torso = Model(8, 12, 8)
    torso.box(1, 2, 2, 6, 10, 6, feather)
    torso.box(2, 3, 6, 5, 9, 6, feather_l)  # front chest plume
    torso.box(1, 0, 2, 6, 2, 6, feather_d)  # waist sash
    torso.box(2, 0, 0, 5, 4, 2, feather_d)  # tail feathers
    torso.box(2, 0, 0, 2, 2, 1, feather_l)
    torso.box(5, 0, 0, 5, 2, 1, feather_l)

    # Head: sculpted face, eyes, flowing multi-layer azure hair
    head = Model(8, 9, 9)
    head.box(1, 0, 2, 6, 6, 7, skin)
    head.box(1, 0, 2, 6, 1, 3, skin_d)
    head.box(0, 5, 1, 7, 8, 8, hair)       # hair crown
    head.box(0, 2, 0, 7, 6, 2, hair_d)     # back hair
    head.box(0, 2, 3, 0, 6, 6, hair)       # left side lock
    head.box(7, 2, 3, 7, 6, 6, hair)       # right side lock
    head.box(2, 7, 3, 5, 8, 7, hair_l)     # top highlight
    head.set(2, 3, 7, hexc("#182038"))     # eyes
    head.set(5, 3, 7, hexc("#182038"))
    head.set(2, 4, 7, hexc("#ffffff"))
    head.set(5, 4, 7, hexc("#ffffff"))

    # Tapered scalloped feathered wings
    wing_l = Model(16, 10, 3)
    wing_l.box(10, 6, 0, 15, 9, 2, feather_d)  # shoulder covert
    wing_l.box(4, 4, 0, 14, 7, 2, feather)     # mid wing
    wing_l.box(1, 2, 1, 10, 5, 2, feather_l)   # primary feathers
    for i, x in enumerate((0, 3, 6, 9)):
        wing_l.box(x, 0 + (i % 2), 1, x + 1, 3, 1, feather_l)

    wing_r = Model(16, 10, 3)
    wing_r.box(0, 6, 0, 5, 9, 2, feather_d)
    wing_r.box(1, 4, 0, 11, 7, 2, feather)
    wing_r.box(5, 2, 1, 14, 5, 2, feather_l)
    for i, x in enumerate((14, 11, 8, 5)):
        wing_r.box(x, 0 + (i % 2), 1, x + 1, 3, 1, feather_l)

    # Feathered thighs + golden avian talons
    leg_l = Model(4, 10, 5)
    leg_l.box(0, 4, 1, 3, 9, 4, feather)
    leg_l.box(1, 1, 2, 2, 4, 3, talon)
    leg_l.box(0, 0, 2, 3, 0, 4, talon_l)
    leg_r = Model(4, 10, 5)
    leg_r.box(0, 4, 1, 3, 9, 4, feather)
    leg_r.box(1, 1, 2, 2, 4, 3, talon)
    leg_r.box(0, 0, 2, 3, 0, 4, talon_l)

    save_parts("harpy", {"torso": torso, "head": head,
                         "wing_l": wing_l, "wing_r": wing_r,
                         "leg_l": leg_l, "leg_r": leg_r})
    write_rig(
        out("harpy"),
        [
            ("torso", (4, 0, 4), (0, 10, 0), None),
            ("head", (4, 0, 4), (0, 12, 0), "torso"),
            ("wing_l", (15, 7, 1), (-4, 10, -2), "torso"),
            ("wing_r", (0, 7, 1), (4, 10, -2), "torso"),
            ("leg_l", (2, 9, 2), (-2, 0, 0), "torso"),
            ("leg_r", (2, 9, 2), (2, 0, 0), "torso"),
        ],
        {
            "idle": (0.56, True, [
                key(0.0, "torso", rot=(0.08, 0.0, 0.0)),
                key(0.28, "torso", rot=(-0.06, 0.0, 0.0)),
                key(0.56, "torso", rot=(0.08, 0.0, 0.0)),
                key(0.0, "wing_l", rot=(0.0, 0.0, 0.78)),
                key(0.28, "wing_l", rot=(0.0, 0.0, -0.68)),
                key(0.56, "wing_l", rot=(0.0, 0.0, 0.78)),
                key(0.0, "wing_r", rot=(0.0, 0.0, -0.78)),
                key(0.28, "wing_r", rot=(0.0, 0.0, 0.68)),
                key(0.56, "wing_r", rot=(0.0, 0.0, -0.78)),
                key(0.0, "leg_l", rot=(0.25, 0.0, 0.0)),
                key(0.28, "leg_l", rot=(-0.15, 0.0, 0.0)),
                key(0.56, "leg_l", rot=(0.25, 0.0, 0.0)),
                key(0.0, "leg_r", rot=(-0.15, 0.0, 0.0)),
                key(0.28, "leg_r", rot=(0.25, 0.0, 0.0)),
                key(0.56, "leg_r", rot=(-0.15, 0.0, 0.0)),
            ]),
        },
    )


# ---- 19-20. shark & piranha -------------------------------------------------
def _fish(kind, w, h, length, top_c, bot_c, fin_c):
    body = Model(w, h, length)
    body.ellipsoid(w / 2 - 0.5, h / 2 - 0.5, length / 2 - 0.5,
                   w / 2, h / 2, length / 2, top_c)
    for (x, y, z) in list(body.v):
        if y < h // 2:
            body.v[(x, y, z)] = bot_c
    # Dorsal fin
    body.box(w // 2 - 1, h - 1, length // 2 - 2, w // 2, h + 3 if h + 3 < h else h - 1, length // 2 + 1, fin_c)
    # Eyes
    body.set(1, h // 2 + 1, length - 3, hexc("#111111"))
    body.set(w - 2, h // 2 + 1, length - 3, hexc("#111111"))

    tail = Model(4, h, 8)
    tail.box(1, h // 4, 2, 2, h * 3 // 4, 7, fin_c)
    tail.box(1, 0, 0, 2, h - 1, 3, fin_c)
    tail.v.pop((1, 0, 0), None)

    jaw = Model(w - 2, 3, 6)
    jaw.box(0, 0, 0, w - 3, 2, 5, bot_c)

    save_parts(kind, {"body": body, "tail": tail, "jaw": jaw})
    write_rig(
        out(kind),
        [
            ("body", (w // 2, h // 2, length // 2), (0, 8, 0), None),
            ("tail", (2, h // 2, 7), (0, 0, -length // 2), "body"),
            ("jaw", ((w - 2) // 2, 2, 0), (0, -h // 3, length // 2 - 4), "body"),
        ],
        {
            "idle": (0.7, True, [
                key(0.0, "tail", rot=(0, 0.35, 0)),
                key(0.35, "tail", rot=(0, -0.35, 0)),
                key(0.0, "jaw", rot=(0.25, 0, 0)),
                key(0.35, "jaw", rot=(0.0, 0, 0)),
            ]),
        },
    )


def shark():
    _fish("shark", 10, 10, 24, hexc("#5a6b7c"), hexc("#dce4ec"), hexc("#3f4d5c"))


def piranha():
    _fish("piranha", 6, 8, 12, hexc("#4a7c59"), hexc("#c93b3b"), hexc("#2f523a"))


def main():
    blood_zombie()
    goblin_peon()
    goblin_thief()
    goblin_warrior()
    goblin_sorcerer()
    goblin_archer()
    snow_flinx()
    crab()
    ice_slime()
    jungle_slime()
    servant_of_cthulhu()
    bee()
    drippler()
    antlion()
    vulture()
    hornet()
    man_eater()
    harpy()
    shark()
    piranha()


if __name__ == "__main__":
    main()
