"""Run: python3 -I tools/art/test_check_art.py"""
import os
import random
import sys
import struct
import tempfile
import unittest
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(HERE, "models"))

import _player_base as base  # noqa: E402
import check_art  # noqa: E402
import palette  # noqa: E402
import png  # noqa: E402
import tex  # noqa: E402
from vox import Model  # noqa: E402

PAL = [palette.DIRT, palette.DIRT_D, palette.DIRT_L, palette.STONE, palette.GRASS]
CLEAR = (0, 0, 0, 0)


def noisy(size=palette.TILE, seed=1):
    rng = random.Random(seed)
    return [[rng.choice(PAL) for _ in range(size)] for _ in range(size)]


class Base(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.dir = self._tmp.name

    def tex(self, pixels, name="t.png"):
        p = os.path.join(self.dir, name)
        png.save_pixels(p, pixels)
        return p

    def model(self, m, name="m.vox"):
        p = os.path.join(self.dir, name)
        m.save(p)
        return p


class Textures(Base):
    def test_uniform_2x2_texture_is_flagged(self):
        small = noisy(palette.TILE // 2)
        up = [[small[y // 2][x // 2] for x in range(palette.TILE)] for y in range(palette.TILE)]
        errs = check_art.check_texture(self.tex(up))
        self.assertTrue(any("upscale_ratio" in e for e in errs), errs)

    def test_detailed_texture_passes(self):
        px = noisy()
        px[0][0] = CLEAR
        self.assertEqual(check_art.check_texture(self.tex(px)), [])

    def test_wrong_size_texture_is_flagged(self):
        errs = check_art.check_texture(self.tex(noisy(8)))
        self.assertTrue(any("size" in e for e in errs), errs)

    def test_off_palette_colour_is_flagged(self):
        px = noisy()
        px[3][3] = (1, 2, 3, 255)
        errs = check_art.check_texture(self.tex(px))
        self.assertTrue(any("palette" in e for e in errs), errs)

    def test_partial_alpha_is_flagged(self):
        px = noisy()
        px[3][3] = (*palette.DIRT[:3], 128)
        self.assertTrue(check_art.check_texture(self.tex(px)))


def write_rgb_png(path, px):
    """A valid 8-bit RGB (colour type 2) PNG: the runtime rejects it, load_pixels hides it."""

    def chunk(tag, data):
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data))

    h, w = len(px), len(px[0])
    raw = b"".join(b"\x00" + bytes(v for p in row for v in p[:3]) for row in px)
    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)))
        f.write(chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))


class NonRgba(Base):
    def test_non_rgba_png_is_flagged(self):
        p = os.path.join(self.dir, "rgb.png")
        write_rgb_png(p, noisy())
        errs = check_art.check_texture(p)
        self.assertTrue(any("RGBA" in e for e in errs), errs)


class Emit(Base):
    """emit() must not let an overlay legend silently recolour base cells."""

    def emit_to(self, name, rows, legend, base_legend=None):
        old = tex.ASSETS
        tex.ASSETS = self.dir
        try:
            return tex.emit(name, rows, legend, base_legend=base_legend)
        finally:
            tex.ASSETS = old

    def test_colliding_overlay_legend_raises_before_writing(self):
        rows = ["b" * palette.TILE for _ in range(palette.TILE)]
        base = {"b": palette.STONE, "d": palette.STONE_D}
        with self.assertRaises(ValueError):
            self.emit_to("clash", rows, {"d": palette.COPPER_D}, base_legend=base)
        self.assertEqual(os.listdir(self.dir), [])

    def test_disjoint_overlay_legend_is_merged_with_the_base(self):
        rows = ["b" * palette.TILE for _ in range(palette.TILE)]
        rows[0] = "o" + rows[0][1:]
        self.emit_to("ok", rows, {"o": palette.COPPER}, base_legend={"b": palette.STONE})
        px = png.load_pixels(os.path.join(self.dir, "blocks", "ok.png"))
        self.assertEqual(px[0][0], palette.COPPER)
        self.assertEqual(px[0][1], palette.STONE)


def blob(c=palette.DIRT):
    m = Model(20, 20, 20)
    m.sphere(9, 9, 9, 6, c)
    return m


