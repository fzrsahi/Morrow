//! Discovery readers (#245): the pure half of the per-agent store readers.
//!
//! The platform walks the filesystem, opens the SQLite files and asks the
//! process table; this module only turns the bytes it is handed into a
//! [`DiscoveryHit`]. No `std::fs`, no SQLite, no process table — that is what
//! makes the readers testable against fixtures exported from Swift.
//!
//! The three readers here are the ones whose whole job is a text-to-value
//! transform: Cursor's transcript, Gemini's session file, and the model line of
//! a [CC] transcript. The rest of `Discovery/` follows the same shape; see #232.

use crate::reduce::ChatMessage;

/// One session a store reader found. The fields are `DiscoveredSession`'s, minus
/// the bookkeeping the shell adds (parent, subagent status) — a reader knows
/// what it read, not what the panel will do with it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiscoveryHit {
    pub session_id: String,
    /// The agent's own name for itself, as `--source` spells it.
    pub source: &'static str,
    pub cwd: String,
    pub tty: Option<String>,
    pub model: Option<String>,
    pub pid: Option<i32>,
    /// Milliseconds since the Unix epoch, from the store's own mtime.
    pub modified_at: Option<u64>,
    /// Newest last, at most three: what a card shows without opening anything.
    pub recent_messages: Vec<ChatMessage>,
    /// The file this came from, so a live tail can attach to it.
    pub transcript_path: Option<String>,
}

/// `ProjectDirEncoding.swift`: how Claude Code names a project directory, and how
/// the app-derived stores (`~/.cursor/projects`, `~/.gemini/tmp`) name theirs.
///
/// `/`, a space and every non-ASCII scalar become `-`. Note what does **not**:
/// a Windows backslash is ASCII and is not a separator here, so
/// `C:\Users\dev\morrow` encodes to itself. That is the Swift behaviour, and a
/// port that "fixed" it would look for a directory that does not exist.
pub mod project_dir {
    /// `claudeProjectDirEncoded()`.
    pub fn encoded(path: &str) -> String {
        path.chars()
            .map(|c| {
                if c == '/' || c == ' ' || !c.is_ascii() {
                    '-'
                } else {
                    c
                }
            })
            .collect()
    }

    /// `appProjectDirEncoded()`: the same, without the leading `-` a rooted path
    /// produces.
    pub fn app_encoded(path: &str) -> String {
        let encoded = encoded(path);
        encoded.strip_prefix('-').unwrap_or(&encoded).to_owned()
    }
}

/// `SessionDiscovery.extractTextContent(from:)`: a message's text, whether the
/// store wrote a bare string or the `[{"type":"text","text":…}]` shape.
pub fn extract_text_content(content: Option<&serde_json::Value>) -> Option<String> {
    use serde_json::Value;
    let content = content?;
    if let Some(text) = content.as_str() {
        return (!text.is_empty()).then(|| text.to_owned());
    }
    let items = content.as_array()?;
    for item in items {
        if let Some(text) = item.get("text").and_then(Value::as_str) {
            if !text.is_empty() {
                return Some(text.to_owned());
            }
        }
        if let Some(text) = item
            .get("output")
            .and_then(|output| output.get("text"))
            .and_then(Value::as_str)
        {
            if !text.is_empty() {
                return Some(text.to_owned());
            }
        }
    }
    None
}

/// `ClaudeDiscovery.model(fromTranscript:)`: the first assistant row that names
/// a model. An empty `model` is not a name, so the next row gets the chance.
pub mod claude {
    use serde_json::Value;

    pub fn model(transcript: &str) -> Option<String> {
        for line in transcript.split('\n') {
            if line.is_empty() {
                continue;
            }
            let Ok(json) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let Some(model) = json.get("message").and_then(|m| m.get("model")) else {
                continue;
            };
            let Some(model) = model.as_str() else {
                continue;
            };
            if !model.is_empty() {
                return Some(model.to_owned());
            }
        }
        None
    }
}

/// `CursorDiscovery.readRecentFromCursorTranscript`: the last three turns of a
/// Cursor agent transcript, in the order they were written.
pub mod cursor {
    use serde_json::Value;

    use crate::reduce::ChatMessage;
    use crate::transcript::normalized_cursor_chat_text;

    use super::extract_text_content;

    pub fn recent_messages(transcript: &str) -> Vec<ChatMessage> {
        let mut turns: Vec<ChatMessage> = Vec::new();
        for line in transcript.split('\n') {
            if line.is_empty() {
                continue;
            }
            let Ok(json) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let Some(role) = json.get("role").and_then(Value::as_str) else {
                continue;
            };
            let content = json.get("message").and_then(|m| m.get("content"));
            let Some(text) =
                normalized_cursor_chat_text(content).or_else(|| extract_text_content(content))
            else {
                continue;
            };
            match role {
                "user" => turns.push(ChatMessage::new(true, &text)),
                "assistant" => turns.push(ChatMessage::new(false, &text)),
                _ => {}
            }
        }
        // The Swift reader takes three of each role, merges them by position and
        // keeps the last three overall; for a transcript that alternates, that
        // is the last three turns.
        let start = turns.len().saturating_sub(3);
        turns.split_off(start)
    }
}

/// `GeminiDiscovery.readRecentFromGeminiTranscript`: a session file's id, model
/// and last three turns.
pub mod gemini {
    use serde_json::Value;

    use crate::reduce::ChatMessage;

