#!/usr/bin/env bash
# Smoke test of the natural-world tools in a VIRTUAL display (Xvfb).
#
# It drives the real window: paints the terrain with T, erodes part of it, draws a river with Y and a
# road with U, lowers the ground under the road, asks for the bridges and bueiros and saves. Then it
# checks the scene file for Terrain, raised TerrainChunk heights, an erosion pass that moved material
# without losing it, River, Road and Bridge.
# It never touches your desktop session or your editor settings.
#
# The pointer positions below belong to the 1440x900 screen of this script and to the options bar of
# each tool (terrain: Modo/Raio/Força, river: Largura/Profundidade, road: Largura/Defensas/button).
# If the bar changes, re-measure them from the captures the script leaves behind.
#
#   scripts/natural-smoke.sh [output.png]
set -euo pipefail

editor="${PETUNIA_EDITOR_BIN:-/tmp/petunia-editor-build/petunia-editor}"
output="${1:-/tmp/petunia-shot/natural.png}"
workdir="$(mktemp -d /tmp/petunia-smoke.XXXXXX)"
display="${PETUNIA_EDITOR_DISPLAY:-:98}"
scene="$workdir/mundo/scenes/room.scene.json"
mkdir -p "$(dirname "$output")"

cleanup() {
    [[ -n "${editor_pid:-}" ]] && kill "$editor_pid" 2>/dev/null || true
    [[ -n "${xvfb_pid:-}" ]] && kill "$xvfb_pid" 2>/dev/null || true
    # rm -rf "$workdir"
}
trap cleanup EXIT

# One terrain stroke along a screen band: press, sweep, release.
stroke() {
    local y="$1"
    shift
    xdotool mousemove "$1" "$y" mousedown 1
    shift
    for x in "$@"; do
        xdotool mousemove "$x" "$y"
        sleep 0.18
    done
    xdotool mouseup 1
    sleep 1.2
}

Xvfb "$display" -screen 0 1440x900x24 +extension GLX >/dev/null 2>&1 &
xvfb_pid=$!
sleep 1

env -u WAYLAND_DISPLAY DISPLAY="$display" XDG_CONFIG_HOME="$workdir/config" XDG_CACHE_HOME="$workdir/cache" \
    QT_QPA_PLATFORM=xcb LIBGL_ALWAYS_SOFTWARE=1 QT_LOGGING_RULES='qt.qpa.*=false' \
    "$editor" --sample "$workdir/mundo" --screenshot "$output" --screenshot-delay 45000 2>/dev/null &
editor_pid=$!
sleep 3

export DISPLAY="$display"
xdotool mousemove 835 440 click 1;               sleep 0.4   # focus the viewport
xdotool mousemove 835 440 click 1;               sleep 0.5
xdotool key t;                                   sleep 1.0   # terrain tool; the first drag creates it
import -window root "$output.tool.png"
stroke 470 700 720 740 760 780 800 820 840 860 880
import -window root "$output.terrain.png"

# Baseline for the erosion check: the raised terrain as saved, before the erosion stroke.
xdotool key ctrl+s;                              sleep 1.2
cp "$scene" "$workdir/before-erode.json"

xdotool mousemove 321 88 click 1;                sleep 0.5   # "Modo" combo of the terrain bar
xdotool key End;                                 sleep 0.3   # last entry: Erosão
xdotool key Return;                              sleep 0.8
import -window root "$output.erode-mode.png"
stroke 470 700 720 740 760 780 800 820 840 860 880
xdotool key ctrl+s;                              sleep 1.2
cp "$scene" "$workdir/after-erode.json"

xdotool key y;                                   sleep 0.8   # river tool
xdotool mousemove 700 560 click 1;               sleep 0.5
xdotool mousemove 780 540 click 1;               sleep 0.5
xdotool mousemove 860 570 click 1;               sleep 0.5
xdotool key Return;                              sleep 1.2