class Models(Base):
    def test_clean_model_passes(self):
        self.assertEqual(check_art.check_model(self.model(blob()), 6000), [])

    def test_dilated_model_is_flagged(self):
        small = Model(10, 10, 10)
        small.sphere(4, 4, 4, 3, palette.DIRT)
        errs = check_art.check_model(self.model(small.dilated2()), 6000)
        self.assertTrue(any("dilation" in e for e in errs), errs)

    def test_over_budget_model_is_flagged(self):
        errs = check_art.check_model(self.model(blob()), 100)
        self.assertTrue(any("budget" in e for e in errs), errs)

    def test_floating_island_is_flagged(self):
        m = blob()
        m.box(17, 17, 17, 19, 19, 19, palette.STONE)  # 27 voxels, detached
        errs = check_art.check_model(self.model(m), 6000)
        self.assertTrue(any("floating" in e for e in errs), errs)
        self.assertEqual(check_art.check_model(self.model(m), 6000, detached=27), [])

    def test_empty_model_is_flagged(self):
        errs = check_art.check_model(self.model(Model(8, 8, 8)), 6000)
        self.assertTrue(any("empty" in e for e in errs), errs)

    def test_out_of_bounds_only_model_is_flagged(self):
        m = Model(4, 4, 4)
        m.box(10, 10, 10, 12, 12, 12, palette.DIRT)  # silently dropped by Model.set
        self.assertTrue(check_art.check_model(self.model(m), 6000))

    def test_tiny_island_is_tolerated(self):
        m = blob()
        m.box(18, 18, 18, 19, 18, 18, palette.STONE)
        self.assertEqual(check_art.check_model(self.model(m), 6000), [])

    def test_load_round_trips(self):
        m = blob()
        m.set(2, 5, 7, palette.STONE)
        got = check_art.vox.load(self.model(m))
        self.assertEqual(got.v, m.v)
        self.assertEqual(got.size, m.size)

    def test_hollow_keeps_visible_surface(self):
        def solid():
            m = Model(7, 7, 7)
            m.box(0, 0, 0, 6, 6, 6, palette.DIRT)
            return m

        def surface(m):
            return {
                p
                for p in m.v
                if any(
                    m.get(p[0] + d[0], p[1] + d[1], p[2] + d[2]) is None
                    for d in ((1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1))
                )
            }

        cube = solid()
        visible = surface(cube)
        cube.hollow(2)
        self.assertEqual(cube.voxel_count(), 343 - 27)
        self.assertNotIn((3, 3, 3), cube.v)
        self.assertIn((0, 0, 0), cube.v)
        # The visible surface is never hollowed away. (A 2-voxel-thick shell
        # necessarily has an inner layer whose six neighbours are all filled,
        # so "every remaining voxel touches air" is only true for thickness 1.)
        self.assertLessEqual(visible, set(cube.v))
        self.assertEqual(set(cube.v), {p for p in solid().v if p not in {
            (x, y, z) for x in range(2, 5) for y in range(2, 5) for z in range(2, 5)
        }})

        thin = solid()
        thin.hollow(1)
        self.assertEqual(thin.voxel_count(), 343 - 125)
        self.assertEqual(set(thin.v), visible)
        for p in thin.v:
            self.assertTrue(
                any(
                    thin.get(p[0] + d[0], p[1] + d[1], p[2] + d[2]) is None
                    for d in ((1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1))
                ),
                f"{p} is fully enclosed after hollow(1)",
            )


class Characters(unittest.TestCase):
    """The player, NPC and armor models at 1/16 voxels."""

    ARMOR_TIERS = ("copper", "iron", "silver", "gold")
    # Armor file suffix -> player part file it covers.
    ARMOR_PARTS = {
        "head": "head",
        "body": "torso",
        "arm_l": "arm_l",
        "arm_r": "arm_r",
        "leg_l": "leg_l",
        "leg_r": "leg_r",
    }

    def _armor_world(self, tier, piece, part):
        m = check_art.vox.load(os.path.join(palette.ASSETS, "models", "armor", f"{tier}_{piece}.vox"))
        off, piv = base.OFFSETS[part], base.PIVOTS[part]
        return {
            (x - 1 + off[0] - piv[0],
             y - 1 + off[1] - piv[1],
             z - 1 + off[2] - piv[2])
            for x, y, z in m.v
        }

    def test_armor_shell_encloses_body(self):
        checked = 0
        for tier in self.ARMOR_TIERS:
            for piece, part in self.ARMOR_PARTS.items():
                armor = check_art.vox.load(
                    os.path.join(palette.ASSETS, "models", "armor", f"{tier}_{piece}.vox")
                )
                body = check_art.vox.load(
                    os.path.join(palette.ASSETS, "models", "player", f"{part}.vox")
                )
                body_set = set(body.v)
                for x, y, z in armor.v:
                    p = (x - 1, y - 1, z - 1)
                    near = any(
                        (p[0] + dx, p[1] + dy, p[2] + dz) in body_set
                        for dx in (-1, 0, 1)
                        for dy in (-1, 0, 1)
                        for dz in (-1, 0, 1)
                    )
                    self.assertTrue(
                        near,
                        f"{tier}_{piece}.vox: {p} is not within 1 voxel of player {part}.vox",
                    )
                checked += 1
        self.assertEqual(checked, 24)

    def test_armor_pieces_do_not_overlap_at_rest(self):
        for tier in self.ARMOR_TIERS:
            body = self._armor_world(tier, "body", "torso")
            for arm in ("arm_l", "arm_r"):
                arm_world = self._armor_world(tier, arm, arm)
                overlap = body & arm_world
                self.assertFalse(
                    overlap,
                    f"{tier}: body overlaps {arm} at {len(overlap)} voxels",
                )
            leg_l = self._armor_world(tier, "leg_l", "leg_l")
            leg_r = self._armor_world(tier, "leg_r", "leg_r")
            overlap = leg_l & leg_r
            self.assertFalse(
                overlap,
                f"{tier}: leg_l overlaps leg_r at {len(overlap)} voxels",
            )

    def test_armor_never_below_ground(self):
        for tier in self.ARMOR_TIERS:
            for piece, part in self.ARMOR_PARTS.items():
                world = self._armor_world(tier, piece, part)
                low = min(y for _, y, _ in world)
                self.assertGreaterEqual(
                    low, 0, f"{tier}_{piece}.vox reaches world y={low}"
                )


