//! The session list that survives a restart, and the titles it brings back
//! (`macos/Sources/Morrow/SessionPersistence.swift`, `SessionTitleStore.swift`).
//!
//! Two questions, both answered without touching a clock or the environment: what a
//! session looks like on disk, and where that file lives. The shell injects the second.
//!
//! Three rules worth stating, because each one was a bug somewhere in the macOS tree:
//!
//! - **A file this build cannot read costs one session, not all of them.** Entries are
//!   decoded one at a time, so a field from a newer build after a downgrade drops that
//!   card instead of emptying the panel.
//! - **Only local sessions are written.** A remote session has no window here, and a
//!   Cowork card is rebuilt from Claude Desktop's own store — persisting it brings back
//!   a ghost Claude Code session that nobody can vouch for.
//! - **Prompt and reply are capped at 64 KB.** A pasted 9 MB log made every save — on
//!   the main thread, every couple of seconds — encode it again.
//!
//! Two things macOS writes are **not** written here: `agentTasks` and `recap`. Both are
//! rebuilt from the transcript by the shell's backfill (#265); macOS persists the
//! checklist only so a card keeps its progress for the few hundred milliseconds before
//! that backfill lands. Their Rust types carry no `Serialize`, and growing one just to
//! write a cache that is about to be recomputed is the wrong trade.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::reduce::{SessionSnapshot, Timestamp};
use crate::time::{format_iso8601, parse_iso8601};

/// `AppState.coworkSessionPrefix`: a Cowork card is Claude Desktop's, not ours.
pub const COWORK_SESSION_PREFIX: &str = "cowork:";

/// The most of a prompt or reply the file keeps.
pub const MAX_PERSISTED_TEXT_LENGTH: usize = 64 * 1024;

/// `SessionPersistence.fileName`.
pub const FILE_NAME: &str = "sessions.json";

/// Where the title a session shows came from (`SessionTitleSource`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionTitleSource {
    CodexThreadName,
    ClaudeCustomTitle,
    ClaudeAiTitle,
}

/// A title and the store that named it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedSessionTitle {
    pub title: String,
    pub source: SessionTitleSource,
}

/// One session as it is written out. Field names are the Swift ones (camelCase on the
/// wire), because a `sessions.json` from the macOS build must still read here.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedSession {
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default)]
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_title_source: Option<SessionTitleSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_user_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_assistant_message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub term_app: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iterm_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tty_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kitty_window_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmux_pane: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmux_client_tty: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmux_env: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub term_bundle_id: Option<String>,
    // Multiplexer / fork pane hints, preserved so a precise jump-back keeps working
    // after a restart instead of degrading to the cwd/tty fallback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cmux_surface_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cmux_workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zellij_pane_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zellij_session_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wezterm_pane_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub herdr_pane_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub herdr_socket_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub herdr_binary_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cli_pid: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cli_start_time: Option<String>,
    pub start_time: String,
    pub last_activity: String,
    /// Absolute JSONL path for session fold and transcript tailing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<String>,
    /// Closed subagent ids in insertion order, newest last.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_subagent_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude_desktop_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hermes_chat_elsewhere: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_session_id: Option<String>,
    /// Fields written by the macOS build that this port does not manage. They
    /// must survive a save, or a 2-second timer silently deletes the user's
    /// agentTasks/recap/title-source data (#339).
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Whether a card is written out for the next launch.
pub fn is_persisted(session_id: &str, session: &SessionSnapshot) -> bool {
    !session.is_remote() && !session_id.starts_with(COWORK_SESSION_PREFIX)
}

/// `text` capped at [`MAX_PERSISTED_TEXT_LENGTH`] bytes, on a character boundary.
pub fn capped_text(text: Option<&str>) -> Option<String> {
    let text = text?;
    if text.len() <= MAX_PERSISTED_TEXT_LENGTH {
        return Some(text.to_owned());
    }
    let mut end = MAX_PERSISTED_TEXT_LENGTH;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    Some(text[..end].to_owned())
}