    use super::extract_text_content;

    fn parse(transcript: &str) -> Option<Value> {
        serde_json::from_str(transcript).ok()
    }

    pub fn session_id(transcript: &str) -> Option<String> {
        parse(transcript)?
            .get("sessionId")
            .and_then(Value::as_str)
            .map(str::to_owned)
    }

    pub fn model(transcript: &str) -> Option<String> {
        parse(transcript)?
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_owned)
    }

    pub fn recent_messages(transcript: &str) -> Vec<ChatMessage> {
        let Some(json) = parse(transcript) else {
            return Vec::new();
        };
        let Some(messages) = json.get("messages").and_then(Value::as_array) else {
            return Vec::new();
        };
        let mut turns: Vec<ChatMessage> = Vec::new();
        for message in messages {
            let kind = message
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_ascii_lowercase();
            let content = message.get("content");
            let Some(text) = extract_text_content(content) else {
                continue;
            };
            if text.is_empty() {
                continue;
            }
            // Everything that is not a user turn is the agent's own.
            turns.push(ChatMessage::new(kind == "user", &text));
        }
        let start = turns.len().saturating_sub(3);
        turns.split_off(start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_project_directory_replaces_slashes_spaces_and_non_ascii() {
        assert_eq!(
            project_dir::encoded("/Users/dev/projects/morrow"),
            "-Users-dev-projects-morrow"
        );
        assert_eq!(
            project_dir::encoded("/Users/dev/My Project"),
            "-Users-dev-My-Project"
        );
        assert_eq!(project_dir::encoded("/Users/dev/naïve"), "-Users-dev-na-ve");
        // A Windows path has no `/`, no space and no non-ASCII: it is unchanged.
        assert_eq!(
            project_dir::encoded(r"C:\Users\dev\morrow"),
            r"C:\Users\dev\morrow"
        );
        assert_eq!(project_dir::encoded("/"), "-");
    }

    #[test]
    fn the_app_encoding_drops_the_leading_dash() {
        assert_eq!(
            project_dir::app_encoded("/Users/dev/projects/morrow"),
            "Users-dev-projects-morrow"
        );
        // Nothing to strip: the path did not start with a separator.
        assert_eq!(
            project_dir::app_encoded(r"C:\Users\dev\morrow"),
            r"C:\Users\dev\morrow"
        );
        assert_eq!(project_dir::app_encoded("/"), "");
    }
}

/// The files a store reader lists before it reads any of them.
///
/// `ClaudeDiscovery.claudeTranscriptFiles`: the `.jsonl` transcripts in one
/// project dir with their mtimes, read in one bulk listing instead of a stat per
/// file.
///
/// Two details are the whole point of this being here rather than inline:
///
/// - The **directory** is resolved first. A project dir that is itself a symlink
///   fails a plain listing on macOS, and every multi-account setup that shares a
///   project across accounts has one.
/// - The **entries are not resolved**. A symlinked transcript still reports its
///   own mtime, which is what tells "this session is live" from "this session
///   was left behind" — resolving it would report the target's mtime and, worse,
///   make two spellings look like one session (#248).
pub mod transcript_files {
    use std::time::UNIX_EPOCH;

    use crate::path_identity;

    /// One transcript as the listing sees it.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct TranscriptFile {
        /// The file name alone, which is the session id plus `.jsonl`.
        pub name: String,
        /// Full path, built from the *resolved* directory.
        pub path: String,
        /// Milliseconds since the Unix epoch.
        pub modified_ms: u64,
    }

    /// `None` when the directory cannot be listed at all — a store that is
    /// missing, unreadable, or a symlink loop. An empty `Vec` is a directory
    /// that exists and holds no transcripts, which is a different answer: the
    /// first means "look elsewhere", the second "nothing here".
    pub fn list(project_dir: &str) -> Option<Vec<TranscriptFile>> {
        let dir = path_identity::resolved(project_dir);
        let entries = std::fs::read_dir(&dir).ok()?;
        let mut files: Vec<TranscriptFile> = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".jsonl") {
                continue;
            }
            // A dangling link has no mtime to report; skip it rather than
            // invent one, so it can never win "the most recent".
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if !metadata.is_file() {
                continue;
            }
            let Some(modified_ms) = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|elapsed| elapsed.as_millis() as u64)
            else {
                continue;
            };
            files.push(TranscriptFile {
                path: std::path::Path::new(&dir)
                    .join(&name)
                    .to_string_lossy()
                    .into_owned(),
                name,
                modified_ms,
            });
        }
        // Deterministic, so a tie on mtime does not depend on readdir order.
        files.sort_by(|a, b| a.name.cmp(&b.name));
        Some(files)
    }

    /// How far before the process started a transcript may still belong to it:
    /// the CLI writes the first line after the process is already running, and
    /// clock granularity is not free.
    pub const START_SLACK_MS: u64 = 10_000;

    /// The newest transcript written by a process that started at
    /// `started_at_ms`, skipping files from sessions older than that.
    ///
    /// `started_at_ms == None` means the process start time is unknown, and then
    /// every file is a candidate — the same rule the Swift readers use, where a
    /// missing start time only widens the field.
    pub fn most_recent(
        files: &[TranscriptFile],
        started_at_ms: Option<u64>,
    ) -> Option<&TranscriptFile> {
        files
            .iter()
            .filter(|file| match started_at_ms {
                None => true,
                Some(start) => file.modified_ms + START_SLACK_MS >= start,
            })
            .max_by_key(|file| file.modified_ms)
    }
}

