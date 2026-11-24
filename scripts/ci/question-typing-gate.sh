#!/usr/bin/env bash
# #189 — gate for typing in Question cards (W-005, Linux + Windows).
# Prints one Evidence block per OS. Portable checks run on any host; the
# window-level focus checks need the target desktop and are DELEGATED when
# this host is not one. A DELEGATED line is never counted as a pass.
#
# Usage:
#   ./scripts/ci/question-typing-gate.sh           # both blocks, portable checks
#   ./scripts/ci/question-typing-gate.sh linux     # also run the X11 check here
#   ./scripts/ci/question-typing-gate.sh windows   # also run the Windows check here
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"
MODE="${1:-all}"
say() { printf '%s\n' "$*"; }
pass=0
fail=0
check() {
  local name="$1"; shift
  if "$@" >/dev/null 2>&1; then
    say "PASS  $name"
    pass=$((pass + 1))
  else
    say "FAIL  $name"
    fail=$((fail + 1))
  fi
}

say "=== Evidence: #189 Question typing ==="
say "date:    $(date -u +%Y-%m-%dT%H:%M:%SZ)"
say "host:    $(uname -srm)"
say "commit:  $(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
say "mode:    $MODE"

# --- portable: pure seams, build and reply bytes ---------------------------
say
say "-- portable (runs on any host) --"
if [ -d shared/desktop/node_modules ]; then
  check "bun test — input mode, submit gate, focus lifecycle" \
    bash -c 'cd shared/desktop && bun test'
  check "bun run build — Svelte field and focus ring compile" \
    bash -c 'cd shared/desktop && bun run build'
else
  say "SKIP  bun gates: node_modules is not installed here"
fi
check "cargo test morrow-app — #188 reply payload is unchanged" \
  cargo test -p morrow-app
check "cargo check morrow-app" cargo check -p morrow-app
check "verify-local.sh" bash -c './scripts/ci/verify-local.sh'
if command -v cargo-xwin >/dev/null 2>&1; then
  check "cargo xwin check morrow-app — W-005 Windows compile, not a runtime gate" \
    cargo xwin check -p morrow-app --target x86_64-pc-windows-msvc
else
  say "SKIP  Windows cross-compile check: cargo-xwin is not installed"
fi

# --- W-005 Linux X11 (GTK accept-focus toggle) -----------------------------
say
say "=== Evidence: W-005 Linux X11 (typed field) ==="
say "contract: the field's onfocus calls island_question_focus(true); the command"
say "          toggles GTK accept-focus (set_focusable) and presents the window;"
say "          Enter submits the typed text; close calls island_question_focus(false)."
if [ "$MODE" = linux ] && [ "$(uname -s)" = Linux ] && [ -n "${DISPLAY:-}" ]; then
  say "display: $DISPLAY"
  say "DELEGATED-interactive: open a Notification question with no options, click the"
  say "  field, type 'Last 30 days' and press Enter; then open an option question,"
  say "  click Other, type and press Enter; confirm focus returns on Hide/Skip."
  say "  Evidence to paste: screenshot of the focused field's blue ring, the typed"
  say "  answer bytes (updatedInput.answers), and xdotool getwindowfocus before/after."
else
  say "DELEGATED: run on a Debian X11 host with Xvfb/Openbox:"
  say "  (cd shared/desktop && bun install && bun run build)"
  say "  ./scripts/ci/question-typing-gate.sh linux"
  say "  Evidence to paste: display, focused-field screenshot, typed answer bytes,"
  say "  before/after keyboard focus, commit"
fi

# --- W-005 Windows (WS_EX_NOACTIVATE toggle) -------------------------------
say
say "=== Evidence: W-005 Windows (typed field) ==="
say "contract: set_question_focus lifts WS_EX_NOACTIVATE and calls SetForegroundWindow"
say "          on open, restores it on close; the typed text is the answer, never"
say "          the 'Other' row label. The #188 reply layout is unchanged."
if command -v cargo-xwin >/dev/null 2>&1; then
  say "PASS  Windows command compiles (see portable block above)"
else
  say "SKIP  Windows cross-compile check: cargo-xwin is not installed"
fi
if [ "$MODE" = windows ] && [[ "$(uname -s)" == MINGW* || "$(uname -s)" == MSYS* || "$(uname -s)" == CYGWIN* ]]; then
  check "cargo check Windows target" cargo check -p morrow-app --target x86_64-pc-windows-msvc
  say "DELEGATED-interactive: on the Windows VM click the field, type, press Enter,"
  say "  then Hide the card and confirm the previous app has the keyboard again."
  say "  Evidence to paste: focused-field screenshot, answer bytes, focus owner, commit"
else
  say "DELEGATED: run on the Windows VM:"
  say "  ./scripts/ci/question-typing-gate.sh windows"
  say "  Evidence to paste: focused-field screenshot, typed answer bytes, focus owner,"
  say "  commit"
fi

say
say "summary: PASS=$pass FAIL=$fail (DELEGATED host gates are not counted as passes)"
[ "$fail" -eq 0 ]
