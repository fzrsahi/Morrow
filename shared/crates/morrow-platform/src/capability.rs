//! What each adapter can actually do here (plan §5.2).
//!
//! Every trait gets a `probe()`, and a feature that is not supported **says so
//! in the UI** instead of failing silently. The probe is cheap and touches only
//! what it needs: an environment variable, a path, a `PATH` lookup — never a
//! connection, so Settings opens instantly.

use std::path::Path;

/// The three answers an adapter can give.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Capability {
    Supported,
    /// Works, with a caveat the user should see (GNOME needs an extension, a
    /// compositor is not one we know…).
    Degraded(String),
    Unsupported(String),
}

impl Capability {
    pub fn is_supported(&self) -> bool {
        matches!(self, Self::Supported)
    }

    /// The short label a Settings row shows.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Supported => "Supported",
            Self::Degraded(_) => "Limited",
            Self::Unsupported(_) => "Not supported",
        }
    }

    /// The sentence under the label, if there is one.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Supported => None,
            Self::Degraded(reason) | Self::Unsupported(reason) => Some(reason),
        }
    }
}

/// The adapters `morrow-platform` owns. Ordered as the plan's table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Feature {
    NowPlaying,
    Lyrics,
    SystemVolume,
    TerminalActivator,
    TerminalVisibility,
    Anchoring,
    /// #196: whether the app may place the panel itself (X11/Windows), or the
    /// compositor owns window position (Wayland).
    IslandReposition,
    ProcessTable,
    GlobalHotkey,
    PointerMonitor,
    Fullscreen,
    SessionLock,
    Notifications,
    SyntheticInput,
    SystemStats,
    Geolocation,
    Tray,
    Autostart,
    Secrets,
    ClaudeCredentials,
    Ble,
    Audio,
    Updater,
}

