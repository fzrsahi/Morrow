//! The Settings card's side of the hook installer (`morrow-platform::hooks`):
//! report state, install or repair, remove. The panel never needs this; the
//! user opens Settings and asks for it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use morrow_core::config_roots::{self, ConfigDirCli, EntryKind, PathStyle};
use morrow_core::installer::claude_hooks;
use morrow_platform::hooks;
#[cfg(unix)]
use morrow_platform::hooks::Paths;
use morrow_platform::hooks::{Report, Status, TargetStatus};
#[cfg(windows)]
use morrow_platform::hooks_windows;
use morrow_platform::kimi;
use morrow_platform::opencode;
use morrow_platform::settings as app_settings;
use morrow_platform::yaml_hooks;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::HooksSite;

/// What the card shows.
#[derive(Serialize)]
pub struct HooksSnapshot {
    status: Status,
    /// The island's own endpoint, and whether a listener of this user answers.
    socket: SocketHealth,
    /// The three config-root CLIs, with their primary root and hook state.
    config_roots: Vec<ConfigRootStatus>,
    /// Extra config roots registered for those CLIs in Settings.
    extra_dirs: Vec<ExtraConfigDirStatus>,
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
    let style = path_style();
    let socket = socket_health();
    let config_roots = config_root_statuses(&site, version.as_deref(), style);
    let extra_dirs = extra_dir_statuses(&site, version.as_deref(), style);
    #[cfg(unix)]
    {
        HooksSnapshot {
            status: hooks::status(&site.paths, version.as_deref()),
            socket,
            config_roots,
            extra_dirs,
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
            socket,
            config_roots,
            extra_dirs,
            // The other CLIs' files are `<home>/<target.path>` on both platforms
            // — every one of them is a CLI that calls `os.homedir()`, and that is
            // `%USERPROFILE%` here — so the same table and the same policy read
            // them. What was Windows-specific was the *command* each entry names,
            // and that lives in `hooks_windows`.
            targets: hooks::targets_status(&site.paths),
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
                "On Windows this build leaves OpenCode, Kimi and the YAML sources alone: their plugins and file formats are Unix paths today."
                    .to_owned(),
            ),
        }
    }
}

/// The island's endpoint, and whether a listener this user owns answers there.
/// The probe is the hook client's own `connect`: it checks the private folder,
/// the socket's owner and the peer's credentials without sending a byte. The
/// listener already ignores such an empty connection (`listener.rs`).
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct SocketHealth {
    pub endpoint: String,
    pub listening: bool,
    pub detail: String,
}

/// Read the endpoint's health. Settings shows this beside the install state:
/// hooks installed into a CLI are silent when nothing listens at the endpoint.
#[tauri::command]
pub fn hooks_socket_health() -> SocketHealth {
    socket_health()
}

fn socket_health() -> SocketHealth {
    let endpoint = morrow_ipc::current_endpoint();
    socket_health_with(endpoint.as_str(), || {
        morrow_ipc::Client::connect(&endpoint, Duration::from_millis(250))
            .map(|_| ())
            .map_err(|error| error.to_string())
    })
}

fn socket_health_with(endpoint: &str, probe: impl FnOnce() -> Result<(), String>) -> SocketHealth {
    match probe() {
        Ok(()) => SocketHealth {
            endpoint: endpoint.to_owned(),
            listening: true,
            detail: String::new(),
        },
        Err(detail) => SocketHealth {
            endpoint: endpoint.to_owned(),
            listening: false,
            detail,
        },
    }
}

/// One of the three CLIs whose config root can be moved. `ready` says the root
/// exists and looks like this CLI's; `installed` reads its hooks file.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct ConfigRootStatus {
    pub cli: &'static str,
    pub name: &'static str,
    pub path: String,
    pub ready: bool,
    pub installed: bool,
}

/// A registered extra root, as the Hooks page shows it.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct ExtraConfigDirStatus {
    pub id: String,
    pub cli: &'static str,
    pub name: &'static str,
    pub path: String,
    pub enabled: bool,
    pub ready: bool,
    pub installed: bool,
    /// Why nothing is managed here, or `None` when the root is usable.
    pub problem: Option<String>,
}

const CONFIG_ROOT_CLIS: [ConfigDirCli; 3] = [
    ConfigDirCli::Claude,
    ConfigDirCli::Codex,
    ConfigDirCli::Grok,
];
const EXTRA_DIRS_KEY: &str = "hooks_extra_config_dirs";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
struct ExtraDir {
    id: String,
    cli: String,
    path: String,
    enabled: bool,
}

