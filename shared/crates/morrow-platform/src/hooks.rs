//! Installing, repairing and removing the Claude Code hook on this machine
//! (`ConfigInstaller.install()` / `uninstall()` / `isInstalled()`, Claude
//! slice).
//!
//! The policy is `morrow_core::installer::claude_hooks`; this is the part that
//! needs a file system: `~/.codeisland` (ours, 0700), the hook script, the
//! bridge binary beside it, and the user's `settings.json` (theirs — backed up
//! once, and never rewritten when it does not parse).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use morrow_core::installer::claude_hooks;
use morrow_core::installer::codex_config;
use morrow_core::installer::targets::{self, Root, Target};

/// Bumped when the script below changes, so an install from an older build is
/// rewritten instead of left in place (`hookScriptVersion`).
pub const HOOK_SCRIPT_VERSION: u32 = 2;

/// What the hook command runs. The stable path first, the older in-app one
/// second, so a machine installed before #104 keeps working; a missing binary is
/// a clean no-op instead of an error on every tool call.
pub fn hook_script() -> String {
    format!(
        "#!/bin/sh\n\
         {marker} — the bridge binary, copied out of the package to a stable path\n\
         BRIDGE=\"$HOME/.local/bin/morrow-bridge\"\n\
         [ -x \"$BRIDGE\" ] || BRIDGE=\"$HOME/.codeisland/morrow-bridge\"\n\
         [ -x \"$BRIDGE\" ] || exit 0\n\
         exec \"$BRIDGE\" \"$@\"\n",
        marker = version_marker(),
    )
}

/// Where everything lives. `config_dir` is Claude Code's, i.e.
/// `$CLAUDE_CONFIG_DIR` when set, `~/.claude` otherwise.
pub struct Paths {
    /// The user's home: every other CLI's file hangs off it.
    pub home: PathBuf,
    /// `~/.codeisland` — this user's alone; holds sessions and the hook.
    pub codeisland_dir: PathBuf,
    /// The script every hook command names.
    pub script: PathBuf,
    /// The bridge binary the script runs.
    pub bridge: PathBuf,
    /// The user's own file.
    pub settings: PathBuf,
}

impl Paths {
    pub fn new(home: &Path, config_dir: Option<&Path>) -> Self {
        let codeisland_dir = home.join(".codeisland");
        let config_dir = config_dir.map_or_else(|| home.join(".claude"), Path::to_path_buf);
        Self {
            home: home.to_path_buf(),
            script: codeisland_dir.join("codeisland-hook.sh"),
            bridge: crate::bridge::stable_path(home),
            settings: config_dir.join("settings.json"),
            codeisland_dir,
        }
    }
}

/// What is on disk right now.
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct Status {
    pub codeisland_dir: PathBuf,
    pub script_installed: bool,
    pub bridge_installed: bool,
    /// Every event of this build carries our hook, with no stale `async` key.
    pub hooks_installed: bool,
    pub settings: PathBuf,
    pub settings_exists: bool,
}

/// One of the other hook targets, and where its file stands.
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct TargetStatus {
    pub source: &'static str,
    pub name: &'static str,
    /// The CLI's own directory is there: without it there is nothing to hook
    /// (`installExternalHooks` skips rather than creating one).
    pub detected: bool,
    pub installed: bool,
    pub path: PathBuf,
}

/// Every target the core knows, in table order, plus the two that are not one key
/// in one file: ZCode's `{enabled, events}` wrapper and MiniMax's plugin pack.
pub fn targets_status(paths: &Paths) -> Vec<TargetStatus> {
    let mut rows: Vec<TargetStatus> = targets::TARGETS
        .iter()
        .map(|target| {
            let path = target.file(&paths.home, grok_home_env);
            let text = read_text(&path).ok().flatten();
            TargetStatus {
                source: target.source,
                name: target.name,
                detected: cli_dir_exists(paths, target),
                installed: text
                    .as_deref()
                    .is_some_and(|text| targets::is_installed(target, text)),
                path,
            }
        })
        .collect();
    rows.push(zcode_status(paths));
    rows.push(minimax_status(paths));
    rows.push(cline_status(paths));
    rows.push(mimo_status(paths));
    // AiWork has no hooks at all: "activated" is monitoring being on and the app
    // being present. A row, and nothing else.
    rows.push(aiwork_status(paths, "aiwork", "AiWork"));
    rows.push(aiwork_status(paths, "aiwork-cli", "AiWork CLI"));
    rows
}

