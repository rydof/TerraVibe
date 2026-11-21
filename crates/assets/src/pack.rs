//! Asset pack: a directory of RGBA8 PNG and PCM WAV files plus a `pack.txt` marker.

use std::path::{Path, PathBuf};

use crate::font::format_font;
use crate::{
    AssetError, RgbaImage, SoundEffect, TerrariaContent, decode_sound, decode_sprite_font,
    decode_texture, read_xnb,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PackStats {
    pub textures: usize,
    pub sounds: usize,
    pub fonts: usize,
    pub music: usize,
    pub skipped: usize,
}

const MAX_PNG_BYTES: usize = 64 << 20;

fn bad(e: impl std::fmt::Display) -> AssetError {
    AssetError::Malformed(e.to_string())
}

pub(crate) fn write_png(img: &RgbaImage) -> Result<Vec<u8>, AssetError> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, img.width, img.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().map_err(bad)?;
        let flat: Vec<u8> = img.pixels.iter().flatten().copied().collect();
        w.write_image_data(&flat).map_err(bad)?;
        w.finish().map_err(bad)?;
    }
    Ok(out)
}

/// Accepts only 8-bit RGBA PNGs.
pub(crate) fn read_png(bytes: &[u8]) -> Result<RgbaImage, AssetError> {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .map_err(bad)?;
    let info = reader.info();
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err(AssetError::UnsupportedFormat(format!(
            "pack PNG must be RGBA 8-bit, found {:?} {:?}",
            info.color_type, info.bit_depth
        )));
    }
    let (width, height) = (info.width, info.height);
    // Never allocate from an untrusted IHDR without a bound.
    let size = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .filter(|&n| n <= MAX_PNG_BYTES)
        .ok_or_else(|| bad("PNG dimensions too large"))?;
    let mut buf = vec![0u8; size];
    let frame = reader.next_frame(&mut buf).map_err(bad)?;
    let data = buf
        .get(..frame.buffer_size())
        .ok_or(AssetError::Truncated)?;
    if data.len() != width as usize * height as usize * 4 {
        return Err(bad("PNG size mismatch"));
    }
    let pixels = data.as_chunks::<4>().0.to_vec();
    Ok(RgbaImage {
        width,
        height,
        pixels,
    })
}

fn le16(b: &[u8]) -> u16 {
    u16::from_le_bytes([b[0], b[1]])
}
fn le32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// Accepts only RIFF/WAVE with a PCM `fmt ` chunk and a `data` chunk.
pub(crate) fn parse_wav(b: &[u8]) -> Result<SoundEffect, AssetError> {
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return Err(bad("not a RIFF/WAVE file"));
    }
    let mut pos = 12usize;
    let mut fmt: Option<(u32, u16, u16)> = None;
    let mut data: Option<&[u8]> = None;
    while pos + 8 <= b.len() {
        let id = &b[pos..pos + 4];
        let size = le32(&b[pos + 4..pos + 8]) as usize;
        let start = pos + 8;
        let end = start.checked_add(size).ok_or(AssetError::Truncated)?;
        let body = b.get(start..end).ok_or(AssetError::Truncated)?;
        match id {
            b"fmt " => {
                if body.len() < 16 {
                    return Err(AssetError::Truncated);
                }
                if le16(body) != 1 {
                    return Err(AssetError::UnsupportedFormat(format!(
                        "WAV format tag {} is not PCM",
                        le16(body)
                    )));
                }
                fmt = Some((le32(&body[4..]), le16(&body[2..]), le16(&body[14..])));
            }
            b"data" => data = Some(body),
            _ => {}
        }
        pos = end.saturating_add(size & 1);
    }
    let (sample_rate, channels, bits_per_sample) =
        fmt.ok_or_else(|| bad("WAV has no fmt chunk"))?;
    let data = data.ok_or_else(|| bad("WAV has no data chunk"))?;
    Ok(SoundEffect {
        sample_rate,
        channels,
        bits_per_sample,
        data: data.to_vec(),
    })
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), AssetError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let ty = entry.file_type()?; // does not follow symlinks
        if ty.is_dir() {
            walk(&path, out)?;
        } else if ty.is_file() && path.extension().is_some_and(|e| e == "xnb") {
            out.push(path);
        }
    }
    Ok(())
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), AssetError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)?;
    Ok(())
}

