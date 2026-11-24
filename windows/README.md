# windows/

Windows packaging and the Windows-only CI lane. The Windows *app* is not here: it
is the shared Tauri v2 shell in [`shared/desktop/`](../shared/desktop), with the
Windows backends selected by `#[cfg]` in `morrow-platform`
(see [`docs/memory/`](../docs/memory/README.md))..

| Path | What it is |
|---|---|
| `ci/check.sh` | The Windows lane: type-checks five crates for `x86_64-pc-windows-msvc` with `cargo-xwin`. Runs on every PR; the W-001…W-009 gates themselves need a Windows host. |
| `../shared/desktop/src-tauri/tauri.conf.json` | The NSIS bundle: per-user install mode, `icons/icon.ico`, and the `externalBin` the Linux packager passes. |

## The .NET app that used to live here

Removed in #110 (phase 6 of the port). `windows/` held a complete WPF
implementation — `Morrow.Core`, `Morrow.Ipc`, `Morrow.Bridge`, `Morrow.Hooks`,
`Morrow.Bluetooth`, `Morrow.Protocol`, `Morrow.Windows` and their tests — written
against the same hook protocol before the Rust core existed. It had been dead
code for a while before that: no CI job built it, `global.json` pinned an SDK
nobody installed, and the contract it implemented was not the frozen one in
[`shared/contract/`](../shared/contract).

The decision and its context are recorded in [`docs/memory/`](../docs/memory/README.md) and [`docs/adr/0002-tauri-v2-shell.md`](../docs/adr/0002-tauri-v2-shell.md). `git log -- windows/` still has every line of the code itself.
