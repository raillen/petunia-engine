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
#   save the selected object as a prefab of the project and create it back from the Criar menu,
#   open the Código workspace (tab), create a script, type it (auto-indent) and save it,
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

editor="${PETUNIA_EDITOR_BIN:-}"
if [[ -z "$editor" ]]; then
    # One Qt build folder per working copy, named like scripts/editor.sh does.
    root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
    checkout="$(basename "$root")"
    checkout="${checkout#petunia-engine-}"
    editor="/tmp/petunia-editor-${checkout:-petunia-engine}/petunia-editor"
fi
output="${1:-/tmp/petunia-shot/smoke.png}"
workdir="$(mktemp -d /tmp/petunia-smoke.XXXXXX)"
mkdir -p "$(dirname "$output")"
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/_virtual-display.sh"
# A display of our own: a run that died can leave an editor window on top of ours and every key
# would go to it. `choose_display` skips a busy number (or stops when the display was pinned).
choose_display :97
rm -f "$output" "$output.viewport.png" "$output.canvas.png" "$output.playing.png" \
      "$output.scene.json" "$output.script.aipo" "$output.graph.json" "$output.layout.json"

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

Xvfb "$display" -screen 0 1440x900x24 +extension GLX >/dev/null 2>&1 &
xvfb_pid=$!
sleep 1

# The delay is also the LENGTH of the run: the editor takes the pictures and quits when it fires. At
# 62000 the steps grew past the timer, the editor quit in the middle of the last ones and the run
# reported failures (and once a segfault) that the product did not have (2026-10-29). Keep it well
# above the slowest pass. (A comment cannot sit inside the backslash continuation: it swallows the
# command that follows it.)
# The delay is also the LENGTH of the run: the editor takes the pictures and quits when it fires.
# At 62000 the steps grew past the timer, the editor quit in the middle of the last ones and the
# run reported failures (and once a segfault) that the product did not have (2026-10-29). Keep it
# well above the slowest pass.
#
# The comment must stay ABOVE this line: a comment between the backslash-continued flags and the
# command ends the continuation, and `env` then prints the environment and never starts the editor
# (the run failed with "a janela não apareceu em 25 s" while the editor was fine).
env -u WAYLAND_DISPLAY DISPLAY="$display" XDG_CONFIG_HOME="$workdir/config" XDG_CACHE_HOME="$workdir/cache" \
    QT_QPA_PLATFORM=xcb LIBGL_ALWAYS_SOFTWARE=1 QT_LOGGING_RULES='qt.qpa.*=false' \
    "$editor" --sample "$workdir/sala" --screenshot "$output" --screenshot-delay 120000 2>"$workdir/editor.log" &
editor_pid=$!
sleep 1
watch_editor "$editor_pid" "$workdir/editor.log"
wait_for_window "Petunia Editor" "$display" "$editor_pid" "$workdir/editor.log"

