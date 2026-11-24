"""5 bespoke 16-VPU humanoid rigs for town NPCs: nurse, demolitionist, dryad, goblin_tinkerer, old_man."""
from _player_base import (
    HAIR, HAIR_D, HAIR_L,
    OFFSETS, PANTS, PANTS_D, PIVOTS,
    SHIRT, SHIRT_D, SHOE, SHOE_D,
    SKIN, SKIN_D, parts,
)
from _rigtools import humanoid_anims, out_dir, recolor, save_parts, write_rig
from palette import hexc
from vox import Model


def _write_humanoid(kind, ps, anims, offsets=OFFSETS, pivots=PIVOTS):
    save_parts(kind, ps)
    write_rig(
        out_dir(kind),
        [(n, pivots[n], offsets[n], None) for n in ("torso", "head", "arm_l", "arm_r", "leg_l", "leg_r")],
        anims,
    )


def nurse():
    WHITE = hexc("#f4f6fa")
    WHITE_D = hexc("#cfd6e3")
    RED = hexc("#d42b2b")
    RED_D = hexc("#9e1c1c")
    HAIR_A, HAIR_AD, HAIR_AL = hexc("#8e3a1f"), hexc("#612310"), hexc("#b35130")
    mp = {
        HAIR: HAIR_A, HAIR_D: HAIR_AD, HAIR_L: HAIR_AL,
        SHIRT: WHITE, SHIRT_D: WHITE_D,
        PANTS: WHITE, PANTS_D: WHITE_D,
        SHOE: WHITE, SHOE_D: WHITE_D,
    }
    ps = {n: recolor(m, mp) for n, m in parts().items()}

    # Nurse cap with red cross on front + auburn hair bun on back.
    base = ps["head"]
    head = Model(12, 16, 12)
    head.v.update(base.v)
    head.box(2, 12, 3, 9, 14, 9, WHITE)
    head.box(3, 15, 4, 8, 15, 8, WHITE_D)
    head.box(2, 12, 9, 9, 13, 9, WHITE_D)
    # Red cross on cap front
    head.box(5, 12, 9, 6, 14, 9, RED)
    head.box(4, 13, 9, 7, 13, 9, RED)
    # Auburn hair bun at back of head
    head.box(4, 5, 0, 7, 8, 0, HAIR_A)
    head.box(5, 6, 0, 6, 7, 0, HAIR_AL)
    ps["head"] = head

    # Medical dress + red cross emblem on chest.
    t = ps["torso"]
    t.box(1, 0, 0, 10, 3, 5, WHITE)
    t.box(1, 0, 0, 10, 0, 5, WHITE_D)
    t.box(5, 6, 5, 6, 8, 5, RED)
    t.box(4, 7, 5, 7, 7, 5, RED)
    t.box(5, 3, 5, 6, 3, 5, RED_D)  # belt clasp

    _write_humanoid("nurse", ps, humanoid_anims())


