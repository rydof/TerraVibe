use crate::AssetError;

#[derive(Debug, Clone)]
pub struct Xnb {
    pub readers: Vec<String>,
    /// Bytes after the primary object's 7-bit type id.
    pub primary: Vec<u8>,
}

pub(crate) fn read_7bit(buf: &[u8], pos: &mut usize) -> Result<u32, AssetError> {
    let mut v = 0u32;
    let mut shift = 0;
    loop {
        let b = *buf.get(*pos).ok_or(AssetError::Truncated)?;
        *pos += 1;
        let bits = u32::from(b & 0x7F);
        if shift == 28 && bits > 0x0F {
            return Err(AssetError::Malformed("7-bit value overflows u32".into()));
        }
        v |= bits << shift;
        if b & 0x80 == 0 {
            return Ok(v);
        }
        shift += 7;
        if shift > 28 {
            return Err(AssetError::Malformed("over-long 7-bit value".into()));
        }
    }
}

fn be_u16(buf: &[u8], pos: usize) -> Result<usize, AssetError> {
    let s = buf.get(pos..pos + 2).ok_or(AssetError::Truncated)?;
    Ok(usize::from(u16::from_be_bytes([s[0], s[1]])))
}

fn le_u32(buf: &[u8], pos: usize) -> Result<u32, AssetError> {
    let s = buf.get(pos..pos + 4).ok_or(AssetError::Truncated)?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn decompress(buf: &[u8], file_len: usize, expected: usize) -> Result<Vec<u8>, AssetError> {
    let mut pos = 14;
    let mut out: Vec<u8> = Vec::new();
    let mut lzx = lzxd::Lzxd::new(lzxd::WindowSize::KB64);
    while pos < file_len {
        let (frame_size, block_size);
        if buf[pos] == 0xFF {
            frame_size = be_u16(buf, pos + 1)?;
            block_size = be_u16(buf, pos + 3)?;
            pos += 5;
        } else {
            frame_size = 0x8000;
            block_size = be_u16(buf, pos)?;
            pos += 2;
        }
        if block_size == 0 || frame_size == 0 {
            break;
        }
        if frame_size > 0x8000 {
            return Err(AssetError::Decompress(format!(
                "frame size {frame_size} exceeds 32768"
            )));
        }
        let block = buf
            .get(pos..pos + block_size)
            .ok_or(AssetError::Truncated)?;
        // lzxd has internal asserts/unchecked indexing reachable from crafted data.
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            lzx.decompress_next(block, frame_size)
                .map(|d| d.to_vec())
                .map_err(|e| e.to_string())
        }));
        match res {
            Ok(Ok(data)) => out.extend_from_slice(&data),
            Ok(Err(e)) => return Err(AssetError::Decompress(e)),
            Err(_) => {
                return Err(AssetError::Decompress(
                    "decoder panicked on malformed data".into(),
                ));
            }
        }
        if out.len() > expected {
            return Err(AssetError::Decompress(format!(
                "decompressed more than the declared {expected} bytes"
            )));
        }
        pos += block_size;
    }
    if out.len() != expected {
        return Err(AssetError::Decompress(format!(
            "decompressed {} bytes, expected {expected}",
            out.len()
        )));
    }
    Ok(out)
}

