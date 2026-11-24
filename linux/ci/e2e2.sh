#!/usr/bin/env bash
# Gate E2E-2 (#98 Linux, #108 Windows, docs/plan-windows-linux.md §7, phases 3–4):
# the **real** `claude` CLI, with the hook installed by the product's own
# installer, produces a card in the panel; a click answers it; the CLI obeys;
# and uninstall puts the user's config back byte for byte.
#
# E2E-1 (`linux/ci/e2e1.sh`) proves the wire with a synthetic hook. This one
# proves the product against the CLI it is built for, which is the only thing
# that can catch a hook contract that drifted.
#
# Two modes:
#
#   linux/ci/e2e2.sh --self-check   # installer round-trip only, any host
#   linux/ci/e2e2.sh               # the whole gate, needs `claude` + tmux
#
# --self-check is what makes this script useful before a host is available: it
# proves install → settings.json carries our hook → idempotent re-install →
# uninstall → the file is byte-for-byte what it was, and prints the same
# Evidence block. The full run adds the CLI and the panel on top.
# Evidence block. It needs only an app binary (MORROW_APP, or target/debug) —
# no frontend build, no display, no socket, no bridge — so it runs on a clean
# checkout given a prebuilt binary. The full run adds the CLI/panel.
#
# A real X11 desktop (DISPLAY set; the final judgement, Debian host #70):
#   (cd shared/desktop && bun install && bun run build) && linux/ci/e2e2.sh
# CI, in the image of linux/ci/Dockerfile:
#   docker run --rm -v "$PWD":/work morrow-tauri-x11:dev linux/ci/e2e2.sh
#
# The run uses a HOME of its own ($WORK/home), never the developer's: it
# installs and uninstalls a hook, and the user's real config is not a test
# fixture. `claude` is told that HOME too, so its session lives there as well.
#
# Env: MORROW_APP (binary; built when unset), E2E2_SCREEN (Xvfb only),
#      E2E2_COMMIT (Evidence block when git cannot read the checkout),
#      E2E2_PROMPT (the prompt given to `claude`), CARGO_TARGET_DIR,
#      CLAUDE_BIN (default: claude).
# Env: MORROW_APP (binary; required for --self-check, built in full mode when unset),
#      E2E2_SCREEN (Xvfb only),
#      E2E2_COMMIT (Evidence block when git cannot read the checkout),
#      E2E2_PROMPT (prompt template; keep __MARKER_FILE__ in it),
#      CARGO_TARGET_DIR, CLAUDE_BIN (default: claude).
# Exit: 0 when every check passed, 1 otherwise.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"
NAME=e2e2
# shellcheck source=linux/ci/x11-lib.sh
. linux/ci/x11-lib.sh

CLAUDE="${CLAUDE_BIN:-claude}"
# Something that needs a tool call, so [CC] asks before running it. Kept boring:
# one `echo`, no network, no file written.
PROMPT="${E2E2_PROMPT:-Run the shell command: echo e2e2-ok}"
# The tool's effect is a FILE, not a string in the captured pane: the TUI echoes
# the submitted prompt, so grepping the pane for a marker proves nothing. The
# prompt names a per-run path; the checks below test that path on disk.
MARKER_CONTENT=e2e2-ok
marker_written() { # marker_written <path> <content>: the tool actually ran
  grep -qx "$2" "$1" 2>/dev/null
}
marker_not_written() { ! marker_written "$@"; }
marker_absent() { [ ! -e "$1" ]; }
# E2E2_PROMPT may reword the instruction, but must keep __MARKER_FILE__ where the
# per-run path goes, or the effect cannot be observed.
PROMPT_TEMPLATE="${E2E2_PROMPT:-Run this shell command exactly: echo '$MARKER_CONTENT' > '__MARKER_FILE__'}"
SELF_CHECK=0
[ "${1:-}" = "--self-check" ] && SELF_CHECK=1

# --- prerequisites -------------------------------------------------------
# --self-check is the installer half, and it needs no CLI at all.
if [ "$SELF_CHECK" -eq 0 ]; then
  command -v "$CLAUDE" >/dev/null 2>&1 ||
    die "no '$CLAUDE' on PATH: this gate needs the real CLI (use --self-check for the installer half)"