#[cfg(test)]
mod transcript_file_tests {
    use std::fs;
    use std::path::PathBuf;

    use super::transcript_files::{self, START_SLACK_MS, TranscriptFile};

    /// A scratch directory that removes itself, so a failing assertion cannot
    /// leave a symlinked tree in `/tmp`.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir()
                .join(format!("morrow-transcripts-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("a scratch directory");
            Self(path)
        }

        fn text(&self) -> String {
            self.0.to_string_lossy().into_owned()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_missing_store_is_none_and_an_empty_one_is_empty() {
        let scratch = Scratch::new("empty");
        // A directory that exists but holds no transcript: "nothing here".
        assert_eq!(transcript_files::list(&scratch.text()), Some(Vec::new()));
        // A directory that is not there: "look elsewhere".
        assert_eq!(
            transcript_files::list(&format!("{}/nope", scratch.text())),
            None
        );
    }

    #[test]
    fn only_jsonl_regular_files_are_listed() {
        let scratch = Scratch::new("kinds");
        fs::write(scratch.0.join("a.jsonl"), b"{}\n").expect("a transcript");
        fs::write(scratch.0.join("notes.txt"), b"hi").expect("a file");
        fs::create_dir_all(scratch.0.join("nested.jsonl")).expect("a directory named .jsonl");

        let files = transcript_files::list(&scratch.text()).expect("a listing");
        let names: Vec<&str> = files.iter().map(|file| file.name.as_str()).collect();
        assert_eq!(names, vec!["a.jsonl"]);
    }

    /// The reason the directory is resolved first: a project dir that is itself
    /// a symlink is how two accounts share one project.
    #[cfg(unix)]
    #[test]
    fn a_symlinked_project_directory_is_listed_through_the_link() {
        let scratch = Scratch::new("dir-link");
        fs::create_dir_all(scratch.0.join("real")).expect("a directory");
        fs::write(scratch.0.join("real/s.jsonl"), b"{}\n").expect("a transcript");
        std::os::unix::fs::symlink(scratch.0.join("real"), scratch.0.join("link"))
            .expect("a symlink");

        let through_link =
            transcript_files::list(&scratch.0.join("link").to_string_lossy()).expect("a listing");
        assert_eq!(through_link.len(), 1);
        assert_eq!(through_link[0].name, "s.jsonl");
        // The path is built from the resolved directory, so a caller that opens
        // it does not go through the link a second time.
        assert_eq!(
            through_link[0].path,
            format!(
                "{}/s.jsonl",
                crate::path_identity::resolved(&scratch.0.join("real").to_string_lossy())
            )
        );
    }

    #[test]
    fn the_newest_transcript_wins() {
        let files = vec![
            TranscriptFile {
                name: "old.jsonl".into(),
                path: "/p/old.jsonl".into(),
                modified_ms: 1_000,
            },
            TranscriptFile {
                name: "new.jsonl".into(),
                path: "/p/new.jsonl".into(),
                modified_ms: 5_000,
            },
        ];
        assert_eq!(
            transcript_files::most_recent(&files, None).map(|file| file.name.as_str()),
            Some("new.jsonl")
        );
        assert_eq!(transcript_files::most_recent(&[], None), None);
    }

    #[test]
    fn a_transcript_from_before_the_process_started_is_not_its_session() {
        let files = vec![
            // Written long before the process started: another session.
            TranscriptFile {
                name: "stale.jsonl".into(),
                path: "/p/stale.jsonl".into(),
                modified_ms: 1_000,
            },
            // Written after it started, but not the newest.
            TranscriptFile {
                name: "mine.jsonl".into(),
                path: "/p/mine.jsonl".into(),
                modified_ms: 20_000,
            },
            // Newest of all, and written by this process.
            TranscriptFile {
                name: "mine-later.jsonl".into(),
                path: "/p/mine-later.jsonl".into(),
                modified_ms: 30_000,
            },
        ];
        let started = 20_000;
        assert_eq!(
            transcript_files::most_recent(&files, Some(started)).map(|file| file.name.as_str()),
            Some("mine-later.jsonl")
        );
        // The stale one is excluded even when it is the only candidate.
        assert_eq!(
            transcript_files::most_recent(&files[..1], Some(started)),
            None
        );
        // The slack exists because the CLI writes its first line just after the
        // process is already running.
        let just_before = started - START_SLACK_MS;
        assert_eq!(
            transcript_files::most_recent(&files[..1], Some(just_before))
                .map(|file| file.name.as_str()),
            Some("stale.jsonl")
        );
    }
}

/// OpenCode's (and MiMo Code's) session store (#247c).
///
/// The store is SQLite, not JSONL: a `session` row per session and `message` /
/// `part` rows under it, with the interesting values inside JSON columns. That
/// shape is why this reader asks the schema before it queries — the two forks of
/// the CLI disagree about which columns exist.
pub mod opencode {
    use rusqlite::Connection;

    use crate::reduce::ChatMessage;
    use crate::sqlite::non_empty_text;

    /// A session row worth showing, with the process-start rule applied.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct SessionHit {
        pub session_id: String,
        /// Milliseconds since the Unix epoch, from `time_updated`.
        pub modified_ms: i64,
    }

    /// How far before the process started a row may still belong to it. The CLI
    /// writes its first row after the process is already running.
    pub const START_SLACK_MS: i64 = 10_000;

    /// `findRecentOpenCodeSession`: the newest session that belongs to `cwd`.
    ///
    /// A session counts when its own `directory` matches **or** one of its
    /// messages recorded `path.cwd` — the second is what catches a CLI started in
    /// one directory and moved with `/cd`. `root_sessions_only` drops child
    /// sessions (`parent_id` set), which MiMo Code spawns in the same directory
    /// while a subagent runs: it is then the most recently updated row and would
    /// otherwise win, though it is never the CLI's own.
    pub fn find_recent_session(
        connection: &Connection,
        cwd: &str,
        started_at_ms: Option<i64>,
        root_sessions_only: bool,
    ) -> Option<SessionHit> {
        let columns = crate::sqlite::table_columns(connection, "session");
        if columns.is_empty() {
            return None;
        }
        // Conditions are collected and joined rather than interpolated as
        // fragments: an empty fragment next to `AND` is a syntax error, and a
        // query that fails to prepare answers "no sessions" — which looks exactly
        // like "the agent is not running".
        let mut conditions: Vec<&str> = Vec::new();
        // A fork that has no `parent_id` column has no child sessions either.
        if root_sessions_only && columns.iter().any(|column| column == "parent_id") {
            conditions.push("parent_id IS NULL");
        }
        if columns.iter().any(|column| column == "time_archived") {
            conditions.push("time_archived IS NULL");
        }
        let mine = "(directory = ?1
                 OR EXISTS (
                     SELECT 1 FROM message m
                     WHERE m.session_id = session.id
                       AND json_extract(m.data, '$.path.cwd') = ?2
                 ))";
        let sql = if conditions.is_empty() {
            format!(
                "SELECT id, time_updated FROM session WHERE {mine}
                 ORDER BY time_updated DESC LIMIT 20;"
            )
        } else {
            format!(
                "SELECT id, time_updated FROM session WHERE {} AND {mine}
                 ORDER BY time_updated DESC LIMIT 20;",
                conditions.join(" AND ")
            )
        };
        let mut statement = connection.prepare(&sql).ok()?;
        let mut rows = statement.query([cwd, cwd]).ok()?;
        let floor = started_at_ms.map(|start| start - START_SLACK_MS);
        while let Ok(Some(row)) = rows.next() {
            let Ok(session_id) = row.get::<_, String>(0) else {
                continue;
            };
            let Ok(updated_ms) = row.get::<_, i64>(1) else {
                continue;
            };
            if floor.is_some_and(|floor| updated_ms < floor) {
                continue;
            }
            return Some(SessionHit {
                session_id,
                modified_ms: updated_ms,
            });
        }
        None
    }

