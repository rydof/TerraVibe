"""Eye of Cthulhu rig at 16 voxels per block: ball, iris (phase 1), mouth
(phase 2) and four tendrils.

The ball is a 4-block sphere (64 voxels across) hollowed to a 2-voxel shell;
the iris and the mouth are separate discs on its front so the client can swap
them by phase.
"""
import math
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.dirname(HERE))

from _player_base import *
from _rigtools import key, tapered, write_rig

OUT = os.path.join(ASSETS, "models", "eye_of_cthulhu")
R = 32  # voxel radius: a 4 block sphere
C = R - 0.5
SCLERA = hexc("#f0dcd2")
SCLERA_D = hexc("#d8bcb4")
VEIN = hexc("#c4333a")
IRIS = hexc("#b81f24")
IRIS_L = hexc("#e0402f")
IRIS_D = hexc("#8a1218")
PUPIL = hexc("#0a0a0a")
LIP = hexc("#b84a4a")
FLESH = hexc("#7a1f2a")
FLESH_D = hexc("#4a0d12")
THROAT = hexc("#1a0508")
TOOTH = hexc("#f2ecd8")
TOOTH_D = hexc("#cbc3a8")
TEND = hexc("#c43a3a")
TEND_L = hexc("#e05a52")
TEND_D = hexc("#8a1f26")

STEPS = ((1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1))


def surface(m):
    """Voxels with at least one empty 6-neighbour."""
    return {
        p
        for p in m.v
        if any((p[0] + d[0], p[1] + d[1], p[2] + d[2]) not in m.v for d in STEPS)
    }


def cheb(a, b):
    return max(abs(a[0] - b[0]), abs(a[1] - b[1]), abs(a[2] - b[2]))