fn option(text: Option<&str>) -> Option<String> {
    text.filter(|text| !text.is_empty()).map(str::to_owned)
}

/// The row written for one session.
pub fn to_persisted(session_id: &str, s: &SessionSnapshot) -> PersistedSession {
    PersistedSession {
        extra: BTreeMap::new(),
        session_id: session_id.to_owned(),
        cwd: option(s.cwd.as_deref()),
        source: s.source.clone(),
        model: option(s.model.as_deref()),
        session_title: option(s.session_title.as_deref()),
        session_title_source: None,
        provider_session_id: option(s.provider_session_id.as_deref()),
        last_user_prompt: capped_text(s.last_user_prompt.as_deref()),
        last_assistant_message: capped_text(s.last_assistant_message.as_deref()),
        term_app: option(s.term_app.as_deref()),
        iterm_session_id: option(s.iterm_session_id.as_deref()),
        tty_path: option(s.tty_path.as_deref()),
        kitty_window_id: option(s.kitty_window_id.as_deref()),
        tmux_pane: option(s.tmux_pane.as_deref()),
        tmux_client_tty: option(s.tmux_client_tty.as_deref()),
        tmux_env: option(s.tmux_env.as_deref()),
        term_bundle_id: option(s.term_bundle_id.as_deref()),
        cmux_surface_id: option(s.cmux_surface_id.as_deref()),
        cmux_workspace_id: option(s.cmux_workspace_id.as_deref()),
        zellij_pane_id: option(s.zellij_pane_id.as_deref()),
        zellij_session_name: option(s.zellij_session_name.as_deref()),
        wezterm_pane_id: option(s.wezterm_pane_id.as_deref()),
        herdr_pane_id: option(s.herdr_pane_id.as_deref()),
        herdr_socket_path: option(s.herdr_socket_path.as_deref()),
        herdr_binary_path: option(s.herdr_binary_path.as_deref()),
        cli_pid: s.cli_pid,
        cli_start_time: s.cli_start_time.map(format_iso8601),
        start_time: format_iso8601(s.start_time),
        last_activity: format_iso8601(s.last_activity),
        transcript_path: option(s.transcript_path.as_deref()),
        closed_subagent_ids: {
            let ids = s.closed_subagent_ids();
            (!ids.is_empty()).then(|| ids.to_vec())
        },
        reasoning_effort: option(s.reasoning_effort.as_deref()),
        claude_desktop_session_id: option(s.claude_desktop_session_id.as_deref()),
        hermes_chat_elsewhere: s.hermes_chat_elsewhere.then_some(true),
        parent_session_id: option(s.parent_session_id.as_deref()),
    }
}

fn instant(value: &str, fallback: Timestamp) -> Timestamp {
    parse_iso8601(value).unwrap_or(fallback)
}