fi
command -v sha256sum >/dev/null 2>&1 || command -v shasum >/dev/null 2>&1 ||
  die "no sha256sum or shasum: the byte-for-byte check needs one"

# --self-check needs only the app binary: no frontend, no display.
if [ "$SELF_CHECK" -eq 1 ] && [ -z "${MORROW_APP:-}" ]; then
  cargo build --locked -p morrow-app
  MORROW_APP="${CARGO_TARGET_DIR:-$ROOT/target}/debug/morrow-app"
fi
x11_binaries
x11_workdir
# --self-check needs only the app binary: no frontend, no display, no socket,
# no bridge. The full run still goes through x11_binaries/x11_workdir.
if [ "$SELF_CHECK" -eq 1 ]; then
  if [ -z "${MORROW_APP:-}" ]; then
    MORROW_APP="$TARGET_DIR/debug/morrow-app"
    [ -x "$MORROW_APP" ] ||
      die "no app binary: --self-check needs only the binary (set MORROW_APP=/path/to/morrow-app); building one is Tauri's job and needs the frontend dist"
  fi
  WORK="$(mktemp -d "/tmp/$NAME.XXXXXX")"
  pids=()
  trap x11_cleanup EXIT
else
  x11_binaries
  x11_workdir
fi
# Built first: `cargo build` needs the real HOME for its registry.
export HOME="$WORK/home"
mkdir -p "$HOME/.claude"
# The hook installer resolves everything from HOME, and the app reads it at
# launch, so both see the scratch home.
export CLAUDE_CONFIG_DIR="$HOME/.claude"

SETTINGS="$HOME/.claude/settings.json"
SCRIPT_FILE="$HOME/.codeisland/codeisland-hook.sh"
checksum() { # checksum <path>: sha256, or "absent"
  if [ -f "$1" ]; then
    if command -v sha256sum >/dev/null 2>&1; then
      sha256sum "$1" | cut -d' ' -f1
    else
      shasum -a 256 "$1" | cut -d' ' -f1
    fi
  else
    echo absent
  fi
}
# Two cases, because they prove different things. An absent file proves the
# installer can create one; a file the user already had proves the byte-for-byte
# promise, which is the one that matters when their config is not ours.
before="$(checksum "$SETTINGS")"


# Pin the effect checks on disk: no CLI, no display, no tmux needed. These catch
# a regression back to grepping the echoed prompt instead of the marker file.
probe="$WORK/marker.probe"
rm -f "$probe"
check marker_absent_without_file "no marker file means the tool did not run" \
  marker_absent "$probe"
printf '%s\n' "$MARKER_CONTENT" >"$probe"
check marker_written_with_content "marker with the expected content proves the run" \
  marker_written "$probe" "$MARKER_CONTENT"
printf 'something-else\n' >"$probe"
check marker_rejects_other_content "other content is not the tool's effect" \
  marker_not_written "$probe" "$MARKER_CONTENT"
rm -f "$probe"
# --- the installer -------------------------------------------------------
install_out="$("$MORROW_APP" --hooks install claude 2>&1)" || {
  check install_claude "install failed: $install_out" false
  install_out=""
}
check install_claude "$install_out" grep -q '^claude: changed=' <<<"$install_out"
check settings_written "settings.json exists and names our script" \
  sh -c "grep -q 'codeisland-hook.sh' '$SETTINGS'"
check script_written "$SCRIPT_FILE" test -f "$SCRIPT_FILE"
check install_is_idempotent "a second install changes nothing" \
  sh -c "[ \"\$('$MORROW_APP' --hooks install claude)\" = 'claude: changed=nothing' ]"
check status_says_installed "morrow-app --hooks status" \
  sh -c "'$MORROW_APP' --hooks status | grep -qx 'claude: installed'"

# --- the CLI and the panel (not in --self-check) -------------------------
latency=()

