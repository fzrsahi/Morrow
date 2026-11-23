#!/usr/bin/env bash
# Local tests for scripts/ci/path-router.sh (#369).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ROUTER="$ROOT/scripts/ci/path-router.sh"
FAILED=0
COUNT=0

fail() {
    printf '  [FAIL] %s\n    expected: %s\n    actual:   %s\n' "$1" "$2" "$3"
    FAILED=$((FAILED + 1))
}

check() { # check <name> <expected lanes> -- <args...>
    local name="$1" expected="$2" actual
    shift 3
    actual="$("$ROUTER" "$@" | paste -sd' ' -)"
    COUNT=$((COUNT + 1))
    if [ "$actual" = "$expected" ]; then
        printf '  [PASS] %s\n' "$name"
    else
        fail "$name" "$expected" "$actual"
    fi
}

check_stdin() { # check_stdin <name> <expected> <newline-separated paths>
    local name="$1" expected="$2" input="$3" actual
    actual="$(printf '%s\n' "$input" | "$ROUTER" | paste -sd' ' -)"
    COUNT=$((COUNT + 1))
    if [ "$actual" = "$expected" ]; then
        printf '  [PASS] %s\n' "$name"
    else
        fail "$name" "$expected" "$actual"
    fi
}

echo '--- CI path router ---'

check 'linux-only change runs only linux' 'linux' -- linux/ci/test.sh
check 'macos-only change runs only macos' 'macos' -- macos/Sources/Morrow/App.swift
check 'windows-only change runs only windows' 'windows' -- windows/ci/check.sh
check 'shared Rust change runs only linux' 'linux' -- shared/crates/morrow-core/src/lib.rs
check 'Cargo.lock runs only linux' 'linux' -- Cargo.lock
check 'rust-toolchain runs only linux' 'linux' -- rust-toolchain.toml
check 'contract change runs all test lanes' 'linux macos windows' -- shared/contract/hook-edge-protocol.md
check 'release artifact runs only release' 'release' -- macos/appcast.xml
check 'pipeline config runs all test lanes' 'linux macos windows' -- .woodpecker/01-linux.yaml
check 'runner setup runs all test lanes' 'linux macos windows' -- scripts/setup-linux-runner.sh
check 'the router itself runs all test lanes' 'linux macos windows' -- scripts/ci/path-router.sh
check 'docs-only change runs nothing' '' -- docs/specs/notch-geometry.md
check 'markdown-only change runs nothing' '' -- README.md
check 'mixed change runs both affected lanes' 'linux macos' -- macos/Sources/Morrow/App.swift linux/ci/test.sh

check_stdin 'stdin input works' 'linux' 'linux/ci/test.sh'
check_stdin 'multi-line stdin works' 'linux macos' $'linux/ci/test.sh\nmacos/Sources/Morrow/App.swift'

actual="$(CI_PIPELINE_FILES='linux/ci/test.sh windows/ci/check.sh' "$ROUTER" | paste -sd' ' -)"
COUNT=$((COUNT + 1))
if [ "$actual" = 'linux windows' ]; then
    printf '  [PASS] CI_PIPELINE_FILES input works\n'
else
    fail 'CI_PIPELINE_FILES input works' 'linux windows' "$actual"
fi

echo
echo '--- Results ---'
printf '  %d passed, %d failed\n' "$((COUNT - FAILED))" "$FAILED"
exit "$FAILED"
