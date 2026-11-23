//! Backfill: the sessions that exist before their hook does (#261).
//!
//! Until this, a session could only be born from a hook. Restart the app while an
//! agent is working and the panel is empty until that agent's next event — and a
//! session whose hook never carried a pid is never pruned, so its card outlives
//! the process.
//!
//! The scan itself is `morrow_core::discovery::scan` (#260): roots × readers,
//! deduped, within a budget. What lives here is the one reader that reads
//! something real today — Claude Code's `projects/` tree — and the decision of
//! when to run it.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use morrow_core::agent_process::ProcessInfo;
use morrow_core::discovery::scan::{self, ProcessReader, ScanResult, StoreReader};
use morrow_core::discovery::{DiscoveryHit, read_transcript_tail, transcript_files};
use morrow_core::reduce::ChatMessage;

/// How much of a transcript the backfill reads. The same slice the live tailer
/// reads on attach, so backfill costs one read of bytes that are already going to
/// be read rather than a second parse of the whole store.
const TAIL_BYTES: u64 = 64 * 1024;

/// Claude Code's `projects/<encoded>/<session>.jsonl`.
///
/// Two levels, not a recursive walk: `projects/` holds one directory per working
/// directory and the transcripts directly inside it. `Discovery/` on macOS only
/// ever lists one directory for the same reason.
pub struct ClaudeTranscripts {
    /// Claude Code's config directory, resolved by the installer's own ladder
    /// (#258) so discovery reads the folder the hooks were written into.
    pub config_dir: String,
}

impl ClaudeTranscripts {
    fn config_dir(&self) -> &Path {
        Path::new(&self.config_dir)
    }
}

impl StoreReader for ClaudeTranscripts {
    fn source(&self) -> &'static str {
        "claude"
    }

    /// One store per config root. Declared by the walker that found them, so this
    /// reader does not go looking for accounts it was not told about.
    fn roots(&self) -> Vec<String> {
        claude_roots(self.config_dir())
    }

    fn read(&self, root: &str) -> Vec<DiscoveryHit> {
        let projects = Path::new(root).join("projects");
        let Ok(entries) = std::fs::read_dir(&projects) else {
            return Vec::new();
        };
        let mut hits = Vec::new();
        for entry in entries.flatten() {
            // `entry.path()` is the encoded project directory. Entries are not
            // resolved: a symlinked transcript keeps its own mtime, which is what
            // tells a live session from one left behind (#256).
            let Some(files) = transcript_files::list(&entry.path().to_string_lossy()) else {
                continue;
            };
            for file in files {
                let Some(tail) = read_tail(Path::new(&file.path)) else {
                    continue;
                };
                let Some(summary) = read_transcript_tail(&tail, &file.name) else {
                    continue;
                };
                hits.push(DiscoveryHit {
                    session_id: summary.session_id,
                    source: "claude",
                    cwd: summary.cwd,
                    tty: None,
                    model: summary.model,
                    pid: None,
                    modified_at: Some(file.modified_ms),
                    recent_messages: summary.recent_messages,
                    transcript_path: Some(file.path.clone()),
                });
            }
        }
        hits
    }
}

/// The last `TAIL_BYTES` of a file as text. A partial UTF-8 sequence at the cut
/// is dropped rather than failing the whole read.
fn read_tail(path: &Path) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let size = file.metadata().ok()?.len();
    let start = size.saturating_sub(TAIL_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::new();
    file.take(TAIL_BYTES).read_to_end(&mut bytes).ok()?;
    if start > 0 {
        // Skip to the first newline: the row at the cut is half a line, and the
        // reader tolerates it, but dropping it keeps a cut multi-byte char from
        // making the whole tail lossy.
        if let Some(index) = bytes.iter().position(|byte| *byte == b'\n') {
            bytes.drain(..=index);
        }
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// OpenCode's (and MiMo Code's) session database (#270).
///
/// The one reader that can be backfilled **without a process**, because the store
/// itself records the working directory: `find_recent_session` asks "which session
/// is this process in", which needs a cwd from the process table, while
/// `recent_sessions` asks the database what it holds.
///
/// Cursor and Gemini are the other way round and are deliberately **not**
/// registered: their store directory is derived from a process's cwd
/// (`~/.cursor/projects/<encoded cwd>/agent-transcripts`), and that encoding maps
/// `/` and a space to the same `-`. Guessing a cwd from it would put a card in a
/// directory the user does not have.
pub struct OpenCodeStore {
    /// `~/.local/share`, where both builds keep their database.
    pub data_dir: String,
}

impl OpenCodeStore {
    /// `opencode.db` and `mimocode.db`: the two names the builds ship (#257).
    fn databases(&self) -> Vec<PathBuf> {
        ["opencode/opencode.db", "mimocode/mimocode.db"]
            .iter()
            .map(|relative| Path::new(&self.data_dir).join(relative))
            .filter(|path| path.is_file())
            .collect()
    }
}

impl StoreReader for OpenCodeStore {
    fn source(&self) -> &'static str {
        "opencode"
    }

    /// One root, not one per account: a single database holds every session.
    fn roots(&self) -> Vec<String> {
        let databases = self.databases();
        if databases.is_empty() {
            return Vec::new();
        }
        vec![self.data_dir.clone()]
    }

    fn read(&self, _root: &str) -> Vec<DiscoveryHit> {
        let mut hits = Vec::new();
        for database in self.databases() {
            let Ok(connection) = morrow_core::sqlite::open_read_only(&database) else {
                // A store being read by the CLI is not a failure of discovery.
                continue;
            };
            for row in morrow_core::discovery::opencode::recent_sessions(&connection, 20) {
                let (model, recent_messages) =
                    morrow_core::discovery::opencode::read_recent(&connection, &row.session_id);
                hits.push(DiscoveryHit {
                    session_id: row.session_id,
                    source: "opencode",
                    cwd: row.cwd,
                    tty: None,
                    model,
                    pid: None,
                    // `time_updated` is milliseconds, the same unit a hit wants.
                    modified_at: u64::try_from(row.modified_ms).ok(),
                    recent_messages,
                    transcript_path: Some(database.to_string_lossy().into_owned()),
                });
            }
        }
        hits
    }
}

/// Codex's rollouts: `~/.codex/sessions/<YYYY>/<MM>/<DD>/*.jsonl` (#272).
///
/// Store-first like OpenCode — the rollout carries its thread id, cwd and model —
/// so a Codex session appears without a hook and without a process.
///
/// Only the **last seven days** of directories are listed, which is the window the
/// Swift reader uses. A `sessions/` tree grows forever, and a backfill that walked
/// all of it would read years of rollouts on every attach; the panel shows current
/// work, and anything older arrives through a hook or not at all.
///
/// The window is anchored to `now_ms`, not to however many day-directories the
/// tree happens to hold: seven directories left over from January must not be
/// read in November.
pub struct CodexRollouts {
    /// The Codex config root: `$CODEX_HOME`, else `~/.codex`.
    pub root: String,
    /// The clock for the seven-day window, in milliseconds since the Unix epoch.
    pub now_ms: u64,
}

/// How many days back a backfill looks.
pub const CODEX_DAYS: u64 = 7;

/// The UTC civil date for a count of days since the Unix epoch.
///
/// Howard Hinnant's `civil_from_days`. The window is expressed in whole days, so
/// a date crate would be a dependency for arithmetic this small.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

impl CodexRollouts {
    /// `<root>/sessions/<yyyy>/<mm>/<dd>` for the seven days ending at `now_ms`,
    /// oldest first, for the directories that exist.
    ///
    /// This mirrors `findRecentCodexSessions` in Swift, which asks for the dates
    /// `0..<7` days back from `Date()`. Walking the tree and taking its seven
    /// newest names would instead read a stale tree forever, and never let a day
    /// outside it expire. Building the paths directly also keeps the walk bounded
    /// to seven fixed paths — a symlinked loop cannot be entered.
    fn day_directories_at(&self, now_ms: u64) -> Vec<PathBuf> {
        let today = (now_ms / 86_400_000) as i64;
        (0..CODEX_DAYS)
            .rev()
            .filter_map(|back| {
                let (year, month, day) = civil_from_days(today - back as i64);
                let path = Path::new(&self.root)
                    .join("sessions")
                    .join(format!("{year:04}"))
                    .join(format!("{month:02}"))
                    .join(format!("{day:02}"));
                path.is_dir().then_some(path)
            })
            .collect()
    }
}

impl StoreReader for CodexRollouts {
    fn source(&self) -> &'static str {
        "codex"
    }

    fn roots(&self) -> Vec<String> {
        Path::new(&self.root)
            .join("sessions")
            .is_dir()
            .then(|| self.root.clone())
            .into_iter()
            .collect()
    }

    fn read(&self, _root: &str) -> Vec<DiscoveryHit> {
        let mut hits = Vec::new();
        for day in self.day_directories_at(self.now_ms) {
            let Some(files) = transcript_files::list(&day.to_string_lossy()) else {
                continue;
            };
            for file in files {
                let Some(tail) = read_tail(Path::new(&file.path)) else {
                    continue;
                };
                let Some(summary) =
                    morrow_core::discovery::read_codex_rollout_tail(&tail, &file.name)
                else {
                    continue;
                };
                hits.push(DiscoveryHit {
                    session_id: summary.session_id,
                    source: "codex",
                    cwd: summary.cwd,
                    tty: None,
                    model: summary.model,
                    pid: None,
                    modified_at: Some(file.modified_ms),
                    recent_messages: summary.recent_messages,
                    transcript_path: Some(file.path.clone()),
                });
            }
        }
        hits
    }
}

/// Where VS Code keeps extension storage, per OS.
///
/// Cline is an extension inside VS Code, not a CLI, so its store lives under
/// VS Code's own per-user directory: `~/Library/Application Support` on macOS,
/// `~/.config` on Linux, `%APPDATA%` on Windows.
fn vscode_user_dir(home: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support/Code/User")
    }
    #[cfg(target_os = "windows")]
    {
        // `home` is the roaming profile; VS Code writes to Roaming, not Local.
        home.join("AppData/Roaming/Code/User")
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        home.join(".config/Code/User")
    }
}

/// Cline (#278), found from its own task history.
///
/// The one reader of this batch with no process: Cline runs inside VS Code, so
/// macOS gives it no pid either. Its task history records the cwd, which is what
/// makes a backfill possible with nothing running.
pub struct ClineStore {
    pub root: String,
    pub now_ms: u64,
}

/// Cline allows more staleness than a CLI: ten minutes, because an extension can
/// sit idle in a window without the editor going away.
pub const CLINE_FRESHNESS_MS: u64 = 600_000;

impl StoreReader for ClineStore {
    fn source(&self) -> &'static str {
        "cline"
    }

    fn roots(&self) -> Vec<String> {
        vec![self.root.clone()]
    }

    fn read(&self, root: &str) -> Vec<DiscoveryHit> {
        let Some(history) = std::fs::read_to_string(Path::new(root).join("state/taskHistory.json"))
            .ok()
            .and_then(|json| morrow_core::discovery::cline::latest_task(&json))
        else {
            return Vec::new();
        };
        let conversation = Path::new(root)
            .join("tasks")
            .join(&history.id)
            .join("api_conversation_history.json");
        // The conversation file's mtime beats the task's timestamp: Cline refreshes
        // the history lazily, so `ts` is the time the task was *created*.
        let modified_ms = transcript_files::mtime_ms(&conversation.to_string_lossy())
            .or_else(|| history.ts_ms.map(|ts| ts.max(0.0) as u64));
        let Some(modified_ms) = modified_ms else {
            return Vec::new();
        };
        if self.now_ms.saturating_sub(modified_ms) > CLINE_FRESHNESS_MS {
            return Vec::new();
        }
        let messages = std::fs::read_to_string(&conversation)
            .map(|json| morrow_core::discovery::cline::recent_messages(&json))
            .unwrap_or_default();
        // No cwd means the task never ran anywhere this card could point at.
        let Some(cwd) = history.cwd else {
            return Vec::new();
        };
        vec![DiscoveryHit {
            session_id: history.id,
            source: "cline",
            cwd,
            tty: None,
            model: history.model,
            // No pid: Cline is an extension, not a process.
            pid: None,
            modified_at: Some(modified_ms),
            recent_messages: messages,
            transcript_path: Some(conversation.to_string_lossy().into_owned()),
        }]
    }
}

