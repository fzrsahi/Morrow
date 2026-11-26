# Build and run on Linux or Windows

The macOS app is the finished product. The Tauri shell for Linux and Windows is a port in progress: it builds and its Rust tests pass, but the desktop gates have not been run on a real machine. Read the limits below before treating a successful build as a working app.

## What works today

| | Linux | Windows |
|---|---|---|
| Rust crates (`cargo test`) | yes | yes, portable crates only |
| Cross-compile the `#[cfg(windows)]` code | not needed | yes, with `cargo-xwin` |
| Tauri shell build | yes, X11 first | yes, NSIS packaging |
| Hook install from the app | yes | yes, for Claude Code |
| Real desktop gates | not run | not run |

Issues [#400](https://github.com/fzrsahi/Morrow/issues/400), [#399](https://github.com/fzrsahi/Morrow/issues/399) and [#401](https://github.com/fzrsahi/Morrow/issues/401) are closed. The gates are tooling-complete but still have no real Linux X11 or Windows host evidence; #399 closed with adapter scope decisions, and the purity guard is green.

## Prerequisites

- Rust 1.98.0. The version is pinned in `rust-toolchain.toml`.
- Bun for the Svelte frontend. Run `bun install` once in `shared/desktop`.
- Linux: GTK3 and WebKitGTK 4.1 development packages. `linux/ci/Dockerfile` lists the Debian names: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`.
- Windows: WebView2 runtime, the MSVC target, and `tauri-cli` (`cargo install tauri-cli --version '^2'`).
- Cross-checking Windows from a macOS or Linux host: `cargo-xwin` (`cargo install cargo-xwin --locked`). `windows/ci/check.sh` fails without it.

## Linux

From the repository root:

```sh
(cd shared/desktop && bun install && bun run build)
cargo build -p morrow-app
```

`bun run build` writes `shared/desktop/dist`, which Tauri embeds. There is no dev server. The debug binary is `target/debug/morrow-app`.

Checks that run without a desktop:

```sh
./scripts/ci/verify-local.sh
./linux/ci/test.sh
bash linux/ci/gates.sh --self-check
./linux/ci/e2e1.sh --self-check
./linux/ci/e2e2.sh --self-check
```

`e2e1.sh --self-check` needs no display, frontend, binary or bridge and runs on bash 3.2. It checks the gate's own logic against fixed fixtures: the synthetic PermissionRequest bytes, the button parser, the smoke-log readers, the 300 ms boundary, and that a failed check cannot pass silently. It prints a `SELF-CHECK` Evidence block; that block is not a gate verdict and must not be pasted as one.

`e2e2.sh --self-check` needs an existing app binary: run it after the `cargo build -p morrow-app` above, or point it at one with `MORROW_APP=/path/to/morrow-app`. That build is Tauri's job and requires the frontend `dist` from `bun run build`, so both build steps must come first.

`gates.sh --self-check` only checks the runner's own logic. On a desktop, `bash linux/ci/gates.sh --markdown` produces the real verdict table; the headless run exits 77 because every desktop half is still blocked.

### E2E-1 on a real X11 host

Gate E2E-1 (#91) still needs the final judgement on the Debian host (#70). From the repo root, in the desktop X11 session:

```sh
(cd shared/desktop && bun install && bun run build) && linux/ci/e2e1.sh
```

It needs `xdotool`, `x11-utils`, `wmctrl`, and GTK3 + WebKitGTK 4.1. For about 10 s it moves the pointer over a full-screen `xev` window; keep hands off. Paste the whole `=== Evidence: linux/ci/e2e1.sh (gate E2E-1) ===` block, from `date:` through `result:`, into #91. Close the issue only if that block says `PASS (27 checks)`. A self-check template or a CI/Xvfb run is not that judgement. If click-through or focus fails, stop and report in #33 before Phase 2.

For a packaged build, `linux/ci/package.sh` calls `cargo tauri build --bundles deb,rpm,appimage`.

### Known limits

- X11 is the supported path. Wayland compositors report an anchoring capability from environment variables, but there is no layer surface, input region, or XDG Portal implementation behind that row.
- `linux/ci/test.sh` excludes `morrow-app`, so this lane does not compile the panel. Use `linux/ci/test-desktop.sh` in the image from `linux/ci/Dockerfile`.
- The full E2E-2 run needs a real `claude` on a logged-in host. The installer half has been exercised; the CLI half has not.

## Windows

From a macOS or Linux host you can compile the Windows-only code, but you cannot run it:

```sh
rustup target add x86_64-pc-windows-msvc
./windows/ci/check.sh
```

The script uses `cargo-xwin` to type-check the crates. It does not start a Windows app.

On a Windows machine:

```sh
cd shared/desktop
bun install
bun run build
cargo tauri build --bundles nsis
```

The NSIS bundle is configured for a per-user install. Beta builds are unsigned, so Windows SmartScreen shows a warning until you choose More info and Run anyway. The per-user NSIS decision and deferred beta signing are recorded in `.agents/memory/`.

### Known limits

- The installer currently hooks Claude Code only. Other tools have no Windows config paths yet.
- None of W-001 through W-009 has run on a Windows host. These cover named pipes with two accounts, click-through, SMTC, Core Audio, session changes, the installer, and the release build.

## What to read next

- `.agents/memory/` for the packaging and signing decisions.
- `docs/windows-adapters.md` for the capability table per OS.
- `shared/desktop/README.md` for the panel and its hook wire format.
- The README section "Measure peak RAM usage" for the per-host RSS scripts and their prerequisites.
- `AGENTS.md` section 2 before touching `.woodpecker/` or any runner script. Those files belong to @aazhrt.
