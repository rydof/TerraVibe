//! Microsoft ADPCM decoder to 16-bit PCM.

use crate::AssetError;

pub const MS_ADPCM_COEFFS: [(i32, i32); 7] = [
    (256, 0),
    (512, -256),
    (0, 0),
    (192, 64),
    (240, 0),
    (460, -208),
    (392, -232),
];

pub const MS_ADPCM_ADAPTATION_TABLE: [i32; 16] = [
    230, 230, 230, 230, 307, 409, 512, 614, 768, 614, 512, 409, 307, 230, 230, 230,
];

#[derive(Debug, Clone, Copy)]
struct ChannelState {
    predictor: usize,
    delta: i32,
    samp1: i32,
    samp2: i32,
}

impl ChannelState {
    #[inline]
    fn step(&mut self, nibble: u8) -> i16 {
        let signed_nibble = if nibble >= 8 {
            (nibble as i32) - 16
        } else {
            nibble as i32
        };
        let (c1, c2) = MS_ADPCM_COEFFS[self.predictor];
        let pred = (self.samp1 * c1 + self.samp2 * c2) / 256;
        let mut sample = pred + signed_nibble * self.delta;
        sample = sample.clamp(-32768, 32767);

        self.delta = (self.delta * MS_ADPCM_ADAPTATION_TABLE[nibble as usize]) / 256;
        if self.delta < 16 {
            self.delta = 16;
        }

        self.samp2 = self.samp1;
        self.samp1 = sample;
        sample as i16
    }
}

pub fn decode_ms_adpcm_block(block: &[u8], channels: u16) -> Result<Vec<i16>, AssetError> {
    match channels {
        1 => {
            if block.len() < 7 {
                return Err(AssetError::Truncated);
            }
            let pred = block[0] as usize;
            if pred >= MS_ADPCM_COEFFS.len() {
                return Err(AssetError::Malformed(format!(
                    "invalid ADPCM predictor {pred}"
                )));
            }
            let delta = i16::from_le_bytes([block[1], block[2]]) as i32;
            let samp1 = i16::from_le_bytes([block[3], block[4]]) as i32;
            let samp2 = i16::from_le_bytes([block[5], block[6]]) as i32;

            let mut state = ChannelState {
                predictor: pred,
                delta,
                samp1,
                samp2,
            };

            let remaining = block.len() - 7;
            let mut out = Vec::with_capacity(2 + remaining * 2);
            out.push(samp2 as i16);
            out.push(samp1 as i16);

            for &b in &block[7..] {
                let n1 = (b >> 4) & 0x0F;
                let n2 = b & 0x0F;
                out.push(state.step(n1));
                out.push(state.step(n2));
            }
            Ok(out)
        }
        2 => {
            if block.len() < 14 {
                return Err(AssetError::Truncated);
            }
            let pred_l = block[0] as usize;
            let pred_r = block[1] as usize;
            if pred_l >= MS_ADPCM_COEFFS.len() || pred_r >= MS_ADPCM_COEFFS.len() {
                return Err(AssetError::Malformed(format!(
                    "invalid ADPCM predictor L:{pred_l} R:{pred_r}"
                )));
            }
            let delta_l = i16::from_le_bytes([block[2], block[3]]) as i32;
            let delta_r = i16::from_le_bytes([block[4], block[5]]) as i32;
            let samp1_l = i16::from_le_bytes([block[6], block[7]]) as i32;
            let samp1_r = i16::from_le_bytes([block[8], block[9]]) as i32;
            let samp2_l = i16::from_le_bytes([block[10], block[11]]) as i32;
            let samp2_r = i16::from_le_bytes([block[12], block[13]]) as i32;

            let mut state_l = ChannelState {
                predictor: pred_l,
                delta: delta_l,
                samp1: samp1_l,
                samp2: samp2_l,
            };
            let mut state_r = ChannelState {
                predictor: pred_r,
                delta: delta_r,
                samp1: samp1_r,
                samp2: samp2_r,
            };

            let remaining = block.len() - 14;
            let mut out = Vec::with_capacity(4 + remaining * 2);
            // In stereo, samples are interleaved: L0, R0, L1, R1, L2, R2...
            out.push(samp2_l as i16);
            out.push(samp2_r as i16);
            out.push(samp1_l as i16);
            out.push(samp1_r as i16);

            for &b in &block[14..] {
                let n_l = (b >> 4) & 0x0F;
                let n_r = b & 0x0F;
                out.push(state_l.step(n_l));
                out.push(state_r.step(n_r));
            }
            Ok(out)
        }
        _ => Err(AssetError::UnsupportedFormat(format!(
            "ADPCM {channels} channels not supported"
        ))),
    }
}