prompts=()
if [ "$SELF_CHECK" -eq 0 ]; then
  command -v tmux >/dev/null 2>&1 || die "no tmux: the CLI needs a terminal to ask in"

  [ "${XDG_SESSION_TYPE:-}" != wayland ] || die "a Wayland session: E2E-2 is the X11 gate"
  if [ -n "${DISPLAY:-}" ]; then
    display="real X11 $DISPLAY ($(xdpyinfo | sed -n 's/^vendor string: *//p'), session ${XDG_SESSION_TYPE:-?})"
    read -r SW SH <<<"$(xdotool getdisplaygeometry)"
  else
    SCREEN="${E2E2_SCREEN:-1920x1080}"
    SW="${SCREEN%x*}"
    SH="${SCREEN#*x}"
    x11_xvfb_openbox
    display="Xvfb ${SW}x${SH}x24, no compositor"
  fi
  x11_target
  x11_start_app

  # ask <name>: one prompt into a fresh tmux session; T0 is when it was typed.
  ask() {
    local name="$1"
    T0=$(now_ms)
    tmux new-session -d -s "$name" -x 200 -y 50 "$CLAUDE"
    tmux send-keys -t "$name" "$PROMPT" Enter
    tmux pipe-pane -t "$name" -o "cat >'$WORK/$name.cli'"
    local name="$1" marker="$WORK/$1.marker" prompt
    rm -f "$marker"
    tmux new-session -d -s "$name" -x 200 -y 50 "$CLAUDE"
    # Attach the pipe before typing: the first paint, including the card's
    # clock, starts with the prompt and would be missed if we attached after.
    tmux pipe-pane -t "$name" -o "cat >'$WORK/$name.cli'"
    prompt="${PROMPT_TEMPLATE//__MARKER_FILE__/$marker}"
    prompts+=("$prompt")
    T0=$(now_ms)
    tmux send-keys -t "$name" "$prompt" Enter
  }
  # answer <name> <approve|deny>: waits for the card, clicks the button, and
  # records what the CLI printed after the answer.
  answer() {
    local name="$1" which="$2" line at id
    local name="$1" which="$2" line at id marker
    marker="$WORK/$name.marker"
    id=$([ "$name" = approve ] && echo 1 || echo 2)
    if ! until_ok 120 sh -c "grep -q 'morrow-smoke: card id=$id ' '$WORK/app.out'"; then
      check "${name}_card_shown" "no card $id within 120 s: $(tail -n 5 "$WORK/$name.cli" 2>/dev/null)" false
      return
    fi
    line="$(grep -m1 "morrow-smoke: card id=$id " "$WORK/app.out")"
    at="${line##*at=}"
    check "${name}_card_shown" "card $id, $((at - T0)) ms after the prompt" true
    line="$(grep "morrow-smoke: card id=$id " "$WORK/app.out" | tail -n1)"
    IFS=, read -r BX BY BW BH <<<"$(sed -n "s/.* $which=\([0-9,]*\).*/\1/p" <<<"$line")"
    geometry "$ISLAND"
    xdotool mousemove --sync $((X + BX + BW / 2)) $((Y + BY + BH / 2))
    local t_click
    t_click=$(now_ms)
    xdotool click 1
    # The CLI has to act on the answer, not just receive it.
    if [ "$which" = approve ]; then
      check "${name}_cli_ran_the_tool" "e2e2-ok in the CLI's output" \
        until_ok 60 sh -c "grep -q 'e2e2-ok' '$WORK/$name.cli'"
    else
      check "${name}_cli_did_not_run_the_tool" "no e2e2-ok after a deny" \
        sh -c "! grep -q 'e2e2-ok' '$WORK/$name.cli'"
    fi
    check "${name}_card_gone" "the next push has cards=0" \
      until_ok 5 sh -c "grep 'morrow-smoke: pushed' '$WORK/app.out' | tail -n1 | grep -q 'cards=0$'"
    # Wait for the card to be dismissed first: that is the point at which the
    # CLI has the answer, so an absence check afterwards means something.
    check "${name}_card_gone" "the next push has cards=0" \
      until_ok 5 sh -c "grep 'morrow-smoke: pushed' '$WORK/app.out' | tail -n1 | grep -q 'cards=0$'"
    # The effect, not the echoed prompt: the marker file exists only if the CLI
    # actually ran the tool after the click. Independent of the prompt wording.
    if [ "$which" = approve ]; then
      check "${name}_cli_ran_the_tool" "marker file written with the expected content" \
        until_ok 60 marker_written "$marker" "$MARKER_CONTENT"
    else
      check "${name}_cli_did_not_run_the_tool" "no marker file after a deny" \
        marker_absent "$marker"
    fi
    latency+=("$name: card $((at - T0)) ms")
    tmux kill-session -t "$name" 2>/dev/null || true
  }

  ask approve
  answer approve approve
  ask deny
  answer deny deny
