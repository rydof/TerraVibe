"""Armor sets (copper, iron, silver, gold): helmet, body (torso + both arms) and
legs (both legs). Each model replaces a player part and is exactly one voxel
larger on every side, so the client meshes it with the rig pivot + (1, 1, 1).
Every armor voxel sits within Chebyshev distance 1 of the player part it covers.
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.dirname(HERE))

import _player_base as base
from palette import *
from vox import Model

OUT = os.path.join(ASSETS, "models", "armor")
PAD = 1
RED = hexc("#d8283c")
RED_L = hexc("#ff7a8a")
BLUE_G = hexc("#4fb6ff")
MOLT_CRUST = hexc("#1c171d")
MOLT_BASALT = hexc("#332b34")
MOLT_BASALT_L = hexc("#4a4048")
LAVA_R = hexc("#c4410f")
LAVA_O = hexc("#ff8a1e")
LAVA_Y = hexc("#ffcf33")
LAVA_W = hexc("#fff3a0")

NEIGHBOURS = [(dx, dy, dz) for dx in (-1, 0, 1) for dy in (-1, 0, 1) for dz in (-1, 0, 1)]


def shell(src):
    """The 26-neighbourhood of a part: the only cells an armor piece may use."""
    return {
        (x + dx, y + dy, z + dz)
        for (x, y, z) in src.v
        for dx, dy, dz in NEIGHBOURS
    }


class Sub:
    """A padded model drawn in the base part's own coordinates."""

    def __init__(self, size, allowed):
        self.m = Model(size[0] + 2 * PAD, size[1] + 2 * PAD, size[2] + 2 * PAD)
        self.allowed = allowed

    def set(self, x, y, z, c):
        if (x, y, z) in self.allowed:
            self.m.set(x + PAD, y + PAD, z + PAD, c)

    def box(self, x0, y0, z0, x1, y1, z1, c):
        for x in range(min(x0, x1), max(x0, x1) + 1):
            for y in range(min(y0, y1), max(y0, y1) + 1):
                for z in range(min(z0, z1), max(z0, z1) + 1):
                    self.set(x, y, z, c)

    def blit(self, src):
        for (x, y, z), c in src.v.items():
            self.set(x, y, z, c)

    def save(self, name):
        self.m.save(os.path.join(OUT, name + ".vox"))


class Tier:
    def __init__(self, name, main, light, dark, trim):
        self.name, self.main, self.light, self.dark, self.trim = name, main, light, dark, trim


COPPER_T = Tier("copper", COPPER, COPPER_L, COPPER_D, hexc("#7a3410"))
IRON_T = Tier("iron", IRON, IRON_L, IRON_D, hexc("#44454f"))
SILVER_T = Tier("silver", SILVER, SILVER_L, SILVER_D, hexc("#5a6a86"))
GOLD_T = Tier("gold", GOLD, GOLD_L, GOLD_D, hexc("#7d5f0a"))
SHADOW_T = Tier("shadow", DEMONITE, DEMONITE_L, DEMONITE_D, EBONSTONE_DD)
MOLTEN_T = Tier("molten", MOLT_BASALT, MOLT_BASALT_L, MOLT_CRUST, LAVA_R)
METEOR_T = Tier("meteor", hexc("#5a4468"), hexc("#8b6ea0"), hexc("#2f2238"), hexc("#39d8ff"))
JUNGLE_T = Tier("jungle", hexc("#2e7d32"), hexc("#66bb6a"), hexc("#1b4d20"), hexc("#e94b7a"))
BEE_T = Tier("bee", hexc("#d4a017"), hexc("#ffe066"), hexc("#241c14"), hexc("#1a140e"))
NINJA_T = Tier("ninja", hexc("#22253a"), hexc("#3b4166"), hexc("#121422"), hexc("#c62828"))
NECRO_T = Tier("necro", hexc("#d8d0be"), hexc("#f2ecdc"), hexc("#8c8372"), hexc("#26222c"))


