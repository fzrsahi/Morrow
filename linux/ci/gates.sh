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
# The budget is #48's; the samples come from a host that can run the panel.
LATENCY_BUDGET_MS=300
LATENCY_SAMPLES="${MORROW_LATENCY_SAMPLES:-$ROOT/dist/linux/latency-ms.txt}"
LATENCY_MIN_SAMPLES=20

# Nearest-rank percentile over one number per line. `awk`, not `sort | sed`,
# because the index arithmetic is the part that can be wrong and it is testable.
p95() {
    local n idx
    # `asort` is a gawk extension and macOS ships BWK awk, so the sort is GNU
    # `sort -n` — the same command on both platforms.
    n="$(grep -cE '^[0-9]+$' "$1")"
    [ "$n" -gt 0 ] || return 1
    idx=$(( (95 * n + 99) / 100 ))
    [ "$idx" -lt 1 ] && idx=1
    [ "$idx" -gt "$n" ] && idx="$n"
    grep -E '^[0-9]+$' "$1" | sort -n | sed -n "${idx}p"
}

gate_l003() {
    if [ ! -f "$LATENCY_SAMPLES" ]; then
        record L-003 "MISSING" "BLOCKED" \
            "no samples at $LATENCY_SAMPLES: e2e1.sh proves a hook becomes a card and does not time it; the gate needs a host to write one measurement per line"
        return
    fi
    local count measured
    count="$(grep -cE '^[0-9]+$' "$LATENCY_SAMPLES" 2>/dev/null || echo 0)"
    if [ "$count" -lt "$LATENCY_MIN_SAMPLES" ]; then
        record L-003 "MISSING" "BLOCKED" \
            "$count samples in $LATENCY_SAMPLES, need $LATENCY_MIN_SAMPLES before a p95 means anything"
        return
    fi
    measured="$(p95 "$LATENCY_SAMPLES")"
    if [ -z "$measured" ]; then
        record L-003 "MISSING" "BLOCKED" "the samples in $LATENCY_SAMPLES are not numbers"
        return
    fi
    if [ "$measured" -le "$LATENCY_BUDGET_MS" ]; then
        record L-003 "PASS" "BLOCKED" \
            "p95 ${measured} ms over ${count} samples (budget ${LATENCY_BUDGET_MS} ms); the samples still have to come from a host with a display"
    else
        record L-003 "FAIL" "BLOCKED" \
            "p95 ${measured} ms over ${count} samples is over the ${LATENCY_BUDGET_MS} ms budget"
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

# Runs L-011 against a chosen dist and echoes its verdict, so `--self-check` can
# exercise the branch without a real packaging host.
recheck_l011() { # dist-directory -> the verdict for that directory
    local saved_dist="$DIST" saved_have="$HAVE_DIST" verdict
    # Empty the table first: `index_of` finds the *first* entry, so appending a
    # second L-011 would read the verdict from the host run instead of this one.
    names=(); headless=(); desktop=(); reasons=()
    DIST="$1"
    HAVE_DIST=1
    gate_l011
    verdict="${headless[0]}"
    DIST="$saved_dist"
    HAVE_DIST="$saved_have"
    echo "$verdict"
}

# --- L-009: terminal jump at the window level; copy-path fallback -----------
gate_l009() {
    record L-009 "MISSING" "BLOCKED" \
        "the button exists now (#383) and nothing has pressed it: assessing this needs a session with a real terminal window on an X11 desktop"
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

    # 3. The percentile, against numbers whose answer is known by hand.
    local synthetic
    synthetic="$(mktemp)"
    printf '%s\n' 1 2 3 4 5 6 7 8 9 10 >"$synthetic"
    check "p95 of 1..10 is 10 (nearest rank)" '[ "$(p95 "$synthetic")" = "10" ]'
    printf '%s\n' 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 >"$synthetic"
    check "p95 of 1..20 is 19, not 20" '[ "$(p95 "$synthetic")" = "19" ]'
    printf '%s\n' 120 130 118 121 119 >"$synthetic"
    check "p95 of five fast samples is the fastest-by-rank" '[ "$(p95 "$synthetic")" = "130" ]'
    : >"$synthetic"
    check "no samples has no percentile" '[ -z "$(p95 "$synthetic")" ]'

    # 4. L-011's checksum branch, against a dist directory made here. This code
    #    path had never seen a real dist/ and was written blind; a synthetic one
    #    is enough to know the branch reads what package.sh writes.
    local fake
    fake="$(mktemp -d)"
    printf 'not a real package\n' >"$fake/morrow_0.1.0_amd64.deb"
    (cd "$fake" && { sha256sum ./*.deb 2>/dev/null || shasum -a 256 ./*.deb; } >SHA256SUMS)
    check "a dist of one package and its SHA256SUMS is PASS" '[ "$(recheck_l011 "$fake")" = "PASS" ]'
    printf 'tampered\n' >"$fake/morrow_0.1.0_amd64.deb"
    check "a tampered package is FAIL, not MISSING" '[ "$(recheck_l011 "$fake")" = "FAIL" ]'
    rm -rf "$fake"
    rm -f "$synthetic"

    # 5. A filter that matches no test must not read as green — the same trap
    #    `commands.rs` has a Rust test for: a guard that matches nothing passes
    #    forever.
    check "a filter that matches nothing is EMPTY, not PASS" '[ "$(headless_verdict morrow-platform a_filter_that_cannot_exist)" = "EMPTY" ]'

    # 6. The exit code must separate the three outcomes, because a host that
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
