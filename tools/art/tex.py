"""Helpers for TILExTILE block textures: ascii grids + legend -> assets/blocks/<name>.png."""
import os

from palette import ASSETS, TILE
from png import save_png


def emit(name, rows, legend, base_legend=None):
    """Writes `<assets>/blocks/<name>.png`.

    With `base_legend`, `legend` is the overlay legend and is merged onto the
    base. The two key sets must be disjoint, so an overlay can never silently
    recolour base cells.
    """
    assert len(rows) == TILE and all(len(r) == TILE for r in rows), name
    if base_legend is not None:
        clash = set(legend) & set(base_legend)
        if clash:
            raise ValueError(f"{name}: overlay legend redefines base keys {sorted(clash)}")
        legend = {**base_legend, **legend}
    save_png(os.path.join(ASSETS, "blocks", name + ".png"), rows, legend)


def overlay(rows, patch):
    """Characters of `patch` other than '.' replace those of `rows`."""
    return ["".join(p if p != "." else b for b, p in zip(br, pr)) for br, pr in zip(rows, patch)]


def mirror(rows):
    return [r[::-1] for r in rows]