/// One flat store reader: a base directory, a cwd encoder, and a transcript
/// flavour (#278).
///
/// Qoder, CodeBuddy and Factory differ in exactly these three things, so they are
/// one reader with a table rather than three near-copies.
pub struct FlatStore {
    pub home: String,
    pub source: &'static str,
    pub now_ms: u64,
}

/// Five minutes, as macOS uses for every process-first reader.
pub const FLAT_STORE_FRESHNESS_MS: u64 = 300_000;

impl FlatStore {
    /// Base directories, in precedence order. Qoder has two: the international
    /// build writes `~/.qoder`, the China build `~/.qoder-cn`.
    fn bases(&self) -> Vec<PathBuf> {
        let home = Path::new(&self.home);
        match self.source {
            "qoder" => vec![
                home.join(".qoder/projects"),
                home.join(".qoder-cn/projects"),
            ],
            "codebuddy" => vec![home.join(".codebuddy/projects")],
            // Factory's directory is `~/.factory`, its executable `droid`.
            "droid" => vec![home.join(".factory/sessions")],
            _ => Vec::new(),
        }
    }

    fn encode(&self, cwd: &str) -> String {
        match self.source {
            // CodeBuddy spells the project directory the way its own app does;
            // Qoder and Factory use Claude Code's encoding.
            "codebuddy" => morrow_core::discovery::project_dir::app_encoded(cwd),
            _ => morrow_core::discovery::project_dir::encoded(cwd),
        }
    }

    /// `(model, messages)` from this agent's flavour of a `.jsonl` transcript.
    fn recent(&self, path: &Path) -> (Option<String>, Vec<ChatMessage>) {
        let Some(tail) = read_tail(path) else {
            return (None, Vec::new());
        };
        match self.source {
            "codebuddy" => morrow_core::discovery::flat_store::codebuddy_recent(&tail),
            "droid" => {
                // Factory keeps the model in a sidecar: `x.jsonl` → `x.settings.json`.
                let settings = path.to_string_lossy().replace(".jsonl", ".settings.json");
                let model = std::fs::read_to_string(&settings).ok().and_then(|json| {
                    morrow_core::discovery::flat_store::factory_model_from_settings(&json)
                });
                let summary = morrow_core::discovery::read_transcript_tail(
                    &tail,
                    &path.file_name().unwrap_or_default().to_string_lossy(),
                );
                (
                    model.or_else(|| summary.as_ref().and_then(|s| s.model.clone())),
                    summary.map(|s| s.recent_messages).unwrap_or_default(),
                )
            }
            // Qoder writes Claude Code's own format.
            _ => {
                let summary = morrow_core::discovery::read_transcript_tail(
                    &tail,
                    &path.file_name().unwrap_or_default().to_string_lossy(),
                );
                (
                    summary.as_ref().and_then(|s| s.model.clone()),
                    summary.map(|s| s.recent_messages).unwrap_or_default(),
                )
            }
        }
    }
}

impl StoreReader for FlatStore {
    fn source(&self) -> &'static str {
        self.source
    }

    fn roots(&self) -> Vec<String> {
        Vec::new()
    }

    fn read(&self, _root: &str) -> Vec<DiscoveryHit> {
        Vec::new()
    }
}

impl ProcessReader for FlatStore {
    fn source(&self) -> &'static str {
        self.source
    }

    fn read_process(&self, process: &ProcessInfo) -> Vec<DiscoveryHit> {
        let Some(cwd) = process.cwd.as_deref().filter(|cwd| !cwd.is_empty()) else {
            return Vec::new();
        };
        // Every process in a subagent worktree would otherwise claim the same
        // parent project; macOS skips them and so does this.
        if is_subagent_worktree(cwd) {
            return Vec::new();
        }
        let project = self.encode(cwd);
        // Only the newest file in the project is the session being run.
        let best = self
            .bases()
            .into_iter()
            .filter_map(|base| {
                let files = transcript_files::list(&base.join(&project).to_string_lossy())?;
                transcript_files::most_recent(
                    &files,
                    Some(self.now_ms.saturating_sub(FLAT_STORE_FRESHNESS_MS)),
                )
                .cloned()
            })
            .max_by_key(|file| file.modified_ms);
        let Some(file) = best else {
            return Vec::new();
        };
        let (model, recent_messages) = self.recent(Path::new(&file.path));
        vec![DiscoveryHit {
            session_id: file.name.trim_end_matches(".jsonl").to_owned(),
            source: self.source,
            cwd: cwd.to_owned(),
            tty: None,
            model,
            pid: Some(process.pid),
            modified_at: Some(file.modified_ms),
            recent_messages,
            transcript_path: Some(file.path),
        }]
    }
}

/// GitHub Copilot CLI (#279), found from a process.
///
/// `~/.copilot/session-state/<id>/events.jsonl`. The store directory is named by
/// the session, not by the cwd — so the cwd has to be read **out of the event
/// stream**, and the candidate list is walked newest-first until one matches.
pub struct CopilotSessions {
    pub home: String,
    pub now_ms: u64,
}

/// How many of the newest event streams are tried before giving up. Copilot keeps
/// one directory per session forever, and a machine with a year of them would
/// otherwise walk thousands to find one.
pub const COPILOT_CANDIDATE_LIMIT: usize = 50;

/// Five minutes, as macOS uses.
pub const COPILOT_FRESHNESS_MS: u64 = 300_000;

impl StoreReader for CopilotSessions {
    fn source(&self) -> &'static str {
        "copilot"
    }

    fn roots(&self) -> Vec<String> {
        Vec::new()
    }

    fn read(&self, _root: &str) -> Vec<DiscoveryHit> {
        Vec::new()
    }
}

impl CopilotSessions {
    fn base(&self) -> PathBuf {
        Path::new(&self.home).join(".copilot/session-state")
    }

    /// The newest event stream that names `cwd`, and started no earlier than the
    /// process did.
    fn session_for(&self, cwd: &str, started_ms: Option<u64>) -> Option<(String, String, u64)> {
        // One directory per session, each holding its own `events.jsonl` — the
        // store is named by session, not by cwd.
        let base = self.base();
        let mut files: Vec<(String, u64)> = std::fs::read_dir(&base)
            .ok()?
            .flatten()
            .filter_map(|entry| {
                let path = entry.path().join("events.jsonl");
                let modified = transcript_files::mtime_ms(&path.to_string_lossy())?;
                Some((path.to_string_lossy().into_owned(), modified))
            })
            .collect();
        files.sort_by_key(|file| std::cmp::Reverse(file.1));
        for (path, modified_ms) in files.into_iter().take(COPILOT_CANDIDATE_LIMIT) {
            // Ten seconds of slack: a session writes its first event after the
            // process has already been recorded as started.
            if let Some(started_ms) = started_ms {
                if modified_ms.saturating_add(10_000) < started_ms {
                    continue;
                }
            }
            let Some(head) = read_head(Path::new(&path), 32_768) else {
                continue;
            };
            if !morrow_core::discovery::copilot::matches_cwd(&head, cwd) {
                continue;
            }
            let Some(session_id) = Path::new(&path)
                .parent()
                .and_then(|directory| directory.file_name())
                .map(|name| name.to_string_lossy().into_owned())
            else {
                continue;
            };
            return Some((session_id, path, modified_ms));
        }
        None
    }
}

impl ProcessReader for CopilotSessions {
    fn source(&self) -> &'static str {
        "copilot"
    }

    fn read_process(&self, process: &ProcessInfo) -> Vec<DiscoveryHit> {
        let Some(cwd) = process.cwd.as_deref().filter(|cwd| !cwd.is_empty()) else {
            return Vec::new();
        };
        if is_subagent_worktree(cwd) {
            return Vec::new();
        }
        let Some((session_id, path, modified_ms)) = self.session_for(cwd, process.start_time_ms)
        else {
            return Vec::new();
        };
        if self.now_ms.saturating_sub(modified_ms) > COPILOT_FRESHNESS_MS {
            return Vec::new();
        }
        let (model, recent_messages) = read_tail(Path::new(&path))
            .map(|tail| morrow_core::discovery::copilot::recent_messages(&tail))
            .unwrap_or_default();
        vec![DiscoveryHit {
            session_id,
            source: "copilot",
            cwd: cwd.to_owned(),
            tty: None,
            model,
            pid: Some(process.pid),
            modified_at: Some(modified_ms),
            recent_messages,
            transcript_path: Some(path),
        }]
    }
}

/// Grok CLI (#279), found from a process.
///
/// `$GROK_HOME/sessions/<percent-encoded cwd>/<session dir>/`, where the summary
/// names the cwd, the model and the last activity. Grok's
/// `session_search.sqlite` is **not** read: it indexes sessions whose transcript
/// is gone or absent, and a card with no transcript is a card that cannot be
/// shown — every row it adds would be empty.
pub struct GrokSessions {
    pub home: String,
    pub now_ms: u64,
}

/// Five minutes, as macOS uses.
pub const GROK_FRESHNESS_MS: u64 = 300_000;

impl StoreReader for GrokSessions {
    fn source(&self) -> &'static str {
        "grok"
    }

    fn roots(&self) -> Vec<String> {
        Vec::new()
    }

    fn read(&self, _root: &str) -> Vec<DiscoveryHit> {
        Vec::new()
    }
}

impl GrokSessions {
    /// `GROK_HOME` first: a second account keeps its own store, and reading only
    /// `~/.grok` would show the wrong account's sessions.
    fn homes(&self) -> Vec<PathBuf> {
        match std::env::var("GROK_HOME") {
            Ok(root) if !root.trim().is_empty() => vec![PathBuf::from(root.trim())],
            _ => vec![Path::new(&self.home).join(".grok")],
        }
    }
}

impl ProcessReader for GrokSessions {
    fn source(&self) -> &'static str {
        "grok"
    }

    fn read_process(&self, process: &ProcessInfo) -> Vec<DiscoveryHit> {
        let Some(cwd) = process.cwd.as_deref().filter(|cwd| !cwd.is_empty()) else {
            return Vec::new();
        };
        if is_subagent_worktree(cwd) {
            return Vec::new();
        }
        let encoded = morrow_core::discovery::grok::encoded_cwd(cwd);
        let mut best: Option<(String, String, String, u64, Option<String>)> = None;
        for home in self.homes() {
            let cwd_directory = home.join("sessions").join(&encoded);
            let Ok(entries) = std::fs::read_dir(&cwd_directory) else {
                continue;
            };
            for entry in entries.flatten().filter(|entry| entry.path().is_dir()) {
                let directory = entry.path();
                let Ok(summary_json) = std::fs::read_to_string(directory.join("summary.json"))
                else {
                    continue;
                };
                let Some(summary) = morrow_core::discovery::grok::summary_metadata(&summary_json)
                else {
                    continue;
                };
                // The directory is keyed by cwd, but the summary says so itself:
                // trusting the key alone would take a session whose cwd moved.
                if summary.cwd.as_deref() != Some(cwd) {
                    continue;
                }
                let session_id = summary
                    .session_id
                    .clone()
                    .or_else(|| {
                        entry
                            .file_name()
                            .to_str()
                            .filter(|name| !name.is_empty())
                            .map(str::to_owned)
                    })
                    .unwrap_or_default();
                if session_id.is_empty() {
                    continue;
                }
                let Some(activity_ms) = summary.activity_at_ms else {
                    continue;
                };
                if let Some(started_ms) = process.start_time_ms {
                    if activity_ms < started_ms.saturating_sub(10_000) {
                        continue;
                    }
                }
                let chat = directory.join("chat_history.jsonl");
                let chat_path = chat.is_file().then(|| chat.to_string_lossy().into_owned());
                match &best {
                    Some((_, _, _, best_ms, _)) if *best_ms >= activity_ms => {}
                    _ => {
                        best = Some((
                            session_id,
                            cwd.to_owned(),
                            chat_path.clone().unwrap_or_default(),
                            activity_ms,
                            summary.model,
                        ))
                    }
                }
            }
        }
        let Some((session_id, _, chat_path, activity_ms, model)) = best else {
            return Vec::new();
        };
        if self.now_ms.saturating_sub(activity_ms) > GROK_FRESHNESS_MS {
            return Vec::new();
        }
        // An index-only session has no transcript once chat_history is missing;
        // there is nothing to tail and nothing to show.
        let Some(chat_path) = Some(chat_path).filter(|path| !path.is_empty()) else {
            return Vec::new();
        };
        let recent_messages = read_tail(Path::new(&chat_path))
            .and_then(|tail| {
                let name = Path::new(&chat_path)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                morrow_core::discovery::read_transcript_tail(&tail, &name)
            })
            .map(|summary| summary.recent_messages)
            .unwrap_or_default();
        vec![DiscoveryHit {
            session_id,
            source: "grok",
            cwd: cwd.to_owned(),
            tty: None,
            model,
            pid: Some(process.pid),
            modified_at: Some(activity_ms),
            recent_messages,
            transcript_path: Some(chat_path),
        }]
    }
}

