# Port status map (#399)

This is the status source of truth for the Swift-to-Rust gap in #399. It replaces the plan and progress documents deleted in PR #406. Background lives in the [memory index](../.agents/memory/memory.md), [fork differences](fork-differences.md) and the [build guide](build-windows-linux.md). Paths in the table are from the repository root.

A row is `ported` only under the #240 definition: the module exists, is callable from the shell, and has a test. `library-only` means Rust code with no UI consumer; an IPC command may exist without a UI that uses it. `host gate pending` means no real Linux or Windows run has happened.

| Area | Status | Evidence | Ticket/decision |
|---|---|---|---|
| Capability honesty and `ADAPTERS` guard | ported | `shared/crates/morrow-platform/src/capability.rs:309`; `shared/crates/morrow-platform/tests/capability_and_settings.rs:95-110` | #235, PR #249 |
| Discovery: 13 readers, SQLite, PID correlation, backfill | ported | `shared/crates/morrow-core/src/discovery.rs`; `shared/desktop/src-tauri/src/discovery.rs::scan_stores`; `shared/desktop/src-tauri/src/feed.rs::backfill_with`; [fork differences](fork-differences.md) `:37-38` | #232, #244-#248, #261, #339 |
| Secrets | decision pending | `shared/crates/morrow-platform/src/capability.rs:270`; #235 demoted the row and no scope decision had been recorded when this map was written. Issue #441 is open and recommends retiring this row in favour of the already-ported `ClaudeCredentials` (`capability.rs:466-475`), the only secret path the Swift app reads | #235 (honesty), #240 (audit), #441 (decision) |
| SystemStats | library-only | PR #438 added `morrow_platform::system_stats` with Linux and Windows readers, real capability probes and both `ADAPTERS` rows (`src/system_stats.rs`); no shell command or UI asks for system stats | #235, #240, #437, PR #438 |
| Geolocation | decision pending | `capability.rs:247-250` still hardcodes `Unsupported` and there is no adapter. Issue #443 is open and recommends out-of-scope for the no-dependency port, with the ported offline city catalog and manual-city path as the supported prayer-location flow. Not ported | #235, #240, #443 (decision) |
| SyntheticInput | library-only | PR #440, for issue #439, added `morrow_platform::synthetic_input` (xdotool/XTEST on Linux, PowerShell SendKeys on Windows) with both `ADAPTERS` rows (`src/synthetic_input.rs`); no shell command or UI calls it | #235, #240, #439, PR #440 |
| TerminalVisibility | library-only | PR #436 added the Linux X11 reader (`src/x11.rs`) and its `ADAPTERS` row; the capability probe is no longer hardcoded `Unsupported` on X11, it now follows `xdotool` and `xprop`. No shell or UI call site | #235, #240, PR #436 |
| PointerMonitor | library-only | PR #436 added the Linux X11 reader (`src/x11.rs`) and its `ADAPTERS` row; the capability probe is no longer hardcoded `Unsupported` on X11, it now follows `xdotool`. No shell or UI call site | #235, #240, PR #436 |
| Fullscreen | library-only | PR #436 added the Linux X11 reader (`src/x11.rs`) and its `ADAPTERS` row; the capability probe is no longer hardcoded `Unsupported` on X11, it now follows `_NET_WM_STATE`. The existing `fullscreen()` command calls the reader on Unix, but no UI exposes the state. A Windows adapter exists at `capability.rs:426` | #235, #240, PR #436 |
| Ble | decision pending | `capability.rs:277-279`; #240 §11 excludes Apple Companion BLE only | #240 |
| Updater / Sparkle | out-of-scope | `capability.rs:287,478`; `shared/crates/morrow-core/README.md:318`; macOS keeps Sparkle, the port uses `tauri-plugin-updater` | #337, #240 |
| RemoteInstaller + SSHForwarder + reconnect policy | library-only | PR #429 adds `morrow_core::remote` and `morrow_platform::remote`; no Settings card or IPC caller | #233, PR #429 |
| NotchHoopsEngine | decision pending | Swift only: `macos/Sources/MorrowCore/NotchHoopsEngine.swift` (~415 LOC, #238); no Rust module and no triage decision | #433 |
| Power banner rules | library-only | PR #418 adds `morrow-core/src/power_banner.rs` with conformance fixtures; no shell caller | #399, PR #418 |
| Full `AttentionPolicy::settle` | library-only | PR #431 adds the decision and `tests/settle_conformance.rs`; the shell still calls `settle_completion` at `shared/desktop/src-tauri/src/completion.rs:153` | #430, PR #431 |
| `morrow-watch` real directory run | host gate pending | `shared/crates/morrow-watch/` compiles; [fork differences](fork-differences.md) `:68` records no host run | #399, #400 |
| E2E-2 and L/W host gates | host gate pending | [build guide](build-windows-linux.md) `:13,47,83`; `linux/ci/gates.sh`, `windows/ci/e2e2.ps1` | #400 |
| Wayland layer-shell | host gate pending | `shared/crates/morrow-platform/src/compositor.rs:4-19` reports anchoring; [build guide](build-windows-linux.md) `:53` says no layer surface, input region or portal is behind it | #400 |
| `pointer_position` | decision pending | Command removed in #374; only a stale guard test and comment remain at `shared/desktop/src-tauri/src/commands.rs:208` and `capability.rs:218`, removed by PR #428 | #399, PR #428 |
| Apple-only surfaces (companion, haptics, Apple Mail, audio capture) and line-by-line Swift port | out-of-scope | `shared/crates/morrow-core/README.md:348`; #240 §11 lists the exclusions | #240 |
| One Rust core with FFI into macOS | out-of-scope | `docs/adr/0001-two-cores-one-contract.md:11` rejects FFI; the IPC seam already exists | #240, ADR-0001 |
| Flatpak packaging | decision pending | Named in #399; no scope decision in current docs or memory | none yet |

`decision pending` means there is no recorded scope decision, not that the work is forgotten. #235 changed the table to `Unsupported(reason)` and #240 audited it; neither declares those rows out of scope. Promote a row to `out-of-scope` only with a decision issue number. #433 is the decision issue for NotchHoopsEngine and recommends out-of-scope (game UI) subject to maintainer confirmation. #441 and #443 are the decision issues for Secrets and Geolocation; both recommend an out-of-scope outcome but neither decision is final.