/// The session a restored row rebuilds. Everything the card draws comes back; what a
/// hook would have to say about it (status, current tool) does not, because a card
/// restored from disk is idle until the next event says otherwise.
pub fn into_snapshot(p: &PersistedSession) -> SessionSnapshot {
    let last_activity = instant(&p.last_activity, Timestamp(0.0));
    let start_time = instant(&p.start_time, last_activity);
    let mut snapshot = SessionSnapshot::new(start_time);
    snapshot.last_activity = last_activity;
    snapshot.cwd = p.cwd.clone();
    snapshot.model = p.model.clone();
    snapshot.source = p.source.clone();
    snapshot.session_title = p.session_title.clone();
    snapshot.provider_session_id = p.provider_session_id.clone();
    snapshot.last_user_prompt = p.last_user_prompt.clone();
    snapshot.last_assistant_message = p.last_assistant_message.clone();
    snapshot.term_app = p.term_app.clone();
    snapshot.iterm_session_id = p.iterm_session_id.clone();
    snapshot.tty_path = p.tty_path.clone();
    snapshot.kitty_window_id = p.kitty_window_id.clone();
    snapshot.tmux_pane = p.tmux_pane.clone();
    snapshot.tmux_client_tty = p.tmux_client_tty.clone();
    snapshot.tmux_env = p.tmux_env.clone();
    snapshot.term_bundle_id = p.term_bundle_id.clone();
    snapshot.cmux_surface_id = p.cmux_surface_id.clone();
    snapshot.cmux_workspace_id = p.cmux_workspace_id.clone();
    snapshot.zellij_pane_id = p.zellij_pane_id.clone();
    snapshot.zellij_session_name = p.zellij_session_name.clone();
    snapshot.wezterm_pane_id = p.wezterm_pane_id.clone();
    snapshot.herdr_pane_id = p.herdr_pane_id.clone();
    snapshot.herdr_socket_path = p.herdr_socket_path.clone();
    snapshot.herdr_binary_path = p.herdr_binary_path.clone();
    snapshot.cli_pid = p.cli_pid;
    snapshot.cli_start_time = p.cli_start_time.as_deref().and_then(parse_iso8601);
    snapshot.transcript_path = p.transcript_path.clone();
    snapshot.reasoning_effort = p.reasoning_effort.clone();
    snapshot.claude_desktop_session_id = p.claude_desktop_session_id.clone();
    snapshot.hermes_chat_elsewhere = p.hermes_chat_elsewhere.unwrap_or(false);
    snapshot.parent_session_id = p.parent_session_id.clone();
    if let Some(ids) = &p.closed_subagent_ids {
        snapshot.restore_closed_subagent_ids(ids);
    }
    snapshot
}

/// The file's text for a list.
pub fn encode(list: &[PersistedSession]) -> String {
    let mut text = serde_json::to_string_pretty(list).unwrap_or_else(|_| "[]".to_owned());
    text.push('\n');
    text
}

/// Decode entry by entry: one this build cannot read costs that session, not the file.
pub fn decode(text: &str) -> Vec<PersistedSession> {
    let Ok(entries) = serde_json::from_str::<Vec<serde_json::Value>>(text) else {
        return Vec::new();
    };
    entries
        .into_iter()
        .filter_map(|entry| serde_json::from_value(entry).ok())
        .collect()
}

/// The first index that names the session wins, matching
/// `SessionTitleStore.codexThreadName(sessionId:)`: each Codex root keeps its
/// own `session_index.jsonl` (#339).
pub fn codex_thread_name_from_roots(indexes: &[&str], session_id: &str) -> Option<String> {
    indexes
        .iter()
        .find_map(|index| codex_thread_name(index, session_id))
}

/// Where `sessions.json` lives: `~/.morrow`, unless an override names another folder.
///
/// The override and the legacy folder are injected rather than read from the process,
/// so a test can exercise them; the copy is `~/.codeisland` → `~/.morrow` for a machine
/// upgrading from the old name, and only when `~/.morrow` does not exist yet.
pub fn directory(override_dir: Option<&str>, home: &Path) -> PathBuf {
    if let Some(dir) = override_dir.map(str::trim).filter(|dir| !dir.is_empty()) {
        return expand(dir, home);
    }
    let morrow_dir = home.join(".morrow");
    let legacy_dir = home.join(".codeisland");
    if !morrow_dir.exists() && legacy_dir.is_dir() {
        copy_tree(&legacy_dir, &morrow_dir);
    }
    morrow_dir
}

/// `~` and `~/x` mean the home directory; anything else is taken as written.
pub fn expand(raw: &str, home: &Path) -> PathBuf {
    if raw == "~" {
        return home.to_path_buf();
    }
    match raw.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None => PathBuf::from(raw),
    }
}

fn copy_tree(from: &Path, to: &Path) {
    let Ok(entries) = std::fs::read_dir(from) else {
        return;
    };
    if std::fs::create_dir_all(to).is_err() {
        return;
    }
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            let _ = std::fs::copy(&path, to.join(entry.file_name()));
        }
    }
}

/// How often the shell may rewrite the list. macOS saves on a 2 s timer: the file holds
/// every prompt and reply, and encoding it on every hook event is work nobody sees.
pub const SAVE_GAP_MS: u64 = 2_000;

