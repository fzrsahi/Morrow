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

The gap is tracked in [issue #400](https://github.com/fzrsahi/Morrow/issues/400). Missing adapters and modules are in [issue #399](https://github.com/fzrsahi/Morrow/issues/399). The purity guard is currently red in [issue #401](https://github.com/fzrsahi/Morrow/issues/401).

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
./linux/ci/e2e2.sh --self-check
```

`e2e2.sh --self-check` needs an existing app binary: run it after the `cargo build -p morrow-app` above, or point it at one with `MORROW_APP=/path/to/morrow-app`. That build is Tauri's job and requires the frontend `dist` from `bun run build`, so both build steps must come first.

`gates.sh --self-check` only checks the runner's own logic. On a desktop, `bash linux/ci/gates.sh --markdown` produces the real verdict table; the headless run exits 77 because every desktop half is still blocked.

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
- `windows/ci/e2e2.ps1` still matches the Linux hook name `codeisland-hook.sh`, while the Windows installer writes `morrow-hook.cmd`. The script will fail a correct install until that is fixed.
- None of W-001 through W-009 has run on a Windows host. These cover named pipes with two accounts, click-through, SMTC, Core Audio, session changes, the installer, and the release build.

## What to read next

- `.agents/memory/` for the packaging and signing decisions.
- `docs/windows-adapters.md` for the capability table per OS.
- `shared/desktop/README.md` for the panel and its hook wire format.
- The README section "Measure peak RAM usage" for the per-host RSS scripts and their prerequisites.
- `AGENTS.md` section 2 before touching `.woodpecker/` or any runner script. Those files belong to @aazhrt.