    /// `readRecentFromOpenCodeSession`: the model and the last three text turns.
    ///
    /// The model is read from the newest message rows first, because an older
    /// message may predate the user switching model. The turns come from `part`
    /// rows, one per message, newest first, then put back in time order — a
    /// message is one turn no matter how many parts it was written in.
    pub fn read_recent(
        connection: &Connection,
        session_id: &str,
    ) -> (Option<String>, Vec<ChatMessage>) {
        let model = model_of_newest_message(connection, session_id);

        let Ok(mut statement) = connection.prepare(
            "SELECT p.message_id, json_extract(m.data, '$.role'), p.time_created, p.data
             FROM part p
             JOIN message m ON m.id = p.message_id
             WHERE p.session_id = ?1
             ORDER BY p.time_created DESC
             LIMIT 80;",
        ) else {
            return (model, Vec::new());
        };
        let Ok(mut rows) = statement.query([session_id]) else {
            return (model, Vec::new());
        };
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut turns: Vec<(i64, ChatMessage)> = Vec::new();
        while let Ok(Some(row)) = rows.next() {
            let Ok(message_id) = row.get::<_, String>(0) else {
                continue;
            };
            if seen.contains(&message_id) {
                continue;
            }
            let Some(role) = non_empty_text(row, 1) else {
                continue;
            };
            let is_user = match role.as_str() {
                "user" => true,
                "assistant" => false,
                _ => continue,
            };
            let Ok(created_ms) = row.get::<_, i64>(2) else {
                continue;
            };
            let Some(data) = non_empty_text(row, 3) else {
                continue;
            };
            let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) else {
                continue;
            };
            if json.get("type").and_then(|value| value.as_str()) != Some("text") {
                continue;
            }
            let Some(text) = json.get("text").and_then(|value| value.as_str()) else {
                continue;
            };
            if text.is_empty() {
                continue;
            }
            seen.insert(message_id);
            turns.push((created_ms, ChatMessage::new(is_user, text)));
        }
        turns.sort_by_key(|(created_ms, _)| *created_ms);
        let start = turns.len().saturating_sub(3);
        (
            model,
            turns.split_off(start).into_iter().map(|(_, m)| m).collect(),
        )
    }

    /// `modelID` on the message itself, else `model.modelID` on the nested
    /// object the newer builds write.
    fn model_of_newest_message(connection: &Connection, session_id: &str) -> Option<String> {
        let mut statement = connection
            .prepare(
                "SELECT data FROM message
                 WHERE session_id = ?1
                 ORDER BY time_updated DESC
                 LIMIT 12;",
            )
            .ok()?;
        let mut rows = statement.query([session_id]).ok()?;
        while let Ok(Some(row)) = rows.next() {
            let Some(data) = non_empty_text(row, 0) else {
                continue;
            };
            let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) else {
                continue;
            };
            if let Some(model) = json.get("modelID").and_then(|value| value.as_str()) {
                if !model.is_empty() {
                    return Some(model.to_owned());
                }
            }
            if let Some(model) = json
                .get("model")
                .and_then(|value| value.get("modelID"))
                .and_then(|value| value.as_str())
            {
                if !model.is_empty() {
                    return Some(model.to_owned());
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod opencode_tests {
    use rusqlite::Connection;

    use super::opencode;
    use crate::sqlite::open_read_only;

    /// A store shaped like OpenCode's, filled by `rows`, then opened the way
    /// discovery opens it: read-only.
    struct Store {
        dir: std::path::PathBuf,
    }

    impl Store {
        fn new(name: &str, schema_and_rows: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("morrow-opencode-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("a scratch directory");
            let path = dir.join("opencode.db");
            let connection = Connection::open(&path).expect("a writable store");
            connection
                .execute_batch(schema_and_rows)
                .expect("a schema and its rows");
            Self { dir }
        }

        fn open(&self) -> Connection {
            open_read_only(&self.dir.join("opencode.db")).expect("a read-only store")
        }
    }

    impl Drop for Store {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    const SCHEMA: &str = "
        CREATE TABLE session (
            id TEXT PRIMARY KEY,
            directory TEXT,
            parent_id TEXT,
            time_updated INTEGER,
            time_archived INTEGER
        );
        CREATE TABLE message (
            id TEXT PRIMARY KEY,
            session_id TEXT,
            data TEXT,
            time_updated INTEGER
        );
        CREATE TABLE part (
            id TEXT PRIMARY KEY,
            message_id TEXT,
            session_id TEXT,
            data TEXT,
            time_created INTEGER
        );
    ";

    fn part(
        id: &str,
        message_id: &str,
        session_id: &str,
        kind: &str,
        text: &str,
        at: i64,
    ) -> String {
        format!(
            "INSERT INTO part VALUES ('{id}', '{message_id}', '{session_id}',
                '{{\"type\":\"{kind}\",\"text\":\"{text}\"}}', {at});"
        )
    }

    fn message(id: &str, session_id: &str, role: &str, at: i64) -> String {
        format!(
            "INSERT INTO message VALUES ('{id}', '{session_id}',
                '{{\"role\":\"{role}\"}}', {at});"
        )
    }

    fn session(id: &str, directory: &str, updated: i64) -> String {
        format!("INSERT INTO session VALUES ('{id}', '{directory}', NULL, {updated}, NULL);")
    }

    #[test]
    fn the_newest_session_in_the_same_directory_wins() {
        let store = Store::new(
            "newest",
            &format!(
                "{SCHEMA}{}{}{}",
                session("old", "/home/dev/project", 1_000),
                session("new", "/home/dev/project", 5_000),
                session("other", "/home/dev/elsewhere", 9_000)
            ),
        );
        let connection = store.open();
        let hit = opencode::find_recent_session(&connection, "/home/dev/project", None, false)
            .expect("a session");
        assert_eq!(hit.session_id, "new");
        assert_eq!(hit.modified_ms, 5_000);
    }

    /// A CLI started in one directory and moved with `/cd` records the new cwd
    /// on its messages, not on the session row. Without the second half of the
    /// query that session is invisible.
    #[test]
    fn a_session_whose_messages_record_the_cwd_is_found() {
        let store = Store::new(
            "moved",
            &format!(
                "{SCHEMA}{}{}",
                session("moved", "/home/dev/original", 2_000),
                "INSERT INTO message VALUES ('m1', 'moved',
                    '{\"role\":\"user\",\"path\":{\"cwd\":\"/home/dev/project\"}}', 2_000);"
            ),
        );
        let connection = store.open();
        let hit = opencode::find_recent_session(&connection, "/home/dev/project", None, false)
            .expect("a session");
        assert_eq!(hit.session_id, "moved");
    }

    #[test]
    fn an_archived_session_is_not_a_candidate() {
        let store = Store::new(
            "archived",
            &format!(
                "{SCHEMA}INSERT INTO session VALUES ('a', '/home/dev/project', NULL, 5000, 5000);"
            ),
        );
        let connection = store.open();
        assert_eq!(
            opencode::find_recent_session(&connection, "/home/dev/project", None, false),
            None
        );
    }

    /// MiMo Code spawns subagent sessions in the same directory; while one runs
    /// it is the newest row and would win, though it is never the CLI's own.
    #[test]
    fn a_child_session_is_skipped_when_only_root_sessions_count() {
        let store = Store::new(
            "child",
            &format!(
                "{SCHEMA}{}{}",
                session("root", "/home/dev/project", 1_000),
                "INSERT INTO session VALUES ('child', '/home/dev/project', 'root', 5000, NULL);"
            ),
        );
        let connection = store.open();

        // Without the filter the child wins, which is the bug the filter fixes.
        assert_eq!(
            opencode::find_recent_session(&connection, "/home/dev/project", None, false)
                .map(|hit| hit.session_id),
            Some("child".to_owned())
        );
        assert_eq!(
            opencode::find_recent_session(&connection, "/home/dev/project", None, true)
                .map(|hit| hit.session_id),
            Some("root".to_owned())
        );
    }

    #[test]
    fn a_session_updated_before_the_process_started_is_not_its_session() {
        let store = Store::new(
            "stale",
            &format!("{SCHEMA}{}", session("stale", "/home/dev/project", 1_000)),
        );
        let connection = store.open();

        assert!(
            opencode::find_recent_session(&connection, "/home/dev/project", None, false).is_some()
        );
        // The process started well after the row was last touched.
        assert_eq!(
            opencode::find_recent_session(&connection, "/home/dev/project", Some(600_000), false),
            None
        );
        // Inside the slack it still counts: the CLI writes its first row just
        // after the process is running.
        assert!(
            opencode::find_recent_session(&connection, "/home/dev/project", Some(5_000), false)
                .is_some()
        );
    }

    #[test]
    fn the_last_three_text_turns_are_returned_in_time_order() {
        let mut rows = String::new();
        rows.push_str(&session("s", "/home/dev/project", 9_000));
        for (index, (role, text, at)) in [
            ("user", "first", 1_000),
            ("assistant", "second", 2_000),
            ("user", "third", 3_000),
            ("assistant", "fourth", 4_000),
        ]
        .iter()
        .enumerate()
        {
            let id = format!("m{index}");
            rows.push_str(&message(&id, "s", role, *at));
            rows.push_str(&part(&format!("p{index}"), &id, "s", "text", text, *at));
        }
        // A non-text part must not become a turn.
        rows.push_str(&message("m-tool", "s", "assistant", 5_000));
        rows.push_str(&part("p-tool", "m-tool", "s", "tool", "not a turn", 5_000));

        let store = Store::new("turns", &format!("{SCHEMA}{rows}"));
        let connection = store.open();
        let (_, messages) = opencode::read_recent(&connection, "s");

        let rendered: Vec<(bool, &str)> = messages
            .iter()
            .map(|message| (message.is_user, message.text.as_str()))
            .collect();
        assert_eq!(
            rendered,
            vec![(false, "second"), (true, "third"), (false, "fourth")]
        );
    }

    /// One message is one turn, however many parts it was written in. Which
    /// part represents it is the newest one, because the rows arrive
    /// newest-first and the first to claim the message id wins — the same rule
    /// the Swift reader gets from its `ORDER BY ... DESC`.
    #[test]
    fn several_parts_of_one_message_are_one_turn() {
        let store = Store::new(
            "parts",
            &format!(
                "{SCHEMA}{}{}{}{}",
                session("s", "/home/dev/project", 9_000),
                message("m1", "s", "assistant", 1_000),
                part("p1", "m1", "s", "text", "one turn", 1_000),
                part("p2", "m1", "s", "text", "same turn", 1_001)
            ),
        );
        let connection = store.open();
        let (_, messages) = opencode::read_recent(&connection, "s");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "same turn");
    }

    #[test]
    fn the_model_comes_from_the_newest_message_that_names_one() {
        let store = Store::new(
            "model",
            &format!(
                "{SCHEMA}{}{}{}{}",
                session("s", "/home/dev/project", 9_000),
                "INSERT INTO message VALUES ('m1', 's',
                    '{\"role\":\"assistant\",\"model\":{\"modelID\":\"claude-opus-4\"}}', 1000);",
                "INSERT INTO message VALUES ('m2', 's',
                    '{\"role\":\"user\",\"modelID\":\"gpt-5\"}', 2000);",
                "INSERT INTO message VALUES ('m3', 's', '{\"role\":\"user\"}', 3000);"
            ),
        );
        let connection = store.open();
        let (model, _) = opencode::read_recent(&connection, "s");
        assert_eq!(model, Some("gpt-5".to_owned()));
    }

    #[test]
    fn a_store_without_the_expected_tables_answers_nothing() {
        let store = Store::new("empty", "CREATE TABLE unrelated (a TEXT);");
        let connection = store.open();

        assert_eq!(
            opencode::find_recent_session(&connection, "/home/dev/project", None, false),
            None
        );
        assert_eq!(opencode::read_recent(&connection, "s"), (None, Vec::new()));
    }

    /// A fork that has no `parent_id` column has no child sessions either, and
    /// asking for one must not turn into a SQL error that loses every session.
    #[test]
    fn a_fork_without_the_parent_column_still_answers() {
        let store = Store::new(
            "fork",
            "CREATE TABLE session (id TEXT PRIMARY KEY, directory TEXT, time_updated INTEGER);
             CREATE TABLE message (id TEXT, session_id TEXT, data TEXT, time_updated INTEGER);
             INSERT INTO session VALUES ('s', '/home/dev/project', 1000);",
        );
        let connection = store.open();
        assert_eq!(
            opencode::find_recent_session(&connection, "/home/dev/project", None, true)
                .map(|hit| hit.session_id),
            Some("s".to_owned())
        );
    }
}