/// The same rule as the scan throttle, for the same reason: a save that cannot change
/// the next launch is a write on the main thread for nothing.
#[derive(Debug, Default)]
pub struct SaveThrottle {
    last: Option<u64>,
}

impl SaveThrottle {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a save is due at `now_ms`, marking it. The first call is always due.
    pub fn due(&mut self, now_ms: u64) -> bool {
        match self.last {
            Some(last) if now_ms.saturating_sub(last) < SAVE_GAP_MS => false,
            _ => {
                self.last = Some(now_ms);
                true
            }
        }
    }

    pub fn reset(&mut self) {
        self.last = None;
    }
}

/// The full path of the session file inside `dir`.
pub fn file_path(dir: &Path) -> PathBuf {
    dir.join(FILE_NAME)
}

/// Write the list, creating the folder. Failure is not an error to report: a session
/// list that cannot be written must not take the panel down with it.
pub fn save(dir: &Path, list: &[PersistedSession]) {
    let _ = std::fs::create_dir_all(dir);
    let path = file_path(dir);
    let prior = load(dir);
    let merged: Vec<PersistedSession> = list
        .iter()
        .map(
            |row| match prior.iter().find(|old| old.session_id == row.session_id) {
                Some(old) => merge_unmanaged(row, old),
                None => row.clone(),
            },
        )
        .collect();
    if std::fs::write(&path, encode(&merged)).is_ok() {
        restrict(&path);
    }
}

/// Keep anything the fresh row leaves unset (or never declares) from the row
/// already on disk. Working on the JSON form covers both declared Option fields
/// and fields only the macOS build knows about.
fn merge_unmanaged(fresh: &PersistedSession, old: &PersistedSession) -> PersistedSession {
    let Ok(mut value) = serde_json::to_value(fresh) else {
        return fresh.clone();
    };
    let Ok(previous) = serde_json::to_value(old) else {
        return fresh.clone();
    };
    if let (Some(current), Some(previous)) = (value.as_object_mut(), previous.as_object()) {
        for (key, old_value) in previous {
            let unset = current.get(key).is_none_or(Value::is_null);
            if unset && !old_value.is_null() {
                current.insert(key.clone(), old_value.clone());
            }
        }
    }
    serde_json::from_value(value).unwrap_or_else(|_| fresh.clone())
}

/// Read the list. A missing or unreadable file is an empty list, not an error.
pub fn load(dir: &Path) -> Vec<PersistedSession> {
    let path = file_path(dir);
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            // Restore the private-file promise too: a `sessions.json` copied in
            // from another build (or an older install) may be world-readable.
            restrict(&path);
            decode(&text)
        }
        Err(_) => Vec::new(),
    }
}

/// `PrivateFiles`: 0600 in a 0700 folder, because the file holds what the user typed
/// and what the agent answered. A no-op where there are no Unix modes.
fn restrict(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        if let Some(dir) = path.parent() {
            let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
        }
    }
    #[cfg(not(unix))]
    let _ = path;
}

// MARK: - Titles

/// The providers whose own store names a session (`SessionTitleStore.supports`).
pub fn supports_title(provider: &str) -> bool {
    matches!(provider, "codex" | "claude")
}

/// The newest `thread_name` a Codex `session_index.jsonl` has for `session_id`.
///
/// The index is append-only and a renamed thread appears again, so the last line that
/// names this session wins — but only after comparing `updated_at`, because a file
/// written by two processes is not guaranteed to be in time order. A line without a
/// timestamp counts as the distant past, so any dated line beats it.
pub fn codex_thread_name(index_contents: &str, session_id: &str) -> Option<String> {
    let mut latest: Option<(Timestamp, String)> = None;
    for line in index_contents.lines() {
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if entry.get("id").and_then(|id| id.as_str()) != Some(session_id) {
            continue;
        }
        let Some(title) = entry
            .get("thread_name")
            .and_then(|name| name.as_str())
            .map(str::trim)
            .filter(|title| !title.is_empty())
        else {
            continue;
        };
        let updated_at = entry
            .get("updated_at")
            .and_then(|value| value.as_str())
            .and_then(parse_iso8601)
            .unwrap_or(Timestamp(f64::NEG_INFINITY));
        match &latest {
            Some((best, _)) if *best > updated_at => {}
            _ => latest = Some((updated_at, title.to_owned())),
        }
    }
    latest.map(|(_, title)| title)
}