/// ZCode's single file, and the switch inside it.
pub fn zcode_status(paths: &Paths) -> TargetStatus {
    let path = zcode_path(paths);
    let text = read_text(&path).ok().flatten();
    TargetStatus {
        source: "zcode",
        name: "ZCode",
        detected: paths.home.join(".zcode").is_dir(),
        installed: text
            .as_deref()
            .is_some_and(morrow_core::installer::zcode::is_installed),
        path,
    }
}

fn zcode_path(paths: &Paths) -> PathBuf {
    paths.home.join(".zcode/cli/config.json")
}

/// MiniMax's pack: two files, so `installed` means both are current.
pub fn minimax_status(paths: &Paths) -> TargetStatus {
    let plugin = minimax_plugin_dir(paths);
    let manifest = read_text(&plugin.join(".claude-plugin/plugin.json"))
        .ok()
        .flatten();
    let hooks = read_text(&plugin.join("hooks/hooks.json")).ok().flatten();
    TargetStatus {
        source: "minimax",
        name: "MiniMax Code",
        detected: minimax_root(paths).is_dir(),
        installed: manifest
            .as_deref()
            .is_some_and(morrow_core::installer::minimax::manifest_is_ours)
            && hooks.is_some_and(|text| text.contains("codeisland")),
        path: plugin,
    }
}

/// `$MINIMAX_DATA_DIR` or `$MAVIS_DATA_DIR`, expanded, else `~/.minimax`.
fn minimax_root(paths: &Paths) -> PathBuf {
    for key in ["MINIMAX_DATA_DIR", "MAVIS_DATA_DIR"] {
        if let Some(raw) = std::env::var(key).ok().map(|raw| raw.trim().to_owned()) {
            if raw.is_empty() {
                continue;
            }
            if raw == "~" {
                return paths.home.clone();
            }
            if let Some(rest) = raw.strip_prefix("~/") {
                return paths.home.join(rest);
            }
            return PathBuf::from(raw);
        }
    }
    paths.home.join(".minimax")
}

fn minimax_plugin_dir(paths: &Paths) -> PathBuf {
    minimax_root(paths)
        .join("plugins")
        .join(morrow_core::installer::minimax::PLUGIN_NAME)
}

/// The directory each CLI proves itself by. Codex keeps `~/.codex`, Gemini
/// `~/.gemini`; a file the user has never run their CLI for is ours to leave
/// alone.
const CLI_DIRS: &[(&str, &str)] = &[
    ("codex", ".codex"),
    ("gemini", ".gemini"),
    ("cursor", ".cursor"),
    ("qoder", ".qoder"),
    ("droid", ".factory"),
    ("codebuddy", ".codebuddy"),
    ("copilot", ".copilot"),
    // #234 batch 1. A target missing from this table is treated as always
    // present, which is how Settings would end up claiming a CLI the user never
    // installed — `tests/hooks_installer.rs` fails if a row is forgotten.
    ("qoder-cn", ".qoder-cn"),
    ("qoderwork", ".qoderwork"),
    ("codybuddycn", ".codybuddycn"),
    ("stepfun", ".stepfun"),
    ("antigravity", ".antigravity"),
    ("workbuddy", ".workbuddy"),
    ("qwen", ".qwen"),
    // #234 batch 2. Grok is deliberately absent: it hangs from `$GROK_HOME`, and a
    // target with its own root proves itself by that root existing.
    ("traecli-next", ".trae"),
    // #234 batch 3a. Kiro's marker is `.kiro`; its file lives under `agents/`,
    // which an install creates if the directory exists.
    ("trae", ".trae"),
    ("traecn", ".trae-cn"),
    ("kiro", ".kiro"),
    // Google Antigravity proves itself by its own folder inside Gemini's, not by
    // `~/.gemini` — a Gemini CLI user has that one and may have no Antigravity.
    ("google-antigravity", ".gemini/antigravity-cli"),
];

