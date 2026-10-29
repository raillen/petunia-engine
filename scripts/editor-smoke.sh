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
#   open the Código workspace (F4), create a script, type it (auto-indent) and save it,
#   open the Lógica workspace (F7), create a node graph, add an event and an action from the palettes,
#   wire an input to the event's dt and look at the generated code,
#   look at the same graph as boxes and wires (Caixas e fios), drag a box and check that only
#   scripts/gira.layout.json changed, play with the canvas open (F6) and keep a picture of the
#   highlighted node (the picture is evidence, not an assertion: reading it needs eyes),
#   then play (F6), walk with W, stop (Esc) and save: the scene must not keep the stand-in hero.
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
mkdir -p "$(dirname "$output")"

cleanup() {
    if [[ -n "${editor_pid:-}" ]]; then
        kill "$editor_pid" 2>/dev/null || true
        wait "$editor_pid" 2>/dev/null || true
    fi
    if [[ -n "${xvfb_pid:-}" ]]; then
        kill "$xvfb_pid" 2>/dev/null || true
        wait "$xvfb_pid" 2>/dev/null || true
    fi
    rm -rf "$workdir"
}
trap cleanup EXIT

# Our own virtual display. The asked number is used when free; otherwise the next one, so two
# runs back to back (or another agent) can never fight over :97.
start_display() {
    local first="${1#:}" candidate number
    for number in $(seq "$first" $((first + 30))); do
        candidate=":$number"
        if [[ -e "/tmp/.X11-unix/X$number" ]]; then
            continue
        fi
        Xvfb "$candidate" -screen 0 1440x900x24 +extension GLX >/dev/null 2>&1 &
        xvfb_pid=$!
        for _ in $(seq 1 60); do
            if DISPLAY="$candidate" xdpyinfo >/dev/null 2>&1; then
                display="$candidate"
                return 0
            fi
            sleep 0.1
        done
        kill "$xvfb_pid" 2>/dev/null || true
        wait "$xvfb_pid" 2>/dev/null || true
        xvfb_pid=""
    done
    return 1
}

if ! start_display "${PETUNIA_EDITOR_DISPLAY:-:97}"; then
    echo "falha: nenhum display virtual livre a partir de ${PETUNIA_EDITOR_DISPLAY:-:97}" >&2
    exit 1
fi

# The "+" of a panel opens a name dialog. The workspace may still be laying out, so click, wait
# for the dialog BY NAME and only then type. A modal blocks stray clicks, so retrying is safe.
new_named_item() {
    local title="$1" name="$2" attempt
    for attempt in 1 2 3; do
        xdotool mousemove 186 53 click 1
        for _ in $(seq 1 12); do
            # Anchored: the button's tooltip ("Novo script a partir do modelo") is a window too,
            # and an unanchored regex would take it for the dialog.
            if xdotool search --onlyvisible --name "^${title}$" >/dev/null 2>&1; then
                xdotool type --delay 40 "$name"
                xdotool key Return
                for _ in $(seq 1 20); do
                    xdotool search --onlyvisible --name "^${title}$" >/dev/null 2>&1 || break
                    sleep 0.15
                done
                sleep 0.6
                return 0
            fi
            sleep 0.25
        done
    done
    echo "falha: o diálogo \"$title\" não abriu" >&2
    return 1
}

# Saves and waits for the file to land on disk (the write is asynchronous).
save_until() {
    local path="$1" attempt
    for attempt in 1 2 3; do
        xdotool key ctrl+s
        for _ in $(seq 1 12); do
            [[ -f "$path" ]] && return 0
            sleep 0.2
        done
    done
    return 1
}

env -u WAYLAND_DISPLAY DISPLAY="$display" XDG_CONFIG_HOME="$workdir/config" XDG_CACHE_HOME="$workdir/cache" \
    QT_QPA_PLATFORM=xcb LIBGL_ALWAYS_SOFTWARE=1 QT_LOGGING_RULES='qt.qpa.*=false' \
    "$editor" --sample "$workdir/sala" --screenshot "$output" --screenshot-delay 80000 2>/dev/null &
editor_pid=$!