/// The saved roots are the same record the page edits, not a second store: they
/// live in the user's settings document beside every other Settings key.
fn extra_dirs_load(site: &HooksSite) -> Result<Vec<ExtraDir>, String> {
    let document =
        app_settings::load(&site.paths.codeisland_dir).map_err(|error| error.to_string())?;
    match document.get(EXTRA_DIRS_KEY) {
        Some(value) => serde_json::from_value(value.clone())
            .map_err(|error| format!("{EXTRA_DIRS_KEY} in the settings file is invalid: {error}")),
        None => Ok(Vec::new()),
    }
}

fn extra_dirs_save(site: &HooksSite, dirs: &[ExtraDir]) -> Result<(), String> {
    let value = serde_json::to_value(dirs).map_err(|error| error.to_string())?;
    app_settings::set(&site.paths.codeisland_dir, EXTRA_DIRS_KEY, &value)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Path identity for "the same root": a symlink and its target are one account.
/// Case only folds on Windows, where the filesystem does it for us; on Linux a
/// lowercase spelling is a different directory.
fn identity(path: &str, style: PathStyle) -> String {
    let resolved = std::fs::canonicalize(path)
        .map(|resolved| resolved.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_owned());
    if style == PathStyle::Windows {
        resolved.to_lowercase()
    } else {
        resolved
    }
}

fn extra_id(cli: &str, path: &str, style: PathStyle) -> String {
    format!("{}:{}", cli, identity(path, style))
}

fn parse_cli(cli: &str) -> Result<ConfigDirCli, String> {
    CONFIG_ROOT_CLIS
        .iter()
        .copied()
        .find(|entry| entry.source() == cli)
        .ok_or_else(|| format!("unknown config-root CLI {cli}"))
}

/// The CLI's built-in root: the resolved Claude folder for Claude, and the CLI's
/// own environment variable otherwise. Both halves come from `config_roots`.
fn primary_root(cli: ConfigDirCli, site: &HooksSite, style: PathStyle) -> PathBuf {
    match cli {
        ConfigDirCli::Claude => site
            .paths
            .settings
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| site.paths.home.join(".claude")),
        _ => {
            let home = site.paths.home.to_string_lossy();
            let raw = std::env::var(cli.environment_key()).ok();
            config_roots::normalize(raw.as_deref(), &home, style)
                .map(PathBuf::from)
                .unwrap_or_else(|| site.paths.home.join(cli.default_directory_name()))
        }
    }
}

fn markers_hit(path: &str, markers: &[config_roots::Marker], style: PathStyle) -> bool {
    markers.iter().any(|marker| {
        let expected = if marker.is_directory {
            EntryKind::Directory
        } else {
            EntryKind::File
        };
        config_roots::probe_entry(&style.join(path, marker.name)) == expected
    })
}

/// A root is usable when it exists and its distinctive markers say which CLI
/// owns it. Generic markers (`config.toml`, `sessions/`) only settle the question
/// when no other CLI's distinctive marker is there.
fn root_ready(cli: ConfigDirCli, root: &Path, style: PathStyle) -> bool {
    let path = root.to_string_lossy();
    if config_roots::probe_entry(&path) != EntryKind::Directory {
        return false;
    }
    let markers = cli.markers();
    if markers_hit(&path, markers.distinctive, style) {
        return true;
    }
    let claimed_by_other = CONFIG_ROOT_CLIS.iter().any(|other| {
        *other != cli && markers_hit(&path, other.markers().distinctive, style)
    });
    !claimed_by_other && markers_hit(&path, markers.generic, style)
}

fn root_installed(site: &HooksSite, cli: ConfigDirCli, root: &Path, version: Option<&str>) -> bool {
    #[cfg(windows)]
    if cli == ConfigDirCli::Claude {
        return hooks_windows::status(&windows_paths_for(site, root), version).hooks_installed;
    }
    let _ = site;
    hooks::installed_in_root(cli.source(), root, version)
}

fn config_root_statuses(
    site: &HooksSite,
    version: Option<&str>,
    style: PathStyle,
) -> Vec<ConfigRootStatus> {
    CONFIG_ROOT_CLIS
        .iter()
        .map(|cli| {
            let root = primary_root(*cli, site, style);
            ConfigRootStatus {
                cli: cli.source(),
                name: cli.display_name(),
                path: root.display().to_string(),
                ready: root_ready(*cli, &root, style),
                installed: root_installed(site, *cli, &root, version),
            }
        })
        .collect()
}