impl Feature {
    pub const ALL: &'static [Feature] = &[
        Feature::NowPlaying,
        Feature::Lyrics,
        Feature::SystemVolume,
        Feature::TerminalActivator,
        Feature::TerminalVisibility,
        Feature::IslandReposition,
        Feature::ProcessTable,
        Feature::GlobalHotkey,
        Feature::PointerMonitor,
        Feature::Fullscreen,
        Feature::SessionLock,
        Feature::Notifications,
        Feature::SyntheticInput,
        Feature::SystemStats,
        Feature::Geolocation,
        Feature::Tray,
        Feature::Autostart,
        Feature::Secrets,
        Feature::ClaudeCredentials,
        Feature::Ble,
        Feature::Audio,
        Feature::Updater,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::NowPlaying => "Now Playing",
            Self::Lyrics => "Lyrics",
            Self::SystemVolume => "System volume",
            Self::TerminalActivator => "Jump to terminal",
            Self::TerminalVisibility => "Terminal visibility",
            Self::Anchoring => "Panel anchoring",
            Self::IslandReposition => "Island repositioning",
            Self::ProcessTable => "Process table",
            Self::GlobalHotkey => "Global hotkey",
            Self::PointerMonitor => "Pointer outside the island",
            Self::Fullscreen => "Fullscreen detection",
            Self::SessionLock => "Screen lock",
            Self::Notifications => "Notifications",
            Self::SyntheticInput => "Synthetic input",
            Self::SystemStats => "System stats",
            Self::Geolocation => "Geolocation",
            Self::Tray => "Tray icon",
            Self::Autostart => "Start at login",
            Self::Secrets => "Secret storage",
            Self::ClaudeCredentials => "Claude credentials",
            Self::Ble => "Bluetooth (ESP32)",
            Self::Audio => "Sound effects",
            Self::Updater => "Updates",
        }
    }

    /// Probe one feature. `environment` is read once per call site, so a test
    /// can hand in a Wayland session or an empty `PATH` without touching the
    /// process.
    pub fn probe(self, environment: &Environment) -> Capability {
        if environment.os == Os::Windows {
            return windows(self, environment);
        }
        match self {
            // The panel is a window: these need a display to do anything.
            Self::NowPlaying => {
                if !environment.has_bus() {
                    Capability::Unsupported("no D-Bus session: MPRIS needs one".into())
                } else if environment.has("playerctl") {
                    Capability::Supported
                } else {
                    // MPRIS is a bus protocol; `playerctl` is the smallest way
                    // in without a D-Bus library of our own.
                    Capability::Degraded("needs playerctl: MPRIS is read through it".into())
                }
            }
            Self::Lyrics => {
                if environment.has_network_tool() {
                    Capability::Supported
                } else {
                    Capability::Degraded("no HTTP client on PATH: lyrics stay empty".into())
                }
            }
            Self::SystemVolume => {
                if environment.has_any(&["wpctl", "pactl"]) {
                    Capability::Supported
                } else {
                    Capability::Unsupported("needs wpctl (PipeWire) or pactl (PulseAudio)".into())
                }
            }
            Self::TerminalActivator => {
                let window_api = environment.has_any(&["swaymsg", "hyprctl", "wmctrl", "xdotool"]);
                if window_api {
                    Capability::Supported
                } else if environment.has_any(&["wl-copy", "xclip"]) {
                    // L-009: the click still lands somewhere useful.
                    Capability::Degraded(
                        "no window API here: a click copies the session's path instead".into(),
                    )
                } else {
                    Capability::Unsupported("no window API and no clipboard tool".into())
                }
            }
            Self::TerminalVisibility => Capability::Unsupported(if environment.is_x11() {
                // #235: no reader exists yet, so X11 cannot claim this either.
                "no terminal-visibility reader yet: _NET_ACTIVE_WINDOW is not queried".into()
            } else {
                "needs _NET_ACTIVE_WINDOW, which Wayland lacks".into()
            }),
            // #109: a layer surface on a compositor that has wlr-layer-shell,
            // an always-on-top window on GNOME — stated, never simulated
            // (plan §6.4).
            Self::Anchoring => match crate::compositor::current() {
                crate::compositor::Anchoring::Available => Capability::Supported,
                crate::compositor::Anchoring::Unavailable(reason) => Capability::Degraded(reason),
            },
            // A window manager honours a placement request; on Wayland the
            // compositor places surfaces, so drag is stated, not simulated.
            Self::IslandReposition => {
                if environment.is_x11() {
                    Capability::Supported
                } else if environment.is_wayland() {
                    Capability::Unsupported(
                        "Wayland lets the compositor place windows; the island stays at the top centre here".into(),
                    )
                } else {
                    Capability::Unsupported(
                        "no display: there is nowhere to move the island".into(),
                    )
                }
            }
            Self::ProcessTable => Capability::Supported,
            Self::GlobalHotkey => {
                if environment.is_x11() {
                    Capability::Supported
                } else if environment.has_portal_shortcuts {
                    Capability::Degraded(
                        "through the XDG desktop portal, as the desktop allows".into(),
                    )
                } else {
                    Capability::Unsupported(
                        "Wayland has no XGrabKey: bind `morrow toggle` in the desktop instead"
                            .into(),
                    )
                }
            }
            Self::PointerMonitor => Capability::Unsupported(if environment.is_x11() {
                // #235: `pointer_position` answers `None` on every Unix session.
                "no pointer reader yet: XQueryPointer is not wired".into()
            } else {
                "Wayland gives no global pointer position".into()
            }),
            Self::Fullscreen => Capability::Unsupported(if environment.is_x11() {
                // #235: `fullscreen` answers `false` on every Unix session.
                "no fullscreen reader yet: _NET_WM_STATE is not queried".into()
            } else {
                "needs _NET_WM_STATE_FULLSCREEN on the active window".into()
            }),
            Self::SessionLock => {
                if environment.has_bus() {
                    Capability::Supported
                } else {
                    Capability::Unsupported("needs logind on the session bus".into())
                }
            }
            Self::Notifications => {
                if environment.has("notify-send") {
                    Capability::Supported
                } else {
                    Capability::Unsupported("needs notify-send for reminders".into())
                }
            }
            Self::SyntheticInput => Capability::Unsupported(
                "no synthetic-input adapter yet: XTEST and the portal are not wired".into(),
            ),
            Self::SystemStats => Capability::Unsupported(NO_SYSTEM_STATS.into()),
            Self::Geolocation => Capability::Unsupported(
                "no geolocation adapter yet: GeoClue and the offline city database are not wired"
                    .into(),
            ),
            Self::Tray => {
                if environment.has_file("libayatana-appindicator3.so.1")
                    || environment.has_file("libappindicator3.so.1")
                {
                    Capability::Supported
                } else {
                    // Not a crash: the panel still works without a tray.
                    Capability::Unsupported(
                        "no AppIndicator library: the island has no tray icon".into(),
                    )
                }
            }
            Self::Autostart => {
                if environment.config_home.is_dir() {
                    Capability::Supported
                } else {
                    Capability::Unsupported("no ~/.config to write an autostart entry into".into())
                }
            }
            Self::Secrets => Capability::Unsupported(
                "no secret-store adapter yet: the session bus is not used for secrets".into(),
            ),
            // #306: "Supported" now means the real reader can parse the file.
            Self::ClaudeCredentials => {
                claude_credentials_capability(&environment.claude_credentials)
            }
            Self::Ble => {
                Capability::Unsupported("no Bluetooth adapter yet: BlueZ is not used".into())
            }
            // #241: the row follows the same tool list the feed plays through.
            Self::Audio => match crate::sound::player(|tool| environment.has(tool)) {
                Some(_) => Capability::Supported,
                None => Capability::Unsupported(
                    "needs pw-play (PipeWire), paplay (PulseAudio) or canberra-gtk-play".into(),
                ),
            },
            Self::Updater => Capability::Unsupported(NO_UPDATER.into()),
        }
    }
}

