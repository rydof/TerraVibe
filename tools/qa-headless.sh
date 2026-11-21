#!/usr/bin/env bash
# Run TerraVibe's automated QA in a private headless Sway session.
# No window, focus change or workspace switch ever appears on the desktop.
#
# Usage: tools/qa-headless.sh <scenario> [--out DIR] [path-to-terravibe]
set -euo pipefail

out=""
scenario=""
bin=""

while [ $# -gt 0 ]; do
    case "$1" in
        --out)
            if [ $# -lt 2 ]; then
                echo "error: --out needs a DIR" >&2
                exit 1
            fi
            out="$2"
            shift 2
            ;;
        --out=*)
            out="${1#*=}"
            shift
            ;;
        *)
            if [ -z "$scenario" ]; then
                scenario="$1"
            elif [ -z "$bin" ]; then
                bin="$1"
            else
                echo "error: unexpected argument '$1'" >&2
                exit 1
            fi
            shift
            ;;
    esac
done

if [ -z "$scenario" ]; then
    echo "usage: tools/qa-headless.sh <scenario> [--out DIR] [path-to-terravibe]" >&2
    exit 1
fi

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
if [ -z "$bin" ]; then
    if [ -x "$here/target/release/terravibe" ]; then
        bin="$here/target/release/terravibe"
    elif [ -x "$here/target/debug/terravibe" ]; then
        bin="$here/target/debug/terravibe"
    else
        bin="$here/target/release/terravibe"
    fi
fi
if [ ! -x "$bin" ]; then
    echo "missing executable: $bin (run: cargo build -q)" >&2
    exit 1
fi

base=${TERRAVIBE_HEADLESS_DIR:-/tmp/opencode}
mkdir -p "$base"
if [ -z "$out" ]; then
    out=$(mktemp -d "$base/terravibe-qa.XXXXXX")
    echo "qa out: $out"
else
    mkdir -p "$out"
fi

disco=$("$bin" --qa "$scenario" --qa-phase 0)
echo "$disco"
phases=$(echo "$disco" | grep -o 'phases=[0-9]\+' | cut -d= -f2)
if [ -z "$phases" ] || [ "$phases" -lt 1 ]; then
    echo "failed to discover phases for scenario '$scenario' (output: $disco)" >&2
    exit 1
fi

for tool in sway swaymsg; do
    if ! command -v "$tool" >/dev/null; then
        echo "missing required tool: $tool" >&2
        exit 2
    fi
done

runtime=$(mktemp -d "$base/terravibe-sway.XXXXXX")
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
if [ -n "${SWAYSOCK:-}" ]; then
    swaymsg output HEADLESS-1 mode 1920x1080 >/dev/null 2>&1 || true
fi

mkdir -p "$out/config" "$out/saves" "$out/data"

for phase in $(seq 1 "$phases"); do
    phase_out="$out/$scenario/phase-$phase"
    mkdir -p "$phase_out"
    echo "running scenario $scenario phase $phase/$phases -> $phase_out"

    set +e
    XDG_DATA_HOME="$out/data" \
    XDG_CONFIG_HOME="$out/config" \
    TERRAVIBE_MONITOR=HEADLESS \
    "$bin" --qa "$scenario" --qa-phase "$phase" --qa-out "$phase_out" --save-dir "$out/saves"
    rc=$?
    set -e

    if [ "$rc" -ne 0 ]; then
        echo "scenario $scenario failed at phase $phase with exit code $rc" >&2
        exit "$rc"
    fi
done

echo "scenario $scenario passed ($phases phases)"