/// Install or repair one target with the Unix hook command.
///
/// Targets are not a Unix-only idea: on Windows the same files sit at
/// `%USERPROFILE%\<target.path>` and the same policy applies. What differs is the
/// **command** each entry names — `~/.codeisland/codeisland-hook.sh` there,
/// `…\morrow-hook.cmd` here — so the caller that knows its own shape passes it.
/// `install_target` is the Unix answer and stays as the short form.
pub fn install_target(paths: &Paths, source: &str, stamp: &str) -> Result<Option<Report>, Error> {
    install_target_with(paths, source, stamp, claude_hooks::HOOK_COMMAND)
}

/// Install or repair one target, naming the command its hooks should carry.
/// `None` when the CLI is not on this machine, or when its file does not parse as
/// a JSON object (nothing is written then).
pub fn install_target_with(
    paths: &Paths,
    source: &str,
    stamp: &str,
    command: &str,
) -> Result<Option<Report>, Error> {
    // Two sources are installers of their own rather than a key in a file: ZCode
    // wraps its events beside a user-owned switch, and MiniMax loads hooks only
    // from a plugin pack (#234 batch 3b).
    match source {
        "zcode" => return install_zcode(paths, stamp, command),
        "minimax" => return install_minimax(paths, stamp, command),
        "cline" => return install_cline(paths, &format!("{command} --source cline")),
        "mimo" => return install_mimo(paths, stamp),
        // Nothing to write: AiWork has no hooks, so "install" is the row itself.
        "aiwork" | "aiwork-cli" => return Ok(Some(Report::default())),
        _ => {}
    }
    let Some(target) = targets::target_for(source) else {
        return Err(Error::Io(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no hook target named {source}"),
        )));
    };
    if !cli_dir_exists(paths, target) {
        return Ok(None);
    }
    let path = target.file(&paths.home, grok_home_env);
    let existing = read_text(&path)?;
    let outcome = targets::install(target, existing.as_deref(), command)
        .ok_or_else(|| Error::UnparseableSettings(path.clone()))?;

    let mut report = Report::default();
    if outcome.changed {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        if existing.is_some() {
            report.backup = back_up(&path, stamp)?;
        }
        fs::write(&path, &outcome.text)?;
        report.settings_changed = true;
    }

    // Codex only reads `hooks.json` when `config.toml` turns the feature on
    // (`enableCodexFeatureFlag`).
    if target.source == "codex" {
        report.flag_enabled = enable_codex_hooks(paths)?;
    }
    Ok(Some(report))
}

/// `features.hooks = true` in `~/.codex/config.toml`, or an error when that
/// file's layout is one the editor will not touch.
fn enable_codex_hooks(paths: &Paths) -> Result<bool, Error> {
    let config_path = paths.home.join(".codex/config.toml");
    let existing = read_text(&config_path)?.unwrap_or_default();
    let Some(updated) = codex_config::enabling_hooks(&existing) else {
        return Err(Error::UnparseableSettings(config_path));
    };
    if updated == existing {
        return Ok(false);
    }
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&config_path, updated)?;
    Ok(true)
}