def helmet(t):
    src = base.head()
    s = Sub(base.SIZES["head"], shell(src))
    s.blit(src)
    # Plate over the crown, back, sides and hair, with the face window open.
    s.box(0, 8, 0, 11, 11, 11, t.main)
    s.box(0, 0, 0, 11, 11, 1, t.main)
    s.box(0, 0, 0, 1, 11, 11, t.main)
    s.box(10, 0, 0, 11, 11, 11, t.main)
    s.box(2, 8, 11, 9, 11, 11, t.main)  # brow/fringe
    s.box(0, 0, 11, 1, 11, 11, t.main)  # cheek guards
    s.box(10, 0, 11, 11, 11, 11, t.main)
    s.box(0, 0, 11, 11, 0, 11, t.main)  # chin guard
    # The 1-voxel shell: top, back and sides.
    s.box(0, 12, 0, 11, 12, 11, t.light)
    s.box(0, 0, -1, 11, 11, -1, t.dark)
    s.box(-1, 0, 0, -1, 11, 11, t.main)
    s.box(12, 0, 0, 12, 11, 11, t.main)
    if t.name == "copper":
        # Open-faced crested centurion helm with riveted brow band & dorsal ridge.
        s.box(5, 12, 0, 6, 12, 11, t.dark)
        s.box(5, 12, 1, 6, 12, 10, t.main)
        s.box(5, 12, 3, 6, 12, 9, t.light)  # raised dorsal crest
        s.box(2, 8, 12, 9, 9, 12, t.dark)  # riveted brow band
        s.box(3, 9, 12, 8, 9, 12, t.light)
        for x in (1, 10):
            s.set(x, 8, 12, t.light)
        for y in (2, 5):
            s.set(0, y, 11, t.light)
            s.set(11, y, 11, t.light)
        s.box(0, 1, 11, 0, 7, 11, t.dark)
        s.box(11, 1, 11, 11, 7, 11, t.dark)
    elif t.name == "iron":
        # Full closed knight greathelm with deep T-visor slit, steel nasal bar, and domed crest.
        s.box(2, 1, 11, 9, 7, 11, t.main)
        s.box(2, 7, 12, 9, 8, 12, t.main)  # heavy brow
        s.box(2, 5, 12, 9, 6, 12, t.trim)  # deep horizontal visor slit
        s.box(5, 2, 11, 6, 6, 11, t.trim)  # T-visor vertical channel
        s.box(5, 2, 12, 6, 7, 12, t.light)  # steel nasal bar
        s.box(2, 1, 12, 4, 4, 12, t.dark)  # lower cheek plates
        s.box(7, 1, 12, 9, 4, 12, t.dark)
        s.set(3, 2, 12, t.trim)  # breathing vents
        s.set(8, 2, 12, t.trim)
        s.box(5, 12, 0, 6, 12, 11, t.dark)  # domed crest
        s.box(5, 12, 2, 6, 12, 9, t.light)
        for x, y in ((0, 8), (11, 8), (2, 1), (9, 1)):
            s.set(x, y, 12, t.light)  # rivets
    elif t.name == "silver":
        # Radiant paladin helm with bright winged brow ridge and glowing sapphire forehead gem.
        s.box(4, 12, 1, 7, 12, 10, t.light)
        s.box(5, 12, 2, 6, 12, 9, t.main)
        s.box(2, 8, 12, 9, 8, 12, t.dark)
        s.box(1, 9, 12, 3, 9, 12, t.light)  # winged brow ridge
        s.box(8, 9, 12, 10, 9, 12, t.light)
        s.box(5, 8, 12, 6, 9, 12, BLUE_G)  # sapphire forehead gem
        s.box(0, 1, 11, 0, 7, 11, t.dark)
        s.box(11, 1, 11, 11, 7, 11, t.dark)
        s.set(1, 3, 11, t.light)
        s.set(10, 3, 11, t.light)
    elif t.name == "gold":
        # Iconic Terraria winged golden helm with swept side wing-fins and ruby brow gem.
        s.box(5, 12, 1, 6, 12, 10, t.light)
        for x in (-1, 12):
            s.box(x, 5, 1, x, 10, 10, t.main)
            s.box(x, 7, 0, x, 11, 8, t.light)  # swept wing-fin feather tips
            s.set(x, 11, 9, t.dark)
            s.box(x, 5, 2, x, 6, 9, t.dark)
        s.box(2, 8, 12, 9, 9, 12, t.dark)
        s.box(3, 9, 12, 8, 9, 12, t.light)
        s.box(5, 8, 12, 6, 9, 12, RED)  # ruby brow gem
        s.set(5, 9, 12, RED_L)
        s.box(0, 1, 11, 0, 7, 11, t.dark)
        s.box(11, 1, 11, 11, 7, 11, t.dark)
    elif t.name == "molten":
        # Iconic Terraria magma-horned greathelm with blazing white-yellow eye slit.
        s.box(2, 1, 11, 9, 7, 11, t.dark)
        s.box(5, 12, 1, 6, 12, 10, LAVA_R)  # magma dorsal crest
        s.box(5, 12, 3, 6, 12, 9, LAVA_O)
        s.set(5, 12, 5, LAVA_Y)
        s.set(6, 12, 7, LAVA_Y)
        s.box(2, 8, 12, 9, 8, 12, LAVA_R)  # brow band
        s.box(3, 6, 12, 8, 6, 12, LAVA_O)  # blazing eye slit
        s.box(4, 6, 12, 7, 6, 12, LAVA_Y)
        s.set(4, 6, 12, LAVA_W)
        s.set(7, 6, 12, LAVA_W)
        s.box(4, 2, 12, 7, 4, 12, t.main)  # basalt chin plate
        s.set(5, 3, 12, LAVA_R)
        for x in (-1, 12):
            s.box(x, 7, 0, x, 10, 4, t.dark)  # sweeping magma horns
            s.box(x, 9, 1, x, 11, 3, LAVA_O)
            s.box(x, 11, 1, x, 12, 2, LAVA_Y)
        s.box(0, 1, 11, 0, 7, 11, t.dark)
        s.box(11, 1, 11, 11, 7, 11, t.dark)
    else:
        # Shadow: Iconic Terraria curved demonite horned helmet with glowing violet visor eyes.
        s.box(2, 2, 11, 9, 7, 11, t.trim)  # recessed dark visor
        s.box(5, 12, 1, 6, 12, 10, t.light)  # amethyst dorsal crest
        for x in (-1, 12):
            s.box(x, 6, 0, x, 10, 5, t.dark)  # curved demonite horns
            s.box(x, 9, 1, x, 12, 4, t.main)
            s.box(x, 11, 1, x, 12, 2, t.light)
        s.box(2, 8, 12, 9, 8, 12, t.dark)
        s.box(3, 6, 12, 4, 6, 12, t.light)  # glowing violet visor eyes
        s.box(7, 6, 12, 8, 6, 12, t.light)
        s.box(4, 2, 12, 7, 4, 12, t.dark)
        s.box(0, 1, 11, 0, 7, 11, t.dark)
        s.box(11, 1, 11, 11, 7, 11, t.dark)
    return s


