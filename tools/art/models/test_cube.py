"""Smoke model: a 4x4x4 two-tone cube split into two rig parts' worth of data."""
import os

from palette import ASSETS
from vox import Model

RED = (200, 60, 50, 255)
BLUE = (60, 90, 200, 255)

m = Model(4, 4, 4)
m.box(0, 0, 0, 3, 1, 3, RED)
m.box(0, 2, 0, 3, 3, 3, BLUE)
out = os.path.join(ASSETS, "models", "test")
m.save(os.path.join(out, "cube.vox"))
with open(os.path.join(out, "rig.ron"), "w") as f:
    f.write("""(
    parts: [
        (name: "cube", file: "cube.vox", pivot: (2.0, 0.0, 2.0), offset: (0.0, 0.0, 0.0), parent: None),
    ],
    anims: {
        "spin": (len: 1.0, looping: true, keys: [
            (t: 0.0, part: "cube", rot_deg: (0.0, 0.0, 0.0), offset: (0.0, 0.0, 0.0)),
            (t: 0.5, part: "cube", rot_deg: (0.0, 180.0, 0.0), offset: (0.0, 1.0, 0.0)),
        ]),
    },
)
""")
