#!/usr/bin/env bash
# Smoke test of the natural-world tools in a VIRTUAL display (Xvfb).
#
# It drives the real window: paints the terrain with T, erodes part of it, roughens it with the noise
# brush, applies one keyboard stamp (Space) and undoes it, stamps a crater and a mount with the
# "Carimbo" mode, stamps the same crest straight and turned by 90° (DR-37), draws a river with Y and
# a road with U, lowers the ground under the road, asks for the bridges and bueiros and saves. Then
# it checks the scene file for Terrain, raised TerrainChunk heights, an erosion pass, a noise pass
# that moved material without losing it, a step that raised the ground and an undo that gave it
# back, a crater that dug a bowl and banked the rim, a mount that only rose, a crest that runs along
# x when straight and along z when turned, River, Road and Bridge.
# It never touches your desktop session or your editor settings.
#
# The script takes every picture itself and kills the editor at the end. It does not pass
# `--screenshot` to the editor: that flag makes the editor quit after a fixed delay, and the script
# outgrew the delay once — the last capture came out black while every assertion still passed.
#
# The pointer positions below belong to the 1440x900 screen of this script and to the options bar of
# each tool (terrain: Modo/Raio/Força/Forma, river: Largura/Profundidade, road: Largura/Defensas/button).
# If the bar changes, re-measure them from the captures the script leaves behind.
#
#   scripts/natural-smoke.sh [output.png]
set -euo pipefail

editor="${PETUNIA_EDITOR_BIN:-}"
if [[ -z "$editor" ]]; then
    # One Qt build folder per working copy, named like scripts/editor.sh does.
    root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
    checkout="$(basename "$root")"
    checkout="${checkout#petunia-engine-}"
    editor="/tmp/petunia-editor-${checkout:-petunia-engine}/petunia-editor"
fi
output="${1:-/tmp/petunia-shot/natural.png}"
workdir="$(mktemp -d /tmp/petunia-smoke.XXXXXX)"
mkdir -p "$(dirname "$output")"
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/_virtual-display.sh"
# A display of our own: a run that died can leave an editor window on top of ours and every key
# would go to it. `choose_display` skips a busy number (or stops when the display was pinned).
choose_display :98
scene="$workdir/mundo/scenes/room.scene.json"

