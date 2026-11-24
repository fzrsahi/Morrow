//! The Settings card's side of the hook installer (`morrow-platform::hooks`):
//! report state, install or repair, remove. The panel never needs this; the
//! user opens Settings and asks for it.

use std::path::{Path, PathBuf};

#[cfg(unix)]
use morrow_platform::hooks::{self, Paths};
use morrow_platform::hooks::{Report, Status, TargetStatus};
#[cfg(windows)]
use morrow_platform::hooks_windows;
use morrow_platform::kimi;
use morrow_platform::opencode;
use morrow_platform::yaml_hooks;
use tauri::State;

use crate::HooksSite;

/// What the card shows.
#[derive(serde::Serialize)]
pub struct HooksSnapshot {
    status: Status,
    /// The other hook targets and where each one's file stands.
    targets: Vec<TargetStatus>,
    /// OpenCode: a plugin file rather than a hooks key.
    opencode: opencode::Status,
    /// Kimi: hooks in a TOML file, appended rather than re-serialised.
    kimi: kimi::Status,
    /// The YAML sources, as a managed block each.
    yaml: Vec<yaml_hooks::Status>,
    /// The Claude Code version the plan was made for, from `claude --version`.
    version: Option<String>,
    /// What this platform does not do yet, in a sentence, rather than an empty
    /// card that looks like a bug (#108).
    note: Option<String>,
}

/// Read the state of the install.
#[tauri::command]
pub fn hooks_status(site: State<'_, HooksSite>) -> HooksSnapshot {
    let version = claude_version();
    #[cfg(unix)]
    {
        HooksSnapshot {
            status: hooks::status(&site.paths, version.as_deref()),
            targets: hooks::targets_status(&site.paths),
            opencode: opencode::status(&site.opencode),
            kimi: kimi::status(&site.kimi),
            yaml: site.yaml.iter().map(yaml_hooks::status).collect(),
            version,
            note: None,
        }
    }
    #[cfg(windows)]
    {
        HooksSnapshot {
            status: hooks_windows::status(&windows_paths(&site), version.as_deref()),
            // The other eight CLIs' files are read by the same core policy, but
            // their Windows locations are #99's follow-up, not this ticket's:
            // an entry pointing at `~/.codex/config.toml` on Windows would be a
            // path that does not exist.
            targets: Vec::new(),
            opencode: opencode::Status {
                detected: false,
                plugin_installed: false,
                registered: false,
                config: PathBuf::new(),
            },
            kimi: kimi::Status {
                detected: false,
                installed: false,
                config: PathBuf::new(),
            },
            yaml: Vec::new(),
            version,
            note: Some(
                "On Windows this build hooks Claude Code only; the other eight targets arrive with their Windows paths."
                    .to_owned(),
            ),
        }
    }
}

/// Install or repair OpenCode's plugin. `false` when OpenCode is not on this
/// machine or its config does not parse.
#[tauri::command]
pub fn opencode_install(site: State<'_, HooksSite>) -> Result<bool, String> {
    install_opencode(&site)
}

pub(crate) fn install_opencode(site: &HooksSite) -> Result<bool, String> {
    opencode::install(&site.opencode, &stamp())
        .map(|done| done.is_some())
        .map_err(|error| error.to_string())
}

/// Install or repair Kimi's `[[hooks]]` blocks. `false` when Kimi is not on
/// this machine.
#[tauri::command]
pub fn kimi_install(site: State<'_, HooksSite>) -> Result<bool, String> {
    install_kimi(&site)
}

pub(crate) fn install_kimi(site: &HooksSite) -> Result<bool, String> {
    kimi::install(&site.kimi, &stamp())
        .map(|done| done.is_some())
        .map_err(|error| error.to_string())
}

/// Take Kimi's blocks back out, restoring a legacy scalar if there was one.
#[tauri::command]
pub fn kimi_uninstall(site: State<'_, HooksSite>) -> Result<bool, String> {
    uninstall_kimi(&site)
}

pub(crate) fn uninstall_kimi(site: &HooksSite) -> Result<bool, String> {
    kimi::uninstall(&site.kimi, &stamp())
        .map(|done| done.is_some())
        .map_err(|error| error.to_string())
}

/// Install or repair one YAML source's managed block. `false` when that CLI is
/// not on this machine.
#[tauri::command]
pub fn yaml_install(site: State<'_, HooksSite>, source: String) -> Result<bool, String> {
    install_yaml(&site, &source)
}

pub(crate) fn install_yaml(site: &HooksSite, source: &str) -> Result<bool, String> {
    yaml_hooks::install(yaml_site(site, source)?, &stamp())
        .map(|done| done.is_some())
        .map_err(|error| error.to_string())
}

fn yaml_site<'a>(site: &'a HooksSite, source: &str) -> Result<&'a yaml_hooks::Site, String> {
    site.yaml
        .iter()
        .find(|entry| entry.source.source == source)
        .ok_or_else(|| format!("no YAML source named {source}"))
}

