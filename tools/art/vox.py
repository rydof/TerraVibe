"""Voxel model authoring: build in game axes, write MagicaVoxel .vox (v150).

Game axes: x right, y up, z toward the viewer (the model's front). `save` maps
them to MagicaVoxel's z-up frame (file x = x, file y = sz-1-z, file z = y), so
a model opens upright with its front facing MagicaVoxel's default view, and the
client's loader undoes the same mapping.
"""
import math
import os
import struct


class Model:
    def __init__(self, sx, sy, sz):
        assert max(sx, sy, sz) <= 255
        self.size = (sx, sy, sz)
        self.v = {}

    def set(self, x, y, z, c):
        sx, sy, sz = self.size
        if 0 <= x < sx and 0 <= y < sy and 0 <= z < sz:
            if c is None:
                self.v.pop((x, y, z), None)
            else:
                self.v[(x, y, z)] = tuple(c)

    def get(self, x, y, z):
        return self.v.get((x, y, z))

    def box(self, x0, y0, z0, x1, y1, z1, c):
        """Inclusive corners."""
        for x in range(min(x0, x1), max(x0, x1) + 1):
            for y in range(min(y0, y1), max(y0, y1) + 1):
                for z in range(min(z0, z1), max(z0, z1) + 1):
                    self.set(x, y, z, c)

    def ellipsoid(self, cx, cy, cz, rx, ry, rz, c):
        for x in range(math.floor(cx - rx), math.ceil(cx + rx) + 1):
            for y in range(math.floor(cy - ry), math.ceil(cy + ry) + 1):
                for z in range(math.floor(cz - rz), math.ceil(cz + rz) + 1):
                    d = ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2 + ((z - cz) / rz) ** 2
                    if d <= 1.0:
                        self.set(x, y, z, c)

    def sphere(self, cx, cy, cz, r, c):
        self.ellipsoid(cx, cy, cz, r, r, r, c)

    def cyl(self, cx, cz, r, y0, y1, c):
        """Upright cylinder around the y axis."""
        for y in range(y0, y1 + 1):
            for x in range(math.floor(cx - r), math.ceil(cx + r) + 1):
                for z in range(math.floor(cz - r), math.ceil(cz + r) + 1):
                    if (x - cx) ** 2 + (z - cz) ** 2 <= r * r:
                        self.set(x, y, z, c)

    def layers(self, grids, legend, x0=0, y0=0, z0=0):
        """grids[i] is the horizontal slice at y = y0 + i (bottom first); its
        rows run along z (row 0 at z0) and its columns along x. '.' is empty."""
        for i, grid in enumerate(grids):
            for r, row in enumerate(grid):
                for col, ch in enumerate(row):
                    if ch != "." and ch != " ":
                        self.set(x0 + col, y0 + i, z0 + r, legend[ch])

    def voxel_count(self):
        return len(self.v)

    def hollow(self, thickness=1):
        """Keep a shell `thickness` voxels thick, dropping the interior.

        The first layer is the visible surface (a voxel with at least one empty
        6-neighbour); each further layer is the filled 6-neighbours of the
        previous one. Colours are preserved. Unlike removing every fully
        surrounded voxel once, this can keep more than a 1-voxel shell.
        """
        assert thickness >= 1, thickness
        steps = ((1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1))

        def empty(p):
            return p not in self.v

        def around(p):
            return [(p[0] + d[0], p[1] + d[1], p[2] + d[2]) for d in steps]

        layer = {p for p in self.v if any(empty(q) for q in around(p))}
        keep = set(layer)
        for _ in range(thickness - 1):
            layer = {
                q
                for p in layer
                for q in around(p)
                if q in self.v and q not in keep
            }
            keep |= layer
        self.v = {p: c for p, c in self.v.items() if p in keep}

    def dilated2(self):
        """This model with every voxel replaced by a 2x2x2 block (size doubled; may exceed 255)."""
        out = Model(1, 1, 1)
        out.size = tuple(2 * n for n in self.size)
        for (x, y, z), c in self.v.items():
            for dx in (0, 1):
                for dy in (0, 1):
                    for dz in (0, 1):
                        out.v[(2 * x + dx, 2 * y + dy, 2 * z + dz)] = c
        return out

    def save(self, path):
        sx, sy, sz = self.size
        pal = []
        idx = {}
        for c in self.v.values():
            if c not in idx:
                pal.append(c)
                idx[c] = len(pal)
        assert len(pal) <= 255, f"{path}: {len(pal)} colours"
        xyzi = b"".join(
            struct.pack("4B", x, sz - 1 - z, y, idx[c])
            for (x, y, z), c in sorted(self.v.items())
        )
        rgba = b"".join(bytes(c) for c in pal) + bytes(4 * (256 - len(pal)))
        size = struct.pack("<3I", sx, sz, sy)

        def chunk(tag, data):
            return tag + struct.pack("<II", len(data), 0) + data

        children = (
            chunk(b"SIZE", size)
            + chunk(b"XYZI", struct.pack("<I", len(self.v)) + xyzi)
            + chunk(b"RGBA", rgba)
        )
        out = b"VOX " + struct.pack("<I", 150) + b"MAIN" + struct.pack("<II", 0, len(children)) + children
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "wb") as f:
            f.write(out)


def load(path):
    """Reads a .vox written by `save` back into game axes."""
    with open(path, "rb") as f:
        data = f.read()
    assert data[:4] == b"VOX ", "not a .vox: " + path
    size, xyzi, rgba = None, b"", b""
    pos = 20  # header 8 + MAIN chunk header 12
    while pos + 12 <= len(data):
        tag, n, kids = struct.unpack("<4sII", data[pos : pos + 12])
        body = data[pos + 12 : pos + 12 + n]
        pos += 12 + n + kids
        if tag == b"SIZE":
            size = struct.unpack("<3I", body)
        elif tag == b"XYZI":
            xyzi = body
        elif tag == b"RGBA":
            rgba = body
    fx, fy, fz = size  # file x, y(depth), z(up)
    m = Model(1, 1, 1)
    m.size = (fx, fz, fy)
    (count,) = struct.unpack("<I", xyzi[:4])
    for i in range(count):
        x, y, z, ci = xyzi[4 + 4 * i : 8 + 4 * i]
        c = tuple(rgba[(ci - 1) * 4 : ci * 4]) if rgba else (255, 255, 255, 255)
        m.v[(x, z, fy - 1 - y)] = c
    return m
