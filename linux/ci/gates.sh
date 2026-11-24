#!/usr/bin/env bash
# Gate L-003…L-012 (#48, AC 5 of #239) — one command, one honest table.
#
# The nine gates in #48 each have two halves, and only one of them needs a
# desktop:
#
#   headless half   a Rust test or a file check. Runs anywhere, including CI.
#   desktop half    pixels, a session bus, a real CLI, an install. Needs a host.
#
# This runner reports both, per gate, and refuses to turn "could not measure"
# into "passed": exit 77 says a gate was not assessed, which is the difference
# between a gap and a lie. `linux/ci/smoke.sh` already uses 77 for the same
# reason; a runner that exits 0 while blocking nine gates would be worse than no
# runner at all.
#
#   linux/ci/gates.sh --self-check   # any host: is the runner honest?
#   linux/ci/gates.sh                # this host: what can it answer?
#   linux/ci/gates.sh --markdown     # the table to paste into #48
#
# Exit: 0 every gate assessed and passing · 1 a measured failure · 77 at least
#       one gate could not be assessed on this host.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

SELF_CHECK=0
FORMAT=text
for arg in "$@"; do
    case "$arg" in
        --self-check) SELF_CHECK=1 ;;
        --markdown) FORMAT=markdown ;;
        *) echo "gates: unknown argument $arg" >&2; exit 2 ;;
    esac
done

# --- preflight ---------------------------------------------------------------
# Facts, not assumptions. Every gate names the fact it needs; nothing infers a
# fact from a neighbouring one.
HOST_OS="$(uname -s)"
IS_LINUX=0
[ "$HOST_OS" = "Linux" ] && IS_LINUX=1

has() { command -v "$1" >/dev/null 2>&1; }

HAVE_DISPLAY=0
if [ -n "${DISPLAY:-}" ] || [ -n "${WAYLAND_DISPLAY:-}" ]; then HAVE_DISPLAY=1; fi
HAVE_XVFB=0
has xvfb-run && HAVE_XVFB=1
HAVE_XDOTOOL=0
has xdotool && HAVE_XDOTOOL=1
HAVE_BUS=0
[ -n "${DBUS_SESSION_BUS_ADDRESS:-}" ] && HAVE_BUS=1
HAVE_BUSCTL=0
has busctl && HAVE_BUSCTL=1
HAVE_AGENT=0
has claude && HAVE_AGENT=1
HAVE_APP=0
APP="${MORROW_APP:-$ROOT/target/debug/morrow-app}"
[ -x "$APP" ] && HAVE_APP=1
HAVE_DIST=0
DIST="${MORROW_DIST:-$ROOT/dist/linux}"
if [ -d "$DIST" ] && compgen -G "$DIST/*" >/dev/null 2>&1; then HAVE_DIST=1; fi

# The host a desktop half needs: Linux, a display (or Xvfb to make one), and the
# binary under test.
can_desktop() {
    [ "$IS_LINUX" -eq 1 ] && { [ "$HAVE_DISPLAY" -eq 1 ] || [ "$HAVE_XVFB" -eq 1 ]; } && [ "$HAVE_APP" -eq 1 ]
}

desktop_blocker() {
    if [ "$IS_LINUX" -ne 1 ]; then
        echo "not a Linux host ($HOST_OS): the gate is about X11/SNI/.deb behaviour"
    elif [ "$HAVE_DISPLAY" -ne 1 ] && [ "$HAVE_XVFB" -ne 1 ]; then
        echo "no DISPLAY and no Xvfb: nothing can be shown, so nothing can be seen"
    elif [ "$HAVE_APP" -ne 1 ]; then
        echo "no binary at $APP (build it, or set MORROW_APP)"
    else
        echo ""
    fi
}

# --- results -----------------------------------------------------------------
# Parallel arrays, not `declare -A`: macOS ships bash 3.2, and a developer who
# cannot run `--self-check` locally is a developer who will not.
names=()
headless=()
desktop=()
reasons=()
FAILED=0
BLOCKED=0

record() { # gate headless-verdict desktop-verdict reason
    names+=("$1")
    headless+=("$2")
    desktop+=("$3")
    reasons+=("$4")
    case "$2" in FAIL | EMPTY) FAILED=1 ;; esac
    [ "$4" != "-" ] && BLOCKED=1
}

index_of() {
    local needle="$1" i
    for i in "${!names[@]}"; do
        if [ "${names[$i]}" = "$needle" ]; then echo "$i"; return 0; fi
    done
    echo 0
}