fn extra_dir_statuses(
    site: &HooksSite,
    version: Option<&str>,
    style: PathStyle,
) -> Vec<ExtraConfigDirStatus> {
    let dirs = extra_dirs_load(site).unwrap_or_default();
    let primary_paths: Vec<String> = CONFIG_ROOT_CLIS
        .iter()
        .map(|cli| identity(&primary_root(*cli, site, style).to_string_lossy(), style))
        .collect();
    let mut seen = std::collections::HashSet::new();
    dirs.iter()
        .filter_map(|dir| {
            let cli = parse_cli(&dir.cli).ok()?;
            let root = PathBuf::from(&dir.path);
            let own_identity = identity(&dir.path, style);
            let ready = root_ready(cli, &root, style);
            let primary_index = CONFIG_ROOT_CLIS
                .iter()
                .position(|entry| *entry == cli)
                .unwrap_or_default();
            let problem = if !ready {
                Some(format!("{} is missing or is not a {} config root", dir.path, cli.display_name()))
            } else if primary_paths[primary_index] == own_identity {
                Some("This is the CLI's primary config root".to_owned())
            } else if !seen.insert(own_identity) {
                Some("This config root is registered twice".to_owned())
            } else {
                None
            };
            Some(ExtraConfigDirStatus {
                id: dir.id.clone(),
                cli: cli.source(),
                name: cli.display_name(),
                path: dir.path.clone(),
                enabled: dir.enabled,
                ready,
                installed: root_installed(site, cli, &root, version),
                problem,
            })
        })
        .collect()
}

/// The command every hook in `root` names. Windows names a `.cmd` beside the
/// app; Unix names the shared script through the stable `~` path.
fn root_command(site: &HooksSite, root: &Path) -> String {
    #[cfg(windows)]
    {
        hooks_windows::command(&windows_paths_for(site, root))
    }
    #[cfg(unix)]
    {
        let _ = (site, root);
        claude_hooks::HOOK_COMMAND.to_owned()
    }
}

fn install_root(site: &HooksSite, cli: ConfigDirCli, root: &Path) -> Result<bool, String> {
    let version = claude_version();
    let epoch = stamp();
    #[cfg(windows)]
    if cli == ConfigDirCli::Claude {
        return hooks_windows::install(
            &windows_paths_for(site, root),
            site.bridge_source.as_deref(),
            version.as_deref(),
            &epoch,
        )
        .map(|_| true)
        .map_err(|error| error.to_string());
    }
    hooks::install_in_root(
        cli.source(),
        root,
        &epoch,
        &root_command(site, root),
        version.as_deref(),
    )
    .map(|report| report.is_some())
    .map_err(|error| error.to_string())
}

fn uninstall_root(site: &HooksSite, cli: ConfigDirCli, root: &Path) -> Result<bool, String> {
    #[cfg(windows)]
    if cli == ConfigDirCli::Claude {
        return hooks_windows::uninstall(&windows_paths_for(site, root))
            .map(|_| true)
            .map_err(|error| error.to_string());
    }
    let _ = site;
    hooks::uninstall_in_root(cli.source(), root)
        .map(|report| report.is_some())
        .map_err(|error| error.to_string())
}

/// Install or repair a config root. `path` is either omitted (the CLI's primary
/// root) or a root registered in Settings; the page cannot name another path.
#[tauri::command]
pub fn config_root_install(
    site: State<'_, HooksSite>,
    cli: String,
    path: Option<String>,
) -> Result<bool, String> {
    let cli = parse_cli(&cli)?;
    let root = resolve_root(&site, cli, path.as_deref())?;
    install_root(&site, cli, &root)
}

/// Take a config root's hooks back out.
#[tauri::command]
pub fn config_root_uninstall(
    site: State<'_, HooksSite>,
    cli: String,
    path: Option<String>,
) -> Result<bool, String> {
    let cli = parse_cli(&cli)?;
    let root = resolve_root(&site, cli, path.as_deref())?;
    uninstall_root(&site, cli, &root)
}