def body(t):
    src = base.torso()
    s = Sub(base.SIZES["torso"], shell(src))
    s.blit(src)
    # Plate shell: front, back, the torso's own side columns and top.
    s.box(0, 0, 6, 11, 11, 6, t.main)
    s.box(0, 0, -1, 11, 11, -1, t.main)
    s.box(0, 0, 0, 0, 11, 5, t.main)
    s.box(11, 0, 0, 11, 11, 5, t.main)
    s.box(0, 11, 0, 11, 11, 5, t.main)
    s.box(0, 0, -1, 11, 1, 6, t.dark)  # belt band
    if t.name == "copper":
        # Bevelled copper cuirass with pec plates, chainmail skirt, and riveted belt.
        for y in range(2, 6):
            for x in range(1, 11):
                s.set(x, y, 6, t.light if (x + y) % 2 == 0 else t.dark)
        s.box(1, 6, 6, 4, 9, 6, t.light)  # pec relief
        s.box(7, 6, 6, 10, 9, 6, t.light)
        s.box(5, 5, 6, 6, 9, 6, t.dark)  # sternum seam
        s.box(0, 1, 6, 11, 1, 6, t.trim)
        s.box(4, 1, 6, 7, 1, 6, t.light)  # belt buckle
    elif t.name == "iron":
        # Bevelled knight breastplate with sternum ridge, pecs, belt buckle & tassets.
        s.box(1, 2, 6, 10, 10, 6, t.main)
        s.box(2, 6, 6, 4, 9, 6, t.light)
        s.box(7, 6, 6, 9, 9, 6, t.light)
        s.box(5, 2, 6, 6, 10, 6, t.light)
        s.box(1, 10, 6, 10, 10, 6, t.light)
        for x, y in ((1, 2), (10, 2), (1, 9), (10, 9)):
            s.set(x, y, 6, t.trim)
        s.box(0, 1, 6, 11, 1, 6, t.trim)
        s.box(5, 1, 6, 6, 1, 6, t.light)  # belt buckle
        s.box(0, 0, 6, 3, 0, 6, t.dark)  # layered tassets
        s.box(8, 0, 6, 11, 0, 6, t.dark)
        s.box(0, 11, 6, 11, 11, 6, t.light)
    elif t.name == "silver":
        # Fluted paladin chestplate with sapphire emblem, buckle & waist tassets.
        s.box(2, 2, 6, 9, 10, 6, t.light)
        s.box(3, 2, 6, 3, 10, 6, t.dark)
        s.box(8, 2, 6, 8, 10, 6, t.dark)
        s.box(5, 7, 6, 6, 8, 6, BLUE_G)  # sapphire chest emblem
        s.box(0, 1, 6, 11, 1, 6, t.trim)
        s.box(5, 1, 6, 6, 1, 6, t.light)
        s.box(0, 0, 6, 4, 0, 6, t.light)  # waist tassets
        s.box(7, 0, 6, 11, 0, 6, t.light)
        s.box(0, 11, 6, 11, 11, 6, t.main)
    elif t.name == "gold":
        # Ornate gold plate with ruby chest emblem, gilded pec trim & flared tassets.
        s.box(1, 3, 6, 10, 10, 6, t.main)
        s.box(2, 6, 6, 4, 9, 6, t.light)
        s.box(7, 6, 6, 9, 9, 6, t.light)
        s.box(5, 7, 6, 6, 8, 6, RED)  # ruby emblem
        s.set(5, 8, 6, RED_L)
        s.box(1, 2, 6, 10, 2, 6, t.light)
        s.box(0, 11, 6, 11, 11, 6, t.light)
        s.box(-1, -1, -1, 12, -1, 6, t.dark)  # flared skirt tassets
        s.box(0, 0, 6, 11, 0, 6, t.trim)
        s.box(5, 1, 6, 6, 1, 6, t.light)  # gilded buckle
    elif t.name == "molten":
        # Cracked basalt cuirass with glowing molten lava core & ribs.
        s.box(1, 2, 6, 10, 10, 6, t.main)
        s.box(5, 4, 6, 6, 8, 6, LAVA_O)  # molten lava core
        s.box(5, 5, 6, 6, 7, 6, LAVA_Y)
        s.set(5, 6, 6, LAVA_W)
        for y in (4, 6, 8):
            s.box(2, y, 6, 4, y, 6, LAVA_R)  # glowing lava ribs
            s.box(7, y, 6, 9, y, 6, LAVA_R)
            s.set(3, y, 6, LAVA_O)
            s.set(8, y, 6, LAVA_O)
        s.box(5, 2, -1, 6, 10, -1, LAVA_O)  # back lava spine
        s.box(0, 11, 6, 11, 11, 6, LAVA_R)  # glowing collar edge
        s.box(-1, -1, -1, 12, -1, 6, t.dark)  # flared tassets
        s.box(0, 0, 6, 11, 0, 6, t.trim)  # belt
        s.box(5, 1, 6, 6, 1, 6, LAVA_Y)
    else:
        # Shadow scalemail: layered demonite scales with glowing amethyst spine & belt.
        for y in range(2, 11):
            c = t.main if y % 2 == 0 else t.dark
            s.box(1, y, 6, 10, y, 6, c)
        s.box(5, 2, 6, 6, 10, 6, t.light)  # glowing amethyst spine
        s.box(5, 2, -1, 6, 10, -1, t.light)
        s.box(0, 1, 6, 11, 1, 6, t.trim)
        s.box(5, 1, 6, 6, 1, 6, t.light)
        s.box(0, 11, 6, 11, 11, 6, t.dark)
        s.box(-1, -1, -1, 12, -1, 6, t.dark)
    s.box(5, 10, 6, 6, 11, 6, t.light)  # collar below the head
    # Rounded top corners: keep the shell from reading as a 2x dilation.
    for x in (0, 11):
        for z in (0, 5):
            s.set(x, 11, z, None)
    return s