/// Extract `Images/**`, `Sounds/**` and `Fonts/**` from an install into `out`. Undecodable files are
/// skipped and counted. `pack.txt` is written last.
pub fn extract_pack(src: &TerrariaContent, out: &Path) -> Result<PackStats, AssetError> {
    if src.is_pack() {
        return Err(AssetError::Malformed("source is already a pack".into()));
    }
    let mut stats = PackStats::default();
    std::fs::create_dir_all(out)?;
    // A re-extract must not look valid while it is running.
    match std::fs::remove_file(out.join("pack.txt")) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
        _ => {}
    }
    for (sub, is_image) in [("Images", true), ("Sounds", false)] {
        let mut files = Vec::new();
        let base = src.root().join(sub);
        if base.is_dir() {
            walk(&base, &mut files)?;
        }
        files.sort();
        for file in files {
            let rel = file.strip_prefix(src.root()).map_err(bad)?;
            let decoded = std::fs::read(&file)
                .map_err(AssetError::from)
                .and_then(|b| read_xnb(&b))
                .and_then(|x| {
                    if is_image {
                        write_png(&decode_texture(&x)?)
                    } else {
                        Ok(decode_sound(&x)?.to_wav())
                    }
                });
            match decoded {
                Ok(bytes) => {
                    let ext = if is_image { "png" } else { "wav" };
                    write_file(&out.join(rel).with_extension(ext), &bytes)?;
                    if is_image {
                        stats.textures += 1;
                    } else {
                        stats.sounds += 1;
                    }
                }
                Err(_) => stats.skipped += 1,
            }
        }
    }
    let mut fonts = Vec::new();
    let base = src.root().join("Fonts");
    if base.is_dir() {
        walk(&base, &mut fonts)?;
    }
    fonts.sort();
    for file in fonts {
        let rel = file.strip_prefix(src.root()).map_err(bad)?;
        let decoded = std::fs::read(&file)
            .map_err(AssetError::from)
            .and_then(|b| decode_sprite_font(&read_xnb(&b)?))
            .and_then(|f| Ok((write_png(&f.atlas)?, format_font(&f))));
        match decoded {
            Ok((png, text)) => {
                write_file(&out.join(rel).with_extension("png"), &png)?;
                write_file(&out.join(rel).with_extension("font"), text.as_bytes())?;
                stats.fonts += 1;
            }
            Err(_) => stats.skipped += 1,
        }
    }
    // Extract music tracks from Wave Bank.xwb if present
    match crate::extract_music(src, out) {
        Ok(n) => stats.music = n,
        Err(_) => stats.skipped += 1,
    }

    // pack.txt is shared, so it carries no local paths.
    write_file(
        &out.join("pack.txt"),
        format!(
            "terravibe asset pack v2\nsource: terraria-install\nmusic: {}\n",
            stats.music
        )
        .as_bytes(),
    )?;
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_roundtrip() {
        let s = SoundEffect {
            sample_rate: 44100,
            channels: 2,
            bits_per_sample: 16,
            data: (0..100u8).collect(),
        };
        assert_eq!(parse_wav(&s.to_wav()).unwrap(), s);
    }

    #[test]
    fn png_roundtrip() {
        let img = RgbaImage {
            width: 3,
            height: 2,
            pixels: vec![
                [255, 0, 0, 255],
                [10, 20, 30, 0],
                [200, 100, 50, 128],
                [0, 0, 0, 0],
                [255, 255, 255, 1],
                [1, 2, 3, 254],
            ],
        };
        assert_eq!(read_png(&write_png(&img).unwrap()).unwrap(), img);
    }

    fn crc32(data: &[u8]) -> u32 {
        let mut c = !0u32;
        for &b in data {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
        !c
    }

    #[test]
    fn forged_huge_ihdr_is_error() {
        let mut png = write_png(&RgbaImage::new(1, 1)).unwrap();
        // IHDR data is bytes 16..29; patch dimensions and fix the CRC (bytes 29..33).
        png[16..20].copy_from_slice(&60000u32.to_be_bytes());
        png[20..24].copy_from_slice(&60000u32.to_be_bytes());
        let crc = crc32(&png[12..29]);
        png[29..33].copy_from_slice(&crc.to_be_bytes());
        let err = read_png(&png).unwrap_err();
        assert!(err.to_string().contains("too large"), "{err}");
    }

    fn sound_xnb() -> Vec<u8> {
        let mut body = Vec::new();
        let name = b"Microsoft.Xna.Framework.Content.SoundEffectReader";
        body.push(1);
        body.push(name.len() as u8);
        body.extend(name);
        body.extend([0u8; 4]);
        body.extend([0u8, 1]);
        body.extend(16u32.to_le_bytes());
        body.extend(1u16.to_le_bytes());
        body.extend(1u16.to_le_bytes());
        body.extend(8000u32.to_le_bytes());
        body.extend(8000u32.to_le_bytes());
        body.extend(1u16.to_le_bytes());
        body.extend(8u16.to_le_bytes());
        body.extend(3u32.to_le_bytes());
        body.extend([1u8, 2, 3]);
        let mut b = b"XNBw".to_vec();
        b.extend([5, 0]);
        b.extend(((10 + body.len()) as u32).to_le_bytes());
        b.extend(body);
        b
    }

    #[test]
    fn extract_pack_on_fake_install() {
        let base = std::env::temp_dir().join(format!("tv_fake_install_{}", std::process::id()));
        let out = std::env::temp_dir().join(format!("tv_fake_out_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(base.join("Images")).unwrap();
        std::fs::create_dir_all(base.join("Sounds/Sub")).unwrap();
        std::fs::write(base.join("Images/Tiles_0.xnb"), b"garbage").unwrap();
        std::fs::write(base.join("Sounds/Sub/Beep.xnb"), sound_xnb()).unwrap();
        let src = TerrariaContent::locate(Some(base.clone())).unwrap();
        let stats = extract_pack(&src, &out).unwrap();
        assert_eq!(
            stats,
            PackStats {
                textures: 0,
                sounds: 1,
                fonts: 0,
                music: 0,
                skipped: 1,
            }
        );
        let pack_txt = std::fs::read_to_string(out.join("pack.txt")).unwrap();
        assert!(pack_txt.starts_with("terravibe asset pack v2\n"));
        assert!(pack_txt.contains("music: 0"));
        assert!(!pack_txt.contains(base.to_str().unwrap()));
        let pack = TerrariaContent::open(Some(out.clone()), None).unwrap();
        assert!(pack.is_pack());
        assert_eq!(pack.sound("Sounds/Sub/Beep").unwrap().data, vec![1, 2, 3]);
        let err = extract_pack(&pack, &out).unwrap_err();
        assert!(err.to_string().contains("source is already a pack"));
        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn corrupt_pack_files_are_errors() {
        assert!(read_png(b"definitely not a png").is_err());
        assert!(read_png(&[]).is_err());
        assert!(parse_wav(b"garbage bytes that are not riff").is_err());
        assert!(parse_wav(&[]).is_err());
        let mut w = SoundEffect {
            sample_rate: 1,
            channels: 1,
            bits_per_sample: 8,
            data: vec![1, 2, 3],
        }
        .to_wav();
        let mut w2 = w.clone();
        w2[40] = 0xff; // data size overruns the file
        assert!(parse_wav(&w2).is_err());
        w[20] = 3; // non-PCM format tag
        assert!(parse_wav(&w).is_err());
    }

    #[test]
    fn pack_v1_without_music_still_opens() {
        let dir = std::env::temp_dir().join(format!("tv_pack_v1_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("pack.txt"),
            b"terravibe asset pack v1\nsource: terraria-install\n",
        )
        .unwrap();

        let pack = TerrariaContent::open(Some(dir.clone()), None).unwrap();
        assert!(pack.is_pack());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
