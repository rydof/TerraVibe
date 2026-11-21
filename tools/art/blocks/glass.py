"""Glass: light frame, two diagonal highlights, transparent middle."""
from palette import *
from tex import emit

emit(
    "glass",
    [
        "ffffffffffffffff",
        "f..............f",
        "f.gg...........f",
        "f..gg..........f",
        "f...gg.........f",
        "f....g.........f",
        "f.....g........f",
        "f..............f",
        "f..............f",
        "f........g.....f",
        "f.........g....f",
        "f..........g...f",
        "f..............f",
        "f..............f",
        "f..............f",
        "ffffffffffffffff",
    ],
    {"f": GLASS_F, "g": GLASS_G, ".": (*GLASS_G[:3], 0)},
)