def arm_piece(t, outer_x):
    src = base.arm(outer_x)
    s = Sub(base.SIZES["arm_l"], shell(src))
    s.blit(src)
    # outer_x is 3 for arm_l and 0 for arm_r; pad only the side away from the torso.
    pad_x = 4 if outer_x == 3 else -1
    x0, x1 = (0, 4) if outer_x == 3 else (-1, 3)
    # Sleeve plate, leaving the two skin hand rows visible.
    s.box(0, 2, 0, 3, 11, 3, t.main)
    s.box(outer_x, 2, 0, outer_x, 11, 3, t.dark)
    s.box(0, 2, 0, 3, 2, 3, t.dark)  # cuff
    # One-voxel shell around the sleeve: front, back and the outer side only.
    s.box(0, 3, -1, 3, 11, -1, t.main)
    s.box(0, 3, 4, 3, 11, 4, t.main)
    s.box(pad_x, 3, 0, pad_x, 11, 3, t.main)
    s.box(x0, 12, -1, x1, 12, 4, t.light)  # shoulder pad
    if t.name == "copper":
        s.box(x0, 10, -1, x1, 11, 4, t.main)
        s.set(0, 12, 0, t.light)
        s.set(3, 12, 3, t.light)
        s.set(pad_x, 11, 1, t.light)
        s.box(0, 2, 4, 3, 2, 4, t.trim)
    elif t.name == "iron":
        s.box(x0, 10, -1, x1, 12, 4, t.main)
        s.box(x0, 12, -1, x1, 12, 4, t.light)
        s.box(pad_x, 10, 0, pad_x, 11, 3, t.dark)
        s.box(0, 3, 4, 3, 3, 4, t.trim)
        s.box(0, 2, 4, 3, 2, 4, t.light)  # gauntlet cuff
    elif t.name == "silver":
        s.box(x0, 10, -1, x1, 11, 4, t.main)
        s.box(x0, 12, 0, x1, 12, 3, t.light)
        s.box(pad_x, 11, 1, pad_x, 12, 2, BLUE_G)
        s.box(0, 3, 4, 3, 3, 4, t.light)
        s.box(0, 2, 4, 3, 2, 4, t.dark)
    elif t.name == "gold":
        s.box(x0, 10, -1, x1, 11, 4, t.light)
        s.box(0, 12, 0, 3, 12, 3, t.main)
        s.set(pad_x, 11, 1, RED)
        s.set(pad_x, 11, 2, RED_L)
        s.box(0, 3, 4, 3, 3, 4, t.light)
        s.box(0, 2, 4, 3, 2, 4, t.main)
    elif t.name == "molten":
        # Fiery-spiked pauldron with glowing lava ridges & ember gauntlet.
        s.box(x0, 10, -1, x1, 12, 4, t.dark)
        s.box(x0, 12, -1, x1, 12, 4, LAVA_O)
        s.box(pad_x, 10, 1, pad_x, 12, 2, LAVA_Y)
        s.box(pad_x, 5, 1, pad_x, 8, 2, LAVA_R)  # glowing arm crack
        s.box(0, 3, 4, 3, 3, 4, t.trim)
        s.box(0, 2, 4, 3, 2, 4, LAVA_O)
    else:
        # Spiked dark-purple demonite pauldron & vambrace.
        s.box(x0, 10, -1, x1, 12, 4, t.dark)
        s.box(pad_x, 11, 0, pad_x, 12, 3, t.main)
        s.set(pad_x, 12, 1, t.light)
        s.set(pad_x, 12, 2, t.light)
        s.box(1, 12, 1, 2, 12, 2, t.light)
        s.box(0, 3, 4, 3, 3, 4, t.trim)
        s.box(0, 2, 4, 3, 2, 4, t.light)
    return s


