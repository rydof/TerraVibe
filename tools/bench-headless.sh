#!/usr/bin/env bash
# Run TerraVibe's --bench in a private headless Sway session.
# No window, focus change or workspace switch ever appears on the desktop.
#
# Usage: tools/bench-headless.sh [--liquid-stress|--effects] [path-to-terravibe]
#   Default binary: target/release/terravibe
#
# Requires sway + swaymsg (both installed on the dev laptop). Rendering still
# uses the real Vulkan GPU; the compositor's output is a virtual HEADLESS-1.
set -euo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
bin_arg=""
mode_args=(--bench)
for arg in "$@"; do
    case "$arg" in
        --liquid-stress)
            export TERRAVIBE_BENCH_SCENARIO="liquid_stress"
            ;;
        --effects)
            mode_args=(--qa bench_effects)
            ;;
        *)
            bin_arg="$arg"
            ;;
    esac
done
bin=${bin_arg:-"$here/target/release/terravibe"}
if [ ! -x "$bin" ]; then
    echo "missing executable: $bin (run: cargo build -q --release)" >&2
    exit 1
fi
for tool in sway swaymsg; do
    if ! command -v "$tool" >/dev/null; then
        echo "missing required tool: $tool" >&2
        exit 2
    fi
done

base=${TERRAVIBE_HEADLESS_DIR:-/tmp/opencode}
mkdir -p "$base"
runtime=$(mktemp -d "$base/terravibe-headless.XXXXXX")
chmod 700 "$runtime"
export XDG_RUNTIME_DIR=$runtime
log=$runtime/sway.log

WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=gles2 \
    sway -c /dev/null >"$log" 2>&1 &
pid=$!
cleanup() {
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
    rm -rf "$runtime"
}
trap cleanup EXIT

socket=""
for _ in $(seq 1 200); do
    socket=$(find "$runtime" -maxdepth 1 -type s -name 'wayland-*' -print -quit)
    if [ -n "$socket" ]; then
        break
    fi
    if ! kill -0 "$pid" 2>/dev/null; then
        echo "headless sway exited before creating a socket" >&2
        cat "$log" >&2
        exit 2
    fi
    sleep 0.05
done
if [ -z "$socket" ]; then
    echo "headless sway did not create a Wayland socket" >&2
    cat "$log" >&2
    exit 2
fi

export WAYLAND_DISPLAY
WAYLAND_DISPLAY=$(basename "$socket")
export SWAYSOCK
SWAYSOCK=$(find "$runtime" -maxdepth 1 -type s -name 'sway-ipc.*' -print -quit)
# Match the physical benchmark's 1920x1080 output when the headless mode allows it.
if [ -n "${SWAYSOCK:-}" ]; then
    swaymsg output HEADLESS-1 mode 1920x1080 >/dev/null 2>&1 || true
fi

TERRAVIBE_MONITOR=HEADLESS "$bin" "${mode_args[@]}"
