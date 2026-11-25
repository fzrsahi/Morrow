#!/usr/bin/env bash
# Gate E2E-1 (#91, docs/plan-windows-linux.md §7, Phase 1): a synthetic
# Claude Code PermissionRequest goes into the release morrow-bridge, the
# panel shows its card, xdotool clicks Approve (then Deny), and the bridge
# prints the macOS island's answer on stdout and exits 0, in under 300 ms
# without the human: (card shown − hook sent) + (bridge exit − click).
# Also: focus stays on the target window, a click outside the island
# reaches it, the panel is ABOVE and SKIP_TASKBAR, a pending card survives
# collapsing the panel, and a bridge that goes drops its card. Prints an
# Evidence block.
#
# Two modes:
#
#   linux/ci/e2e1.sh --self-check   # this script's own logic, any host, no display
#   linux/ci/e2e1.sh               # the full gate, needs X11 (real or Xvfb)
#
# --self-check feeds fixed fixtures to the same functions the full gate uses:
# the synthetic hook bytes, the button parser, the smoke-log readers, the
# 300 ms boundary and the honesty of check(). It prints a labelled template,
# never a gate verdict. Only the full run's Evidence block may be pasted as
# E2E-1 evidence.
#
# CI, in the image of linux/ci/Dockerfile (DISPLAY unset: Xvfb + Openbox):
#   (cd shared/desktop && bun install && bun run build)   # on the host
#   docker run --rm -v "$PWD":/work morrow-tauri-x11:dev linux/ci/e2e1.sh
#
# A real X11 desktop (DISPLAY set; the final judgement, Debian host #70):
# needs xdotool, x11-utils (xev, xprop, xdpyinfo), wmctrl, and GTK3 +
# WebKitGTK 4.1 to build. From the repo root, in the desktop session:
#   (cd shared/desktop && bun install && bun run build) && linux/ci/e2e1.sh
# For about 10 s it opens a full-screen xev window and moves the pointer:
# keep hands off. It uses a socket of its own, not a running Morrow's.
# Paste the whole '=== Evidence: linux/ci/e2e1.sh (gate E2E-1) ===' block
# into #91. A self-check template is not that block. If click-through or
# focus fails on the host, stop and report in #33 before Phase 2.
#
# Env: MORROW_APP, MORROW_BRIDGE (binaries; built when unset: app debug,
#      bridge release), E2E1_SCREEN (Xvfb only, default 1920x1080),
#      E2E1_COMMIT (for the Evidence block when git cannot read the
#      checkout, e.g. a worktree), CARGO_TARGET_DIR.
# Exit: 0 when every check passed, 1 otherwise; --self-check exits 0 only
#       when the script's own logic passed, and never assesses the gate.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"
NAME=e2e1
# shellcheck source=linux/ci/x11-lib.sh
. linux/ci/x11-lib.sh

LIMIT_MS=300
ALLOW='{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}'
DENY='{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny"}}}'

SELF_CHECK=0
for arg in "$@"; do
  case "$arg" in
    --self-check) SELF_CHECK=1 ;;
    *) echo "$NAME: unknown argument $arg (expected --self-check)" >&2; exit 2 ;;
  esac
done

# --- deterministic helpers ------------------------------------------------
# The full gate and --self-check share these. A fixture under --self-check
# exercises the same bytes and the same parser a host will use.
request() { # request <session> <command>: what Claude Code pipes into its hook
  printf '{"session_id":"%s","transcript_path":"/tmp/e2e1/%s.jsonl","cwd":"/home/dev/e2e1","permission_mode":"default","hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{"command":"%s","description":"E2E-1"}}' \
    "$1" "$1" "$2"
}
latency_ms() { # latency_ms <at> <T0> <t_exit> <t_click>: card delay + answer delay
  echo $(($1 - $2 + $3 - $4))
}
# card <id>/has_card <id>/reports <id>/cards_now read the app's smoke log in
# $WORK: the real scratch dir in full mode, a fixture in --self-check.
card() { grep "morrow-smoke: card id=$1 " "$WORK/app.out" | tail -n1; }
has_card() { grep -q "morrow-smoke: card id=$1 " "$WORK/app.out"; }
reports() { grep -c "morrow-smoke: card id=$1 " "$WORK/app.out" || true; }
cards_now() { grep 'morrow-smoke: pushed' "$WORK/app.out" | tail -n1 | sed -n 's/.*cards=//p'; }
# button <line> <approve|deny>: sets BX BY BW BH (window pixels).
button() { IFS=, read -r BX BY BW BH <<<"$(sed -n "s/.* $2=\([0-9,]*\).*/\1/p" <<<"$1")"; }
button_is() { # button_is <line> <which> <expected BX,BY,BW,BH>
  button "$1" "$2"
  [ "$BX,$BY,$BW,$BH" = "$3" ]
}

