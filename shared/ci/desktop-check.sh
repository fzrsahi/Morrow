#!/usr/bin/env bash
# Desktop shell check for shared/desktop: lint, type check, unit tests and the
# vite build. The Svelte/TypeScript half of the CI contract
# (docs/plan-windows-linux.md §8.1); linux/ci/test.sh covers the Rust crates.
#
# Usage: shared/ci/desktop-check.sh
#
# Output: dist/desktop/check.log (every step's full output, overwritten).
# Exit:   0 when every step passed, 1 otherwise. Every step runs either way.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DESKTOP="$ROOT/shared/desktop"
LOG="$ROOT/dist/desktop/check.log"
mkdir -p "$(dirname "$LOG")"
: >"$LOG"

if ! command -v bun >/dev/null 2>&1; then
  echo "desktop-check: 'bun' not found in PATH; install it (https://bun.sh) first" | tee -a "$LOG"
  exit 1
fi

cd "$DESKTOP"

# bun.lock is the install contract: fail on a dirty lock instead of resolving a
# different tree than the one the tests were written against.
if [ ! -d node_modules ]; then
  echo "==> bun install --frozen-lockfile" | tee -a "$LOG"
  bun install --frozen-lockfile 2>&1 | tee -a "$LOG"
fi

summary=()
failed=0

# step <label> <command…>: as in linux/ci/test.sh.
step() {
  local label="$1"
  shift
  printf '\n==> %s: %s\n' "$label" "$*" | tee -a "$LOG"
  local start=$SECONDS
  if "$@" 2>&1 | tee -a "$LOG"; then
    summary+=("PASS  $label ($((SECONDS - start))s)")
  else
    summary+=("FAIL  $label ($((SECONDS - start))s)")
    failed=1
  fi
}

{
  echo "desktop-check: $(date -u +%Y-%m-%dT%H:%M:%SZ) on $(uname -sm)"
  echo "commit: $(git -c safe.directory="$ROOT" rev-parse --short HEAD 2>/dev/null || echo unknown)"
  bun --version
} 2>&1 | tee -a "$LOG"

step lint bun run lint
step check bun run check
step test bun test
step build bun run build

{
  echo
  echo "Summary:"
  printf '  %s\n' "${summary[@]}"
  if [ "$failed" -eq 0 ]; then
    echo "desktop-check: all steps passed (log: ${LOG#"$ROOT"/})"
  else
    echo "desktop-check: FAILED (log: ${LOG#"$ROOT"/})"
  fi
} | tee -a "$LOG"

exit "$failed"
