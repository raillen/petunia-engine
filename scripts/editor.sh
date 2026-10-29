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
# One Qt build folder per working copy. CMake keeps the source directory of whoever configured the
# folder first, so this worktree built `petunia-engine-mundo`'s editor for a while and the smoke
# tests ran a binary whose strings did not exist here. The checkout name picks the folder:
# `petunia-engine-mundo-natural` -> `/tmp/petunia-editor-mundo-natural`.
checkout="${root##*/}"
checkout="${checkout#petunia-engine-}"
build_dir="${PETUNIA_BUILD_DIR:-/tmp/petunia-editor-${checkout:-petunia-engine}}"
# The Rust build folder is the one `.cargo/config.toml` names for this working copy: sharing it with
# another checkout makes cargo overwrite the other copy's metadata (phantom unresolved imports).
target_dir="$(sed -n 's/^target-dir *= *"\(.*\)"/\1/p' "$root/.cargo/config.toml")"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${target_dir:-$root/target}}"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"

mode="${1:-run}"
shift || true
if [[ "${1:-}" == "--" ]]; then shift; fi

build() {
    local configured
    configured="$(sed -n 's/^CMAKE_HOME_DIRECTORY:INTERNAL=//p' "$build_dir/CMakeCache.txt" 2>/dev/null || true)"
    if [[ -n "$configured" && "$configured" != "$root/editor" ]]; then
        echo "erro: $build_dir foi configurado para $configured" >&2
        echo "      apague essa pasta ou peça outra com PETUNIA_BUILD_DIR (uma por cópia de trabalho)" >&2
        exit 2
    fi
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
