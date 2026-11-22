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
