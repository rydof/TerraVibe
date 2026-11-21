"""Forest canopies: three leafy clumps, about 4x3x4 blocks (64x48x64 voxels).

Each variant is 8-12 overlapping ellipsoids (radius 10-18 voxels) shaded with
exactly three leaf colours: the sunlit tops light, the underside dark. Small
notches are carved out of the outline so the silhouette reads as leaf clumps.
A 6x6-voxel bark stub at the bottom centre (x/z 29..=34) joins the trunk top.
"""
import os
import random

from palette import ASSETS, hexc
from vox import Model

# Exactly three leaf shades: dark underside, mid body, light crown.
LEAF_D = hexc("#1f6b2a")
LEAF = hexc("#2f9a3a")
LEAF_L = hexc("#4fc24d")
BARK = hexc("#6d4c2c")

# (cx, cy, cz, rx, ry, rz) blobs per variant. Radii stay within 10..18 voxels;
# every variant fills the 64-wide/64-deep/48-tall box with its own lobe layout.
VARIANTS = [
    [
        (32, 25, 32, 18, 15, 17),
        (14, 21, 28, 14, 12, 14),
        (50, 23, 36, 14, 12, 14),
        (31, 20, 50, 13, 11, 13),
        (33, 21, 14, 14, 12, 13),
        (32, 37, 32, 12, 10, 12),
        (19, 31, 20, 11, 10, 11),
        (45, 32, 42, 11, 10, 11),
        (26, 14, 42, 12, 10, 11),
        (40, 13, 22, 12, 10, 11),
    ],
    [
        (32, 24, 32, 17, 17, 16),
        (16, 19, 24, 13, 12, 13),
        (48, 22, 40, 13, 12, 13),
        (30, 18, 49, 13, 11, 12),
        (36, 20, 16, 14, 12, 13),
        (33, 37, 30, 13, 11, 12),
        (20, 33, 38, 11, 10, 11),
        (44, 34, 22, 11, 10, 11),
        (25, 12, 36, 12, 10, 11),
        (41, 13, 25, 12, 10, 11),
    ],
    [
        (32, 22, 32, 18, 14, 18),
        (13, 20, 31, 13, 12, 14),
        (51, 20, 33, 13, 12, 14),
        (32, 18, 51, 13, 11, 12),
        (32, 19, 13, 13, 11, 12),
        (31, 34, 32, 12, 10, 12),
        (18, 30, 19, 11, 10, 11),
        (46, 31, 45, 11, 10, 11),
        (24, 12, 43, 12, 10, 11),
        (42, 12, 20, 12, 10, 11),
    ],
]

NEIGHBOURS = [
    (dx, dy, dz) for dx in (-1, 0, 1) for dy in (-1, 0, 1) for dz in (-1, 0, 1) if (dx, dy, dz) != (0, 0, 0)
]


def notch(m, rng, count):
    """Carve 1-2 voxel dents out of the exposed surface (never the trunk stub)."""
    for _ in range(count):
        surface = [
            (x, y, z)
            for (x, y, z) in m.v
            if y >= 6
            and any((x + dx, y + dy, z + dz) not in m.v for dx, dy, dz in NEIGHBOURS)
        ]
        if not surface:
            return
        x, y, z = rng.choice(surface)
        m.set(x, y, z, None)
        if rng.random() < 0.5:
            deep = [(x + dx, y + dy, z + dz) for dx, dy, dz in NEIGHBOURS if (x + dx, y + dy, z + dz) in m.v]
            if deep:
                m.set(*rng.choice(deep), None)


for i, blobs in enumerate(VARIANTS):
    rng = random.Random(100 + i)
    m = Model(64, 48, 64)
    for b in blobs:
        m.ellipsoid(*b, LEAF)
    notch(m, rng, 26)
    # Shade all leaf voxels: the dark underside wins whenever there is empty
    # space below (even on thin, upward-exposed clumps); light only on supported
    # voxels open to the sky; mid in the body. The bark stub is stamped after
    # this, so it overwrites its footprint.
    for (x, y, z) in list(m.v):
        if (x, y - 1, z) not in m.v:
            m.set(x, y, z, LEAF_D)
        elif (x, y + 1, z) not in m.v:
            m.set(x, y, z, LEAF_L)
        else:
            m.set(x, y, z, LEAF)
    # Bark stub: 6x6 voxels at the base, centred on x = 32, reaching up into the
    # clump so the canopy is one connected mesh on the trunk top.
    m.box(29, 0, 29, 34, 9, 34, BARK)
    m.save(os.path.join(ASSETS, "models", "blocks", f"canopy_{i}.vox"))