/// Take one target's hook back out.
pub fn uninstall_target(paths: &Paths, source: &str) -> Result<Option<Report>, Error> {
    match source {
        "zcode" => return uninstall_zcode(paths),
        "minimax" => return uninstall_minimax(paths),
        "cline" => return uninstall_cline(paths),
        "mimo" => return uninstall_mimo(paths),
        "aiwork" | "aiwork-cli" => return Ok(None),
        _ => {}
    }
    let Some(target) = targets::target_for(source) else {
        return Err(Error::Io(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no hook target named {source}"),
        )));
    };
    let path = target.file(&paths.home, grok_home_env);
    let Some(existing) = read_text(&path)? else {
        return Ok(None);
    };
    let outcome = targets::uninstall(target, &existing)
        .ok_or_else(|| Error::UnparseableSettings(path.clone()))?;
    let mut report = Report::default();
    if outcome.changed {
        fs::write(&path, &outcome.text)?;
        report.settings_changed = true;
    }
    Ok(Some(report))
}

fn cli_dir_exists(paths: &Paths, target: &Target) -> bool {
    // A target that does not hang from the home proves itself by its **root**
    // existing: `$GROK_HOME` is the store, and there is no fixed directory name to
    // look for beside it.
    if target.root != Root::Home {
        return target.root.directory(&paths.home, grok_home_env).is_dir();
    }
    CLI_DIRS
        .iter()
        .find(|(source, _)| *source == target.source)
        .is_none_or(|(_, directory)| paths.home.join(directory).is_dir())
}

/// ZCode's single file, written through the core's plan (#234 batch 3b).
fn install_zcode(paths: &Paths, stamp: &str, command: &str) -> Result<Option<Report>, Error> {
    if !zcode_status(paths).detected {
        return Ok(None);
    }
    // ZCode's command names its source itself: the caller hands over the bare hook
    // command, as the target table's `command_for` does for every other CLI.
    let command = format!("{command} --source zcode");
    let path = zcode_path(paths);
    let existing = read_text(&path)?;
    let outcome = morrow_core::installer::zcode::install(existing.as_deref(), &command)
        .ok_or_else(|| Error::UnparseableSettings(path.clone()))?;
    let mut report = Report::default();
    if outcome.changed {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        if existing.is_some() {
            report.backup = back_up(&path, stamp)?;
        }
        fs::write(&path, &outcome.text)?;
        report.settings_changed = true;
    }
    Ok(Some(report))
}

fn uninstall_zcode(paths: &Paths) -> Result<Option<Report>, Error> {
    let path = zcode_path(paths);
    let Some(existing) = read_text(&path)? else {
        return Ok(None);
    };
    let Some(outcome) = morrow_core::installer::zcode::uninstall(&existing) else {
        return Ok(None);
    };
    let mut report = Report::default();
    if outcome.changed {
        fs::write(&path, &outcome.text)?;
        report.settings_changed = true;
    }
    Ok(Some(report))
}

/// MiniMax's plugin pack: two files under `<root>/plugins/morrow` (#234 batch 3b).
///
/// A pack someone else put there is refused, not overwritten — the manifest's
/// marker is the only proof of authorship this installer has.
fn install_minimax(paths: &Paths, _stamp: &str, command: &str) -> Result<Option<Report>, Error> {
    if !minimax_root(paths).is_dir() {
        return Ok(None);
    }
    let command = format!("{command} --source minimax");
    let plugin = minimax_plugin_dir(paths);
    let manifest_path = plugin.join(".claude-plugin/plugin.json");
    if let Some(existing) = read_text(&manifest_path)? {
        if !morrow_core::installer::minimax::manifest_is_ours(&existing) {
            return Err(Error::Io(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "{} holds a plugin this installer did not write",
                    plugin.display()
                ),
            )));
        }
    }
    let mut report = Report::default();
    for (relative, text) in morrow_core::installer::minimax::files(&command) {
        let path = plugin.join(relative);
        if read_text(&path)?.as_deref() == Some(text.as_str()) {
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, text)?;
        report.settings_changed = true;
    }
    Ok(Some(report))
}

