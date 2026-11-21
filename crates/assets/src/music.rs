//! Music tracks table and extractor from Terraria's Wave Bank.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::adpcm::{decode_ms_adpcm, pcm_to_wav};
use crate::xwb::{WaveFormat, read_wave_bank_index, read_wave_bank_index_from_reader};
use crate::{AssetError, TerrariaContent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MusicTrack {
    pub id: &'static str,
    pub entry: usize,
    pub file_stem: &'static str,
    pub loops: bool,
}

pub const MUSIC_TRACKS: &[MusicTrack] = &[
    MusicTrack {
        id: "night",
        entry: 0,
        file_stem: "night",
        loops: true,
    },
    MusicTrack {
        id: "eerie",
        entry: 1,
        file_stem: "eerie",
        loops: true,
    },
    MusicTrack {
        id: "day",
        entry: 2,
        file_stem: "day",
        loops: true,
    },
    MusicTrack {
        id: "boss1",
        entry: 3,
        file_stem: "boss1",
        loops: true,
    },
    MusicTrack {
        id: "title",
        entry: 4,
        file_stem: "title",
        loops: true,
    },
    MusicTrack {
        id: "jungle",
        entry: 5,
        file_stem: "jungle",
        loops: true,
    },
    MusicTrack {
        id: "corruption",
        entry: 6,
        file_stem: "corruption",
        loops: true,
    },
    MusicTrack {
        id: "underground",
        entry: 11,
        file_stem: "underground",
        loops: true,
    },
    MusicTrack {
        id: "snow",
        entry: 14,
        file_stem: "snow",
        loops: true,
    },
    MusicTrack {
        id: "ice",
        entry: 19,
        file_stem: "ice",
        loops: true,
    },
    MusicTrack {
        id: "desert",
        entry: 20,
        file_stem: "desert",
        loops: true,
    },
    MusicTrack {
        id: "ocean",
        entry: 21,
        file_stem: "ocean",
        loops: true,
    },
    MusicTrack {
        id: "dungeon",
        entry: 22,
        file_stem: "dungeon",
        loops: true,
    },
    MusicTrack {
        id: "underworld",
        entry: 25,
        file_stem: "underworld",
        loops: true,
    },
];

pub fn decode_track_entry(bank_bytes: &[u8], entry_idx: usize) -> Result<Vec<u8>, AssetError> {
    let entries = read_wave_bank_index(bank_bytes)?;
    let entry = entries
        .get(entry_idx)
        .ok_or_else(|| AssetError::Malformed(format!("track entry {entry_idx} out of range")))?;

    let start = entry.offset as usize;
    let end = start
        .checked_add(entry.length as usize)
        .ok_or(AssetError::Truncated)?;
    let entry_data = bank_bytes.get(start..end).ok_or(AssetError::Truncated)?;

    match entry.format {
        WaveFormat::Adpcm {
            channels,
            rate,
            block_align,
        } => {
            let samples = decode_ms_adpcm(entry_data, channels, block_align)?;
            Ok(pcm_to_wav(&samples, channels, rate))
        }
        WaveFormat::Pcm {
            channels,
            rate,
            bits,
        } if bits == 16 => {
            let mut samples = Vec::with_capacity(entry_data.len() / 2);
            for chunk in entry_data.chunks_exact(2) {
                samples.push(i16::from_le_bytes([chunk[0], chunk[1]]));
            }
            Ok(pcm_to_wav(&samples, channels, rate))
        }
        _ => Err(AssetError::UnsupportedFormat(format!(
            "unsupported wave bank format: {:?}",
            entry.format
        ))),
    }
}

pub fn extract_music(src: &TerrariaContent, out: &Path) -> Result<usize, AssetError> {
    let bank_path = src.root().join("Wave Bank.xwb");
    if !bank_path.is_file() {
        return Ok(0);
    }
    let mut file = File::open(&bank_path)?;
    let file_len = file.metadata()?.len();
    let entries = read_wave_bank_index_from_reader(&mut file, file_len)?;

    let music_dir = out.join("Music");
    std::fs::create_dir_all(&music_dir)?;

    let mut extracted = 0;
    for track in MUSIC_TRACKS {
        if track.entry >= entries.len() {
            continue;
        }
        let entry = &entries[track.entry];
        file.seek(SeekFrom::Start(entry.offset as u64))?;
        let mut data = vec![0u8; entry.length as usize];
        file.read_exact(&mut data)?;

        let wav = match entry.format {
            WaveFormat::Adpcm {
                channels,
                rate,
                block_align,
            } => {
                let samples = decode_ms_adpcm(&data, channels, block_align)?;
                pcm_to_wav(&samples, channels, rate)
            }
            WaveFormat::Pcm {
                channels,
                rate,
                bits,
            } if bits == 16 => {
                let mut samples = Vec::with_capacity(data.len() / 2);
                for chunk in data.chunks_exact(2) {
                    samples.push(i16::from_le_bytes([chunk[0], chunk[1]]));
                }
                pcm_to_wav(&samples, channels, rate)
            }
            _ => continue,
        };

        let dest = music_dir.join(format!("{}.wav", track.file_stem));
        std::fs::write(&dest, wav)?;
        extracted += 1;
    }

    Ok(extracted)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le(v: &mut Vec<u8>, x: u32) {
        v.extend_from_slice(&x.to_le_bytes());
    }

    fn sample_bank(meta: &[u8], wave_data: &[u8]) -> Vec<u8> {
        let (bd, md) = (52u32, 52 + 96);
        let wd = md + meta.len() as u32;
        let mut v = b"WBND".to_vec();
        le(&mut v, 44); // version
        le(&mut v, 1); // header version
        for (o, l) in [
            (bd, 96),
            (md, meta.len() as u32),
            (0, 0),
            (0, 0),
            (wd, wave_data.len() as u32),
        ] {
            le(&mut v, o);
            le(&mut v, l);
        }
        let mut b = vec![0u8; 96];
        let count = (meta.len() / 24) as u32;
        b[4..8].copy_from_slice(&count.to_le_bytes());
        b[72..76].copy_from_slice(&24u32.to_le_bytes());
        b[80..84].copy_from_slice(&1u32.to_le_bytes());
        v.extend(b);
        v.extend_from_slice(meta);
        v.extend_from_slice(wave_data);
        v
    }

    #[test]
    fn unknown_entry_index_errors() {
        let fake_bank = b"WBND...";
        assert!(decode_track_entry(fake_bank, 999).is_err());
    }

    #[test]
    fn extract_music_on_fake_wave_bank_writes_files() {
        let fake_dir = std::env::temp_dir().join(format!("tv_fake_bank_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&fake_dir);
        std::fs::create_dir_all(fake_dir.join("Images")).unwrap();
        std::fs::write(fake_dir.join("Images/Tiles_0.xnb"), b"dummy").unwrap();

        // Build a synthetic bank with 2 entries:
        // entry 0: night track (MusicTrack::entry = 0)
        // entry 1: eerie track (MusicTrack::entry = 1)
        // Using PCM format for simplicity: tag 0, mono (1<<2), 44100Hz (44100<<5), 16-bit (1<<31)
        let fmt = (1 << 2) | (44100 << 5) | (1 << 31);
        let mut meta = Vec::new();
        // 4 bytes of 16-bit PCM wave per entry
        for (off, len) in [(0, 4), (4, 4)] {
            for x in [0, fmt, off, len, 0, 0] {
                le(&mut meta, x);
            }
        }
        let wave_data = vec![0u8; 8];
        let bank_bytes = sample_bank(&meta, &wave_data);
        std::fs::write(fake_dir.join("Wave Bank.xwb"), bank_bytes).unwrap();

        let src = TerrariaContent::locate(Some(fake_dir.clone())).unwrap();
        let out_dir = fake_dir.join("out");
        let count = extract_music(&src, &out_dir).unwrap();
        assert_eq!(count, 2);
        assert!(out_dir.join("Music/night.wav").exists());
        assert!(out_dir.join("Music/eerie.wav").exists());
        let _ = std::fs::remove_dir_all(&fake_dir);
    }
}
