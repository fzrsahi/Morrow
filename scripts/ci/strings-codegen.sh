#!/usr/bin/env bash
# #239 AC 4 — one string resource, used by macOS and by the desktop.
#
# `macos/Sources/Morrow/L10n.swift` holds seven tables of UI strings as Swift
# literals (en 568 keys, zh/zh-Hant 564 each, and de/ja/ko/tr as `en.merging(…)`
# overrides). Nothing outside Swift could read them, so the port was retyping the
# same labels by hand and the two drifted silently.
#
# This script owns both directions:
#
#   ./scripts/ci/strings-codegen.sh           write the JSON and both readers
#   ./scripts/ci/strings-codegen.sh --check    fail if either side has drifted
#
# The JSON in `shared/contract/strings/` is the source of truth. Swift keeps
# working through a generated extension (same values, emitted rather than typed),
# and the desktop imports the generated TypeScript.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
exec python3 "$root/scripts/ci/strings_codegen.py" "$@"