class Rigs(Base):
    RIG = '(parts: [(name: "a", file: "a.vox", pivot: (0,0,0))])'

    def test_missing_part_is_flagged(self):
        p = os.path.join(self.dir, "rig.ron")
        with open(p, "w") as f:
            f.write(self.RIG)
        self.assertTrue(check_art.check_rig(p))

    def test_existing_part_passes(self):
        p = os.path.join(self.dir, "rig.ron")
        with open(p, "w") as f:
            f.write(self.RIG)
        self.model(blob(), "a.vox")
        self.assertEqual(check_art.check_rig(p), [])


class Families(Base):
    def test_unlisted_families_have_no_errors(self):
        empty = {
            name: {"textures": [], "models": [], "rigs": [], "detached": {}, "max_voxels": 1}
            for name in check_art.FAMILY_NAMES
        }
        for name in check_art.FAMILY_NAMES:
            self.assertEqual(check_art.check_family(name, self.dir, empty), [])

    def test_family_reports_missing_and_bad_assets(self):
        self.tex(noisy(), "ok.png")
        fams = {"x": {"textures": ["ok.png", "gone.png"], "models": [], "rigs": [], "detached": {}, "max_voxels": 10}}
        errs = check_art.check_family("x", self.dir, fams)
        self.assertEqual(len(errs), 1)
        self.assertIn("missing", errs[0])