# Wait for the window, not for a guessed number of seconds.
window_ready=""
for _ in $(seq 1 160); do
    if ! kill -0 "$editor_pid" 2>/dev/null; then
        echo "falha: o editor terminou antes de abrir a janela" >&2
        exit 1
    fi
    if DISPLAY="$display" xdotool search --onlyvisible --name "Sala de exemplo" >/dev/null 2>&1; then
        window_ready="yes"
        break
    fi
    sleep 0.25
done
if [[ -z "$window_ready" ]]; then
    echo "falha: a janela do editor não apareceu" >&2
    exit 1
fi
sleep 1.5   # the first frame drawn

export DISPLAY="$display"
xdotool mousemove 835 440 click 1;               sleep 0.6   # select the center of the big crate (#9)
xdotool key g;                                   sleep 0.6   # Move tool (G on the keyboard; the tool column coordinates are fragile)
xdotool mousemove 875 431;                       sleep 0.5   # over the middle of the X arrow
xdotool mousedown 1                                          # drag the arrow to the right
for x in 895 915 935 955 975 990; do xdotool mousemove "$x" 431; sleep 0.15; done
xdotool mouseup 1;                               sleep 0.6
# Regression: choosing a table in "Adicionar tabela" rebuilds the inspector from inside that
# button's menu. Deleting a widget inside its own signal used to crash Qt (SIGSEGV).
xdotool mousemove 1185 600; for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16; do xdotool click 5; sleep 0.05; done; sleep 0.4
xdotool mousemove 1185 457 click 1;              sleep 0.6   # open the "Adicionar tabela" menu
xdotool key Down Down Down Down Down Return;     sleep 0.8   # choose PlayerStart (list: Character, Floor, Light, Mesh, PlayerStart, ...)
xdotool mousemove 600 450; xdotool key shift+a;  sleep 0.8   # radial menu of primitives
xdotool key 2;                                   sleep 0.8   # item 2: a ramp, on the current floor
xdotool key ctrl+d;                              sleep 0.6   # duplicate it (the copy is selected)
xdotool key ctrl+1;                              sleep 0.6   # new box
xdotool keydown Up; sleep 0.7; xdotool keyup Up; sleep 0.3   # walk forward (arrows always move)
xdotool mousemove 700 300 mousedown 3 mousemove_relative 70 0 mouseup 3; sleep 0.4  # look right
xdotool key ctrl+Prior;                          sleep 0.6   # up a floor: creates Térreo and Andar 1
xdotool key ctrl+1;                              sleep 0.6   # a box on floor 1
xdotool key ctrl+Next;                           sleep 0.6   # back down (the box turns into a ghost)
xdotool key ctrl+k; sleep 0.6; xdotool type --delay 30 "caverna"; sleep 0.6; xdotool key Return; sleep 1.0   # command palette: Caverna (menu positions are fragile)
xdotool mousemove 600 380; xdotool key b;        sleep 0.6   # sculpting tool
xdotool mousedown 1                                          # drag across the cave: carve
for x in 620 640 660 680 700 720; do xdotool mousemove "$x" 380; sleep 0.25; done
xdotool mouseup 1;                               sleep 0.8
xdotool key shift+b;                             sleep 0.6   # path tool: three clicks on the rock, then Enter
xdotool mousemove 560 400 click 1;               sleep 0.5
xdotool mousemove 660 360 click 1;               sleep 0.5
xdotool mousemove 740 420 click 1;               sleep 0.5
xdotool key Return;                              sleep 0.8