/// The coordinator: several roots × several readers, one list of sessions
/// (#260).
///
/// This is the half of `SessionDiscoveryCoordinator` that is not the watcher.
/// What goes wrong without it is quiet: one account scanned twice because two
/// roots are the same directory under different spellings, one session shown
/// twice because two decoders saw it, and a tick that never returns because one
/// store is enormous.
pub mod scan {
    use std::collections::HashSet;

    use super::DiscoveryHit;
    use crate::config_roots;
    use crate::path_identity;

    /// One decoder: the sessions of one agent, under one root.
    ///
    /// A trait rather than a function pointer so a reader can own state — the
    /// incremental cache (#259) is per-reader, not per-scan — and so a test can
    /// hand in a fake without a filesystem.
    pub trait StoreReader {
        /// The agent this reads, as `--source` spells it.
        fn source(&self) -> &'static str;

        /// Every session this root holds. A root the reader does not understand
        /// answers an empty list, not an error: one unknown store must not lose
        /// the others.
        fn read(&self, root: &str) -> Vec<DiscoveryHit>;
    }

    /// What one scan produced.
    #[derive(Debug, Default)]
    pub struct ScanResult {
        /// Deduped, ordered by the readers' own order.
        pub hits: Vec<DiscoveryHit>,
        /// Readers that were not reached before the budget ran out, as
        /// `(source, root)`. Reported rather than dropped, so a scan that keeps
        /// never finishing is visible instead of silently partial.
        pub unfinished: Vec<(&'static str, String)>,
        pub elapsed_ms: u64,
    }

