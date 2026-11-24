//! Where the CLI config roots are (#247).
//!
//! The macOS side splits this in two: `ClaudeConfigPaths` / `ExtraConfigDirs`
//! decide *which* paths, `ConfigPathIdentity` / the discovery walk decide what is
//! on disk. This is the first half — every filesystem question is an injected
//! probe, so the whole ladder is a table test rather than a fixture tree.
//!
//! Shared with the installer on purpose (#234): the same function that finds a
//! root writes hooks into it, or the two would disagree about what "the primary
//! root" means.
//!
//! ## Deliberately not ported: NFC
//!
//! `ClaudeConfigPaths.normalized` ends with `precomposedStringWithCanonicalMapping`
//! because Claude Code normalizes its own path to NFC on macOS, where APFS keeps
//! either form. Linux and Windows filesystems do not normalize, so a decomposed
//! name there is a *different directory* — applying NFC in the port would make it
//! look in a directory that does not exist. The returned path keeps the spelling
//! it was given; case folding and normalization belong to path *identity*, which
//! is a separate seam (#247b).

use std::collections::HashSet;

/// One CLI whose config root can be moved. Mirrors `ConfigDirCLI`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConfigDirCli {
    Claude,
    Codex,
    Grok,
}

/// A file or directory that marks a config root as belonging to a CLI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Marker {
    pub name: &'static str,
    pub is_directory: bool,
}

const fn file(name: &'static str) -> Marker {
    Marker {
        name,
        is_directory: false,
    }
}

const fn directory(name: &'static str) -> Marker {
    Marker {
        name,
        is_directory: true,
    }
}

/// `distinctive` entries are written only by this CLI, so one settles the
/// question. `generic` ones are shared with a sibling (`config.toml`,
/// `sessions/`) and only count when no *other* CLI's distinctive entry is
/// present — that is what tells `~/.codex` apart from `~/.grok` although both
/// hold `config.toml` and `sessions/`.
pub struct Markers {
    pub distinctive: &'static [Marker],
    pub generic: &'static [Marker],
}

// Declared as `const` items rather than array literals in the match: a `const
// fn` call is not promoted to `'static` on its own.
const CLAUDE_DISTINCTIVE: &[Marker] = &[
    file("settings.json"),
    file(".claude.json"),
    file(".credentials.json"),
    directory("statsig"),
    directory("todos"),
    directory("shell-snapshots"),
];
const CLAUDE_GENERIC: &[Marker] = &[
    directory("projects"),
    file("history.jsonl"),
    directory("plugins"),
];
const CODEX_DISTINCTIVE: &[Marker] = &[
    file("auth.json"),
    file(".codex-global-state.json"),
    file("state_5.sqlite"),
    file("hooks.json"),
    directory("archived_sessions"),
];
const CODEX_GENERIC: &[Marker] = &[
    file("config.toml"),
    directory("sessions"),
    file("history.jsonl"),
    directory("log"),
];
const GROK_DISTINCTIVE: &[Marker] = &[
    file("active_sessions.json"),
    file(".metadata_version"),
    file("trusted_folders.toml"),
    file("worktrees.db"),
];
const GROK_GENERIC: &[Marker] = &[
    file("config.toml"),
    directory("sessions"),
    directory("hooks"),
    directory("logs"),
];

impl ConfigDirCli {
    /// `CLIConfig.source` of the built-in entry these directories extend.
    pub fn source(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Grok => "grok",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex",
            Self::Grok => "Grok CLI",
        }
    }

    /// The variable the CLI reads to pick its root.
    pub fn environment_key(self) -> &'static str {
        match self {
            Self::Claude => "CLAUDE_CONFIG_DIR",
            Self::Codex => "CODEX_HOME",
            Self::Grok => "GROK_HOME",
        }
    }

    /// Where the CLI's per-session transcripts live, relative to its root.
    pub fn session_store_subdirectory(self) -> &'static str {
        match self {
            Self::Claude => "projects",
            Self::Codex => "sessions",
            Self::Grok => "sessions",
        }
    }

    /// The directory name under the home folder when the variable is unset.
    pub fn default_directory_name(self) -> &'static str {
        match self {
            Self::Claude => ".claude",
            Self::Codex => ".codex",
            Self::Grok => ".grok",
        }
    }

    pub fn markers(self) -> Markers {
        match self {
            Self::Claude => Markers {
                distinctive: CLAUDE_DISTINCTIVE,
                generic: CLAUDE_GENERIC,
            },
            Self::Codex => Markers {
                distinctive: CODEX_DISTINCTIVE,
                generic: CODEX_GENERIC,
            },
            Self::Grok => Markers {
                distinctive: GROK_DISTINCTIVE,
                generic: GROK_GENERIC,
            },
        }
    }
}