/// Where a row's answer actually comes from (#235).
///
/// A row may not say more than the code does: `Supported` and `Degraded` both
/// mean "there is an adapter", so both need an entry here. The registry is an
/// explicit list rather than a search, and `module` is a path from this crate's
/// root so `tests/capability_and_settings.rs` can prove the file exists — a
/// registry that names a module nobody wrote is the same lie in a new place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Adapter {
    pub os: Os,
    pub feature: Feature,
    pub module: &'static str,
}

/// Every adapter that exists today. Adding a row here is a claim that the file
/// behind it answers that feature on that OS; removing one is how a feature
/// goes back to `Unsupported` without the table lying.
pub const ADAPTERS: &[Adapter] = &[
    // Linux: the Unix modules shell out to a tool, so the tool's presence is
    // the probe and the module is the caller.
    Adapter {
        os: Os::Linux,
        feature: Feature::NowPlaying,
        module: "src/music.rs",
    },
    Adapter {
        os: Os::Linux,
        feature: Feature::Lyrics,
        module: "src/music.rs",
    },
    Adapter {
        os: Os::Linux,
        feature: Feature::SystemVolume,
        module: "src/music.rs",
    },
    Adapter {
        os: Os::Linux,
        feature: Feature::TerminalActivator,
        module: "src/terminal.rs",
    },
    Adapter {
        os: Os::Linux,
        feature: Feature::Anchoring,
        module: "src/compositor.rs",
    },
    Adapter {
        os: Os::Linux,
        feature: Feature::IslandReposition,
        module: "../../desktop/src-tauri/src/position.rs",
    },
    Adapter {
        os: Os::Linux,
        feature: Feature::ProcessTable,
        module: "src/process.rs",
    },
    Adapter {
        os: Os::Linux,
        feature: Feature::GlobalHotkey,
        module: "../../desktop/src-tauri/src/hotkey.rs",
    },
    Adapter {
        os: Os::Linux,
        feature: Feature::SessionLock,
        module: "src/lock.rs",
    },
    Adapter {
        os: Os::Linux,
        feature: Feature::Notifications,
        module: "src/notify.rs",
    },
    Adapter {
        os: Os::Linux,
        feature: Feature::Tray,
        module: "../../desktop/src-tauri/src/tray.rs",
    },
    Adapter {
        os: Os::Linux,
        feature: Feature::Autostart,
        module: "src/autostart.rs",
    },
    Adapter {
        os: Os::Linux,
        feature: Feature::Audio,
        module: "src/sound.rs",
    },
    // Windows: each of these is a WinRT/Win32 call, so the module is the probe.
    Adapter {
        os: Os::Windows,
        feature: Feature::NowPlaying,
        module: "src/windows/media.rs",
    },
    Adapter {
        os: Os::Windows,
        feature: Feature::Lyrics,
        module: "src/music.rs",
    },
    Adapter {
        os: Os::Windows,
        feature: Feature::SystemVolume,
        module: "src/windows/volume.rs",
    },
    Adapter {
        os: Os::Windows,
        feature: Feature::TerminalActivator,
        module: "src/windows/terminal.rs",
    },
    Adapter {
        os: Os::Windows,
        feature: Feature::Anchoring,
        module: "../../desktop/src-tauri/src/windows_panel.rs",
    },
    Adapter {
        os: Os::Windows,
        feature: Feature::IslandReposition,
        module: "../../desktop/src-tauri/src/position.rs",
    },
    Adapter {
        os: Os::Windows,
        feature: Feature::ProcessTable,
        module: "src/windows/process.rs",
    },
    Adapter {
        os: Os::Windows,
        feature: Feature::GlobalHotkey,
        module: "../../desktop/src-tauri/src/hotkey.rs",
    },
    Adapter {
        os: Os::Windows,
        feature: Feature::PointerMonitor,
        module: "src/windows/pointer.rs",
    },
    Adapter {
        os: Os::Windows,
        feature: Feature::Fullscreen,
        module: "src/windows/fullscreen.rs",
    },
    Adapter {
        os: Os::Windows,
        feature: Feature::SessionLock,
        module: "src/windows/lock.rs",
    },
    Adapter {
        os: Os::Windows,
        feature: Feature::Notifications,
        module: "src/windows/toast.rs",
    },
    Adapter {
        os: Os::Windows,
        feature: Feature::Tray,
        module: "../../desktop/src-tauri/src/tray.rs",
    },
    Adapter {
        os: Os::Windows,
        feature: Feature::Autostart,
        module: "src/windows/autostart.rs",
    },
    Adapter {
        os: Os::Windows,
        feature: Feature::Audio,
        module: "src/windows/sound.rs",
    },
    // #306: the credentials reader exists on both OSes — the same module parses
    // the same file, so the row may say more than "Unsupported" now.
    Adapter {
        os: Os::Linux,
        feature: Feature::ClaudeCredentials,
        module: "src/quota.rs",
    },
    Adapter {
        os: Os::Windows,
        feature: Feature::ClaudeCredentials,
        module: "src/quota.rs",
    },
];

