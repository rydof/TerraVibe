"""Workbench, anvil, furnace, chest, door (closed/open), table, chair, platform,
demon altar.

Furniture fills one block: 16x16x16 voxels, x right, y up, z front. A closed
door is one block wide and tall and 4 voxels thick (the sim places a door in
one cell). The demon altar fills its whole 3x2x1 group: 48x32x16 voxels.
"""
import os

from palette import *
from vox import Model

OUT = os.path.join(ASSETS, "models", "blocks")
PLANK = hexc("#b97a3e")
PLANK_L = hexc("#d79a5a")
PLANK_D = hexc("#8a5a2c")
PLANK_DD = hexc("#5e3b1c")
IRON_G = hexc("#8b8f9a")
IRON_GL = hexc("#b9bec9")
IRON_GD = hexc("#5d616c")
GOLDEN = hexc("#e6c040")
EMBER = hexc("#ff9420")
EMBER_L = hexc("#ffd84a")
EMBER_D = hexc("#c4410f")


def save(m, name):
    m.save(os.path.join(OUT, name + ".vox"))


# Workbench: a plank top with two gaps, four legs, an apron and a low shelf;
# a hammer and a saw lie on the top.
m = Model(16, 16, 16)
m.box(0, 12, 1, 15, 13, 14, PLANK)
m.box(0, 13, 1, 15, 13, 14, PLANK_L)
m.box(0, 12, 1, 15, 12, 14, PLANK_D)
for z in (5, 10):  # the gaps between the planks
    m.box(0, 12, z, 15, 13, z, PLANK_DD)
for x, z in [(0, 1), (14, 1), (0, 13), (14, 13)]:
    m.box(x, 0, z, x + 1, 11, z + 1, PLANK_D)
m.box(2, 9, 2, 13, 11, 2, PLANK_D)  # apron
m.box(2, 4, 3, 13, 5, 12, PLANK_DD)  # shelf
m.box(3, 2, 4, 12, 3, 4, PLANK_DD)
m.box(2, 14, 6, 5, 15, 8, IRON_G)  # hammer head
m.box(5, 14, 7, 10, 14, 7, PLANK_DD)  # hammer handle
m.box(11, 14, 10, 14, 15, 10, IRON_GL)  # saw blade
m.set(12, 14, 11, IRON_GD)
save(m, "workbench")

# Anvil: a foot, a waist, the wide face with a light top and a dark front
# edge, and a tapered horn on the left.
m = Model(16, 16, 16)
m.box(2, 0, 3, 13, 2, 12, IRON_GD)
m.box(4, 3, 4, 11, 5, 11, IRON_G)
m.box(3, 6, 2, 15, 8, 13, IRON_G)
m.box(3, 9, 2, 15, 9, 13, IRON_GL)  # face top
m.box(3, 6, 13, 15, 9, 13, IRON_GD)  # front edge
m.box(0, 7, 6, 2, 8, 9, IRON_G)  # horn
m.box(0, 7, 7, 0, 8, 8, IRON_GL)  # horn tip
m.box(11, 9, 6, 13, 9, 9, IRON_GD)  # hardy hole
save(m, "anvil")

# Furnace: a stone body with a dark plinth, a chimney and a glowing mouth
# (an ember ring, a bright bed and a hot core).
m = Model(16, 16, 16)
m.box(0, 0, 0, 15, 13, 15, STONE_D)
m.box(0, 0, 15, 15, 13, 15, STONE)
m.box(0, 0, 15, 0, 13, 15, STONE_DD)
m.box(15, 0, 15, 15, 13, 15, STONE_DD)
m.box(0, 13, 0, 15, 13, 15, STONE_L)
m.box(0, 0, 0, 15, 0, 15, STONE_DD)
m.box(10, 14, 10, 13, 15, 13, STONE_DD)  # chimney
for x, y, z in [(0, 13, 0), (15, 13, 0), (0, 13, 15), (15, 13, 15)]:
    m.set(x, y, z, None)  # one-voxel bevels on the top corners
for x, y, z in [(2, 10, 15), (12, 11, 15), (13, 3, 15), (1, 5, 15), (5, 12, 15), (9, 2, 15)]:
    m.set(x, y, z, STONE_DD)  # mortar