    /// How long one scan may take before it gives the tick back.
    ///
    /// The panel redraws on a tick, so a scan is a latency budget, not a
    /// throughput problem: it is better to report two sessions and a leftover
    /// than to block the feed on a store nobody is looking at.
    pub const DEFAULT_BUDGET_MS: u64 = 250;

    /// The roots to scan: the primary plus the extras, with the same directory
    /// under two spellings collapsed to one.
    ///
    /// This is the caller `path_identity` was built for (#256): a symlinked extra
    /// directory that is the primary one must not be scanned twice, or its
    /// sessions are counted twice and its hooks installed twice.
    pub fn scan_roots(primary: &str, extras: &[&str]) -> Vec<String> {
        config_roots::roots(primary, extras, path_identity::identity)
    }

    /// Run every reader over every root, within `budget_ms`.
    ///
    /// `now_ms` is injected so a test can drive the budget without sleeping.
    ///
    /// Dedup is by session id, and the **first** reader to return a session keeps
    /// it: readers are given in precedence order, so an agent whose store is
    /// authoritative for its own sessions comes before one that merely indexes
    /// somebody else's. Keeping the first is deliberate — a later reader with a
    /// thinner projection must not replace a fuller one.
    pub fn run(
        roots: &[String],
        readers: &[&dyn StoreReader],
        budget_ms: u64,
        now_ms: impl Fn() -> u64,
    ) -> ScanResult {
        let started = now_ms();
        let mut result = ScanResult::default();
        let mut claimed: HashSet<String> = HashSet::new();
        let mut exhausted = false;

        for root in roots {
            for reader in readers {
                // Checked before starting a reader, never in the middle of one: a
                // reader that begins inside the budget is allowed to finish, so a
                // big store is not cut in half and reported as complete — and
                // nothing new starts after the deadline, so the overrun is at most
                // the one reader that was already running.
                if !exhausted && now_ms().saturating_sub(started) > budget_ms {
                    exhausted = true;
                }
                if exhausted {
                    result.unfinished.push((reader.source(), root.clone()));
                    continue;
                }
                for hit in reader.read(root) {
                    if claimed.insert(hit.session_id.clone()) {
                        result.hits.push(hit);
                    }
                }
            }
        }

        result.elapsed_ms = now_ms().saturating_sub(started);
        result
    }
}

#[cfg(test)]
mod scan_tests {
    use super::DiscoveryHit;
    use super::scan::{self, ScanResult, StoreReader};