xdotool key u;                                   sleep 0.8   # road tool
xdotool mousemove 680 490 click 1;               sleep 0.5
xdotool mousemove 780 500 click 1;               sleep 0.5
xdotool mousemove 880 490 click 1;               sleep 0.5
xdotool key Return;                              sleep 1.2
import -window root "$output.road.png"

# Take the ground away under the road so it ends up on top of a bridge. Two strokes of a 2 m lower
# brush keep the trench shallow enough to see the deck above it.
xdotool key t;                                   sleep 0.8   # terrain tool
xdotool mousemove 321 88 click 1;                sleep 0.5   # "Modo" combo
xdotool key Home;                                sleep 0.3   # first entry: Subir
xdotool key Down;                                sleep 0.3   # second entry: Descer
xdotool key Return;                              sleep 0.8
xdotool mousemove 573 88 click 1;                sleep 0.4   # "Força" field
xdotool key ctrl+a;                              sleep 0.2
xdotool type --delay 80 2;                       sleep 0.3
xdotool key Return;                              sleep 0.8
stroke 490 700 720 740 760 780 800 820 840 860
stroke 490 860 840 820 800 780 760 740 720 700

# The brush selected the terrain; pick the road again in the hierarchy and ask for the structures.
xdotool mousemove 80 417 click 1;                sleep 0.8   # "Estrada" row of the hierarchy
xdotool key u;                                   sleep 0.8   # road tool: the button lives in its bar
import -window root "$output.structures.png"
xdotool mousemove 532 88 click 1;                sleep 1.5   # "Pontes e bueiros"
import -window root "$output.detected.png"                    # the status bar names what was found
xdotool key ctrl+s;                              sleep 1.2
import -window root "$output.natural.png"

python3 - "$scene" "$workdir/before-erode.json" "$workdir/after-erode.json" <<'PY'
import json
import sys

scene_path, before_path, after_path = sys.argv[1], sys.argv[2], sys.argv[3]


def rows(path):
    with open(path, encoding="utf-8") as file:
        return [entity.get("rows", {}) for entity in json.load(file)["entities"]]


def chunks(all_rows):
    return {
        (row["TerrainChunk"].get("chunk_x"), row["TerrainChunk"].get("chunk_z")): row["TerrainChunk"]
        for row in all_rows
        if "TerrainChunk" in row
    }


now = rows(scene_path)
checks = []
checks.append(("Terrain criado", any("Terrain" in row for row in now)))
heights = [value for chunk in chunks(now).values() for value in chunk.get("heights", [])]
checks.append(("alturas pintadas", any(abs(value) > 1e-4 for value in heights)))

before = chunks(rows(before_path))
after = chunks(rows(after_path))
common = sorted(before.keys() & after.keys())
lost = gained = False
before_sum = after_sum = 0.0
for key in common:
    old = before[key].get("heights", [])
    new = after[key].get("heights", [])
    for old_height, new_height in zip(old, new):
        lost = lost or new_height < old_height - 1e-6
        gained = gained or new_height > old_height + 1e-6
    before_sum += sum(old)
    after_sum += sum(new)
checks.append(("erosão mexeu no terreno", bool(common) and lost and gained))
checks.append(("erosão conservou o volume", bool(common) and abs(before_sum - after_sum) < 1e-2))

checks.append(("Rio criado", any("River" in row for row in now)))
checks.append(("Estrada criada", any("Road" in row for row in now)))
bridges = [row["Bridge"] for row in now if "Bridge" in row]
checks.append(("ponte ou bueiro detectado", bool(bridges)))

failed = False
for label, ok in checks:
    print(f"verificação: {label} -> {'ok' if ok else 'FALHOU'}")
    failed = failed or not ok
for bridge in bridges:
    print(f"  estrutura: {bridge.get('kind')} {bridge.get('start_parameter')}..{bridge.get('end_parameter')}")
sys.exit(1 if failed else 0)
PY

echo "imagem: $output (e $output.terrain.png, $output.erode-mode.png, $output.road.png, $output.structures.png, $output.detected.png, $output.natural.png)"