/// What a path is on disk — the only fact the pure checks need.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Missing,
    File,
    Directory,
}

/// How to read a path. Injected so the Windows rules are testable on a POSIX
/// host — the port's whole risk is that a `/`-only assumption compiles fine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathStyle {
    Posix,
    Windows,
}

impl PathStyle {
    /// The style of the host this binary runs on.
    pub const fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Posix
        }
    }

    pub fn is_separator(self, c: char) -> bool {
        match self {
            Self::Posix => c == '/',
            // Windows accepts both, and a hand-typed variable often mixes them.
            Self::Windows => c == '\\' || c == '/',
        }
    }

    pub fn separator(self) -> char {
        match self {
            Self::Posix => '/',
            Self::Windows => '\\',
        }
    }

    pub fn is_absolute(self, path: &str) -> bool {
        match self {
            Self::Posix => path.starts_with('/'),
            // `C:\…`, `C:/…`, and a UNC `\\server\share`.
            Self::Windows => {
                let bytes = path.as_bytes();
                let drive = bytes.len() >= 2
                    && bytes[0].is_ascii_alphabetic()
                    && (bytes[1] == b':' || bytes[1] == b'\\' || bytes[1] == b'/');
                drive || path.starts_with("\\\\") || path.starts_with("//")
            }
        }
    }

    /// `path` joined with `name`, using the host's separator but never
    /// doubling one that is already there.
    pub fn join(self, path: &str, name: &str) -> String {
        let tail = path.chars().last();
        match tail {
            Some(c) if self.is_separator(c) => format!("{path}{name}"),
            _ => format!("{path}{}{name}", self.separator()),
        }
    }

    /// The length of the shortest form this path may be trimmed to: `/` on
    /// POSIX, `C:\` or `\\server` on Windows. Anything shorter stops being a
    /// root.
    fn root_len(self, path: &str) -> usize {
        match self {
            Self::Posix => 1,
            Self::Windows => {
                let bytes = path.as_bytes();
                if bytes.len() >= 3
                    && bytes[0].is_ascii_alphabetic()
                    && bytes[1] == b':'
                    && self.is_separator(bytes[2] as char)
                {
                    return 3;
                }
                if path.starts_with("\\\\") || path.starts_with("//") {
                    return 2;
                }
                0
            }
        }
    }

    /// Drop trailing separators without eating the root separator itself.
    fn trim_trailing_separators(self, path: &str) -> &str {
        let floor = self.root_len(path);
        let mut end = path.len();
        while end > floor {
            let c = path[..end].chars().next_back().expect("non-empty");
            if !self.is_separator(c) {
                break;
            }
            end -= c.len_utf8();
        }
        &path[..end]
    }
}

/// `ClaudeConfigPaths.normalized`: trim, expand `~`, drop trailing
/// separators. `None` for empty, relative, or a bare root.
///
/// A relative path is rejected rather than resolved against the app's cwd, and
/// `/` is rejected because it would yield `//projects` — the installer would
/// happily create that and report success while the CLI read somewhere else.
/// Falling through to auto-detect is strictly better than acting on a typo.
///
/// The path is returned with the separators it was given: rewriting them would
/// change the path's identity for no gain, since both spellings reach the same
/// file.
pub fn normalize(raw: Option<&str>, home_dir: &str, style: PathStyle) -> Option<String> {
    let raw = raw.unwrap_or_default().trim();
    if raw.is_empty() {
        return None;
    }
    if raw == "~" {
        return Some(home_dir.to_owned());
    }
    let mut value = raw.to_owned();
    if let Some(rest) = raw.strip_prefix("~/").or_else(|| {
        if style == PathStyle::Windows {
            raw.strip_prefix("~\\")
        } else {
            None
        }
    }) {
        value = style.join(home_dir, rest);
    }
    let trimmed = style.trim_trailing_separators(&value);
    if !style.is_absolute(trimmed) {
        return None;
    }
    if trimmed.len() <= style.root_len(trimmed) {
        return None;
    }
    Some(trimmed.to_owned())
}