/// The newest `customTitle` and `aiTitle` a Claude transcript carries.
///
/// Two line types answer this (`custom-title`, `ai-title`) and a session can have both,
/// so they are kept apart: a title the user typed outranks one the model suggested.
pub fn claude_titles(contents: &str) -> (Option<String>, Option<String>) {
    let mut custom = None;
    let mut ai = None;
    for line in contents.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let (kind, key) = match value.get("type").and_then(|kind| kind.as_str()) {
            Some("custom-title") => (&mut custom, "customTitle"),
            Some("ai-title") => (&mut ai, "aiTitle"),
            _ => continue,
        };
        if let Some(title) = value
            .get(key)
            .and_then(|title| title.as_str())
            .map(str::trim)
            .filter(|title| !title.is_empty())
        {
            *kind = Some(title.to_owned());
        }
    }
    (custom, ai)
}

/// The title of a Claude session from the head and tail of its transcript.
///
/// The tail is read because a rename lands at the end of the file; the head is read
/// because a session whose tail is one enormous tool result has its title only at the
/// start. Tail first, then head.
pub fn claude_title(head: &str, tail: &str) -> Option<ResolvedSessionTitle> {
    let (tail_custom, tail_ai) = claude_titles(tail);
    let (head_custom, head_ai) = claude_titles(head);
    if let Some(title) = tail_custom.or(head_custom) {
        return Some(ResolvedSessionTitle {
            title,
            source: SessionTitleSource::ClaudeCustomTitle,
        });
    }
    let title = tail_ai.or(head_ai)?;
    Some(ResolvedSessionTitle {
        title,
        source: SessionTitleSource::ClaudeAiTitle,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> SessionSnapshot {
        let mut s = SessionSnapshot::new(Timestamp(1_000.0));
        s.source = "claude".to_owned();
        s.cwd = Some("/home/dev/proj".to_owned());
        s.model = Some("opus".to_owned());
        s.last_user_prompt = Some("add filters".to_owned());
        s.last_assistant_message = Some("adding them".to_owned());
        s.tty_path = Some("/dev/ttys003".to_owned());
        s.agent_tasks = crate::agent_tasks::AgentTaskList::default();
        s
    }

    #[test]
    fn a_session_survives_the_round_trip() {
        let s = snapshot();
        let row = to_persisted("a", &s);
        let text = encode(std::slice::from_ref(&row));
        let back = decode(&text);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].session_id, "a");
        let restored = into_snapshot(&back[0]);
        assert_eq!(restored.cwd, s.cwd);
        assert_eq!(restored.source, "claude");
        assert_eq!(restored.last_assistant_message, s.last_assistant_message);
        assert_eq!(restored.start_time, Timestamp(1_000.0));
        assert_eq!(restored.last_activity, Timestamp(1_000.0));
    }

    #[test]
    fn a_remote_session_and_a_cowork_card_are_not_written() {
        let mut remote = snapshot();
        remote.claude_desktop_session_id = None;
        remote.remote_host_id = Some("host".to_owned());
        assert!(!is_persisted("a", &remote));
        assert!(!is_persisted("cowork:store1", &snapshot()));
        assert!(is_persisted("a", &snapshot()));
    }

    #[test]
    fn a_huge_prompt_is_capped_on_a_character_boundary() {
        let long = format!("{}é", "a".repeat(MAX_PERSISTED_TEXT_LENGTH - 1));
        let capped = capped_text(Some(&long)).expect("capped");
        assert_eq!(capped.len(), MAX_PERSISTED_TEXT_LENGTH - 1);
        assert!(long.starts_with(&capped));

        let short = "hello";
        assert_eq!(capped_text(Some(short)), Some(short.to_owned()));
        assert_eq!(capped_text(None), None);
    }

    #[test]
    fn one_unreadable_entry_costs_only_itself() {
        let row = to_persisted("a", &snapshot());
        let mut text = encode(std::slice::from_ref(&row));
        // A newer build's entry this one cannot read, beside a readable one.
        text = text.replacen('[', "[{\"sessionId\": \"b\", \"startTime\": 5},", 1);
        let back = decode(&text);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].session_id, "a");
        assert!(decode("not json").is_empty());
    }

    #[test]
    fn an_override_directory_beats_the_default_and_expands_a_tilde() {
        let home = Path::new("/home/dev");
        assert_eq!(
            directory(Some("/tmp/sessions"), home),
            PathBuf::from("/tmp/sessions")
        );
        assert_eq!(directory(Some("~/work"), home), home.join("work"));
        assert_eq!(directory(Some("   "), home), home.join(".morrow"));
        assert_eq!(directory(None, home), home.join(".morrow"));
    }

    #[test]
    fn the_old_folder_is_copied_once_when_the_new_one_is_absent() {
        let root = tempdir("legacy");
        let home = root.join("home");
        std::fs::create_dir_all(home.join(".codeisland")).unwrap();
        std::fs::write(home.join(".codeisland/sessions.json"), "[]\n").unwrap();

        let dir = directory(None, &home);
        assert_eq!(dir, home.join(".morrow"));
        assert!(dir.join("sessions.json").is_file(), "copied");

        // A second lookup does not overwrite what the new folder now holds.
        std::fs::write(dir.join("sessions.json"), "[{\"sessionId\":\"a\"}]\n").unwrap();
        directory(None, &home);
        assert_eq!(
            std::fs::read_to_string(dir.join("sessions.json")).unwrap(),
            "[{\"sessionId\":\"a\"}]\n"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn saving_and_loading_on_disk_keeps_the_list() {
        let root = tempdir("save");
        let rows = vec![
            to_persisted("a", &snapshot()),
            to_persisted("b", &snapshot()),
        ];
        save(&root, &rows);
        let back = load(&root);
        assert_eq!(back.len(), 2);
        assert_eq!(back[1].session_id, "b");
        assert!(load(&root.join("nothing-here")).is_empty());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_save_keeps_fields_the_mac_build_wrote() {
        let root = tempdir("preserve");
        std::fs::write(
            file_path(&root),
            r#"[{"sessionId":"a","startTime":"2026-01-01T00:00:00Z","lastActivity":"2026-01-01T00:00:00Z","sessionTitleSource":"claudeCustomTitle","agentTasks":{"open":3},"recap":{"text":"user note"}}]"#,
        )
        .expect("a macOS-written file");
        save(&root, std::slice::from_ref(&to_persisted("a", &snapshot())));

        let text = std::fs::read_to_string(file_path(&root)).expect("the saved file");
        assert!(text.contains("agentTasks"), "{text}");
        assert!(text.contains("recap"), "{text}");
        let back = load(&root);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].extra["agentTasks"]["open"], 3);
        assert_eq!(back[0].extra["recap"]["text"], "user note");
        assert_eq!(
            back[0].session_title_source,
            Some(SessionTitleSource::ClaudeCustomTitle)
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn loading_a_world_readable_file_restores_private_modes() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempdir("restore-perms");
        std::fs::write(file_path(&root), "[]\n").expect("a file");
        std::fs::set_permissions(file_path(&root), std::fs::Permissions::from_mode(0o644))
            .expect("world readable");
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755))
            .expect("world readable dir");
        assert!(load(&root).is_empty());
        let mode = std::fs::metadata(file_path(&root))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        let dir_mode = std::fs::metadata(&root).unwrap().permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700);
        std::fs::remove_dir_all(&root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn the_file_is_the_users_alone() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempdir("private");
        save(&root, std::slice::from_ref(&to_persisted("a", &snapshot())));
        let mode = std::fs::metadata(file_path(&root))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        let dir_mode = std::fs::metadata(&root).unwrap().permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_title_is_resolved_from_the_first_index_that_names_it() {
        let primary = "{\"id\":\"b\",\"thread_name\":\"from-primary\"}\n";
        let secondary = "{\"id\":\"a\",\"thread_name\":\"from-secondary\"}\n";
        assert_eq!(
            codex_thread_name_from_roots(&[primary, secondary], "a"),
            Some("from-secondary".to_owned())
        );
        assert_eq!(
            codex_thread_name_from_roots(&[primary, secondary], "b"),
            Some("from-primary".to_owned())
        );
        assert_eq!(codex_thread_name_from_roots(&[primary], "a"), None);
    }

    #[test]
    fn the_newest_thread_name_wins_and_a_blank_one_is_ignored() {
        let index = "\
{\"id\":\"a\",\"thread_name\":\"first\",\"updated_at\":\"2026-01-01T00:00:00Z\"}
{\"id\":\"b\",\"thread_name\":\"other\",\"updated_at\":\"2026-01-02T00:00:00Z\"}
{\"id\":\"a\",\"thread_name\":\"  \",\"updated_at\":\"2026-01-05T00:00:00Z\"}
{\"id\":\"a\",\"thread_name\":\"renamed\",\"updated_at\":\"2026-01-03T00:00:00Z\"}
";
        assert_eq!(codex_thread_name(index, "a"), Some("renamed".to_owned()));
        // A dated line beats one with no timestamp, whatever the order.
        let mixed = "\
{\"id\":\"a\",\"thread_name\":\"undated\"}
{\"id\":\"a\",\"thread_name\":\"dated\",\"updated_at\":\"2000-01-01T00:00:00Z\"}
";
        assert_eq!(codex_thread_name(mixed, "a"), Some("dated".to_owned()));
        assert_eq!(codex_thread_name(index, "missing"), None);
        assert_eq!(codex_thread_name("garbage", "a"), None);
    }

    #[test]
    fn a_title_the_user_typed_beats_the_one_the_model_suggested() {
        let head = "{\"type\":\"ai-title\",\"aiTitle\":\"Suggested\"}\n";
        let tail = "{\"type\":\"custom-title\",\"customTitle\":\"Mine\"}\n";
        let title = claude_title(head, tail).expect("a title");
        assert_eq!(title.title, "Mine");
        assert_eq!(title.source, SessionTitleSource::ClaudeCustomTitle);

        // Only in the head: the tail lost it to a long tool result.
        let title = claude_title(head, "").expect("a title");
        assert_eq!(title.source, SessionTitleSource::ClaudeAiTitle);

        // The tail's newer rename wins over the head's.
        let tail = "{\"type\":\"ai-title\",\"aiTitle\":\"Newer\"}\n";
        let head = "{\"type\":\"ai-title\",\"aiTitle\":\"Older\"}\n";
        assert_eq!(claude_title(head, tail).expect("a title").title, "Newer");

        assert!(claude_title("", "").is_none());
        assert!(
            claude_title("{\"type\":\"custom-title\",\"customTitle\":\"   \"}\n", "").is_none()
        );
    }

    #[test]
    fn the_save_throttle_holds_for_the_gap_and_saves_again_after_it() {
        let mut throttle = SaveThrottle::new();
        assert!(throttle.due(1_000), "the first save is always due");
        assert!(!throttle.due(1_500));
        assert!(!throttle.due(2_999));
        assert!(throttle.due(3_000));
        throttle.reset();
        assert!(throttle.due(3_001), "a reset makes the next one due");
    }

    #[test]
    fn only_codex_and_claude_name_their_own_sessions() {
        assert!(supports_title("codex"));
        assert!(supports_title("claude"));
        assert!(!supports_title("cursor"));
        assert!(!supports_title(""));
    }

    fn tempdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "morrow-session-store-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
