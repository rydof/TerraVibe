//! Index-only reader for XACT3 wave banks (`.xwb`, little-endian "WBND").
//!
//! Entry `offset` values are absolute file offsets of the wave data.

use crate::AssetError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaveFormat {
    Pcm {
        channels: u16,
        rate: u32,
        bits: u16,
    },
    Xma,
    Adpcm {
        channels: u16,
        rate: u32,
        block_align: u16,
    },
    Wma,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaveBankEntry {
    pub format: WaveFormat,
    /// Absolute offset in the file of this entry's wave data.
    pub offset: u32,
    pub length: u32,
}

fn u32_at(b: &[u8], pos: usize) -> Result<u32, AssetError> {
    let end = pos.checked_add(4).ok_or(AssetError::Truncated)?;
    let s = b.get(pos..end).ok_or(AssetError::Truncated)?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Returns (start, len) of a segment, validated against the file size.
fn segment(b: &[u8], pos: usize, file_len: usize) -> Result<(usize, usize), AssetError> {
    let off = u32_at(b, pos)? as usize;
    let len = u32_at(b, pos + 4)? as usize;
    let end = off.checked_add(len).ok_or(AssetError::Truncated)?;
    if end > file_len {
        return Err(AssetError::Malformed(format!(
            "wave bank segment {off}+{len} exceeds file size {file_len}"
        )));
    }
    Ok((off, len))
}

fn decode_mini(v: u32) -> WaveFormat {
    let tag = v & 0x3;
    let channels = ((v >> 2) & 0x7) as u16;
    let rate = (v >> 5) & 0x3_FFFF;
    let align = ((v >> 23) & 0xFF) as u16;
    let wide = (v >> 31) & 1;
    match tag {
        0 => WaveFormat::Pcm {
            channels,
            rate,
            bits: if wide == 1 { 16 } else { 8 },
        },
        1 => WaveFormat::Xma,
        2 => WaveFormat::Adpcm {
            channels,
            rate,
            block_align: (align + 22) * channels.max(1),
        },
        _ => WaveFormat::Wma,
    }
}

pub fn read_wave_bank_index(bytes: &[u8]) -> Result<Vec<WaveBankEntry>, AssetError> {
    read_index_internal(bytes, bytes.len())
}

pub fn read_wave_bank_index_from_reader<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    file_len: u64,
) -> Result<Vec<WaveBankEntry>, AssetError> {
    use std::io::SeekFrom;
    reader.seek(SeekFrom::Start(0))?;
    let mut header = [0u8; 64];
    reader.read_exact(&mut header)?;

    match header.get(0..4) {
        Some(b"WBND") => {}
        Some(b"DNBW") => {
            return Err(AssetError::UnsupportedFormat("big-endian wave bank".into()));
        }
        Some(_) => return Err(AssetError::BadMagic),
        None => return Err(AssetError::Truncated),
    }

    let version = u32_at(&header, 4)?;
    let mut pos = 8;
    if version >= 42 {
        pos += 4;
    }
    let nseg = if version <= 3 { 4 } else { 5 };
    let mut segs = [(0usize, 0usize); 5];
    for (i, s) in segs.iter_mut().enumerate().take(nseg) {
        *s = segment(&header, pos + i * 8, file_len as usize)?;
    }
    let (bd_off, bd_len) = segs[0];
    let (md_off, md_len) = segs[1];

    let needed = (bd_off + bd_len).max(md_off + md_len);
    if needed > file_len as usize {
        return Err(AssetError::Truncated);
    }
    let mut buf = vec![0u8; needed];
    reader.seek(SeekFrom::Start(0))?;
    reader.read_exact(&mut buf)?;

    read_index_internal(&buf, file_len as usize)
}

fn read_index_internal(bytes: &[u8], file_len: usize) -> Result<Vec<WaveBankEntry>, AssetError> {
    match bytes.get(0..4) {
        Some(b"WBND") => {}
        Some(b"DNBW") => {
            return Err(AssetError::UnsupportedFormat("big-endian wave bank".into()));
        }
        Some(_) => return Err(AssetError::BadMagic),
        None => return Err(AssetError::Truncated),
    }
    let version = u32_at(bytes, 4)?;
    let mut pos = 8;
    if version >= 42 {
        pos += 4; // header version
    }
    let nseg = if version <= 3 { 4 } else { 5 };
    let mut segs = [(0usize, 0usize); 5];
    for (i, s) in segs.iter_mut().enumerate().take(nseg) {
        *s = segment(bytes, pos + i * 8, file_len)?;
    }
    let (bd_off, bd_len) = segs[0];
    let (md_off, md_len) = segs[1];
    let (wd_off, wd_len) = segs[nseg - 1];

    if bd_len < 96 || bytes.len() < bd_off + bd_len {
        return Err(AssetError::Truncated);
    }
    let flags = u32_at(bytes, bd_off)?;
    let count = u32_at(bytes, bd_off + 4)? as usize;
    let elem = u32_at(bytes, bd_off + 72)? as usize;
    let alignment = u32_at(bytes, bd_off + 80)? as usize;
    let compact_fmt = u32_at(bytes, bd_off + 84)?;
    let compact = flags & 0x0002_0000 != 0;

    let elem = if compact { 4 } else { elem };
    if !compact && elem < 24 {
        return Err(AssetError::Malformed(format!(
            "wave bank entry size {elem} too small"
        )));
    }
    // Bound the entry count by what the metadata segment can hold.
    if count.checked_mul(elem).is_none_or(|n| n > md_len) || bytes.len() < md_off + md_len {
        return Err(AssetError::Malformed(format!(
            "wave bank claims {count} entries, metadata segment is {md_len} bytes"
        )));
    }

    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let p = md_off + i * elem;
        let (format, start, length) = if compact {
            if alignment == 0 {
                return Err(AssetError::Malformed(
                    "compact wave bank with zero alignment".into(),
                ));
            }
            let word = u32_at(bytes, p)?;
            let start = ((word & 0x1F_FFFF) as usize)
                .checked_mul(alignment)
                .ok_or(AssetError::Truncated)?;
            let deviation = (word >> 21) as usize;
            let next = if i + 1 < count {
                ((u32_at(bytes, p + 4)? & 0x1F_FFFF) as usize)
                    .checked_mul(alignment)
                    .ok_or(AssetError::Truncated)?
            } else {
                wd_len
            };
            let length = next
                .checked_sub(start)
                .and_then(|l| l.checked_sub(deviation))
                .ok_or_else(|| {
                    AssetError::Malformed("compact wave entries not ascending".into())
                })?;
            (decode_mini(compact_fmt), start, length)
        } else {
            (
                decode_mini(u32_at(bytes, p + 4)?),
                u32_at(bytes, p + 8)? as usize,
                u32_at(bytes, p + 12)? as usize,
            )
        };
        let end = start.checked_add(length).ok_or(AssetError::Truncated)?;
        if end > wd_len {
            return Err(AssetError::Malformed(format!(
                "wave entry {i} ({start}+{length}) outside wave data ({wd_len})"
            )));
        }
        out.push(WaveBankEntry {
            format,
            offset: u32::try_from(wd_off + start).map_err(|_| AssetError::Truncated)?,
            length: u32::try_from(length).map_err(|_| AssetError::Truncated)?,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le(v: &mut Vec<u8>, x: u32) {
        v.extend_from_slice(&x.to_le_bytes());
    }

    /// Builds a version-44 bank: header, bank data, metadata, wave data.
    fn bank(
        flags: u32,
        count: u32,
        elem: u32,
        align: u32,
        cfmt: u32,
        meta: &[u8],
        wave: usize,
    ) -> Vec<u8> {
        let (bd, md) = (52u32, 52 + 96);
        let wd = md + meta.len() as u32;
        let mut v = b"WBND".to_vec();
        for x in [
            44,
            1,
            bd,
            96,
            md,
            meta.len() as u32,
            0,
            0,
            0,
            0,
            wd,
            wave as u32,
        ] {
            le(&mut v, x);
        }
        // segments: bankdata, metadata, seektables, names, wavedata
        v.truncate(12);
        for (o, l) in [
            (bd, 96),
            (md, meta.len() as u32),
            (0, 0),
            (0, 0),
            (wd, wave as u32),
        ] {
            le(&mut v, o);
            le(&mut v, l);
        }
        let mut b = vec![0u8; 96];
        b[0..4].copy_from_slice(&flags.to_le_bytes());
        b[4..8].copy_from_slice(&count.to_le_bytes());
        b[72..76].copy_from_slice(&elem.to_le_bytes());
        b[80..84].copy_from_slice(&align.to_le_bytes());
        b[84..88].copy_from_slice(&cfmt.to_le_bytes());
        v.extend(b);
        v.extend_from_slice(meta);
        v.resize(v.len() + wave, 0);
        v
    }

    #[test]
    fn decodes_pcm_and_adpcm_entries() {
        let pcm = (1 << 2) | (22050 << 5) | (1 << 31); // tag 0, mono, 16-bit
        let adpcm = 2 | (2 << 2) | (44100 << 5) | (48 << 23);
        let mut m = Vec::new();
        for (f, off, len) in [(pcm, 0, 10), (adpcm, 10, 20)] {
            for x in [0, f, off, len, 0, 0] {
                le(&mut m, x);
            }
        }
        let e = read_wave_bank_index(&bank(0, 2, 24, 1, 0, &m, 40)).unwrap();
        assert_eq!(
            e[0].format,
            WaveFormat::Pcm {
                channels: 1,
                rate: 22050,
                bits: 16
            }
        );
        assert_eq!(
            e[1].format,
            WaveFormat::Adpcm {
                channels: 2,
                rate: 44100,
                block_align: 140
            }
        );
        assert_eq!((e[1].length, e[1].offset - e[0].offset), (20, 10));
    }

    #[test]
    fn decodes_compact_with_deviation() {
        let cf = 2 | (1 << 2) | (32000 << 5);
        let mut m = Vec::new();
        le(&mut m, 0);
        le(&mut m, 4 | (3 << 21)); // offset 4*16, deviation 3
        let e = read_wave_bank_index(&bank(0x20000, 2, 4, 16, cf, &m, 100)).unwrap();
        assert_eq!(e[0].length, 64);
        assert_eq!(e[1].length, 100 - 64 - 3);
        assert_eq!(e[1].offset - e[0].offset, 64);
        let r = read_wave_bank_index(&bank(0x20000, 2, 4, 0, cf, &m, 100));
        assert!(matches!(r, Err(AssetError::Malformed(_))));
    }

    #[test]
    fn rejects_count_beyond_metadata() {
        let r = read_wave_bank_index(&bank(0, 1_000_000, 24, 1, 0, &[0; 24], 8));
        assert!(matches!(r, Err(AssetError::Malformed(_))));
    }

    #[test]
    fn rejects_bad_magic_and_truncation() {
        assert!(matches!(
            read_wave_bank_index(b"XXXXabcdefgh"),
            Err(AssetError::BadMagic)
        ));
        assert!(matches!(
            read_wave_bank_index(b"WBND\x2b\0\0\0"),
            Err(AssetError::Truncated)
        ));
        assert!(matches!(
            read_wave_bank_index(b"DNBW"),
            Err(AssetError::UnsupportedFormat(_))
        ));
    }
}