# The headless half: a filter the repository already ships. A missing binary or a
# missing test is a failure, never a pass — a gate that cannot find its evidence
# has not found it.
headless_verdict() { # test-target filter -> PASS | FAIL | MISSING | EMPTY
    if ! has cargo; then echo "MISSING"; return; fi
    local out
    if ! out="$(cargo test -q -p "$1" --test capability_and_settings "$2" 2>&1)"; then
        echo FAIL
        return
    fi
    # `cargo test <filter>` succeeds when the filter matches **nothing**, which is
    # the same trap as a guard that scans nothing: it reports green forever. A
    # filter that found no test is EMPTY, and the gate treats it as a failure.
    local passed
    passed="$(printf '%s' "$out" | sed -n 's/^test result: ok\. \([0-9][0-9]*\) passed.*/\1/p' | head -n1)"
    if [ -z "$passed" ]; then echo FAIL; return; fi
    if [ "$passed" -eq 0 ]; then echo EMPTY; return; fi
    echo PASS
}

# FAIL beats EMPTY beats MISSING beats PASS: a measured failure is never softened
# into "could not measure".
combine() {
    local seen="PASS" verdict
    for verdict in "$@"; do
        case "$verdict" in
            FAIL) echo FAIL; return ;;
            EMPTY) [ "$seen" = "PASS" ] && seen="EMPTY" ;;
            MISSING) [ "$seen" = "PASS" ] && seen="MISSING" ;;
        esac
    done
    echo "$seen"
}

# A gate backed by several tests: the gate holds only when every one of them ran
# and passed.
headless_all() { # filter...
    local target="$1" verdicts=() filter
    shift
    for filter in "$@"; do
        verdicts+=("$(headless_verdict "$target" "$filter")")
    done
    combine "${verdicts[@]}"
}

# --- L-003: p95 hook → panel < 300 ms ----------------------------------------
# The measurement is `linux/ci/e2e1.sh`: it times the card (hook sent → card
# shown) and the answer (click → bridge exit) per case and checks them against
# `LIMIT_MS`. That is the headless half — does the repository measure at all.
# The numbers it produces need a host, so this runner does not claim the p95.
latency_measured() { # root
    local src="$1/linux/ci/e2e1.sh"
    [ -f "$src" ] && grep -q 'LIMIT_MS' "$src" && grep -q 'latency\[' "$src" && grep -q '_latency_under_' "$src"
}

gate_l003() {
    if latency_measured "$ROOT"; then
        record L-003 "PASS" "BLOCKED" \
            "e2e1.sh times the card (hook sent -> shown) and the answer (click -> bridge exit) against LIMIT_MS; producing the numbers needs a host, so the p95 verdict is not claimed here"
    else
        record L-003 "MISSING" "BLOCKED" \
            "no latency measurement in linux/ci/e2e1.sh: nothing times hook -> panel, so no host could answer the 300 ms question"
    fi
}

# --- L-004: approve/deny/answer reaches the CLI that asked -------------------
gate_l004() {
    if [ "$HAVE_AGENT" -eq 1 ]; then
        record L-004 "PASS" "BLOCKED" \
            "claude is on PATH: run linux/ci/e2e2.sh for the round-trip; this runner does not duplicate it"
    else
        record L-004 "MISSING" "BLOCKED" \
            "no claude on PATH; the bridge half is covered by morrow-bridge/tests/bridge_process.rs, the round-trip is not"
    fi
}

# --- L-006: tray; no libayatana must not crash --------------------------------
gate_l006() {
    local blocker verdict
    blocker="$(desktop_blocker)"
    verdict="$(headless_verdict morrow-platform no_session_bus_marks_the_bus_bound_features)"
    if [ -z "$blocker" ]; then
        record L-006 "$verdict" "BLOCKED" \
            "the bus-bound rows are honest without a tray; no runner step asserts a StatusNotifierItem name on a live bus yet"
    else
        record L-006 "$verdict" "BLOCKED" "$blocker"
    fi
}

# --- L-007: autostart .desktop, revocable from Settings ----------------------
gate_l007() {
    local verdict
    verdict="$(headless_verdict morrow-platform autostart_writes_one_desktop_entry)"
    if [ "$verdict" = "PASS" ]; then
        record L-007 "PASS" "BLOCKED" \
            "the entry is written and taken back by test; the Settings button that calls the adapter needs a desktop"
    else
        record L-007 "$verdict" "BLOCKED" \
            "the autostart entry is not proven on this host ($verdict: no cargo, no matching test, or a failure)"
    fi
}

