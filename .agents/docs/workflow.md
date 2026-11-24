# Local-first development and PR workflow (normative)

## 1. Verify locally before pushing

Never use CI runners as a trial-and-error environment. Code must pass local build and unit tests before a PR opens.

```bash
./scripts/ci/verify-local.sh
git config core.hooksPath .githooks   # one-time per clone
```

The `.githooks/pre-commit` hook runs the same guard. It also runs `shared/ci/check-crate-boundaries.sh`, which enforces the allowed internal dependencies listed below. `linux/ci/test.sh` runs the same check in CI. On macOS, `cargo tauri dev` validates only the Svelte preview, not native window docking. The Rust compiler strips Wayland and Windows code when built on macOS. Use containers (OrbStack, Xvfb) or VMs for OS-level windowing.

## 2. Issue first, one issue per PR

Open an issue before writing code. Branch from fresh `main`:

```bash
git checkout main && git pull origin main
git checkout -b <type>/#<id>-<short-description>
```

Allowed types: `feat`, `fix`, `perf`, `refactor`, `chore`. A PR addresses exactly one issue and links it with `Fixes #<id>` or `Refs #<id>`. Keep diffs under 250 lines where possible.

## 3. Stacked PRs

When stages depend on each other, split into separate issues. Branch the child from the parent branch and target it in GitHub:

```bash
git checkout -b feat/#121-ui-integration feat/#120-core-parser
gh pr create --base feat/#120-core-parser --title "feat(#121): ui integration"
```

After the base merges, retarget the child to `main` with `gh pr edit <pr-id> --base main`. Never stack on a base whose CI is red. Keep `Cargo.lock` in the same commit as any dependency change, because CI uses `--locked`.

## 4. Commit and PR format

Commit format: `feat(#<id>): <description>` or `fix(#<id>): <description>`. PR body must contain Summary (visual flow or architecture), Evidence (test logs, Before/After for UI), and Merge Danger (one-way or two-way door plus blast radius). Missing sections are notes, not blockers; see the review policy doc.

## 5. `gh` cheatsheet

```bash
gh issue create --title "<type>(<scope>): <summary>" --body "<requirements & acceptance criteria>"
gh pr create --base main --title "<type>(#<id>): <summary>" --body "Fixes #<id>\n\n## Summary\n..."
gh pr create --base <parent-branch> --title "<type>(#<id>): <summary>" --body "Refs #<id>\nDepends on #<parent-pr>\n\n## Summary\n..."
gh pr edit <pr-id> --base main
```

## 6. Post-merge cleanup

```bash
git checkout main
git pull origin main
git branch -d <old-branch>
git fetch --prune
```

## 7. Two rules from the wider organisation that still apply

Reuse before writing: search for an existing helper or crate before adding a new one. Ponytail: prefer the shortest working solution, the standard library, and the existing table over a new abstraction or dependency.

## 8. Crate boundaries

`shared/ci/check-crate-boundaries.sh` is the test behind this rule. Allowed direct internal dependencies:

- `morrow-protocol`, `morrow-core`, `morrow-ipc`, `morrow-watch`: no internal dependencies.
- `morrow-platform`: `morrow-core`.
- `morrow-bridge`: `morrow-core`, `morrow-ipc`, `morrow-protocol`.
- `morrow-app`: all six crates.

`morrow-core` and `morrow-protocol` are pure. They must not add `libc`, `windows`, `windows-sys`, `nix`, `zbus`, or `tokio` directly. `shared/ci/rust-purity.sh` checks the transitive tree; this script checks `Cargo.toml`. Add a new crate to both the script and this table in the same PR.
