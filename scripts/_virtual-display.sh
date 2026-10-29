#!/usr/bin/env bash
# Shared by the smoke tests (`editor-smoke.sh`, `natural-smoke.sh`): a virtual display of our own and
# a hard stop when the editor dies while opening.
#
# Why this exists. A run that dies can leave its display busy, and an editor window from that run can
# keep sitting on top of the new one. Every click and key then goes to the WRONG window while the
# script still "runs" happily: the strokes land in another project, the scene file is never written
# and the assertions fail as if a feature were broken. That happened here on 2026-10-29 and cost an
# hour of guessing, so both scripts now pick a free display and check that the editor is alive.
#
#   choose_display :98        -> sets `display`; `PETUNIA_EDITOR_DISPLAY` pins it (a busy pinned
#                                display is an error, never a different number)
#   watch_editor <pid> <log>  -> aborts, printing the editor log, when it died during startup
#   wait_for_window <title> <display> <pid> <log>
#                             -> waits until the window is mapped and the first frame drawn; a
#                                fixed `sleep 3` is not enough right after a build

# A display nobody is using. Xvfb refuses to start twice on the same number, and the lock plus the
# socket are the two traces a previous server leaves behind.
choose_display() {
    local first="${1#:}"
    local pinned="${PETUNIA_EDITOR_DISPLAY:-}"
    local offset candidate
    for offset in 0 1 2 3 4 5 6 7 8 9; do
        candidate=":$((first + offset))"
        if [[ ! -e "/tmp/.X${candidate#:}-lock" && ! -e "/tmp/.X11-unix/X${candidate#:}" ]]; then
            display="$candidate"
            return 0
        fi
        if [[ -n "$pinned" ]]; then
            echo "erro: $candidate já está em uso; libere-o ou escolha outro com PETUNIA_EDITOR_DISPLAY" >&2
            return 1
        fi
    done
    echo "erro: nenhum display virtual livre a partir de :$first" >&2
    return 1
}

# The editor has to survive its startup: when it dies, nothing below reaches a window and the scene
# file is never written, so the run must stop here with the log instead of failing later.
watch_editor() {
    local pid="$1" log="$2"
    if kill -0 "$pid" 2>/dev/null; then
        return 0
    fi
    echo "erro: o editor saiu durante a abertura" >&2
    if [[ -s "$log" ]]; then
        sed -e 's/^/  editor: /' "$log" >&2
    fi
    return 1
}

# A fixed `sleep 3` used to be enough and stopped being enough right after a build: the first frame
# under software OpenGL takes longer, every click that arrives before the window is mapped goes
# nowhere, and the whole run then fails as if the features were broken (2026-10-29, the two smoke
# tests failed at once on a freshly built editor). Wait for the window itself, then settle.
wait_for_window() {
    local title="$1" display="$2" pid="$3" log="$4"
    local deadline=$((SECONDS + 25))
    while ((SECONDS < deadline)); do
        if DISPLAY="$display" xdotool search --name "$title" >/dev/null 2>&1; then
            sleep 1
            return 0
        fi
        if ! kill -0 "$pid" 2>/dev/null; then
            watch_editor "$pid" "$log" || return 1
        fi
        sleep 0.2
    done
    echo "erro: a janela \"$title\" não apareceu em 25 s" >&2
    if [[ -s "$log" ]]; then
        sed -e 's/^/  editor: /' "$log" >&2
    fi
    return 1
}