class ItemsAndFurniture(unittest.TestCase):
    """Held items and furniture at 1/16 voxels (one block = 16 voxels)."""

    BLOCKS = os.path.join(palette.ASSETS, "models", "blocks")
    ITEMS = os.path.join(palette.ASSETS, "models", "items")
    # Melee weapons: the spec's sword, about 1.2 blocks long.
    SWORDS = ("4.vox", "24.vox", "3507.vox", "3508.vox", "3514.vox", "3520.vox")
    # Pickaxes and axes, as held items.
    TOOLS = ("1.vox", "10.vox", "3506.vox", "3509.vox", "3512.vox", "3515.vox", "3518.vox", "3521.vox")

    def item(self, name):
        return check_art.vox.load(os.path.join(self.ITEMS, name))

    def block(self, name):
        return check_art.vox.load(os.path.join(self.BLOCKS, name))

    def test_every_item_file_is_registered(self):
        on_disk = {f for f in os.listdir(self.ITEMS) if f.endswith(".vox")}
        registered = {os.path.basename(p) for p in check_art.FAMILIES["items"]["models"]}
        self.assertEqual(on_disk, registered)

    def test_all_m6_items_exist_and_pass_checks(self):
        m6 = ("49.vox", "50.vox", "53.vox", "54.vox", "55.vox", "158.vox", "159.vox", "274.vox")
        for name in m6:
            p = os.path.join(self.ITEMS, name)
            self.assertTrue(os.path.isfile(p), f"missing {name}")
            errs = check_art.check_model(p, check_art.FAMILIES["items"]["max_voxels"])
            self.assertEqual(errs, [], f"{name} check failed: {errs}")

    def test_all_m9_items_exist_and_pass_checks(self):
        m9 = (
            "172.vox",
            "173.vox",
            "174.vox",
            "192.vox",
            "193.vox",
            "214.vox",
            "221.vox",
            "267.vox",
            "288.vox",
            "906.vox",
        )
        for name in m9:
            p = os.path.join(self.ITEMS, name)
            self.assertTrue(os.path.isfile(p), f"missing {name}")
            errs = check_art.check_model(p, check_art.FAMILIES["items"]["max_voxels"])
            self.assertEqual(errs, [], f"{name} check failed: {errs}")

    def test_melee_swords_are_nineteen_voxels_long(self):
        for name in self.SWORDS:
            m = self.item(name)
            self.assertEqual(max(m.size[0], m.size[1]), 19, f"{name}: want a 19-voxel sword")
            # The blade runs corner to corner, so its tip reaches the far corner region.
            self.assertTrue(
                any(x >= 17 and y >= 17 for x, y, _ in m.v),
                f"{name}: the tip does not reach the box corner",
            )

    def test_held_items_are_one_or_two_voxels_thick(self):
        for name in self.SWORDS + self.TOOLS + ("39.vox", "40.vox", "8.vox"):
            m = self.item(name)
            depth = {z for _, _, z in m.v}
            self.assertLessEqual(max(depth), 1, f"{name}: a voxel sits above z=1")

    def test_held_items_fit_within_nineteen_voxels(self):
        for name in self.SWORDS + self.TOOLS + ("39.vox", "40.vox", "8.vox"):
            m = self.item(name)
            self.assertLessEqual(max(m.size[0], m.size[1]), 19, f"{name}: larger than 1.2 blocks")

    def test_metal_heads_are_two_tone_with_an_edge_line(self):
        for name in self.SWORDS + self.TOOLS:
            m = self.item(name)
            self.assertGreaterEqual(len(set(m.v.values())), 4, f"{name}: needs main, light, dark and handle tones")

    def test_single_cell_furniture_fills_one_block(self):
        for name in ("workbench.vox", "anvil.vox", "furnace.vox", "chest.vox", "door_open.vox", "table.vox", "chair.vox", "platform.vox"):
            self.assertEqual(self.block(name).size, (16, 16, 16), name)
        self.assertEqual(self.block("door_closed.vox").size, (16, 16, 4))

    def test_closed_door_is_four_voxels_thick(self):
        m = self.block("door_closed.vox")
        self.assertEqual({z for _, _, z in m.v}, set(range(4)))

    def test_platform_is_three_voxels_thick(self):
        m = self.block("platform.vox")
        ys = [y for _, y, _ in m.v]
        self.assertEqual(max(ys) - min(ys) + 1, 3)

    def test_table_and_chair_have_two_by_two_legs(self):
        for name in ("table.vox", "chair.vox"):
            m = self.block(name)
            feet = {(x, z) for x, y, z in m.v if y == 0}
            self.assertEqual(len(feet), 16, f"{name}: four 2x2 legs = 16 floor cells")

    def test_chest_has_gold_lock_and_iron_straps(self):
        m = self.block("chest.vox")
        colours = set(m.v.values())
        self.assertIn(palette.hexc("#e6c040"), colours, "chest needs its gold lock")
        straps = {x for (x, y, z), c in m.v.items() if c == palette.hexc("#8b8f9a")}
        self.assertGreaterEqual(len(straps), 2, "chest needs two iron straps")

    def test_furnace_mouth_glows(self):
        colours = set(self.block("furnace.vox").v.values())
        self.assertIn(palette.hexc("#ffd84a"), colours, "furnace mouth needs its light shade")
        self.assertIn(palette.hexc("#ff9420"), colours, "furnace mouth needs its ember shade")

    def test_demon_altar_spans_three_by_two_by_one_blocks(self):
        self.assertEqual(self.block("demon_altar.vox").size, (48, 32, 16))

    def test_axes_have_bearded_silhouette_and_tapered_edge(self):
        for name in ("3506.vox", "10.vox", "3512.vox", "3518.vox", "45.vox", "217.vox"):
            m = self.item(name)
            # Haft extends cleanly through socket up to at least (13, 13)
            self.assertTrue(
                any(x == 13 and y == 13 for (x, y, z) in m.v),
                f"{name}: haft/head must extend to (13, 13)",
            )
            # Clear negative space (air gap) under the beard between blade bottom and haft
            gap_voxels = [
                (x, y)
                for (x, y, z) in m.v
                if (x, y) in ((10, 7), (11, 7), (11, 6), (12, 6))
            ]
            self.assertEqual(
                gap_voxels,
                [],
                f"{name}: expected negative space under axe beard, found {gap_voxels}",
            )
            # Blade projects on +X side (x >= 16) with a sharp 1-2 voxel tapered edge
            edge_cols = {}
            for x, y, z in m.v:
                if x >= 16:
                    edge_cols.setdefault((x, y), []).append(z)
            self.assertTrue(edge_cols, f"{name}: missing flared blade at x >= 16")
            self.assertTrue(
                all(len(zs) <= 2 for zs in edge_cols.values()),
                f"{name}: cutting edge at x >= 16 must taper to 1-2 voxels thick",
            )


if __name__ == "__main__":
    unittest.main()