fi

# --- uninstall puts the config back --------------------------------------
uninstall_out="$("$MORROW_APP" --hooks uninstall claude 2>&1)" || {
  check uninstall_claude "uninstall failed: $uninstall_out" false
  uninstall_out=""
}
check uninstall_claude "$uninstall_out" grep -q '^claude: changed=' <<<"$uninstall_out"
# An absent file comes back as `{}` rather than gone, which is what macOS
# `ConfigInstaller.uninstallHooks` leaves too: an empty object is "no settings",
# and deleting a file the user may have made is not ours to decide.
if [ "$before" = absent ]; then
  check settings_restored "no hook left, file is absent or {}" \
    sh -c "! grep -q 'codeisland-hook.sh' '$SETTINGS' 2>/dev/null"
else
  check settings_restored "settings.json is $before again" \
    test "$(checksum "$SETTINGS")" = "$before"
fi
check status_says_absent "morrow-app --hooks status" \
  sh -c "'$MORROW_APP' --hooks status | grep -qx 'claude: absent'"

# --- the case that matters: a config the user already had -----------------
# Their own keys, their own formatting. Install must add our hook without
# touching the rest, and uninstall must give the file back byte for byte.
rm -f "$SETTINGS"
printf '%s\n' '{"theme":"dark","permissions":{"allow":["Bash(ls:*)"]}}' >"$SETTINGS"
user_before="$(checksum "$SETTINGS")"
"$MORROW_APP" --hooks install claude >/dev/null 2>&1 || true
check user_keys_kept "the user's own keys survive the install" \
  sh -c "grep -q '\"theme\":\"dark\"' '$SETTINGS' && grep -q 'Bash(ls:\*)' '$SETTINGS'"
check user_hook_added "our hook is in the same file" \
  sh -c "grep -q 'codeisland-hook.sh' '$SETTINGS'"
"$MORROW_APP" --hooks uninstall claude >/dev/null 2>&1 || true
check user_settings_byte_for_byte "settings.json is $user_before again" \
  test "$(checksum "$SETTINGS")" = "$user_before"

# --- Evidence -----------------------------------------------------------
echo
echo "=== Evidence: linux/ci/e2e2.sh (gate E2E-2) ==="
echo "date:       $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "commit:     ${E2E2_COMMIT:-$(git -c safe.directory="$ROOT" rev-parse --short HEAD 2>/dev/null || echo unknown)}"
echo "mode:       $([ "$SELF_CHECK" -eq 1 ] && echo 'self-check (installer only)' || echo 'full')"
echo "kernel:     $(uname -srm)"
echo "claude:     $([ "$SELF_CHECK" -eq 1 ] && echo 'not needed in self-check' || "$CLAUDE" --version 2>&1 | head -n1)"
echo "home:       $HOME (scratch; the developer's config was not touched)"
echo "install:    $install_out"
echo "uninstall:  $uninstall_out"
echo "settings:   before=$before after=$(checksum "$SETTINGS")"
if [ "$SELF_CHECK" -eq 0 ]; then
  echo "display:    $display"
  echo "wm:         $(wmctrl -m 2>/dev/null | sed -n 's/^Name: //p')"
  echo "prompt:     $PROMPT"
  for entry in "${prompts[@]}"; do echo "prompt:     $entry"; done
  for entry in "${latency[@]}"; do echo "latency:    $entry"; done
fi
printf '%s\n' "${results[@]}"
if [ "$failed" -eq 0 ]; then
  echo "result:     PASS (${#results[@]} checks)"
else
  echo "result:     FAIL"
  for log in "$WORK"/*.cli; do
    [ -f "$log" ] || continue
    echo "--- $(basename "$log") ---"
    tail -n 20 "$log"
  done
  [ -f "$WORK/app.out" ] && {
    echo "--- app stdout ---"
    tail -n 20 "$WORK/app.out"
  }
fi
exit "$failed"
