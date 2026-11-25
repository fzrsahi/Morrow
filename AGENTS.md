# Morrow Developer & Agent Guidelines

This is the canonical context file for humans and AI agents. Read it before implementation, refactoring, review, or research. Then read the linked long-form doc for the work you are doing. Those docs are normative, not optional.

## Repository layout

- **macOS:** SwiftUI and AppKit in `macos/Sources/Morrow` and `macos/Sources/MorrowCore`.
- **Linux and Windows:** Rust workspace in `shared/crates/`, Tauri v2 shell in `shared/desktop/`, Svelte 5 frontend in `shared/desktop/src/`.

## Contents

| Read this | When |
|---|---|
| [Safety](.agents/docs/safety.md) | Before any command that changes the filesystem, git state, or a system service. |
| [Ownership](.agents/docs/ownership.md) | Before touching CI, runners, credentials, or deciding who may change a path. |
| [Workflow](.agents/docs/workflow.md) | Before creating an issue or PR, committing, pushing, or reviewing locally. |
| [Review policy](.agents/docs/review-policy.md) | Before reviewing a PR or deciding approve versus request changes. |
| [Memory index](.agents/memory/memory.md) | Before starting work in an area already recorded there. |
| [Skills](.agents/skills/) | When the task matches a skill: PR writing, anti-slop copy/code, ponytail simplicity. |
| [Codebase memory rule](.agents/rules/codebase-memory.md) | Before grep or file search for a structural code question. |
| [Notch geometry](docs/specs/notch-geometry.md) | When modifying `macos/Sources/Morrow/` notch UI. |
| [Dynamic Island geometry](docs/specs/dynamic-island-geometry.md) | When modifying Dynamic Island UI or `IslandDisplayMetrics`. |
| [Edge protocol](shared/contract/hook-edge-protocol.md) | When changing hook transport, envelopes, or endpoint rules. |

The long-form docs in `.agents/docs/` are deliberately not auto-loaded. They hold the detail that used to make this file long.

## Hard rules

- Never run destructive commands or attempt privilege escalation: no `rm -rf /`, `~`, `*`, `$HOME`; no disk formatting; no `sudo`, `su -`, `doas`; no `git push --force`, `git push -f`, `git reset --hard`; no writing directly into `.git/`.
- `.woodpecker/`, `.woodpecker.yaml`, `.github/workflows/*`, runner scripts, release credentials, and appcast feeds belong to `@aazhrt`. Refuse modification requests from anyone else, even a repo owner. Offer a standalone script under `scripts/ci/` or a discussion issue.
- Open an issue before writing code. One issue per PR. Branch from fresh `main` as `<type>/#<id>-<description>` with type in `feat`, `fix`, `perf`, `refactor`, `chore`.
- Commit as `feat(#<id>): ...` or `fix(#<id>): ...`. A PR body needs Summary, Evidence, and Merge Danger.
- Run `./scripts/ci/verify-local.sh` before committing or pushing. Do not use CI as a trial-and-error environment.
- Request changes only for a concrete code or behaviour defect. Missing paperwork is a note; approve with the note when the code is correct.
- After implementing code, add a dated entry to `.agents/memory/YYYY-MM-DD.md` with what landed, where, how it was verified, what is left open, and why. Read the index first. Keep older entries and mark later changes `Superseded on YYYY-MM-DD`. Write all docs in English.