/// Kimi (#282), found from a process.
///
/// Two layouts, and the modern one is preferred because it is a **map**: kimi-code
/// writes `~/.kimi-code/session_index.jsonl` with `{sessionId, sessionDir, workDir}`,
/// so the session is looked up by cwd. Legacy kimi-cli instead names the directory
/// `md5(cwd)`, which is the only reason this port carries a hash at all.
pub struct KimiSessions {
    pub home: String,
    pub now_ms: u64,
}

/// Five minutes, as macOS uses.
pub const KIMI_FRESHNESS_MS: u64 = 300_000;

impl StoreReader for KimiSessions {
    fn source(&self) -> &'static str {
        "kimi"
    }

    fn roots(&self) -> Vec<String> {
        Vec::new()
    }

    fn read(&self, _root: &str) -> Vec<DiscoveryHit> {
        Vec::new()
    }
}

impl KimiSessions {
    /// The indexed session for `cwd`, newest activity first.
    fn indexed_session(&self, cwd: &str) -> Option<DiscoveryHit> {
        let index =
            std::fs::read_to_string(Path::new(&self.home).join(".kimi-code/session_index.jsonl"))
                .ok()?;
        let mut best: Option<(u64, String, String)> = None;
        for entry in morrow_core::discovery::kimi::index_entries(&index) {
            if entry.work_dir != cwd {
                continue;
            }
            // `state.json` moves whenever the session does; the directory's own
            // mtime does not.
            let state = Path::new(&entry.session_dir).join("state.json");
            let stamp = transcript_files::mtime_ms(&state.to_string_lossy())
                .or_else(|| transcript_files::mtime_ms(&entry.session_dir))?;
            if best.as_ref().is_none_or(|(best, _, _)| stamp > *best) {
                best = Some((stamp, entry.session_id, entry.session_dir));
            }
        }
        let (modified_ms, session_id, session_dir) = best?;
        let wire = Path::new(&session_dir).join("agents/main/wire.jsonl");
        let recent_messages = read_tail(&wire)
            .map(|tail| morrow_core::discovery::kimi::recent_messages(&tail))
            .unwrap_or_default();
        Some(DiscoveryHit {
            session_id,
            source: "kimi",
            cwd: cwd.to_owned(),
            tty: None,
            // kimi-code's index records no model.
            model: None,
            pid: None,
            modified_at: Some(modified_ms),
            recent_messages,
            transcript_path: wire.is_file().then(|| wire.to_string_lossy().into_owned()),
        })
    }

    /// Legacy kimi-cli: `~/.kimi-code/sessions/<md5(cwd)>/<id>/wire.jsonl`.
    fn hashed_session(&self, cwd: &str) -> Option<DiscoveryHit> {
        let hash = morrow_core::discovery::kimi::workdir_hash(cwd);
        let mut best: Option<(u64, String, String)> = None;
        for base in [".kimi-code/sessions", ".kimi/sessions"] {
            let workdir = Path::new(&self.home).join(base).join(&hash);
            let Ok(entries) = std::fs::read_dir(&workdir) else {
                continue;
            };
            for entry in entries.flatten().filter(|entry| entry.path().is_dir()) {
                let wire = entry.path().join("wire.jsonl");
                let Some(stamp) = transcript_files::mtime_ms(&wire.to_string_lossy()) else {
                    continue;
                };
                let Some(session_id) = entry.file_name().to_str().map(str::to_owned) else {
                    continue;
                };
                if best.as_ref().is_none_or(|(best, _, _)| stamp > *best) {
                    best = Some((stamp, session_id, wire.to_string_lossy().into_owned()));
                }
            }
        }
        let (modified_ms, session_id, wire) = best?;
        let recent_messages = read_tail(Path::new(&wire))
            .map(|tail| morrow_core::discovery::kimi::recent_messages(&tail))
            .unwrap_or_default();
        Some(DiscoveryHit {
            session_id,
            source: "kimi",
            cwd: cwd.to_owned(),
            tty: None,
            model: None,
            pid: None,
            modified_at: Some(modified_ms),
            recent_messages,
            transcript_path: Some(wire),
        })
    }
}

impl ProcessReader for KimiSessions {
    fn source(&self) -> &'static str {
        "kimi"
    }

    fn read_process(&self, process: &ProcessInfo) -> Vec<DiscoveryHit> {
        let Some(cwd) = process.cwd.as_deref().filter(|cwd| !cwd.is_empty()) else {
            return Vec::new();
        };
        if is_subagent_worktree(cwd) {
            return Vec::new();
        }
        // The index first; the hashed layout only when it has nothing.
        let hit = self
            .indexed_session(cwd)
            .or_else(|| self.hashed_session(cwd));
        let Some(mut hit) = hit else {
            return Vec::new();
        };
        let Some(modified_ms) = hit.modified_at else {
            return Vec::new();
        };
        if self.now_ms.saturating_sub(modified_ms) > KIMI_FRESHNESS_MS {
            return Vec::new();
        }
        hit.pid = Some(process.pid);
        vec![hit]
    }
}

/// Kiro CLI (#282), found from a process.
///
/// Three layouts under `~/.kiro/sessions`: the V3 workspace directories, and the V2
/// `cli/<id>.json`. `session-leases` is **not** read: a lease names a pid and its
/// parent, and following it needs a liveness syscall this port does not have — the
/// kiro process rule already covers `kiro-cli`, so a lease would only add sessions
/// whose process this port cannot see anyway.
pub struct KiroSessions {
    pub home: String,
}

/// Five minutes, as macOS uses.
pub const KIRO_FRESHNESS_MS: u64 = 300_000;

impl StoreReader for KiroSessions {
    fn source(&self) -> &'static str {
        "kiro"
    }

    fn roots(&self) -> Vec<String> {
        Vec::new()
    }

    fn read(&self, _root: &str) -> Vec<DiscoveryHit> {
        Vec::new()
    }
}

impl KiroSessions {
    fn root(&self) -> PathBuf {
        Path::new(&self.home).join(".kiro")
    }
}

impl ProcessReader for KiroSessions {
    fn source(&self) -> &'static str {
        "kiro"
    }

    fn read_process(&self, process: &ProcessInfo) -> Vec<DiscoveryHit> {
        let Some(cwd) = process.cwd.as_deref().filter(|cwd| !cwd.is_empty()) else {
            return Vec::new();
        };
        if is_subagent_worktree(cwd) {
            return Vec::new();
        }
        let sessions = self.root().join("sessions");
        let mut best: Option<DiscoveryHit> = None;

        // V3: sessions/<workspace>/<id>/session.json
        if let Ok(workspaces) = std::fs::read_dir(&sessions) {
            for workspace in workspaces.flatten().filter(|entry| entry.path().is_dir()) {
                // `cli` is the V2 directory, not a workspace.
                if workspace.file_name().to_str() == Some("cli") {
                    continue;
                }
                let Ok(children) = std::fs::read_dir(workspace.path()) else {
                    continue;
                };
                for child in children.flatten().filter(|entry| entry.path().is_dir()) {
                    let session_json = child.path().join("session.json");
                    let Ok(json) = std::fs::read_to_string(&session_json) else {
                        continue;
                    };
                    let Some(metadata) = morrow_core::discovery::kiro::session_metadata(&json)
                    else {
                        continue;
                    };
                    if metadata.cwd.as_deref() != Some(cwd) {
                        continue;
                    }
                    let modified_ms = metadata
                        .activity_at_ms
                        .or_else(|| transcript_files::mtime_ms(&session_json.to_string_lossy()));
                    let Some(modified_ms) = modified_ms else {
                        continue;
                    };
                    if let Some(started_ms) = process.start_time_ms {
                        // A session that last moved long before the process began is
                        // another session in the same workspace.
                        if modified_ms.saturating_add(KIRO_FRESHNESS_MS) < started_ms {
                            continue;
                        }
                    }
                    let messages = child.path().join("messages.jsonl");
                    let (model, recent_messages) = read_kiro_messages(&messages);
                    let session_id = metadata
                        .session_id
                        .or_else(|| child.file_name().to_str().map(str::to_owned))
                        .unwrap_or_default();
                    if session_id.is_empty() {
                        continue;
                    }
                    let hit = DiscoveryHit {
                        session_id,
                        source: "kiro",
                        cwd: cwd.to_owned(),
                        tty: None,
                        model: metadata.model.or(model),
                        pid: Some(process.pid),
                        modified_at: Some(modified_ms),
                        recent_messages,
                        transcript_path: messages
                            .is_file()
                            .then(|| messages.to_string_lossy().into_owned()),
                    };
                    if best
                        .as_ref()
                        .is_none_or(|best| best.modified_at.unwrap_or(0) < modified_ms)
                    {
                        best = Some(hit);
                    }
                }
            }
        }
        if best.is_some() {
            return best.into_iter().collect();
        }

        // V2: sessions/cli/<id>.json, with the transcript beside it.
        if let Ok(entries) = std::fs::read_dir(sessions.join("cli")) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                    continue;
                }
                let Ok(json) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) else {
                    continue;
                };
                if value.get("cwd").and_then(|cwd| cwd.as_str()) != Some(cwd) {
                    continue;
                }
                let messages = path.with_extension("jsonl");
                let (model, recent_messages) = read_kiro_messages(&messages);
                let Some(session_id) = path
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
                else {
                    continue;
                };
                let modified_ms = value
                    .get("updated_at")
                    .and_then(|stamp| stamp.as_str())
                    .and_then(morrow_core::discovery::time::parse_iso8601_ms)
                    .or_else(|| transcript_files::mtime_ms(&path.to_string_lossy()))
                    .unwrap_or(0);
                return vec![DiscoveryHit {
                    session_id,
                    source: "kiro",
                    cwd: cwd.to_owned(),
                    tty: None,
                    model,
                    pid: Some(process.pid),
                    modified_at: Some(modified_ms),
                    recent_messages,
                    transcript_path: messages
                        .is_file()
                        .then(|| messages.to_string_lossy().into_owned()),
                }];
            }
        }
        Vec::new()
    }
}

/// `(model, messages)` from a Kiro transcript, whichever layout wrote it.
fn read_kiro_messages(path: &Path) -> (Option<String>, Vec<ChatMessage>) {
    read_tail(path)
        .map(|tail| morrow_core::discovery::kiro::recent_messages(&tail))
        .unwrap_or((None, Vec::new()))
}

