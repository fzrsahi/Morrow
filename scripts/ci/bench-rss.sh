#!/usr/bin/env bash
# Peak RSS of the built Morrow app, measured on the host that runs it (#64).
#
# Not a build script and not a CI gate. It starts an already-built binary,
# samples its resident set at a fixed interval for a fixed window, then stops
# it. A cross-build or a container cannot produce RSS for the target OS, so run
# this on the desktop being measured.
#
# Host prerequisites
#   macOS 14+: build with `swift build` or `macos/build.sh` first.
#   Linux: GTK3 + WebKitGTK 4.1 dev libs, a frontend dist (`bun run build`),
#          `cargo build -p morrow-app`, and a working X11 session.
#   Both: bash, ps, kill, and a sleep that accepts fractional seconds.
#   No sudo, no CI runner, and no changes to the user's config.
#
# Usage, from the repo root:
#   ./scripts/ci/bench-rss.sh
#   MORROW_APP=/path/to/app ./scripts/ci/bench-rss.sh
#   BENCH_SECONDS=20 BENCH_INTERVAL=0.5 ./scripts/ci/bench-rss.sh
#
# Env: MORROW_APP (binary, auto-detected when unset), BENCH_SECONDS (default
#      10), BENCH_INTERVAL (default 0.2).
# Exit: 0 when samples were collected on this host, 1 otherwise.
# Scope: the app's main process only. WebView/WebKit children are not summed;
#        see the README section "Measure peak RAM usage" before comparing OSes.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

die() { printf 'bench-rss: %s\n' "$1" >&2; exit 1; }

case "${1:-}" in
  -h|--help)
    sed -n '2,24p' "${BASH_SOURCE[0]}"
    exit 0
    ;;
  '') ;;
  *) die "unknown argument: $1 (use --help)" ;;
esac

WINDOW="${BENCH_SECONDS:-10}"
INTERVAL="${BENCH_INTERVAL:-0.2}"
case "$WINDOW" in (*[!0-9]*|'') die "BENCH_SECONDS must be a positive integer" ;; esac
case "$INTERVAL" in (*[!0-9.]*|''|.|*.*.*) die "BENCH_INTERVAL must be a positive number" ;; esac
[ "$WINDOW" -gt 0 ] || die "BENCH_SECONDS must be a positive integer"

HOST="$(uname -s)"
case "$HOST" in
  Darwin) candidates=(
    "$ROOT/macos/.build/release/Morrow.app/Contents/MacOS/Morrow"
    "$ROOT/macos/.build/release/Morrow"
    "$ROOT/macos/.build/debug/Morrow"
  ) ;;
  Linux) candidates=(
    "$ROOT/target/release/morrow-app"
    "$ROOT/target/debug/morrow-app"
  ) ;;
  *) die "unsupported host $HOST; use windows/ci/bench-rss.ps1 on Windows" ;;
esac

APP="${MORROW_APP:-}"
if [ -z "$APP" ]; then
  for candidate in "${candidates[@]}"; do
    if [ -x "$candidate" ]; then APP="$candidate"; break; fi
  done
fi
[ -n "$APP" ] || die "no built app found; build it first or set MORROW_APP (see --help)"
[ -x "$APP" ] || die "not executable: $APP"

command -v ps >/dev/null 2>&1 || die "no ps on PATH"
ps -o rss= -p $$ >/dev/null 2>&1 || die "ps cannot report rss on this host"

WORK="$(mktemp -d "${TMPDIR:-/tmp}/bench-rss.XXXXXX")"
LOG="$WORK/app.log"
"$APP" >"$LOG" 2>&1 &
APP_PID=$!
# Removing the job from bash's table keeps the SIGTERM notice out of the report.
disown "$APP_PID" 2>/dev/null || true

cleanup() {
  rc=$?
  trap - EXIT
  if kill -0 "$APP_PID" 2>/dev/null; then
    kill -TERM "$APP_PID" 2>/dev/null || true
    waited=0
    while kill -0 "$APP_PID" 2>/dev/null && [ "$waited" -lt 30 ]; do
      sleep 0.1
      waited=$((waited + 1))
    done
    kill -KILL "$APP_PID" 2>/dev/null || true
  fi
  wait "$APP_PID" 2>/dev/null || true
  exit "$rc"
}
trap cleanup EXIT

peak_kb=0
last_kb=0
samples=0
deadline=$(( $(date +%s) + WINDOW ))
while [ "$(date +%s)" -lt "$deadline" ]; do
  kill -0 "$APP_PID" 2>/dev/null || die "app exited before the window ended; see $LOG"
  rss="$(ps -o rss= -p "$APP_PID" 2>/dev/null | tr -d '[:space:]')"
  [ -n "$rss" ] || die "ps lost the app process; see $LOG"
  case "$rss" in (*[!0-9]*) die "unexpected ps rss value: $rss" ;; esac
  if [ "$rss" -gt "$peak_kb" ]; then peak_kb="$rss"; fi
  last_kb="$rss"
  samples=$((samples + 1))
  sleep "$INTERVAL"
done
[ "$samples" -gt 0 ] || die "no samples collected; see $LOG"

peak_mib="$(awk -v k="$peak_kb" 'BEGIN { printf "%.1f", k / 1024 }')"
printf '%s\n' \
  '=== Evidence: scripts/ci/bench-rss.sh (#64) ===' \
  "date:      $(date -u '+%Y-%m-%dT%H:%M:%SZ')" \
  "commit:    $(git rev-parse --short HEAD 2>/dev/null || printf unknown)" \
  "host:      $(uname -srm)" \
  "app:       $APP" \
  "window:    ${WINDOW}s, interval ${INTERVAL}s, ${samples} samples" \
  'scope:     main process RSS only (child/webview processes excluded)' \
  "peak_rss:  ${peak_kb} kB (${peak_mib} MiB)" \
  "last_rss:  ${last_kb} kB" \
  "app_log:   $LOG" \
  "verdict:   MEASURED on $HOST; not evidence for any other host"
