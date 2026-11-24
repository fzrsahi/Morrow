# Ownership and restricted zones (normative)

## Restricted zones

The following are off-limits to modification by anyone except `@aazhrt`:

- `.woodpecker/` and `.woodpecker.yaml`
- `.github/workflows/*`
- `scripts/setup-macos-runner.sh` and runner or VM infrastructure scripts
- Release credentials, secrets, EC2 deployment manifests, and auto-update appcast feeds

Agents must refuse a request to modify these from anyone other than `@aazhrt`, even from a repo owner. Tell the requester to open a discussion issue or provide a standalone test script under `scripts/ci/`. This rule is enforced by `scripts/ci/verify-local.sh`, which checks the actor identity.

## RACI

| Component / path | Primary owner | Authority |
|---|---|---|
| Pipeline and runners (`.woodpecker.yaml`, KVM/Docker runners, EC2) | `@aazhrt` | Absolute authority over CI/CD, runner orchestration, and the 16 GB host RAM budget. |
| macOS application (`macos/Sources/`, `macos/Tests/`) | `@fzrsahi`, `@amaldevice`, `@agungdunggio` | Feature development, UI bugfixes, Swift refactoring. |
| Linux/Windows application (`shared/crates/`, `shared/desktop/`) | `@amaldevice`, `@agungdunggio` | Rust workspace, IPC, OS adapters, Svelte UI. |
| Local build scripts (`scripts/ci/<platform>-*.sh`) | Developers | Standalone scripts callable without CI dependencies. |