xdotool mousemove 130 78 click 1;                sleep 0.4   # hierarchy search: pick the crate by name
xdotool type --delay 20 "Caixa grande";          sleep 0.6
xdotool key Down;                                sleep 0.5   # Down in the filter picks the first object left visible
# The command palette finds the action by name: no menu row counting.
xdotool key ctrl+k; sleep 0.6; xdotool type --delay 30 "modo de edicao"; sleep 0.6; xdotool key Return; sleep 0.9
xdotool key 1;                                   sleep 0.4   # switch to vertex mode
xdotool mousemove 990 431 click 1;               sleep 0.4   # pick component
xdotool key 2;                                   sleep 0.4   # switch to edge mode
xdotool mousemove 990 431 click 1;               sleep 0.4   # pick component
xdotool key 3;                                   sleep 0.4   # switch to face mode
xdotool mousemove 990 431 click 1;               sleep 0.4   # pick component
xdotool key alt+e; sleep 0.6; xdotool key Home Down Down Down Down Down Down Return; sleep 0.8   # exit Edit Mode
# Terrain last: it covers the room, so create it only after the mesh steps. Criar > Terreno from
# the command palette, generate it, focus it and drag the terrain brush across the surface.
xdotool key alt+o; sleep 0.7; xdotool key Return; sleep 1.0   # Operações > Gerar terreno (the shortcut activates the first item)
xdotool key Return;                              sleep 1.2   # the biome dialog: keep "Pela semente"
xdotool key f;                                   sleep 0.8   # focus the selected terrain
xdotool mousemove 600 400 mousedown 1
for x in 620 640 660 680; do xdotool mousemove "$x" 400; sleep 0.2; done
xdotool mouseup 1;                               sleep 0.8
xdotool key ctrl+s;                              sleep 0.6   # save the scene
xdotool mousemove 385 649 click 1;               sleep 0.4   # open the history tab
xdotool key F4;                                  sleep 1.2   # the Código workspace opens
new_named_item "Novo script" smoke                          # + (new script) in the Scripts list
xdotool mousemove 600 300 click 1;               sleep 0.3   # focus the code editor
xdotool key ctrl+a; xdotool key Delete;          sleep 0.3
xdotool type --delay 25 "fn on_update(entity, dt) {"; xdotool key Return
xdotool type --delay 25 "log.info(\"oi\", entity)"; xdotool key Return
xdotool type --delay 25 "}";                     sleep 0.6
save_until "$workdir/sala/scripts/smoke.aipo" || true            # saves the script to scripts/smoke.aipo
xdotool mousemove 632 17 click 1;                sleep 1.2   # the Lógica workspace tab (node graphs as an outline; F7 on the keyboard,
                                                              # but xdotool's typing remaps keys, so the test clicks)
new_named_item "Novo grafo" gira                             # + (new graph)
xdotool mousemove 252 51 click 1;                sleep 1.0   # Evento: the palette
xdotool type --delay 40 quadro;                  sleep 0.6
xdotool key Return;                              sleep 1.0   # A cada quadro
xdotool mousemove 360 51 click 1;                sleep 1.0   # Adicionar depois: the palette
xdotool type --delay 40 "girar y";               sleep 0.6
xdotool key Return;                              sleep 1.0   # Girar no Y, after the event
xdotool mousemove 360 146 click 1;               sleep 0.5   # the "Graus" input row
xdotool key Return;                              sleep 0.6   # its menu
xdotool key Down Down Return;                    sleep 0.6   # "Usar o valor de…"
xdotool key Return;                              sleep 1.0   # the first source: dt of the event
xdotool mousemove 1153 51 click 1;               sleep 0.8   # Ver como código
xdotool mousemove 925 51 click 1;                sleep 0.8   # Caixas e fios: the same graph as boxes
import -window root "$output.canvas.png"                     # one picture of the canvas
xdotool mousemove 600 330 mousedown 1;                       # drag the event box
for y in 320 310 300 292; do xdotool mousemove 640 "$y"; sleep 0.15; done
xdotool mouseup 1;                               sleep 0.8
xdotool mousemove 1350 51 click 1;               sleep 0.8   # Ligar ao objeto: the graph now belongs to the selected object
xdotool key F6;                                  sleep 1.6   # Jogar with the canvas open: marked boxes
import -window root "$output.playing.png"                    # evidence: what ran, in the canvas
xdotool key F6;                                  sleep 1.0   # stop it again (F6 toggles, focus is on the grafo)
xdotool key F3;                                  sleep 0.8   # back to the Cena workspace
xdotool mousemove 700 300 click 1;               sleep 0.5   # focus the viewport
xdotool key F6;                                  sleep 1.5   # Jogar: the scene runs on a COPY (a stand-in hero is made there)
xdotool keydown w; sleep 1.0; xdotool keyup w;   sleep 0.3   # walk
xdotool key Escape;                              sleep 1.0   # Parar: the copy is thrown away
xdotool key ctrl+s;                              sleep 0.8   # save the scene again: it must not contain the hero

