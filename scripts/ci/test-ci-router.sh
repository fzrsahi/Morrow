#!/usr/bin/env bash
# Local tests for scripts/ci/ci-router.sh (#369). No dependencies beyond bash.

set -eu

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
ROUTER="$ROOT/scripts/ci/ci-router.sh"

if [ ! -f "$ROUTER" ]; then
    echo "ERROR: missing $ROUTER" >&2
    exit 1
fi

unset CI_PIPELINE_FILES || true

PASSED=0
FAILED=0

assert_eq() {
    desc=$1
    expected=$2
    actual=$3
    if [ "$expected" = "$actual" ]; then
        echo "  [PASS] $desc"
        PASSED=$((PASSED + 1))
    else
        echo "  [FAIL] $desc"
        echo "         expected: '$expected'"
        echo "         actual:   '$actual'"
        FAILED=$((FAILED + 1))
    fi
}

normalize() {
    s=$1
    s=${s//$'\n'/ }
    printf '%s' "${s% }"
}

assert_args() {
    desc=$1
    expected=$2
    shift 2
    assert_eq "$desc" "$expected" "$(normalize "$("$ROUTER" "$@")")"
}

echo "==> ci-router lane tests"

HELP=$("$ROUTER" --help)
case $HELP in
    *"CI infra => ALL lanes"*) assert_eq "--help documents CI infra rule" "ok" "ok" ;;
    *) assert_eq "--help documents CI infra rule" "ok" "missing" ;;
esac

assert_args "linux/ only" "linux" "linux/ci/test.sh"
assert_args "macos/ only" "macos" "macos/Sources/Morrow/NotchView.swift"
assert_args "windows/ only" "windows" "windows/ci/check.sh"
assert_args "shared/ crate hits Rust lanes" "linux windows" "shared/crates/morrow-core/src/lib.rs"
assert_args "root Cargo.toml hits Rust lanes" "linux windows" "Cargo.toml"
assert_args "root Cargo.lock hits Rust lanes" "linux windows" "Cargo.lock"
assert_args "nested Cargo.toml hits Rust lanes" "linux windows" "shared/desktop/src-tauri/Cargo.toml"
assert_args "rust-toolchain hits Rust lanes" "linux windows" "rust-toolchain.toml"
assert_args ".woodpecker/ hits all lanes" "linux macos windows release" ".woodpecker/01-linux.yaml"
assert_args ".woodpecker.yaml hits all lanes" "linux macos windows release" ".woodpecker.yaml"
assert_args "runner setup hits all lanes" "linux macos windows release" "scripts/setup-macos-runner.sh"
assert_args "router itself hits all lanes" "linux macos windows release" "scripts/ci/ci-router.sh"
assert_args "router test hits all lanes" "linux macos windows release" "scripts/ci/test-ci-router.sh"
assert_args "release packaging hits release lane" "release" "scripts/ci/macos-package.sh"
assert_args "macOS packaging hits macos and release" "macos release" "macos/ci/package.sh"
assert_args "manifest hits Rust and release lanes" "linux windows release" "shared/ci/manifest.sh"
assert_args "appcast hits release lane" "release" "dist/appcast.xml"
assert_args "macOS source does not hit release lane" "macos" "macos/Info.plist"
assert_args "mixed input is deduplicated and ordered" "linux macos windows release" "macos/Sources/A.swift" "linux/src/main.rs" "windows/ci/check.sh" "shared/crates/lib.rs" "deploy/ec2.yml" "linux/src/main.rs"
assert_args "comma-separated input" "linux windows" "linux/a.rs,windows/b.rs"
assert_args "leading ./ is ignored" "macos" "./macos/Sources/A.swift"
assert_args "paths with spaces survive" "linux" "linux/my file.rs"
assert_args "docs only has no lane" "" "docs/specs/notch-geometry.md"
assert_args "no input has no lane" ""

STDIN_OUT=$(printf '%s\n' 'macos/ci/test.sh' | "$ROUTER")
assert_eq "stdin input" "macos" "$(normalize "$STDIN_OUT")"

ENV_OUT=$(CI_PIPELINE_FILES='linux/ci/test.sh
windows/src/main.rs' "$ROUTER")
assert_eq "CI_PIPELINE_FILES input" "linux windows" "$(normalize "$ENV_OUT")"

ARGS_OVER_ENV=$(CI_PIPELINE_FILES='macos/ci/test.sh' "$ROUTER" 'windows/ci/check.sh')
assert_eq "arguments override CI_PIPELINE_FILES" "windows" "$(normalize "$ARGS_OVER_ENV")"

echo "==> $PASSED passed, $FAILED failed"
[ "$FAILED" -eq 0 ]
