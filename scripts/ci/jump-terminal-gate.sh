#!/usr/bin/env bash
# #187 — gate for the jump-to-terminal card action (L-009 Linux X11, W-007
# Windows). Prints one Evidence block per OS so a reviewer does not need to
# know the internals. Portable checks run anywhere; the window-level checks
# need the target desktop and are DELEGATED when this host is not one.
#
# Usage:
#   ./scripts/ci/jump-terminal-gate.sh            # both blocks, portable checks
#   ./scripts/ci/jump-terminal-gate.sh linux      # also run the X11 check here
#   ./scripts/ci/jump-terminal-gate.sh windows    # also run the Windows check here
#
# Exit: 0 when every check that ran passed. A DELEGATED line is not a pass.
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

say "=== Evidence: #187 jump to terminal ==="
say "date:    $(date -u +%Y-%m-%dT%H:%M:%SZ)"
say "host:    $(uname -srm)"
say "commit:  $(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
say "mode:    $MODE"

# --- portable: the card's three outcomes and the icon rule ----------------
say
say "-- portable (runs on any host) --"
if [ -d shared/desktop/node_modules ]; then
  check "bun test — Focused / Copied / NotPossible / remote icon" \
    bash -c 'cd shared/desktop && bun test'
else
  say "SKIP  bun test: node_modules is not installed here"
fi
check "cargo test windows_rules — process chain incl. VS Code and IntelliJ" \
  cargo test -p morrow-platform --test windows_rules
check "cargo test terminal_and_process — X11 focuses a window, then copies the path" \
  cargo test -p morrow-platform --test terminal_and_process
check "cargo check morrow-app" cargo check -p morrow-app

# --- L-009 Linux X11 ------------------------------------------------------
say
say "=== Evidence: L-009 Linux X11 (window-level focus) ==="
say "contract: terminal_activate uses wmctrl/xdotool windowactivate, then copies cwd"
if [ "$MODE" = linux ] && [ "$(uname -s)" = Linux ] && [ -n "${DISPLAY:-}" ]; then
  for tool in xdotool wmctrl xclip; do
    command -v "$tool" >/dev/null 2>&1 || { say "FAIL  missing $tool"; fail=$((fail + 1)); }
  done
  say "display: $DISPLAY"
  say "DELEGATED-interactive: start a session in Xterm/kitty, note xdotool getactivewindow,"
  say "  click the card's context row in the panel, and re-read xdotool getactivewindow."
  say "  Evidence to paste: before=<window id> after=<terminal id> outcome=Focused|Copied"
else
  say "DELEGATED: run on a Debian X11 host with Xvfb/Openbox:"
  say "  (cd shared/desktop && bun install && bun run build)"
  say "  ./scripts/ci/jump-terminal-gate.sh linux"
  say "  Evidence to paste: display, before/after active window, outcome, commit"
fi

# --- W-007 Windows --------------------------------------------------------
say
say "=== Evidence: W-007 Windows (SetForegroundWindow) ==="
say "contract: cli_pid -> Toolhelp32 parent chain -> HOSTS (Windows Terminal, VS Code,"
say "          IntelliJ, conhost, WezTerm, Alacritty, kitty) -> first visible titled"
say "          window owned by that process -> SetForegroundWindow; cwd copied if none."
if command -v cargo-xwin >/dev/null 2>&1; then
  check "cargo xwin check morrow-app — Windows compile only, not a runtime gate" \
    cargo xwin check -p morrow-app --target x86_64-pc-windows-msvc
else
  say "SKIP  Windows cross-compile check: cargo-xwin is not installed"
fi
if [ "$MODE" = windows ] && [[ "$(uname -s)" == MINGW* || "$(uname -s)" == MSYS* || "$(uname -s)" == CYGWIN* ]]; then
  check "cargo check Windows target" cargo check -p morrow-platform --target x86_64-pc-windows-msvc
  say "DELEGATED-interactive: in Windows Terminal, VS Code and IntelliJ start a session,"
  say "  click the card's context row and confirm the owning window comes forward."
  say "  Evidence to paste: host=<app>, process=<exe>, outcome=Focused, commit"
else
  say "DELEGATED: run on the Windows VM (or cross-check with scripts above):"
  say "  windows/ci/check.sh"
  say "  ./scripts/ci/jump-terminal-gate.sh windows"
  say "  Evidence to paste: per-app window that came forward, outcome, commit"
fi

say
say "summary: PASS=$pass FAIL=$fail (DELEGATED host gates are not counted as passes)"
[ "$fail" -eq 0 ]
