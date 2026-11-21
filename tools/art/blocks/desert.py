"""sand, sandstone, hardened sand."""
from palette import *
from tex import emit

# Sand: fine grains in small clumps with sparse bright specks.
SAND_LEG = {"s": SAND, "d": SAND_D, "l": SAND_L}
SAND0 = [
    "ssssssssddssssss",
    "sssssssldsssssss",
    "ssssssssssssssss",
    "sssddssssssslsss",
    "ddssdssssssssssd",
    "dsssssssslssssdd",
    "slssssssssssssss",
    "sssssssssssddsss",
    "sssssssssssdssss",
    "ssdsssssssssssss",
    "ssslssssssssssls",
    "ssssssssssssdsss",
    "ssssssddssssssss",
    "sssssssdssssssss",
    "ssssslsssdsssdds",
    "ssssssssddssssss",
]
emit("sand", SAND0, SAND_LEG)

# Sandstone: broken horizontal strata every 5 rows, lighter chips between.
SANDSTONE_LEG = {"s": SANDSTONE, "d": SANDSTONE_D, "l": SANDSTONE_L}
SANDSTONE0 = [
    "sssslsssssssdsss",
    "ssssssdsssssssss",
    "ssssssssssssdsss",
    "slssslssslssslss",
    "dddsddsdddsddsdd",
    "ssssssssssssssss",
    "dsssssssssdsssdd",
    "ddsssdsssssssssd",
    "sslssslssslsssls",
    "dsddsddsdsddsddd",
    "ssssssssssssssss",
    "sssdssssssssssss",
    "ssssssssssssssss",
    "ssslssslssslssls",
    "sddsdddsddsdsdds",
    "sssslsssssssdsss",
]
emit("sandstone", SANDSTONE0, SANDSTONE_LEG)

# Hardened sand: compact grains split by short angular cracks.
HSAND_LEG = {"h": HSAND, "d": HSAND_D, "l": HSAND_L}
HSAND0 = [
    "hhhhhddhhhhhhhhh",
    "hlhhhdhhhhhhhhhh",
    "hhhhhhhhlhhdhhhh",
    "hhddhhhhhhhhdhhh",
    "hhhdhhhhhhhhdhlh",
    "hhhhdhhhhhhhhhhh",
    "hhhhhlhhhhlhhhhh",
    "ddhhhhhhhhhhhhhd",
    "dhhhhhhhhhhhlhdd",
    "hhhlhhdhhhhhhhhh",
    "hhhhhhhddhhhhhhh",
    "hhhhhhhhhhhhhdhh",
    "hhhhhhhhhhhhhhdh",
    "hhddhhhhhhhhhhdh",
    "hhhhhhlhhhhhhhhh",
    "hhhhhddhhhhhhhhh",
]
emit("hardened_sand", HSAND0, HSAND_LEG)