/// The real filesystem probe: is `path` a directory, a file, or nothing?
///
/// Mirrors `ConfigDirEntryKind.probe`, and follows symlinks — a symlinked config
/// dir (a common multi-account setup) counts as a directory.
pub fn probe_entry(path: &str) -> EntryKind {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => EntryKind::Directory,
        Ok(_) => EntryKind::File,
        Err(_) => EntryKind::Missing,
    }
}

/// A directory only counts as a live Claude config dir if it holds `projects/`,
/// which is written only by Claude Code itself.
///
/// `settings.json` deliberately does **not** count: CodeIsland's own installer
/// creates it, so treating it as evidence would let us manufacture the very
/// signal we then read back — an empty `~/.claude` containing nothing but a
/// hook file of ours would outrank the user's real config dir forever.
///
/// It asks whether `projects` is a *directory*, not merely that the path exists
/// — a stray regular file of that name must not mark a root live.
pub fn is_live_config_dir(
    path: &str,
    style: PathStyle,
    kind_of: impl Fn(&str) -> EntryKind,
) -> bool {
    kind_of(&style.join(path, "projects")) == EntryKind::Directory
}

/// `ClaudeConfigPaths.resolve`: which Claude root to use, in the order that
/// decides it.
///
/// A live `~/.claude` wins over the XDG probe, because Claude Code itself has no
/// XDG rung — it reads `$CLAUDE_CONFIG_DIR`, else `~/.claude`. Preferring a stale
/// `~/.config/claude-code` would silently repoint us away from the directory
/// Claude Code actually reads.
///
/// When nothing is live the answer is still `~/.claude`: the installer creates
/// it. A missing directory is not an error.
pub fn resolve(
    preference: Option<&str>,
    environment: Option<&str>,
    home_dir: &str,
    style: PathStyle,
    kind_of: impl Fn(&str) -> EntryKind,
) -> String {
    if let Some(preference) = normalize(preference, home_dir, style) {
        return preference;
    }
    if let Some(environment) = normalize(environment, home_dir, style) {
        return environment;
    }
    let dot_claude = style.join(home_dir, ".claude");
    if is_live_config_dir(&dot_claude, style, &kind_of) {
        return dot_claude;
    }
    // Both spellings exist in the wild: ccusage & co. default to
    // ~/.config/claude, older setups used ~/.config/claude-code.
    for name in ["claude", "claude-code"] {
        let path = style.join(&style.join(home_dir, ".config"), name);
        if is_live_config_dir(&path, style, &kind_of) {
            return path;
        }
    }
    dot_claude
}

/// `ExtraConfigDirs.roots`: primary first, then the extras, without duplicates.
///
/// `identity` decides "the same root" (symlink, case, normalization). A
/// different spelling of the primary is dropped from the extras, which is what
/// keeps one account from being scanned twice and its session counted as two.
pub fn roots(primary: &str, extras: &[&str], identity: impl Fn(&str) -> String) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for path in std::iter::once(primary).chain(extras.iter().copied()) {
        if seen.insert(identity(path)) {
            result.push(path.to_owned());
        }
    }
    result
}

/// `ExtraConfigDirs.processRoot`: the root a running CLI process reads, from its
/// own environment.
///
/// - `environment == None`: the environment could not be read (or only in
///   part), so the root is unknown — see `ConfigRootSnapshot`.
/// - variable set to an absolute path: that directory, registered or not — the
///   process writes there no matter what Settings says.
/// - variable set to anything else (a relative path, a bare root): unknown too.
///   The CLI resolves it against its own cwd; it is not "unset", and treating it
///   so would pin the process to the wrong account.
/// - variable unset or blank: the CLI's built-in default.
pub fn process_root(
    cli: ConfigDirCli,
    environment: Option<&std::collections::HashMap<String, String>>,
    home_dir: &str,
    default_root: &str,
    style: PathStyle,
) -> Option<String> {
    let environment = environment?;
    let raw = environment.get(cli.environment_key());
    match raw.map(|raw| raw.trim()) {
        None | Some("") => Some(default_root.to_owned()),
        Some(raw) => normalize(Some(raw), home_dir, style),
    }
}