pub fn read_xnb(bytes: &[u8]) -> Result<Xnb, AssetError> {
    if bytes.len() < 4 {
        return Err(AssetError::Truncated);
    }
    if &bytes[..4] != b"XNBw" {
        return Err(AssetError::BadMagic);
    }
    if bytes.len() < 10 {
        return Err(AssetError::Truncated);
    }
    if bytes[4] != 5 {
        return Err(AssetError::UnsupportedVersion(bytes[4]));
    }
    let compressed = bytes[5] & 0x80 != 0;
    let file_len = le_u32(bytes, 6)? as usize;
    if file_len > bytes.len() {
        return Err(AssetError::Truncated);
    }
    let buf = &bytes[..file_len];
    let owned;
    let (body, mut pos): (&[u8], usize) = if compressed {
        let size = le_u32(buf, 10)? as usize;
        owned = decompress(buf, file_len, size)?;
        (&owned, 0)
    } else {
        (buf, 10)
    };

    let count = read_7bit(body, &mut pos)?;
    let mut readers = Vec::new();
    for _ in 0..count {
        let len = read_7bit(body, &mut pos)? as usize;
        let end = pos.checked_add(len).ok_or(AssetError::Truncated)?;
        let name = body.get(pos..end).ok_or(AssetError::Truncated)?;
        readers.push(String::from_utf8_lossy(name).into_owned());
        pos = end;
        if body.len() < pos + 4 {
            return Err(AssetError::Truncated);
        }
        pos += 4; // reader version
    }
    if read_7bit(body, &mut pos)? != 0 {
        return Err(AssetError::UnsupportedFormat(
            "shared resources are not supported".into(),
        ));
    }
    read_7bit(body, &mut pos)?; // primary type id
    Ok(Xnb {
        readers,
        primary: body.get(pos..).ok_or(AssetError::Truncated)?.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synth(payload: &[u8]) -> Vec<u8> {
        let mut body = vec![1u8, 1, b'X', 0, 0, 0, 0, 0, 1];
        body.extend_from_slice(payload);
        let mut b = b"XNBw".to_vec();
        b.push(5);
        b.push(0);
        b.extend_from_slice(&((10 + body.len()) as u32).to_le_bytes());
        b.extend(body);
        b
    }

    #[test]
    fn parses_uncompressed() {
        let x = read_xnb(&synth(&[1, 2, 3])).unwrap();
        assert_eq!(x.readers, vec!["X".to_string()]);
        assert_eq!(x.primary, vec![1, 2, 3]);
    }
    #[test]
    fn truncated_is_error() {
        let b = synth(&[1, 2, 3]);
        assert!(matches!(read_xnb(&b[..8]), Err(AssetError::Truncated)));
    }
    #[test]
    fn bad_magic_is_error() {
        let mut b = synth(&[]);
        b[0] = b'Q';
        assert!(matches!(read_xnb(&b), Err(AssetError::BadMagic)));
    }
    #[test]
    fn truncated_past_header_is_error() {
        let b = synth(&[1, 2, 3]);
        // Header intact (>=10 bytes) but body cut; file_len still claims full size.
        assert!(matches!(read_xnb(&b[..12]), Err(AssetError::Truncated)));
        // file_len consistent with cut, body ends mid reader table.
        let mut c = b[..12].to_vec();
        c[6..10].copy_from_slice(&12u32.to_le_bytes());
        assert!(read_xnb(&c).is_err());
    }
    fn compressed(file_len: u32, size: u32, payload: &[u8]) -> Vec<u8> {
        let mut b = b"XNBw".to_vec();
        b.extend_from_slice(&[5, 0x80]);
        b.extend_from_slice(&file_len.to_le_bytes());
        b.extend_from_slice(&size.to_le_bytes());
        b.extend_from_slice(payload);
        b
    }
    #[test]
    fn garbage_lzx_is_error_not_panic() {
        for seed in 0u32..200 {
            let mut x = seed.wrapping_mul(2654435761).wrapping_add(1);
            let payload: Vec<u8> = (0..64)
                .map(|_| {
                    x ^= x << 13;
                    x ^= x >> 17;
                    x ^= x << 5;
                    x as u8
                })
                .collect();
            let mut p = vec![0xFF, 0x80, 0x00, 0x00, 64];
            p.extend(&payload);
            let b = compressed(14 + p.len() as u32, 4096, &p);
            assert!(read_xnb(&b).is_err());
        }
        // Block of zeros / ones too.
        for fill in [0u8, 0xFF] {
            let mut p = vec![0xFF, 0x80, 0x00, 0x00, 32];
            p.extend(std::iter::repeat_n(fill, 32));
            let b = compressed(14 + p.len() as u32, 4096, &p);
            assert!(matches!(read_xnb(&b), Err(AssetError::Decompress(_))));
        }
    }
    #[test]
    fn oversized_frame_size_is_error() {
        let p = [0xFF, 0xFF, 0xFF, 0x00, 4, 1, 2, 3, 4];
        let b = compressed(14 + p.len() as u32, 4096, &p);
        assert!(matches!(read_xnb(&b), Err(AssetError::Decompress(_))));
    }
    #[test]
    fn huge_decompressed_size_is_error() {
        let b = compressed(14, u32::MAX, &[]);
        assert!(matches!(read_xnb(&b), Err(AssetError::Decompress(_))));
    }
    #[test]
    fn bogus_file_len_is_error() {
        let mut b = synth(&[1]);
        b[6..10].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(read_xnb(&b), Err(AssetError::Truncated)));
        b[6..10].copy_from_slice(&0u32.to_le_bytes());
        assert!(read_xnb(&b).is_err());
    }
    #[test]
    fn oversized_reader_name_len_is_error() {
        let mut body = vec![1u8, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F, b'X'];
        body.extend_from_slice(&[0; 8]);
        let mut b = b"XNBw".to_vec();
        b.extend_from_slice(&[5, 0]);
        b.extend_from_slice(&((10 + body.len()) as u32).to_le_bytes());
        b.extend(body);
        assert!(matches!(read_xnb(&b), Err(AssetError::Truncated)));
    }
    #[test]
    fn seven_bit_truncated_and_overlong() {
        let mut p = 0;
        assert!(matches!(
            read_7bit(&[0x80], &mut p),
            Err(AssetError::Truncated)
        ));
        let mut p = 0;
        assert!(matches!(
            read_7bit(&[0x80, 0x80, 0x80, 0x80, 0x80, 0x01], &mut p),
            Err(AssetError::Malformed(_))
        ));
        let mut p = 0;
        assert!(matches!(
            read_7bit(&[0xFF, 0xFF, 0xFF, 0xFF, 0x1F], &mut p),
            Err(AssetError::Malformed(_))
        ));
    }
    #[test]
    fn seven_bit_multi_byte() {
        let mut p = 0;
        assert_eq!(read_7bit(&[0xAC, 0x02], &mut p).unwrap(), 300);
        assert_eq!(p, 2);
    }
}