def leg_piece(t, outer_x):
    src = base.leg(outer_x)
    s = Sub(base.SIZES["leg_l"], shell(src))
    s.blit(src)
    # outer_x is 4 for leg_l and 0 for leg_r; pad only the side away from the other leg.
    pad_x = 5 if outer_x == 4 else -1
    x0, x1 = (0, 5) if outer_x == 4 else (-1, 4)
    # Greave over the pants and boot.
    s.box(0, 3, 0, 4, 11, 4, t.main)
    s.box(outer_x, 3, 0, outer_x, 11, 4, t.dark)
    s.box(0, 3, 0, 4, 3, 4, t.dark)  # boot cuff
    s.box(0, 0, 0, 4, 2, 4, t.main)  # boot plate
    s.box(0, 0, 0, 4, 0, 4, t.dark)
    s.box(0, 3, -1, 4, 11, -1, t.main)
    s.box(0, 3, 5, 4, 11, 5, t.main)
    s.box(pad_x, 3, 0, pad_x, 11, 4, t.main)
    s.box(x0, 2, -1, x1, 2, 5, t.light)  # boot cuff rim
    if t.name == "copper":
        s.box(1, 6, 5, 3, 9, 5, t.light)  # sculpted knee guard
        s.set(2, 7, 5, t.main)
        s.box(2, 4, 5, 2, 5, 5, t.dark)  # shin ridge
        s.box(0, 2, 5, 4, 2, 5, t.trim)
        s.box(0, 3, 5, 4, 3, 5, t.light)  # cuffed greave rim
    elif t.name == "iron":
        s.box(1, 6, 5, 3, 9, 5, t.main)
        s.box(1, 9, 5, 3, 9, 5, t.light)
        s.box(2, 4, 5, 2, 7, 5, t.light)  # bevelled shin ridge
        s.set(2, 8, 5, t.trim)
        s.box(0, 3, 5, 4, 3, 5, t.trim)
        s.box(0, 2, 5, 4, 2, 5, t.light)
    elif t.name == "silver":
        s.box(1, 6, 5, 3, 9, 5, t.light)
        s.box(2, 4, 5, 2, 9, 5, t.dark)
        s.set(2, 8, 5, BLUE_G)
        s.set(2, 7, 5, BLUE_G)
        s.box(0, 3, 5, 4, 3, 5, t.dark)
        s.box(0, 2, 5, 4, 2, 5, t.light)
    elif t.name == "gold":
        s.box(x0, 0, -1, x1, 0, 5, t.dark)  # flared boot, not below the ground
        s.box(1, 6, 5, 3, 9, 5, t.main)
        s.box(1, 9, 5, 3, 9, 5, t.light)
        s.box(2, 4, 5, 2, 7, 5, t.light)
        s.set(2, 8, 5, RED)
        s.set(2, 7, 5, RED_L)
        s.box(0, 3, 5, 4, 3, 5, t.trim)
        s.box(0, 2, 5, 4, 2, 5, t.light)
    elif t.name == "molten":
        # Molten greaves: basalt plate, magma knee rim and glowing lava cracks.
        s.box(x0, 0, -1, x1, 0, 5, t.dark)
        s.box(1, 6, 5, 3, 9, 5, t.main)  # knee plate
        s.box(1, 9, 5, 3, 9, 5, LAVA_O)  # knee rim
        s.box(2, 4, 5, 2, 8, 5, LAVA_R)  # lava shin crack
        s.set(2, 7, 5, LAVA_Y)
        s.box(pad_x, 4, 2, pad_x, 8, 2, LAVA_O)
        s.box(0, 2, 5, 4, 2, 5, t.trim)
    else:
        # Shadow greaves: layered demonite plates with glowing amethyst knee ridges.
        s.box(x0, 0, -1, x1, 0, 5, t.dark)
        s.box(1, 6, 5, 3, 9, 5, t.dark)
        s.box(1, 9, 5, 3, 9, 5, t.light)
        s.box(2, 5, 5, 2, 8, 5, t.light)
        s.box(pad_x, 6, 2, pad_x, 8, 2, t.light)
        s.box(0, 2, 5, 4, 2, 5, t.trim)
    return s


for t in (
    COPPER_T,
    IRON_T,
    SILVER_T,
    GOLD_T,
    SHADOW_T,
    MOLTEN_T,
    METEOR_T,
    JUNGLE_T,
    BEE_T,
    NINJA_T,
    NECRO_T,
):
    helmet(t).save(f"{t.name}_head")
    body(t).save(f"{t.name}_body")
    arm_piece(t, 3).save(f"{t.name}_arm_l")
    arm_piece(t, 0).save(f"{t.name}_arm_r")
    leg_piece(t, 4).save(f"{t.name}_leg_l")
    leg_piece(t, 0).save(f"{t.name}_leg_r")
