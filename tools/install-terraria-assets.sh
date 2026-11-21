#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
dest="$root/assets/terraria-pack"

echo "Extracting Terraria assets (textures, sounds, fonts, and music) into $dest..."
cargo run --release --manifest-path "$root/Cargo.toml" -p terravibe_client -- extract "$dest" "$@"

if [ ! -f "$dest/pack.txt" ]; then
    echo "error: $dest/pack.txt was not created" >&2
    exit 1
fi

echo "Terraria asset pack installed successfully:"
du -sh "$dest"