/// The adapter behind a row, if one is registered for that OS.
pub fn adapter(os: Os, feature: Feature) -> Option<&'static Adapter> {
    ADAPTERS
        .iter()
        .find(|entry| entry.os == os && entry.feature == feature)
}

/// The reason a row gives while no adapter exists (#235). Shared by the two
/// arms where the absence is the same on both OSes, so Linux and Windows cannot
/// drift into describing one missing adapter two different ways.
const NO_SYSTEM_STATS: &str = "no system-stats adapter yet: CPU, memory and battery are not read";
const NO_UPDATER: &str = "no updater yet: no update feed is configured";

/// One row of the Settings table.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Row {
    pub feature: &'static str,
    pub capability: Capability,
}

/// The Windows answers (#107). Every row here is a mechanism Windows has and
/// the Linux arms do not: WinRT for media and toasts, Core Audio for the volume,
/// WTS for the lock, `SHQueryUserNotificationState` for fullscreen, the registry
/// for autostart, the Credential Manager for secrets.
/// #306: "Supported" means the reader in `crate::quota` can actually parse
/// the file, not just that a file with that name exists. Shared by the macOS
/// and Windows probes so the two rows can never drift apart.
fn claude_credentials_capability(path: &Path) -> Capability {
    match crate::quota::read_credentials_at(path) {
        Ok(Some(_)) => Capability::Supported,
        Ok(None) => Capability::Degraded("no usable ~/.claude/.credentials.json to read".into()),
        Err(_) => Capability::Degraded("Claude credentials file could not be read".into()),
    }
}

fn windows(feature: Feature, environment: &Environment) -> Capability {
    match feature {
        // SMTC through WinRT: no tool to find, no bus to have.
        Feature::NowPlaying => Capability::Supported,
        Feature::Lyrics => {
            if environment.has_network_tool() {
                Capability::Supported
            } else {
                Capability::Unsupported("no curl or wget to fetch lyrics".into())
            }
        }
        // Core Audio's endpoint volume: one COM interface, nothing to install.
        Feature::SystemVolume => Capability::Supported,
        // A window whose process owns the session, then `SetForegroundWindow`.
        Feature::TerminalActivator => Capability::Supported,
        // There is no `_NET_ACTIVE_WINDOW` and no per-terminal title protocol:
        // "is my terminal visible" is not a question Windows answers.
        Feature::TerminalVisibility => Capability::Unsupported(
            "Windows has no cross-application way to say whether a window is visible".into(),
        ),
        // A Win32 window can be topsmost and non-activating; there is no
        // separate anchoring protocol to lack.
        Feature::Anchoring => Capability::Supported,
        // SetWindowPos: Windows lets the app place its own window.
        Feature::IslandReposition => Capability::Supported,
        Feature::ProcessTable => Capability::Supported,
        Feature::GlobalHotkey => Capability::Supported,
        // `GetCursorPos` is global on Windows, unlike on Wayland.
        Feature::PointerMonitor => Capability::Supported,
        Feature::Fullscreen => Capability::Supported,
        Feature::SessionLock => Capability::Supported,
        Feature::Notifications => Capability::Supported,
        Feature::SyntheticInput => {
            Capability::Unsupported("no synthetic-input adapter yet: SendInput is not wired".into())
        }
        Feature::SystemStats => Capability::Unsupported(NO_SYSTEM_STATS.into()),
        Feature::Geolocation => Capability::Unsupported(
            "no geolocation adapter yet: WinRT Geolocation is not called".into(),
        ),
        Feature::Tray => Capability::Supported,
        // `HKCU\\…\\CurrentVersion\\Run`, one value, no administrator.
        Feature::Autostart => Capability::Supported,
        Feature::Secrets => Capability::Unsupported(
            "no secret-store adapter yet: the Credential Manager is not used".into(),
        ),
        // #306: the reader exists, so the probe reads the file.
        Feature::ClaudeCredentials => {
            claude_credentials_capability(&environment.claude_credentials)
        }
        Feature::Ble => Capability::Unsupported(
            "no Bluetooth adapter yet: WinRT Bluetooth LE is not called".into(),
        ),
        // `PlaySoundW` is part of the OS: no tool to find, no bus to have.
        Feature::Audio => Capability::Supported,
        Feature::Updater => Capability::Unsupported(NO_UPDATER.into()),
    }
}

