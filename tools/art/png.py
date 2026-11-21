"""Tiny stdlib PNG writer and reader for 8-bit RGBA textures."""
import os
import struct
import zlib


def _chunk(tag, data):
    c = struct.pack(">I", len(data)) + tag + data
    return c + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)


def save_pixels(path, pixels):
    """pixels: list of rows, each a list of (r,g,b,a)."""
    h, w = len(pixels), len(pixels[0])
    raw = b"".join(b"\x00" + bytes(v for p in row for v in p) for row in pixels)
    png = b"\x89PNG\r\n\x1a\n"
    png += _chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
    png += _chunk(b"IDAT", zlib.compress(raw, 9))
    png += _chunk(b"IEND", b"")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(png)


def save_png(path, rows, legend):
    """rows: list of equal-length strings; legend maps each char to rgba."""
    assert len({len(r) for r in rows}) == 1, "ragged rows"
    save_pixels(path, [[legend[ch] for ch in r] for r in rows])


def header(path):
    """(width, height, bit depth, colour type) from the first IHDR."""
    with open(path, "rb") as f:
        data = f.read(33)
    assert data[:8] == b"\x89PNG\r\n\x1a\n" and data[12:16] == b"IHDR", "not a PNG: " + path
    w, h, depth, ctype = struct.unpack(">IIBB", data[16:26])
    return w, h, depth, ctype


def load_pixels(path):
    """Reads an 8-bit non-interlaced PNG into rows of (r,g,b,a) tuples."""
    with open(path, "rb") as f:
        data = f.read()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", "not a PNG: " + path
    pos, idat, plte, trns, hdr = 8, [], b"", b"", None
    while pos < len(data):
        n, tag = struct.unpack(">I4s", data[pos : pos + 8])
        body = data[pos + 8 : pos + 8 + n]
        pos += 12 + n
        if tag == b"IHDR":
            hdr = struct.unpack(">IIBBBBB", body)
        elif tag == b"PLTE":
            plte = body
        elif tag == b"tRNS":
            trns = body
        elif tag == b"IDAT":
            idat.append(body)
    w, h, depth, ctype, _, _, interlace = hdr
    assert depth == 8 and interlace == 0, "unsupported PNG: " + path
    bpp = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[ctype]
    raw = zlib.decompress(b"".join(idat))
    stride, prev, rows = w * bpp, bytearray(w * bpp), []
    for y in range(h):
        base = y * (stride + 1)
        ft, cur = raw[base], bytearray(raw[base + 1 : base + 1 + stride])
        for i in range(stride):
            a = cur[i - bpp] if i >= bpp else 0
            b = prev[i]
            c = prev[i - bpp] if i >= bpp else 0
            if ft == 1:
                cur[i] = (cur[i] + a) & 255
            elif ft == 2:
                cur[i] = (cur[i] + b) & 255
            elif ft == 3:
                cur[i] = (cur[i] + (a + b) // 2) & 255
            elif ft == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pred = a if pa <= pb and pa <= pc else (b if pb <= pc else c)
                cur[i] = (cur[i] + pred) & 255
        prev = cur
        if ctype == 6:
            rows.append([tuple(cur[x * 4 : x * 4 + 4]) for x in range(w)])
        elif ctype == 2:
            rows.append([(*cur[x * 3 : x * 3 + 3], 255) for x in range(w)])
        elif ctype == 0:
            rows.append([(cur[x], cur[x], cur[x], 255) for x in range(w)])
        elif ctype == 4:
            rows.append([(cur[x * 2],) * 3 + (cur[x * 2 + 1],) for x in range(w)])
        else:
            rows.append(
                [(*plte[cur[x] * 3 : cur[x] * 3 + 3], trns[cur[x]] if cur[x] < len(trns) else 255) for x in range(w)]
            )
    return rows