cleanup() {
    [[ -n "${editor_pid:-}" ]] && kill "$editor_pid" 2>/dev/null || true
    [[ -n "${xvfb_pid:-}" ]] && kill "$xvfb_pid" 2>/dev/null || true
    # A run that fails leaves the scene, the editor log and the pictures behind when
    # PETUNIA_SMOKE_KEEP is set: the log of a crash is the only evidence of where it died.
    if [[ -n "${PETUNIA_SMOKE_KEEP:-}" ]]; then
        echo "arquivos da rodada em $workdir" >&2
    else
        rm -rf "$workdir"
    fi
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

# Picks an entry of the "Modo" combo by its position in the list, counted from the first one
# (Subir, Descer, Suavizar, Nivelar, Ruído, Erosão, Carimbo). Counting from Home on purpose:
# "End" silently changed meaning the day a new mode was added at the bottom of the list.
pick_mode() {
    local steps="$1"
    local step
    xdotool mousemove 321 88 click 1;            sleep 0.5
    xdotool key Home;                            sleep 0.3
    for step in $(seq 1 "$steps"); do
        xdotool key Down;                        sleep 0.15
    done
    xdotool key Return;                          sleep 0.8
}

# Saves and waits for the scene file to be rewritten, then keeps a copy. The editor hands the save to
# the engine and the file appears a moment later; a fixed `sleep` once copied the snapshot of the
# PREVIOUS action, and every later check then compared the wrong pair of files. The wait is bounded
# and loud: a save that never lands stops the run instead of feeding the checks stale data.
save_scene() {
    local target="$1"
    local before after="ausente"
    local attempt
    before="$(stat -c %y "$scene" 2>/dev/null || echo ausente)"
    xdotool key ctrl+s
    for attempt in $(seq 1 40); do
        sleep 0.15
        after="$(stat -c %y "$scene" 2>/dev/null || echo ausente)"
        if [[ "$after" != "$before" ]]; then
            break
        fi
    done
    if [[ "$after" == "$before" ]]; then
        echo "erro: a cena não foi regravada antes de $target" >&2
        return 1
    fi
    cp "$scene" "$target"
}

Xvfb "$display" -screen 0 1440x900x24 +extension GLX >/dev/null 2>&1 &
xvfb_pid=$!
sleep 1

env -u WAYLAND_DISPLAY DISPLAY="$display" XDG_CONFIG_HOME="$workdir/config" XDG_CACHE_HOME="$workdir/cache" \
    QT_QPA_PLATFORM=xcb LIBGL_ALWAYS_SOFTWARE=1 QT_LOGGING_RULES='qt.qpa.*=false' \
    "$editor" --sample "$workdir/mundo" 2>"$workdir/editor.log" &
editor_pid=$!
sleep 1
watch_editor "$editor_pid" "$workdir/editor.log"
require_virtual_display "$editor_pid" "$display"
wait_for_window "Petunia Editor" "$display" "$editor_pid" "$workdir/editor.log"

export DISPLAY="$display"
xdotool mousemove 835 440 click 1;               sleep 0.4   # focus the viewport
xdotool mousemove 835 440 click 1;               sleep 0.5
xdotool key t;                                   sleep 1.0   # terrain tool; the first drag creates it
import -window root "$output.tool.png"
stroke 470 700 720 740 760 780 800 820 840 860 880
import -window root "$output.terrain.png"

# Baseline for the erosion check: the raised terrain as saved, before the erosion stroke.
save_scene "$workdir/before-erode.json"

# The keyboard path of the brush (docs/04-acessibilidade/01-editor.md, "esculpir/pintar sem
# arrastar"): with the terrain tool active, ONE press of Space stamps where the pointer is. The
# script only MOVES the pointer (focus stays in the viewport) — no click, no drag, no hold. Then it
# undoes the step, so the terrain the erosion pass measures is exactly the one saved above.
xdotool mousemove 780 470;                       sleep 0.4
xdotool key space;                               sleep 1.2
import -window root "$output.step.png"                       # the status bar says what the step did
save_scene "$workdir/after-step.json"
xdotool key ctrl+z;                              sleep 1.2
save_scene "$workdir/after-step-undo.json"

pick_mode 5                                                     # Erosão
import -window root "$output.erode-mode.png"
stroke 470 700 720 740 760 780 800 820 840 860 880
save_scene "$workdir/after-erode.json"

# The noise brush adds fractal detail anchored in the world and centred, so the patch keeps its
# volume.
pick_mode 4                                                     # Ruído
xdotool mousemove 573 88 click 1;                sleep 0.4   # "Força" field
xdotool key ctrl+a;                              sleep 0.2
# Whole meters on purpose: the decimal separator of the spin box follows the system locale, so a
# typed "1.5" turns into 15 and buries the patch under spikes.
xdotool type --delay 80 2;                       sleep 0.3
xdotool key Return;                              sleep 0.8
import -window root "$output.noise-mode.png"
stroke 470 700 720 740 760 780 800 820 840 860 880
save_scene "$workdir/after-noise.json"

# The terrain stamps ("Carimbo"): a named shape instead of a raise. Strength first, then the mode.
# One click is one stamp: press and release in the same place, no drag at all.
xdotool mousemove 573 88 click 1;                sleep 0.4   # "Força" field
xdotool key ctrl+a;                              sleep 0.2
xdotool type --delay 80 2;                       sleep 0.3
xdotool key Return;                              sleep 0.8
pick_mode 6                                                     # Carimbo (shape "Cratera")
xdotool mousemove 760 470 click 1;               sleep 1.5   # one stamp of the crater
import -window root "$output.stamp-crater.png"               # the new "Forma" combo is in the bar
save_scene "$workdir/after-stamp-crater.json"

# The same mode with the second shape: a mound only lifts the ground.
xdotool mousemove 700 88 click 1;                sleep 0.5   # "Forma" combo (only in this mode)
xdotool key Home;                                sleep 0.3   # first entry: Cratera
xdotool key Down;                                sleep 0.3   # second entry: Monte
xdotool key Return;                              sleep 0.8
xdotool mousemove 760 470 click 1;               sleep 1.5
import -window root "$output.stamp-mound.png"
save_scene "$workdir/after-stamp-mound.json"

# The turn of the shape (DR-37). Same crest twice: once straight (along x) and, after an undo back
# to the mount, once turned by 90° (along z). Comparing each one with the mount snapshot is what
# tells the two apart without knowing the screen-to-world scale.
xdotool mousemove 700 88 click 1;                sleep 0.5   # "Forma" combo
xdotool key Home;                                sleep 0.3   # first entry: Cratera
xdotool key Down Down Down;                      sleep 0.3   # fourth entry: Crista
xdotool key Return;                              sleep 0.8
xdotool mousemove 760 470 click 1;               sleep 1.5   # crest straight, 0°
save_scene "$workdir/after-ridge-x.json"
xdotool key ctrl+z;                              sleep 1.2   # back to the mount, nothing turned
xdotool mousemove 812 88 click 1;                sleep 0.5   # "Girar" field
xdotool key ctrl+a;                              sleep 0.2
xdotool type --delay 80 90;                      sleep 0.3
xdotool key Return;                              sleep 0.8
xdotool mousemove 760 470 click 1;               sleep 1.5   # the same crest, a quarter turn
import -window root "$output.stamp-turned.png"
save_scene "$workdir/after-ridge-z.json"

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

# Take the ground away under the road so it ends up on top of a bridge. Two strokes of a 4 m lower
# brush clear it by more than the 2 m the detector asks for, even with the roughness the noise pass
# left behind; the deck stays above the trench and keeps the rails visible. The detector needs two
# consecutive samples above the threshold, so a marginal trench is not enough.
xdotool key t;                                   sleep 0.8   # terrain tool
pick_mode 1                                                     # Descer
xdotool mousemove 573 88 click 1;                sleep 0.4   # "Força" field
xdotool key ctrl+a;                              sleep 0.2
xdotool type --delay 80 4;                       sleep 0.3
xdotool key Return;                              sleep 0.8
stroke 490 700 720 740 760 780 800 820 840 860
stroke 490 860 840 820 800 780 760 740 720 700

# The brush selected the terrain; pick the road again in the hierarchy and ask for the structures.
xdotool mousemove 80 417 click 1;                sleep 0.8   # "Estrada" row of the hierarchy
xdotool key u;                                   sleep 0.8   # road tool: the button lives in its bar
import -window root "$output.structures.png"
xdotool mousemove 532 88 click 1;                sleep 1.5   # "Pontes e bueiros"
import -window root "$output.detected.png"                    # the status bar names what was found
# The raised band fills the original view; the top face of the view cube looks down on the work, so
# the final capture shows the roughened patch, the river, the road and the bridge at once.
xdotool mousemove 940 164 click 1;                sleep 1.5
xdotool key ctrl+s;                              sleep 1.5
import -window root "$output"                                # the final frame: terrain, river, road, rows

python3 - "$scene" "$workdir/before-erode.json" "$workdir/after-erode.json" "$workdir/after-noise.json" \
    "$workdir/after-step.json" "$workdir/after-step-undo.json" \
    "$workdir/after-stamp-crater.json" "$workdir/after-stamp-mound.json" \
    "$workdir/after-ridge-x.json" "$workdir/after-ridge-z.json" <<'PY'
import json
import sys

(scene_path, before_path, after_path, noise_path, step_path, undo_path, crater_path,
 mound_path, ridge_x_path, ridge_z_path) = sys.argv[1:11]


def rows(path):
    with open(path, encoding="utf-8") as file:
        return [entity.get("rows", {}) for entity in json.load(file)["entities"]]


def chunks(all_rows):
    return {
        (row["TerrainChunk"].get("chunk_x"), row["TerrainChunk"].get("chunk_z")): row["TerrainChunk"]
        for row in all_rows
        if "TerrainChunk" in row
    }


def terrain_change(first_path, second_path):
    """Whether both signs appeared and how far the total height moved."""
    first = chunks(rows(first_path))
    second = chunks(rows(second_path))
    common = sorted(first.keys() & second.keys())
    lost = gained = False
    first_sum = second_sum = 0.0
    for key in common:
        old = first[key].get("heights", [])
        new = second[key].get("heights", [])
        for old_height, new_height in zip(old, new):
            lost = lost or new_height < old_height - 1e-6
            gained = gained or new_height > old_height + 1e-6
        first_sum += sum(old)
        second_sum += sum(new)
    return bool(common), lost and gained, abs(first_sum - second_sum)


def total_gain(first_path, second_path):
    """How much the ground rose in total and whether any cell went down."""
    first = chunks(rows(first_path))
    second = chunks(rows(second_path))
    common = sorted(first.keys() & second.keys())
    gain = 0.0
    lowered = False
    for key in common:
        for old_height, new_height in zip(first[key].get("heights", []), second[key].get("heights", [])):
            gain += new_height - old_height
            lowered = lowered or new_height < old_height - 1e-6
    return bool(common), gain, lowered


def shape_change(first_path, second_path):
    """Whether a stamp lowered and raised ground, how deep it went and the net height added."""
    first = chunks(rows(first_path))
    second = chunks(rows(second_path))
    common = sorted(first.keys() & second.keys())
    lowered = raised = False
    net = 0.0
    deepest = 0.0
    for key in common:
        for old_height, new_height in zip(first[key].get("heights", []), second[key].get("heights", [])):
            delta = new_height - old_height
            net += delta
            deepest = min(deepest, delta)
            lowered = lowered or delta < -1e-6
            raised = raised or delta > 1e-6
    return bool(common), lowered, raised, net, deepest


def stamp_extent(base_path, after_path):
    """How far along x and z a stamp moved the ground, in meters (cells that moved over 0.25 m)."""
    base = chunks(rows(base_path))
    after = chunks(rows(after_path))
    common = sorted(base.keys() & after.keys())
    min_x = min_z = None
    max_x = max_z = None
    cells = 0
    for key in common:
        column = after[key]
        size = column.get("size", 64)
        cell = column.get("cell_size", 1.0)
        corner_x = column.get("chunk_x", 0) * size * cell
        corner_z = column.get("chunk_z", 0) * size * cell
        for index, (old_height, new_height) in enumerate(
                zip(base[key].get("heights", []), column.get("heights", []))):
            if abs(new_height - old_height) <= 0.25:
                continue
            x = corner_x + (index % size) * cell
            z = corner_z + (index // size) * cell
            cells += 1
            min_x = x if min_x is None else min(min_x, x)
            max_x = x if max_x is None else max(max_x, x)
            min_z = z if min_z is None else min(min_z, z)
            max_z = z if max_z is None else max(max_z, z)
    if cells < 8:
        return 0.0, 0.0
    return max_x - min_x, max_z - min_z


now = rows(scene_path)
checks = []
checks.append(("Terrain criado", any("Terrain" in row for row in now)))
heights = [value for chunk in chunks(now).values() for value in chunk.get("heights", [])]
checks.append(("alturas pintadas", any(abs(value) > 1e-4 for value in heights)))

erosion_shared, erosion_both_signs, erosion_volume = terrain_change(before_path, after_path)
checks.append(("erosão mexeu no terreno", erosion_shared and erosion_both_signs))
checks.append(("erosão conservou o volume", erosion_shared and erosion_volume < 1e-2))

noise_shared, noise_both_signs, noise_volume = terrain_change(after_path, noise_path)
checks.append(("ruído mexeu no terreno", noise_shared and noise_both_signs))
checks.append(("ruído conservou o volume", noise_shared and noise_volume < 1e-2))

step_shared, step_gain, step_lowered = total_gain(before_path, step_path)
checks.append(("passo por teclado subiu o terreno", step_shared and step_gain > 0.05 and not step_lowered))
undo_shared, undo_gain, undo_lowered = total_gain(before_path, undo_path)
checks.append(("desfazer devolveu o passo", undo_shared and abs(undo_gain) < 1e-6 and not undo_lowered))

crater_shared, crater_lowered, crater_raised, crater_net, crater_deepest = shape_change(noise_path, crater_path)
checks.append(("carimbo de cratera cavou o meio", crater_shared and crater_lowered and crater_deepest < -1.0))
checks.append(("carimbo de cratera levantou a borda", crater_shared and crater_raised and crater_net < -0.2))
mound_shared, mound_lowered, mound_raised, mound_net, _ = shape_change(crater_path, mound_path)
checks.append(("carimbo de monte subiu sem cavar", mound_shared and mound_raised and not mound_lowered and mound_net > 0.5))

# A crest is long one way and narrow the other; the turn swaps which way. The margin is in meters,
# not a ratio, because the narrow side still measures its own footprint.
ridge_x_long, ridge_x_across = stamp_extent(mound_path, ridge_x_path)
checks.append((
    "carimbo de crista corre ao longo de x",
    ridge_x_long - ridge_x_across > 1.5,
))
ridge_z_across, ridge_z_long = stamp_extent(mound_path, ridge_z_path)
checks.append((
    "carimbo de crista girado 90° corre ao longo de z",
    ridge_z_long - ridge_z_across > 1.5,
))

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

echo "imagem final: $output (passos: $output.tool.png, $output.terrain.png, $output.step.png, $output.erode-mode.png, $output.noise-mode.png, $output.stamp-crater.png, $output.stamp-mound.png, $output.stamp-turned.png, $output.road.png, $output.structures.png, $output.detected.png)"