/// Every feature, probed. This is what Settings renders.
pub fn table(environment: &Environment) -> Vec<Row> {
    Feature::ALL
        .iter()
        .map(|feature| Row {
            feature: feature.name(),
            capability: feature.probe(environment),
        })
        .collect()
}

/// Which platform's answers apply. A probe reads this instead of `cfg!` so a
/// test can hand in Windows on a Linux machine and check the table it produces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Linux,
    Windows,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

/// What the probes are allowed to know: read once, from the process, or handed
/// in by a test. Only what a probe actually reads is here — a field nothing
/// reads is a promise the table cannot keep (#235).
#[derive(Clone, Debug)]
pub struct Environment {
    pub os: Os,
    pub session_type: Option<String>,
    pub has_session_bus: bool,
    pub path_tools: Vec<String>,
    pub config_home: std::path::PathBuf,
    /// #306: the Claude credentials probe parses this file, so the field earns
    /// its place here — unlike the fields #235 removed as unread.
    pub claude_credentials: std::path::PathBuf,
    pub has_portal_shortcuts: bool,
}

impl Environment {
    /// The real machine.
    pub fn probe() -> Self {
        let home = std::env::var_os("HOME").map_or_else(|| "/".into(), std::path::PathBuf::from);
        let config_home = std::env::var_os("XDG_CONFIG_HOME")
            .map_or_else(|| home.join(".config"), std::path::PathBuf::from);
        let claude_dir = std::env::var_os("CLAUDE_CONFIG_DIR")
            .map_or_else(|| home.join(".claude"), std::path::PathBuf::from);
        let session_type = std::env::var("XDG_SESSION_TYPE").ok().or_else(|| {
            std::env::var_os("WAYLAND_DISPLAY")
                .is_some()
                .then(|| "wayland".to_owned())
        });
        Self {
            os: Os::current(),
            session_type,
            has_session_bus: std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some(),
            path_tools: std::env::var("PATH")
                .map(|path| path.split(':').map(str::to_owned).collect())
                .unwrap_or_default(),
            config_home,
            claude_credentials: claude_dir.join(".credentials.json"),
            has_portal_shortcuts: Path::new("/usr/share/xdg-desktop-portal/portals").is_dir(),
        }
    }

    fn is_x11(&self) -> bool {
        self.session_type
            .as_deref()
            .is_some_and(|kind| kind.eq_ignore_ascii_case("x11"))
            || (self.session_type.is_none() && std::env::var_os("DISPLAY").is_some())
    }

    fn is_wayland(&self) -> bool {
        self.session_type
            .as_deref()
            .is_some_and(|kind| kind.eq_ignore_ascii_case("wayland"))
    }

    fn has_bus(&self) -> bool {
        self.has_session_bus
    }

    pub fn has(&self, tool: &str) -> bool {
        // Only PATH entries: a stray file in the working directory is not a
        // tool the shell can spawn, so it may not make a row say "Supported".
        // Being a file is not enough — it must be executable too, or a row
        // would promise a tool the feed cannot run. This is the same lookup
        // the sound player uses (`runner::is_executable_tool`).
        self.path_tools
            .iter()
            .any(|entry| crate::runner::is_executable_tool(&Path::new(entry).join(tool)))
    }

    /// A file that only has to exist, not be spawnable. The Tray row looks for
    /// shared libraries (`*.so.1`), which are not executables, so it keeps its
    /// file-only meaning on every OS.
    fn has_file(&self, name: &str) -> bool {
        self.path_tools
            .iter()
            .any(|entry| Path::new(entry).join(name).is_file())
    }

    fn has_any(&self, tools: &[&str]) -> bool {
        tools.iter().any(|tool| self.has(tool))
    }

    fn has_network_tool(&self) -> bool {
        self.has_any(&["curl", "wget"])
    }
}
