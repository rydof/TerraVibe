//! Terraria `SpriteFont` (ReLogic dynamic font XNB) and its pack text format.

use crate::texture::{check_reader, decode_texture_body};
use crate::xnb::read_7bit;
use crate::{AssetError, RgbaImage, Xnb};

const READER: &str = "ReLogic.Graphics.DynamicSpriteFontReader";
/// Upper bound on list lengths, so a corrupt count cannot trigger a huge allocation.
const MAX_GLYPHS: usize = 1 << 20;
const MAX_FONT_TEXT: usize = 16 << 20;

#[derive(Debug, Clone, PartialEq)]
pub struct Glyph {
    pub ch: char,
    /// x, y, w, h in the atlas.
    pub src: [u32; 4],
    pub crop: [i32; 4],
    /// Left bearing, advance width, right bearing.
    pub kerning: [f32; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpriteFont {
    pub atlas: RgbaImage,
    pub glyphs: Vec<Glyph>,
    pub line_spacing: i32,
    pub spacing: f32,
    pub default_char: Option<char>,
}

impl SpriteFont {
    pub fn glyph(&self, c: char) -> Option<&Glyph> {
        self.glyphs.iter().find(|g| g.ch == c)
    }
}

fn bad(m: impl Into<String>) -> AssetError {
    AssetError::Malformed(m.into())
}

struct Cur<'a> {
    b: &'a [u8],
    pos: usize,
}

impl Cur<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], AssetError> {
        let end = self.pos.checked_add(n).ok_or(AssetError::Truncated)?;
        let s = self.b.get(self.pos..end).ok_or(AssetError::Truncated)?;
        self.pos = end;
        Ok(s)
    }
    fn u32(&mut self) -> Result<u32, AssetError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32, AssetError> {
        Ok(self.u32()? as i32)
    }
    fn f32(&mut self) -> Result<f32, AssetError> {
        Ok(f32::from_bits(self.u32()?))
    }
    /// Non-null object marker (7-bit type id); the id itself is not needed.
    fn object(&mut self) -> Result<(), AssetError> {
        if read_7bit(self.b, &mut self.pos)? == 0 {
            return Err(bad("unexpected null object"));
        }
        Ok(())
    }
    /// List length, rejected if `n * elem` bytes cannot fit in the remaining input.
    fn count(&mut self, elem: usize) -> Result<usize, AssetError> {
        let n = self.u32()? as usize;
        let remaining = self.b.len() - self.pos.min(self.b.len());
        if n > MAX_GLYPHS || n.checked_mul(elem).is_none_or(|t| t > remaining) {
            return Err(bad("font list too long"));
        }
        Ok(n)
    }
    /// UTF-8 encoded char, as .NET `BinaryReader.ReadChar` writes it. Lone surrogates, overlong
    /// forms and stray bytes are rejected (`from_utf8` is strict).
    fn char(&mut self) -> Result<char, AssetError> {
        let first = self.take(1)?[0];
        let len = match first {
            0..=0x7f => 1,
            0xc0..=0xdf => 2,
            0xe0..=0xef => 3,
            _ => return Err(bad("invalid char encoding")),
        };
        let mut buf = [first, 0, 0];
        buf[1..len].copy_from_slice(self.take(len - 1)?);
        std::str::from_utf8(&buf[..len])
            .ok()
            .and_then(|s| s.chars().next())
            .ok_or_else(|| bad("invalid char"))
    }
}

/// Decodes a Terraria `.xnb` font (`ReLogic.Graphics.DynamicSpriteFontReader`).
///
/// Layout: f32 spacing, i32 line spacing, char default, i32 page count, then per page a
/// Texture2D, glyph rects, cropping rects, chars and Vector3 kerning (each a typed object).
/// Only page 0 (Basic Latin and neighbours) is decoded; later pages hold other scripts.
pub fn decode_sprite_font(xnb: &Xnb) -> Result<SpriteFont, AssetError> {
    check_reader(xnb, READER)?;
    let mut c = Cur {
        b: &xnb.primary,
        pos: 0,
    };
    let spacing = c.f32()?;
    let line_spacing = c.i32()?;
    let default_char = Some(c.char()?);
    if c.count(1)? == 0 {
        return Err(bad("font has no pages"));
    }
    c.object()?;
    let atlas = decode_texture_body(c.b, &mut c.pos)?;
    c.object()?;
    let n = c.count(16)?;
    let mut src = Vec::with_capacity(n);
    for _ in 0..n {
        let r = [c.i32()?, c.i32()?, c.i32()?, c.i32()?];
        if r.iter().any(|&v| v < 0) {
            return Err(bad("negative glyph rectangle"));
        }
        src.push(r.map(|v| v as u32));
    }
    c.object()?;
    if c.count(16)? != n {
        return Err(bad("cropping list length mismatch"));
    }
    let mut crop = Vec::with_capacity(n);
    for _ in 0..n {
        crop.push([c.i32()?, c.i32()?, c.i32()?, c.i32()?]);
    }
    c.object()?;
    if c.count(1)? != n {
        return Err(bad("char list length mismatch"));
    }
    let mut chars = Vec::with_capacity(n);
    for _ in 0..n {
        chars.push(c.char()?);
    }
    c.object()?;
    if c.count(12)? != n {
        return Err(bad("kerning list length mismatch"));
    }
    let mut kerning = Vec::with_capacity(n);
    for _ in 0..n {
        kerning.push([c.f32()?, c.f32()?, c.f32()?]);
    }
    let glyphs = (0..n)
        .map(|i| Glyph {
            ch: chars[i],
            src: src[i],
            crop: crop[i],
            kerning: kerning[i],
        })
        .collect();
    Ok(SpriteFont {
        atlas,
        glyphs,
        line_spacing,
        spacing,
        default_char,
    })
}

