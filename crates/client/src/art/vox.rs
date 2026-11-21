//! MagicaVoxel `.vox` (v150) reader: first model's SIZE, XYZI and RGBA chunks.

use std::path::Path;

#[derive(Debug)]
/// A parsed model in file coordinates (MagicaVoxel's z-up frame).
pub struct VoxModel {
    pub size: [u8; 3],
    /// `[x, y, z, palette index]`; index 0 is never used by a voxel.
    pub voxels: Vec<[u8; 4]>,
    /// `palette[i]` is the colour of voxel index `i` (file entry `i - 1`).
    pub palette: [[u8; 4]; 256],
}

fn u32_at(b: &[u8], at: usize) -> Result<u32, String> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| "truncated file".to_string())
}

pub fn parse(bytes: &[u8]) -> Result<VoxModel, String> {
    if bytes.get(..4) != Some(b"VOX ") {
        return Err("bad magic (not a .vox file)".into());
    }
    let version = u32_at(bytes, 4)?;
    if bytes.get(8..12) != Some(b"MAIN") {
        return Err("missing MAIN chunk".into());
    }
    let main_content = u32_at(bytes, 12)? as usize;
    let mut at = 20 + main_content;
    let end = (at + u32_at(bytes, 16)? as usize).min(bytes.len());
    let mut size = None;
    let mut voxels = None;
    let mut palette = [[255u8; 4]; 256];
    while at + 12 <= end {
        let tag = &bytes[at..at + 4];
        let n = u32_at(bytes, at + 4)? as usize;
        let m = u32_at(bytes, at + 8)? as usize;
        let body = bytes
            .get(at + 12..at + 12 + n)
            .ok_or_else(|| format!("chunk {} runs past the end", String::from_utf8_lossy(tag)))?;
        match tag {
            b"SIZE" if size.is_none() => {
                let d = |i: usize| u32_at(body, i * 4);
                let dim =
                    |v: u32| u8::try_from(v).map_err(|_| format!("model dimension {v} > 255"));
                size = Some([dim(d(0)?)?, dim(d(1)?)?, dim(d(2)?)?]);
            }
            b"XYZI" if voxels.is_none() => {
                let count = u32_at(body, 0)? as usize;
                let data = body
                    .get(4..4 + count * 4)
                    .ok_or("XYZI shorter than its count")?;
                voxels = Some(
                    data.chunks_exact(4)
                        .map(|c| [c[0], c[1], c[2], c[3]])
                        .collect(),
                );
            }
            b"RGBA" => {
                for i in 0..255 {
                    if let Some(c) = body.get(i * 4..i * 4 + 4) {
                        palette[i + 1] = [c[0], c[1], c[2], c[3]];
                    }
                }
            }
            _ => {}
        }
        at += 12 + n + m;
    }
    let _ = version;
    Ok(VoxModel {
        size: size.ok_or("missing SIZE chunk")?,
        voxels: voxels.ok_or("missing XYZI chunk")?,
        palette,
    })
}

/// Non-fatal loader; the error names `path`.
pub fn try_load(path: &Path) -> Result<VoxModel, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("missing art: {}: {e}", path.display()))?;
    parse(&bytes).map_err(|e| format!("bad art: {}: {e}", path.display()))
}

/// Loads `path`, exiting with a message that names it on any error.
pub fn load(path: &Path) -> VoxModel {
    try_load(path).unwrap_or_else(|e| crate::view::fatal(e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(tag: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = tag.to_vec();
        v.extend((body.len() as u32).to_le_bytes());
        v.extend(0u32.to_le_bytes());
        v.extend(body);
        v
    }

    fn tiny() -> Vec<u8> {
        let mut size = Vec::new();
        for d in [2u32, 1, 1] {
            size.extend(d.to_le_bytes());
        }
        let mut xyzi = 2u32.to_le_bytes().to_vec();
        xyzi.extend([0, 0, 0, 1, 1, 0, 0, 2]);
        let mut rgba = vec![0u8; 1024];
        rgba[..8].copy_from_slice(&[10, 20, 30, 255, 40, 50, 60, 255]);
        let mut kids = chunk(b"SIZE", &size);
        kids.extend(chunk(b"XYZI", &xyzi));
        kids.extend(chunk(b"RGBA", &rgba));
        let mut out = b"VOX ".to_vec();
        out.extend(150u32.to_le_bytes());
        out.extend(b"MAIN");
        out.extend(0u32.to_le_bytes());
        out.extend((kids.len() as u32).to_le_bytes());
        out.extend(kids);
        out
    }

    #[test]
    fn parse_roundtrip_tiny_vox() {
        let m = parse(&tiny()).unwrap();
        assert_eq!(m.size, [2, 1, 1]);
        assert_eq!(m.voxels, vec![[0, 0, 0, 1], [1, 0, 0, 2]]);
        assert_eq!(m.palette[1], [10, 20, 30, 255]);
        assert_eq!(m.palette[2], [40, 50, 60, 255]);
    }

    #[test]
    fn parse_rejects_bad_magic() {
        let mut b = tiny();
        b[0] = b'X';
        assert!(parse(&b).unwrap_err().contains("magic"));
        assert!(parse(&b[..6]).is_err());
    }

    #[test]
    fn load_missing_reports_path() {
        let p = Path::new("/nonexistent/dir/thing.vox");
        let e = try_load(p).err().unwrap();
        assert!(e.contains("/nonexistent/dir/thing.vox"), "{e}");
    }

    #[test]
    fn loads_committed_test_model() {
        let m = try_load(&crate::art::art_dir().join("models/test/cube.vox")).unwrap();
        assert_eq!(m.size, [4, 4, 4]);
        assert_eq!(m.voxels.len(), 64);
    }
}