pub fn decode_ms_adpcm(
    data: &[u8],
    channels: u16,
    block_align: u16,
) -> Result<Vec<i16>, AssetError> {
    if block_align == 0 {
        return Err(AssetError::Malformed("zero block_align".into()));
    }
    let align = block_align as usize;
    if data.len() % align != 0 {
        return Err(AssetError::Truncated);
    }
    let num_blocks = data.len() / align;
    let samples_per_block = match channels {
        1 => 2 + (align - 7) * 2,
        2 => 4 + (align - 14) * 2,
        _ => {
            return Err(AssetError::UnsupportedFormat(format!(
                "ADPCM {channels} channels"
            )));
        }
    };
    let mut out = Vec::with_capacity(num_blocks * samples_per_block);
    for chunk in data.chunks_exact(align) {
        let block_samples = decode_ms_adpcm_block(chunk, channels)?;
        out.extend(block_samples);
    }
    Ok(out)
}

pub fn pcm_to_wav(samples: &[i16], channels: u16, sample_rate: u32) -> Vec<u8> {
    let bits_per_sample = 16u16;
    let block_align = channels * bits_per_sample / 8;
    let byte_rate = sample_rate.wrapping_mul(block_align as u32);
    let data_len = (samples.len() * 2) as u32;

    let mut w = Vec::with_capacity(44 + data_len as usize);
    w.extend(b"RIFF");
    w.extend((36 + data_len).to_le_bytes());
    w.extend(b"WAVEfmt ");
    w.extend(16u32.to_le_bytes());
    w.extend(1u16.to_le_bytes()); // PCM format
    w.extend(channels.to_le_bytes());
    w.extend(sample_rate.to_le_bytes());
    w.extend(byte_rate.to_le_bytes());
    w.extend(block_align.to_le_bytes());
    w.extend(bits_per_sample.to_le_bytes());
    w.extend(b"data");
    w.extend(data_len.to_le_bytes());
    for s in samples {
        w.extend(s.to_le_bytes());
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adpcm_block_decodes_to_golden_samples() {
        // Hand-crafted stereo MS-ADPCM block:
        // Predictors: 0, 0
        // Deltas: 16, 16
        // Samp1: 100, -200
        // Samp2: 50, -100
        // 1 data byte: 0x12 -> high nibble 1 (left), low nibble 2 (right)
        let mut block = Vec::new();
        block.push(0); // predictor L
        block.push(0); // predictor R
        block.extend(16i16.to_le_bytes()); // delta L
        block.extend(16i16.to_le_bytes()); // delta R
        block.extend(100i16.to_le_bytes()); // samp1 L
        block.extend((-200i16).to_le_bytes()); // samp1 R
        block.extend(50i16.to_le_bytes()); // samp2 L
        block.extend((-100i16).to_le_bytes()); // samp2 R
        block.push(0x12); // nibble 1 (L), nibble 2 (R)

        let samples = decode_ms_adpcm_block(&block, 2).unwrap();
        // Golden samples:
        // samp2 (50, -100), samp1 (100, -200),
        // next L: 100 + 1 * 16 = 116
        // next R: -200 + 2 * 16 = -168
        assert_eq!(samples, vec![50, -100, 100, -200, 116, -168]);
    }

    #[test]
    fn adpcm_stereo_interleave() {
        let mut block = Vec::new();
        block.push(0);
        block.push(0);
        block.extend(16i16.to_le_bytes());
        block.extend(16i16.to_le_bytes());
        block.extend(0i16.to_le_bytes());
        block.extend(0i16.to_le_bytes());
        block.extend(0i16.to_le_bytes());
        block.extend(0i16.to_le_bytes());
        block.push(0x12);
        block.push(0x34);

        let samples = decode_ms_adpcm_block(&block, 2).unwrap();
        assert_eq!(samples.len(), 8); // 2 initial pairs + 2 nibble pairs
        // L0, R0, L1, R1, L2, R2, L3, R3
        assert_eq!(samples[0], 0);
        assert_eq!(samples[1], 0);
        assert_eq!(samples[2], 0);
        assert_eq!(samples[3], 0);
    }
}
