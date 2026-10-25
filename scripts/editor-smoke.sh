#!/usr/bin/env bash
# Smoke test of the Qt editor in a VIRTUAL display (Xvfb). Nothing appears on your desktop and
# your editor settings are not touched.
#
# It opens the sample room and drives the real window with xdotool:
#   click a crate to select it, pick the Move tool, drag the X arrow of the gizmo,
#   add a table from the inspector combo (crash regression), create a ramp from the Shift+A
#   radial menu, duplicate, create a box, walk with the arrow keys, look around with the right button,
#   go up a floor (creates the floors) and build a box there, come back down, create a cave and
#   carve it with the sculpting tool, dig a tunnel with the path tool, save, open the history tab,
#   open the code editor (F4), create a script, type it (auto-indent) and save it.
# It then CHECKS the saved scene (the dragged crate must have moved to x = 4.5 on the 0.5 m grid
# and must have gained the PlayerStart table)
# and writes one picture. This is a smoke test (does it run, draw and edit?), not a pixel-exact
# visual regression test.
#
# The pixel coordinates below match the window layout of this build at its default size.
#
#   scripts/editor-smoke.sh [output.png]
set -euo pipefail

editor="${PETUNIA_EDITOR_BIN:-/tmp/petunia-editor-build/petunia-editor}"
output="${1:-/tmp/petunia-shot/smoke.png}"
workdir="$(mktemp -d /tmp/petunia-smoke.XXXXXX)"
display=":97"
mkdir -p "$(dirname "$output")"

cleanup() {
    [[ -n "${editor_pid:-}" ]] && kill "$editor_pid" 2>/dev/null || true
    [[ -n "${xvfb_pid:-}" ]] && kill "$xvfb_pid" 2>/dev/null || true
    rm -rf "$workdir"
}
trap cleanup EXIT

Xvfb "$display" -screen 0 1440x900x24 +extension GLX >/dev/null 2>&1 &
xvfb_pid=$!
sleep 1

env -u WAYLAND_DISPLAY DISPLAY="$display" XDG_CONFIG_HOME="$workdir/config" \
    QT_QPA_PLATFORM=xcb LIBGL_ALWAYS_SOFTWARE=1 QT_LOGGING_RULES='qt.qpa.*=false' \
    "$editor" --sample "$workdir/sala" --screenshot "$output" --screenshot-delay 38000 2>/dev/null &
editor_pid=$!
sleep 3

export DISPLAY="$display"
xdotool mousemove 800 410 click 1;               sleep 0.6   # select the big crate (#9)
xdotool mousemove 549 17 click 1;                sleep 0.6   # Move tool (top bar; G on the keyboard)
xdotool mousemove 843 409;                       sleep 0.5   # over the middle of the X arrow
xdotool mousedown 1                                          # drag the arrow to the right
for x in 863 883 903 923 943 958; do xdotool mousemove "$x" 409; sleep 0.15; done
xdotool mouseup 1;                               sleep 0.6
# Regression: choosing a table in "Adicionar tabela" rebuilds the inspector from inside that
# button's menu. Deleting a widget inside its own signal used to crash Qt (SIGSEGV).
xdotool mousemove 1180 330; for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16; do xdotool click 5; sleep 0.05; done; sleep 0.4
xdotool mousemove 1050 523 click 1;              sleep 0.6   # open the "Adicionar tabela" menu
xdotool key Down Down Return;                    sleep 0.8   # choose PlayerStart (list: Floor, PlayerStart)
xdotool mousemove 600 450; xdotool key shift+a;  sleep 0.8   # radial menu of primitives
xdotool key 2;                                   sleep 0.8   # item 2: a ramp, on the current floor
xdotool key ctrl+d;                              sleep 0.6   # duplicate it (the copy is selected)
xdotool key ctrl+1;                              sleep 0.6   # new box
xdotool keydown Up; sleep 0.7; xdotool keyup Up; sleep 0.3   # walk forward (arrows always move)
xdotool mousemove 700 300 mousedown 3 mousemove_relative 70 0 mouseup 3; sleep 0.4  # look right
xdotool key ctrl+Prior;                          sleep 0.6   # up a floor: creates Térreo and Andar 1
xdotool key ctrl+1;                              sleep 0.6   # a box on floor 1
xdotool key ctrl+Next;                           sleep 0.6   # back down (the box turns into a ghost)
xdotool key alt+c; sleep 0.6; xdotool key Up Up Return; sleep 1.0   # Criar > Caverna (the item before the random one)
xdotool mousemove 600 380; xdotool key b;        sleep 0.6   # sculpting tool
xdotool mousedown 1                                          # drag across the cave: carve
for x in 620 640 660 680 700 720; do xdotool mousemove "$x" 380; sleep 0.25; done
xdotool mouseup 1;                               sleep 0.8
xdotool key shift+b;                             sleep 0.6   # path tool: three clicks on the rock, then Enter
xdotool mousemove 560 400 click 1;               sleep 0.5
xdotool mousemove 660 360 click 1;               sleep 0.5
xdotool mousemove 740 420 click 1;               sleep 0.5
xdotool key Return;                              sleep 0.8
xdotool key v;                                   sleep 0.3   # back to select
xdotool key ctrl+s;                              sleep 0.6   # save the scene
xdotool mousemove 393 610 click 1;               sleep 0.4   # open the history tab
xdotool key F4;                                  sleep 1.2   # the code editor opens
xdotool mousemove 441 465 click 1;               sleep 1.0   # Novo
xdotool type --delay 40 smoke; xdotool key Return; sleep 1.0
xdotool key ctrl+a; xdotool key Delete;          sleep 0.3
xdotool type --delay 25 "fn on_update(entity, dt) {"; xdotool key Return
xdotool type --delay 25 "log.info(\"oi\", entity)"; xdotool key Return
xdotool type --delay 25 "}";                     sleep 0.6
xdotool key ctrl+s;                              sleep 1.0   # saves the script to scripts/smoke.aipo

