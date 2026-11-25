# The Windows adapters (#107)

What each adapter uses on Windows, why that and not something else, and what
only a Windows host can confirm. The Linux answers are in
[`.agents/memory/`](../.agents/memory/memory.md); the Capability
table that reports these rows is `morrow-platform::capability`, and it is
probed per OS (`Environment::os`).

A row may only say `Supported` or `Limited` when `capability::ADAPTERS` names
the module behind it, and `tests/capability_and_settings.rs` fails when one does
not (#235). That is what keeps this document honest: the table below describes
adapters that exist, and the ones that do not are listed at the end.

| Ability | Linux ran | Windows uses | Verified by |
|---|---|---|---|
| Now Playing | `playerctl` (MPRIS over D-Bus) | `GlobalSystemMediaTransportControlsSessionManager` (WinRT SMTC) | W-006 |
| Volume | `wpctl` / `pactl` | `IAudioEndpointVolume` (Core Audio) | W-006 |
| Screen lock | `loginctl … LockedHint` | `WTSSessionInfoEx` → `SessionFlags` | W-007 |
| Fullscreen | `_NET_WM_STATE` via `xprop`, X11 only (see below) | `SHQueryUserNotificationState` | W-007 |
| Reminders | `notify-send` | `ToastNotificationManager` | W-009 (installed app) |
| Start at login | `~/.config/autostart/*.desktop` | `HKCU\…\CurrentVersion\Run` via `reg.exe` | W-009 |
| Jump to terminal | `swaymsg`/`hyprctl`/`wmctrl` by app id | the session's process chain, then `SetForegroundWindow` | W-005 |
| Pointer | `xdotool getmouselocation`, X11 only (see below) | `GetCursorPos`, global here | — |
| Sound effects | `pw-play` / `paplay` / `canberra-gtk-play` | `PlaySoundW` with `SND_ASYNC` | — |
| Secrets | — no adapter yet (see below) | — no adapter yet (see below) | W-008 |

## What is not wired yet (#235)

These rows answer `Not supported` with a reason on both OSes, because no module
behind them exists. They are the list the Capability table used to overstate:

`Geolocation` · `Secret storage` · `Claude credentials` ·
`Bluetooth (ESP32)` · `Updates`

On Linux, `Fullscreen detection`, `Pointer outside the island` and
`Terminal visibility` now have readers in `src/x11.rs`: `_NET_WM_STATE` through
`xprop`, pointer coordinates through `xdotool getmouselocation`, and the active
terminal class through `xdotool getactivewindow`. Their probes say `Supported`,
`Degraded` or `Unsupported` instead of a boolean guess, and the Windows
adapters for the first two remain real (`windows/fullscreen.rs`,
`windows/pointer.rs`). A real X11 host run is still pending.

Adding a row back is two steps: write the adapter, then add it to
`capability::ADAPTERS` — the guard test fails until both are true.

## The three worth arguing about

**Toasts need an installed app.** `CreateToastNotifierWithId` takes an
application id the shell knows; from `cargo run` there is none and Windows drops
the toast silently. That is why the ticket says "test notifications with the app
installed", and why `toast::APP_ID` is the bundle identifier the NSIS installer
registers.

**`SHQueryUserNotificationState` answers two questions.** It reports
`QUNS_RUNNING_D3D_FULL_SCREEN`, `QUNS_PRESENTATION_MODE`, `QUNS_BUSY`,
`QUNS_QUIET_TIME` and `QUNS_ACCEPTS_NOTIFICATIONS`, which covers both "is
something fullscreen in front" and "may I interrupt". One call, two rows in the
table. An unknown value counts as *may interrupt*: a permission prompt that never
appears is worse than one shown a second early.

**A registry value is a command line.** `Run` holds the string Windows will
parse with `CommandLineToArgvW`, so a path with a space is quoted once and a
trailing backslash is doubled — otherwise `"C:\Program Files\Morrow\"` closes its
own quote and the argument runs into the next. `windows_rules::run_value` builds
it and `tests/windows_rules.rs` pins the four cases.

## What a compile lane cannot prove

W-005, W-006, W-007 and W-009 all need a Windows host: SMTC with a real player,
Core Audio on a device with an endpoint, WTS with a session that can actually lock,
toasts from an installed build, and a terminal whose window comes forward. The
lane proves the code compiles against the real headers and types; it cannot prove
the OS agrees.

## The installer and the hook (#108)

NSIS, per-user (`installMode: currentUser`): no administrator prompt, and the app
writes only inside `%LOCALAPPDATA%`, `%USERPROFILE%` and `HKCU`. That is also why
start-at-login is a `Run` value rather than a scheduled task or a service.

**Beta is unsigned.** There is no Authenticode certificate, so SmartScreen shows
"Windows protected your PC" until the user picks *More info → Run anyway*. This is
a decision recorded for #70, and it is documented on the download page rather than
worked around: the updater's own Ed25519 signature is separate and cannot be
switched off.

The hook installer (`morrow-platform::hooks_windows`) writes:

| path | what |
|---|---|
| `%USERPROFILE%\.codeisland\morrow-hook.cmd` | the script every hook command names |
| `%USERPROFILE%\.local\bin\morrow-bridge.exe` | the bridge, copied out of the package |
| `%CLAUDE_CONFIG_DIR%\settings.json` (or `%USERPROFILE%\.claude\settings.json`) | the user's file, backed up once, never rewritten when it does not parse |

The command recorded in `settings.json` is the **absolute, quoted** path of the
`.cmd`: Claude Code hands the string to `cmd.exe`, which splits on spaces, and a
profile folder can contain one. `%USERPROFILE%` would work in most contexts and
fail in the one nobody tested.

Only Claude Code is hooked by this build. The other eight CLIs' files are read by
the same core policy, but their Windows locations are a follow-up: a path of
`~/.codex/config.toml` on Windows names a file that does not exist, and Settings
says so in a sentence rather than showing eight rows that do nothing.
