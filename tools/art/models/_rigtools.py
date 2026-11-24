"""Helpers shared by the rig-writing model scripts: ron emitters."""
import math
import os

from vox import Model
from palette import ASSETS, VPU, hexc

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
    """parts: (name, pivot, offset, parent|None) or (name, file, pivot, offset, parent|None);
    anims: name -> (len, looping, keys).
    Pivots and offsets are in voxels at VPU voxels per world unit."""
    os.makedirs(out, exist_ok=True)
    ron = "(\n    parts: [\n"
    for entry in parts:
        if len(entry) == 5:
            name, file, pivot, offset, parent = entry
        else:
            name, pivot, offset, parent = entry
            file = f"{name}.vox"
        par = f'Some("{parent}")' if parent else "None"
        ron += (
            f'        (name: "{name}", file: "{file}", pivot: {vec(pivot)}, '
            f"offset: {vec(offset)}, parent: {par}),\n"
        )
    ron += "    ],\n    anims: {\n"
    for name, (length, looping, keys) in anims.items():
        body = ",\n            ".join(keys)
        ron += f'        "{name}": (len: {length}, looping: {str(looping).lower()}, keys: [\n            {body},\n        ]),\n'
    ron += "    },\n)\n"
    with open(os.path.join(out, "rig.ron"), "w") as f:
        f.write(ron)


def out_dir(kind):
    return os.path.join(ASSETS, "models", kind)


def save_parts(kind, parts):
    out = out_dir(kind)
    os.makedirs(out, exist_ok=True)
    for name, model in parts.items():
        model.save(os.path.join(out, f"{name}.vox"))


def recolor(m, mp):
    out = Model(*m.size)
    for (x, y, z), col in m.v.items():
        out.v[(x, y, z)] = mp.get(col, col)
    return out


def mirror_x(m):
    sx, sy, sz = m.size
    out = Model(sx, sy, sz)
    for (x, y, z), col in m.v.items():
        out.v[(sx - 1 - x, y, z)] = col
    return out


def humanoid_anims(arm_base=0.0, arm_swing=35.0, leg_swing=35.0, walk_len=0.6, idle_bob=0.8, walk_bob=1.0):
    half = walk_len / 2.0
    return {
        "idle": (
            2.0,
            True,
            [
                key(0.0, "torso", off=(0, 0, 0)),
                key(1.0, "torso", off=(0, idle_bob, 0)),
                key(2.0, "torso", off=(0, 0, 0)),
                key(0.0, "arm_l", rot=(arm_base, 0, 0)),
                key(0.0, "arm_r", rot=(arm_base, 0, 0)),
            ],
        ),
        "walk": (
            walk_len,
            True,
            [
                key(0.0, "torso", off=(0, 0, 0)),
                key(half / 2.0, "torso", off=(0, walk_bob, 0)),
                key(half, "torso", off=(0, 0, 0)),
                key(half + half / 2.0, "torso", off=(0, walk_bob, 0)),
                key(walk_len, "torso", off=(0, 0, 0)),
                key(0.0, "arm_l", rot=(arm_base + arm_swing, 0, 0)),
                key(half, "arm_l", rot=(arm_base - arm_swing, 0, 0)),
                key(walk_len, "arm_l", rot=(arm_base + arm_swing, 0, 0)),
                key(0.0, "arm_r", rot=(arm_base - arm_swing, 0, 0)),
                key(half, "arm_r", rot=(arm_base + arm_swing, 0, 0)),
                key(walk_len, "arm_r", rot=(arm_base - arm_swing, 0, 0)),
                key(0.0, "leg_l", rot=(-leg_swing, 0, 0)),
                key(half, "leg_l", rot=(leg_swing, 0, 0)),
                key(walk_len, "leg_l", rot=(-leg_swing, 0, 0)),
                key(0.0, "leg_r", rot=(leg_swing, 0, 0)),
                key(half, "leg_r", rot=(-leg_swing, 0, 0)),
                key(walk_len, "leg_r", rot=(leg_swing, 0, 0)),
            ],
        ),
    }


def slime_anims(body_part="body", squash=(1.22, 0.74, 1.22), stretch=(0.88, 1.22, 0.88)):
    return {
        "idle": (
            1.4,
            True,
            [
                key(0.0, body_part, scale=(1, 1, 1)),
                key(0.7, body_part, scale=(1.06, 0.92, 1.06)),
                key(1.4, body_part, scale=(1, 1, 1)),
            ],
        ),
        "hop": (
            0.5,
            False,
            [
                key(0.0, body_part, scale=squash),
                key(0.2, body_part, scale=stretch),
                key(0.5, body_part, scale=(1, 1, 1)),
            ],
        ),
    }


def wing_flap_anims(body_part="body", wing_l="wing_l", wing_r="wing_r", period=0.4, angle=35.0, bob=1.0):
    half = period / 2.0
    keys = [
        key(0.0, wing_l, rot=(0, 0, angle)),
        key(half, wing_l, rot=(0, 0, -angle)),
        key(period, wing_l, rot=(0, 0, angle)),
        key(0.0, wing_r, rot=(0, 0, -angle)),
        key(half, wing_r, rot=(0, 0, angle)),
        key(period, wing_r, rot=(0, 0, -angle)),
    ]
    if body_part is not None:
        keys.extend([
            key(0.0, body_part, off=(0, 0, 0)),
            key(half, body_part, off=(0, bob, 0)),
            key(period, body_part, off=(0, 0, 0)),
        ])
    return {"idle": (period, True, keys)}


def jaw_chomp_anims(jaw_part="jaw", period=0.6, drop=-2.5, rot_x=18.0, bob_part=None):
    half = period / 2.0
    keys = [
        key(0.0, jaw_part, rot=(0, 0, 0), off=(0, 0, 0)),
        key(half, jaw_part, rot=(rot_x, 0, 0), off=(0, drop, 0)),
        key(period, jaw_part, rot=(0, 0, 0), off=(0, 0, 0)),
    ]
    if bob_part is not None:
        keys.extend([
            key(0.0, bob_part, off=(0, 0, 0)),
            key(half, bob_part, off=(0, 1.0, 0)),
            key(period, bob_part, off=(0, 0, 0)),
        ])
    return {"idle": (period, True, keys)}


def swim_anims(tail_part="tail", period=0.5, yaw=28.0, bob_part="body"):
    half = period / 2.0
    keys = [
        key(0.0, tail_part, rot=(0, yaw, 0)),
        key(half, tail_part, rot=(0, -yaw, 0)),
        key(period, tail_part, rot=(0, yaw, 0)),
    ]
    if bob_part is not None:
        keys.extend([
            key(0.0, bob_part, off=(0, 0, 0)),
            key(half, bob_part, off=(0, 0.8, 0)),
            key(period, bob_part, off=(0, 0, 0)),
        ])
    return {"idle": (period, True, keys)}


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