editor_status=0
wait "$editor_pid" || editor_status=$?
echo "editor terminou com código $editor_status"
cp "$workdir/sala/scenes/room.scene.json" "$output.scene.json"
cp "$workdir/sala/scripts/smoke.aipo" "$output.script.aipo" 2>/dev/null || true
cp "$workdir/sala/scripts/gira.graph.json" "$output.graph.json" 2>/dev/null || true
cp "$workdir/sala/scripts/gira.layout.json" "$output.layout.json" 2>/dev/null || true

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
terrains = [e for e in scene["entities"] if "Terrain" in e["rows"]]
terrain_chunks = len(terrains[0]["rows"]["Terrain"]["data"].get("chunks", [])) if terrains else 0
terrain_ok = len(terrains) == 1 and terrain_chunks >= 1
print(f"verificação: terreno criado e gerado ({terrain_chunks} blocos) ->", "ok" if terrain_ok else "FALHOU")
import os
script_path = sys.argv[1].replace(".scene.json", ".script.aipo")
scripted = os.path.exists(script_path) and "fn on_update(entity, dt) {\n    log.info(" in open(script_path).read()
print("verificação: script escrito no editor de código e salvo em scripts/ ->", "ok" if scripted else "FALHOU")
graph_path = sys.argv[1].replace(".scene.json", ".graph.json")
graph_ok = False
if os.path.exists(graph_path):
    graph = json.load(open(graph_path))
    kinds = sorted(node["kind"] for node in graph["nodes"])
    wired = any(edge["from"]["pin"] == "dt" and edge["to"]["pin"] == "degrees" for edge in graph["edges"])
    graph_ok = kinds == ["api.transform.rotate_y", "event.on_update"] and wired
print("verificação: grafo de nós criado, com evento, ação e o dt ligado em Graus (arquivo .graph.json) ->", "ok" if graph_ok else "FALHOU")
# The canvas drags boxes without touching the logic: positions live in their own file.
layout_path = sys.argv[1].replace(".scene.json", ".layout.json")
moved_box = False
if os.path.exists(layout_path) and os.path.exists(graph_path):
    layout = json.load(open(layout_path))
    places = layout.get("nodes", {})
    # Only the box that moved is saved, and its place is no longer the grid the editor computes.
    moved_box = (
        layout.get("schema_version") == 1
        and len(places) >= 1
        and all(isinstance(p.get("x"), (int, float)) and isinstance(p.get("y"), (int, float)) for p in places.values())
        and any(p["x"] % 300 != 0 or p["y"] % 180 != 0 for p in places.values())
    )
    graph = json.load(open(graph_path))
    moved_box = moved_box and all("x" not in n and "y" not in n for n in graph["nodes"])
print("verificação: caixa arrastada no canvas mudou só scripts/gira.layout.json ->", "ok" if moved_box else "FALHOU")
converted_to_mesh = any(
    e["rows"].get("Name", {}).get("text") in ("Caixa", "Caixa grande") and "Mesh" in e["rows"] and "Brush" not in e["rows"]
    for e in scene["entities"]
)
print("verificação: modo de edição (Tab) converteu caixa para malha editável ->", "ok" if converted_to_mesh else "FALHOU")
hero_free = not any(e["rows"].get("Name", {}).get("text") == "Jogador" for e in scene["entities"]) and not any("Character" in e["rows"] for e in scene["entities"])
print("verificação: Jogar (F6) e parar (Esc) não deixaram o herói de reserva na cena salva ->", "ok" if hero_free else "FALHOU")
sys.exit(0 if (moved and added and on_cursor and built_up and carved and dug and terrain_ok and scripted and graph_ok and moved_box and converted_to_mesh and hero_free) else 1)
PY
echo "imagem: $output (e $output.viewport.png, $output.canvas.png, $output.playing.png); cena salva: $output.scene.json"
if [[ "$editor_status" -ne 0 ]]; then
    echo "falha: o editor saiu com código $editor_status" >&2
    exit 1
fi