m.box(3, 2, 15, 12, 9, 15, STONE_DD)  # recess frame
m.box(4, 3, 15, 11, 8, 15, EMBER_D)
m.box(5, 4, 15, 10, 7, 15, EMBER)
m.box(6, 5, 15, 9, 6, 15, EMBER_L)
m.box(3, 2, 15, 12, 2, 15, STONE_DD)  # hearth lip
m.box(5, 1, 15, 10, 1, 15, PLANK_DD)  # logs
save(m, "furnace")

# Chest: a wooden box with a domed lid, a lid seam, two iron straps and a
# gold lock with a keyhole.
m = Model(16, 16, 16)
m.box(0, 0, 1, 15, 8, 14, PLANK)
m.box(0, 0, 1, 15, 0, 14, PLANK_D)
m.box(0, 4, 14, 15, 4, 14, PLANK_D)  # plank line
m.box(0, 9, 1, 15, 9, 14, PLANK_DD)  # lid seam
m.box(0, 10, 1, 15, 13, 14, PLANK)
m.box(0, 13, 1, 15, 13, 14, PLANK_L)
m.box(2, 14, 3, 13, 14, 12, PLANK_L)  # dome
for x in (3, 11):
    m.box(x, 0, 14, x + 1, 13, 15, IRON_G)  # straps
    m.box(x, 0, 1, x + 1, 13, 1, IRON_GD)
m.box(7, 5, 14, 8, 8, 15, GOLDEN)  # lock
m.set(8, 6, 15, PLANK_DD)  # keyhole
save(m, "chest")

# Door, closed: a 16x16 slab four voxels thick, framed, with two plank seams,
# iron hinges on the left and a gold knob.
m = Model(16, 16, 4)
m.box(0, 0, 0, 15, 15, 3, PLANK)
m.box(0, 0, 0, 0, 15, 3, PLANK_D)
m.box(15, 0, 0, 15, 15, 3, PLANK_D)
m.box(1, 0, 0, 14, 0, 3, PLANK_D)
m.box(1, 15, 0, 14, 15, 3, PLANK_L)
m.box(5, 1, 3, 5, 14, 3, PLANK_DD)  # plank seams
m.box(10, 1, 3, 10, 14, 3, PLANK_DD)
for y in (3, 12):
    m.box(0, y, 3, 1, y + 1, 3, IRON_G)  # hinges
m.box(12, 7, 3, 12, 8, 3, GOLDEN)  # knob
for x, y in [(0, 0), (15, 0), (0, 15), (15, 15)]:
    for z in range(4):
        m.set(x, y, z, None)  # one-voxel rounded corners
save(m, "door_closed")

# Door, open: the same slab swung against the left side of the cell, so its
# 4-voxel thickness runs along x and its 16-voxel length along z.
m = Model(16, 16, 16)
m.box(0, 0, 0, 3, 15, 15, PLANK)
m.box(0, 0, 0, 0, 15, 15, PLANK_D)
m.box(0, 0, 0, 3, 0, 0, PLANK_D)
m.box(0, 0, 15, 3, 15, 15, PLANK_D)
m.box(3, 1, 1, 3, 14, 14, PLANK_L)  # inner face
m.box(0, 5, 1, 0, 5, 14, PLANK_DD)  # seams on the outer face
m.box(0, 10, 1, 0, 10, 14, PLANK_DD)
for y in (3, 12):
    m.box(0, y, 0, 3, y + 1, 0, IRON_G)
m.box(2, 7, 12, 3, 8, 12, GOLDEN)  # knob
for y, z in [(0, 0), (15, 0), (0, 15), (15, 15)]:
    m.set(0, y, z, None)  # one-voxel rounded corners on the outer face
save(m, "door_open")

# Table: a broad plank top with a gap and a shaded edge, four 2x2 legs and
# an apron.
m = Model(16, 16, 16)
m.box(0, 12, 0, 15, 13, 15, PLANK)
m.box(0, 13, 0, 15, 13, 15, PLANK_L)
m.box(0, 12, 0, 15, 12, 15, PLANK_D)
m.box(0, 13, 7, 15, 13, 7, PLANK_DD)  # plank gap
for x, z in [(0, 0), (14, 0), (0, 14), (14, 14)]:
    m.box(x, 0, z, x + 1, 11, z + 1, PLANK_D)
    m.box(x, 0, z, x, 11, z, PLANK_DD)
