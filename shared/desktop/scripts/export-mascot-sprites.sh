#!/usr/bin/env bash
# Regenerate the web mascot sprite sheets from the macOS render harness (#181).
#
# The sheets are real frames of the SwiftUI mascots: the harness renders each
# (mascot, status) at fixed timeline instants through the same routing as the
# app, writes one horizontal PNG strip per pair and a manifest with frame
# counts. The export is opt-in (env var) and touches no macOS app code.
#
# Requires macOS + Swift. Run from anywhere:
#   shared/desktop/scripts/export-mascot-sprites.sh
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DESKTOP="$(cd "$HERE/.." && pwd)"
REPO="$(cd "$DESKTOP/../.." && pwd)"
ASSETS="$DESKTOP/src/lib/assets/mascots"

mkdir -p "$ASSETS"
MASCOT_WEB_SPRITE_DIR="$ASSETS" swift test --package-path "$REPO/macos" \
  --filter 'MascotRenderHarness/testExportWebSpriteSheets'
mv "$ASSETS/manifest.json" "$DESKTOP/src/lib/mascot-sprites.json"

echo "Exported mascot sprites to $ASSETS (manifest: src/lib/mascot-sprites.json)"