    fn hit(session_id: &str, source: &'static str, cwd: &str) -> DiscoveryHit {
        DiscoveryHit {
            session_id: session_id.to_owned(),
            source,
            cwd: cwd.to_owned(),
            ..DiscoveryHit::default()
        }
    }

    /// A reader that answers a fixed table per root, and counts its calls.
    struct Fake {
        source: &'static str,
        table: Vec<(&'static str, Vec<DiscoveryHit>)>,
        calls: std::cell::RefCell<Vec<String>>,
    }

    impl Fake {
        fn new(source: &'static str, table: Vec<(&'static str, Vec<DiscoveryHit>)>) -> Self {
            Self {
                source,
                table,
                calls: std::cell::RefCell::new(Vec::new()),
            }
        }
    }

    impl StoreReader for Fake {
        fn source(&self) -> &'static str {
            self.source
        }

        fn read(&self, root: &str) -> Vec<DiscoveryHit> {
            self.calls.borrow_mut().push(root.to_owned());
            self.table
                .iter()
                .filter(|(table_root, _)| *table_root == root)
                .flat_map(|(_, hits)| hits.clone())
                .collect()
        }
    }

    fn ids(result: &ScanResult) -> Vec<&str> {
        result
            .hits
            .iter()
            .map(|hit| hit.session_id.as_str())
            .collect()
    }

    #[test]
    fn every_reader_reads_every_root() {
        let claude = Fake::new(
            "claude",
            vec![
                (r"/r1", vec![hit("a", "claude", "/w")]),
                (r"/r2", vec![hit("b", "claude", "/w")]),
            ],
        );
        let codex = Fake::new("codex", vec![(r"/r2", vec![hit("c", "codex", "/w")])]);

        let readers: Vec<&dyn StoreReader> = vec![&claude, &codex];
        let result = scan::run(
            &[r"/r1".to_owned(), r"/r2".to_owned()],
            &readers,
            scan::DEFAULT_BUDGET_MS,
            || 0,
        );

        assert_eq!(ids(&result), vec!["a", "b", "c"]);
        assert_eq!(*claude.calls.borrow(), vec![r"/r1", r"/r2"]);
        assert_eq!(*codex.calls.borrow(), vec![r"/r1", r"/r2"]);
        assert!(result.unfinished.is_empty());
    }