m.box(2, 10, 14, 13, 11, 14, PLANK_D)  # front apron
m.box(2, 10, 0, 13, 11, 0, PLANK_DD)
m.box(0, 10, 2, 0, 11, 13, PLANK_DD)
m.box(15, 10, 2, 15, 11, 13, PLANK_DD)
save(m, "table")

# Chair: a tall back on the left (Terraria chairs face right), a seat, and
# four 2x2 legs.
m = Model(16, 16, 16)
m.box(2, 7, 2, 13, 8, 13, PLANK)
m.box(2, 8, 2, 13, 8, 13, PLANK_L)
m.box(2, 7, 2, 13, 7, 13, PLANK_D)
m.box(2, 9, 2, 3, 15, 13, PLANK)  # back post and rail
m.box(2, 9, 2, 2, 15, 13, PLANK_D)
m.box(2, 12, 6, 3, 13, 9, PLANK_DD)  # splat
for x, z in [(2, 2), (2, 12), (12, 2), (12, 12)]:
    m.box(x, 0, z, x + 1, 6, z + 1, PLANK_D)
save(m, "chair")

# Platform: a three-voxel-thick plank slab at the top of its cell, its boards
# separated by dark gaps across z.
m = Model(16, 16, 16)
m.box(0, 13, 0, 15, 15, 15, PLANK)
m.box(0, 15, 0, 15, 15, 15, PLANK_L)
m.box(0, 13, 0, 15, 13, 15, PLANK_D)
for z in (4, 8, 12):
    m.box(0, 13, z, 15, 15, z, PLANK_DD)
m.box(0, 13, 0, 0, 15, 15, PLANK_D)
m.box(15, 13, 0, 15, 15, 15, PLANK_D)
save(m, "platform")

# Demon altar: the six cells of one 3x2x1 group, drawn at 16 voxels per block.
# A broad corrupted plinth, two pillars with horn tips curling over a burning
# brazier bowl, candle stubs, a skull relief and cracks in the plinth. The
# plinth and horn tips touch every extreme of the group (x 0..47, y 0..31,
# z 0..15) so the client mesh spans exactly the six world cells (see
# block_models.rs).
m = Model(48, 32, 16)


def cell(x0, y0, z0, x1, y1, z1, c):
    """A box in the old 8-voxel layout, scaled to 16 voxels per block."""
    m.box(2 * x0, 2 * y0, 2 * z0, 2 * x1 + 1, 2 * y1 + 1, 2 * z1 + 1, c)


# Plinth: three courses, dark at the base, with a lighter cap course.
cell(0, 0, 0, 23, 0, 7, EBONSTONE_DD)
cell(0, 1, 0, 23, 1, 7, EBONSTONE_D)
cell(0, 2, 0, 23, 2, 7, EBONSTONE)
for x, z in [(0, 0), (23, 0), (0, 7), (23, 7), (11, 0), (12, 0)]:
    cell(x, 2, z, x, 2, z, EBONSTONE_L)
for x in range(4, 44, 6):  # cracks in the plinth's front face
    m.box(x, 2, 15, x + 1, 3, 15, CORRUPT_D)
# Side pillars: chunky shafts with darker front and back faces.
cell(0, 3, 1, 6, 12, 6, EBONSTONE)
cell(17, 3, 1, 23, 12, 6, EBONSTONE)
cell(0, 3, 6, 6, 12, 6, EBONSTONE_D)
cell(17, 3, 6, 23, 12, 6, EBONSTONE_D)
cell(0, 3, 0, 6, 12, 0, EBONSTONE_D)
cell(17, 3, 0, 23, 12, 0, EBONSTONE_D)
cell(0, 3, 1, 0, 12, 1, EBONSTONE_L)  # outer corner highlights
cell(23, 3, 1, 23, 12, 1, EBONSTONE_L)
cell(6, 3, 1, 6, 4, 1, EBONSTONE_L)  # inner base highlights
cell(17, 3, 1, 17, 4, 1, EBONSTONE_L)
# Corrupted veins crawling up the shafts.
for x, y in [(2, 6), (4, 9), (3, 11), (20, 7), (18, 10), (21, 11)]:
    cell(x, y, 6, x, y, 6, CORRUPT_D)
    m.set(2 * x + 1, 2 * y + 2, 15, CORRUPT_D)