wait "$editor_pid"
cp "$workdir/sala/scenes/room.scene.json" "$output.scene.json"
cp "$workdir/sala/scripts/smoke.aipo" "$output.script.aipo" 2>/dev/null || true

# The gizmo drag must have moved crate #9 along X only, onto the 0.5 m grid.
python3 -I - "$output.scene.json" <<'PY'
import json, sys
scene = json.load(open(sys.argv[1]))
crate = next(e for e in scene["entities"] if e["id"] == 9)
position = crate["rows"]["Transform"]["position"]
moved = position["x"] > 3.0 and position["x"] % 0.5 == 0 and position["y"] == 0.5 and position["z"] == 3.0
added = "PlayerStart" in crate["rows"]
ramps = [e for e in scene["entities"] if e["id"] > 11 and e["rows"].get("Name", {}).get("text") == "Rampa"]
on_cursor = bool(ramps) and ramps[0]["rows"]["Transform"]["position"]["y"] == 1.0
print(f"verificação: caixa #9 em x={position['x']} y={position['y']} z={position['z']} (gizmo) ->", "ok" if moved else "FALHOU")
print(f"verificação: tabela PlayerStart adicionada pelo menu do inspetor ->", "ok" if added else "FALHOU")
print(f"verificação: rampa do menu radial apoiada no andar 0 (y=1.0) ->", "ok" if on_cursor else "FALHOU")
floors = sorted((e["rows"]["Floor"]["elevation"], e["rows"]["Name"]["text"]) for e in scene["entities"] if "Floor" in e["rows"] and "Brush" not in e["rows"])
upstairs = [e for e in scene["entities"] if e["rows"].get("Name", {}).get("text") == "Caixa" and e["rows"]["Transform"]["position"]["y"] == 4.0]
built_up = floors == [(0.0, "Térreo"), (3.0, "Andar 1")] and bool(upstairs)
print(f"verificação: Ctrl+PageUp criou {floors} e a caixa nasceu no andar 1 ->", "ok" if built_up else "FALHOU")
caves = [e["id"] for e in scene["entities"] if "Sculpt" in e["rows"]]
strokes = [e for e in scene["entities"] if "SculptStroke" in e["rows"]]
carved = len(caves) == 1 and len(strokes) >= 3 and all(s["rows"]["SculptStroke"]["volume"] == caves[0] for s in strokes)
print(f"verificação: caverna criada e esculpida ({len(strokes)} traços) ->", "ok" if carved else "FALHOU")
# 1 tunnel + at least 2 dabs from the drag + at least 3 capsules from the path (Enter dug it).
dug = len(strokes) >= 6
print(f"verificação: túnel por caminho cavado com Enter ({len(strokes)} traços no total) ->", "ok" if dug else "FALHOU")
import os
script_path = sys.argv[1].replace(".scene.json", ".script.aipo")
scripted = os.path.exists(script_path) and "fn on_update(entity, dt) {\n    log.info(" in open(script_path).read()
print("verificação: script escrito no editor de código e salvo em scripts/ ->", "ok" if scripted else "FALHOU")
sys.exit(0 if (moved and added and on_cursor and built_up and carved and dug and scripted) else 1)
PY
echo "imagem: $output (e $output.viewport.png); cena salva: $output.scene.json"
