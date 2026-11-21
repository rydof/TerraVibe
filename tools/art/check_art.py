"""Validates generated 16px art. Run: python3 -I tools/art/check_art.py [family ...]"""
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import palette  # noqa: E402
import png  # noqa: E402
import vox  # noqa: E402
from families import FAMILIES, FAMILY_NAMES, MAX_AXIS, MAX_FREE_ISLAND, MAX_UPSCALE_RATIO, TEXTURE_SIZE  # noqa: E402


def palette_colours():
    return {v for v in vars(palette).values() if isinstance(v, tuple) and len(v) == 4 and all(isinstance(n, int) for n in v)}


def upscale_ratio(pixels):
    """Share of aligned 2x2 pixel blocks whose four pixels are identical (1.0 = a pure 2x upscale)."""
    h, w = len(pixels), len(pixels[0])
    total = same = 0
    for y in range(0, h - 1, 2):
        for x in range(0, w - 1, 2):
            total += 1
            if pixels[y][x] == pixels[y][x + 1] == pixels[y + 1][x] == pixels[y + 1][x + 1]:
                same += 1
    return same / total if total else 0.0


def check_texture(path, colours=None):
    colours = palette_colours() if colours is None else colours
    _, _, depth, ctype = png.header(path)
    if (depth, ctype) != (8, 6):
        return [f"{path}: PNG depth {depth} colour type {ctype}, expected 8-bit RGBA (type 6)"]
    px = png.load_pixels(path)
    h, w = len(px), len(px[0])
    if (w, h) != TEXTURE_SIZE:
        return [f"{path}: size {w}x{h}, expected {TEXTURE_SIZE[0]}x{TEXTURE_SIZE[1]}"]
    errs = []
    bad = {p for row in px for p in row if p[3] not in (0, 255) or (p[3] == 255 and p not in colours)}
    for p in sorted(bad)[:5]:
        errs.append(f"{path}: colour {p} is not a palette colour with alpha 0/255")
    ratio = upscale_ratio(px)
    if ratio > MAX_UPSCALE_RATIO:
        errs.append(f"{path}: upscale_ratio {ratio:.2f} > {MAX_UPSCALE_RATIO} (looks like an upscaled low-res texture)")
    return errs


def is_dilation(model):
    """True if the voxel set is exactly a 2x dilation of some voxel set (at either parity)."""
    if not model.v:
        return False
    mins = [min(p[i] for p in model.v) for i in range(3)]
    norm = {(x - mins[0], y - mins[1], z - mins[2]) for x, y, z in model.v}
    if len(norm) % 8:
        return False
    return all(
        ((x & ~1) + a, (y & ~1) + b, (z & ~1) + c) in norm
        for x, y, z in norm
        for a in (0, 1)
        for b in (0, 1)
        for c in (0, 1)
    )


def islands(model):
    """Sizes of 26-connected components, largest first."""
    left, sizes = set(model.v), []
    while left:
        stack, n = [left.pop()], 1
        while stack:
            x, y, z = stack.pop()
            for dx in (-1, 0, 1):
                for dy in (-1, 0, 1):
                    for dz in (-1, 0, 1):
                        q = (x + dx, y + dy, z + dz)
                        if q in left:
                            left.remove(q)
                            stack.append(q)
                            n += 1
        sizes.append(n)
    return sorted(sizes, reverse=True)


def check_model(path, max_voxels, detached=0):
    m = vox.load(path)
    errs = []
    if not m.v:
        return [f"{path}: empty model (no voxels; out-of-bounds writes are dropped silently)"]
    if max(m.size) > MAX_AXIS:
        errs.append(f"{path}: axis {max(m.size)} > {MAX_AXIS}")
    if m.voxel_count() > max_voxels:
        errs.append(f"{path}: {m.voxel_count()} voxels > budget {max_voxels}")
    if is_dilation(m):
        errs.append(f"{path}: is a 2x dilation of a coarser model")
    stray = [s for s in islands(m)[1:] if s > MAX_FREE_ISLAND]
    if sum(stray) > detached:
        errs.append(f"{path}: floating islands {stray} (declared allowance {detached})")
    return errs


def check_rig(path):
    with open(path) as f:
        text = f.read()
    d = os.path.dirname(path)
    return [
        f"{path}: missing part {name}"
        for name in re.findall(r'file:\s*"([^"]+)"', text)
        if not os.path.isfile(os.path.join(d, name))
    ]


def check_family(name, root, families=None):
    fam = (FAMILIES if families is None else families)[name]
    errs, colours = [], palette_colours()
    for rel in fam["textures"]:
        errs += _guard(check_texture, os.path.join(root, rel), colours)
    for rel in fam["models"]:
        errs += _guard(check_model, os.path.join(root, rel), fam["max_voxels"], fam["detached"].get(rel, 0))
    for rel in fam["rigs"]:
        errs += _guard(check_rig, os.path.join(root, rel))
    return errs


def _guard(fn, path, *args):
    if not os.path.isfile(path):
        return [f"{path}: missing"]
    return fn(path, *args)


def main(argv):
    names = argv or FAMILY_NAMES
    unknown = [n for n in names if n not in FAMILIES]
    if unknown:
        print(f"unknown family: {', '.join(unknown)} (known: {' '.join(FAMILY_NAMES)})")
        return 2
    failed = False
    for name in names:
        fam = FAMILIES[name]
        count = len(fam["textures"]) + len(fam["models"]) + len(fam["rigs"])
        if not count:
            if argv:
                print(f"{name}: FAIL no assets listed in families.py")
                failed = True
            else:
                print(f"{name}: skipped (no assets listed yet)")
            continue
        errs = check_family(name, palette.ASSETS)
        print(f"{name}: {'FAIL' if errs else 'ok'} ({count} assets)")
        for e in errs:
            print("  " + e)
        failed |= bool(errs)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