# --- --self-check: prove this script's own logic without a host -----------
# bash 3.2 compatible; needs no X11, display, frontend, binary or bridge. It
# cannot pass gate E2E-1: it proves the script under test is honest about the
# things a host will measure, before a host spends time on them.
self_check() {
  local fixture before_failed before_n probe_failed probe_n line

  check "the script parses" "bash -n on itself" bash -n "$0"

  # 1. The synthetic hook must be the exact bytes the bridge reads.
  check "request builds the PermissionRequest hook" "session, cwd, event and command" \
    test "$(request e2e1-approve 'echo approve')" = \
      '{"session_id":"e2e1-approve","transcript_path":"/tmp/e2e1/e2e1-approve.jsonl","cwd":"/home/dev/e2e1","permission_mode":"default","hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{"command":"echo approve","description":"E2E-1"}}'
  check "ALLOW is the allow decision bytes" "exact stdout of an approve" \
    test "$ALLOW" = '{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}'
  check "DENY is the deny decision bytes" "exact stdout of a deny" \
    test "$DENY" = '{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny"}}}'

  # 2. The button parser must read the coordinates the frontend prints.
  line='morrow-smoke: card id=1 approve=10,20,100,30 deny=10,60,100,30 at=12345'
  check "button() parses approve" "approve=10,20,100,30" button_is "$line" approve 10,20,100,30
  check "button() parses deny" "deny=10,60,100,30" button_is "$line" deny 10,60,100,30
  check "button() does not invent a missing button" "no such key gives an empty rectangle" button_is "$line" nope ,,,

  # 3. The log readers must count and pick the right lines in a fixed smoke log.
  fixture="$(mktemp -d "/tmp/$NAME-selfcheck.XXXXXX")"
  WORK="$fixture"
  cat >"$WORK/app.out" <<'LOG'
morrow-smoke: pushed sessions=0 cards=0
morrow-smoke: card id=1 approve=10,20,100,30 deny=10,60,100,30 at=100
morrow-smoke: pushed sessions=1 cards=1
morrow-smoke: card id=1 approve=12,22,100,30 deny=12,62,100,30 at=200
morrow-smoke: pushed sessions=1 cards=1
morrow-smoke: card id=2 approve=10,20,100,30 deny=10,60,100,30 at=300
morrow-smoke: pushed sessions=2 cards=1
morrow-smoke: pushed sessions=2 cards=0
LOG
  check "has_card finds a shown card" "id 1 is in the fixture log" has_card 1
  check "has_card does not invent a card" "id 9 is not" eval '! has_card 9'
  check "reports counts every fit of a card" "id 1 appears twice" eval '[ "$(reports 1)" = 2 ]'
  check "reports returns zero for an unknown card" "id 9 appears zero times" eval '[ "$(reports 9)" = 0 ]'
  check "card() returns the latest fit" "id 1 at=200, not at=100" eval 'case "$(card 1)" in *at=200) true;; *) false;; esac'
  check "cards_now reads the last push" "last push has cards=0" eval '[ "$(cards_now)" = 0 ]'

  # 4. The latency formula and the 300 ms boundary are the gate's timing claim.
  check "latency adds card delay and answer delay" "20 + 10 = 30 ms" test "$(latency_ms 500 480 540 530)" = 30
  check "a 299 ms round is under the limit" "299 < 300" test "$(latency_ms 300 1 1 1)" -lt "$LIMIT_MS"
  check "a 300 ms round is not under the limit" "300 is not < 300" test "$(latency_ms 300 0 0 0)" -ge "$LIMIT_MS"

  # 5. A runner that records a failure as a pass is worse than one that cannot
  # run. Probe that with a deliberately failing check, then undo only the probe.
  # Report the outcome WITHOUT check(): a check() that always passes must not
  # be the one that says it is honest.
  before_failed=$failed
  before_n=${#results[@]}
  check __honesty_probe "deliberate failure" false
  probe_failed=$failed
  probe_n=${#results[@]}
  failed=$before_failed
  unset "results[$((probe_n - 1))]"
  if [ "$probe_failed" -eq 1 ] && [ "$probe_n" -eq $((before_n + 1)) ]; then
    results+=("PASS  check() records failures honestly  (false set failed=1 and added one result)")
  else
    results+=("FAIL  check() records failures honestly  (failed=$probe_failed, results $before_n -> $probe_n)")
    failed=1
  fi

  rm -rf "$fixture"

  echo
  echo "=== Evidence: linux/ci/e2e1.sh --self-check (NOT an E2E-1 verdict) ==="
  echo "date:      $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "commit:    ${E2E1_COMMIT:-$(git -c safe.directory="$ROOT" rev-parse --short HEAD 2>/dev/null || echo unknown)}"
  echo "mode:      self-check only: no display, no app, no bridge used"
  echo "kernel:    $(uname -srm)"
  echo "fixtures:  synthetic PermissionRequest, smoke log lines, 299/300 ms latency"
  printf '%s\n' "${results[@]}"
  if [ "$failed" -eq 0 ]; then
    echo "result:    SELF-CHECK PASS (${#results[@]} checks); gate E2E-1 is NOT assessed"
    echo
    echo "On the Debian X11 host, from the repo root, in the desktop session:"
    echo "  (cd shared/desktop && bun install && bun run build) && linux/ci/e2e1.sh"
    echo "Paste ONLY the '=== Evidence: linux/ci/e2e1.sh (gate E2E-1) ===' block"
    echo "from that run into #91. This template is not that block."
  else
    echo "result:    SELF-CHECK FAIL"
  fi
  return "$failed"
}

if [ "$SELF_CHECK" -eq 1 ]; then
  self_check
  exit $?
fi

# --- full gate: prerequisites ---------------------------------------------
[ "${XDG_SESSION_TYPE:-}" != wayland ] || die "a Wayland session: E2E-1 is the X11 gate (XWayland is not X11)"
x11_binaries
x11_workdir
if [ -n "${DISPLAY:-}" ]; then
  display="real X11 $DISPLAY ($(xdpyinfo | sed -n 's/^vendor string: *//p'), session ${XDG_SESSION_TYPE:-?}, desktop ${XDG_CURRENT_DESKTOP:-?})"
  read -r SW SH <<<"$(xdotool getdisplaygeometry)"
else
  SCREEN="${E2E1_SCREEN:-1920x1080}"
  SW="${SCREEN%x*}"
  SH="${SCREEN#*x}"
  x11_xvfb_openbox
  display="Xvfb ${SW}x${SH}x24, no compositor"
fi
x11_target
x11_start_app

# --- one permission request --------------------------------------------
# send <name>: the bridge in the background; T0 is when the hook was sent,
# and $WORK/<name>.done gets "<status> <exit ms>" when it exits.
send() {
  request "e2e1-$1" "echo $1" >"$WORK/$1.in"
  T0=$(now_ms)
  (
    rc=0
    "$MORROW_BRIDGE" <"$WORK/$1.in" >"$WORK/$1.out" 2>"$WORK/$1.err" || rc=$?
    echo "$rc $(now_ms)" >"$WORK/$1.done"
  ) &
  pids+=($!)
}
holds() { geometry "$ISLAND" && [ "$H" -ge "$1" ]; }

declare -A latency
# round <name> <card id> <approve|deny> <expected stdout> [collapse]
round() {
  local name="$1" id="$2" which="$3" expected="$4" line at before t_click rc t_exit out total
  send "$name"
  if ! until_ok 5 has_card "$id"; then
    check "${name}_card_shown" "no card $id within 5 s" false
    return
  fi
  # Shown: the first report, once the window was fitted to the card.
  line="$(grep -m1 "morrow-smoke: card id=$id " "$WORK/app.out")"
  at="${line##*at=}"
  line="$(card "$id")"
  check "${name}_card_shown" "card $id, $((at - T0)) ms after the hook" true
  if [ "${5:-}" = collapse ]; then
    # Expand and collapse the panel (the pill, at the top) with the card
    # pending: the card stays, and is fitted again each time.
    before="$(reports "$id")"
    geometry "$ISLAND"
    xdotool mousemove --sync $((X + W / 2)) $((Y + 17)) click 1
    sleep 0.3
    xdotool mousemove --sync $((X + W / 2)) $((Y + 17)) click 1
    sleep 0.3
    check "${name}_card_survives_collapse" "fitted $before -> $(reports "$id") times, cards=$(cards_now)" \
      eval '[ "$(reports "$id")" -ge $((before + 2)) ] && [ "$(cards_now)" = 1 ]'
    line="$(card "$id")"
  fi
  button "$line" "$which"
  check "${name}_window_holds_card" "${which} at $BX,$BY ${BW}x$BH in the window" until_ok 2 holds $((BY + BH))
  check "${name}_bridge_waits" "no exit before the click" test ! -e "$WORK/$name.done"
  geometry "$ISLAND"
  before="$(clicks)"
  xdotool mousemove --sync $((X + BX + BW / 2)) $((Y + BY + BH / 2))
  t_click=$(now_ms)
  xdotool click 1
  if ! until_ok 5 test -s "$WORK/$name.done"; then
    check "${name}_bridge_exits" "still waiting 5 s after the click" false
    return
  fi
  read -r rc t_exit <"$WORK/$name.done"
  out="$(cat "$WORK/$name.out")"
  check "${name}_bridge_exit_0" "status $rc" test "$rc" = 0
  check "${name}_stdout" "$out" test "$out" = "$expected"
  total="$(latency_ms "$at" "$T0" "$t_exit" "$t_click")"
  latency[$name]="$((at - T0)) + $((t_exit - t_click)) = $total"
  check "${name}_latency_under_${LIMIT_MS}ms" "card $((at - T0)) ms + answer $((t_exit - t_click)) ms" \
    test "$total" -lt "$LIMIT_MS"
  check "${name}_click_stays_on_island" "target clicks $before -> $(clicks)" test "$(clicks)" = "$before"
  check "${name}_focus_kept_after_click" "active $(xdotool getactivewindow), target $TARGET" focused_is_target
  check "${name}_card_gone" "the next push has cards=0" until_ok 2 sh -c "grep 'morrow-smoke: pushed' '$WORK/app.out' | tail -n1 | grep -q 'cards=0$'"
}

# --- checks -------------------------------------------------------------
x11_window_checks
check focus_kept_after_show "active $(xdotool getactivewindow), target $TARGET" focused_is_target

round approve 1 approve "$ALLOW"
round deny 2 deny "$DENY" collapse

# A bridge that goes before the answer (Ctrl-C in Claude Code): its card goes.
request e2e1-gone "echo gone" >"$WORK/gone.in"
"$MORROW_BRIDGE" <"$WORK/gone.in" >"$WORK/gone.out" 2>&1 &
gone=$!
pids+=($gone)
if until_ok 5 has_card 3; then
  kill "$gone"
  check bridge_gone_drops_card "card 3, bridge killed: the next push has cards=0" \
    until_ok 2 sh -c "grep 'morrow-smoke: pushed' '$WORK/app.out' | tail -n1 | grep -q 'cards=0$'"
else
  check bridge_gone_drops_card "no card 3 within 5 s" false
fi

sleep 0.3 # the panel shrinks back
geometry "$ISLAND"
ox=$((SW / 2 - 300))
oy=$((SH / 2))
check outside_click_reaches_target "click ($ox,$oy), outside the ${W}x$H island" click_reaches_target "$ox" "$oy"
check focus_kept_at_end "active $(xdotool getactivewindow)" focused_is_target

# --- Evidence -----------------------------------------------------------
echo
echo "=== Evidence: linux/ci/e2e1.sh (gate E2E-1) ==="
echo "date:      $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "commit:    ${E2E1_COMMIT:-$(git -c safe.directory="$ROOT" rev-parse --short HEAD 2>/dev/null || echo unknown)}"
echo "display:   $display"
echo "wm:        $(wmctrl -m 2>/dev/null | sed -n 's/^Name: //p')"
echo "kernel:    $(uname -srm)"
echo "binaries:  app $MORROW_APP, bridge $MORROW_BRIDGE"
echo "windows:   target $TARGET (xev), island $ISLAND"
echo "latency:   (card shown - hook sent) + (bridge exit - click), limit < ${LIMIT_MS} ms"
for name in approve deny; do
  echo "           $name: ${latency[$name]:-none} ms"
done
echo "app start: island subscribed $((t_shown - t_app)) ms"
printf '%s\n' "${results[@]}"
if [ "$failed" -eq 0 ]; then
  echo "result:    PASS (${#results[@]} checks)"
else
  echo "result:    FAIL"
  echo "--- app stdout ---"
  tail -n 20 "$WORK/app.out"
  echo "--- app stderr ---"
  tail -n 20 "$WORK/app.err"
fi
exit "$failed"