def vein_paths(m, rng, count, steps, skip, prefer=None):
    """Paint `count` separate 1-voxel-wide branching surface veins.

    Each path is a random walk over surface voxels with optional short side
    twigs. Separate paths never come within Chebyshev distance 1 of each other,
    so a 26-connected count finds one component per vein, and no 2x2x2 block
    is formed.
    """
    surf = sorted(surface(m))
    preferred = [p for p in surf if prefer(p) and not skip(p)] if prefer else []
    taken = set()
    paths = []
    attempts = 0

    def has_2x2x2(p, group):
        for ox in (-1, 0):
            for oy in (-1, 0):
                for oz in (-1, 0):
                    if all(
                        (p[0] + ox + dx, p[1] + oy + dy, p[2] + oz + dz) in group
                        for dx in (0, 1)
                        for dy in (0, 1)
                        for dz in (0, 1)
                    ):
                        return True
        return False

    while len(paths) < count and attempts < 120_000:
        attempts += 1
        pool = preferred if (preferred and len(paths) < count * 2 // 3) else surf
        p = rng.choice(pool)
        if skip(p) or any(cheb(p, q) <= 1 for q in taken):
            continue
        path = [p]
        path_set = {p}
        for _ in range(steps - 1):
            cand = [
                q
                for q in (
                    (path[-1][0] + d[0], path[-1][1] + d[1], path[-1][2] + d[2])
                    for d in STEPS
                )
                if q in m.v
                and q not in path_set
                and not skip(q)
                and all(cheb(q, r) > 1 for r in taken)
                and all(cheb(q, r) > 1 for r in path[:-1])
                and not has_2x2x2(q, path_set | {q})
            ]
            if not cand:
                break
            nxt = rng.choice(cand)
            path.append(nxt)
            path_set.add(nxt)
        if len(path) < 5:
            continue
        # Add 1-2 short branching twigs from mid-path nodes.
        for branch_anchor in path[2:-2:4]:
            twig_cur = branch_anchor
            for _ in range(3):
                tcand = [
                    q
                    for q in (
                        (twig_cur[0] + d[0], twig_cur[1] + d[1], twig_cur[2] + d[2])
                        for d in STEPS
                    )
                    if q in m.v
                    and q not in path_set
                    and not skip(q)
                    and all(cheb(q, r) > 1 for r in taken)
                    and not has_2x2x2(q, path_set | {q})
                ]
                if not tcand:
                    break
                twig_cur = rng.choice(tcand)
                path.append(twig_cur)
                path_set.add(twig_cur)
        for q in path:
            taken.add(q)
            m.v[q] = VEIN
        paths.append(path)
    assert len(paths) >= count, f"only {len(paths)} of {count} vein paths"
    return paths


rng = random.Random(11)
ball = Model(2 * R, 2 * R, 2 * R)
ball.sphere(C, C, C, R, SCLERA)
# Two sclera shades: the lower and back hemisphere sits in shadow.
for (x, y, z) in list(ball.v):
    if y < 26 or z < 20:
        ball.v[(x, y, z)] = SCLERA_D


def iris_zone(p):
    """No veins under the iris cap on the front of the ball."""
    return p[2] >= 30 and math.hypot(p[0] - C, p[1] - C) < 21


def front_ring(p):
    """Prefer veins framing the front iris/mouth circumference."""
    r = math.hypot(p[0] - C, p[1] - C)
    return p[2] >= 18 and 19 <= r <= 32


vein_paths(ball, rng, 28, 16, iris_zone, prefer=front_ring)
ball.hollow(2)

# Iris: three concentric rings around a 12-voxel pupil with radial fiber texture.
IRIS_R = 20.5
iris = Model(42, 42, 4)
for x in range(42):
    for y in range(42):
        d = math.hypot(x - IRIS_R, y - IRIS_R)
        if d > IRIS_R:
            continue
        ang = math.atan2(y - IRIS_R, x - IRIS_R)
        ripple = 0.65 * math.sin(ang * 14) + 0.35 * math.cos(ang * 22)
        spoke = ((round(ang * 12 / math.pi) + int(d)) % 3 == 0)
        if d <= 6:
            # Keep exact 12x12 pupil extent; recess pupil slightly at z=0.
            iris.box(x, y, 1, x, y, 3, PUPIL)
            continue
        elif d <= 11.2 + ripple:
            col = IRIS_L if not spoke or d <= 8.5 else IRIS
        elif d <= 16.2 + ripple:
            col = IRIS_L if (spoke and d <= 13.5) else (IRIS_D if (not spoke and d >= 14.8) else IRIS)
        else:
            col = IRIS if (spoke and d <= 18.0) else IRIS_D
        iris.box(x, y, 0, x, y, 3, col)

# Mouth: ragged fleshy lip ring around darker flesh, an open throat and 12 curved fangs.
MOUTH_C = 27.5
mouth = Model(56, 56, 12)
for x in range(56):
    for y in range(56):
        d = math.hypot(x - MOUTH_C, y - MOUTH_C)
        ang = math.atan2(y - MOUTH_C, x - MOUTH_C)
        lip_r = 26.2 + 0.8 * math.sin(ang * 12) + 0.4 * math.cos(ang * 18)
        if d > lip_r:
            continue
        if d > 22.4 + 0.5 * math.sin(ang * 12):
            z0 = 1 if (int(round(ang * 8)) % 2 == 0 and d > 25.0) else 0
            mouth.box(x, y, z0, x, y, 7, LIP)
        elif d < 10.5:
            # Open throat: the front layers (z=0..4) stay empty, the back is dark.
            mouth.box(x, y, 5, x, y, 9, THROAT)
        else:
            # Front face and deeper wall: flesh around the open centre.
            col = FLESH if d >= 15.0 + 0.6 * math.sin(ang * 10) else FLESH_D
            mouth.box(x, y, 1, x, y, 9, col)
for k in range(12):
    a_base = k * math.tau / 12
    # Each fang curves inward from the gum rim (r=16) to a sharp tip (r=9.5) jutting forward (z=0..4).
    for i in range(26):
        t = i / 25.0
        r = 16.0 - 6.5 * t
        a = a_base + 0.08 * math.sin(t * math.pi)
        x = round(MOUTH_C + r * math.cos(a))
        y = round(MOUTH_C + r * math.sin(a))
        z0 = 0 if t >= 0.45 else 1
        z1 = 4 if t <= 0.35 else (3 if t <= 0.65 else 2)
        col = TOOTH if (t >= 0.4 or k % 2 == 0) else TOOTH_D
        mouth.box(x, y, z0, x, y, z1, col)
        # Widen the gum base slightly while keeping teeth well separated.
        if t <= 0.3:
            nx = round(MOUTH_C + r * math.cos(a + 0.05))
            ny = round(MOUTH_C + r * math.sin(a + 0.05))
            mouth.box(nx, ny, 2, nx, ny, 4, TOOTH_D)

# Optic-nerve tendrils trailing from the back, at 16 voxels per block.
TENDRILS = {"tendril1": (1, 1), "tendril2": (-1, 1), "tendril3": (1, -1), "tendril4": (-1, -1)}
SIZES = [8, 8, 7, 7, 7, 6, 6, 6, 5, 5, 5, 4, 4, 4, 3, 3, 3, 3]
tend = tapered(SIZES, [TEND, TEND_L, TEND_D], 8, 36)
# A 1-voxel highlight ridge down one corner, so the rod is not a 2x dilation.
for z in range(36):
    ring = [(x, y) for (x, y, zz) in tend.v if zz == z]
    if ring:
        x, y = min(ring)
        tend.set(x, y, z, TEND_L)

model_parts = {"ball": ball, "iris": iris, "mouth": mouth}
for n in TENDRILS:
    model_parts[n] = tend
for name, m in model_parts.items():
    m.save(os.path.join(OUT, name + ".vox"))

parts = [
    ("ball", (32, 32, 32), (0, 55, 0), None),
    ("iris", (20.5, 20.5, 0), (0, 0, 31.0), "ball"),
    ("mouth", (27.5, 27.5, 0), (0, 0, 30.0), "ball"),
]
for n, (sx, sy) in TENDRILS.items():
    parts.append((n, (4, 4, 36), (sx * 12, sy * 12, -20), "ball"))


def sway(n, phase):
    sx, sy = TENDRILS[n]
    ks = []
    for t, w in ((0.0, 1), (0.8, -1), (1.6, 1)):
        w *= phase
        ks.append(key(t, n, (sy * 22 + 10 * w, -sx * 22 + 14 * w, 0)))
    return ks


idle = []
for i, n in enumerate(TENDRILS):
    idle += sway(n, 1 if i % 2 == 0 else -1)
write_rig(OUT, parts, {"idle": (1.6, True, idle)})
