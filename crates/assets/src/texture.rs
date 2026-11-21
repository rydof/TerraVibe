use crate::{AssetError, Xnb};

const READER: &str = "Microsoft.Xna.Framework.Content.Texture2DReader";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<[u8; 4]>,
}

impl RgbaImage {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![[0; 4]; width as usize * height as usize],
        }
    }

    pub fn get(&self, x: u32, y: u32) -> [u8; 4] {
        self.pixels[y as usize * self.width as usize + x as usize]
    }
}

pub(crate) fn unpremultiply(p: [u8; 4]) -> [u8; 4] {
    let a = p[3] as u32;
    if a == 0 {
        return [0, 0, 0, 0];
    }
    let f = |c: u8| ((c as u32 * 255) / a).min(255) as u8;
    [f(p[0]), f(p[1]), f(p[2]), p[3]]
}

fn u32_at(b: &[u8], pos: &mut usize) -> Result<u32, AssetError> {
    let end = pos.checked_add(4).ok_or(AssetError::Truncated)?;
    let s = b.get(*pos..end).ok_or(AssetError::Truncated)?;
    *pos = end;
    Ok(u32::from_le_bytes(s.try_into().unwrap()))
}

pub(crate) fn check_reader(xnb: &Xnb, expected: &'static str) -> Result<(), AssetError> {
    let found = xnb.readers.first().map(String::as_str).unwrap_or("");
    if found.split(',').next().unwrap_or("") == expected {
        Ok(())
    } else {
        Err(AssetError::UnexpectedReader {
            expected,
            found: found.to_string(),
        })
    }
}

pub fn decode_texture(xnb: &Xnb) -> Result<RgbaImage, AssetError> {
    check_reader(xnb, READER)?;
    let mut pos = 0;
    decode_texture_body(&xnb.primary, &mut pos)
}

/// Texture2D payload (after any type id), starting at `pos`; advances `pos` past it.
pub(crate) fn decode_texture_body(b: &[u8], pos: &mut usize) -> Result<RgbaImage, AssetError> {
    let format = u32_at(b, pos)? as i32;
    if format != 0 && format != 5 {
        return Err(AssetError::UnsupportedFormat(format!(
            "texture surface format {format}"
        )));
    }
    let width = u32_at(b, pos)?;
    let height = u32_at(b, pos)?;
    let mips = u32_at(b, pos)?;
    if mips == 0 {
        return Err(AssetError::Malformed("texture has no mip levels".into()));
    }
    let size = u32_at(b, pos)? as usize;
    let data = b
        .get(*pos..pos.checked_add(size).ok_or(AssetError::Truncated)?)
        .ok_or(AssetError::Truncated)?;
    let overflow = || AssetError::Malformed("texture dimensions overflow".into());
    let (w, h) = (width as usize, height as usize);
    let expected = if format == 0 {
        w.checked_mul(h).and_then(|n| n.checked_mul(4))
    } else {
        w.div_ceil(4)
            .checked_mul(h.div_ceil(4))
            .and_then(|n| n.checked_mul(16))
    }
    .ok_or_else(overflow)?;
    if size != expected {
        return Err(AssetError::Malformed(format!(
            "mip 0 has {size} bytes, expected {expected}"
        )));
    }
    *pos += size;
    // Skip the remaining mip levels.
    for _ in 1..mips {
        let n = u32_at(b, pos)? as usize;
        *pos = pos
            .checked_add(n)
            .filter(|&e| e <= b.len())
            .ok_or(AssetError::Truncated)?;
    }
    let pixels = if format == 0 {
        data.as_chunks::<4>()
            .0
            .iter()
            .map(|c| unpremultiply(*c))
            .collect()
    } else {
        decode_dxt3(data, w, h)
    };
    Ok(RgbaImage {
        width,
        height,
        pixels,
    })
}