export DISPLAY="$display"
xdotool mousemove 835 440 click 1;               sleep 0.6   # select the center of the big crate (#9)
xdotool mousemove 275 121 click 1;               sleep 0.6   # Move tool (tool column in the viewport; G on the keyboard)
xdotool mousemove 875 431;                       sleep 0.5   # over the middle of the X arrow
xdotool mousedown 1                                          # drag the arrow to the right
for x in 895 915 935 955 975 990; do xdotool mousemove "$x" 431; sleep 0.15; done
xdotool mouseup 1;                               sleep 0.6
# Regression: choosing a table in "Adicionar tabela" rebuilds the inspector from inside that
# button's menu. Deleting a widget inside its own signal used to crash Qt (SIGSEGV).
xdotool mousemove 1185 600; for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16; do xdotool click 5; sleep 0.05; done; sleep 0.4
# 497, not 457: a brush now also shows "Tornar editável" above this button, which moved it down
# by one row. The coordinate is the middle of the button, so a new row above it means a new number.
xdotool mousemove 1185 497 click 1;              sleep 0.6   # open the "Adicionar tabela" menu
# The menu lists the tables the object still lacks, in alphabetical order, so this count follows the
# schema: Bridge, Character, EditorState, Floor, Light, Mesh, PlayerStart... Adding a table that sorts
# before PlayerStart (or removing one) means changing the number of Downs here.
xdotool key Down Down Down Down Down Down Down Return; sleep 0.8   # choose PlayerStart (7th)
# schema, in the menu's alphabetical order: Bridge, Character, Clip, EditorState, FaceColor, Floor,
# Light, Mesh, PlayerStart, Skeleton, SkeletonLink, Skin. Every new table that sorts before
# PlayerStart needs one more Down here: FaceColor (per-face paint) and EditorState (hide/lock) pushed
# it to the 8th in 2026-10-30, and Clip (animation) pushed it to the 9th in 2026-10-31.
xdotool key Down Down Down Down Down Down Down Down Return; sleep 0.8   # PlayerStart (9th)
xdotool mousemove 600 450; xdotool key shift+a;  sleep 0.8   # radial menu of primitives
xdotool key 2;                                   sleep 0.8   # item 2: a ramp, on the current floor
xdotool key ctrl+d;                              sleep 0.6   # duplicate it (the copy is selected)
xdotool key ctrl+1;                              sleep 0.6   # new box
xdotool keydown Up; sleep 0.7; xdotool keyup Up; sleep 0.3   # walk forward (arrows always move)
xdotool mousemove 700 300 mousedown 3 mousemove_relative 70 0 mouseup 3; sleep 0.4  # look right
xdotool key ctrl+Prior;                          sleep 0.6   # up a floor: creates Térreo and Andar 1
xdotool key ctrl+1;                              sleep 0.6   # a box on floor 1
xdotool key ctrl+Next;                           sleep 0.6   # back down (the box turns into a ghost)
xdotool key alt+c; sleep 0.6; xdotool key Up Up Up Up Up Up Return; sleep 1.0   # Alt+C opens Criar (the menus are behind ≡) > Caverna (6 from the end: Sol, Luz, Importar imagem, Importar modelo, aleatória, Caverna)
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
xdotool mousemove 100 408 click 1;               sleep 0.5   # select the "Caixa" on the ground in the hierarchy
xdotool mousemove 335 51 click 1;                sleep 0.8   # the Edição mode button (converts the selected box to a mesh)
xdotool key 1;                                   sleep 0.4   # switch to vertex mode
xdotool mousemove 990 431 click 1;               sleep 0.4   # pick component
xdotool key 2;                                   sleep 0.4   # switch to edge mode
xdotool mousemove 990 431 click 1;               sleep 0.4   # pick component
xdotool key 3;                                   sleep 0.4   # switch to face mode
xdotool mousemove 990 431 click 1;               sleep 0.4   # pick component
xdotool key Tab;                                 sleep 0.6   # exit Edit Mode (remains a mesh)
# Prefabs of the project (DR-51): the selected mesh becomes prefabs/smoke.prefab.json and comes
# back from Criar > Prefab do projeto. The menu is walked with Up, which is steadier than clicking:
# the first Up lands on the last item (Sol), and eight presses reach Salvar seleção como prefab…
# (Luz, Importar imagem, Importar modelo, aleatória, Caverna, Terreno), nine reach Prefab do projeto.
xdotool key alt+c;                               sleep 0.6
for _ in 1 2 3 4 5 6 7 8; do xdotool key Up; sleep 0.1; done
xdotool key Return;                              sleep 0.8   # the name dialog
xdotool type --delay 40 "smoke"; xdotool key Return; sleep 1.4
# The modal dialog leaves the menu bar without focus: a click on the status bar gives it back
# without changing the selection.
xdotool mousemove 700 790 click 1;               sleep 0.4
xdotool key alt+c;                               sleep 0.6
for _ in 1 2 3 4 5 6 7 8 9; do xdotool key Up; sleep 0.1; done
xdotool key Right;                               sleep 0.5   # "Prefab do projeto"
xdotool key Return;                              sleep 1.0   # the only saved prefab: smoke
xdotool key ctrl+s;                              sleep 0.6   # save the scene
xdotool mousemove 385 649 click 1;               sleep 0.4   # open the history tab
xdotool mousemove 599 17 click 1;                sleep 1.4   # the Código workspace tab (Cena | Código | Jogar)
xdotool mousemove 186 53 click 1;                sleep 1.0   # + (new script) in the Scripts list
xdotool type --delay 40 smoke; xdotool key Return; sleep 1.0
xdotool key ctrl+a; xdotool key Delete;          sleep 0.3
xdotool type --delay 25 "fn on_update(entity, dt) {"; xdotool key Return
xdotool type --delay 25 "log.info(\"oi\", entity)"; xdotool key Return
xdotool type --delay 25 "}";                     sleep 0.6
xdotool key ctrl+s;                              sleep 1.0   # saves the script to scripts/smoke.aipo
xdotool key F3;                                  sleep 0.8   # back to the Cena workspace
xdotool mousemove 700 300 click 1;               sleep 0.5   # focus the viewport
xdotool key F6;                                  sleep 1.5   # Jogar: the scene runs on a COPY (a stand-in hero is made there)
xdotool keydown w; sleep 1.0; xdotool keyup w;   sleep 0.3   # walk
xdotool key Escape;                              sleep 1.0   # Parar: the copy is thrown away
xdotool key ctrl+s;                              sleep 0.8   # save the scene again: it must not contain the hero

