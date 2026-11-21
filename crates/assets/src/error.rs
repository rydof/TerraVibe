use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("not an XNB file (bad magic)")]
    BadMagic,
    #[error("unsupported XNB version {0}")]
    UnsupportedVersion(u8),
    #[error("file is truncated or malformed")]
    Truncated,
    #[error("malformed data: {0}")]
    Malformed(String),
    #[error("LZX decompression failed: {0}")]
    Decompress(String),
    #[error("unexpected content reader: expected {expected}, found {found}")]
    UnexpectedReader {
        expected: &'static str,
        found: String,
    },
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
    #[error("{}", not_found_message(tried))]
    NotFound { tried: Vec<PathBuf> },
}

fn not_found_message(tried: &[PathBuf]) -> String {
    let mut msg = String::from("Terraria content not found. Tried:");
    for p in tried {
        msg.push_str("\n  ");
        msg.push_str(&p.display().to_string());
    }
    msg.push_str(
        "\nPass --terraria-dir <path> or set the TERRAVIBE_TERRARIA_DIR environment variable \
         to your Terraria (or Terraria/Content) directory, or pass --assets-dir <path> or set \
         TERRAVIBE_ASSETS_DIR to an extracted asset pack.",
    );
    msg
}
