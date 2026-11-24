# What changed from CodeIsland

Morrow is a fork of [wxtsky/CodeIsland](https://github.com/wxtsky/CodeIsland). Upstream is a SwiftUI and AppKit app for macOS. This fork adds a Rust and Tauri shell for Linux and Windows, renames many configuration paths, and adds features upstream does not document.

This list compares `wxtsky/CodeIsland` `main` (upstream README, 396 lines) with this repository. The evidence is code, ADRs, and both READMEs, not guesswork. "Wired" means the code is called from the shell or UI. "Library only" means the module exists but the UI does not call it.

## Identity and paths

| Change | Evidence |
|---|---|
| Product name CodeIsland to Morrow, repo to `fzrsahi/morrow` | Our README `:8-25`; upstream `:1-15`. The local checkout folder is still named `CodeIsland`, but repo paths and artifacts use `morrow`. |
| IPC socket `/tmp/codeisland-<uid>.sock` to `/tmp/morrow-<uid>.sock` | Our README `:355`; upstream `:322`. #78 / PR #161 added resolution to the legacy socket at send time so older hooks keep working. |
| Bridge `codeisland-bridge` to `morrow-bridge` | Our README `:355`; upstream `:322`. `build.sh` copies both, with an alias for the old name. |
| Eight hook and plugin paths renamed | Our README `:200-215`; upstream `:166-181`: Grok, Copilot, Kiro, MiniMax, OpenCode, MiMo, Pi / Oh My Pi, OpenClaw. Kiro also uses `kiro --agent morrow`. |
| Homebrew `codeisland` to `morrow`, DMG and app to `Morrow.*` | Our README `:227-233`; upstream `:193-199`. |
| Companion path `ios/CodeIslandCompanion` to `ios/MorrowCompanion` | Our README `:257`; upstream `:212`. |

The supported tool list did **not** change. The 28-tool grid in our README `:137-195` matches upstream `:103-150`. Only icon paths and file names differ. No tool was added to or removed from that list.

## Architecture

| Change | Evidence |
|---|---|
| Two cores, not one. macOS keeps Swift; Linux and Windows use a Rust core. They share a contract, not code. | ADR-0001 `:5`, `:34`, `:38-39`; ADR-0003 `:3,10` rejects Option E. Cost: two cores kept in step and a port of roughly 37K to 40K lines. |
| A Tauri v2 and Svelte 5 shell replaces WPF on Windows and serves as the Linux panel. | ADR-0002 `:5,7,9,15-16`; `shared/desktop/src-tauri/tauri.conf.json`. The decision was one shell for two OSes, not performance. |
| `morrow-core` is pure and guarded by `rust-purity.sh`: no dependency on `windows`, `libc`, `nix`, `zbus`, or `tokio`. | `shared/ci/rust-purity.sh`; `.agents/memory/2026-11-21.md`. Side effects are injected through `Clock`, `CustomSources`, and `FileSystem`. |
| One capability table is the source of truth. A `Supported` row needs an adapter in `ADAPTERS` or the guard test fails. | `capability.rs`; `tests/capability_and_settings.rs`; `.agents/memory/2026-11-23.md`. #235 demoted 11 Linux and 8 Windows rows to `Unsupported(reason)`. |
| Hook and endpoint contracts are frozen in `shared/contract/`, including golden fixtures and peer verification rules. | ADR-0001 `:5`; `shared/contract/hook-edge-protocol.md`; `.agents/memory/2026-11-21.md`. |
| Monorepo layout: `macos/`, `shared/`, `linux/`, `windows/`. | #86, PR #171 (`9680d4a5`). 712 files moved without behavior changes. |
| The Linux panel uses a tight-fit, nonactivating window instead of GTK click-through. | ADR-0002 `:20-25`; `.agents/memory/2026-11-21.md`. |
| Local verification is mandatory before push. Host gates do not run per PR. | `AGENTS.md:54-68`; `.agents/memory/2026-11-23.md`. |

## Features added in this fork

### Wired into the UI or shell

- **Session discovery from running processes.** 13 readers: `ClaudeTranscripts`, `OpenCodeStore`, `CodexRollouts`, `ClineStore`, `CursorTranscripts`, `GeminiChats`, `KimiSessions`, `KiroSessions`, `CopilotSessions`, `GrokSessions`, and three `FlatStore` instances (qoder, codebuddy, droid). Evidence: `morrow-core/src/discovery.rs`, `src-tauri/src/discovery.rs`, `src-tauri/src/feed.rs::scan_stores`. Upstream only saw sessions through hooks.
- **Backfill on attach.** Sessions already running appear before the first hook arrives. Evidence: `feed.rs::backfill_with`, `morrow-core/src/session_store.rs`, #261 and #339.
- **Markdown replies folded into one card line.** `morrow-core/src/markdown_preview.rs::{plain, preview_source, segments, code_summary}`, used by `MarkdownReply.svelte`.
- **Follow-up reminders.** `morrow-core/src/follow_up.rs`, `src-tauri/src/reminders.rs::ReminderBook`, `lib/reminders.ts`.
- **Prayer times and offline city data.** `morrow-core/src/prayer.rs`, `prayer/cities.rs`, `src-tauri/src/prayer.rs`, `PrayerCard.svelte`, `PrayerPage.svelte`. #296, #305, #318.
- **Badge colors and theme following settings.** `lib/tokens.ts`, `session-cards.ts`, `AppearancePage.svelte`. #391.
- **Three music tabs, a spectrum visualizer, and sub-notch floating lyrics with four presets.** Our README `:31-73`; absent upstream. Presets: `pureText`, `capsule`, `frostedGlass`, `neonHud`.
- **Interactive Git graph and workspace diff.** Our README `:67-70`; upstream only mentions branch and worktree on a card.
- **Idle indicator privacy fix.** The green dot was replaced with `chevron.down` and a `terminal` badge so it cannot be mistaken for the macOS camera indicator. Our README `:72-73`.

### Library only, not used by the UI

- Push: `morrow-core/src/push.rs`, `push_policy.rs`, `push_transport.rs`, `push_channel.rs`. There is no `push` command in `src-tauri/src/main.rs`. #350, #358, #363.
- Claude quota: `morrow-core/src/quota.rs`, `morrow-platform/src/quota.rs`. The visible quota card is still Swift side (`ClaudeQuotaMonitor.swift`). #297, #306.

None of these are usable on Linux or Windows yet. Keep them separate from the wired features above.

## Removed or no longer displayed

| Item | Evidence |
|---|---|
| Star history section | Upstream `:384-392`. Its chart uses `star-history.dera.page`, which returns 404 for this repo. Removal is in PR #406. |
| Language switcher `English | 简体中文` and the `README.zh-CN.md` link | Upstream `:27`; absent from our README. Our Highlights text still claims 7 UI languages. |
| Downloads badge and App Store Buddy badge | Upstream `:13,15`; absent from our badge row. Buddy itself is still documented. |
| "How it works" navigation link | Upstream `:24`; the section still exists in our README `:351`. |
| Contributing section | Added in this fork, our README `:433`; upstream has none. |

## Platform readiness

Linux and Windows remain **experimental**. Code compiles and Rust tests pass, but host gates have not run: E2E-1, E2E-2, L-001 through L-012, and W-001 through W-009. Build commands and limits are in `docs/build-windows-linux.md` (PR #406). The debt is tracked in #399, #400, #401, and #402.

Do not read the presence of a Rust module as parity with macOS. `morrow-watch`, for example, compiles for Linux and Windows but has never watched a real directory on either OS.

## How to verify this list

```sh
grep -n '^## ' README.md
grep -rn 'PR #' .agents/memory/
cat docs/adr/0001-two-cores-one-contract.md docs/adr/0002-tauri-v2-shell.md docs/adr/0003-swift-core-sidecar.md
./shared/ci/rust-purity.sh
bash linux/ci/gates.sh --self-check
```

Refs #47 · #240 · #407.