    /// One session seen by two decoders is one card. Which source it carries is
    /// decided by the reader order, so the caller states precedence instead of
    /// inheriting it from a scan order.
    #[test]
    fn a_session_two_decoders_see_appears_once_with_the_first_source() {
        let authoritative = Fake::new("claude", vec![(r"/r", vec![hit("same", "claude", "/w")])]);
        let index = Fake::new("codex", vec![(r"/r", vec![hit("same", "codex", "/w")])]);
        let readers: Vec<&dyn StoreReader> = vec![&authoritative, &index];

        let result = scan::run(&[r"/r".to_owned()], &readers, scan::DEFAULT_BUDGET_MS, || 0);

        assert_eq!(ids(&result), vec!["same"]);
        assert_eq!(result.hits[0].source, "claude");
    }

    #[test]
    fn the_other_reader_order_gives_the_other_source() {
        // Not a tautology: it shows the outcome is the caller's decision, which
        // is what makes "the right source" testable at all.
        let first = Fake::new("codex", vec![(r"/r", vec![hit("same", "codex", "/w")])]);
        let second = Fake::new("claude", vec![(r"/r", vec![hit("same", "claude", "/w")])]);
        let readers: Vec<&dyn StoreReader> = vec![&first, &second];

        let result = scan::run(&[r"/r".to_owned()], &readers, scan::DEFAULT_BUDGET_MS, || 0);
        assert_eq!(result.hits[0].source, "codex");
    }

    /// The budget gives the tick back: work not reached is reported, not lost
    /// quietly. A scan that always times out is then visible as such.
    #[test]
    fn work_past_the_budget_is_reported_rather_than_dropped() {
        // Zero for the first two reads (start + the first check), then past the
        // budget: deterministic, no sleeps.
        let clock = std::cell::Cell::new(0usize);
        let first = Fake::new("claude", vec![(r"/r1", vec![hit("a", "claude", "/w")])]);
        let second = Fake::new("codex", vec![(r"/r2", vec![hit("b", "codex", "/w")])]);
        let readers: Vec<&dyn StoreReader> = vec![&first, &second];

        let result = scan::run(
            &[r"/r1".to_owned(), r"/r2".to_owned()],
            &readers,
            100,
            || {
                let call = clock.get();
                clock.set(call + 1);
                if call < 2 { 0 } else { 10_000 }
            },
        );

        assert_eq!(ids(&result), vec!["a"]);
        assert_eq!(
            result.unfinished,
            vec![
                ("codex", r"/r1".to_owned()),
                ("claude", r"/r2".to_owned()),
                ("codex", r"/r2".to_owned()),
            ],
            "every reader that did not reach a root is named, not just the first"
        );
        // Nothing new was started once the deadline had passed.
        assert_eq!(*second.calls.borrow(), Vec::<String>::new());
    }

    /// A reader that starts inside the budget is allowed to finish, so a big
    /// store is never cut in half and reported as complete.
    #[test]
    fn a_reader_that_starts_within_the_budget_completes() {
        let clock = std::cell::Cell::new(0u64);
        let slow = Fake::new(
            "claude",
            vec![(
                r"/r",
                vec![hit("a", "claude", "/w"), hit("b", "claude", "/w")],
            )],
        );
        let readers: Vec<&dyn StoreReader> = vec![&slow];

        let result = scan::run(&[r"/r".to_owned()], &readers, 50, || {
            let now = clock.get();
            clock.set(now + 40);
            now
        });

        assert_eq!(ids(&result), vec!["a", "b"]);
        assert!(result.unfinished.is_empty());
    }

    #[test]
    fn a_reader_that_answers_nothing_is_not_an_error() {
        let empty = Fake::new("claude", vec![]);
        let readers: Vec<&dyn StoreReader> = vec![&empty];
        let result = scan::run(&[r"/r".to_owned()], &readers, scan::DEFAULT_BUDGET_MS, || 0);
        assert!(result.hits.is_empty());
        assert!(result.unfinished.is_empty());
    }

    #[test]
    fn an_empty_root_list_reads_nothing() {
        let reader = Fake::new("claude", vec![(r"/r", vec![hit("a", "claude", "/w")])]);
        let readers: Vec<&dyn StoreReader> = vec![&reader];
        let result = scan::run(&[], &readers, scan::DEFAULT_BUDGET_MS, || 0);
        assert!(result.hits.is_empty());
        assert!(reader.calls.borrow().is_empty());
    }

    /// The caller `path_identity` was built for (#256).
    #[cfg(unix)]
    #[test]
    fn a_symlinked_extra_root_is_scanned_once() {
        let dir = std::env::temp_dir().join(format!("morrow-scan-roots-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("primary")).expect("a directory");
        std::os::unix::fs::symlink(dir.join("primary"), dir.join("alias")).expect("a symlink");

        let primary = dir.join("primary").to_string_lossy().into_owned();
        let alias = dir.join("alias").to_string_lossy().into_owned();
        let other = dir.join("other").to_string_lossy().into_owned();

        let roots = scan::scan_roots(&primary, &[&alias, &other]);
        assert_eq!(roots, vec![primary, other]);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
