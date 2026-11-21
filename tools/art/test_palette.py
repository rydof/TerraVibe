"""Run: python3 -I tools/art/test_palette.py"""
import os
import re
import subprocess
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
DATA = os.path.join(HERE, "palette_data.py")

# Names exported by palette.py before the sampled-palette change.
LEGACY = """DIRT DIRT_D DIRT_L DIRT_P STONE STONE_D STONE_DD STONE_L GRASS GRASS_D GRASS_L
SAND SAND_D SAND_L MUD MUD_D MUD_L JUNGLE JUNGLE_D JUNGLE_L SNOW SNOW_D SNOW_L ICE ICE_D
ICE_L WOOD WOOD_D WOOD_L WOOD_G BARK BARK_D BARK_L RING RING_D SANDSTONE SANDSTONE_D
SANDSTONE_L HSAND HSAND_D HSAND_L GLASS_F GLASS_G COPPER COPPER_L IRON IRON_L SILVER
SILVER_L GOLD GOLD_L EBONSTONE EBONSTONE_D EBONSTONE_DD EBONSTONE_L CORRUPT CORRUPT_D
CORRUPT_L CDIRT CDIRT_D CDIRT_P DEMONITE DEMONITE_L ROOT ASSETS hexc shade""".split()


def image(rows):
    """rows of (r,g,b,a) tuples -> pixel grid."""
    return [list(r) for r in rows]


class Sampler(unittest.TestCase):
    def test_dominant_picks_most_frequent_opaque_colours(self):
        import sample_palette as sp

        red, green, blue = (200, 8, 8, 255), (8, 200, 8, 255), (8, 8, 200, 255)
        img = image([
            [red, red, red, red],
            [red, green, green, green],
            [green, blue, blue, blue],
            [blue, blue, blue, blue],
        ])
        # blue x 7, red x 5, green x 4; 5-bit quantised values are expanded back to 8 bits
        got = sp.dominant(img, (0, 0, 4, 4), 2)
        self.assertEqual(len(got), 2)
        self.assertEqual(set(got), {"#ce0808", "#0808ce"})

    def test_dominant_ignores_transparent(self):
        import sample_palette as sp

        clear, one = (255, 255, 255, 0), (80, 80, 80, 255)
        img = image([[clear, clear], [clear, one]])
        self.assertEqual(sp.dominant(img, (0, 0, 2, 2), 1), ["#525252"])

    def test_dominant_sorts_dark_to_light(self):
        import sample_palette as sp

        a, b = (240, 240, 240, 255), (16, 16, 16, 255)
        img = image([[a, a, a, b]])
        got = sp.dominant(img, (0, 0, 4, 1), 2)
        self.assertEqual(got, ["#101010", "#f7f7f7"])


class Output(unittest.TestCase):
    def lines(self):
        with open(DATA) as f:
            return f.read().splitlines()

    def test_data_has_no_keys_the_sampler_does_not_produce(self):
        import sample_palette as sp

        expected = set()
        for family, (_, _, n) in sp.SOURCES.items():
            expected |= {family + suf for suf in sp.OVERRIDES.get(family, sp.SUFFIXES[n])}
        keys = {ln.split(" = ")[0] for ln in self.lines() if " = " in ln and not ln.startswith("#")}
        self.assertEqual(sorted(keys - expected), [])
        self.assertEqual(sorted(expected - keys), [])

    def test_regenerating_from_pack_reproduces_data(self):
        import sample_palette as sp

        pack = os.path.join(ROOT, "assets", "terraria-pack", "Images")
        if not os.path.isdir(pack):
            self.skipTest("Terraria pack not installed")
        want = {k: v for k, v in sp.sample(pack).items()}
        got = {}
        for ln in self.lines():
            if " = " in ln and not ln.startswith("#"):
                k, v = ln.split(" = ")
                got[k] = v.strip('"')
        self.assertEqual(got, want)

    def test_every_family_ramp_is_strictly_dark_to_light(self):
        import palette
        import sample_palette as sp

        def lum(c):
            return sum(c[:3])

        families = {f: sp.OVERRIDES.get(f, sp.SUFFIXES[n]) for f, (_, _, n) in sp.SOURCES.items()}
        families["CDIRT"] = ["_P", "_D", ""]
        for fam, sufs in families.items():
            vals = [getattr(palette, fam + s) for s in sufs]
            self.assertEqual(sorted(vals, key=lum), vals, fam)
            self.assertEqual(len({lum(v) for v in vals}), len(vals), fam + " has equal shades")
            if not (hasattr(palette, fam + "_D") and hasattr(palette, fam + "_L")):
                continue
            dark, base, light = palette.ramp(fam)
            self.assertLess(lum(dark), lum(base), fam)
            self.assertLess(lum(base), lum(light), fam)

    def test_corruption_dirt_is_not_surface_dirt(self):
        import palette

        for s in ("_P", "_D", ""):
            self.assertNotEqual(getattr(palette, "CDIRT" + s), getattr(palette, "DIRT" + s))
        r, g, b, _ = palette.CDIRT
        self.assertGreater(b, g)  # purple-leaning

    def test_ores_keep_a_bright_body(self):
        import palette

        for ore in ("COPPER", "IRON", "SILVER", "GOLD"):
            self.assertGreater(sum(getattr(palette, ore)[:3]), 270, ore)

    def test_sampler_fails_loudly_on_too_few_colours(self):
        import sample_palette as sp

        old = dict(sp.SOURCES)
        sp.SOURCES.clear()
        sp.SOURCES["FLAT"] = ("Tiles_0.png", (0, 0, 1, 1), 3)
        try:
            pack = os.path.join(ROOT, "assets", "terraria-pack", "Images")
            if not os.path.isdir(pack):
                self.skipTest("Terraria pack not installed")
            with self.assertRaisesRegex(ValueError, "FLAT"):
                sp.sample(pack)
        finally:
            sp.SOURCES.clear()
            sp.SOURCES.update(old)

    def test_output_has_only_hex_values(self):
        pat = re.compile(r'^[A-Z_0-9]+ = "#[0-9a-f]{6}"$')
        for ln in self.lines():
            if ln.strip() and not ln.startswith("#") and not ln.startswith('"""'):
                self.assertRegex(ln, pat)

    def test_output_has_no_pixel_grids(self):
        self.assertLess(os.path.getsize(DATA), 6 * 1024)
        for ln in self.lines():
            self.assertLessEqual(len(ln), 80)

    def test_palette_exports_all_legacy_names(self):
        import palette

        missing = [n for n in LEGACY if not hasattr(palette, n)]
        self.assertEqual(missing, [])
        self.assertEqual((palette.TILE, palette.VPU), (16, 16))
        dark, base, light = palette.ramp("DIRT")
        self.assertEqual(base, palette.DIRT)
        self.assertEqual((dark, light), (palette.DIRT_D, palette.DIRT_L))

    def test_no_terraria_images_tracked(self):
        out = subprocess.run(["git", "ls-files"], cwd=ROOT, capture_output=True, text=True).stdout
        self.assertEqual([l for l in out.splitlines() if "terraria-pack" in l.lower()], [])


if __name__ == "__main__":
    unittest.main()
