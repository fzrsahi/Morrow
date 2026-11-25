#!/usr/bin/env bash
# Print the Woodpecker lanes impacted by a set of changed files (#369).
# This script is standalone: @aazhrt wires its output into the pipelines.
# Full rule table: ci-router.sh --help

set -eu

LINUX=0
MACOS=0
WINDOWS=0
RELEASE=0

print_rules() {
    cat <<'RULES'
Usage: ci-router.sh [path ...]
       ci-router.sh < changed-files.txt
       CI_PIPELINE_FILES='path1
path2' ci-router.sh

Input precedence: arguments, then CI_PIPELINE_FILES, then stdin.
Paths are repo-relative; one path per line or comma-separated; leading ./ is ignored.
Output: impacted lanes in fixed order (linux, macos, windows, release), one per line.
No impacted lane prints nothing to stdout.

Rules (a file may match more than one rule):

  CI infra => ALL lanes
    .woodpecker/ or .woodpecker.yaml
    .github/workflows/
    scripts/setup-*-runner.sh, scripts/systemd/
    scripts/ci/ci-router.sh, scripts/ci/test-ci-router.sh
    scripts/ci/verify-local.sh, scripts/ci/test-multiplatform-ci.sh,
    scripts/ci/test-verify-local.sh
    scripts/report-status.py, scripts/report-pr-failure.py

  macos
    macos/**, root Package.swift or Info.plist

  linux
    linux/**, scripts/ci/linux-*

  windows
    windows/**, scripts/ci/windows-*

  linux + windows (the Rust lanes)
    shared/** (crates, desktop shell, contract),
    Cargo.toml / Cargo.lock at any level, rust-toolchain* at any level

  release
    deploy/**, macos/ci/package.sh, scripts/ci/macos-package.sh,
    shared/ci/manifest.sh, scripts/upload-github-release.py,
    appcast*.xml at any level

  Anything else (docs/, assets/, ...) => no lane
RULES
}

set_all() {
    LINUX=1
    MACOS=1
    WINDOWS=1
    RELEASE=1
}

route_path() {
    p=$1
    # Trim whitespace and normalize a leading ./ so callers can pass either form.
    p=${p#"${p%%[![:space:]]*}"}
    p=${p%"${p##*[![:space:]]}"}
    [ -n "$p" ] || return 0
    while :; do
        case $p in
            ./*) p=${p#./} ;;
            *) break ;;
        esac
    done

    case $p in
        .woodpecker|.woodpecker/*|.woodpecker.yaml|.github/workflows/*) set_all; return 0 ;;
        scripts/setup-*-runner.sh|scripts/systemd/*) set_all; return 0 ;;
        scripts/ci/ci-router.sh|scripts/ci/test-ci-router.sh|scripts/ci/verify-local.sh|scripts/ci/test-multiplatform-ci.sh|scripts/ci/test-verify-local.sh|scripts/report-status.py|scripts/report-pr-failure.py) set_all; return 0 ;;
    esac

    case $p in
        macos/*|Package.swift|Info.plist) MACOS=1 ;;
    esac
    case $p in
        linux/*|scripts/ci/linux-*) LINUX=1 ;;
    esac
    case $p in
        windows/*|scripts/ci/windows-*) WINDOWS=1 ;;
    esac
    case $p in
        shared/*|Cargo.toml|Cargo.lock|*/Cargo.toml|*/Cargo.lock|rust-toolchain*|*/rust-toolchain*) LINUX=1; WINDOWS=1 ;;
    esac
    case $p in
        deploy/*|macos/ci/package.sh|scripts/ci/macos-package.sh|shared/ci/manifest.sh|scripts/upload-github-release.py|appcast*.xml|*/appcast*.xml) RELEASE=1 ;;
    esac
}

if [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
    print_rules
    exit 0
fi

if [ "$#" -gt 0 ]; then
    INPUT=""
    for arg in "$@"; do
        INPUT="$INPUT$arg
"
    done
elif [ -n "${CI_PIPELINE_FILES:-}" ]; then
    INPUT=$CI_PIPELINE_FILES
else
    INPUT=""
    while IFS= read -r line || [ -n "$line" ]; do
        INPUT="$INPUT$line
"
    done
fi

# Split on commas only, so a path containing spaces survives intact.
OLD_IFS=$IFS
IFS=,
set -f
while IFS= read -r line || [ -n "$line" ]; do
    for token in $line; do
        route_path "$token"
    done
done <<< "$INPUT"
set +f
IFS=$OLD_IFS

if [ "$LINUX" -eq 1 ]; then printf 'linux\n'; fi
if [ "$MACOS" -eq 1 ]; then printf 'macos\n'; fi
if [ "$WINDOWS" -eq 1 ]; then printf 'windows\n'; fi
if [ "$RELEASE" -eq 1 ]; then printf 'release\n'; fi
exit 0