fn uninstall_minimax(paths: &Paths) -> Result<Option<Report>, Error> {
    let path = minimax_plugin_dir(paths).join(".claude-plugin/plugin.json");
    let Some(existing) = read_text(&path)? else {
        return Ok(None);
    };
    if !morrow_core::installer::minimax::manifest_is_ours(&existing) {
        return Ok(None);
    }
    let mut report = Report::default();
    // The pack is entirely ours — two files in a directory named after us — so the
    // whole directory goes, rather than a pack with no hooks in it left behind.
    if let Some(plugin) = path.parent().and_then(|dir| dir.parent()) {
        if fs::remove_dir_all(plugin).is_ok() {
            report.settings_changed = true;
        }
    }
    Ok(Some(report))
}

/// Cline's eight scripts (#234 batch 4). No file to merge: a directory of hooks.
fn cline_hooks_dir(paths: &Paths) -> PathBuf {
    paths.home.join("Documents/Cline/Hooks")
}

pub fn cline_status(paths: &Paths) -> TargetStatus {
    let directory = cline_hooks_dir(paths);
    let installed = morrow_core::installer::cline::EVENTS.iter().all(|event| {
        read_text(&directory.join(event))
            .ok()
            .flatten()
            .is_some_and(|text| morrow_core::installer::cline::is_ours(&text))
    });
    TargetStatus {
        source: "cline",
        name: "Cline",
        // The VS Code extension's storage, or Cline's own folder. Creating
        // `Documents/Cline` for someone without Cline would make the row claim a
        // CLI they do not have.
        detected: paths
            .home
            .join("Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev")
            .exists()
            || paths.home.join("Documents/Cline").is_dir(),
        installed,
        path: directory,
    }
}

fn install_cline(paths: &Paths, command: &str) -> Result<Option<Report>, Error> {
    if !cline_status(paths).detected {
        return Ok(None);
    }
    let directory = cline_hooks_dir(paths);
    fs::create_dir_all(&directory)?;
    let script = morrow_core::installer::cline::script(command);
    let mut report = Report::default();
    for event in morrow_core::installer::cline::EVENTS {
        let path = directory.join(event);
        if read_text(&path)?.as_deref() == Some(script.as_str()) {
            continue;
        }
        fs::write(&path, &script)?;
        set_executable(&path)?;
        report.settings_changed = true;
    }
    Ok(Some(report))
}

fn uninstall_cline(paths: &Paths) -> Result<Option<Report>, Error> {
    let directory = cline_hooks_dir(paths);
    let mut report = Report::default();
    for event in morrow_core::installer::cline::EVENTS {
        let path = directory.join(event);
        // Only a file that says it is ours: the user's own hook for the same
        // event is theirs.
        let Some(text) = read_text(&path)? else {
            continue;
        };
        if !morrow_core::installer::cline::is_ours(&text) {
            continue;
        }
        fs::remove_file(&path)?;
        report.settings_changed = true;
    }
    Ok(report.settings_changed.then_some(report))
}

#[cfg(unix)]
fn set_executable(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn set_executable(_path: &std::path::Path) -> std::io::Result<()> {
    // Cline on Windows runs these through Git Bash, where the mode is not what
    // decides: the file existing is. Nothing to set.
    Ok(())
}

/// MiMo Code's plugin, which is OpenCode's file re-labelled (#234 batch 5).
pub fn mimo_status(paths: &Paths) -> TargetStatus {
    let path = paths
        .home
        .join(morrow_core::installer::mimo::MIMO_PLUGIN_PATH);
    TargetStatus {
        source: "mimo",
        name: "MiMo Code",
        detected: paths.home.join(".config/mimocode").is_dir()
            || paths
                .home
                .join(morrow_core::installer::mimo::MIMO_INSTALL_DIR)
                .is_dir(),
        installed: read_text(&path)
            .ok()
            .flatten()
            .is_some_and(|text| morrow_core::installer::mimo::is_our_mimo_plugin(&text)),
        path,
    }
}

fn install_mimo(paths: &Paths, stamp: &str) -> Result<Option<Report>, Error> {
    if !mimo_status(paths).detected {
        return Ok(None);
    }
    let path = paths
        .home
        .join(morrow_core::installer::mimo::MIMO_PLUGIN_PATH);
    // An artefact that lost its marker yields nothing, and shipping OpenCode's own
    // file under MiMo's name would mislabel every event.
    let source = morrow_core::installer::mimo::mimo_plugin_source(crate::opencode::PLUGIN_SOURCE)
        .ok_or_else(|| {
        Error::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            "the bundled plugin no longer names its source line exactly once",
        ))
    })?;
    if read_text(&path)?.as_deref() == Some(source.as_str()) {
        return Ok(Some(Report::default()));
    }
    let existing = read_text(&path)?;
    if existing
        .as_deref()
        .is_some_and(|text| !morrow_core::installer::mimo::is_our_mimo_plugin(text))
    {
        return Ok(None);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut report = Report::default();
    if existing.is_some() {
        report.backup = back_up(&path, stamp)?;
    }
    fs::write(&path, source)?;
    report.settings_changed = true;
    Ok(Some(report))
}

