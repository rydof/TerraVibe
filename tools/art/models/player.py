"""Player rig: part models plus rig.ron with idle, walk, jump and swing."""
import os

from _player_base import *

OUT = os.path.join(ASSETS, "models", "player")

for name, m in parts().items():
    m.save(os.path.join(OUT, name + ".vox"))


def vec(v):
    return "(" + ", ".join(f"{float(x):.3f}" for x in v) + ")"


def key(t, part, rot=(0, 0, 0), off=(0, 0, 0)):
    return f'(t: {t}, part: "{part}", rot_deg: {vec(rot)}, offset: {vec(off)})'


BOBBED = ("torso", "head", "arm_l", "arm_r")


def bob(t, dy):
    return [key(t, p, off=(0, dy, 0)) for p in BOBBED]


idle = bob(0.0, 0.0) + bob(1.0, 0.8)  # breathing
walk = []
for t, s, dy in ((0.0, 1, 0.0), (0.15, 0, 1.0), (0.3, -1, 0.0), (0.45, 0, 1.0)):
    walk += bob(t, dy)
    walk += [
        key(t, "arm_l", (35 * s, 0, 0), (0, dy, 0)),
        key(t, "arm_r", (-35 * s, 0, 0), (0, dy, 0)),
        key(t, "leg_l", (-35 * s, 0, 0)),
        key(t, "leg_r", (35 * s, 0, 0)),
    ]
# Bobbed arm keys above were overridden by the swinging ones; drop duplicates.
seen = {}
for k in walk:
    seen[(k.split(",")[0], k.split('part: "')[1].split('"')[0])] = k
walk = list(seen.values())
jump = [
    key(0.0, "arm_l", (0, 0, 0)),
    key(0.0, "arm_r", (0, 0, 0)),
    key(0.0, "leg_l"),
    key(0.0, "leg_r"),
    key(0.2, "arm_l", (-150, 0, 0)),
    key(0.2, "arm_r", (-150, 0, 0)),
    key(0.2, "leg_l", (-25, 0, 0)),
    key(0.2, "leg_r", (30, 0, 0)),
]
swing = [key(0.0, "arm_r", (-150, 0, 0)), key(0.3, "arm_r", (35, 0, 0))]


def anim(name, length, looping, keys):
    body = ",\n            ".join(keys)
    return f'        "{name}": (len: {length}, looping: {str(looping).lower()}, keys: [\n            {body},\n        ]),\n'


ron = "(\n    parts: [\n"
for name in ("torso", "head", "arm_l", "arm_r", "leg_l", "leg_r"):
    ron += (
        f'        (name: "{name}", file: "{name}.vox", pivot: {vec(PIVOTS[name])}, '
        f"offset: {vec(OFFSETS[name])}, parent: None),\n"
    )
ron += "    ],\n    anims: {\n"
ron += anim("idle", 2.0, True, idle)
ron += anim("walk", 0.6, True, walk)
ron += anim("jump", 0.4, False, jump)
ron += anim("swing", 0.3, False, swing)
ron += "    },\n)\n"
with open(os.path.join(OUT, "rig.ron"), "w") as f:
    f.write(ron)