/// Take one YAML source's block back out.
#[tauri::command]
pub fn yaml_uninstall(site: State<'_, HooksSite>, source: String) -> Result<bool, String> {
    uninstall_yaml(&site, &source)
}

pub(crate) fn uninstall_yaml(site: &HooksSite, source: &str) -> Result<bool, String> {
    yaml_hooks::uninstall(yaml_site(site, source)?, &stamp())
        .map(|done| done.is_some())
        .map_err(|error| error.to_string())
}

/// Take OpenCode's plugin and its registration back out.
#[tauri::command]
pub fn opencode_uninstall(site: State<'_, HooksSite>) -> Result<bool, String> {
    uninstall_opencode(&site)
}

pub(crate) fn uninstall_opencode(site: &HooksSite) -> Result<bool, String> {
    opencode::uninstall(&site.opencode, &stamp())
        .map(|done| done.is_some())
        .map_err(|error| error.to_string())
}

/// Install or repair.
#[tauri::command]
pub fn hooks_install(site: State<'_, HooksSite>) -> Result<Report, String> {
    install_claude(&site)
}

pub(crate) fn install_claude(site: &HooksSite) -> Result<Report, String> {
    let version = claude_version();
    let epoch = stamp();
    #[cfg(windows)]
    return hooks_windows::install(
        &windows_paths(site),
        site.bridge_source.as_deref(),
        version.as_deref(),
        &epoch,
    )
    .map_err(|error| error.to_string());
    #[cfg(unix)]
    return hooks::install(
        &site.paths,
        site.bridge_source.as_deref(),
        version.as_deref(),
        &epoch,
    )
    .map_err(|error| error.to_string());
}

/// Install or repair one of the other CLIs (`None` when that CLI is not on
/// this machine, so there is nothing to hook).
#[tauri::command]
pub fn target_install(
    site: State<'_, HooksSite>,
    source: String,
) -> Result<Option<Report>, String> {
    #[cfg(windows)]
    {
        let _ = (site, source);
        Err(
            "The other eight targets have no Windows paths yet: that follows this build."
                .to_owned(),
        )
    }
    #[cfg(unix)]
    {
        hooks::install_target(&site.paths, &source, &stamp()).map_err(|error| error.to_string())
    }
}

/// Take one CLI's hook back out.
#[tauri::command]
pub fn target_uninstall(
    site: State<'_, HooksSite>,
    source: String,
) -> Result<Option<Report>, String> {
    #[cfg(windows)]
    {
        let _ = (site, source);
        Err(
            "The other eight targets have no Windows paths yet: that follows this build."
                .to_owned(),
        )
    }
    #[cfg(unix)]
    {
        hooks::uninstall_target(&site.paths, &source).map_err(|error| error.to_string())
    }
}

/// Remove: the script, the bridge, and our entries in `settings.json`.
#[tauri::command]
pub fn hooks_uninstall(site: State<'_, HooksSite>) -> Result<Report, String> {
    uninstall_claude(&site)
}

pub(crate) fn uninstall_claude(site: &HooksSite) -> Result<Report, String> {
    #[cfg(windows)]
    return hooks_windows::uninstall(&windows_paths(site)).map_err(|error| error.to_string());
    #[cfg(unix)]
    return hooks::uninstall(&site.paths).map_err(|error| error.to_string());
}

/// The Claude hook's paths on Windows: `%USERPROFILE%`, with Claude Code's own
/// folder taken from the site the shell already resolved (#108).
#[cfg(windows)]
pub(crate) fn windows_paths(site: &HooksSite) -> hooks_windows::Paths {
    // The unix `Paths` already resolved `$CLAUDE_CONFIG_DIR` against a `.claude`
    // default, so its settings folder is the config folder — unless it is the
    // default, which `Paths::new` derives again from the same home.
    let default = site.paths.home.join(".claude");
    let config = site
        .paths
        .settings
        .parent()
        .filter(|parent| *parent != default)
        .map(std::path::Path::to_path_buf);
    hooks_windows::Paths::new(&site.paths.home, config.as_deref())
}

/// `$CLAUDE_CONFIG_DIR` when set, `~/.claude` otherwise; the bridge beside the
/// app binary unless `$MORROW_BRIDGE_PATH` names another.
pub fn site() -> HooksSite {
    // `%USERPROFILE%` on Windows, `$HOME` everywhere else.
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map_or_else(|| PathBuf::from("/"), PathBuf::from);
    let config_dir = claude_config_dir(&home, std::env::var("CLAUDE_CONFIG_DIR").ok().as_deref());
    HooksSite {
        yaml: yaml_hooks::sites(&home),
        kimi: kimi::Site::new(&home),
        opencode: opencode::Site::new(&home),
        #[cfg(unix)]
        paths: Paths::new(&home, config_dir.as_deref()),
        // The field stays the Unix shape on Windows only so `site()` has one
        // signature; the Windows calls build their own paths from `home` and
        // the settings folder (`windows_paths`).
        #[cfg(windows)]
        paths: morrow_platform::hooks::Paths::new(&home, config_dir.as_deref()),
        bridge_source: bridge_source(),
    }
}