fn uninstall_mimo(paths: &Paths) -> Result<Option<Report>, Error> {
    let path = paths
        .home
        .join(morrow_core::installer::mimo::MIMO_PLUGIN_PATH);
    let Some(existing) = read_text(&path)? else {
        return Ok(None);
    };
    if !morrow_core::installer::mimo::is_our_mimo_plugin(&existing) {
        return Ok(None);
    }
    fs::remove_file(&path)?;
    Ok(Some(Report {
        settings_changed: true,
        ..Report::default()
    }))
}

/// AiWork: nothing to install, so `installed` is `detected` and the install is a
/// no-op that says what it did.
pub fn aiwork_status(paths: &Paths, source: &'static str, name: &'static str) -> TargetStatus {
    let detected = if source == "aiwork" {
        // The GUI's state directory is what decides in practice: a GUI-launched
        // app inherits a minimal PATH and cannot see the CLI's binary.
        paths
            .home
            .join(morrow_core::installer::mimo::AIWORK_STATE_DIR)
            .join("run")
            .is_dir()
    } else {
        // The CLI is on PATH; the state directory is the same evidence.
        paths
            .home
            .join(morrow_core::installer::mimo::AIWORK_STATE_DIR)
            .join("run")
            .is_dir()
            || in_path("aiwork")
            || in_path("dtcoder")
    };
    TargetStatus {
        source,
        name,
        detected,
        installed: detected,
        path: paths
            .home
            .join(morrow_core::installer::mimo::AIWORK_STATE_DIR),
    }
}