/// The first `limit` bytes of a file, for a store that names the session rather
/// than the cwd.
fn read_head(path: &Path, limit: u64) -> Option<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    let mut buffer = Vec::new();
    file.by_ref().take(limit).read_to_end(&mut buffer).ok()?;
    Some(String::from_utf8_lossy(&buffer).into_owned())
}

/// A `.worktrees/` or `-worktree` checkout a subagent was given: it is not a
/// project the user opened, so a card for it would be a card nobody asked for.
fn is_subagent_worktree(cwd: &str) -> bool {
    cwd.contains("/.git/worktrees/")
        || cwd.contains("\\worktrees\\")
        || cwd.contains("/worktrees/")
        || cwd.contains("-worktree/")
}

/// How recently a Cursor transcript must have been written to count as the
/// session a process is running. macOS uses five minutes when the process start is
/// known, and this reader always knows the process — it was handed one.
pub const CURSOR_FRESHNESS_MS: u64 = 300_000;

/// Cursor, found from a process (#276).
///
/// `~/.cursor/projects/<encoded cwd>/agent-transcripts`: the store directory is
/// derived from the working directory, so there is nothing to read without a
/// process — and the process that pointed at it is the pid the card needs.
pub struct CursorTranscripts {
    pub home: String,
    pub now_ms: u64,
}

impl StoreReader for CursorTranscripts {
    fn source(&self) -> &'static str {
        "cursor"
    }

    /// A process-first reader has no roots of its own: it is walked by
    /// `ProcessReader`, and this is only here because the trait asks.
    fn roots(&self) -> Vec<String> {
        Vec::new()
    }

    fn read(&self, _root: &str) -> Vec<DiscoveryHit> {
        Vec::new()
    }
}

impl ProcessReader for CursorTranscripts {
    fn source(&self) -> &'static str {
        "cursor"
    }

    fn read_process(&self, process: &ProcessInfo) -> Vec<DiscoveryHit> {
        let Some(cwd) = process.cwd.as_deref().filter(|cwd| !cwd.is_empty()) else {
            return Vec::new();
        };
        let directory = Path::new(&self.home)
            .join(".cursor/projects")
            .join(morrow_core::discovery::project_dir::app_encoded(cwd))
            .join("agent-transcripts");
        let Some(files) = transcript_files::list(&directory.to_string_lossy()) else {
            return Vec::new();
        };
        let Some(file) = transcript_files::most_recent(
            &files,
            Some(self.now_ms.saturating_sub(CURSOR_FRESHNESS_MS)),
        ) else {
            return Vec::new();
        };
        let Some(tail) = read_tail(Path::new(&file.path)) else {
            return Vec::new();
        };
        let messages = morrow_core::discovery::cursor::recent_messages(&tail);
        if messages.is_empty() {
            return Vec::new();
        }
        vec![DiscoveryHit {
            session_id: file.name.trim_end_matches(".jsonl").to_owned(),
            source: "cursor",
            cwd: cwd.to_owned(),
            tty: None,
            // Cursor's transcript records no model; the Swift reader returns nil
            // for the same reason.
            model: None,
            // Known from birth: this is the pid that pointed at the store.
            pid: Some(process.pid),
            modified_at: Some(file.modified_ms),
            recent_messages: messages,
            transcript_path: Some(file.path.clone()),
        }]
    }
}

/// Gemini, found from a process (#276).
///
/// `~/.gemini/tmp/<project>/chats`, where `<project>` comes from
/// `~/.gemini/projects.json` — a map, not a derivation, because Gemini keys the
/// folder by a hash it does not document. When the map does not name it, the
/// `.project_root` file inside each candidate folder is the fallback.
pub struct GeminiChats {
    pub home: String,
    pub now_ms: u64,
}

impl StoreReader for GeminiChats {
    fn source(&self) -> &'static str {
        "gemini"
    }

    fn roots(&self) -> Vec<String> {
        Vec::new()
    }

    fn read(&self, _root: &str) -> Vec<DiscoveryHit> {
        Vec::new()
    }
}

impl GeminiChats {
    /// `findGeminiProjectDirectory`: the map first, then `.project_root`.
    fn project_directory(&self, cwd: &str) -> Option<String> {
        let tmp = Path::new(&self.home).join(".gemini/tmp");
        let map = std::fs::read_to_string(Path::new(&self.home).join(".gemini/projects.json"))
            .map(|json| morrow_core::discovery::gemini::projects_map(&json))
            .unwrap_or_default();
        if let Some(directory) = map.get(cwd) {
            if tmp.join(directory).is_dir() {
                return Some(directory.clone());
            }
        }
        let mut directories: Vec<String> = std::fs::read_dir(&tmp)
            .ok()?
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                let root = std::fs::read_to_string(entry.path().join(".project_root")).ok()?;
                morrow_core::discovery::gemini::project_root_matches(&root, cwd).then_some(name)
            })
            .collect();
        directories.sort();
        directories.into_iter().next()
    }
}

impl ProcessReader for GeminiChats {
    fn source(&self) -> &'static str {
        "gemini"
    }

    fn read_process(&self, process: &ProcessInfo) -> Vec<DiscoveryHit> {
        let Some(cwd) = process.cwd.as_deref().filter(|cwd| !cwd.is_empty()) else {
            return Vec::new();
        };
        let Some(directory) = self.project_directory(cwd) else {
            return Vec::new();
        };
        let chats = Path::new(&self.home)
            .join(".gemini/tmp")
            .join(directory)
            .join("chats");
        let Some(files) = transcript_files::list_any(&chats.to_string_lossy(), "json") else {
            return Vec::new();
        };
        let Some(file) = transcript_files::most_recent(
            &files,
            Some(self.now_ms.saturating_sub(CURSOR_FRESHNESS_MS)),
        ) else {
            return Vec::new();
        };
        let Some(tail) = read_tail(Path::new(&file.path)) else {
            return Vec::new();
        };
        // A Gemini session file is one JSON document, not JSONL: the whole file is
        // what the reader wants, and the tail may have cut it in half.
        let whole = std::fs::read_to_string(&file.path).unwrap_or(tail);
        let session_id = morrow_core::discovery::gemini::session_id(&whole)
            .unwrap_or_else(|| file.name.trim_end_matches(".json").to_owned());
        let messages = morrow_core::discovery::gemini::recent_messages(&whole);
        if messages.is_empty() {
            return Vec::new();
        }
        vec![DiscoveryHit {
            session_id,
            source: "gemini",
            cwd: cwd.to_owned(),
            tty: None,
            model: morrow_core::discovery::gemini::model(&whole),
            pid: Some(process.pid),
            modified_at: Some(file.modified_ms),
            recent_messages: messages,
            transcript_path: Some(file.path.clone()),
        }]
    }
}

/// Give every hit that has no pid the process that is running it (#267).
///
/// Reads the process table **once**, for the whole scan: a table read per session
/// is a `/proc` walk per session, which is the cost #246 exists to avoid.
///
/// A table that cannot be read, or that names no candidate, leaves the hit
/// without a pid — the state before #266. Discovery must not fail because
/// correlation did: an empty panel is a worse outcome than a card without a
/// watchdog.
pub fn correlate_pids(hits: &mut [DiscoveryHit]) {
    if hits.iter().all(|hit| hit.pid.is_some()) {
        return;
    }
    let table = morrow_platform::agent_process::table();
    correlate_with(hits, &table, |pid| {
        morrow_platform::process::exists(i64::from(pid))
    });
}

/// `correlate_pids` with the table and the liveness probe injected, so the real
/// function — not a copy of it — is what the tests exercise.
pub fn correlate_with(
    hits: &mut [DiscoveryHit],
    table: &[morrow_core::agent_process::ProcessInfo],
    alive: impl Fn(i32) -> bool,
) {
    let needs: Vec<morrow_core::agent_process::PidRequest> = hits
        .iter()
        .filter(|hit| hit.pid.is_none())
        .map(|hit| morrow_core::agent_process::PidRequest {
            session_id: hit.session_id.clone(),
            source: hit.source.to_owned(),
            cwd: hit.cwd.clone(),
            modified_at_ms: hit.modified_at,
        })
        .collect();
    if needs.is_empty() || table.is_empty() {
        return;
    }
    // A pid a hook already named is not a candidate: that session is watched by
    // its own process, and handing its pid to another card would make two cards
    // share one watchdog.
    let mut claimed: std::collections::HashSet<i32> =
        hits.iter().filter_map(|hit| hit.pid).collect();
    let assignments = morrow_core::agent_process::assign(table, &needs, &mut claimed, alive);
    for assignment in assignments {
        if let Some(hit) = hits
            .iter_mut()
            .find(|hit| hit.session_id == assignment.session_id)
        {
            hit.pid = Some(assignment.pid);
        }
    }
}

/// One scan with every reader this port has, each over its own roots.
///
/// `home` is the user's, so each reader derives what it needs: Codex's root is
/// `$CODEX_HOME` or `~/.codex`, OpenCode's database is under `~/.local/share`.
pub fn scan_once(config_dir: &Path, home: &Path) -> ScanResult {
    let transcripts = ClaudeTranscripts {
        config_dir: config_dir.to_string_lossy().into_owned(),
    };
    let opencode = OpenCodeStore {
        data_dir: home.join(".local/share").to_string_lossy().into_owned(),
    };
    let codex = CodexRollouts {
        root: std::env::var("CODEX_HOME")
            .ok()
            .filter(|root| !root.trim().is_empty())
            .unwrap_or_else(|| home.join(".codex").to_string_lossy().into_owned()),
        now_ms: now_ms(),
    };
    // Cline is the one reader of this batch with no process: it runs inside VS
    // Code, so it is discovered from its own task history (#278).
    let cline = ClineStore {
        root: vscode_user_dir(home)
            .join("globalStorage/saoudrizwan.claude-dev")
            .to_string_lossy()
            .into_owned(),
        now_ms: now_ms(),
    };
    // Order is precedence: Claude first, then Codex, then OpenCode, then Cline. A
    // session id two readers both report keeps the earlier one's source.
    let readers: Vec<&dyn StoreReader> = vec![&transcripts, &codex, &opencode, &cline];
    scan_with(
        &readers,
        home,
        &morrow_platform::agent_process::table(),
        now_ms(),
    )
}

/// One scan with the process table injected, so a test drives the process-first
/// readers against a fake table instead of the machine's own processes.
pub fn scan_with(
    readers: &[&dyn StoreReader],
    home: &Path,
    table: &[ProcessInfo],
    now_ms: u64,
) -> ScanResult {
    // Process-first readers: their store directory is derived from a process's
    // cwd, and the process is the pid the card needs (#276).
    let cursor = CursorTranscripts {
        home: home.to_string_lossy().into_owned(),
        now_ms,
    };
    let gemini = GeminiChats {
        home: home.to_string_lossy().into_owned(),
        now_ms,
    };
    // Flat stores: one reader per agent, because each spells its store
    // differently and the source has to be the agent's own name (#278).
    let qoder = FlatStore {
        home: home.to_string_lossy().into_owned(),
        source: "qoder",
        now_ms,
    };
    let codebuddy = FlatStore {
        home: home.to_string_lossy().into_owned(),
        source: "codebuddy",
        now_ms,
    };
    let droid = FlatStore {
        home: home.to_string_lossy().into_owned(),
        source: "droid",
        now_ms,
    };
    let copilot = CopilotSessions {
        home: home.to_string_lossy().into_owned(),
        now_ms,
    };
    let grok = GrokSessions {
        home: home.to_string_lossy().into_owned(),
        now_ms,
    };
    let kimi = KimiSessions {
        home: home.to_string_lossy().into_owned(),
        now_ms,
    };
    let kiro = KiroSessions {
        home: home.to_string_lossy().into_owned(),
    };
    let process_readers: Vec<&dyn ProcessReader> = vec![
        &cursor, &gemini, &qoder, &codebuddy, &droid, &copilot, &grok, &kimi, &kiro,
    ];
    scan::run_all(
        readers,
        &process_readers,
        table,
        scan::DEFAULT_BUDGET_MS,
        || now_ms,
    )
}