def demolitionist():
    BEARD = hexc("#c84a1e")
    BEARD_D = hexc("#943010")
    BEARD_L = hexc("#e36835")
    HELM = hexc("#5a606e")
    HELM_D = hexc("#3c414d")
    LAMP = hexc("#ffe45e")
    APRON = hexc("#6b4423")
    APRON_D = hexc("#4a2d15")
    DYN = hexc("#d42b2b")
    FUSE = hexc("#e8d490")
    mp = {
        HAIR: BEARD, HAIR_D: BEARD_D, HAIR_L: BEARD_L,
        SHIRT: hexc("#545b68"), SHIRT_D: hexc("#3a3f4a"),
        PANTS: hexc("#3d3630"), PANTS_D: hexc("#28231e"),
        SHOE: hexc("#2b1c12"), SHOE_D: hexc("#1a100a"),
    }
    ps = {n: recolor(m, mp) for n, m in parts().items()}

    # Miner's helmet with yellow headlamp + bushy red-orange beard.
    base = ps["head"]
    head = Model(12, 16, 12)
    head.v.update(base.v)
    head.box(1, 10, 1, 10, 12, 10, HELM)
    head.box(2, 13, 2, 9, 14, 9, HELM)
    head.box(3, 15, 3, 8, 15, 8, HELM_D)
    head.box(0, 10, 2, 11, 10, 11, HELM_D)  # brim
    head.box(5, 11, 11, 6, 12, 11, LAMP)   # headlamp
    # Bushy beard
    head.box(2, 0, 11, 9, 4, 11, BEARD)
    head.box(3, 0, 11, 8, 1, 11, BEARD_D)
    head.box(4, 5, 11, 7, 5, 11, BEARD_L)
    head.box(1, 2, 9, 1, 5, 10, BEARD_D)
    head.box(10, 2, 9, 10, 5, 10, BEARD_D)
    ps["head"] = head

    # Stocky leather apron with strapped red dynamite sticks.
    t = ps["torso"]
    t.box(2, 1, 5, 9, 8, 5, APRON)
    t.box(3, 2, 5, 8, 2, 5, APRON_D)
    # Dynamite sticks strapped across chest
    for dx in (3, 5, 7):
        t.box(dx, 4, 5, dx, 7, 5, DYN)
        t.set(dx, 8, 5, FUSE)
    t.box(2, 5, 5, 9, 5, 5, APRON_D)  # strap

    # Stocky dwarf offsets (slightly shorter legs/lower torso).
    dwarf_offsets = dict(OFFSETS)
    dwarf_offsets["torso"] = (0, 6.5, 0)
    dwarf_offsets["head"] = (0, 18.5, 0)
    dwarf_offsets["arm_l"] = (8, 18.5, 0)
    dwarf_offsets["arm_r"] = (-8, 18.5, 0)
    dwarf_offsets["leg_l"] = (2.5, 10.5, 0)
    dwarf_offsets["leg_r"] = (-2.5, 10.5, 0)
    _write_humanoid("demolitionist", ps, humanoid_anims(), offsets=dwarf_offsets)


def dryad():
    SKIN_N, SKIN_ND = hexc("#f2c8b0"), hexc("#d4a288")
    HAIR_N, HAIR_ND, HAIR_NL = hexc("#1f5e36"), hexc("#144024"), hexc("#33854f")
    LEAF, LEAF_D, LEAF_L = hexc("#2e8f46"), hexc("#1e6330"), hexc("#4eb862")
    VINE = hexc("#5a8f3a")
    mp = {
        SKIN: SKIN_N, SKIN_D: SKIN_ND,
        HAIR: HAIR_N, HAIR_D: HAIR_ND, HAIR_L: HAIR_NL,
        SHIRT: LEAF, SHIRT_D: LEAF_D,
        PANTS: SKIN_N, PANTS_D: SKIN_ND,
        SHOE: VINE, SHOE_D: LEAF_D,
    }
    ps = {n: recolor(m, mp) for n, m in parts().items()}

    # Long flowing dark-green hair down back of head.
    h = ps["head"]
    h.box(2, 0, 0, 9, 9, 0, HAIR_ND)
    h.box(1, 2, 1, 10, 9, 1, HAIR_N)
    h.set(3, 10, 11, LEAF_L)  # leaf hair ornament
    h.set(8, 10, 11, LEAF_L)

    # Layered emerald leaf tunic.
    t = ps["torso"]
    t.box(1, 0, 0, 10, 2, 5, LEAF_D)
    t.box(2, 1, 5, 9, 3, 5, LEAF)
    t.box(3, 6, 5, 8, 9, 5, LEAF_L)
    t.box(2, 4, 0, 9, 8, 0, HAIR_ND)  # flowing hair over back

    # Vine-wrapped forearms and ankles.
    for arm in (ps["arm_l"], ps["arm_r"]):
        arm.box(0, 0, 0, 3, 5, 3, SKIN_N)
        arm.box(0, 2, 0, 3, 2, 3, VINE)
        arm.box(0, 4, 0, 3, 4, 3, LEAF)
    for leg in (ps["leg_l"], ps["leg_r"]):
        leg.box(0, 2, 0, 4, 3, 4, VINE)

    _write_humanoid("dryad", ps, humanoid_anims())


