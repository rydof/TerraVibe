use crate::texture::check_reader;
use crate::{AssetError, Xnb};

const READER: &str = "Microsoft.Xna.Framework.Content.SoundEffectReader";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundEffect {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub data: Vec<u8>,
}

impl SoundEffect {
    pub fn to_wav(&self) -> Vec<u8> {
        let block_align = self.channels as u32 * self.bits_per_sample as u32 / 8;
        let byte_rate = self.sample_rate.wrapping_mul(block_align);
        let len = self.data.len() as u32;
        let mut w = Vec::with_capacity(44 + self.data.len());
        w.extend(b"RIFF");
        w.extend((36 + len).to_le_bytes());
        w.extend(b"WAVEfmt ");
        w.extend(16u32.to_le_bytes());
        w.extend(1u16.to_le_bytes());
        w.extend(self.channels.to_le_bytes());
        w.extend(self.sample_rate.to_le_bytes());
        w.extend(byte_rate.to_le_bytes());
        w.extend((block_align as u16).to_le_bytes());
        w.extend(self.bits_per_sample.to_le_bytes());
        w.extend(b"data");
        w.extend(len.to_le_bytes());
        w.extend(&self.data);
        w
    }
}

fn take<'a>(b: &'a [u8], pos: &mut usize, n: usize) -> Result<&'a [u8], AssetError> {
    let end = pos.checked_add(n).ok_or(AssetError::Truncated)?;
    let s = b.get(*pos..end).ok_or(AssetError::Truncated)?;
    *pos = end;
    Ok(s)
}

fn u32_at(b: &[u8], pos: &mut usize) -> Result<u32, AssetError> {
    Ok(u32::from_le_bytes(take(b, pos, 4)?.try_into().unwrap()))
}

pub fn decode_sound(xnb: &Xnb) -> Result<SoundEffect, AssetError> {
    check_reader(xnb, READER)?;
    let b = &xnb.primary;
    let mut pos = 0;
    let fmt_size = u32_at(b, &mut pos)? as usize;
    if fmt_size < 16 {
        return Err(AssetError::Malformed(format!(
            "fmt chunk too small: {fmt_size}"
        )));
    }
    let fmt = take(b, &mut pos, fmt_size)?;
    let tag = u16::from_le_bytes([fmt[0], fmt[1]]);
    if tag != 1 {
        return Err(AssetError::UnsupportedFormat(format!(
            "wave format tag {tag}"
        )));
    }
    let channels = u16::from_le_bytes([fmt[2], fmt[3]]);
    let sample_rate = u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]);
    let bits_per_sample = u16::from_le_bytes([fmt[14], fmt[15]]);
    let size = u32_at(b, &mut pos)? as usize;
    let data = take(b, &mut pos, size)?.to_vec();
    Ok(SoundEffect {
        sample_rate,
        channels,
        bits_per_sample,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header() {
        let s = SoundEffect {
            sample_rate: 44100,
            channels: 1,
            bits_per_sample: 16,
            data: vec![0; 10],
        };
        let w = s.to_wav();
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..12], b"WAVE");
        assert_eq!(u32::from_le_bytes(w[24..28].try_into().unwrap()), 44100);
        assert_eq!(w.len(), 44 + 10);
    }

    fn snd(primary: Vec<u8>) -> Xnb {
        Xnb {
            readers: vec![READER.to_string()],
            primary,
        }
    }

    #[test]
    fn truncated_is_error() {
        assert!(decode_sound(&snd(vec![0xFF; 4])).is_err());
        let mut p = 18u32.to_le_bytes().to_vec();
        p.extend([0u8; 18]);
        p.extend(u32::MAX.to_le_bytes());
        assert!(decode_sound(&snd(p)).is_err());
    }

    #[test]
    fn non_pcm_is_error() {
        let mut p = 16u32.to_le_bytes().to_vec();
        p.extend(2u16.to_le_bytes());
        p.extend([0u8; 14]);
        assert!(matches!(
            decode_sound(&snd(p)),
            Err(AssetError::UnsupportedFormat(_))
        ));
    }
}
