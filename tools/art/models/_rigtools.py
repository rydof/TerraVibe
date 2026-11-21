"""Helpers shared by the rig-writing model scripts: ron emitters."""
import math
import os

from vox import Model
from palette import VPU, hexc

VEIN = hexc("#c4333a")


def vec(v):
    return "(" + ", ".join(f"{float(x):.3f}" for x in v) + ")"


def key(t, part, rot=(0, 0, 0), off=(0, 0, 0), scale=None):
    s = f", scale: {vec(scale)}" if scale else ""
    return f'(t: {t}, part: "{part}", rot_deg: {vec(rot)}, offset: {vec(off)}{s})'


def px(n_at_8):
    """An offset authored at the old 8 voxels/unit, in voxels at VPU. Only for shared
    values (pivots, offsets); shapes are redrawn at the finer size, never scaled."""
    return n_at_8 * VPU // 8


def write_rig(out, parts, anims):
    """parts: (name, pivot, offset, parent|None); anims: name -> (len, looping, keys).
    Pivots and offsets are in voxels at VPU voxels per world unit."""
    ron = "(\n    parts: [\n"
    for name, pivot, offset, parent in parts:
        par = f'Some("{parent}")' if parent else "None"
        ron += (
            f'        (name: "{name}", file: "{name}.vox", pivot: {vec(pivot)}, '
            f"offset: {vec(offset)}, parent: {par}),\n"
        )
    ron += "    ],\n    anims: {\n"
    for name, (length, looping, keys) in anims.items():
        body = ",\n            ".join(keys)
        ron += f'        "{name}": (len: {length}, looping: {str(looping).lower()}, keys: [\n            {body},\n        ]),\n'
    ron += "    },\n)\n"
    with open(os.path.join(out, "rig.ron"), "w") as f:
        f.write(ron)


def surface(m, c):
    return [
        p for p in m.v
        if any(m.get(p[0] + d[0], p[1] + d[1], p[2] + d[2]) is None
               for d in ((1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)))
    ]


def veins(m, rng, count, steps, skip, col=VEIN):
    surf = set(surface(m, None))
    pts = sorted(surf)
    for _ in range(count):
        p = rng.choice(pts)
        for _ in range(steps):
            if not skip(p):
                m.v[p] = col
            nxt = [
                (p[0] + d[0], p[1] + d[1], p[2] + d[2])
                for d in ((1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1))
            ]
            nxt = [q for q in nxt if q in surf]
            if not nxt:
                break
            p = rng.choice(nxt)


def tapered(sizes, cols, w, d):
    """A rod along -z (front end at z = d): sizes[i] is the cross-section of the
    i-th run of 2 voxels counted from the front."""
    m = Model(w, w, d)
    c0 = (w - 1) / 2
    for i, s in enumerate(sizes):
        z1 = d - 1 - 2 * i
        lo = math.ceil(c0 - s / 2 + 0.5)
        hi = lo + s - 1
        m.box(lo, lo, max(z1 - 1, 0), hi, hi, z1, cols[i % len(cols)])
    return m