fn in_path(binary: &str) -> bool {
    std::env::var_os("PATH")
        .map(|path| {
            std::env::split_paths(&path).any(|directory| {
                std::fs::metadata(directory.join(binary))
                    .map(|metadata| metadata.is_file())
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// The one environment lookup the target table needs, in one place so it can be
/// read, and so `Root` itself stays a pure function (#234).
fn grok_home_env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// What an install or uninstall did.
#[derive(Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Report {
    pub script_written: bool,
    pub bridge_copied: bool,
    pub settings_changed: bool,
    /// Codex's `features.hooks = true` was added (or flipped) in `config.toml`.
    pub flag_enabled: bool,
    /// The back-up this run made, if it made one.
    pub backup: Option<PathBuf>,
}

#[derive(Debug)]
pub enum Error {
    /// The user's `settings.json` is not a JSON object. It stays untouched
    /// (#89): a settings file that half-parses is worth more than ours.
    UnparseableSettings(PathBuf),
    /// The core refused a layout rather than guess at it — an inline `hooks:`
    /// value, tab indentation. The file is untouched.
    Refused(String),
    Io(io::Error),
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnparseableSettings(path) => write!(
                f,
                "{} is not a JSON object; leaving it alone",
                path.display()
            ),
            Self::Refused(reason) => write!(f, "not written: {reason}"),
            Self::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for Error {}

/// Install or repair everything. `bridge_source` is the binary shipped beside
/// the app; when it is missing the script still lands, and reports it.
pub fn install(
    paths: &Paths,
    bridge_source: Option<&Path>,
    version: Option<&str>,
    stamp: &str,
) -> Result<Report, Error> {
    // Plan the settings change first: a refusal must leave the machine as it
    // was, script and binary included.
    let existing = read_text(&paths.settings)?;
    let plan = existing
        .as_deref()
        .map_or_else(
            || claude_hooks::install(None, claude_hooks::HOOK_COMMAND, version),
            |text| claude_hooks::install(Some(text), claude_hooks::HOOK_COMMAND, version),
        )
        .ok_or_else(|| Error::UnparseableSettings(paths.settings.clone()))?;

    let mut report = Report::default();

    fs::create_dir_all(&paths.codeisland_dir)?;
    private(&paths.codeisland_dir, 0o700)?;

    match read_text(&paths.script)? {
        Some(text) if text.contains(&version_marker()) => private(&paths.script, 0o700)?,
        _ => {
            write_script(&paths.script)?;
            report.script_written = true;
        }
    }

    if let Some(source) = bridge_source {
        report.bridge_copied = crate::bridge::ensure(source, &paths.home)?.copied;
    }

    if plan.changed {
        if existing.is_some() {
            report.backup = back_up(&paths.settings, stamp)?;
        }
        fs::write(&paths.settings, &plan.text)?;
        report.settings_changed = true;
    }

    Ok(report)
}

/// Take the hook back out: the script, the bridge, and our entries in
/// `settings.json` — which comes back byte for byte when it held only ours.
pub fn uninstall(paths: &Paths) -> Result<Report, Error> {
    let existing = read_text(&paths.settings)?;
    let plan = match existing.as_deref() {
        Some(text) => claude_hooks::uninstall(text)
            .ok_or_else(|| Error::UnparseableSettings(paths.settings.clone()))?,
        None => return Ok(Report::default()),
    };

    let mut report = Report::default();
    if plan.changed {
        fs::write(&paths.settings, &plan.text)?;
        report.settings_changed = true;
    }
    report.script_written = remove_file(&paths.script)?;
    report.bridge_copied = remove_file(&paths.bridge)?;
    Ok(report)
}

/// What a Settings page shows.
pub fn status(paths: &Paths, version: Option<&str>) -> Status {
    let settings = read_text(&paths.settings).ok().flatten();
    Status {
        codeisland_dir: paths.codeisland_dir.clone(),
        script_installed: paths.script.is_file(),
        bridge_installed: paths.bridge.is_file(),
        hooks_installed: settings.as_deref().is_some_and(|text| {
            claude_hooks::is_installed(text, claude_hooks::HOOK_COMMAND, version)
        }),
        settings: paths.settings.clone(),
        settings_exists: paths.settings.is_file(),
    }
}

fn version_marker() -> String {
    format!("# Morrow hook v{HOOK_SCRIPT_VERSION}")
}

pub(crate) fn read_text(path: &Path) -> io::Result<Option<String>> {
    match fs::read(path) {
        Ok(bytes) => Ok(String::from_utf8(bytes).ok()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// 0700: `~/.codeisland` holds this user's prompts and replies.
fn private(path: &Path, mode: u32) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
    Ok(())
}

fn write_script(path: &Path) -> io::Result<()> {
    fs::write(path, hook_script())?;
    private(path, 0o700)
}

/// One back-up per file, ever: a second install does not pile them up
/// (`backupOpencodeConfig`).
pub(crate) fn back_up(settings: &Path, stamp: &str) -> io::Result<Option<PathBuf>> {
    let directory = settings.parent().unwrap_or(Path::new("."));
    let names: Vec<String> = fs::read_dir(directory)?
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    let Some(path) = claude_hooks::backup_path(&settings.to_string_lossy(), stamp, &names) else {
        return Ok(None);
    };
    fs::copy(settings, &path)?;
    Ok(Some(PathBuf::from(path)))
}

fn remove_file(path: &Path) -> io::Result<bool> {
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}