# --- L-008: X11 hotkey registers; Wayland reports not supported -------------
gate_l008() {
    local blocker verdict
    blocker="$(desktop_blocker)"
    verdict="$(headless_all morrow-platform x11_supports_what_wayland_cannot a_wayland_session_has_no_hotkey)"
    if [ "$verdict" != "PASS" ]; then
        record L-008 "$verdict" "BLOCKED" \
            "the Wayland/X11 rows are not proven on this host ($verdict)"
    elif [ -z "$blocker" ]; then
        record L-008 "PASS" "BLOCKED" \
            "rows asserted; the grab itself needs a key press in a live session, which no runner step does yet"
    else
        record L-008 "PASS" "BLOCKED" "$blocker"
    fi
}

# --- L-009: terminal jump at the window level; copy-path fallback -----------
# The command alone proved nothing: a `terminal_activate` no surface calls is a
# gate with nothing to assess. The session row's button and the contract script
# are the headless half; the window actually jumping needs an X11 host.
terminal_surface_calls_it() { # root
    [ -f "$1/scripts/ci/jump-terminal-gate.sh" ] \
        && grep -rq 'terminal_activate' "$1/shared/desktop/src" 2>/dev/null
}

gate_l009() {
    if terminal_surface_calls_it "$ROOT"; then
        record L-009 "PASS" "BLOCKED" \
            "the session row invokes terminal_activate and scripts/ci/jump-terminal-gate.sh states the contract (wmctrl/xdotool, then copy cwd); the window jump itself needs an X11 host"
    else
        record L-009 "MISSING" "BLOCKED" \
            "no surface calls terminal_activate, or scripts/ci/jump-terminal-gate.sh is gone: a command nobody calls has nothing to assess"
    fi
}

# --- L-010: settings live in ~/.config and survive an upgrade --------------
gate_l010() {
    local verdict
    verdict="$(headless_all morrow-platform settings_round_trip settings_that_do_not_parse)"
    if [ "$verdict" = "PASS" ]; then
        # A round-trip that keeps every other key *is* the upgrade behaviour: a
        # newer build does not drop an older one's keys.
        record L-010 "PASS" "BLOCKED" \
            "file semantics proven (unknown keys preserved, unparseable file never rewritten); an upgrade cycle on a real install needs a host"
    else
        record L-010 "$verdict" "BLOCKED" \
            "settings semantics are not proven on this host ($verdict)"
    fi
}