/// The roots to scan for Claude Code today: the live config dir, plus any other
/// directory under it that holds `projects/`.
///
/// The extras are not Settings-registered ones yet (`ExtraConfigDir` has no Rust
/// counterpart); they are the sibling accounts a multi-account user already has,
/// discovered by the same marker that makes a root live.
pub fn claude_roots(primary: &Path) -> Vec<String> {
    let primary = primary.to_string_lossy().into_owned();
    let mut roots = scan::scan_roots(&primary, &[]);
    // A `~/.claude-work` style sibling: same parent, holds `projects/`.
    if let Some(parent) = Path::new(&primary).parent() {
        if let Ok(entries) = std::fs::read_dir(parent) {
            let mut siblings: Vec<String> = entries
                .flatten()
                .filter_map(|entry| {
                    let path = entry.path();
                    let name = path.file_name()?.to_string_lossy().into_owned();
                    if !name.starts_with(".claude") || path == Path::new(&primary) {
                        return None;
                    }
                    path.join("projects")
                        .is_dir()
                        .then(|| path.to_string_lossy().into_owned())
                })
                .collect();
            // Deterministic, so two runs scan in the same order.
            siblings.sort();
            roots.extend(scan::scan_roots(
                &primary,
                &siblings.iter().map(String::as_str).collect::<Vec<_>>(),
            ));
            // `scan_roots` puts the primary first and drops duplicates; the first
            // call above is subsumed by this one.
            return dedupe(roots);
        }
    }
    roots
}