fn resolve_root(site: &HooksSite, cli: ConfigDirCli, path: Option<&str>) -> Result<PathBuf, String> {
    let Some(raw) = path else {
        return Ok(primary_root(cli, site, path_style()));
    };
    let style = path_style();
    let home = site.paths.home.to_string_lossy();
    let normalized = config_roots::normalize(Some(raw), &home, style)
        .ok_or_else(|| format!("{raw} is not an absolute config-root path"))?;
    let id = extra_id(cli.source(), &normalized, style);
    let dirs = extra_dirs_load(site)?;
    dirs.iter()
        .find(|dir| dir.id == id)
        .map(|dir| PathBuf::from(&dir.path))
        .ok_or_else(|| format!("{normalized} is not a registered config root"))
}

/// Register an extra root and put its hooks in, the way the macOS page does.
#[tauri::command]
pub fn extra_config_dir_add(
    site: State<'_, HooksSite>,
    cli: String,
    path: String,
) -> Result<ExtraConfigDirStatus, String> {
    add_extra_dir(&site, &cli, &path)
}

pub(crate) fn add_extra_dir(
    site: &HooksSite,
    cli: &str,
    path: &str,
) -> Result<ExtraConfigDirStatus, String> {
    let cli = parse_cli(cli)?;
    let style = path_style();
    let home = site.paths.home.to_string_lossy();
    let normalized = config_roots::normalize(Some(path), &home, style)
        .ok_or_else(|| format!("{path} is not an absolute directory path"))?;
    if config_roots::probe_entry(&normalized) != EntryKind::Directory {
        return Err(format!("{normalized} is not a directory"));
    }
    let primary = primary_root(cli, site, style);
    if config_roots::roots(
        &primary.to_string_lossy(),
        &[normalized.as_str()],
        |candidate| identity(candidate, style),
    )
    .len()
        < 2
    {
        return Err("That is already this CLI's primary config root".to_owned());
    }
    let mut dirs = extra_dirs_load(site)?;
    let id = extra_id(cli.source(), &normalized, style);
    if dirs.iter().any(|dir| dir.id == id) {
        return Err("That config root is already registered".to_owned());
    }
    dirs.push(ExtraDir {
        id: id.clone(),
        cli: cli.source().to_owned(),
        path: normalized.clone(),
        enabled: true,
    });
    extra_dirs_save(site, &dirs)?;
    install_root(site, cli, Path::new(&normalized))?;
    extra_dir_statuses(site, None, style)
        .into_iter()
        .find(|row| row.id == id)
        .ok_or_else(|| "the new config root did not appear in the snapshot".to_owned())
}

/// Unregister an extra root and take its hooks out, unless another root of the
/// same CLI owns the same file.
#[tauri::command]
pub fn extra_config_dir_remove(site: State<'_, HooksSite>, id: String) -> Result<bool, String> {
    remove_extra_dir(&site, &id)
}

pub(crate) fn remove_extra_dir(site: &HooksSite, id: &str) -> Result<bool, String> {
    let mut dirs = extra_dirs_load(site)?;
    let Some(index) = dirs.iter().position(|dir| dir.id == id) else {
        return Ok(false);
    };
    let dir = dirs.remove(index);
    let cli = parse_cli(&dir.cli)?;
    uninstall_root(site, cli, Path::new(&dir.path))?;
    extra_dirs_save(site, &dirs)?;
    Ok(true)
}

/// Turn an extra root on or off. Off means no hooks are managed there; turning
/// it back on reinstalls them.
#[tauri::command]
pub fn extra_config_dir_set_enabled(
    site: State<'_, HooksSite>,
    id: String,
    enabled: bool,
) -> Result<bool, String> {
    set_extra_dir_enabled(&site, &id, enabled)
}

