#!/usr/bin/env bash
# Direct-dependency boundaries between workspace crates (#408).
# Mirrors the idea of ai-core's boundary_test.go: the rule lives in docs, and a
# check makes the rule real. Transitive OS dependencies are checked separately by
# rust-purity.sh; this script only reads Cargo.toml [dependencies] sections.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

python3 - <<'PY'
import re
from pathlib import Path

root = Path('.')
manifests = {
    'morrow-core': root / 'shared/crates/morrow-core/Cargo.toml',
    'morrow-protocol': root / 'shared/crates/morrow-protocol/Cargo.toml',
    'morrow-ipc': root / 'shared/crates/morrow-ipc/Cargo.toml',
    'morrow-platform': root / 'shared/crates/morrow-platform/Cargo.toml',
    'morrow-bridge': root / 'shared/crates/morrow-bridge/Cargo.toml',
    'morrow-watch': root / 'shared/crates/morrow-watch/Cargo.toml',
    'morrow-app': root / 'shared/desktop/src-tauri/Cargo.toml',
}
allowed_internal = {
    'morrow-core': set(),
    'morrow-protocol': set(),
    'morrow-ipc': set(),
    'morrow-watch': set(),
    'morrow-platform': {'morrow-core'},
    'morrow-bridge': {'morrow-core', 'morrow-ipc', 'morrow-protocol'},
    'morrow-app': {'morrow-core', 'morrow-ipc', 'morrow-platform', 'morrow-protocol', 'morrow-watch'},
}
os_crates = {'libc', 'windows', 'windows-sys', 'nix', 'zbus', 'tokio', 'socket2'}
pure_crates = {'morrow-core', 'morrow-protocol'}

section_re = re.compile(r'^\[([^\]]+)\]')
dep_re = re.compile(r'^\s*([A-Za-z0-9_-]+)\s*(?:[.=]|\s)')

def runtime_deps(path):
    deps = set()
    section = ''
    for line in path.read_text().splitlines():
        m = section_re.match(line)
        if m:
            section = m.group(1)
            continue
        if section == 'dependencies' or (section.startswith('target.') and section.endswith('.dependencies')):
            d = dep_re.match(line)
            if d:
                deps.add(d.group(1))
    return deps

problems = []
for crate, manifest in manifests.items():
    deps = runtime_deps(manifest)
    for dep in sorted(d for d in deps if d.startswith('morrow-')):
        if dep not in allowed_internal[crate]:
            problems.append(f'{crate}: may not depend on {dep}')
    if crate in pure_crates:
        for dep in sorted(deps & os_crates):
            problems.append(f'{crate}: pure crate may not depend on OS crate {dep}')

if problems:
    print('crate boundaries violated:')
    for p in problems:
        print(' -', p)
    raise SystemExit(1)
print('crate boundaries ok (7 crates)')
PY