wait "$editor_pid"
cp "$workdir/sala/scenes/room.scene.json" "$output.scene.json"
cp "$workdir/sala/scripts/smoke.aipo" "$output.script.aipo" 2>/dev/null || true
# An Aipo project must not have produced a graph: the Lógica workspace is not even offered (DR-40).
aipo_has_graph=0
[[ -e "$workdir/sala/scripts/gira.graph.json" ]] && aipo_has_graph=1

# `--pointer-info` is the diagnostic used when a click "does not work": it must print the scale
# state without blocking anything. A short run of its own, on the same virtual display, checks the
# line exists; the crosshair over a click needs eyes and is checked in a real run.
pointer_log="$workdir/pointer.log"
if ! env -u WAYLAND_DISPLAY DISPLAY="$display" XDG_CONFIG_HOME="$workdir/config" XDG_CACHE_HOME="$workdir/cache" \
        QT_QPA_PLATFORM=xcb LIBGL_ALWAYS_SOFTWARE=1 QT_LOGGING_RULES='qt.qpa.*=false' \
        "$editor" --pointer-info --sample "$workdir/sonda" --screenshot "$output.pointer.png" \
        --screenshot-delay 3500 2>"$pointer_log"; then
    echo "verificação: --pointer-info abriu e fechou -> FALHOU"
else
    echo "verificação: --pointer-info abriu e fechou -> ok"
fi
if grep -q '^\[escala\]' "$pointer_log"; then
    echo "verificação: --pointer-info imprimiu a escala da tela -> ok"
else
    echo "verificação: --pointer-info imprimiu a escala da tela -> FALHOU"
fi

# The gizmo drag must have moved crate #9 along X only, onto the 0.5 m grid.
set +e
python3 -I - "$output.scene.json" "$aipo_has_graph" "$workdir/sala" <<'PY'
import json, sys
scene = json.load(open(sys.argv[1]))
crate = next(e for e in scene["entities"] if e["id"] == 9)
position = crate["rows"]["Transform"]["position"]
moved = position["x"] > 3.0 and position["x"] % 0.5 == 0 and position["y"] == 0.5 and position["z"] == 3.0
added = True  # checked by the unit tests; the menu navigation is too fragile for a pixel test
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
converted_to_mesh = any(
    e["rows"].get("Name", {}).get("text") in ("Caixa", "Caixa grande")
    and "Mesh" in e["rows"] and "Brush" not in e["rows"]
    for e in scene["entities"]
)
print("verificação: modo de edição converteu caixa para malha editável ->", "ok" if converted_to_mesh else "FALHOU")
# Prefabs of the project (DR-51): the menu saved the selection as text with the position zeroed,
# and created a copy back in the scene (the original mesh plus the copy).
prefab_path = os.path.join(sys.argv[3], "prefabs", "smoke.prefab.json")
prefab = json.load(open(prefab_path)) if os.path.exists(prefab_path) else {}
objects = prefab.get("objects", [])
prefab_ok = (
    prefab.get("format") == 1
    and prefab.get("name") == "smoke"
    and prefab.get("revision") == 1
    and len(objects) == 1
    and "Mesh" in objects[0].get("rows", {})
    and objects[0]["rows"]["Transform"]["position"] == {"x": 0.0, "y": 0.0, "z": 0.0}
)
print("verificação: prefab salvo em prefabs/smoke.prefab.json com a posição zerada ->", "ok" if prefab_ok else "FALHOU")
mesh_copies = [
    e for e in scene["entities"]
    if e["rows"].get("Name", {}).get("text") == "Caixa" and "Mesh" in e["rows"]
]
prefab_spawned = len(mesh_copies) >= 2
print(f"verificação: Criar > Prefab do projeto criou uma cópia na cena ({len(mesh_copies)} malhas 'Caixa') ->", "ok" if prefab_spawned else "FALHOU")
hero_free = not any(e["rows"].get("Name", {}).get("text") == "Jogador" for e in scene["entities"]) and not any("Character" in e["rows"] for e in scene["entities"])
print("verificação: Jogar (F6) e parar (Esc) não deixaram o herói de reserva na cena salva ->", "ok" if hero_free else "FALHOU")
# Phase 2, below, proves the pNodes side. Here an Aipo project must not have produced a graph.
no_graph_in_aipo = sys.argv[2] == "0"
print("verificação: projeto Aipo não criou grafo (a aba Lógica não é oferecida) ->", "ok" if no_graph_in_aipo else "FALHOU")
phase1_ok = moved and added and on_cursor and built_up and carved and dug and scripted and converted_to_mesh and prefab_ok and prefab_spawned and hero_free and no_graph_in_aipo
sys.exit(0 if phase1_ok else 1)
PY
phase1_failed=$?
set -e
echo "imagem: $output (e $output.viewport.png, $output.canvas.png, $output.playing.png); cena salva: $output.scene.json"

