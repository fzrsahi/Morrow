//! The Settings page's other half (#101): the Capability table, start at login,
//! and the settings file itself. Batch 1 of the adapters — tray, hotkey and the
//! rest of the runtime wiring come with the panel work they depend on.

use std::path::PathBuf;

use morrow_platform::capability::{self, Feature};
use morrow_platform::{autostart, settings};
use serde_json::Value;
use tauri::{AppHandle, Emitter, State};

use crate::HooksSite;

/// #202 — the island listens for this while Settings writes from its own window.
pub const SETTINGS_CHANGED_EVENT: &str = "settings-changed";

/// Every adapter and what it can do on this machine.
///
/// The probes read the environment; for the two adapters that register at
/// startup, what actually happened wins over what was expected — a tray whose
/// status notifier did not come up says so, rather than repeating the guess.
#[tauri::command]
pub fn capabilities() -> Vec<capability::Row> {
    let mut rows = capability::table(&capability::Environment::probe());
    for (feature, outcome) in [
        (Feature::Tray, crate::tray::outcome()),
        (Feature::GlobalHotkey, crate::hotkey::outcome()),
    ] {
        if let (Some(outcome), Some(row)) = (
            outcome,
            rows.iter_mut().find(|row| row.feature == feature.name()),
        ) {
            row.capability = outcome;
        }
    }
    // #235: the Fullscreen row used to be corrected here because the generic
    // table promised "Supported" on every Linux session. The probe says so
    // itself now, so the shell no longer keeps a second copy of that answer.
    rows
}

/// Whether the autostart entry is there: a `.desktop` file on Linux, a value
/// under `HKCU\…\Run` on Windows (#107).
#[tauri::command]
pub fn autostart_status(site: State<'_, HooksSite>) -> autostart::Status {
    #[cfg(windows)]
    {
        let _ = &site;
        let entry =
            morrow_platform::windows::autostart::status(&morrow_platform::runner::SystemRunner)
                .ok()
                .flatten()
                .unwrap_or_default();
        autostart::Status {
            installed: !entry.is_empty(),
            path: PathBuf::from(morrow_platform::windows::autostart::KEY),
        }
    }
    #[cfg(unix)]
    {
        autostart::status(&site.paths.home.join(".config"))
    }
}

/// Install or remove the autostart entry. Returns whether anything changed.
#[tauri::command]
pub fn autostart_set(site: State<'_, HooksSite>, enabled: bool) -> Result<bool, String> {
    #[cfg(windows)]
    {
        let _ = &site;
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let before =
            morrow_platform::windows::autostart::status(&morrow_platform::runner::SystemRunner)?;
        if enabled {
            morrow_platform::windows::autostart::install(
                &morrow_platform::runner::SystemRunner,
                &executable.display().to_string(),
            )?;
        } else {
            morrow_platform::windows::autostart::uninstall(&morrow_platform::runner::SystemRunner)?;
        }
        Ok(before.is_some() != enabled)
    }
    #[cfg(unix)]
    {
        let config_home = site.paths.home.join(".config");
        if enabled {
            let executable = std::env::current_exe().map_err(|error| error.to_string())?;
            autostart::install(&config_home, &executable).map_err(|error| error.to_string())
        } else {
            autostart::uninstall(&config_home).map_err(|error| error.to_string())
        }
    }
}

/// The user's settings document.
#[tauri::command]
pub fn settings_load(site: State<'_, HooksSite>) -> Result<Value, String> {
    settings::load(&site.paths.codeisland_dir).map_err(|error| error.to_string())
}

/// Set one top-level setting. The saved document is broadcast to both windows
/// so a theme choice in Settings reaches the island without a restart (#202).
#[tauri::command]
pub fn settings_set(
    app: AppHandle,
    site: State<'_, HooksSite>,
    key: String,
    value: Value,
) -> Result<bool, String> {
    let changed = settings::set(&site.paths.codeisland_dir, &key, &value)
        .map_err(|error| error.to_string())?;
    if let Ok(stored) = settings::load(&site.paths.codeisland_dir) {
        let _ = app.emit(SETTINGS_CHANGED_EVENT, stored);
    }
    Ok(changed)
}

/// Where the settings file is, for a "reveal" button.
#[tauri::command]
pub fn settings_path(site: State<'_, HooksSite>) -> PathBuf {
    settings::path(&site.paths.codeisland_dir)
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;
    use morrow_platform::capability::{Capability, Environment, Os};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn x11(path_tools: Vec<String>) -> Environment {
        Environment {
            os: Os::Linux,
            session_type: Some("x11".into()),
            has_session_bus: false,
            path_tools,
            config_home: PathBuf::new(),
            claude_credentials: PathBuf::new(),
            has_portal_shortcuts: false,
        }
    }

    #[test]
    fn fullscreen_detection_follows_x11_tools_off_windows() {
        // #435: the row used to say "Not supported" on every non-Windows host.
        // It now follows the readers in `morrow_platform::x11`.
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("a clock")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("morrow-shell-caps-{}-{nanos}", std::process::id()));
        let bin = root.join("bin");
        fs::create_dir_all(&bin).expect("bin");

        let without = Feature::Fullscreen.probe(&x11(Vec::new()));
        assert!(
            matches!(without, Capability::Degraded(_)),
            "no tools says {without:?}"
        );
        for tool in ["xdotool", "xprop"] {
            let path = bin.join(tool);
            fs::write(&path, "#!/bin/sh\n").expect("tool");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod");
            }
        }
        let with = Feature::Fullscreen.probe(&x11(vec![bin.to_string_lossy().into_owned()]));
        assert_eq!(with, Capability::Supported);

        let row = capabilities()
            .into_iter()
            .find(|row| row.feature == Feature::Fullscreen.name())
            .expect("a Fullscreen detection row");
        assert!(
            row.capability
                .reason()
                .map_or(true, |reason| !reason.contains("no fullscreen reader")),
            "the rendered table must not repeat the old excuse: {row:?}"
        );
    }
}