/// Pack `.font` text: header line, then one line per glyph.
pub(crate) fn format_font(f: &SpriteFont) -> String {
    let dc = f
        .default_char
        .map_or("-".to_string(), |c| (c as u32).to_string());
    let mut s = format!("{} {} {}\n", f.line_spacing, f.spacing, dc);
    for g in &f.glyphs {
        let (a, b, k) = (g.src, g.crop, g.kerning);
        s.push_str(&format!(
            "{} {} {} {} {} {} {} {} {} {} {} {}\n",
            g.ch as u32, a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3], k[0], k[1], k[2]
        ));
    }
    s
}

fn parse_tok<T: std::str::FromStr>(t: Option<&str>) -> Result<T, AssetError> {
    t.and_then(|t| t.parse().ok())
        .ok_or_else(|| bad("bad number in .font file"))
}

fn parse_char(t: Option<&str>) -> Result<char, AssetError> {
    // `char::from_u32` rejects lone surrogates (0xD800..=0xDFFF) and values above 0x10FFFF.
    char::from_u32(parse_tok(t)?).ok_or_else(|| bad("invalid char in .font file"))
}

pub(crate) fn parse_font(text: &str, atlas: RgbaImage) -> Result<SpriteFont, AssetError> {
    if text.len() > MAX_FONT_TEXT {
        return Err(bad(".font file too large"));
    }
    let mut lines = text.lines();
    let mut h = lines.next().ok_or(AssetError::Truncated)?.split(' ');
    let line_spacing = parse_tok(h.next())?;
    let spacing = parse_tok(h.next())?;
    let default_char = match h.next() {
        Some("-") => None,
        t => Some(parse_char(t)?),
    };
    let mut glyphs = Vec::new();
    for line in lines.filter(|l| !l.is_empty()) {
        let mut t = line.split(' ');
        let ch = parse_char(t.next())?;
        let src = [
            parse_tok(t.next())?,
            parse_tok(t.next())?,
            parse_tok(t.next())?,
            parse_tok(t.next())?,
        ];
        let crop = [
            parse_tok(t.next())?,
            parse_tok(t.next())?,
            parse_tok(t.next())?,
            parse_tok(t.next())?,
        ];
        let kerning = [
            parse_tok(t.next())?,
            parse_tok(t.next())?,
            parse_tok(t.next())?,
        ];
        glyphs.push(Glyph {
            ch,
            src,
            crop,
            kerning,
        });
    }
    Ok(SpriteFont {
        atlas,
        glyphs,
        line_spacing,
        spacing,
        default_char,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SpriteFont {
        let mut atlas = RgbaImage::new(2, 2);
        atlas.pixels[1] = [1, 2, 3, 4];
        SpriteFont {
            atlas,
            glyphs: vec![
                Glyph {
                    ch: 'A',
                    src: [0, 0, 1, 2],
                    crop: [0, -1, 1, 3],
                    kerning: [0.5, 7.25, -1.0],
                },
                Glyph {
                    ch: '\u{e9}',
                    src: [1, 0, 1, 2],
                    crop: [1, 1, 2, 2],
                    kerning: [0.0, 3.0, 0.125],
                },
            ],
            line_spacing: 20,
            spacing: -2.5,
            default_char: Some('?'),
        }
    }

    #[test]
    fn font_roundtrip_pack_format() {
        let f = sample();
        let back = parse_font(&format_font(&f), f.atlas.clone()).unwrap();
        assert_eq!(f, back);
        let g = SpriteFont {
            default_char: None,
            ..f
        };
        assert_eq!(g, parse_font(&format_font(&g), g.atlas.clone()).unwrap());
        assert_eq!(g.glyph('A').unwrap().src, [0, 0, 1, 2]);
        assert!(g.glyph('Z').is_none());
    }

    fn obj(v: &mut Vec<u8>, id: u8, count: u32) {
        v.push(id);
        v.extend(count.to_le_bytes());
    }

    #[test]
    fn font_decodes_synthetic_page() {
        let mut p = Vec::new();
        p.extend(1.5f32.to_le_bytes());
        p.extend(29i32.to_le_bytes());
        p.push(b'*');
        p.extend(7u32.to_le_bytes());
        p.push(2); // Texture2D: Color, 1x1, 1 mip, 4 bytes
        for v in [0u32, 1, 1, 1, 4] {
            p.extend(v.to_le_bytes());
        }
        p.extend([255, 0, 0, 255]);
        obj(&mut p, 3, 1);
        for v in [0i32, 0, 1, 1] {
            p.extend(v.to_le_bytes());
        }
        obj(&mut p, 3, 1);
        for v in [0i32, 0, 1, 1] {
            p.extend(v.to_le_bytes());
        }
        obj(&mut p, 5, 1);
        p.push(b'A');
        obj(&mut p, 7, 1);
        for v in [1f32, 2.0, 3.0] {
            p.extend(v.to_le_bytes());
        }
        let x = Xnb {
            readers: vec![format!("{READER}, ReLogic")],
            primary: p,
        };
        let f = decode_sprite_font(&x).unwrap();
        assert_eq!(
            (f.line_spacing, f.spacing, f.default_char),
            (29, 1.5, Some('*'))
        );
        assert_eq!(f.glyph('A').unwrap().kerning, [1.0, 2.0, 3.0]);
        assert_eq!(f.atlas.get(0, 0), [255, 0, 0, 255]);
    }

    #[test]
    fn font_truncated_is_error() {
        let x = Xnb {
            readers: vec![format!("{READER}, Microsoft.Xna.Framework")],
            primary: vec![0, 0, 0, 0, 29, 0, 0, 0, 42, 5, 0],
        };
        assert!(decode_sprite_font(&x).is_err());
        assert!(parse_font("20 1.0", RgbaImage::new(1, 1)).is_err());
        assert!(parse_font("20 1.0 -\n65 0 0 1", RgbaImage::new(1, 1)).is_err());
    }

    fn header(pages: u32) -> Vec<u8> {
        let mut p = Vec::new();
        p.extend(1.5f32.to_le_bytes());
        p.extend(29i32.to_le_bytes());
        p.push(b'*');
        p.extend(pages.to_le_bytes());
        p
    }

    /// Valid 1-glyph page body after the header; lengths of crop/char/kerning lists are tunable.
    fn page(crop_n: u32, char_n: u32, kern_n: u32, ch: &[u8]) -> Vec<u8> {
        let mut p = header(1);
        p.push(2);
        for v in [0u32, 1, 1, 1, 4] {
            p.extend(v.to_le_bytes());
        }
        p.extend([255, 0, 0, 255]);
        obj(&mut p, 3, 1);
        p.extend([0u8; 16]);
        obj(&mut p, 3, crop_n);
        p.extend(vec![0u8; 16 * crop_n.min(8) as usize]);
        obj(&mut p, 5, char_n);
        p.extend(ch);
        obj(&mut p, 7, kern_n);
        p.extend(vec![0u8; 12 * kern_n.min(8) as usize]);
        p
    }

    fn xnb(primary: Vec<u8>) -> Xnb {
        Xnb {
            readers: vec![format!("{READER}, ReLogic")],
            primary,
        }
    }

    #[test]
    fn font_page_helper_is_valid() {
        assert!(decode_sprite_font(&xnb(page(1, 1, 1, b"A"))).is_ok());
    }

    #[test]
    fn font_list_errors() {
        for (c, ch, k) in [(2, 1, 1), (1, 2, 1), (1, 1, 2), (0, 1, 1)] {
            let n = if ch == 2 { b"AB".as_slice() } else { b"A" };
            assert!(decode_sprite_font(&xnb(page(c, ch, k, n))).is_err());
        }
        // Count over the cap, and count larger than the remaining bytes.
        assert!(decode_sprite_font(&xnb(page(1, 1, u32::MAX, b"A"))).is_err());
        let mut p = header(1);
        p.push(2);
        p.extend([0u8; 20]);
        assert!(decode_sprite_font(&xnb(p)).is_err());
        assert!(decode_sprite_font(&xnb(header(u32::MAX))).is_err());
        assert!(decode_sprite_font(&xnb(header(0))).is_err());
    }

    #[test]
    fn font_bad_char_encodings_are_errors() {
        for bad in [
            &[0xffu8][..],
            &[0xc0, 0x80],
            &[0xed, 0xa0, 0x80],
            &[0xe2, 0x82],
        ] {
            assert!(decode_sprite_font(&xnb(page(1, 1, 1, bad))).is_err());
        }
    }

    #[test]
    fn parse_font_rejects_surrogate_and_oversize() {
        let img = RgbaImage::new(1, 1);
        let line = "20 1 -\n55296 0 0 1 1 0 0 1 1 0 1 0\n";
        assert!(parse_font(line, img.clone()).is_err());
        assert!(parse_font(&" ".repeat(MAX_FONT_TEXT + 1), img).is_err());
    }
}