def goblin_tinkerer():
    GOB, GOB_D = hexc("#639c49"), hexc("#477333")
    HAIR_G, HAIR_GD, HAIR_GL = hexc("#2b3a2e"), hexc("#1b261d"), hexc("#3f5443")
    GOG_BRASS = hexc("#c89832")
    GOG_LENS = hexc("#7be3f5")
    LEATHER = hexc("#6e4524")
    LEATHER_D = hexc("#4a2d16")
    WRENCH = hexc("#9aa3b2")
    mp = {
        SKIN: GOB, SKIN_D: GOB_D,
        HAIR: HAIR_G, HAIR_D: HAIR_GD, HAIR_L: HAIR_GL,
        SHIRT: hexc("#8a5d3b"), SHIRT_D: hexc("#634128"),
        PANTS: hexc("#353945"), PANTS_D: hexc("#23262e"),
        SHOE: LEATHER_D, SHOE_D: hexc("#26160b"),
    }
    ps = {n: recolor(m, mp) for n, m in parts().items()}

    # Pointed goblin ears + brass aviator goggles on forehead.
    base = ps["head"]
    head = Model(16, 12, 12)
    for (x, y, z), c in base.v.items():
        head.set(x + 2, y, z, c)
    # Pointed ears
    head.box(0, 6, 5, 1, 7, 6, GOB_D)
    head.set(0, 8, 5, GOB)
    head.box(14, 6, 5, 15, 7, 6, GOB)
    head.set(15, 8, 5, GOB)
    # Brass aviator goggles on forehead
    head.box(3, 9, 11, 12, 10, 11, GOG_BRASS)
    head.box(4, 9, 11, 6, 10, 11, GOG_LENS)
    head.box(9, 9, 11, 11, 10, 11, GOG_LENS)
    ps["head"] = head

    # Leather toolbelt/apron with pouches & wrench accent.
    t = ps["torso"]
    t.box(1, 2, 0, 10, 3, 5, LEATHER)
    t.box(2, 1, 5, 4, 3, 5, LEATHER_D)  # pouch
    t.box(7, 1, 5, 9, 3, 5, LEATHER_D)
    t.box(5, 4, 5, 6, 8, 5, WRENCH)     # wrench tucked in belt

    gob_pivots = dict(PIVOTS)
    gob_pivots["head"] = (8, 0, 6)
    _write_humanoid("goblin_tinkerer", ps, humanoid_anims(), pivots=gob_pivots)


def old_man():
    ROBE = hexc("#5a4e46")
    ROBE_D = hexc("#3d342e")
    ROBE_L = hexc("#73655b")
    BEARD = hexc("#e8e8ee")
    BEARD_D = hexc("#b8b8c4")
    STAFF = hexc("#6b4423")
    STAFF_D = hexc("#4a2d15")
    mp = {
        HAIR: BEARD, HAIR_D: BEARD_D, HAIR_L: BEARD,
        SHIRT: ROBE, SHIRT_D: ROBE_D,
        PANTS: ROBE_D, PANTS_D: ROBE_D,
        SHOE: hexc("#2b211a"), SHOE_D: hexc("#1a130f"),
    }
    ps = {n: recolor(m, mp) for n, m in parts().items()}

    # Deep tattered hood + long white beard + shadowed face.
    base = ps["head"]
    head = Model(12, 14, 12)
    head.v.update(base.v)
    head.box(1, 9, 0, 10, 12, 11, ROBE)
    head.box(2, 13, 1, 9, 13, 10, ROBE_D)
    head.box(0, 3, 0, 1, 10, 10, ROBE_D)
    head.box(10, 3, 0, 11, 10, 10, ROBE_D)
    head.box(2, 8, 11, 9, 10, 11, ROBE_D)  # hood brow shadow
    # Long white beard
    head.box(3, 0, 11, 8, 4, 11, BEARD)
    head.box(4, 0, 11, 7, 1, 11, BEARD_D)
    ps["head"] = head

    # Tattered robe hem.
    t = ps["torso"]
    t.box(1, 0, 0, 10, 3, 5, ROBE_D)
    t.box(2, 1, 5, 9, 2, 5, ROBE_L)

    # Knotted wooden staff held in right hand (arm_r).
    ar = ps["arm_r"]
    ar_staff = Model(4, 16, 8)
    for (x, y, z), c in ar.v.items():
        ar_staff.set(x, y + 4, z + 2, c)
    ar_staff.box(1, 0, 6, 2, 14, 7, STAFF)
    ar_staff.box(1, 13, 5, 2, 15, 7, STAFF_D)  # knotted top
    ps["arm_r"] = ar_staff

    old_pivots = dict(PIVOTS)
    old_pivots["arm_r"] = (2, 16, 4)
    _write_humanoid("old_man", ps, humanoid_anims(), pivots=old_pivots)


nurse()
demolitionist()
dryad()
goblin_tinkerer()
old_man()