/// DXT3 (BC2): 4x4 blocks of 8 bytes explicit 4-bit alpha plus an 8-byte colour block.
/// `data` length must equal `ceil(w/4) * ceil(h/4) * 16`.
fn decode_dxt3(data: &[u8], w: usize, h: usize) -> Vec<[u8; 4]> {
    let mut out = vec![[0u8; 4]; w * h];
    let expand = |c: u16| {
        let (r, g, b) = ((c >> 11) & 31, (c >> 5) & 63, c & 31);
        [
            (r * 255 / 31) as u32,
            (g * 255 / 63) as u32,
            (b * 255 / 31) as u32,
        ]
    };
    for (i, blk) in data.as_chunks::<16>().0.iter().enumerate() {
        let (bx, by) = (i % w.div_ceil(4), i / w.div_ceil(4));
        let c0 = expand(u16::from_le_bytes([blk[8], blk[9]]));
        let c1 = expand(u16::from_le_bytes([blk[10], blk[11]]));
        let mix = |a: u32, b: u32| (2 * a + b) / 3;
        let pal = [
            c0,
            c1,
            [mix(c0[0], c1[0]), mix(c0[1], c1[1]), mix(c0[2], c1[2])],
            [mix(c1[0], c0[0]), mix(c1[1], c0[1]), mix(c1[2], c0[2])],
        ];
        let idx = u32::from_le_bytes([blk[12], blk[13], blk[14], blk[15]]);
        for py in 0..4 {
            for px in 0..4 {
                let (x, y) = (bx * 4 + px, by * 4 + py);
                if x >= w || y >= h {
                    continue;
                }
                let n = py * 4 + px;
                let a = (blk[n / 2] >> (4 * (n % 2))) & 15;
                let c = pal[((idx >> (2 * n)) & 3) as usize];
                out[y * w + x] = unpremultiply([c[0] as u8, c[1] as u8, c[2] as u8, a * 17]);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tex_xnb(primary: Vec<u8>) -> Xnb {
        Xnb {
            readers: vec![format!("{READER}, Microsoft.Xna.Framework.Graphics")],
            primary,
        }
    }

    #[test]
    fn unpremultiply_zero_alpha() {
        assert_eq!(unpremultiply([0, 0, 0, 0]), [0, 0, 0, 0]);
    }
    #[test]
    fn unpremultiply_half() {
        assert_eq!(unpremultiply([64, 32, 0, 128]), [127, 63, 0, 128]);
    }
    #[test]
    fn non_color_format_is_error() {
        let x = tex_xnb(4i32.to_le_bytes().to_vec());
        assert!(matches!(
            decode_texture(&x),
            Err(AssetError::UnsupportedFormat(_))
        ));
    }
    #[test]
    fn wrong_reader_is_error() {
        let mut x = tex_xnb(vec![]);
        x.readers[0] = "Other".into();
        assert!(matches!(
            decode_texture(&x),
            Err(AssetError::UnexpectedReader { .. })
        ));
    }
    #[test]
    fn huge_size_is_error_not_panic() {
        let mut p = vec![];
        for v in [0u32, 0xFFFF_FFFF, 0xFFFF_FFFF, 1, 0xFFFF_FFFF] {
            p.extend(v.to_le_bytes());
        }
        assert!(decode_texture(&tex_xnb(p)).is_err());
    }
    #[test]
    fn decodes_one_pixel() {
        let mut p = vec![];
        for v in [0u32, 1, 1, 1, 4] {
            p.extend(v.to_le_bytes());
        }
        p.extend([64, 32, 0, 128]);
        let img = decode_texture(&tex_xnb(p)).unwrap();
        assert_eq!(img.get(0, 0), [127, 63, 0, 128]);
    }
}

#[cfg(test)]
mod dxt_tests {
    use super::*;

    fn tex(format: u32, w: u32, h: u32, mips: &[&[u8]]) -> Vec<u8> {
        let mut v = Vec::new();
        for n in [format, w, h, mips.len() as u32] {
            v.extend(n.to_le_bytes());
        }
        for m in mips {
            v.extend((m.len() as u32).to_le_bytes());
            v.extend(*m);
        }
        v
    }

    fn block() -> [u8; 16] {
        let mut b = [0u8; 16];
        // Alpha nibbles: pixel n has alpha n (low nibble = even pixel).
        for (i, byte) in b.iter_mut().take(8).enumerate() {
            *byte = (2 * i as u8) | ((2 * i as u8 + 1) << 4);
        }
        b[8..10].copy_from_slice(&0xF800u16.to_le_bytes()); // c0 = red
        b[10..12].copy_from_slice(&0x001Fu16.to_le_bytes()); // c1 = blue
        // Pixel n uses palette index n % 4.
        let idx: u32 = (0..16).fold(0, |a, n| a | ((n % 4) << (2 * n)));
        b[12..16].copy_from_slice(&idx.to_le_bytes());
        b
    }

    #[test]
    fn dxt3_block_alpha_palette_and_unpremultiply() {
        let data = tex(5, 4, 4, &[&block()]);
        let img = decode_texture_body(&data, &mut 0).unwrap();
        assert_eq!(img.pixels.len(), 16);
        // Pixel 0: alpha 0 -> fully transparent.
        assert_eq!(img.get(0, 0), [0, 0, 0, 0]);
        // Pixel 15 (idx 3 = (c0+2c1)/3): alpha 15*17 = 255, no change.
        assert_eq!(img.get(3, 3), [85, 0, 170, 255]);
        // Pixel 4 (idx 0 = red), alpha 68: 255*255/68 clamps to 255.
        assert_eq!(img.get(0, 1), [255, 0, 0, 68]);
        // Pixel 5 (idx 1 = blue), alpha 85.
        assert_eq!(img.get(1, 1), [0, 0, 255, 85]);
        // Pixel 6 (idx 2 = (2*c0+c1)/3), alpha 102: 170*255/102 = 425 clamps to 255.
        assert_eq!(img.get(2, 1), [255, 0, 212, 102]);
        // Alpha values follow the nibble layout across the whole block.
        for n in 0..16u32 {
            assert_eq!(img.get(n % 4, n / 4)[3] as u32, n * 17);
        }
    }

    #[test]
    fn dxt3_clips_non_multiple_of_four() {
        // 5x3 -> 2x1 blocks.
        let (a, b) = (block(), block());
        let blocks: Vec<u8> = a.iter().chain(b.iter()).copied().collect();
        let img = decode_texture_body(&tex(5, 5, 3, &[&blocks]), &mut 0).unwrap();
        assert_eq!((img.width, img.height, img.pixels.len()), (5, 3, 15));
        assert_eq!(img.get(4, 2)[3], 8 * 17); // block 1, local (0,2) = pixel 8
        // Wrong payload size for the dimensions is an error.
        assert!(decode_texture_body(&tex(5, 5, 3, &[&a]), &mut 0).is_err());
    }

    #[test]
    fn extra_mips_are_skipped() {
        let mut data = tex(0, 1, 1, &[&[1, 2, 3, 255], &[9, 9, 9, 9, 9, 9, 9, 9, 9, 9]]);
        data.push(0xAB); // trailing byte after the texture
        let mut pos = 0;
        let img = decode_texture_body(&data, &mut pos).unwrap();
        assert_eq!(img.get(0, 0), [1, 2, 3, 255]);
        assert_eq!(pos, data.len() - 1);
        // A truncated extra mip is an error.
        let cut = &data[..data.len() - 5];
        assert!(decode_texture_body(cut, &mut 0).is_err());
    }
}