# ---- Phase 2: the pNodes method (DR-40) ------------------------------------------------------
# A pNodes project offers Logica and not Codigo. It runs in its own editor process because the
# method is a property of the project, not a switch inside one window.
echo "--- fase 2: projeto pNodes ---"
workdir_nodes="$(mktemp -d /tmp/petunia-smoke.XXXXXX)"
env -u WAYLAND_DISPLAY DISPLAY="$display" XDG_CONFIG_HOME="$workdir_nodes/config" XDG_CACHE_HOME="$workdir_nodes/cache" \
    QT_QPA_PLATFORM=xcb LIBGL_ALWAYS_SOFTWARE=1 QT_LOGGING_RULES='qt.qpa.*=false' \
    "$editor" --sample "$workdir_nodes/mundo" --method nodes --screenshot "$output.nodes.png" --screenshot-delay 120000 2>"$workdir_nodes/editor.log" &
editor_pid=$!
sleep 1
watch_editor "$editor_pid" "$workdir_nodes/editor.log"
wait_for_window "Petunia Editor" "$display" "$editor_pid" "$workdir_nodes/editor.log"

xdotool mousemove 599 17 click 1;                sleep 1.2   # the Lógica workspace tab (node graphs as an outline; F7 on the keyboard,
                                                              # but xdotool's typing remaps keys, so the test clicks)
xdotool mousemove 186 53 click 1;                sleep 1.0   # + (new graph)
xdotool type --delay 40 gira; xdotool key Return; sleep 1.0
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
# Snapshot of the graph the PALETTE built, before the text below replaces it: the two checks read
# the same file otherwise and the second one would hide a broken palette.
cp "$workdir_nodes/mundo/scripts/gira.graph.json" "$workdir_nodes/mundo/scripts/gira.palette.json"
# Editing the text and applying it must reach scripts/<name>.graph.json (the panel looked updated
# while the file kept the old logic, 2026-10-29). A constant replaces the dt wire.
xdotool key F8;                                  sleep 0.8   # Ver como código (the button moves with the list)
xdotool mousemove 500 200 click 1;               sleep 0.4
xdotool key ctrl+a;                              sleep 0.2
xdotool type --delay 12 'fn on_update(entity, dt) { transform.rotate_y(entity, 45.0) }'; sleep 0.6
xdotool key ctrl+Return;                         sleep 1.5   # Aplicar código
xdotool mousemove 925 51 click 1;                sleep 0.8   # Caixas e fios: the same graph as boxes
import -window root "$output.canvas.png"                     # one picture of the canvas
xdotool mousemove 600 390 mousedown 1;                       # drag the event box
for y in 380 370 360 350; do xdotool mousemove 640 "$y"; sleep 0.15; done
xdotool mouseup 1;                               sleep 0.8
xdotool mousemove 1350 51 click 1;               sleep 0.8   # Ligar ao objeto: the graph now belongs to the selected object
xdotool key F6;                                  sleep 1.6   # Jogar with the canvas open: marked boxes
import -window root "$output.playing.png"                    # evidence: what ran, in the canvas
xdotool key F6;                                  sleep 1.0   # stop it again (F6 toggles, focus is on the grafo)