/// `ExtraConfigDirs.owningRoot`: the root among `roots` that contains `path` —
/// how a transcript a hook reported is traced back to the account it came from.
///
/// The deepest match wins, so a root nested in another is not swallowed by its
/// parent. Both sides are compared by `identity`, because the CLI reports the
/// path it built from its own variable while the root may be registered as a
/// symlink or with different letter case. A spelling mismatch must not fall back
/// to the primary account — that is how "Always allow" would land in the wrong
/// account's rules.
pub fn owning_root(
    path: Option<&str>,
    roots: &[String],
    style: PathStyle,
    identity: impl Fn(&str) -> String,
) -> Option<String> {
    let path = path.filter(|path| !path.is_empty())?;
    let candidate = identity(path);
    roots
        .iter()
        .map(|root| (root, identity(root)))
        .filter(|(_, root_identity)| {
            candidate == *root_identity || candidate.starts_with(&style.join(root_identity, ""))
        })
        .max_by_key(|(_, root_identity)| root_identity.len())
        .map(|(root, _)| root.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn style() -> PathStyle {
        PathStyle::Posix
    }

    #[test]
    fn a_variable_that_names_a_directory_is_used_as_written() {
        // The table #247 asks for: environment in, path out.
        assert_eq!(
            normalize(Some("/Users/dev/.claude-work"), "/Users/dev", style()),
            Some("/Users/dev/.claude-work".to_owned())
        );
        assert_eq!(
            normalize(Some("  /opt/claude  "), "/Users/dev", style()),
            Some("/opt/claude".to_owned())
        );
        assert_eq!(
            normalize(Some("/opt/claude///"), "/Users/dev", style()),
            Some("/opt/claude".to_owned())
        );
        assert_eq!(
            normalize(Some("~/.claude-work"), "/Users/dev", style()),
            Some("/Users/dev/.claude-work".to_owned())
        );
        assert_eq!(
            normalize(Some("~"), "/Users/dev", style()),
            Some("/Users/dev".to_owned())
        );
    }

    #[test]
    fn a_variable_that_cannot_be_a_config_dir_is_not_used() {
        // A typo must fall through to auto-detect, not be acted on.
        assert_eq!(normalize(Some(""), "/Users/dev", style()), None);
        assert_eq!(normalize(Some("   "), "/Users/dev", style()), None);
        assert_eq!(normalize(None, "/Users/dev", style()), None);
        // Relative: would resolve against the app's cwd.
        assert_eq!(normalize(Some(".claude"), "/Users/dev", style()), None);
        assert_eq!(normalize(Some("./claude"), "/Users/dev", style()), None);
        // A bare root would yield "//projects".
        assert_eq!(normalize(Some("/"), "/Users/dev", style()), None);
        assert_eq!(normalize(Some("//"), "/Users/dev", style()), None);
    }

    #[test]
    fn windows_paths_are_absolute_without_a_leading_slash() {
        let windows = PathStyle::Windows;
        assert_eq!(
            normalize(Some(r"C:\Users\dev\.claude-work"), r"C:\Users\dev", windows),
            Some(r"C:\Users\dev\.claude-work".to_owned())
        );
        assert_eq!(
            normalize(
                Some(r"C:\Users\dev\.claude-work\"),
                r"C:\Users\dev",
                windows
            ),
            Some(r"C:\Users\dev\.claude-work".to_owned())
        );
        // Mixed separators are what a hand-typed variable looks like.
        assert_eq!(
            normalize(Some(r"C:/Users/dev\.claude"), r"C:\Users\dev", windows),
            Some(r"C:/Users/dev\.claude".to_owned())
        );
        assert_eq!(
            normalize(Some(r"~\.claude-work"), r"C:\Users\dev", windows),
            Some(r"C:\Users\dev\.claude-work".to_owned())
        );
        assert_eq!(
            normalize(Some(r"\\nas\claude\"), r"C:\Users\dev", windows),
            Some(r"\\nas\claude".to_owned())
        );
        // The roots themselves stay roots.
        assert_eq!(normalize(Some(r"C:\"), r"C:\Users\dev", windows), None);
        assert_eq!(
            normalize(Some(r"C:\Users\dev"), r"C:\Users\dev", windows),
            Some(r"C:\Users\dev".to_owned())
        );
        // Still not absolute on Windows: no drive, no UNC.
        assert_eq!(normalize(Some(r"claude"), r"C:\Users\dev", windows), None);
        assert_eq!(normalize(Some(r"\claude"), r"C:\Users\dev", windows), None);
    }

    #[test]
    fn a_posix_style_reads_a_drive_letter_as_relative() {
        // The same input on the other host is not a path, and must not be
        // treated as one — that is how a Windows-shaped config leaks into Linux.
        assert_eq!(
            normalize(Some(r"C:\Users\dev\.claude"), "/home/dev", style()),
            None
        );
    }

    #[test]
    fn a_directory_is_live_only_when_projects_is_a_directory() {
        let live = |path: &str| -> EntryKind {
            if path == "/home/dev/.claude/projects" {
                EntryKind::Directory
            } else {
                EntryKind::Missing
            }
        };
        assert!(is_live_config_dir("/home/dev/.claude", style(), live));
        assert!(!is_live_config_dir("/home/dev/other", style(), live));

        // A stray regular file named `projects` is not evidence.
        let stray = |path: &str| -> EntryKind {
            if path == "/home/dev/.claude/projects" {
                EntryKind::File
            } else {
                EntryKind::Missing
            }
        };
        assert!(!is_live_config_dir("/home/dev/.claude", style(), stray));
    }

    #[test]
    fn the_preference_beats_the_environment_which_beats_the_ladder() {
        let live_xdg = |path: &str| -> EntryKind {
            if path == "/home/dev/.config/claude-code/projects" {
                EntryKind::Directory
            } else {
                EntryKind::Missing
            }
        };
        assert_eq!(
            resolve(
                Some("/opt/pref"),
                Some("/opt/env"),
                "/home/dev",
                style(),
                live_xdg
            ),
            "/opt/pref"
        );
        assert_eq!(
            resolve(None, Some("/opt/env"), "/home/dev", style(), live_xdg),
            "/opt/env"
        );
        assert_eq!(
            resolve(None, None, "/home/dev", style(), live_xdg),
            "/home/dev/.config/claude-code"
        );
    }

    /// The bug this closes: `~/.claude` holding only `settings.json` — which our
    /// own installer wrote — must not outrank a real config dir.
    #[test]
    fn a_dot_claude_with_only_a_settings_file_is_not_live() {
        let ours_only = |path: &str| -> EntryKind {
            match path {
                "/home/dev/.claude/settings.json" => EntryKind::File,
                "/home/dev/.config/claude/projects" => EntryKind::Directory,
                _ => EntryKind::Missing,
            }
        };
        assert_eq!(
            resolve(None, None, "/home/dev", style(), ours_only),
            "/home/dev/.config/claude",
            "our own hook file must not become the reason we pick that folder"
        );
    }

    #[test]
    fn a_live_dot_claude_wins_over_the_xdg_probe() {
        let both = |path: &str| -> EntryKind {
            if path == "/home/dev/.claude/projects"
                || path == "/home/dev/.config/claude-code/projects"
            {
                EntryKind::Directory
            } else {
                EntryKind::Missing
            }
        };
        assert_eq!(
            resolve(None, None, "/home/dev", style(), both),
            "/home/dev/.claude"
        );
    }

    #[test]
    fn nothing_live_ends_at_the_default_that_the_installer_creates() {
        assert_eq!(
            resolve(None, None, "/home/dev", style(), |_| EntryKind::Missing),
            "/home/dev/.claude"
        );
    }

    /// `claude` is probed before `claude-code`, because that is the spelling
    /// ccusage and its followers use today.
    #[test]
    fn the_newer_xdg_spelling_wins_when_both_exist() {
        let both = |path: &str| -> EntryKind {
            if path == "/home/dev/.config/claude/projects"
                || path == "/home/dev/.config/claude-code/projects"
            {
                EntryKind::Directory
            } else {
                EntryKind::Missing
            }
        };
        assert_eq!(
            resolve(None, None, "/home/dev", style(), both),
            "/home/dev/.config/claude"
        );
    }

    #[test]
    fn the_ladder_reads_windows_paths_on_windows() {
        let windows = PathStyle::Windows;
        let live = |path: &str| -> EntryKind {
            if path == r"C:\Users\dev\.config\claude\projects" {
                EntryKind::Directory
            } else {
                EntryKind::Missing
            }
        };
        assert_eq!(
            resolve(None, None, r"C:\Users\dev", windows, live),
            r"C:\Users\dev\.config\claude"
        );
        assert_eq!(
            resolve(None, None, r"C:\Users\dev", windows, |_| EntryKind::Missing),
            r"C:\Users\dev\.claude"
        );
    }

    #[test]
    fn roots_keeps_the_first_spelling_of_a_duplicate() {
        // A real `identity` (realpath + case folding) also drops the trailing
        // separator; `roots` only trusts what it is handed, which is why the
        // two spellings below collapse.
        let identity = |path: &str| path.trim_end_matches('/').to_lowercase();
        assert_eq!(
            roots(
                "/home/dev/.claude",
                &["/home/dev/.Claude", "/opt/work", "/opt/work/"],
                identity
            ),
            vec!["/home/dev/.claude".to_owned(), "/opt/work".to_owned()]
        );
    }

    #[test]
    fn a_process_root_is_unknown_when_its_environment_is_unreadable() {
        let mut environment = HashMap::new();
        environment.insert("CLAUDE_CONFIG_DIR".to_owned(), "/opt/work".to_owned());

        assert_eq!(
            process_root(
                ConfigDirCli::Claude,
                None,
                "/home/dev",
                "/home/dev/.claude",
                style()
            ),
            None
        );
        assert_eq!(
            process_root(
                ConfigDirCli::Claude,
                Some(&environment),
                "/home/dev",
                "/home/dev/.claude",
                style()
            ),
            Some("/opt/work".to_owned())
        );
        // Blank is "unset": the CLI's own default.
        let mut blank = HashMap::new();
        blank.insert("CODEX_HOME".to_owned(), "  ".to_owned());
        assert_eq!(
            process_root(
                ConfigDirCli::Codex,
                Some(&blank),
                "/home/dev",
                "/home/dev/.codex",
                style()
            ),
            Some("/home/dev/.codex".to_owned())
        );
        // A relative value is unknown, not "unset".
        let mut relative = HashMap::new();
        relative.insert("GROK_HOME".to_owned(), "grok".to_owned());
        assert_eq!(
            process_root(
                ConfigDirCli::Grok,
                Some(&relative),
                "/home/dev",
                "/home/dev/.grok",
                style()
            ),
            None
        );
    }

    #[test]
    fn the_owning_root_is_the_deepest_match() {
        let identity = |path: &str| path.to_owned();
        let roots = vec![
            "/home/dev/.claude".to_owned(),
            "/home/dev/.claude/work".to_owned(),
        ];
        assert_eq!(
            owning_root(
                Some("/home/dev/.claude/work/projects/abc.jsonl"),
                &roots,
                style(),
                identity
            ),
            Some("/home/dev/.claude/work".to_owned())
        );
        assert_eq!(
            owning_root(
                Some("/home/dev/.claude/projects/abc.jsonl"),
                &roots,
                style(),
                identity
            ),
            Some("/home/dev/.claude".to_owned())
        );
        assert_eq!(
            owning_root(Some("/tmp/abc.jsonl"), &roots, style(), identity),
            None
        );
        assert_eq!(owning_root(None, &roots, style(), identity), None);
        assert_eq!(owning_root(Some(""), &roots, style(), identity), None);
    }
}
