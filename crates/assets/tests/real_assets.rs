use std::path::PathBuf;
use terravibe_assets::{
    WaveFormat, Xnb, decode_sound, decode_texture, read_wave_bank_index, read_xnb,
};

fn load(c: &std::path::Path, name: &str) -> Xnb {
    read_xnb(&std::fs::read(c.join(format!("{name}.xnb"))).unwrap()).unwrap()
}

fn content() -> Option<PathBuf> {
    let p = std::env::var_os("TERRAVIBE_TERRARIA_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| {
                PathBuf::from(h).join(".local/share/Steam/steamapps/common/Terraria/Content")
            })
        })?;
    if p.is_dir() {
        Some(p)
    } else {
        println!("skipped: Terraria not installed");
        None
    }
}

#[test]
fn tiles_0_is_dirt_sheet() {
    let Some(c) = content() else { return };
    let img = decode_texture(&load(&c, "Images/Tiles_0")).unwrap();
    assert_eq!((img.width, img.height), (288, 270));
    let p = img.get(18 + 8, 18 + 8);
    assert_eq!(p[3], 255);
    assert!(p[0] > p[2]);
}

#[test]
fn dig_0_sound() {
    let Some(c) = content() else { return };
    let s = decode_sound(&load(&c, "Sounds/Dig_0")).unwrap();
    // Brief expected 1 channel; the installed Terraria 1.4.5.8 file is stereo.
    assert_eq!(
        (s.sample_rate, s.channels, s.bits_per_sample),
        (44100, 2, 16)
    );
    assert_eq!(s.data.len(), 5604);
}

#[test]
fn npc_1_slime_decodes() {
    let Some(c) = content() else { return };
    let img = decode_texture(&load(&c, "Images/NPC_1")).unwrap();
    assert!(img.width > 0 && img.height.is_multiple_of(2));
}

#[test]
fn reads_compressed_tiles_0() {
    let Some(c) = content() else { return };
    let x = read_xnb(&std::fs::read(c.join("Images/Tiles_0.xnb")).unwrap()).unwrap();
    assert!(
        x.readers
            .first()
            .is_some_and(|r| r.starts_with("Microsoft.Xna.Framework.Content.Texture2DReader"))
    );
}

#[test]
fn indexes_music_bank() {
    let Some(c) = content() else { return };
    let e = read_wave_bank_index(&std::fs::read(c.join("Wave Bank.xwb")).unwrap()).unwrap();
    assert!(e.len() >= 50);
    assert!(matches!(e[0].format, WaveFormat::Adpcm { .. }));
    eprintln!(
        "wave bank: {} entries, first format {:?}",
        e.len(),
        e[0].format
    );
    let mut h = std::collections::BTreeMap::new();
    for x in &e {
        *h.entry(format!("{:?}", x.format)).or_insert(0usize) += 1;
    }
    eprintln!("format histogram: {h:?}");
}

struct RemoveOnDrop(std::path::PathBuf);
impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
#[ignore = "slow: full extraction; run with --ignored --release"]
fn extract_pack_roundtrip() {
    use terravibe_assets::{TerrariaContent, extract_pack};
    let Some(c) = content() else { return };
    let src = TerrariaContent::locate(Some(c)).unwrap();
    let tmp = std::env::temp_dir().join(format!("tv_pack_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let _guard = RemoveOnDrop(tmp.clone());
    let t = std::time::Instant::now();
    let stats = extract_pack(&src, &tmp).unwrap();
    eprintln!("extract: PackStats {stats:?} in {:.1?}", t.elapsed());
    assert!(stats.textures > 100 && stats.sounds > 50 && stats.fonts >= 1);
    let pack = TerrariaContent::open(Some(tmp.clone()), None).unwrap();
    assert!(pack.is_pack());
    let a = pack.texture("Images/Tiles_0").unwrap();
    assert_eq!((a.width, a.height), (288, 270));
    assert_eq!(a, src.texture("Images/Tiles_0").unwrap());
    assert_eq!(
        pack.sound("Sounds/Dig_0").unwrap(),
        src.sound("Sounds/Dig_0").unwrap()
    );
    assert_eq!(
        pack.font("Fonts/Mouse_Text").unwrap(),
        src.font("Fonts/Mouse_Text").unwrap()
    );
}

#[test]
fn mouse_text_font_decodes() {
    let Some(c) = content() else { return };
    let tc = terravibe_assets::TerrariaContent::locate(Some(c)).unwrap();
    let f = tc.font("Fonts/Mouse_Text").unwrap();
    assert!(f.line_spacing > 0);
    for ch in ('A'..='Z').chain('a'..='z').chain('0'..='9') {
        assert!(f.glyph(ch).is_some(), "missing glyph {ch}");
    }
}

#[test]
#[ignore = "needs Terraria install"]
fn real_extract_music_lengths_match_spike_table() {
    let Some(c) = content() else { return };
    let tc = terravibe_assets::TerrariaContent::locate(Some(c.clone())).unwrap();
    let tmp = std::env::temp_dir().join(format!("tv_music_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let _guard = RemoveOnDrop(tmp.clone());
    let count = terravibe_assets::extract_music(&tc, &tmp).unwrap();
    assert!(count >= 14);
    // Check night track (entry 0) file exists and has plausible WAV size (> 1MB)
    let night = std::fs::read(tmp.join("Music/night.wav")).unwrap();
    assert!(night.len() > 1_000_000);
}