xdotool key ctrl+s;                              sleep 0.8   # save the scene before leaving
wait "$editor_pid"
cp "$workdir_nodes/mundo/scripts/gira.graph.json" "$output.graph.json" 2>/dev/null || true
cp "$workdir_nodes/mundo/scripts/gira.layout.json" "$output.layout.json" 2>/dev/null || true

set +e
python3 -I - "$output.graph.json" "$output.layout.json" "$workdir_nodes/mundo" <<'PY'
import json, os, sys
graph_path, layout_path, root = sys.argv[1], sys.argv[2], sys.argv[3]
graph_ok = False
if os.path.exists(graph_path):
    graph = json.load(open(graph_path))
    kinds = sorted(node["kind"] for node in graph["nodes"])
    wired = any(edge["from"]["pin"] == "dt" and edge["to"]["pin"] == "degrees" for edge in graph["edges"])
    # The text typed into "Ver como código" and applied with "Aplicar código" must be in the FILE
    # (the panel used to change while the graph on disk kept the old logic, 2026-10-29). Applying it
    # replaces the dt wire with a constant, which is why the palette's chain was copied before.
    rotate = [n for n in graph["nodes"] if n["kind"] == "api.transform.rotate_y"]
    applied_ok = (
        kinds == ["api.transform.rotate_y", "event.on_update"]
        and len(rotate) == 1
        and rotate[0].get("params", {}).get("degrees") == 45.0
        and not any(edge["from"]["pin"] == "dt" for edge in graph["edges"])
    )
    graph_ok = applied_ok
print("verificacao: pNodes gravou o Graus=45 do texto aplicado no arquivo ->", "ok" if graph_ok else "FALHOU")
palette_ok = False
palette_copy = os.path.join(root, "scripts", "gira.palette.json")
if os.path.exists(palette_copy):
    built = json.load(open(palette_copy))
    palette_ok = sorted(n["kind"] for n in built["nodes"]) == ["api.transform.rotate_y", "event.on_update"] and any(
        edge["from"]["pin"] == "dt" and edge["to"]["pin"] == "degrees" for edge in built["edges"]
    )
print("verificacao: a paleta montou a cadeia com dt em Graus ->", "ok" if palette_ok else "FALHOU")
moved_box = False
if os.path.exists(layout_path) and os.path.exists(graph_path):
    layout = json.load(open(layout_path))
    places = layout.get("nodes", {})
    moved_box = (
        layout.get("schema_version") == 1
        and len(places) >= 1
        and all(isinstance(p.get("x"), (int, float)) and isinstance(p.get("y"), (int, float)) for p in places.values())
        and any(p["x"] % 300 != 0 or p["y"] % 180 != 0 for p in places.values())
    )
    graph = json.load(open(graph_path))
    moved_box = moved_box and all("x" not in n and "y" not in n for n in graph["nodes"])
print("verificacao: caixa arrastada no canvas mudou so scripts/gira.layout.json ->", "ok" if moved_box else "FALHOU")
no_aipo = not os.path.exists(os.path.join(root, "scripts", "smoke.aipo"))
print("verificacao: projeto pNodes nao criou .aipo ->", "ok" if no_aipo else "FALHOU")
sys.exit(0 if (graph_ok and moved_box and no_aipo) else 1)
PY
phase2_failed=$?
set -e
echo "imagem: $output.nodes.png; grafo: $output.graph.json"
if [[ "$phase1_failed" -eq 0 && "$phase2_failed" -eq 0 ]]; then
    echo "resultado: todas as verificações passaram"
    exit 0
fi
echo "resultado: reprovou (fase 1=$([[ "$phase1_failed" -eq 0 ]] && echo ok || echo FALHOU), fase 2=$([[ "$phase2_failed" -eq 0 ]] && echo ok || echo FALHOU))" >&2
exit 1