# Capstones: one course wider than the shafts.
cell(0, 13, 0, 6, 13, 7, EBONSTONE_L)
cell(17, 13, 0, 23, 13, 7, EBONSTONE_L)
cell(0, 14, 0, 6, 14, 7, EBONSTONE)
cell(17, 14, 0, 23, 14, 7, EBONSTONE)
# Horns curling inward from the capstones, tips at the top of the group.
cell(7, 14, 3, 8, 14, 4, EBONSTONE_L)
cell(6, 15, 3, 7, 15, 4, EBONSTONE_L)
cell(16, 15, 3, 17, 15, 4, EBONSTONE_L)
cell(15, 14, 3, 16, 14, 4, EBONSTONE_L)
cell(7, 15, 3, 7, 15, 3, CORRUPT_L)
cell(16, 15, 4, 16, 15, 4, CORRUPT_L)
# The brazier bowl: pedestal, floor, rim and a glowing interior.
cell(7, 3, 2, 16, 4, 5, EBONSTONE_D)
cell(7, 5, 2, 16, 5, 5, EBONSTONE)
cell(7, 6, 2, 16, 6, 5, EBONSTONE_L)
cell(8, 5, 3, 15, 5, 4, None)  # hollow the interior
cell(9, 5, 3, 14, 5, 4, EMBER_D)
cell(10, 5, 3, 13, 5, 3, EMBER)
cell(11, 5, 3, 12, 5, 3, EMBER_L)
# Flame tongues licking above the rim.
for x in (13, 17, 21):
    m.set(x, 14, 8, EMBER_L)
    m.set(x, 15, 8, EMBER)
# Two candle stubs burning at the bowl's ends.
for cx in (8, 15):
    cell(cx, 7, 3, cx, 7, 4, EBONSTONE_L)
    cell(cx, 8, 3, cx, 8, 3, EMBER_L)
    cell(cx, 8, 4, cx, 8, 4, EMBER)
# A skull relief on the front of the left pillar: a brow, eye sockets, a jaw
# with teeth.
cell(2, 9, 6, 4, 11, 6, EBONSTONE_L)
cell(2, 10, 6, 2, 10, 6, EBONSTONE_DD)  # eye sockets
cell(4, 10, 6, 4, 10, 6, EBONSTONE_DD)
cell(3, 9, 6, 3, 9, 6, EBONSTONE_D)  # jaw line
for x in (2, 4):
    m.set(2 * x + 1, 2 * 9 + 1, 15, EBONSTONE_D)  # teeth
# Only the surface is kept (a 2-voxel shell) so the altar stays inside the
# furniture voxel budget; the interior is never seen.
m.hollow(2)
save(m, "demon_altar")

# Hellforge: dark hellstone-brick underworld forge with glowing magma hearth
# and chimney.
m = Model(16, 16, 16)
m.box(0, 0, 0, 15, 13, 15, HELL_BRICK_D)
m.box(0, 0, 15, 15, 13, 15, HELL_BRICK)
m.box(0, 0, 15, 0, 13, 15, OBS_BRICK_D)
m.box(15, 0, 15, 15, 13, 15, OBS_BRICK_D)
m.box(0, 13, 0, 15, 13, 15, HELL_BRICK_L)
m.box(0, 0, 0, 15, 0, 15, OBS_BRICK_D)
m.box(10, 14, 10, 13, 15, 13, OBS_BRICK)  # chimney
for x, y, z in [(0, 13, 0), (15, 13, 0), (0, 13, 15), (15, 13, 15)]:
    m.set(x, y, z, None)
m.box(3, 2, 15, 12, 9, 15, OBS_BRICK_D)  # magma recess frame
m.box(4, 3, 15, 11, 8, 15, EMBER_D)
m.box(5, 4, 15, 10, 7, 15, EMBER)
m.box(6, 5, 15, 9, 6, 15, EMBER_L)
m.box(3, 1, 15, 12, 2, 15, OBS_BRICK)
save(m, "hellforge")