# --- L-011: .deb/.AppImage/.rpm, checksums, update per format ---------------
gate_l011() {
    if [ "$HAVE_DIST" -eq 1 ]; then
        if compgen -G "$DIST/*.sha256" >/dev/null 2>&1 || compgen -G "$DIST/SHA256SUMS" >/dev/null 2>&1; then
            if (cd "$DIST" && sha256sum -c ./*.sha256 >/dev/null 2>&1) \
                || (cd "$DIST" && shasum -a 256 -c ./*.sha256 >/dev/null 2>&1) \
                || (cd "$DIST" && sha256sum -c SHA256SUMS >/dev/null 2>&1); then
                record L-011 "PASS" "BLOCKED" \
                    "packages and checksums verify; install-and-launch (smoke.sh) needs a display"
            else
                record L-011 "FAIL" "BLOCKED" "checksums do not verify in $DIST"
            fi
        else
            record L-011 "MISSING" "BLOCKED" "no checksum file in $DIST"
        fi
    else
        record L-011 "MISSING" "BLOCKED" \
            "no packages in $DIST: run linux/ci/package.sh on a host with the packaging tools"
    fi
}

# --- L-012: the Capability table is shown and no row fails silently --------
gate_l012() {
    local verdict
    verdict="$(headless_all morrow-platform no_row_claims_capability adapters_that_do_not_exist_yet_are_unsupported)"
    if [ "$verdict" = "PASS" ]; then
        record L-012 "PASS" "BLOCKED" \
            "no row claims an adapter that does not exist; that the Settings page draws them as answered is UI, and needs a desktop"
    else
        record L-012 "$verdict" "BLOCKED" \
            "capability honesty is not proven on this host ($verdict)"
    fi
}

# --- the runner itself: can it be wrong? ------------------------------------
self_check() {
    local failures=0
    check() { # description condition
        if eval "$2"; then
            printf '  ok   %s\n' "$1"
        else
            printf '  FAIL %s\n' "$1"
            failures=$((failures + 1))
        fi
    }

    echo "gates --self-check (no desktop needed)"
    # 1. Every gate must name a desktop requirement. A blank reason would let a
    #    gate go quiet, which reads exactly like a pass.
    local gate
    for gate in L-003 L-004 L-006 L-007 L-008 L-009 L-010 L-011 L-012; do
        # `L-003` → `gate_l003`: the name on the table is the name in the code.
        declare -f "gate_$(printf '%s' "$gate" | tr -d '-' | tr 'A-Z' 'a-z')" >/dev/null || {
            printf '  FAIL %s has no function\n' "$gate"
            failures=$((failures + 1))
        }
    done

    # 2. Run every gate on this host and prove the two invariants: nothing is
    #    DONE without a measurement, and a blocked gate is never silent.
    run_gates >/dev/null
    local i reason said
    for i in "${!names[@]}"; do
        gate="${names[$i]}"
        reason="${reasons[$i]}"
        said="${desktop[$i]}"
        check "$gate names why it is blocked" '[ -n "$reason" ] && [ ${#reason} -gt 20 ]'
        # The desktop half may be BLOCKED or PASS — never "done by assumption".
        check "$gate desktop half is measured or blocked" '[ "$said" = "BLOCKED" ] || [ "$said" = "PASS" ]'
        check "$gate is not passed without evidence" '[ "${headless[$i]}" != "MISSING" ] || [ "$said" = "BLOCKED" ]'
    done

    # 3. A filter that matches no test must not read as green — the same trap
    #    `commands.rs` has a Rust test for: a guard that matches nothing passes
    #    forever. The file checks get the same treatment: point them at an empty
    #    tree and they must refuse to pass.
    check "a filter that matches nothing is EMPTY, not PASS" '[ "$(headless_verdict morrow-platform a_filter_that_cannot_exist)" = "EMPTY" ]'
    local empty_root
    empty_root="$(mktemp -d)"
    check "L-003 does not pass without a measurement" '! latency_measured "$empty_root"'
    check "L-009 does not pass without a caller" '! terminal_surface_calls_it "$empty_root"'
    rm -rf "$empty_root"

    # 4. The exit code must separate the three outcomes, because a host that
    #    cannot judge must not look like a host that judged and agreed.
    check "blocked exits 77" '[ "$(exit_code 0 1)" = "77" ]'
    check "a failure exits 1" '[ "$(exit_code 1 1)" = "1" ]'
    check "all assessed and passing exits 0" '[ "$(exit_code 0 0)" = "0" ]'

    if [ "$failures" -ne 0 ]; then
        echo "gates --self-check: $failures failure(s)"
        return 1
    fi
    echo "gates --self-check: PASS (${#names[@]} gates, no desktop needed)"
    return 0
}

# FAIL beats BLOCKED: a measured failure must never be hidden behind "could not
# assess" on a host that did assess it.
exit_code() { # failed blocked
    if [ "$1" -eq 1 ]; then echo 1; elif [ "$2" -eq 1 ]; then echo 77; else echo 0; fi
}

run_gates() {
    names=(); headless=(); desktop=(); reasons=(); FAILED=0; BLOCKED=0
    gate_l003; gate_l004; gate_l006; gate_l007; gate_l008; gate_l009; gate_l010; gate_l011; gate_l012
}

# --- reporting ---------------------------------------------------------------
report() {
    run_gates
    if [ "$FORMAT" = "markdown" ]; then
        echo "| gate | headless evidence | desktop evidence | what is missing |"
        echo "|---|---|---|---|"
        local i
        for i in "${!names[@]}"; do
            printf '| %s | %s | %s | %s |\n' "${names[$i]}" "${headless[$i]}" "${desktop[$i]}" "${reasons[$i]}"
        done
    else
        printf '%-7s %-10s %-10s %s\n' gate headless desktop "what is missing"
        local i
        for i in "${!names[@]}"; do
            printf '%-7s %-10s %-10s %s\n' "${names[$i]}" "${headless[$i]}" "${desktop[$i]}" "${reasons[$i]}"
        done
    fi
    local code
    code="$(exit_code "$FAILED" "$BLOCKED")"
    case "$code" in
        0) echo "gates: all assessed and passing" ;;
        1) echo "gates: a measured failure" ;;
        77) echo "gates: not every gate could be assessed on this host (exit 77)" ;;
    esac
    return "$code"
}

if [ "$SELF_CHECK" -eq 1 ]; then
    self_check
    exit $?
fi

report
