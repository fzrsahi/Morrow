#!/usr/bin/env bash
# Which CI lanes actually need to run for a set of changed paths (#369).
#
# Usage:
#   scripts/ci/path-router.sh <path> [<path> ...]     # explicit list
#   printf '%s\n' path... | scripts/ci/path-router.sh # stdin
#   CI_PIPELINE_FILES='a b' scripts/ci/path-router.sh  # Woodpecker
#
# Prints one lane per line, in a fixed order: linux, macos, windows, release.
# Prints nothing when the change cannot affect any lane (docs-only PRs).
set -euo pipefail

# Input priority: arguments, then the Woodpecker variable, then stdin.
# stdin is last so an empty inherited pipe cannot shadow CI_PIPELINE_FILES.
# Paths with spaces are not expected in CI file lists.
files=()
if [ "$#" -gt 0 ]; then
    files=("$@")
elif [ -n "${CI_PIPELINE_FILES:-}" ]; then
    read -r -a files <<<"$(printf '%s' "$CI_PIPELINE_FILES" | tr '\n' ' ')"
elif [ ! -t 0 ]; then
    while IFS= read -r line; do
        [ -n "$line" ] && files+=("$line")
    done
fi

want_linux=0
want_macos=0
want_windows=0
want_release=0

for raw in ${files[@]+"${files[@]}"}; do
    path="${raw#./}"

    # Infrastructure changes apply to every test lane. Be conservative here:
    # a broken runner or hook governs all platforms, so skipping one is worse
    # than running an extra lane.
    case "$path" in
        .woodpecker/*|.woodpecker.yaml|.githooks/*|\
        scripts/setup-*-runner.sh|scripts/ci/path-router.sh|\
        scripts/ci/test-path-router.sh|scripts/ci/test-multiplatform-ci.sh|\
        scripts/ci/verify-local.sh)
            want_linux=1; want_macos=1; want_windows=1
            continue
            ;;
    esac

    # Release-only artifacts must not drag the test lanes along.
    case "$path" in
        macos/appcast.xml|macos/ci/*|scripts/release/*)
            want_release=1
            continue
            ;;
    esac

    # The contract is shared by every agent implementation.
    case "$path" in
        shared/contract/*)
            want_linux=1; want_macos=1; want_windows=1
            continue
            ;;
    esac

    case "$path" in
        macos/*|Sources/*|Tests/*|Package.swift|Package.resolved|Info.plist|\
        *.entitlements|scripts/*macos*|scripts/run-xctest-runner.py)
            want_macos=1
            ;;
        linux/*|shared/*|crates/*|Cargo.toml|Cargo.lock|rust-toolchain.toml|\
        scripts/ci/linux-*|scripts/ci/rust-*)
            want_linux=1
            ;;
        windows/*|scripts/ci/windows-*)
            want_windows=1
            ;;
        docs/*|*.md)
            # Documentation-only changes cannot break a build.
            ;;
        *)
            # Unknown path: run everything rather than silently skip CI. A new
            # top-level directory needs its owner to add a rule here.
            want_linux=1; want_macos=1; want_windows=1
            ;;
    esac
done

[ "$want_linux" -eq 1 ] && echo linux
[ "$want_macos" -eq 1 ] && echo macos
[ "$want_windows" -eq 1 ] && echo windows
[ "$want_release" -eq 1 ] && echo release
exit 0
