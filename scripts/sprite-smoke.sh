#!/usr/bin/env bash
# Visual test of the SPRITE PASS (DR-38, DR-39) in a VIRTUAL display: builds a tiny project whose
# scene has one sprite showing a 2x1 SHEET in front of a 3D box, opens it in the editor, and reads
# the pixels of the viewport picture. Nothing appears on your desktop and your editor settings are
# not touched.
#
#   scripts/sprite-smoke.sh [output.png]
#
# The sheet is magenta on the left frame and cyan on the right one, and the sprite's row says frame
# 1 (the right one). The picture is the evidence; four pixels of it are read:
#   - the middle of the sprite must be CYAN: the picture reaches the screen, textured and unlit,
#     over the 3D box behind it, and the row chose the frame (frame 0 would be magenta);
#   - a point a quarter from the centre must be cyan too: the whole quad shows the chosen frame,
#     not half of the sheet;
#   - the picture's 2-pixel transparent border must let the box show through (the alpha cut-off
#     works: a quad that painted its whole rectangle would be coloured here);
#   - a point well outside the quad must not be coloured (the quad is bounded and placed where the
#     scene says, not stretched over the screen).
#
# A missing GL context is a FAILURE, never a silent pass (AGENTS.md, layered verification).
set -euo pipefail

editor="${PETUNIA_EDITOR_BIN:-}"
if [[ -z "$editor" ]]; then
    # One Qt build folder per working copy, named like scripts/editor.sh does.
    root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
    checkout="$(basename "$root")"
    checkout="${checkout#petunia-engine-}"
    editor="/tmp/petunia-editor-${checkout:-petunia-engine}/petunia-editor"
fi
output="${1:-/tmp/petunia-shot/sprite.png}"
workdir="$(mktemp -d /tmp/petunia-sprite.XXXXXX)"
mkdir -p "$(dirname "$output")"
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/_virtual-display.sh"
choose_display :107
rm -f "$output" "$output.viewport.png"

cleanup() {
    [[ -n "${editor_pid:-}" ]] && kill "$editor_pid" 2>/dev/null || true
    [[ -n "${xvfb_pid:-}" ]] && kill "$xvfb_pid" 2>/dev/null || true
    # A failed run keeps its scene, log and pictures: the log of a crash is the only evidence.
    if [[ -n "${PETUNIA_SMOKE_KEEP:-}" ]]; then
        echo "arquivos da rodada em $workdir" >&2
    else
        rm -rf "$workdir"
    fi
}
trap cleanup EXIT

# The project: a 3D box line and one sprite in front of it, showing a 2x1 sheet (two 16x16
# pictures, each with a transparent border). Handing the editor a folder it did not create also
# exercises the loading path.
python3 -I - "$workdir/sala" <<'PY'
import json, os, struct, sys, zlib

root = sys.argv[1]
os.makedirs(os.path.join(root, "scenes"), exist_ok=True)
os.makedirs(os.path.join(root, "assets"), exist_ok=True)

MAGENTA = bytes((255, 0, 255, 255))
CYAN = bytes((0, 255, 255, 255))
CLEAR = bytes((0, 0, 0, 0))
FRAME = 16
BORDER = 2
SIDE = (FRAME * 2, FRAME)


def pixel(x, y):
    inside = BORDER <= x % FRAME < FRAME - BORDER and BORDER <= y < FRAME - BORDER
    return (MAGENTA if x < FRAME else CYAN) if inside else CLEAR


def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)


rows = b"".join(b"\x00" + b"".join(pixel(x, y) for x in range(SIDE[0])) for y in range(SIDE[1]))
png = b"\x89PNG\r\n\x1a\n"
png += chunk(b"IHDR", struct.pack(">IIBBBBB", SIDE[0], SIDE[1], 8, 6, 0, 0, 0))
png += chunk(b"IDAT", zlib.compress(rows))
png += chunk(b"IEND", b"")
open(os.path.join(root, "assets", "folha.png"), "wb").write(png)