pub(crate) fn set_extra_dir_enabled(
    site: &HooksSite,
    id: &str,
    enabled: bool,
) -> Result<bool, String> {
    let mut dirs = extra_dirs_load(site)?;
    let Some(dir) = dirs.iter_mut().find(|dir| dir.id == id) else {
        return Ok(false);
    };
    dir.enabled = enabled;
    let cli = parse_cli(&dir.cli)?;
    let path = dir.path.clone();
    extra_dirs_save(site, &dirs)?;
    if enabled {
        install_root(site, cli, Path::new(&path))?;
    } else {
        uninstall_root(site, cli, Path::new(&path))?;
    }
    Ok(true)
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
        // The script first: a fork installed on a profile whose Claude hooks were
        // never installed still needs the `.cmd` its entries name.
        let paths = windows_paths(&site);
        hooks_windows::ensure_script(&paths).map_err(|error| error.to_string())?;
        hooks::install_target_with(
            &site.paths,
            &source,
            &stamp(),
            &hooks_windows::command(&paths),
        )
        .map_err(|error| error.to_string())
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
    // No command needed: uninstall removes entries by recognising our own, and
    // that test is the same on both platforms.
    hooks::uninstall_target(&site.paths, &source).map_err(|error| error.to_string())
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

/// The Windows paths of an explicitly chosen root. Extra config dirs point at
/// another real folder, not at whatever `CLAUDE_CONFIG_DIR` said at launch.
#[cfg(windows)]
fn windows_paths_for(site: &HooksSite, config: &Path) -> hooks_windows::Paths {
    hooks_windows::Paths::new(&site.paths.home, Some(config))
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

/// Claude Code's own folder: `$CLAUDE_CONFIG_DIR` when it names a directory,
/// otherwise the same ladder macOS walks (#258).
///
/// The variable is **not** used verbatim. `~/work/claude` is a documented form
/// and reaches this process unexpanded, so writing `settings.json` beside a
/// literal `~` would put hooks in a folder Claude Code never reads — while still
/// reporting success.
///
/// And when the variable is unset, `~/.claude` is not automatically the answer:
/// macOS prefers a **live** root — one holding `projects/`, which only Claude
/// Code writes — and only then falls back. A `~/.claude` holding nothing but the
/// `settings.json` our own installer wrote is not evidence, so the ladder keeps
/// walking past it. Without that step a user whose config lives in
/// `~/.config/claude` gets hooks written where Claude Code never looks, and
/// everything reports success.
///
/// `morrow_core::config_roots` is the one implementation of both halves, shared
/// with discovery, so the installer and the scanner cannot disagree about which
/// folder is "the" folder.
pub(crate) fn claude_config_dir(home: &Path, raw: Option<&str>) -> Option<PathBuf> {
    let home_text = home.to_str()?;
    let style = path_style();
    let resolved = morrow_core::config_roots::resolve(
        None,
        raw,
        home_text,
        style,
        morrow_core::config_roots::probe_entry,
    );
    // A live root is chosen even when the variable was set-but-unusable, so the
    // answer is never empty; `Paths::new` reads `None` as `~/.claude`.
    let default = morrow_core::config_roots::PathStyle::join(style, home_text, ".claude");
    if resolved == default {
        return None;
    }
    Some(PathBuf::from(resolved))
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

    /// The ladder (#258): without it a user whose config lives in
    /// `~/.config/claude` gets hooks written where Claude Code never looks.
    #[test]
    fn an_unset_variable_falls_through_to_the_ladder() {
        let scratch = std::env::temp_dir().join(format!("morrow-ladder-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&scratch);
        // A `~/.claude` holding only a hook file of ours is not evidence.
        std::fs::create_dir_all(scratch.join(".claude")).expect("a directory");
        std::fs::write(scratch.join(".claude/settings.json"), b"{}").expect("a file");
        // The real config dir is the one with `projects/`.
        std::fs::create_dir_all(scratch.join(".config/claude/projects")).expect("a directory");

        let resolved = claude_config_dir(&scratch, None).expect("a config dir");
        assert_eq!(
            resolved,
            scratch.join(".config/claude"),
            "our own settings.json must not decide which folder is the root"
        );

        // With a live `~/.claude` that one wins again — and because it *is* the
        // default, the answer is `None` rather than a path that repeats it.
        std::fs::create_dir_all(scratch.join(".claude/projects")).expect("a directory");
        assert_eq!(claude_config_dir(&scratch, None), None);

        let _ = std::fs::remove_dir_all(&scratch);
    }

    /// The default is expressed as `None` so `Paths::new` derives it; a path
    /// equal to it must not be passed through as "explicitly configured".
    #[test]
    fn the_default_root_is_not_reported_as_an_explicit_choice() {
        assert_eq!(claude_config_dir(Path::new("/home/dev"), None), None);
        assert_eq!(
            claude_config_dir(Path::new("/home/dev"), Some("/home/dev/.claude")),
            None
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

    #[test]
    fn a_socket_probe_reports_both_outcomes_without_a_transport_of_its_own() {
        let listening = socket_health_with("/tmp/island.sock", || Ok(()));
        assert_eq!(
            listening,
            SocketHealth {
                endpoint: "/tmp/island.sock".to_owned(),
                listening: true,
                detail: String::new(),
            }
        );
        let down = socket_health_with("/tmp/island.sock", || {
            Err("another user owns the path".to_owned())
        });
        assert_eq!(
            down,
            SocketHealth {
                endpoint: "/tmp/island.sock".to_owned(),
                listening: false,
                detail: "another user owns the path".to_owned(),
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn generic_markers_do_not_claim_a_root_another_cli_owns() {
        let scratch = std::env::temp_dir().join(format!(
            "morrow-root-markers-{}-{}",
            std::process::id(),
            stamp()
        ));
        let _ = std::fs::remove_dir_all(&scratch);
        let root = scratch.join("claude-work");
        std::fs::create_dir_all(root.join("projects")).expect("projects");
        let style = PathStyle::Posix;
        assert!(
            root_ready(ConfigDirCli::Claude, &root, style),
            "projects/ is generic but nobody else claims this root"
        );
        // `auth.json` is Codex's distinctive marker: the same folder is now its
        // root, and must not also be offered as a Claude root.
        std::fs::write(root.join("auth.json"), b"{}").expect("auth");
        assert!(!root_ready(ConfigDirCli::Claude, &root, style));
        assert!(root_ready(ConfigDirCli::Codex, &root, style));
        let _ = std::fs::remove_dir_all(&scratch);
    }

    #[test]
    fn an_extra_root_is_registered_hooked_toggled_and_forgotten() {
        let scratch = std::env::temp_dir().join(format!(
            "morrow-extra-root-{}-{}",
            std::process::id(),
            stamp()
        ));
        let _ = std::fs::remove_dir_all(&scratch);
        let home = scratch.join("home");
        let extra = scratch.join("claude-work");
        std::fs::create_dir_all(&home).expect("home");
        std::fs::create_dir_all(extra.join("projects")).expect("extra root");
        let site = HooksSite {
            paths: hooks::Paths::new(&home, None),
            opencode: opencode::Site::new(&home),
            kimi: kimi::Site::new(&home),
            yaml: Vec::new(),
            bridge_source: None,
        };
        let raw = extra.to_string_lossy().into_owned();
        let added = add_extra_dir(&site, "claude", &raw).expect("registered");
        assert_eq!(added.path, raw);
        assert!(added.ready);
        assert!(added.installed, "adding a root installs its hooks");
        assert_eq!(extra_dirs_load(&site).expect("store").len(), 1);

        assert!(set_extra_dir_enabled(&site, &added.id, false).expect("off"));
        assert!(!root_installed(&site, ConfigDirCli::Claude, &extra, None));
        assert!(set_extra_dir_enabled(&site, &added.id, true).expect("on"));
        assert!(root_installed(&site, ConfigDirCli::Claude, &extra, None));

        assert!(remove_extra_dir(&site, &added.id).expect("forgotten"));
        assert!(extra_dirs_load(&site).expect("store").is_empty());
        assert!(!root_installed(&site, ConfigDirCli::Claude, &extra, None));
        let _ = std::fs::remove_dir_all(&scratch);
    }

    #[test]
    fn a_codex_extra_root_gets_its_own_hooks_file_and_feature_flag() {
        let scratch = std::env::temp_dir().join(format!(
            "morrow-codex-root-{}-{}",
            std::process::id(),
            stamp()
        ));
        let _ = std::fs::remove_dir_all(&scratch);
        let home = scratch.join("home");
        let extra = scratch.join("codex-work");
        std::fs::create_dir_all(&home).expect("home");
        std::fs::create_dir_all(&extra).expect("extra root");
        std::fs::write(extra.join("auth.json"), b"{}").expect("Codex marker");
        let site = HooksSite {
            paths: hooks::Paths::new(&home, None),
            opencode: opencode::Site::new(&home),
            kimi: kimi::Site::new(&home),
            yaml: Vec::new(),
            bridge_source: None,
        };
        let added = add_extra_dir(&site, "codex", &extra.to_string_lossy()).expect("registered");
        assert!(added.installed);
        assert!(extra.join("hooks.json").is_file());
        let config = std::fs::read_to_string(extra.join("config.toml")).expect("flag");
        assert!(config.contains("hooks"), "Codex must read the file: {config}");
        assert!(remove_extra_dir(&site, &added.id).expect("forgotten"));
        let after = std::fs::read_to_string(extra.join("hooks.json")).expect("file stays");
        assert!(!after.contains("codeisland"), "our entries are gone: {after}");
        let _ = std::fs::remove_dir_all(&scratch);
    }
}