/// Claude Code's own folder: `$CLAUDE_CONFIG_DIR` when it names a directory, the
/// home's `.claude` otherwise.
///
/// The variable is **not** used verbatim. `~/work/claude` is a documented form
/// and reaches this process unexpanded, so writing `settings.json` beside a
/// literal `~` would put hooks in a folder Claude Code never reads — while still
/// reporting success. `morrow_core::config_roots::normalize` is the same seam the
/// installer and discovery share (#247), so the answer is the same as macOS's:
/// `~` expanded, trailing separators dropped, and a relative value or a bare root
/// rejected in favour of the default.
fn claude_config_dir(home: &Path, raw: Option<&str>) -> Option<PathBuf> {
    let home_text = home.to_str()?;
    morrow_core::config_roots::normalize(raw, home_text, path_style()).map(PathBuf::from)
}

/// `$HOME` and the style it is written in agree, but they are separate facts
/// when the two are named on Windows: a drive letter is not a `/` prefix.
const fn path_style() -> morrow_core::config_roots::PathStyle {
    morrow_core::config_roots::PathStyle::current()
}

fn bridge_source() -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os("MORROW_BRIDGE_PATH").map(PathBuf::from) {
        return explicit.is_file().then_some(explicit);
    }
    // Beside the app binary, which is where a deb, an AppImage and a `cargo
    // run` all put the two together.
    let exe = std::env::current_exe().ok()?;
    let name = if cfg!(windows) {
        "morrow-bridge.exe"
    } else {
        "morrow-bridge"
    };
    let sibling = exe.parent()?.join(name);
    sibling.is_file().then_some(sibling)
}

/// `claude --version` → `"2.1.89"`. Unknown when it is not installed, when it
/// answers something else, or when it hangs: the gated events then stay out.
fn claude_version() -> Option<String> {
    let output = std::process::Command::new("claude")
        .arg("--version")
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let first = text.split_whitespace().next()?;
    let numeric = first
        .trim_start_matches('v')
        .split('-')
        .next()
        .unwrap_or(first);
    numeric
        .split('.')
        .all(|part| part.parse::<u32>().is_ok())
        .then(|| numeric.to_owned())
}

/// Seconds since the epoch, for the back-up name: unique enough for one
/// machine's first write, and not worth a date library.
pub(crate) fn stamp() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The variable reaches this process exactly as the user typed it, and a
    /// `~` in it is not expanded by anyone else. Used verbatim, it would create
    /// a literal `~` folder beside wherever the app happens to run.
    #[test]
    fn a_config_dir_variable_starting_with_a_tilde_is_expanded() {
        assert_eq!(
            claude_config_dir(Path::new("/home/dev"), Some("~/work/claude")),
            Some(PathBuf::from("/home/dev/work/claude"))
        );
        assert_eq!(
            claude_config_dir(Path::new("/home/dev"), Some("~")),
            Some(PathBuf::from("/home/dev"))
        );
    }

    #[test]
    fn a_config_dir_variable_is_trimmed() {
        assert_eq!(
            claude_config_dir(Path::new("/home/dev"), Some("  /opt/claude///  ")),
            Some(PathBuf::from("/opt/claude"))
        );
    }

    /// A value that cannot be a config dir falls back to the default, the way
    /// macOS does. A relative one would resolve against the app's cwd and a bare
    /// `/` would yield `//settings.json`; both would "succeed" in the wrong place.
    #[test]
    fn a_config_dir_variable_that_cannot_be_a_directory_falls_back() {
        assert_eq!(claude_config_dir(Path::new("/home/dev"), None), None);
        assert_eq!(claude_config_dir(Path::new("/home/dev"), Some("")), None);
        assert_eq!(claude_config_dir(Path::new("/home/dev"), Some("  ")), None);
        assert_eq!(
            claude_config_dir(Path::new("/home/dev"), Some(".claude")),
            None
        );
        assert_eq!(claude_config_dir(Path::new("/home/dev"), Some("/")), None);
        // The default is the home's own folder, which `Paths::new` derives when
        // this answers `None`.
        assert_eq!(
            path_style().join("/home/dev", ".claude"),
            "/home/dev/.claude"
        );
    }

    #[test]
    fn a_config_dir_variable_on_windows_keeps_its_drive_letter() {
        let style = morrow_core::config_roots::PathStyle::Windows;
        assert_eq!(
            morrow_core::config_roots::normalize(
                Some(r"C:\Users\dev\.claude-work\"),
                r"C:\Users\dev",
                style
            ),
            Some(r"C:\Users\dev\.claude-work".to_owned())
        );
    }
}