# The camera of a new project sits at the origin looking at -Z (70 degrees vertical, so 3.5011 world
# units fit in half the height at 5 m). The sprite's centre is 2 m up, clear of the floor rule, and
# the box behind it is big enough to fill the view around it.
scene = {
    "format_version": 1,
    "next_persistent_id": 3,
    "entities": [
        {
            "id": 1,
            "rows": {
                "Name": {"text": "Fundo"},
                "Transform": {"position": {"x": 0.0, "y": 2.0, "z": -9.0}},
                "Brush": {
                    "shape": "box",
                    "size": {"x": 30.0, "y": 30.0, "z": 6.0},
                    "color": {"r": 0.2, "g": 0.55, "b": 0.2, "a": 1.0},
                    "detail": 16,
                },
            },
        },
        {
            "id": 2,
            "rows": {
                "Name": {"text": "Folha"},
                "Transform": {"position": {"x": 0.0, "y": 2.0, "z": -5.0}},
                "Sprite": {
                    "asset": "assets/folha.png",
                    "size": {"x": 2.0, "y": 2.0, "z": 0.0},
                    "color": {"r": 1.0, "g": 1.0, "b": 1.0, "a": 1.0},
                    "layer": 0,
                    "flip_x": False,
                    "flip_y": False,
                },
                # Two frames side by side; the row says frame 1 (the right one, cyan) and the
                # animation is paused, so the picture does not depend on when the screenshot fires.
                "SpriteAnimation": {
                    "frames": 2,
                    "columns": 2,
                    "fps": 8.0,
                    "loop": True,
                    "playing": False,
                    "frame": 1,
                },
            },
        },
    ],
}
json.dump(scene, open(os.path.join(root, "scenes", "room.scene.json"), "w"), indent=2)
manifest = {
    "format_version": 1,
    "name": "Sprite",
    "default_scene": "scenes/room.scene.json",
    "scripting_language": "aipo",
}
json.dump(manifest, open(os.path.join(root, "petunia.project.json"), "w"), indent=2)
PY

Xvfb "$display" -screen 0 1440x900x24 +extension GLX >/dev/null 2>&1 &
xvfb_pid=$!
sleep 1

# The display and the scratch folders are EXPORTED, never prefixed to a `\`-continued command: a
# comment inside a continuation ends the command and the editor would start on the real display
# with the real settings folder (that happened to editor-smoke.sh on 2026-10-29).
export DISPLAY="$display" XDG_CONFIG_HOME="$workdir/config" XDG_CACHE_HOME="$workdir/cache"
export QT_QPA_PLATFORM=xcb LIBGL_ALWAYS_SOFTWARE=1 QT_LOGGING_RULES='qt.qpa.*=false'
unset WAYLAND_DISPLAY
"$editor" --project "$workdir/sala" --screenshot "$output" --screenshot-delay 5000 2>"$workdir/editor.log" &
editor_pid=$!
wait_for_window "Petunia Editor" "$display" "$editor_pid" "$workdir/editor.log"
wait "$editor_pid" || { sed -e 's/^/  editor: /' "$workdir/editor.log" >&2; exit 1; }

# Everything is read from the viewport picture: the window picture also has the panels, the
# viewport picture is the rendered scene alone.
[[ -s "$output.viewport.png" ]] || { echo "erro: o editor não gravou $output.viewport.png" >&2; exit 1; }

python3 -I - "$output.viewport.png" <<'PY'
import subprocess, sys

picture = sys.argv[1]
width, height = (int(value) for value in subprocess.run(
    ["identify", "-format", "%w %h", picture], check=True, capture_output=True, text=True
).stdout.split())


def rgb(x, y):
    text = subprocess.run(
        ["convert", picture, "-format", "%%[pixel:p{%d,%d}]" % (int(x), int(y)), "info:"],
        check=True, capture_output=True, text=True
    ).stdout
    numbers = text[text.find("(") + 1:text.find(")")].replace("%", "").split(",")
    return tuple(float(value) for value in numbers[:3])


def magenta(color):
    red, green, blue = color
    return red > 120 and blue > 120 and green < 90


def cyan(color):
    red, green, blue = color
    return red < 90 and green > 120 and blue > 120


def painted(color):
    return magenta(color) or cyan(color)


# Screen geometry of the quad, from the scene (70 degrees vertical, centre 2 m up at 5 m, half a
# unit each way). The picture has y growing downwards, so "up" subtracts.
half_picture = height / 2
pixels_per_world = half_picture / 3.5011
centre_x = width / 2
centre_y = half_picture - 2.0 * pixels_per_world
half = 1.0 * pixels_per_world

middle = rgb(centre_x, centre_y)
quarter = rgb(centre_x - 0.5 * half, centre_y)
border = rgb(centre_x + 0.93 * half, centre_y)
outside = rgb(centre_x, centre_y + 1.6 * half)
print(f"verificação: meio do sprite em {middle} é o quadro 1 da folha (ciano) ->", "ok" if cyan(middle) else "FALHOU")
print(f"verificação: a um quarto do centro em {quarter} também é ciano ->", "ok" if cyan(quarter) else "FALHOU")
print(f"verificação: borda transparente em {border} deixa a caixa 3D aparecer ->", "ok" if not painted(border) else "FALHOU")
print(f"verificação: fora do quadrado em {outside} não há sprite ->", "ok" if not painted(outside) else "FALHOU")
sys.exit(0 if cyan(middle) and cyan(quarter) and not painted(border) and not painted(outside) else 1)
PY
echo "imagem: $output (e $output.viewport.png); projeto: $workdir/sala"
