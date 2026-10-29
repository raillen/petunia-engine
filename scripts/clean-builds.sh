#!/usr/bin/env bash
# Removes stale, unnecessary build outputs of THIS project only (AGENTS.md, section 4).
#
#   scripts/clean-builds.sh          stale outputs: duplicate ./target, incremental caches older
#                                    than a day, leftover test folders
#   scripts/clean-builds.sh --all    also the build folders themselves (next build is from zero)
#
# It prints every path it removes and how much space came back. It never touches anything else.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# Same folder rules as scripts/editor.sh: one per working copy, named after the checkout.
checkout="${root##*/}"
checkout="${checkout#petunia-engine-}"
build="${CARGO_TARGET_DIR:-$(sed -n 's/^target-dir *= *"\(.*\)"/\1/p' "$root/.cargo/config.toml")}"
build="${build:-$root/target}"
editor_build="${PETUNIA_BUILD_DIR:-/tmp/petunia-editor-${checkout:-petunia-engine}}"
mode="${1:-stale}"

free_kib() { df -k --output=avail "$1" | tail -1 | tr -d ' '; }
before_disk=$(free_kib "$root")
before_tmp=$(free_kib /tmp)

remove() {
    local path="$1"
    if [[ -e "$path" ]]; then
        printf '  removendo %-55s %s\n' "$path" "$(du -sh "$path" 2>/dev/null | cut -f1)"
        rm -rf -- "$path"
    fi
}

echo "Limpando compilações e restos de teste da Petunia Engine:"

# A second build folder inside the repository only duplicates the real one (see .cargo/config.toml).
# Guard: it must look like a cargo output folder.
if [[ -f "$root/target/CACHEDIR.TAG" ]]; then
    remove "$root/target"
fi

# Leftovers of tests that were interrupted before their own cleanup.
shopt -s nullglob
for leftover in /tmp/petunia-smoke.* /tmp/petunia-x.* /tmp/petunia_editor_test_*; do
    remove "$leftover"
done

# Incremental compilation sessions older than a day (cargo keeps the recent ones it needs).
if [[ -d "$build/debug/incremental" ]]; then
    while IFS= read -r -d '' stale; do
        remove "$stale"
    done < <(find "$build/debug/incremental" -mindepth 1 -maxdepth 1 -mtime +0 -print0)
fi

if [[ "$mode" == "--all" ]]; then
    remove "$build"
    remove "$editor_build"
fi

after_disk=$(free_kib "$root")
after_tmp=$(free_kib /tmp)
printf 'Liberado: %s MB no disco do projeto, %s MB em /tmp.\n' \
    "$(( (after_disk - before_disk) / 1024 ))" "$(( (after_tmp - before_tmp) / 1024 ))"
