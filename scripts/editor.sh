#!/usr/bin/env bash
# Builds the Rust engine library and the Qt editor, then runs the editor.
#
#   scripts/editor.sh                 build and open the editor
#   scripts/editor.sh build           only build
#   scripts/editor.sh run -- <args>   run with arguments, e.g. --sample /tmp/sala
#
# Build output goes to /tmp (RAM) by default because the disk has been nearly full. Override with
# PETUNIA_BUILD_DIR. Limits parallel jobs to 2 (see AGENTS.md, host guardrails).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
build_dir="${PETUNIA_BUILD_DIR:-/tmp/petunia-editor-build}"
# The Rust build folder is the one `.cargo/config.toml` names for this working copy: sharing it with
# another checkout makes cargo overwrite the other copy's metadata (phantom unresolved imports).
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/petunia-engine-mundo-natural-target}"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"

mode="${1:-run}"
shift || true
if [[ "${1:-}" == "--" ]]; then shift; fi

build() {
    (cd "$root" && cargo build -p petunia_editor_ffi)
    local library="$CARGO_TARGET_DIR/debug/libpetunia_editor_ffi.a"
    cmake -S "$root/editor" -B "$build_dir" -G Ninja \
        -DCMAKE_BUILD_TYPE=Debug -DPETUNIA_FFI_LIB="$library"
    # clangd only looks for the compilation database next to the sources (and in parents), so link it
    # there. The link is ignored by git; without it the IDE cannot find the Qt headers.
    ln -sfn "$build_dir/compile_commands.json" "$root/editor/compile_commands.json"
    cmake --build "$build_dir" --parallel 2
}

case "$mode" in
    build) build ;;
    run) build; exec "$build_dir/petunia-editor" "$@" ;;
    *) echo "uso: scripts/editor.sh [build|run] [-- argumentos]" >&2; exit 2 ;;
esac