fn dedupe(roots: Vec<String>) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    roots
        .into_iter()
        .filter(|root| seen.insert(morrow_core::path_identity::identity(root)))
        .collect()
}

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::path::PathBuf;

    use super::*;

    /// A scratch `~` that removes itself.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("morrow-backfill-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("a scratch home");
            Self(path)
        }

        fn project_dir(&self, root: &str, encoded: &str) -> PathBuf {
            let path = self.0.join(root).join("projects").join(encoded);
            fs::create_dir_all(&path).expect("a project directory");
            path
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn transcript(cwd: &str, model: &str) -> String {
        format!(
            "{}\n{}\n",
            format_args!(
                r#"{{"type":"user","cwd":"{cwd}","message":{{"role":"user","content":"Add a filter bar"}}}}"#
            ),
            format_args!(
                r#"{{"type":"assistant","cwd":"{cwd}","message":{{"model":"{model}","content":[{{"type":"text","text":"Added the bar"}}]}}}}"#
            )
        )
    }

    fn write(path: &Path, contents: &str) {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("a transcript");
        file.write_all(contents.as_bytes()).expect("a write");
    }

    #[test]
    fn a_transcript_without_a_hook_becomes_a_session() {
        let scratch = Scratch::new("one");
        let projects = scratch.project_dir(".claude", "-home-dev-project");
        write(
            &projects.join("session-a.jsonl"),
            &transcript("/home/dev/project", "model-x"),
        );

        let result = scan_once(&scratch.0.join(".claude"), &scratch.0);

        assert_eq!(result.hits.len(), 1);
        let hit = &result.hits[0];
        assert_eq!(hit.session_id, "session-a");
        assert_eq!(hit.source, "claude");
        assert_eq!(hit.cwd, "/home/dev/project");
        assert_eq!(hit.model.as_deref(), Some("model-x"));
        assert_eq!(hit.recent_messages.len(), 2);
        assert!(
            hit.transcript_path
                .as_deref()
                .is_some_and(|path| path.ends_with("session-a.jsonl"))
        );
        assert!(hit.modified_at.is_some());
    }

    #[test]
    fn a_store_that_is_not_there_is_no_sessions() {
        let scratch = Scratch::new("missing");
        assert!(
            scan_once(&scratch.0.join(".claude"), &scratch.0)
                .hits
                .is_empty()
        );
    }

    /// #267: a hit with no pid gets the process that is running it, so `Watches`
    /// can prune it. Exercises `correlate_with`, the function `correlate_pids`
    /// delegates to, not a copy of its filter.
    #[test]
    fn a_hit_without_a_pid_gets_the_process_running_it() {
        let table = vec![process(
            4242,
            "/usr/bin/claude",
            &["claude"],
            "/home/dev/project",
            1_000,
        )];
        let mut hits = vec![hit("s", "/home/dev/project", 5_000, None)];

        correlate_with(&mut hits, &table, |_| true);

        assert_eq!(hits[0].pid, Some(4242));
    }

    /// A pid a hook already named is not handed to another card, or two cards
    /// would share one watchdog and one of them would outlive its process.
    #[test]
    fn a_pid_a_hook_named_is_not_given_away() {
        let table = vec![process(7, "/usr/bin/claude", &["claude"], "/w", 1_000)];
        let mut hits = vec![
            hit("has-pid", "/w", 2_000, Some(7)),
            hit("needs-pid", "/w", 2_000, None),
        ];

        correlate_with(&mut hits, &table, |_| true);

        assert_eq!(hits[0].pid, Some(7), "the hook's own pid is untouched");
        assert_eq!(hits[1].pid, None, "the only candidate is already taken");
    }

    /// An empty table — macOS, a permission wall, a host with no `/proc` — leaves
    /// the hits without pids and must not fail: an empty panel is a worse outcome
    /// than a card without a watchdog.
    #[test]
    fn a_table_that_cannot_be_read_leaves_the_hits_alone() {
        let mut hits = vec![hit("s", "/w", 2_000, None)];
        correlate_with(&mut hits, &[], |_| true);
        assert_eq!(hits[0].pid, None);
    }

    /// The table is read once for the whole scan. A hit that already has a pid
    /// asks for nothing, so a scan with nothing to correlate never reads a table.
    #[test]
    fn a_hit_that_already_has_a_pid_asks_for_nothing() {
        let mut hits = vec![hit("s", "/w", 2_000, Some(9))];
        correlate_with(&mut hits, &[], |_| panic!("a table must not be read"));
        assert_eq!(hits[0].pid, Some(9));
    }

    /// Two sessions in one directory, neither with a pid: each gets its own, and
    /// a dead process is not a candidate.
    #[test]
    fn two_sessions_in_one_directory_get_their_own_processes() {
        let table = vec![
            process(10, "/usr/bin/claude", &["claude"], "/w", 1_000),
            process(11, "/usr/bin/claude", &["claude"], "/w", 5_000),
            // Long dead, so never a candidate whatever the order.
            process(12, "/usr/bin/claude", &["claude"], "/w", 500),
        ];
        let mut hits = vec![hit("a", "/w", 1_500, None), hit("b", "/w", 5_500, None)];

        correlate_with(&mut hits, &table, |pid| pid != 12);

        assert_eq!(hits[0].pid, Some(10));
        assert_eq!(hits[1].pid, Some(11));
    }

    fn process(
        pid: i32,
        executable: &str,
        args: &[&str],
        cwd: &str,
        started: u64,
    ) -> morrow_core::agent_process::ProcessInfo {
        morrow_core::agent_process::ProcessInfo {
            pid,
            executable: executable.to_owned(),
            arguments: args.iter().map(|arg| (*arg).to_owned()).collect(),
            cwd: Some(cwd.to_owned()),
            start_time_ms: Some(started),
        }
    }

    fn hit(session_id: &str, cwd: &str, modified_at: u64, pid: Option<i32>) -> DiscoveryHit {
        DiscoveryHit {
            session_id: session_id.to_owned(),
            source: "claude",
            cwd: cwd.to_owned(),
            pid,
            modified_at: Some(modified_at),
            ..DiscoveryHit::default()
        }
    }

    /// #270: a real OpenCode database becomes a card, with the working directory
    /// the store itself records — the one reader that can be backfilled without a
    /// process.
    #[test]
    fn an_opencode_database_becomes_a_session() {
        let scratch = Scratch::new("opencode");
        let data = scratch.0.join(".local/share");
        std::fs::create_dir_all(data.join("opencode")).expect("a data directory");
        {
            let connection =
                rusqlite::Connection::open(data.join("opencode/opencode.db")).expect("a store");
            connection
                .execute_batch(
                    "CREATE TABLE session (id TEXT PRIMARY KEY, directory TEXT, parent_id TEXT,
                        time_updated INTEGER, time_archived INTEGER);
                     CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT, data TEXT,
                        time_updated INTEGER);
                     CREATE TABLE part (id TEXT PRIMARY KEY, message_id TEXT, session_id TEXT,
                        data TEXT, time_created INTEGER);
                     INSERT INTO session VALUES ('oc-1', '/home/dev/project', NULL, 7000, NULL);
                     INSERT INTO message VALUES ('m1', 'oc-1',
                        '{\"role\":\"assistant\",\"modelID\":\"model-x\"}', 6000);
                     INSERT INTO part VALUES ('p1', 'm1', 'oc-1',
                        '{\"type\":\"text\",\"text\":\"Done\"}', 6000);",
                )
                .expect("a schema");
        }

        let result = scan_once(&scratch.0.join(".claude"), &scratch.0);

        assert_eq!(result.hits.len(), 1);
        let hit = &result.hits[0];
        assert_eq!(hit.session_id, "oc-1");
        assert_eq!(hit.source, "opencode");
        assert_eq!(
            hit.cwd, "/home/dev/project",
            "the store's own directory column"
        );
        assert_eq!(hit.model.as_deref(), Some("model-x"));
        assert_eq!(hit.modified_at, Some(7_000));
        assert_eq!(hit.recent_messages.len(), 1);
    }

    /// Two readers in one scan: Claude's transcripts and OpenCode's database both
    /// arrive, each from its own root.
    #[test]
    fn both_readers_contribute_to_one_scan() {
        let scratch = Scratch::new("both");
        let projects = scratch.project_dir(".claude", "-home-dev-project");
        write(
            &projects.join("claude-1.jsonl"),
            &transcript("/home/dev/p", "model-x"),
        );
        let data = scratch.0.join(".local/share");
        std::fs::create_dir_all(data.join("opencode")).expect("a data directory");
        {
            let connection =
                rusqlite::Connection::open(data.join("opencode/opencode.db")).expect("a store");
            connection
                .execute_batch(
                    "CREATE TABLE session (id TEXT PRIMARY KEY, directory TEXT,
                        time_updated INTEGER);
                     INSERT INTO session VALUES ('oc-1', '/home/dev/q', 7000);",
                )
                .expect("a schema");
        }

        let mut hits = scan_once(&scratch.0.join(".claude"), &scratch.0).hits;
        hits.sort_by(|a, b| a.session_id.cmp(&b.session_id));

        let ids: Vec<&str> = hits.iter().map(|hit| hit.session_id.as_str()).collect();
        assert_eq!(ids, vec!["claude-1", "oc-1"]);
        assert_eq!(hits[0].source, "claude");
        assert_eq!(hits[1].source, "opencode");
    }

    /// A database that is not there is no sessions, and a database that cannot be
    /// opened does not lose the other reader's results.
    #[test]
    fn a_missing_or_unreadable_database_is_not_an_error() {
        let scratch = Scratch::new("opencode-missing");
        assert!(
            scan_once(&scratch.0.join(".claude"), &scratch.0.join("nowhere"))
                .hits
                .is_empty()
        );

        // A file where the database should be, holding junk.
        let data = scratch.0.join(".local/share");
        std::fs::create_dir_all(data.join("opencode")).expect("a data directory");
        std::fs::write(data.join("opencode/opencode.db"), b"not a database").expect("a file");
        let projects = scratch.project_dir(".claude", "-home-dev-project");
        write(
            &projects.join("claude-1.jsonl"),
            &transcript("/home/dev/p", "model-x"),
        );

        let hits = scan_once(&scratch.0.join(".claude"), &scratch.0).hits;
        assert_eq!(hits.len(), 1, "the transcripts still arrive");
        assert_eq!(hits[0].source, "claude");
    }

    /// #272: a real Codex rollout becomes a card, with the thread id, cwd and model
    /// the rollout itself records — no hook, no process.
    #[test]
    fn a_codex_rollout_becomes_a_session() {
        let scratch = Scratch::new("codex");
        // Today's directory, which is one of the seven the reader looks at.
        let today = today_rollout_directory(&scratch.0.join(".codex"));
        std::fs::create_dir_all(&today).expect("a rollout directory");
        write(
            &today.join("rollout-2026-11-22T10-00-00-abc123.jsonl"),
            &codex_rollout("thread-9", "/home/dev/project", "gpt-5-codex"),
        );

        let result = scan_once(&scratch.0.join(".claude"), &scratch.0);

        assert_eq!(result.hits.len(), 1);
        let hit = &result.hits[0];
        assert_eq!(hit.session_id, "thread-9");
        assert_eq!(hit.source, "codex");
        assert_eq!(hit.cwd, "/home/dev/project");
        assert_eq!(hit.model.as_deref(), Some("gpt-5-codex"));
        assert_eq!(hit.recent_messages.len(), 2);
    }

    /// 2026-11-22T12:00:00Z, pinned so the window tests do not depend on the
    /// clock the CI runner happens to have.
    const PINNED_NOW_MS: u64 = 1_795_348_800_000;

    /// A rollout in the Codex day-directory `days_ago` before `now_ms`.
    fn seed_rollout(root: &Path, now_ms: u64, days_ago: u64, id: &str) {
        let (year, month, day) = civil_from_days((now_ms / 86_400_000) as i64 - days_ago as i64);
        let directory = root
            .join("sessions")
            .join(format!("{year:04}"))
            .join(format!("{month:02}"))
            .join(format!("{day:02}"));
        fs::create_dir_all(&directory).expect("a rollout directory");
        write(
            &directory.join(format!("rollout-{id}.jsonl")),
            &codex_rollout(id, "/w", "model-x"),
        );
    }

    fn codex_ids(root: &Path) -> Vec<String> {
        let reader = CodexRollouts {
            root: root.to_string_lossy().into_owned(),
            now_ms: PINNED_NOW_MS,
        };
        let mut ids: Vec<String> = StoreReader::read(&reader, &reader.root)
            .into_iter()
            .map(|hit| hit.session_id)
            .collect();
        ids.sort();
        ids
    }

    /// The store grows forever; a backfill that read all of it would walk years of
    /// rollouts on every attach. Only the seven days ending today count — not the
    /// seven newest directory names a stale tree happens to hold.
    #[test]
    fn only_the_last_seven_rollout_days_are_read() {
        let scratch = Scratch::new("codex-days");
        let root = scratch.0.join(".codex");
        // Ten consecutive days ending today, plus a tree left over from a month ago.
        for back in 0..10 {
            seed_rollout(&root, PINNED_NOW_MS, back, &format!("s{back}"));
        }
        seed_rollout(&root, PINNED_NOW_MS, 40, "stale");

        let ids = codex_ids(&root);

        // `back` 7..9 and the 40-day-old rollout are outside the window.
        assert_eq!(
            ids,
            vec!["s0", "s1", "s2", "s3", "s4", "s5", "s6"],
            "seven days ending today, not seven directory names"
        );
    }

    /// The regression this window exists for: a full set of seven old directories
    /// used to be read forever, because the filter took the last seven names
    /// instead of comparing them to today.
    #[test]
    fn a_stale_sessions_tree_is_never_backfilled() {
        let scratch = Scratch::new("codex-stale");
        let root = scratch.0.join(".codex");
        // Seven consecutive days 34–40 days back: the old filter would take all
        // seven of these and ignore how old they are.
        for back in 34..=40 {
            seed_rollout(&root, PINNED_NOW_MS, back, &format!("stale-{back}"));
        }
        seed_rollout(&root, PINNED_NOW_MS, 0, "today");

        assert_eq!(codex_ids(&root), vec!["today"]);
    }

    /// A rollout that is not a rollout does not become a card, and does not lose
    /// the other readers' hits.
    #[test]
    fn an_unusable_rollout_is_skipped() {
        let scratch = Scratch::new("codex-junk");
        let today = today_rollout_directory(&scratch.0.join(".codex"));
        std::fs::create_dir_all(&today).expect("a rollout directory");
        write(&today.join("empty.jsonl"), "");
        write(&today.join("junk.jsonl"), "not json\n");
        let projects = scratch.project_dir(".claude", "-home-dev-project");
        write(
            &projects.join("claude-1.jsonl"),
            &transcript("/home/dev/p", "model-x"),
        );

        let hits = scan_once(&scratch.0.join(".claude"), &scratch.0).hits;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].source, "claude");
    }

    /// `~/.codex/sessions/<yyyy>/<mm>/<dd>` for today, which the reader's window
    /// always includes.
    fn today_rollout_directory(root: &Path) -> PathBuf {
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("a clock")
            .as_secs();
        let (year, month, day) = civil_date(seconds);
        root.join("sessions")
            .join(format!("{year:04}"))
            .join(format!("{month:02}"))
            .join(format!("{day:02}"))
    }

    /// A UTC civil date from a Unix timestamp: enough for a test directory name.
    fn civil_date(seconds: u64) -> (i64, u32, u32) {
        civil_from_days((seconds / 86_400) as i64)
    }

    fn codex_rollout(id: &str, cwd: &str, model: &str) -> String {
        format!(
            "{}\n{}\n{}\n{}\n",
            format_args!(r#"{{"type":"session_meta","payload":{{"id":"{id}","cwd":"{cwd}"}}}}"#),
            format_args!(r#"{{"type":"turn_context","payload":{{"model":"{model}"}}}}"#),
            r#"{"type":"event_msg","payload":{"type":"user_message","message":"Add a filter bar"}}"#,
            r#"{"type":"event_msg","payload":{"type":"agent_message","message":"Added the bar"}}"#,
        )
    }

    /// #276: Cursor's store directory is derived from a process's cwd, so the
    /// process is what finds it — and the pid comes with it.
    #[test]
    fn a_cursor_process_finds_its_transcript_and_keeps_its_pid() {
        let scratch = Scratch::new("cursor");
        let cwd = "/home/dev/project";
        let directory = scratch
            .0
            .join(".cursor/projects")
            .join(morrow_core::discovery::project_dir::app_encoded(cwd))
            .join("agent-transcripts");
        std::fs::create_dir_all(&directory).expect("a store");
        write(
            &directory.join("cursor-session-1.jsonl"),
            &format!(
                "{}\n{}\n",
                r#"{"role":"user","message":{"content":"Add a filter bar"}}"#,
                r#"{"role":"assistant","message":{"content":[{"type":"text","text":"Added the bar"}]}}"#
            ),
        );

        let table = vec![process_with_cwd(
            4242,
            "/usr/bin/cursor-agent",
            "cursor-agent",
            cwd,
        )];
        let readers: Vec<&dyn StoreReader> = Vec::new();
        let now = now_ms();
        let result = scan_with(&readers, &scratch.0, &table, now);

        assert_eq!(result.hits.len(), 1);
        let hit = &result.hits[0];
        assert_eq!(hit.session_id, "cursor-session-1");
        assert_eq!(hit.source, "cursor");
        assert_eq!(hit.cwd, cwd);
        assert_eq!(
            hit.pid,
            Some(4242),
            "process-first: the pid is known from birth"
        );
        assert_eq!(hit.recent_messages.len(), 2);
    }

    /// A transcript from hours ago is not what this process is running.
    #[test]
    fn a_stale_cursor_transcript_is_not_a_session() {
        let scratch = Scratch::new("cursor-stale");
        let cwd = "/home/dev/project";
        let directory = scratch
            .0
            .join(".cursor/projects")
            .join(morrow_core::discovery::project_dir::app_encoded(cwd))
            .join("agent-transcripts");
        std::fs::create_dir_all(&directory).expect("a store");
        write(
            &directory.join("old.jsonl"),
            "{\"role\":\"user\",\"message\":{\"content\":\"hi\"}}\n",
        );

        let table = vec![process_with_cwd(
            1,
            "/usr/bin/cursor-agent",
            "cursor-agent",
            cwd,
        )];
        let readers: Vec<&dyn StoreReader> = Vec::new();
        // Ten minutes later: past the five-minute freshness window.
        let result = scan_with(&readers, &scratch.0, &table, now_ms() + 600_000);
        assert!(result.hits.is_empty());
    }

    /// Gemini's folder is named by a hash, so it is looked up in `projects.json`.
    #[test]
    fn a_gemini_process_finds_its_chats_through_the_projects_map() {
        let scratch = Scratch::new("gemini");
        let cwd = "/home/dev/project";
        let chats = scratch.0.join(".gemini/tmp/hash123/chats");
        std::fs::create_dir_all(&chats).expect("a store");
        std::fs::write(
            scratch.0.join(".gemini/projects.json"),
            format!(r#"{{"projects":{{"{cwd}":"hash123"}}}}"#),
        )
        .expect("a map");
        std::fs::write(
            chats.join("session-1.json"),
            r#"{"sessionId":"gem-1","model":"gemini-pro","messages":[
                    {"type":"user","content":"Where is the router?"},
                    {"type":"gemini","content":"In src/router.ts"}]}"#,
        )
        .expect("a session");

        let table = vec![process_with_cwd(555, "/usr/bin/gemini", "gemini", cwd)];
        let readers: Vec<&dyn StoreReader> = Vec::new();
        let result = scan_with(&readers, &scratch.0, &table, now_ms());

        assert_eq!(result.hits.len(), 1);
        let hit = &result.hits[0];
        assert_eq!(hit.session_id, "gem-1");
        assert_eq!(hit.source, "gemini");
        assert_eq!(hit.cwd, cwd);
        assert_eq!(hit.model.as_deref(), Some("gemini-pro"));
        assert_eq!(hit.pid, Some(555));
        assert_eq!(hit.recent_messages.len(), 2);
    }

    /// When the map does not name the folder, `.project_root` does.
    #[test]
    fn a_gemini_folder_is_found_by_its_project_root_file() {
        let scratch = Scratch::new("gemini-root");
        let cwd = "/home/dev/project";
        let chats = scratch.0.join(".gemini/tmp/other/chats");
        std::fs::create_dir_all(&chats).expect("a store");
        std::fs::write(scratch.0.join(".gemini/tmp/other/.project_root"), cwd)
            .expect("a root file");
        std::fs::write(
            chats.join("session-1.json"),
            r#"{"sessionId":"gem-2","messages":[{"type":"user","content":"hi"}]}"#,
        )
        .expect("a session");

        let table = vec![process_with_cwd(7, "/usr/bin/gemini", "gemini", cwd)];
        let readers: Vec<&dyn StoreReader> = Vec::new();
        let result = scan_with(&readers, &scratch.0, &table, now_ms());
        assert_eq!(result.hits.len(), 1);
        assert_eq!(result.hits[0].session_id, "gem-2");
    }

    /// A process whose cwd could not be read is skipped, not guessed: a made-up
    /// directory would put a card in a folder the user does not have.
    #[test]
    fn a_process_without_a_cwd_is_skipped() {
        let scratch = Scratch::new("no-cwd");
        let table = vec![ProcessInfo {
            pid: 9,
            executable: "/usr/bin/cursor-agent".to_owned(),
            arguments: vec!["cursor-agent".to_owned()],
            cwd: None,
            start_time_ms: Some(1),
        }];
        let readers: Vec<&dyn StoreReader> = Vec::new();
        assert!(
            scan_with(&readers, &scratch.0, &table, now_ms())
                .hits
                .is_empty()
        );
    }

    /// #278: Cline is discovered with nothing running, because its task history
    /// records the cwd.
    #[test]
    fn a_cline_task_becomes_a_session_with_no_process() {
        let scratch = Scratch::new("cline");
        // The real per-OS path, so a wrong `vscode_user_dir` fails here rather
        // than on the only machine that has it.
        let root = vscode_user_dir(&scratch.0).join("globalStorage/saoudrizwan.claude-dev");
        std::fs::create_dir_all(root.join("state")).expect("a state dir");
        std::fs::create_dir_all(root.join("tasks/task-1")).expect("a task dir");
        std::fs::write(
            root.join("state/taskHistory.json"),
            r#"[{"id":"task-1","ts":1000,"cwdOnTaskInitialization":"/home/dev/project","modelId":"claude-sonnet"}]"#,
        )
        .expect("a history");
        std::fs::write(
            root.join("tasks/task-1/api_conversation_history.json"),
            r#"[{"role":"user","content":"Why is the build red?"},
                {"role":"assistant","content":"A missing import"}]"#,
        )
        .expect("a conversation");

        let now = now_ms();
        let cline_store = ClineStore {
            root: root.to_string_lossy().into_owned(),
            now_ms: now,
        };
        let readers: Vec<&dyn StoreReader> = vec![&cline_store];
        let result = scan_with(&readers, &scratch.0, &[], now);
        let cline: Vec<_> = result.hits.iter().filter(|h| h.source == "cline").collect();
        assert_eq!(cline.len(), 1);
        assert_eq!(cline[0].session_id, "task-1");
        assert_eq!(cline[0].cwd, "/home/dev/project");
        assert_eq!(cline[0].model.as_deref(), Some("claude-sonnet"));
        assert_eq!(cline[0].pid, None, "an extension has no process");
        assert_eq!(cline[0].recent_messages.len(), 2);
    }

    /// A task whose conversation was last written hours ago is not what the user
    /// is looking at, even if it is the newest row in the history.
    #[test]
    fn a_stale_cline_task_is_not_a_session() {
        let scratch = Scratch::new("cline-stale");
        // The real per-OS path, so a wrong `vscode_user_dir` fails here rather
        // than on the only machine that has it.
        let root = vscode_user_dir(&scratch.0).join("globalStorage/saoudrizwan.claude-dev");
        std::fs::create_dir_all(root.join("state")).expect("a state dir");
        std::fs::create_dir_all(root.join("tasks/task-1")).expect("a task dir");
        std::fs::write(
            root.join("state/taskHistory.json"),
            r#"[{"id":"task-1","ts":1000,"cwdOnTaskInitialization":"/home/dev/project"}]"#,
        )
        .expect("a history");
        std::fs::write(
            root.join("tasks/task-1/api_conversation_history.json"),
            "[]",
        )
        .expect("a conversation");

        // An hour past the ten-minute window: the clock the reader is given is
        // the one that decides, so it is the clock the store carries.
        let now = now_ms() + 3_600_000;
        let cline_store = ClineStore {
            root: root.to_string_lossy().into_owned(),
            now_ms: now,
        };
        let readers: Vec<&dyn StoreReader> = vec![&cline_store];
        let result = scan_with(&readers, &scratch.0, &[], now);
        assert!(result.hits.iter().all(|h| h.source != "cline"));
    }

    /// #278: Qoder, CodeBuddy and Factory are one reader with a table, because
    /// they differ only in the base directory, the encoder, and the flavour.
    #[test]
    fn a_flat_store_process_becomes_a_session() {
        let scratch = Scratch::new("flat");
        let cwd = "/home/dev/project";
        let project = scratch
            .0
            .join(".codebuddy/projects")
            .join(morrow_core::discovery::project_dir::app_encoded(cwd));
        std::fs::create_dir_all(&project).expect("a store");
        std::fs::write(
            project.join("cb-1.jsonl"),
            concat!(
                r#"{"type":"message","role":"user","content":"Ship it","providerData":{"model":"claude-sonnet"}}"#,
                "\n",
                r#"{"type":"message","role":"assistant","content":"Shipped"}"#
            ),
        )
        .expect("a transcript");

        let table = vec![process_with_cwd(88, "/usr/bin/codebuddy", "codebuddy", cwd)];
        let readers: Vec<&dyn StoreReader> = Vec::new();
        let result = scan_with(&readers, &scratch.0, &table, now_ms());
        let hits: Vec<_> = result
            .hits
            .iter()
            .filter(|h| h.source == "codebuddy")
            .collect();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].session_id, "cb-1");
        assert_eq!(hits[0].model.as_deref(), Some("claude-sonnet"));
        assert_eq!(hits[0].pid, Some(88));
    }

    /// Factory's model lives in a sidecar file, not in the transcript.
    #[test]
    fn a_factory_store_reads_its_model_from_the_sidecar() {
        let scratch = Scratch::new("factory");
        let cwd = "/home/dev/project";
        let project = scratch
            .0
            .join(".factory/sessions")
            .join(morrow_core::discovery::project_dir::encoded(cwd));
        std::fs::create_dir_all(&project).expect("a store");
        std::fs::write(
            project.join("d-1.jsonl"),
            r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Deploy"}]}}"#,
        )
        .expect("a transcript");
        std::fs::write(
            project.join("d-1.settings.json"),
            r#"{"model":"gpt-5.1-codex"}"#,
        )
        .expect("a sidecar");

        let table = vec![process_with_cwd(33, "/usr/bin/droid", "droid", cwd)];
        let readers: Vec<&dyn StoreReader> = Vec::new();
        let result = scan_with(&readers, &scratch.0, &table, now_ms());
        let hits: Vec<_> = result.hits.iter().filter(|h| h.source == "droid").collect();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].model.as_deref(), Some("gpt-5.1-codex"));
        assert_eq!(hits[0].pid, Some(33));
    }

    /// #279: Copilot names its store by session, so the cwd has to be read out of
    /// the event stream — and a stream belonging to another cwd must be refused.
    #[test]
    fn a_copilot_process_finds_the_stream_that_names_its_cwd() {
        let scratch = Scratch::new("copilot");
        let base = scratch.0.join(".copilot/session-state");
        for (id, cwd) in [("other", "/home/dev/other"), ("mine", "/home/dev/project")] {
            std::fs::create_dir_all(base.join(id)).expect("a store");
            std::fs::write(
                base.join(id).join("events.jsonl"),
                format!(
                    concat!(
                        r#"{{"type":"session.start","data":{{"context":{{"cwd":"{cwd}"}}}}}}"#,
                        "\n",
                        r#"{{"type":"user.message","data":{{"content":"Fix the flaky test"}}}}"#,
                        "\n",
                        r#"{{"type":"session.info","data":{{"model":"claude-sonnet"}}}}"#
                    ),
                    cwd = cwd
                ),
            )
            .expect("a stream");
        }

        let table = vec![process_with_cwd(
            71,
            "/usr/bin/copilot",
            "copilot",
            "/home/dev/project",
        )];
        let readers: Vec<&dyn StoreReader> = Vec::new();
        let result = scan_with(&readers, &scratch.0, &table, now_ms());
        let hits: Vec<_> = result
            .hits
            .iter()
            .filter(|h| h.source == "copilot")
            .collect();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].session_id, "mine");
        assert_eq!(hits[0].pid, Some(71));
        assert_eq!(hits[0].model.as_deref(), Some("claude-sonnet"));
        assert_eq!(hits[0].recent_messages.len(), 1);
    }

    /// A newline cannot be read out of a store that names the cwd, so the head is
    /// what decides: a stream whose cwd never appears is nothing.
    #[test]
    fn a_copilot_stream_for_another_cwd_is_not_this_session() {
        let scratch = Scratch::new("copilot-other");
        let base = scratch.0.join(".copilot/session-state/mine");
        std::fs::create_dir_all(&base).expect("a store");
        std::fs::write(
            base.join("events.jsonl"),
            r#"{"type":"session.start","data":{"context":{"cwd":"/home/dev/elsewhere"}}}"#,
        )
        .expect("a stream");

        let table = vec![process_with_cwd(
            72,
            "/usr/bin/copilot",
            "copilot",
            "/home/dev/project",
        )];
        let readers: Vec<&dyn StoreReader> = Vec::new();
        assert!(
            scan_with(&readers, &scratch.0, &table, now_ms())
                .hits
                .iter()
                .all(|h| h.source != "copilot")
        );
    }

    /// #279: Grok's directory is keyed by the percent-encoded cwd, and the summary
    /// is what names the session and its activity.
    #[test]
    fn a_grok_process_finds_its_session_and_reads_its_transcript() {
        let scratch = Scratch::new("grok");
        let cwd = "/home/dev/project";
        let directory = scratch
            .0
            .join(".grok/sessions")
            .join(morrow_core::discovery::grok::encoded_cwd(cwd))
            .join("session-abc");
        std::fs::create_dir_all(&directory).expect("a store");
        let now = now_ms();
        let stamp = iso8601(now - 60_000);
        std::fs::write(
            directory.join("summary.json"),
            format!(
                r#"{{"info":{{"id":"0192f0aa-7cbb-7000-8000-000000000000","cwd":"{cwd}"}},
                    "current_model_id":"grok-code-fast-1",
                    "updated_at":"{stamp}"}}"#
            ),
        )
        .expect("a summary");
        std::fs::write(
            directory.join("chat_history.jsonl"),
            concat!(
                r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Add a retry"}]}}"#,
                "\n",
                r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Added it"}]}}"#
            ),
        )
        .expect("a transcript");

        let table = vec![process_with_cwd(81, "/usr/bin/grok", "grok", cwd)];
        let readers: Vec<&dyn StoreReader> = Vec::new();
        let result = scan_with(&readers, &scratch.0, &table, now);
        let hits: Vec<_> = result.hits.iter().filter(|h| h.source == "grok").collect();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].session_id, "0192f0aa-7cbb-7000-8000-000000000000");
        assert_eq!(hits[0].model.as_deref(), Some("grok-code-fast-1"));
        assert_eq!(hits[0].pid, Some(81));
        assert_eq!(hits[0].recent_messages.len(), 2);
    }

    /// The directory is keyed by cwd, but the summary says so itself: a session
    /// whose cwd moved is not this process's session.
    #[test]
    fn a_grok_session_whose_summary_names_another_cwd_is_skipped() {
        let scratch = Scratch::new("grok-moved");
        let cwd = "/home/dev/project";
        let directory = scratch
            .0
            .join(".grok/sessions")
            .join(morrow_core::discovery::grok::encoded_cwd(cwd))
            .join("session-abc");
        std::fs::create_dir_all(&directory).expect("a store");
        let now = now_ms();
        std::fs::write(
            directory.join("summary.json"),
            format!(
                r#"{{"info":{{"id":"s","cwd":"/home/dev/moved"}},"updated_at":"{}"}}"#,
                iso8601(now)
            ),
        )
        .expect("a summary");

        let table = vec![process_with_cwd(82, "/usr/bin/grok", "grok", cwd)];
        let readers: Vec<&dyn StoreReader> = Vec::new();
        assert!(
            scan_with(&readers, &scratch.0, &table, now)
                .hits
                .iter()
                .all(|h| h.source != "grok")
        );
    }

    /// An index-only session has no transcript, so there is nothing to show.
    #[test]
    fn a_grok_session_without_a_transcript_is_skipped() {
        let scratch = Scratch::new("grok-notranscript");
        let cwd = "/home/dev/project";
        let directory = scratch
            .0
            .join(".grok/sessions")
            .join(morrow_core::discovery::grok::encoded_cwd(cwd))
            .join("session-abc");
        std::fs::create_dir_all(&directory).expect("a store");
        let now = now_ms();
        std::fs::write(
            directory.join("summary.json"),
            format!(
                r#"{{"info":{{"id":"s","cwd":"{cwd}"}},"updated_at":"{}"}}"#,
                iso8601(now)
            ),
        )
        .expect("a summary");

        let table = vec![process_with_cwd(83, "/usr/bin/grok", "grok", cwd)];
        let readers: Vec<&dyn StoreReader> = Vec::new();
        assert!(
            scan_with(&readers, &scratch.0, &table, now)
                .hits
                .iter()
                .all(|h| h.source != "grok")
        );
    }

    /// #282: kimi-code's index names the session directory, so no hash is needed.
    #[test]
    fn a_kimi_index_entry_becomes_a_session() {
        let scratch = Scratch::new("kimi-index");
        let cwd = "/home/dev/project";
        let session = scratch.0.join(".kimi-code/sessions/s1");
        std::fs::create_dir_all(session.join("agents/main")).expect("a store");
        std::fs::write(session.join("state.json"), "{}").expect("a state file");
        std::fs::write(
            session.join("agents/main/wire.jsonl"),
            concat!(
                r#"{"type":"turn.prompt","input":[{"type":"text","text":"Read the logs"}]}"#,
                "\n",
                r#"{"type":"context.append_loop_event","event":{"type":"content.part","part":{"type":"text","text":"Found it"}}}"#
            ),
        )
        .expect("a wire transcript");
        std::fs::write(
            scratch.0.join(".kimi-code/session_index.jsonl"),
            format!(
                r#"{{"sessionId":"s1","sessionDir":"{}","workDir":"{cwd}"}}"#,
                session.to_string_lossy()
            ),
        )
        .expect("an index");

        let table = vec![process_with_cwd(91, "/usr/bin/kimi", "kimi", cwd)];
        let readers: Vec<&dyn StoreReader> = Vec::new();
        let result = scan_with(&readers, &scratch.0, &table, now_ms());
        let hits: Vec<_> = result.hits.iter().filter(|h| h.source == "kimi").collect();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].session_id, "s1");
        assert_eq!(hits[0].pid, Some(91));
        assert_eq!(hits[0].recent_messages.len(), 2);
    }

    /// The legacy layout is the fallback, and its directory is `md5(cwd)`.
    #[test]
    fn a_kimi_legacy_hashed_directory_is_the_fallback() {
        let scratch = Scratch::new("kimi-hash");
        let cwd = "/home/dev/project";
        let session = scratch
            .0
            .join(".kimi/sessions")
            .join(morrow_core::discovery::kimi::workdir_hash(cwd))
            .join("legacy-1");
        std::fs::create_dir_all(&session).expect("a store");
        std::fs::write(
            session.join("wire.jsonl"),
            concat!(
                r#"{"message":{"type":"TurnBegin","payload":{"user_input":[{"type":"text","text":"Why?"}]}}}"#,
                "\n",
                r#"{"message":{"type":"TurnEnd"}}"#
            ),
        )
        .expect("a wire transcript");

        let table = vec![process_with_cwd(92, "/usr/bin/kimi", "kimi", cwd)];
        let readers: Vec<&dyn StoreReader> = Vec::new();
        let result = scan_with(&readers, &scratch.0, &table, now_ms());
        let hits: Vec<_> = result.hits.iter().filter(|h| h.source == "kimi").collect();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].session_id, "legacy-1");
        assert_eq!(hits[0].recent_messages.len(), 1);
    }

    /// #282: Kiro's V3 layout puts the session under a workspace directory, and the
    /// workspace list is what says it belongs to this cwd.
    #[test]
    fn a_kiro_v3_session_becomes_a_session() {
        let scratch = Scratch::new("kiro-v3");
        let cwd = "/home/dev/project";
        let session = scratch.0.join(".kiro/sessions/workspace-hash/kiro-1");
        std::fs::create_dir_all(&session).expect("a store");
        std::fs::write(
            session.join("session.json"),
            format!(r#"{{"id":"kiro-1","workspacePaths":["{cwd}"],"modelId":"claude-sonnet"}}"#),
        )
        .expect("a session file");
        std::fs::write(
            session.join("messages.jsonl"),
            concat!(
                r#"{"payload":{"type":"user","content":"Explain this repo"}}"#,
                "\n",
                r#"{"payload":{"type":"assistant","content":"It is a notch app"}}"#
            ),
        )
        .expect("a transcript");

        let table = vec![process_with_cwd(101, "/usr/bin/kiro-cli", "kiro-cli", cwd)];
        let readers: Vec<&dyn StoreReader> = Vec::new();
        let result = scan_with(&readers, &scratch.0, &table, now_ms());
        let hits: Vec<_> = result.hits.iter().filter(|h| h.source == "kiro").collect();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].session_id, "kiro-1");
        assert_eq!(hits[0].model.as_deref(), Some("claude-sonnet"));
        assert_eq!(hits[0].recent_messages.len(), 2);
    }

    /// The V2 layout keeps one file per session under `sessions/cli`, and `cli` is
    /// not a workspace.
    #[test]
    fn a_kiro_v2_session_becomes_a_session() {
        let scratch = Scratch::new("kiro-v2");
        let cwd = "/home/dev/project";
        let cli = scratch.0.join(".kiro/sessions/cli");
        std::fs::create_dir_all(&cli).expect("a store");
        std::fs::write(
            cli.join("kiro-legacy.json"),
            format!(r#"{{"cwd":"{cwd}","updated_at":"{}"}}"#, iso8601(now_ms())),
        )
        .expect("a session file");
        std::fs::write(
            cli.join("kiro-legacy.jsonl"),
            r#"{"type":"user","message":{"content":[{"type":"text","text":"Hello"}]}}"#,
        )
        .expect("a transcript");

        let table = vec![process_with_cwd(102, "/usr/bin/kiro-cli", "kiro-cli", cwd)];
        let readers: Vec<&dyn StoreReader> = Vec::new();
        let result = scan_with(&readers, &scratch.0, &table, now_ms());
        let hits: Vec<_> = result.hits.iter().filter(|h| h.source == "kiro").collect();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].session_id, "kiro-legacy");
        assert_eq!(hits[0].recent_messages.len(), 1);
    }

    /// A session in the same workspace that last moved long before the process
    /// began is not what it is running.
    #[test]
    fn a_kiro_session_older_than_its_process_is_skipped() {
        let scratch = Scratch::new("kiro-old");
        let cwd = "/home/dev/project";
        let session = scratch.0.join(".kiro/sessions/shared/kiro-old");
        std::fs::create_dir_all(&session).expect("a store");
        std::fs::write(
            session.join("session.json"),
            format!(
                r#"{{"id":"kiro-old","workspacePaths":["{cwd}"],"lastModifiedAt":"{}"}}"#,
                iso8601(now_ms() - 86_400_000)
            ),
        )
        .expect("a session file");

        let mut process = process_with_cwd(103, "/usr/bin/kiro-cli", "kiro-cli", cwd);
        // Started long after that session last moved.
        process.start_time_ms = Some(now_ms());
        let readers: Vec<&dyn StoreReader> = Vec::new();
        assert!(
            scan_with(&readers, &scratch.0, &[process], now_ms())
                .hits
                .iter()
                .all(|h| h.source != "kiro")
        );
    }

    /// An ISO 8601 stamp for a millisecond instant, so a fresh store can be seeded
    /// without a date crate.
    fn iso8601(ms: u64) -> String {
        let seconds = ms / 1000;
        let days = seconds / 86_400;
        let rest = seconds % 86_400;
        let (year, month, day) = civil_from_days(days as i64);
        format!(
            "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
            rest / 3_600,
            (rest % 3_600) / 60,
            rest % 60,
            ms % 1000
        )
    }

    /// Howard Hinnant's `civil_from_days`, the inverse of the core's parser.
    fn civil_from_days(days: i64) -> (i64, u32, u32) {
        let z = days + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
    }

    /// A subagent worktree is not a project the user opened.
    #[test]
    fn a_flat_store_process_in_a_worktree_is_skipped() {
        let scratch = Scratch::new("flat-worktree");
        let cwd = "/home/dev/project/worktrees/agent-1";
        let project = scratch
            .0
            .join(".qoder/projects")
            .join(morrow_core::discovery::project_dir::encoded(cwd));
        std::fs::create_dir_all(&project).expect("a store");
        std::fs::write(project.join("q-1.jsonl"), "{}").expect("a transcript");

        let table = vec![process_with_cwd(12, "/usr/bin/qoder", "qoder", cwd)];
        let readers: Vec<&dyn StoreReader> = Vec::new();
        let result = scan_with(&readers, &scratch.0, &table, now_ms());
        assert!(result.hits.iter().all(|h| h.source != "qoder"));
    }

    fn process_with_cwd(pid: i32, executable: &str, name: &str, cwd: &str) -> ProcessInfo {
        ProcessInfo {
            pid,
            executable: executable.to_owned(),
            arguments: vec![name.to_owned()],
            cwd: Some(cwd.to_owned()),
            start_time_ms: Some(1),
        }
    }

    /// A half-written or foreign file is skipped, not turned into a card with an
    /// empty prompt.
    #[test]
    fn an_unusable_file_is_skipped() {
        let scratch = Scratch::new("unusable");
        let projects = scratch.project_dir(".claude", "-home-dev-project");
        write(&projects.join("empty.jsonl"), "");
        write(&projects.join("junk.jsonl"), "not json\n");
        write(&projects.join("notes.txt"), "hello\n");

        assert!(
            scan_once(&scratch.0.join(".claude"), &scratch.0)
                .hits
                .is_empty()
        );
    }

    /// A session written after the app was closed is found and its newest turn is
    /// what the card shows — the whole point of running at attach.
    #[test]
    fn the_newest_session_is_the_one_a_panel_shows_first() {
        let scratch = Scratch::new("newest");
        let projects = scratch.project_dir(".claude", "-home-dev-project");
        write(
            &projects.join("older.jsonl"),
            &transcript("/home/dev/project", "model-x"),
        );
        // A second session, written later.
        std::thread::sleep(std::time::Duration::from_millis(2));
        write(
            &projects.join("newer.jsonl"),
            &transcript("/home/dev/project", "model-y"),
        );

        let mut hits = scan_once(&scratch.0.join(".claude"), &scratch.0).hits;
        hits.sort_by_key(|hit| hit.modified_at);
        assert_eq!(
            hits.last().map(|hit| hit.session_id.as_str()),
            Some("newer")
        );
    }

    /// Two accounts under the same home are two roots, and a session in each is
    /// found once — not once per root.
    #[test]
    fn a_second_account_under_the_same_home_is_its_own_root() {
        let scratch = Scratch::new("accounts");
        let primary = scratch.project_dir(".claude", "-home-dev-project");
        let work = scratch.project_dir(".claude-work", "-home-dev-project");
        write(
            &primary.join("personal.jsonl"),
            &transcript("/home/dev/project", "model-x"),
        );
        write(
            &work.join("work.jsonl"),
            &transcript("/home/dev/project", "model-x"),
        );

        let roots = claude_roots(&scratch.0.join(".claude"));
        assert_eq!(roots.len(), 2, "roots: {roots:?}");

        let mut ids: Vec<String> = scan_once(Path::new(&roots[0]), &scratch.0)
            .hits
            .into_iter()
            .map(|hit| hit.session_id)
            .collect();
        ids.sort();
        assert_eq!(ids, vec!["personal", "work"]);
    }

    /// The tail is honoured, not the whole file: a session's last three turns come
    /// from the end.
    #[test]
    fn only_the_tail_of_a_large_transcript_is_read() {
        let scratch = Scratch::new("tail");
        let projects = scratch.project_dir(".claude", "-home-dev-project");
        let path = projects.join("big.jsonl");
        // Well past TAIL_BYTES of filler, then the real turns.
        for _ in 0..200 {
            write(
                &path,
                &format!(
                    "{}\n",
                    r#"{"type":"assistant","cwd":"/home/dev/project","message":{"content":[{"type":"text","text":"filler line with some length to it"}]}}"#
                ),
            );
        }
        write(&path, &transcript("/home/dev/project", "model-x"));

        let hits = scan_once(&scratch.0.join(".claude"), &scratch.0).hits;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].model.as_deref(), Some("model-x"));
        assert_eq!(hits[0].cwd, "/home/dev/project");
    }
}
