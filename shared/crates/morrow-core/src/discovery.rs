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

/// What a Codex rollout says about its session, without a hook (#272).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CodexRolloutSummary {
    pub session_id: String,
    pub cwd: String,
    pub model: Option<String>,
    pub recent_messages: Vec<ChatMessage>,
}

/// `CodexDiscovery`'s rollout read, from the tail of one rollout file.
///
/// Codex is **store-first**: the rollout itself carries the thread id, the
/// working directory and the model, so a session can be found with no process at
/// all — unlike Cursor and Gemini, whose store directory is derived from a
/// process's cwd (#270).
///
/// Three rules that are not obvious and are all in the Swift reader:
///
/// - **The model is on `turn_context`, not `session_meta`.** `session_meta` names
///   the provider (`openai`), which is not a label; `turn_context` names the model
///   each turn actually ran on, so the last one wins.
/// - **User text comes from `event_msg.user_message`.** A `response_item` user
///   entry carries the system and tool context Codex injects, which is not
///   something the user typed. It is only a fallback when no clean message exists.
/// - **The session id is `session_meta.payload.id`**, and the file name only when
///   that is missing — a Desktop thread keeps a uuid the file name does not match.
pub fn read_codex_rollout_tail(tail: &str, file_name: &str) -> Option<CodexRolloutSummary> {
    let mut summary = CodexRolloutSummary::default();
    // Position matters: user and assistant turns are interleaved, and collecting
    // them in two lists then concatenating loses the order — which turns a
    // conversation into "the agent said three things".
    let mut users: Vec<(usize, String)> = Vec::new();
    let mut assistants: Vec<(usize, String)> = Vec::new();
    let mut saw_meta = false;
    let mut index = 0usize;

    for line in tail.split('\n') {
        index += 1;
        if line.is_empty() {
            continue;
        }
        let Ok(json) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(kind) = json.get("type").and_then(|value| value.as_str()) else {
            continue;
        };
        let Some(payload) = json.get("payload") else {
            continue;
        };

        match kind {
            "session_meta" => {
                saw_meta = true;
                if summary.session_id.is_empty() {
                    summary.session_id = payload
                        .get("id")
                        .and_then(|value| value.as_str())
                        .filter(|id| !id.is_empty())
                        .unwrap_or_default()
                        .to_owned();
                }
                if summary.cwd.is_empty() {
                    summary.cwd = payload
                        .get("cwd")
                        .and_then(|value| value.as_str())
                        .unwrap_or_default()
                        .to_owned();
                }
            }
            // The model each turn actually ran on. The last one wins: it is what
            // the newest turn used.
            "turn_context" => {
                if let Some(model) = payload
                    .get("model")
                    .and_then(|value| value.as_str())
                    .filter(|model| !model.is_empty())
                {
                    summary.model = Some(model.to_owned());
                }
            }
            "event_msg" => {
                let text = if payload.get("type").and_then(|value| value.as_str())
                    == Some("user_message")
                {
                    payload
                        .get("message")
                        .and_then(|value| value.as_str())
                        .filter(|text| !text.is_empty())
                } else {
                    None
                };
                if let Some(text) = text {
                    users.push((index, text.to_owned()));
                }
            }
            _ => {}
        }

        // The assistant text rule is shared with the live tailer, so a card and its
        // backfill cannot disagree about what the agent said.
        if let Some(text) = json
            .as_object()
            .and_then(crate::transcript::codex_public_assistant_text)
        {
            // The same assistant text twice in a row is one turn, not two.
            if assistants.last().map(|(_, last)| last) != Some(&text) {
                assistants.push((index, text));
            }
        }
    }

    if summary.session_id.is_empty() {
        // Fall back to the file name, which is `rollout-<timestamp>-<id>.jsonl`.
        summary.session_id = file_name
            .strip_suffix(".jsonl")
            .and_then(|name| name.rsplit('-').next())
            .unwrap_or_default()
            .to_owned();
    }
    if summary.session_id.is_empty() || (!saw_meta && users.is_empty() && assistants.is_empty()) {
        return None;
    }

    // Three of each, then the last three overall, in the order they were written —
    // the Swift reader's rule, and the only one that keeps "asked, answered" from
    // becoming "answered, answered".
    let user_start = users.len().saturating_sub(3);
    let assistant_start = assistants.len().saturating_sub(3);
    let mut turns: Vec<(usize, ChatMessage)> = users[user_start..]
        .iter()
        .map(|(index, text)| (*index, ChatMessage::new(true, text)))
        .chain(
            assistants[assistant_start..]
                .iter()
                .map(|(index, text)| (*index, ChatMessage::new(false, text))),
        )
        .collect();
    turns.sort_by_key(|(index, _)| *index);
    let start = turns.len().saturating_sub(3);
    summary.recent_messages = turns.split_off(start).into_iter().map(|(_, m)| m).collect();
    Some(summary)
}

/// What a [CC] transcript tail says about its session, without a hook.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TranscriptSummary {
    pub session_id: String,
    /// The working directory the CLI recorded, which is what a card shows.
    pub cwd: String,
    pub model: Option<String>,
    /// The last few turns, oldest first.
    pub recent_messages: Vec<ChatMessage>,
}

/// `ClaudeDiscovery`'s session read, from the tail of one transcript (#261).
///
/// The tail is what the shell already reads for a live session (`read_tail`,
/// last 64 KB), so backfill costs one read of the same bytes and no second
/// parser. `file_name` carries the session id: Claude Code names a transcript
/// after the session it records.
///
/// `None` when the tail holds nothing usable — a half-written first line, an
/// empty file, a store from a version this does not understand. The caller
/// skips it rather than inventing a session.
pub fn read_transcript_tail(tail: &str, file_name: &str) -> Option<TranscriptSummary> {
    let session_id = file_name.strip_suffix(".jsonl")?;
    if session_id.is_empty() {
        return None;
    }
    let mut summary = TranscriptSummary {
        session_id: session_id.to_owned(),
        ..TranscriptSummary::default()
    };
    let mut turns: Vec<ChatMessage> = Vec::new();

    for line in tail.split('\n') {
        if line.is_empty() {
            continue;
        }
        let Ok(json) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        // `cwd` is repeated on every row; the first one is as good as the last.
        if summary.cwd.is_empty() {
            if let Some(cwd) = json.get("cwd").and_then(|value| value.as_str()) {
                summary.cwd = cwd.to_owned();
            }
        }
        if summary.model.is_none() {
            summary.model = json
                .get("message")
                .and_then(|message| message.get("model"))
                .and_then(|model| model.as_str())
                .filter(|model| !model.is_empty())
                .map(str::to_owned);
        }
        let Some(kind) = json.get("type").and_then(|value| value.as_str()) else {
            continue;
        };
        let is_user = match kind {
            "user" => true,
            "assistant" => false,
            // `summary`, `system`, `queue-operation` and the rest are not turns.
            _ => continue,
        };
        let content = json
            .get("message")
            .and_then(|message| message.get("content"));
        let Some(text) = extract_text_content(content) else {
            continue;
        };
        // A tool result is a user row but not something the user said: it is
        // the reply to a tool the assistant called, and showing it as a prompt
        // would put JSON on the card.
        if is_user && looks_like_tool_result(&text) {
            continue;
        }
        turns.push(ChatMessage::new(is_user, &text));
    }

    if summary.cwd.is_empty() && turns.is_empty() {
        return None;
    }
    let start = turns.len().saturating_sub(3);
    summary.recent_messages = turns.split_off(start);
    Some(summary)
}

/// Whether a `user` row's text is a tool result rather than a prompt.
fn looks_like_tool_result(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with("[{") || trimmed.starts_with("{\"type\":\"tool_result\"")
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

    /// `readGeminiProjectsMap`: `~/.gemini/projects.json` maps a working directory
    /// to the folder under `~/.gemini/tmp` that holds its chats.
    ///
    /// A map rather than a derivation, unlike Cursor: Gemini keys the folder by a
    /// hash it does not document, so the only way to find it is to be told.
    pub fn projects_map(json: &str) -> std::collections::HashMap<String, String> {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
            return std::collections::HashMap::new();
        };
        value
            .get("projects")
            .and_then(|projects| projects.as_object())
            .map(|projects| {
                projects
                    .iter()
                    .filter_map(|(cwd, directory)| {
                        directory
                            .as_str()
                            .map(|directory| (cwd.clone(), directory.to_owned()))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// A `.project_root` file's contents, trimmed: the fallback when
    /// `projects.json` does not name the directory.
    pub fn project_root_matches(contents: &str, cwd: &str) -> bool {
        contents.trim() == cwd
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

    /// One file's modification time, in milliseconds, without listing its
    /// directory. `None` when it is not there.
    pub fn mtime_ms(path: &str) -> Option<u64> {
        let modified = std::fs::metadata(path).ok()?.modified().ok()?;
        modified
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|since| since.as_millis() as u64)
    }

    /// The same listing with any extension, for a store that is not JSONL
    /// (Gemini's chats are `*.json`).
    pub fn list_any(directory: &str, extension: &str) -> Option<Vec<TranscriptFile>> {
        let suffix = format!(".{extension}");
        list_matching(directory, &suffix)
    }

    /// `None` when the directory cannot be listed at all — a store that is
    /// missing, unreadable, or a symlink loop. An empty `Vec` is a directory
    /// that exists and holds no transcripts, which is a different answer: the
    /// first means "look elsewhere", the second "nothing here".
    pub fn list(project_dir: &str) -> Option<Vec<TranscriptFile>> {
        list_matching(project_dir, ".jsonl")
    }

    fn list_matching(project_dir: &str, suffix: &str) -> Option<Vec<TranscriptFile>> {
        let dir = path_identity::resolved(project_dir);
        let entries = std::fs::read_dir(&dir).ok()?;
        let mut files: Vec<TranscriptFile> = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.ends_with(suffix) {
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

/// Timestamps, parsed the one way every store here needs (#279).
///
/// Not a general ISO 8601 implementation: Grok, Kiro and Cline write
/// `YYYY-MM-DDTHH:MM:SS[.fff](Z|±HH:MM)` and nothing else, and a full parser would
/// be more surface than the three callers need.
pub mod time {
    /// Milliseconds since the Unix epoch. `None` for anything that is not the
    /// shape above — a guess here would put a session in the wrong order.
    pub fn parse_iso8601_ms(text: &str) -> Option<u64> {
        let text = text.trim();
        let bytes = text.as_bytes();
        if bytes.len() < 19 {
            return None;
        }
        let year: i64 = text.get(0..4)?.parse().ok()?;
        let month: u32 = text.get(5..7)?.parse().ok()?;
        let day: u32 = text.get(8..10)?.parse().ok()?;
        let hour: i64 = text.get(11..13)?.parse().ok()?;
        let minute: i64 = text.get(14..16)?.parse().ok()?;
        let second: i64 = text.get(17..19)?.parse().ok()?;
        if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            return None;
        }
        // The separators are checked rather than skipped: nineteen digits parse
        // field by field and are not a timestamp.
        if bytes[4] != b'-' || bytes[7] != b'-' {
            return None;
        }
        if bytes[10] != b'T' && bytes[10] != b' ' {
            return None;
        }
        if bytes[13] != b':' || bytes[16] != b':' {
            return None;
        }

        let mut rest = &text[19..];
        let mut millis: u64 = 0;
        if let Some(fraction) = rest.strip_prefix('.') {
            let digits: String = fraction.chars().take_while(char::is_ascii_digit).collect();
            if digits.is_empty() {
                return None;
            }
            // `.5` is half a second, not five milliseconds.
            let scaled = format!("{digits:0<3}");
            millis = scaled.get(0..3)?.parse().ok()?;
            rest = &fraction[digits.len()..];
        }

        let offset_seconds: i64 = match rest {
            "" | "Z" | "z" => 0,
            _ => {
                let (sign, tail) = match rest.split_at(1) {
                    ("+", tail) => (1, tail),
                    ("-", tail) => (-1, tail),
                    _ => return None,
                };
                let tail = tail.strip_suffix(['Z', 'z']).unwrap_or(tail);
                let (hours, minutes) = tail.split_once(':')?;
                let hours: i64 = hours.parse().ok()?;
                let minutes: i64 = minutes.parse().ok()?;
                sign * (hours * 3600 + minutes * 60)
            }
        };

        let days = days_from_civil(year, month, day);
        let seconds = days * 86_400 + hour * 3_600 + minute * 60 + second - offset_seconds;
        if seconds < 0 {
            return None;
        }
        Some(seconds as u64 * 1_000 + millis)
    }

    /// Howard Hinnant's `days_from_civil`: days since 1970-01-01, for a proleptic
    /// Gregorian date. Written out rather than pulled in, because the alternative
    /// is a date crate for three callers.
    fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
        let year = if month <= 2 { year - 1 } else { year };
        let era = if year >= 0 { year } else { year - 399 } / 400;
        let year_of_era = year - era * 400;
        let month_prime = (month as i64 + 9) % 12;
        let day_of_year = (153 * month_prime + 2) / 5 + day as i64 - 1;
        let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
        era * 146_097 + day_of_era - 719_468
    }
}

/// GitHub Copilot CLI sessions (#279): `~/.copilot/session-state/<id>/events.jsonl`,
/// where the event stream itself records which cwd the session belongs to.
pub mod copilot {
    use super::ChatMessage;

    /// Whether the head of an event stream belongs to `cwd`.
    ///
    /// Two events carry it: `session.start` under `data.context.cwd` and
    /// `hook.start` under `data.input.cwd`. Either is enough — a session recorded
    /// by a hook-only path has no `session.start`.
    pub fn matches_cwd(head: &str, cwd: &str) -> bool {
        for line in head.lines().filter(|line| !line.is_empty()) {
            let Ok(json) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            let kind = json.get("type").and_then(|kind| kind.as_str());
            let data = json.get("data");
            let found = match kind {
                Some("session.start") => data
                    .and_then(|data| data.get("context"))
                    .and_then(|context| context.get("cwd")),
                Some("hook.start") => data
                    .and_then(|data| data.get("input"))
                    .and_then(|input| input.get("cwd")),
                _ => None,
            };
            if found.and_then(|found| found.as_str()) == Some(cwd) {
                return true;
            }
        }
        false
    }

    /// The last three turns from an event stream, and the model if it names one.
    ///
    /// Copilot is the second agent whose model is spelled three ways
    /// (`currentModel`, `model`, or the keys of `modelMetrics`); the first one seen
    /// wins, as in the Swift reader.
    pub fn recent_messages(tail: &str) -> (Option<String>, Vec<ChatMessage>) {
        let mut model: Option<String> = None;
        let mut turns: Vec<(usize, ChatMessage)> = Vec::new();
        for (index, line) in tail.lines().filter(|line| !line.is_empty()).enumerate() {
            let Ok(json) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            let kind = json.get("type").and_then(|kind| kind.as_str());
            let Some(payload) = json.get("data") else {
                continue;
            };
            if model.is_none() {
                model = ["currentModel", "model"]
                    .into_iter()
                    .find_map(|key| {
                        payload
                            .get(key)
                            .and_then(|model| model.as_str())
                            .filter(|model| !model.is_empty())
                            .map(str::to_owned)
                    })
                    .or_else(|| {
                        // `modelMetrics` is keyed by model name; the last key is the
                        // one sorted last, which is what the Swift reader takes.
                        let metrics = payload.get("modelMetrics")?.as_object()?;
                        let mut keys: Vec<&String> = metrics.keys().collect();
                        keys.sort();
                        keys.last()
                            .filter(|key| !key.is_empty())
                            .map(|key| (*key).clone())
                    });
            }
            let text = payload
                .get("content")
                .and_then(|content| content.as_str())
                .filter(|text| !text.is_empty());
            let Some(text) = text else { continue };
            let is_user = match kind {
                Some("user.message") => true,
                Some("assistant.message") => false,
                _ => continue,
            };
            turns.push((
                index,
                ChatMessage {
                    is_user,
                    text: text.to_owned(),
                },
            ));
        }
        let start = turns.len().saturating_sub(3);
        (
            model,
            turns.split_off(start).into_iter().map(|(_, t)| t).collect(),
        )
    }
}

/// Grok CLI sessions (#279): `~/<GROK_HOME>/sessions/<percent-encoded cwd>/<id>/`.
pub mod grok {
    /// Grok percent-encodes the **whole** cwd into one path component, `/` as
    /// `%2F`, so `/Users/me` becomes `%2FUsers%2Fme`.
    ///
    /// Unreserved characters only (`A-Z a-z 0-9 - . _ ~`), which is what
    /// `addingPercentEncoding(withAllowedCharacters:)` is given on the Swift side.
    /// A different allowed set produces a directory Grok never wrote, and the
    /// reader then finds nothing while looking correct.
    pub fn encoded_cwd(cwd: &str) -> String {
        let mut encoded = String::with_capacity(cwd.len());
        for byte in cwd.as_bytes() {
            let keep = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~');
            if keep {
                encoded.push(*byte as char);
            } else {
                encoded.push_str(&format!("%{byte:02X}"));
            }
        }
        encoded
    }

    /// What `summary.json` says about a session.
    #[derive(Debug, Clone, PartialEq)]
    pub struct Summary {
        pub session_id: Option<String>,
        pub cwd: Option<String>,
        pub model: Option<String>,
        /// The newest of `last_active_at`, `updated_at`, `created_at`.
        pub activity_at_ms: Option<u64>,
    }

    pub fn summary_metadata(summary_json: &str) -> Option<Summary> {
        let json = serde_json::from_str::<serde_json::Value>(summary_json).ok()?;
        let info = json.get("info");
        let string = |value: Option<&serde_json::Value>| {
            value
                .and_then(|value| value.as_str())
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
        };
        let activity = ["last_active_at", "updated_at", "created_at"]
            .into_iter()
            .filter_map(|key| {
                super::time::parse_iso8601_ms(json.get(key).and_then(|v| v.as_str())?)
            })
            .max();
        Some(Summary {
            session_id: string(info.and_then(|info| info.get("id"))),
            cwd: string(info.and_then(|info| info.get("cwd"))),
            model: string(json.get("current_model_id")),
            activity_at_ms: activity,
        })
    }

    /// Grok mints UUIDv7 session ids, whose leading 48 bits are the creation time
    /// in Unix milliseconds. That stands in for `created_at` when the summary has
    /// none, and it is the only creation time an index-only session has.
    pub fn creation_ms_from_session_id(session_id: &str) -> Option<u64> {
        let hex: String = session_id.chars().filter(|c| *c != '-').collect();
        if hex.len() != 32 {
            return None;
        }
        let first_twelve = hex.get(0..12)?;
        // Version 7 and the RFC 4122 variant, or this is not a time-ordered id.
        if hex.get(12..13)? != "7" || !matches!(hex.get(16..17)?, "8" | "9" | "a" | "b") {
            return None;
        }
        u64::from_str_radix(first_twelve, 16).ok()
    }
}

/// Cline sessions (#278): the VS Code extension `saoudrizwan.claude-dev`.
///
/// No process: Cline is an extension inside VS Code, so macOS gives it no pid
/// either — and that makes it the one reader of this batch that can be backfilled
/// with nothing running.
pub mod cline {
    use super::ChatMessage;
    use super::extract_text_content;

    /// One row of `state/taskHistory.json`, newest last.
    #[derive(Debug, Clone, PartialEq)]
    pub struct Task {
        pub id: String,
        pub cwd: Option<String>,
        pub model: Option<String>,
        /// Milliseconds since the epoch, as Cline writes it.
        pub ts_ms: Option<f64>,
    }

    /// The most recent task, by `ts`.
    ///
    /// The list is not reliably ordered — it is whatever order the extension last
    /// wrote — so the newest is chosen by timestamp rather than by position.
    pub fn latest_task(history_json: &str) -> Option<Task> {
        let history: Vec<serde_json::Value> = serde_json::from_str(history_json).ok()?;
        history
            .iter()
            .filter_map(|entry| {
                Some(Task {
                    id: entry.get("id")?.as_str()?.to_owned(),
                    cwd: entry
                        .get("cwdOnTaskInitialization")
                        .and_then(|cwd| cwd.as_str())
                        .filter(|cwd| !cwd.is_empty())
                        .map(str::to_owned),
                    model: entry
                        .get("modelId")
                        .and_then(|model| model.as_str())
                        .filter(|model| !model.is_empty())
                        .map(str::to_owned),
                    ts_ms: entry.get("ts").and_then(|ts| ts.as_f64()),
                })
            })
            .filter(|task| !task.id.is_empty())
            .max_by(|a, b| {
                a.ts_ms
                    .unwrap_or(f64::MIN)
                    .total_cmp(&b.ts_ms.unwrap_or(f64::MIN))
            })
    }

    /// The last three turns of `tasks/<id>/api_conversation_history.json`.
    pub fn recent_messages(conversation_json: &str) -> Vec<ChatMessage> {
        let Ok(entries) = serde_json::from_str::<Vec<serde_json::Value>>(conversation_json) else {
            return Vec::new();
        };
        let mut turns: Vec<(usize, ChatMessage)> = Vec::new();
        for (index, entry) in entries.iter().enumerate() {
            let Some(role) = entry.get("role").and_then(|role| role.as_str()) else {
                continue;
            };
            let Some(text) = extract_text_content(entry.get("content")) else {
                continue;
            };
            match role {
                "user" => turns.push((
                    index,
                    ChatMessage {
                        is_user: true,
                        text,
                    },
                )),
                "assistant" => turns.push((
                    index,
                    ChatMessage {
                        is_user: false,
                        text,
                    },
                )),
                _ => {}
            }
        }
        let start = turns.len().saturating_sub(3);
        turns.split_off(start).into_iter().map(|(_, t)| t).collect()
    }
}

/// Flat stores (#278): Qoder, CodeBuddy and Factory (droid) each keep JSONL
/// transcripts one directory per project, and each spells the store slightly
/// differently. What differs is only the encoding and the file's flavour.
pub mod flat_store {
    use super::ChatMessage;

    /// CodeBuddy's own transcript flavour: `{type:"message", role, content,
    /// providerData:{model}}`. Qoder and Factory write Claude's format, so they
    /// borrow `read_transcript_tail` instead.
    pub fn codebuddy_recent(tail: &str) -> (Option<String>, Vec<ChatMessage>) {
        let mut model: Option<String> = None;
        let mut turns: Vec<(usize, ChatMessage)> = Vec::new();
        for (index, line) in tail.lines().filter(|line| !line.is_empty()).enumerate() {
            let Ok(json) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if json.get("type").and_then(|kind| kind.as_str()) != Some("message") {
                continue;
            }
            let Some(role) = json.get("role").and_then(|role| role.as_str()) else {
                continue;
            };
            let Some(text) = super::extract_text_content(json.get("content")) else {
                continue;
            };
            if model.is_none() {
                model = json
                    .get("providerData")
                    .and_then(|data| data.get("model"))
                    .and_then(|model| model.as_str())
                    .filter(|model| !model.is_empty())
                    .map(str::to_owned);
            }
            match role {
                "user" => turns.push((
                    index,
                    ChatMessage {
                        is_user: true,
                        text,
                    },
                )),
                "assistant" => turns.push((
                    index,
                    ChatMessage {
                        is_user: false,
                        text,
                    },
                )),
                _ => {}
            }
        }
        let start = turns.len().saturating_sub(3);
        (
            model,
            turns.split_off(start).into_iter().map(|(_, t)| t).collect(),
        )
    }

    /// Factory keeps the model beside the transcript rather than in it:
    /// `x.jsonl` has a sibling `x.settings.json` holding `{"model": …}`.
    pub fn factory_model_from_settings(settings_json: &str) -> Option<String> {
        serde_json::from_str::<serde_json::Value>(settings_json)
            .ok()?
            .get("model")
            .and_then(|model| model.as_str())
            .filter(|model| !model.is_empty())
            .map(str::to_owned)
    }
}

/// Kimi sessions (#282): kimi-code's session index and legacy kimi-cli's per-cwd
/// directories, and the wire-protocol transcript reader for both.
pub mod kimi {
    use super::ChatMessage;

    /// Legacy kimi-cli names the per-cwd directory `md5(cwd)`.
    ///
    /// The one read of a hash anywhere in this port, and it is here because the
    /// directory name cannot be derived any other way. kimi-code's index is
    /// preferred precisely so this path is the fallback.
    pub fn workdir_hash(cwd: &str) -> String {
        use md5::{Digest, Md5};
        let digest = Md5::digest(cwd.as_bytes());
        digest.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    /// What `session_index.jsonl` says about a session.
    #[derive(Debug, Clone, PartialEq)]
    pub struct IndexEntry {
        pub session_id: String,
        /// The directory holding `agents/main/wire.jsonl`.
        pub session_dir: String,
        pub work_dir: String,
    }

    /// Every usable row of kimi-code's index.
    ///
    /// A row missing any of the three fields is skipped rather than half-used: a
    /// session without a directory has no transcript to read.
    pub fn index_entries(jsonl: &str) -> Vec<IndexEntry> {
        jsonl
            .lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| {
                let json = serde_json::from_str::<serde_json::Value>(line).ok()?;
                let field = |key: &str| {
                    json.get(key)
                        .and_then(|value| value.as_str())
                        .filter(|value| !value.is_empty())
                        .map(str::to_owned)
                };
                Some(IndexEntry {
                    session_id: field("sessionId")?,
                    session_dir: field("sessionDir")?,
                    work_dir: field("workDir")?,
                })
            })
            .collect()
    }

    /// The last three turns of a `wire.jsonl`, in either dialect.
    ///
    /// Two protocols share the file. Legacy kimi-cli wraps each record in
    /// `{"message":{"type":…}}` with `TurnBegin`/`ContentPart`/`TurnEnd`; kimi-code
    /// writes `turn.prompt` / `context.append_message` /
    /// `context.append_loop_event` at the top level. The state machine is the same
    /// for both: open a turn on the prompt, collect the assistant's parts, flush on
    /// the next prompt.
    pub fn recent_messages(tail: &str) -> Vec<ChatMessage> {
        let mut messages: Vec<ChatMessage> = Vec::new();
        let mut user: Option<String> = None;
        let mut assistant = String::new();
        // A turn opened by `context.append_message` so a later `turn.prompt` can
        // replace the user text instead of flushing a duplicate line.
        let mut opened_by_append = false;

        for line in tail.lines().filter(|line| !line.is_empty()) {
            let Ok(json) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };

            // The legacy envelope is recognised by a `type` *inside* `message`.
            // kimi-code's `context.append_message` also carries a `message`
            // object, so a bare object check would swallow it here.
            let envelope = json.get("message").filter(|value| {
                value.is_object() && value.get("type").is_some_and(|kind| kind.is_string())
            });
            if let Some(envelope) = envelope {
                let kind = envelope.get("type").and_then(|kind| kind.as_str());
                match kind {
                    Some("TurnBegin") => {
                        flush(&mut messages, &mut user, &mut assistant);
                        if let Some(payload) = envelope.get("payload") {
                            user = text_parts(payload.get("user_input"));
                        }
                    }
                    Some("ContentPart") => {
                        if let Some(payload) = envelope.get("payload") {
                            if payload.get("type").and_then(|kind| kind.as_str()) == Some("text") {
                                if let Some(text) = payload.get("text").and_then(|t| t.as_str()) {
                                    assistant.push_str(text);
                                }
                            }
                        }
                    }
                    Some("TurnEnd") => flush(&mut messages, &mut user, &mut assistant),
                    _ => {}
                }
                continue;
            }

            match json.get("type").and_then(|kind| kind.as_str()) {
                Some("turn.prompt") => {
                    let prompt = text_parts(json.get("input"));
                    if opened_by_append && assistant.is_empty() {
                        // An empty prompt must not wipe the appended line.
                        if prompt.as_deref().is_some_and(|text| !text.is_empty()) {
                            user = prompt;
                        }
                    } else {
                        flush(&mut messages, &mut user, &mut assistant);
                        user = prompt;
                    }
                    opened_by_append = false;
                }
                Some("context.append_message") => {
                    let is_user = json
                        .get("message")
                        .and_then(|message| message.get("role"))
                        .and_then(|role| role.as_str())
                        == Some("user");
                    if is_user {
                        let text = json
                            .get("message")
                            .and_then(|message| text_parts(message.get("content")));
                        if let Some(text) = text.filter(|text| !text.is_empty()) {
                            // `turn.prompt` is preferred when both exist for one
                            // turn, so only start a turn here if none is open.
                            if user.is_none() {
                                user = Some(text);
                                opened_by_append = true;
                            }
                        }
                    }
                }
                Some("context.append_loop_event") => {
                    if let Some(event) = json.get("event") {
                        if event.get("type").and_then(|kind| kind.as_str()) == Some("content.part")
                        {
                            if let Some(part) = event.get("part") {
                                if part.get("type").and_then(|kind| kind.as_str()) == Some("text") {
                                    if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                                        assistant.push_str(text);
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        flush(&mut messages, &mut user, &mut assistant);
        let start = messages.len().saturating_sub(3);
        messages.split_off(start)
    }

    fn flush(messages: &mut Vec<ChatMessage>, user: &mut Option<String>, assistant: &mut String) {
        if let Some(text) = user.take().filter(|text| !text.is_empty()) {
            messages.push(ChatMessage {
                is_user: true,
                text,
            });
            if !assistant.is_empty() {
                messages.push(ChatMessage {
                    is_user: false,
                    text: std::mem::take(assistant),
                });
            }
        }
        assistant.clear();
    }

    /// The joined `text` of a content-part list. Parts that are not text carry no
    /// words to show, so they contribute nothing.
    fn text_parts(value: Option<&serde_json::Value>) -> Option<String> {
        let parts = value?.as_array()?;
        let joined: String = parts
            .iter()
            .filter(
                |part| match part.get("type").and_then(|kind| kind.as_str()) {
                    Some(kind) => kind == "text",
                    None => true,
                },
            )
            .filter_map(|part| part.get("text").and_then(|text| text.as_str()))
            .collect();
        Some(joined)
    }
}

/// Kiro CLI sessions (#282): `~/.kiro/sessions/<workspace>/<session>/session.json`
/// and the two older layouts beside it.
pub mod kiro {
    use super::ChatMessage;

    /// What `session.json` says about a session.
    #[derive(Debug, Clone, PartialEq)]
    pub struct Metadata {
        pub session_id: Option<String>,
        /// `workspacePaths` first, then `rootPaths`: the workspace can list
        /// several roots, and the first is the one the session was opened in.
        pub cwd: Option<String>,
        pub model: Option<String>,
        pub activity_at_ms: Option<u64>,
    }

    pub fn session_metadata(session_json: &str) -> Option<Metadata> {
        let json = serde_json::from_str::<serde_json::Value>(session_json).ok()?;
        let first_string = |key: &str| {
            json.get(key)?
                .as_array()?
                .iter()
                .find_map(|value| value.as_str().filter(|value| !value.is_empty()))
                .map(str::to_owned)
        };
        Some(Metadata {
            session_id: json
                .get("id")
                .and_then(|id| id.as_str())
                .filter(|id| !id.is_empty())
                .map(str::to_owned),
            cwd: first_string("workspacePaths").or_else(|| first_string("rootPaths")),
            model: json
                .get("modelId")
                .and_then(|model| model.as_str())
                .filter(|model| !model.is_empty())
                .map(str::to_owned),
            activity_at_ms: json
                .get("lastModifiedAt")
                .and_then(|stamp| stamp.as_str())
                .and_then(super::time::parse_iso8601_ms),
        })
    }

    /// The last three turns of `messages.jsonl`, across the three layouts Kiro has
    /// shipped.
    ///
    /// They are recognised by shape rather than by a version field, which is what
    /// the Swift reader does too — the newest layout has no version marker:
    ///
    /// | shape | how it reads |
    /// |---|---|
    /// | `payload.type` | V3: `user`/`assistant` with a string `content` |
    /// | `type` | direct: `message.content`, any shape `extractTextContent` knows |
    /// | `kind == "Prompt"` | V2: `data.content[].data` |
    ///
    /// `payload.reasoningModelId` is the model when nothing else names one, with
    /// Kiro's `qdev::` prefix removed.
    pub fn recent_messages(tail: &str) -> (Option<String>, Vec<ChatMessage>) {
        let mut users: Vec<(usize, String)> = Vec::new();
        let mut assistants: Vec<(usize, String)> = Vec::new();
        let mut detected_model: Option<String> = None;

        for (index, line) in tail.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(json) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };

            if let Some(payload) = json.get("payload").filter(|value| value.is_object()) {
                let kind = payload.get("type").and_then(|kind| kind.as_str());
                let content = payload
                    .get("content")
                    .and_then(|content| content.as_str())
                    .filter(|content| !content.is_empty());
                match (kind, content) {
                    (Some("user"), Some(text)) => users.push((index, text.to_owned())),
                    (Some("assistant"), Some(text)) => assistants.push((index, text.to_owned())),
                    _ => {
                        if let Some(model) = payload
                            .get("reasoningModelId")
                            .and_then(|model| model.as_str())
                        {
                            detected_model = Some(model.replace("qdev::", ""));
                        }
                    }
                }
                continue;
            }

            if let Some(kind) = json.get("type").and_then(|kind| kind.as_str()) {
                // A direct row may still nest the turn under `message`.
                let message = json.get("message").unwrap_or(&json);
                let text = super::extract_text_content(message.get("content"));
                match (kind, text) {
                    ("user", Some(text)) => users.push((index, text)),
                    ("assistant", Some(text)) => assistants.push((index, text)),
                    _ => {}
                }
                continue;
            }

            if json.get("kind").and_then(|kind| kind.as_str()) == Some("Prompt") {
                let prompt = json
                    .get("data")
                    .and_then(|data| data.get("content"))
                    .and_then(|content| content.as_array());
                if let Some(prompt) = prompt {
                    for item in prompt {
                        if let Some(text) = item
                            .get("data")
                            .and_then(|data| data.as_str())
                            .filter(|text| !text.is_empty())
                        {
                            users.push((index, text.to_owned()));
                        }
                    }
                }
            }
        }

        let start = users.len().saturating_sub(3);
        let mut combined: Vec<(usize, ChatMessage)> = users
            .split_off(start)
            .into_iter()
            .map(|(index, text)| {
                (
                    index,
                    ChatMessage {
                        is_user: true,
                        text,
                    },
                )
            })
            .collect();
        let start = assistants.len().saturating_sub(3);
        combined.extend(
            assistants
                .split_off(start)
                .into_iter()
                .map(|(index, text)| {
                    (
                        index,
                        ChatMessage {
                            is_user: false,
                            text,
                        },
                    )
                }),
        );
        combined.sort_by_key(|(index, _)| *index);
        let start = combined.len().saturating_sub(3);
        (
            detected_model,
            combined
                .split_off(start)
                .into_iter()
                .map(|(_, t)| t)
                .collect(),
        )
    }
}

#[cfg(test)]
mod kimi_tests {
    use super::kimi;

    #[test]
    fn the_legacy_directory_is_the_md5_of_the_cwd() {
        // The one hash in this port, and it is here because the directory name
        // cannot be derived any other way. Known vectors, so a wrong digest is not
        // a directory that merely looks plausible.
        assert_eq!(kimi::workdir_hash(""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(
            kimi::workdir_hash("abc"),
            "900150983cd24fb0d6963f7d28e17f72"
        );
        assert_eq!(kimi::workdir_hash("/a").len(), 32);
        assert_ne!(kimi::workdir_hash("/a"), kimi::workdir_hash("/b"));
    }

    #[test]
    fn an_index_row_missing_a_field_is_skipped_rather_than_half_used() {
        let jsonl = concat!(
            r#"{"sessionId":"s1","sessionDir":"/k/s1","workDir":"/home/dev/p"}"#,
            "\n",
            r#"{"sessionId":"s2","workDir":"/home/dev/p"}"#,
            "\n",
            "not json",
            "\n",
            r#"{"sessionId":"","sessionDir":"/k/s3","workDir":"/home/dev/p"}"#
        );
        let entries = kimi::index_entries(jsonl);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].session_id, "s1");
        assert_eq!(entries[0].session_dir, "/k/s1");
        assert_eq!(entries[0].work_dir, "/home/dev/p");
    }

    #[test]
    fn the_kimi_code_dialect_becomes_turns() {
        let tail = r#"{"type":"turn.prompt","input":[{"type":"text","text":"Why is it slow?"}]}
{"type":"context.append_loop_event","event":{"type":"content.part","part":{"type":"text","text":"Because of the "}}}
{"type":"context.append_loop_event","event":{"type":"content.part","part":{"type":"text","text":"N+1 query."}}}"#;
        let messages = kimi::recent_messages(tail);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].text, "Why is it slow?");
        assert!(messages[0].is_user);
        // Two parts of one answer are one answer, not two.
        assert_eq!(messages[1].text, "Because of the N+1 query.");
    }

    #[test]
    fn the_legacy_dialect_becomes_the_same_turns() {
        let tail = r#"{"message":{"type":"TurnBegin","payload":{"user_input":[{"type":"text","text":"Hello"}]}}}
{"message":{"type":"ContentPart","payload":{"type":"text","text":"Hi there"}}}
{"message":{"type":"TurnEnd"}}"#;
        let messages = kimi::recent_messages(tail);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].text, "Hello");
        assert_eq!(messages[1].text, "Hi there");
        assert!(!messages[1].is_user);
    }

    #[test]
    fn a_prompt_after_an_appended_message_replaces_it_instead_of_duplicating() {
        // kimi-code writes `turn.prompt`, then `context.append_message` with the
        // same text. Two user lines for one turn would be a visible duplicate.
        let tail = r#"{"type":"turn.prompt","input":[{"type":"text","text":"One question"}]}
{"type":"context.append_message","message":{"role":"user","content":[{"type":"text","text":"One question"}]}}"#;
        let messages = kimi::recent_messages(tail);
        assert_eq!(messages.len(), 1, "one turn, one user line");
        assert_eq!(messages[0].text, "One question");
    }

    #[test]
    fn an_appended_message_opens_a_turn_when_no_prompt_came_first() {
        let tail = r#"{"type":"context.append_message","message":{"role":"user","content":[{"type":"text","text":"From a hook"}]}}"#;
        let messages = kimi::recent_messages(tail);
        assert_eq!(messages.len(), 1);
        assert!(messages[0].is_user);
    }

    #[test]
    fn only_the_last_three_turns_are_kept() {
        let mut lines = Vec::new();
        for turn in 1..=5 {
            lines.push(format!(
                r#"{{"type":"turn.prompt","input":[{{"type":"text","text":"ask {turn}"}}]}}"#
            ));
            lines.push(format!(
                r#"{{"type":"context.append_loop_event","event":{{"type":"content.part","part":{{"type":"text","text":"answer {turn}"}}}}}}"#
            ));
        }
        let messages = kimi::recent_messages(&lines.join("\n"));
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].text, "answer 4");
    }
}

#[cfg(test)]
mod kiro_tests {
    use super::kiro;

    #[test]
    fn the_first_workspace_path_is_the_cwd() {
        let json = r#"{"id":"s1","workspacePaths":["/home/dev/p","/home/dev/q"],
                       "modelId":"claude-sonnet","lastModifiedAt":"2025-11-22T10:00:00Z"}"#;
        let meta = kiro::session_metadata(json).expect("metadata");
        assert_eq!(meta.session_id.as_deref(), Some("s1"));
        assert_eq!(meta.cwd.as_deref(), Some("/home/dev/p"));
        assert_eq!(meta.model.as_deref(), Some("claude-sonnet"));
        assert_eq!(meta.activity_at_ms, Some(1_763_805_600_000));
    }

    #[test]
    fn root_paths_are_the_fallback_when_there_is_no_workspace_list() {
        let json = r#"{"rootPaths":["/home/dev/root"]}"#;
        assert_eq!(
            kiro::session_metadata(json).unwrap().cwd.as_deref(),
            Some("/home/dev/root")
        );
        assert_eq!(
            kiro::session_metadata(r#"{"workspacePaths":[],"rootPaths":["/r"]}"#)
                .unwrap()
                .cwd
                .as_deref(),
            Some("/r")
        );
        assert_eq!(kiro::session_metadata("not json"), None);
    }

    #[test]
    fn the_v3_payload_shape_becomes_turns_and_names_the_model() {
        let tail = r#"{"payload":{"type":"user","content":"Refactor this"}}
{"payload":{"type":"assistant","content":"Done"}}
{"payload":{"reasoningModelId":"qdev::claude-opus"}}"#;
        let (model, messages) = kiro::recent_messages(tail);
        assert_eq!(
            model.as_deref(),
            Some("claude-opus"),
            "the qdev:: prefix is Kiro's"
        );
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].text, "Refactor this");
        assert_eq!(messages[1].text, "Done");
    }

    #[test]
    fn the_direct_shape_reads_nested_content() {
        let tail = r#"{"type":"user","message":{"content":[{"type":"text","text":"Run the tests"}]}}
{"type":"assistant","message":{"content":[{"type":"text","text":"All green"}]}}"#;
        let (model, messages) = kiro::recent_messages(tail);
        assert_eq!(model, None);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].text, "Run the tests");
    }

    #[test]
    fn the_v2_prompt_shape_becomes_one_user_turn_per_part() {
        let tail = r#"{"kind":"Prompt","data":{"content":[{"data":"first"},{"data":"second"}]}}"#;
        let (_, messages) = kiro::recent_messages(tail);
        assert_eq!(messages.len(), 2);
        assert!(messages.iter().all(|message| message.is_user));
        assert_eq!(messages[1].text, "second");
    }

    #[test]
    fn the_last_three_turns_are_chosen_from_both_roles_in_order() {
        let tail = r#"{"payload":{"type":"user","content":"one"}}
{"payload":{"type":"assistant","content":"two"}}
{"payload":{"type":"user","content":"three"}}
{"payload":{"type":"assistant","content":"four"}}
{"payload":{"type":"user","content":"five"}}"#;
        let (_, messages) = kiro::recent_messages(tail);
        // The last three per role are collected, then merged by line index and cut
        // again — so the cut is chronological, not "three users then three replies".
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].text, "three");
        assert!(messages[0].is_user);
        assert_eq!(messages[1].text, "four");
        assert!(!messages[1].is_user);
        assert_eq!(messages[2].text, "five");
    }
}

#[cfg(test)]
mod time_tests {
    use super::time::parse_iso8601_ms;

    #[test]
    fn a_utc_timestamp_is_milliseconds_since_the_epoch() {
        assert_eq!(parse_iso8601_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_iso8601_ms("1970-01-01T00:00:01Z"), Some(1_000));
        // A known instant, so a wrong epoch or a wrong leap-day rule shows up.
        assert_eq!(
            parse_iso8601_ms("2024-02-29T12:00:00Z"),
            Some(1_709_208_000_000)
        );
    }

    #[test]
    fn a_fraction_is_a_fraction_of_a_second() {
        assert_eq!(parse_iso8601_ms("1970-01-01T00:00:00.5Z"), Some(500));
        assert_eq!(parse_iso8601_ms("1970-01-01T00:00:00.123Z"), Some(123));
        assert_eq!(parse_iso8601_ms("1970-01-01T00:00:00.123456Z"), Some(123));
    }

    #[test]
    fn an_offset_is_applied_rather_than_ignored() {
        assert_eq!(
            parse_iso8601_ms("1970-01-01T07:00:00+07:00"),
            Some(0),
            "Jakarta is seven hours ahead"
        );
        assert_eq!(parse_iso8601_ms("1969-12-31T19:00:00-05:00"), Some(0));
    }

    #[test]
    fn a_space_instead_of_a_t_is_accepted_and_anything_else_is_not() {
        assert_eq!(parse_iso8601_ms("1970-01-01 00:00:00Z"), Some(0));
        assert_eq!(parse_iso8601_ms(""), None);
        assert_eq!(parse_iso8601_ms("1970-01-01"), None);
        assert_eq!(parse_iso8601_ms("19700101T000000Z"), None);
        assert_eq!(parse_iso8601_ms("not a timestamp"), None);
        assert_eq!(parse_iso8601_ms("1970-13-01T00:00:00Z"), None, "month 13");
    }
}

#[cfg(test)]
mod grok_tests {
    use super::grok;

    #[test]
    fn the_whole_cwd_becomes_one_encoded_component() {
        assert_eq!(grok::encoded_cwd("/Users/me"), "%2FUsers%2Fme");
        assert_eq!(
            grok::encoded_cwd("/home/dev/project"),
            "%2Fhome%2Fdev%2Fproject"
        );
        // Unreserved characters survive: a different allowed set reads a directory
        // Grok never wrote.
        assert_eq!(grok::encoded_cwd("/a/b-c.d_e~f"), "%2Fa%2Fb-c.d_e~f");
        // A space is encoded, not turned into `-`: this is not Claude's encoding.
        assert_eq!(grok::encoded_cwd("/a b"), "%2Fa%20b");
    }

    #[test]
    fn the_summary_names_the_cwd_the_model_and_the_newest_activity() {
        let json = r#"{
            "info": {"id": "0192f0aa-7cbb-7000-8000-000000000000", "cwd": "/home/dev/p"},
            "current_model_id": "grok-code-fast-1",
            "created_at": "2025-11-22T10:00:00Z",
            "updated_at": "2025-11-22T10:05:00Z",
            "last_active_at": "2025-11-22T10:02:00Z"
        }"#;
        let summary = grok::summary_metadata(json).expect("a summary");
        assert_eq!(summary.cwd.as_deref(), Some("/home/dev/p"));
        assert_eq!(summary.model.as_deref(), Some("grok-code-fast-1"));
        assert_eq!(
            summary.activity_at_ms,
            Some(1_763_805_900_000),
            "updated_at is the newest of the three"
        );
    }

    #[test]
    fn an_empty_field_is_absent_rather_than_empty() {
        let json = r#"{"info":{"id":"","cwd":""},"current_model_id":""}"#;
        let summary = grok::summary_metadata(json).expect("a summary");
        assert_eq!(summary.session_id, None);
        assert_eq!(summary.cwd, None);
        assert_eq!(summary.model, None);
        assert_eq!(summary.activity_at_ms, None);
        assert_eq!(grok::summary_metadata("not json"), None);
    }

    #[test]
    fn a_uuidv7_carries_its_creation_time_in_the_first_48_bits() {
        // 0x0192f0aa7cbb = 1_730_614_557_883 ms.
        assert_eq!(
            grok::creation_ms_from_session_id("0192f0aa-7cbb-7000-8000-000000000000"),
            Some(1_730_614_557_883)
        );
    }

    #[test]
    fn an_id_that_is_not_a_uuidv7_has_no_creation_time() {
        // A client may pass its own id with `-s`, and a UUIDv4 has no time in it.
        assert_eq!(grok::creation_ms_from_session_id("my-own-id"), None);
        assert_eq!(
            grok::creation_ms_from_session_id("2f0aa7cb-b123-4000-8000-000000000000"),
            None,
            "version 4, not 7"
        );
        assert_eq!(grok::creation_ms_from_session_id(""), None);
    }
}

#[cfg(test)]
mod copilot_tests {
    use super::copilot;

    fn events(lines: &[&str]) -> String {
        lines.join("\n")
    }

    #[test]
    fn a_session_start_names_its_cwd() {
        let head =
            events(&[r#"{"type":"session.start","data":{"context":{"cwd":"/home/dev/p"}}}"#]);
        assert!(copilot::matches_cwd(&head, "/home/dev/p"));
        assert!(!copilot::matches_cwd(&head, "/home/dev/other"));
    }

    #[test]
    fn a_hook_only_session_names_its_cwd_too() {
        // A session recorded through the hook path has no `session.start`.
        let head = events(&[r#"{"type":"hook.start","data":{"input":{"cwd":"/home/dev/p"}}}"#]);
        assert!(copilot::matches_cwd(&head, "/home/dev/p"));
    }

    #[test]
    fn an_unreadable_head_matches_nothing() {
        let head = events(&["not json", r#"{"type":"user.message","data":{}}"#]);
        assert!(!copilot::matches_cwd(&head, "/home/dev/p"));
        assert!(!copilot::matches_cwd("", ""));
    }

    #[test]
    fn the_last_three_turns_keep_their_order() {
        let tail = events(&[
            r#"{"type":"user.message","data":{"content":"one"}}"#,
            r#"{"type":"assistant.message","data":{"content":"two"}}"#,
            r#"{"type":"user.message","data":{"content":"three"}}"#,
            r#"{"type":"assistant.message","data":{"content":"four"}}"#,
            r#"{"type":"user.message","data":{"content":"five"}}"#,
        ]);
        let (_, messages) = copilot::recent_messages(&tail);
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].text, "three");
        assert!(messages[2].is_user);
    }

    #[test]
    fn the_model_is_spelled_three_ways_and_the_first_one_wins() {
        let tail = events(&[r#"{"type":"session.info","data":{"currentModel":"gpt-5"}}"#]);
        assert_eq!(copilot::recent_messages(&tail).0.as_deref(), Some("gpt-5"));
        let tail = events(&[r#"{"type":"session.info","data":{"model":"claude-sonnet"}}"#]);
        assert_eq!(
            copilot::recent_messages(&tail).0.as_deref(),
            Some("claude-sonnet")
        );
        // `modelMetrics` is keyed by model name; the last key wins.
        let tail = events(&[
            r#"{"type":"session.info","data":{"modelMetrics":{"a-model":1,"b-model":2}}}"#,
        ]);
        assert_eq!(
            copilot::recent_messages(&tail).0.as_deref(),
            Some("b-model")
        );
    }

    #[test]
    fn a_content_that_is_not_a_string_is_skipped() {
        let tail = events(&[
            r#"{"type":"user.message","data":{"content":[{"type":"text","text":"no"}]}}"#,
            r#"{"type":"user.message","data":{"content":"yes"}}"#,
        ]);
        let (_, messages) = copilot::recent_messages(&tail);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "yes");
    }
}

#[cfg(test)]
mod cline_tests {
    use super::cline;

    fn history(entries: &str) -> String {
        format!("[{entries}]")
    }

    #[test]
    fn the_newest_task_is_chosen_by_timestamp_not_by_position() {
        // Cline rewrites the list in whatever order the extension last saved, so
        // the newest row is not reliably last.
        let json = history(
            r#"{"id":"new","ts":2000,"cwdOnTaskInitialization":"/a"},
               {"id":"old","ts":1000,"cwdOnTaskInitialization":"/b"}"#,
        );
        let task = cline::latest_task(&json).expect("a task");
        assert_eq!(task.id, "new");
        assert_eq!(task.cwd.as_deref(), Some("/a"));
    }

    #[test]
    fn a_row_without_an_id_is_not_a_task() {
        let json = history(r#"{"ts":1,"cwdOnTaskInitialization":"/a"}"#);
        assert_eq!(cline::latest_task(&json), None);
        assert_eq!(cline::latest_task("not json"), None);
        assert_eq!(cline::latest_task("{}"), None);
    }

    #[test]
    fn an_empty_cwd_or_model_is_absent_rather_than_empty() {
        let json = history(r#"{"id":"t","cwdOnTaskInitialization":"","modelId":""}"#);
        let task = cline::latest_task(&json).expect("a task");
        assert_eq!(task.cwd, None);
        assert_eq!(task.model, None);
    }

    #[test]
    fn the_last_three_turns_keep_their_order() {
        let conversation = r#"[
            {"role":"user","content":"one"},
            {"role":"assistant","content":"two"},
            {"role":"user","content":"three"},
            {"role":"assistant","content":"four"},
            {"role":"user","content":"five"}
        ]"#;
        let messages = cline::recent_messages(conversation);
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].text, "three");
        assert_eq!(messages[1].text, "four");
        assert_eq!(messages[2].text, "five");
        assert!(messages[2].is_user);
    }

    #[test]
    fn a_role_that_is_neither_user_nor_assistant_is_skipped() {
        let conversation = r#"[
            {"role":"system","content":"ignore me"},
            {"role":"user","content":"hello"}
        ]"#;
        let messages = cline::recent_messages(conversation);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "hello");
        assert!(cline::recent_messages("not json").is_empty());
    }
}

#[cfg(test)]
mod flat_store_tests {
    use super::flat_store;

    #[test]
    fn codebuddy_keeps_the_model_from_its_first_message() {
        let tail = concat!(
            r#"{"type":"message","role":"user","content":"add a test","providerData":{"model":"claude-sonnet"}}"#,
            "\n",
            r#"{"type":"message","role":"assistant","content":"added","providerData":{"model":"other"}}"#
        );
        let (model, messages) = flat_store::codebuddy_recent(tail);
        assert_eq!(model.as_deref(), Some("claude-sonnet"));
        assert_eq!(messages.len(), 2);
        assert!(messages[0].is_user);
    }

    #[test]
    fn codebuddy_rows_that_are_not_messages_are_skipped() {
        let tail = concat!(
            r#"{"type":"summary","role":"user","content":"no"}"#,
            "\n",
            r#"{"type":"message","role":"user","content":"yes"}"#
        );
        let (model, messages) = flat_store::codebuddy_recent(tail);
        assert_eq!(model, None);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "yes");
    }

    #[test]
    fn factory_keeps_its_model_beside_the_transcript() {
        assert_eq!(
            flat_store::factory_model_from_settings(r#"{"model":"gpt-5.1-codex"}"#).as_deref(),
            Some("gpt-5.1-codex")
        );
        assert_eq!(
            flat_store::factory_model_from_settings(r#"{"model":""}"#),
            None
        );
        assert_eq!(flat_store::factory_model_from_settings("not json"), None);
        assert_eq!(flat_store::factory_model_from_settings("{}"), None);
    }
}

#[cfg(test)]
mod gemini_map_tests {
    use super::gemini;

    #[test]
    fn the_projects_map_names_a_chat_folder_for_a_working_directory() {
        let json = r#"{"projects":{"/home/dev/a":"abc123","/home/dev/b":"def456"}}"#;
        let map = gemini::projects_map(json);
        assert_eq!(map.get("/home/dev/a").map(String::as_str), Some("abc123"));
        assert_eq!(map.get("/home/dev/b").map(String::as_str), Some("def456"));
        assert_eq!(map.get("/home/dev/c"), None);
    }

    #[test]
    fn a_map_that_is_not_a_map_is_empty_rather_than_a_panic() {
        assert!(gemini::projects_map("").is_empty());
        assert!(gemini::projects_map("not json").is_empty());
        assert!(gemini::projects_map("{}").is_empty());
        assert!(gemini::projects_map(r#"{"projects":"nope"}"#).is_empty());
        // A non-string entry is skipped, the rest survives.
        assert_eq!(
            gemini::projects_map(r#"{"projects":{"/a":1,"/b":"two"}}"#)
                .get("/b")
                .map(String::as_str),
            Some("two")
        );
    }

    #[test]
    fn a_project_root_file_matches_its_working_directory_trimmed() {
        assert!(gemini::project_root_matches("/home/dev/a\n", "/home/dev/a"));
        assert!(gemini::project_root_matches(
            "  /home/dev/a  ",
            "/home/dev/a"
        ));
        assert!(!gemini::project_root_matches("/home/dev/b", "/home/dev/a"));
        assert!(!gemini::project_root_matches("", "/home/dev/a"));
    }
}

#[cfg(test)]
mod codex_rollout_tests {
    use super::read_codex_rollout_tail;

    const ROLLOUT: &str = concat!(
        r#"{"type":"session_meta","payload":{"id":"thread-1","cwd":"/home/dev/project","originator":"codex_cli"}}"#,
        "\n",
        r#"{"type":"turn_context","payload":{"model":"gpt-5-codex"}}"#,
        "\n",
        r#"{"type":"event_msg","payload":{"type":"user_message","message":"Add a filter bar"}}"#,
        "\n",
        r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Added the bar"}]}}"#,
        "\n",
    );

    #[test]
    fn a_rollout_names_its_thread_and_working_directory() {
        let summary =
            read_codex_rollout_tail(ROLLOUT, "rollout-2026-11-22T10-00-00-thread-1.jsonl")
                .expect("a summary");
        assert_eq!(summary.session_id, "thread-1");
        assert_eq!(summary.cwd, "/home/dev/project");
    }

    /// `session_meta` names the provider, not the model. Only `turn_context` is a
    /// label, and the last one wins because it is what the newest turn ran on.
    #[test]
    fn the_model_comes_from_the_last_turn_context() {
        let rollout = format!(
            "{ROLLOUT}{}\n",
            r#"{"type":"turn_context","payload":{"model":"gpt-5"}}"#
        );
        assert_eq!(
            read_codex_rollout_tail(&rollout, "r.jsonl")
                .expect("a summary")
                .model
                .as_deref(),
            Some("gpt-5")
        );

        // A session_meta with no turn_context has no label, not "openai".
        let meta_only =
            r#"{"type":"session_meta","payload":{"id":"t","cwd":"/w","model":"openai"}}"#;
        assert_eq!(
            read_codex_rollout_tail(meta_only, "r.jsonl")
                .expect("a summary")
                .model,
            None
        );
    }

    #[test]
    fn a_user_turn_comes_from_the_event_message() {
        let summary = read_codex_rollout_tail(ROLLOUT, "r.jsonl").expect("a summary");
        let rendered: Vec<(bool, &str)> = summary
            .recent_messages
            .iter()
            .map(|message| (message.is_user, message.text.as_str()))
            .collect();
        assert_eq!(
            rendered,
            vec![(true, "Add a filter bar"), (false, "Added the bar")]
        );
    }

    #[test]
    fn at_most_three_turns_are_kept() {
        let mut rollout =
            String::from(r#"{"type":"session_meta","payload":{"id":"t","cwd":"/w"}}"#);
        for index in 0..5 {
            rollout.push('\n');
            rollout.push_str(&format!(
                r#"{{"type":"event_msg","payload":{{"type":"user_message","message":"ask {index}"}}}}"#
            ));
            rollout.push('\n');
            rollout.push_str(&format!(
                r#"{{"type":"event_msg","payload":{{"type":"agent_message","message":"answer {index}"}}}}"#
            ));
        }
        let summary = read_codex_rollout_tail(&rollout, "r.jsonl").expect("a summary");
        let texts: Vec<&str> = summary
            .recent_messages
            .iter()
            .map(|message| message.text.as_str())
            .collect();
        assert_eq!(texts.len(), 3);
        assert_eq!(texts, vec!["answer 3", "ask 4", "answer 4"]);
    }

    #[test]
    fn the_session_id_falls_back_to_the_file_name() {
        let rollout = r#"{"type":"event_msg","payload":{"type":"user_message","message":"hi"}}"#;
        let summary = read_codex_rollout_tail(rollout, "rollout-2026-11-22T10-00-00-abc123.jsonl")
            .expect("a summary");
        assert_eq!(summary.session_id, "abc123");
    }

    #[test]
    fn an_unusable_rollout_is_none_rather_than_an_empty_session() {
        assert_eq!(read_codex_rollout_tail("", "r.jsonl"), None);
        assert_eq!(read_codex_rollout_tail("not json\n", "r.jsonl"), None);
        // A cut first line is normal for a tail; the rest still reads.
        let cut = format!("{{\"type\":\"session_meta\",\"payl\n{ROLLOUT}");
        assert_eq!(
            read_codex_rollout_tail(&cut, "r.jsonl")
                .expect("a summary")
                .session_id,
            "thread-1"
        );
    }
}

#[cfg(test)]
mod transcript_summary_tests {
    use super::read_transcript_tail;

    const TAIL: &str = r#"{"type":"user","cwd":"/home/dev/project","sessionId":"abc","message":{"role":"user","content":"Add a filter bar"}}
{"type":"assistant","cwd":"/home/dev/project","message":{"model":"claude-sonnet-4","content":[{"type":"text","text":"Added the bar"}],"role":"assistant"}}
{"type":"user","cwd":"/home/dev/project","message":{"role":"user","content":[{"type":"tool_result","content":"ok"}]}}
{"type":"assistant","cwd":"/home/dev/project","message":{"content":[{"type":"text","text":"Anything else?"}]}}
"#;

    #[test]
    fn a_transcript_tail_names_its_session_and_working_directory() {
        let summary = read_transcript_tail(TAIL, "abc.jsonl").expect("a summary");
        assert_eq!(summary.session_id, "abc");
        assert_eq!(summary.cwd, "/home/dev/project");
        assert_eq!(summary.model.as_deref(), Some("claude-sonnet-4"));
    }

    /// The model is on the assistant rows, and an empty one is not a name.
    #[test]
    fn the_model_comes_from_the_first_row_that_names_one() {
        let tail = r#"{"type":"assistant","cwd":"/w","message":{"model":"","content":"Hi"}}
{"type":"assistant","cwd":"/w","message":{"model":"model-x","content":"Later"}}
"#;
        assert_eq!(
            read_transcript_tail(tail, "s.jsonl")
                .expect("a summary")
                .model
                .as_deref(),
            Some("model-x")
        );
    }

    /// A tool result is a `user` row but not something the user said. Showing it
    /// would put JSON on the card.
    #[test]
    fn a_tool_result_is_not_a_turn() {
        let summary = read_transcript_tail(TAIL, "abc.jsonl").expect("a summary");
        let texts: Vec<&str> = summary
            .recent_messages
            .iter()
            .map(|message| message.text.as_str())
            .collect();
        assert_eq!(
            texts,
            vec!["Add a filter bar", "Added the bar", "Anything else?"]
        );
        let kinds: Vec<bool> = summary
            .recent_messages
            .iter()
            .map(|message| message.is_user)
            .collect();
        assert_eq!(kinds, vec![true, false, false]);
    }

    #[test]
    fn at_most_three_turns_are_kept() {
        let tail = (0..6)
            .map(|index| {
                let kind = if index % 2 == 0 { "user" } else { "assistant" };
                format!(
                    "{{\"type\":\"{kind}\",\"cwd\":\"/w\",\"message\":{{\"content\":\"turn {index}\"}}}}"
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let summary = read_transcript_tail(&tail, "s.jsonl").expect("a summary");
        let texts: Vec<&str> = summary
            .recent_messages
            .iter()
            .map(|message| message.text.as_str())
            .collect();
        assert_eq!(texts, vec!["turn 3", "turn 4", "turn 5"]);
    }

    #[test]
    fn nothing_usable_is_none_rather_than_an_empty_session() {
        assert_eq!(read_transcript_tail("", "abc.jsonl"), None);
        assert_eq!(
            read_transcript_tail("{\"type\":\"user\",\"mess", "abc.jsonl"),
            None
        );
        // A file that is not a transcript at all.
        assert_eq!(read_transcript_tail("not json\n", "abc.jsonl"), None);
        // Not a `.jsonl` name: the session id would be wrong.
        assert_eq!(read_transcript_tail(TAIL, "abc.json"), None);
        // An empty id is not an id.
        assert_eq!(read_transcript_tail(TAIL, ".jsonl"), None);
    }

    /// A tail that starts mid-file is normal: the first line may be cut. The
    /// rest still has to read.
    #[test]
    fn a_cut_first_line_does_not_lose_the_rest() {
        let cut = "{\"type\":\"user\",\"cwd\":";
        let tail = format!("{cut}\n{TAIL}");
        let summary = read_transcript_tail(&tail, "abc.jsonl").expect("a summary");
        assert_eq!(summary.cwd, "/home/dev/project");
        assert!(!summary.recent_messages.is_empty());
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

    /// A session as the store holds it, without a process to explain it.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct SessionRow {
        pub session_id: String,
        pub cwd: String,
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

    /// Every stored session worth showing, newest first — the **backfill** query.
    ///
    /// `find_recent_session` answers "which session is this process running",
    /// which needs a cwd. A backfill has no process and no cwd: it asks the store
    /// what it holds, and the store's own `directory` column becomes the card's
    /// working directory.
    ///
    /// Archived rows are skipped, because they are what the CLI itself hides.
    pub fn recent_sessions(connection: &Connection, limit: usize) -> Vec<SessionRow> {
        let columns = crate::sqlite::table_columns(connection, "session");
        if columns.is_empty() || !columns.iter().any(|column| column == "directory") {
            return Vec::new();
        }
        let mut conditions: Vec<&str> = Vec::new();
        if columns.iter().any(|column| column == "time_archived") {
            conditions.push("time_archived IS NULL");
        }
        if columns.iter().any(|column| column == "parent_id") {
            // A subagent session is never the CLI's own card.
            conditions.push("parent_id IS NULL");
        }
        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };
        let sql = format!(
            "SELECT id, directory, time_updated FROM session{where_clause}
             ORDER BY time_updated DESC LIMIT {limit};"
        );
        let Ok(mut statement) = connection.prepare(&sql) else {
            return Vec::new();
        };
        let Ok(mut rows) = statement.query([]) else {
            return Vec::new();
        };
        let mut sessions = Vec::new();
        while let Ok(Some(row)) = rows.next() {
            let Ok(session_id) = row.get::<_, String>(0) else {
                continue;
            };
            let Ok(updated_ms) = row.get::<_, i64>(2) else {
                continue;
            };
            // A row with no directory cannot make a card: it has no working
            // directory to show and no cwd to correlate against.
            let Some(cwd) = non_empty_text(row, 1) else {
                continue;
            };
            sessions.push(SessionRow {
                session_id,
                cwd,
                modified_ms: updated_ms,
            });
        }
        sessions
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

        /// The roots this reader looks at.
        ///
        /// Declared by the reader rather than passed in, because they are not the
        /// same kind of place: Claude Code keeps one store per config root
        /// (`~/.claude`, `~/.claude-work`), while OpenCode keeps a single database
        /// under `~/.local/share`. A caller holding one list would either re-read
        /// the database once per config root or have to know each reader's
        /// layout.
        fn roots(&self) -> Vec<String>;

        /// Every session this root holds. A root the reader does not understand
        /// answers an empty list, not an error: one unknown store must not lose
        /// the others.
        fn read(&self, root: &str) -> Vec<DiscoveryHit>;
    }

    /// A decoder that needs a **process** before it can find anything (#276).
    ///
    /// Cursor and Gemini keep their store under a directory derived from the
    /// working directory of a running CLI:
    /// `~/.cursor/projects/<encoded cwd>/agent-transcripts`. There is nothing to
    /// read without a cwd, which is why these are not `StoreReader`s — and why they
    /// are worth having anyway: the process that pointed at the store is the pid
    /// the card needs, so no correlation is required.
    pub trait ProcessReader {
        /// The agent this reads, as `--source` spells it.
        fn source(&self) -> &'static str;

        /// The sessions this process is running. A process whose store is not
        /// there, or is stale, answers nothing.
        fn read_process(&self, process: &crate::agent_process::ProcessInfo) -> Vec<DiscoveryHit>;
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
        readers: &[&dyn StoreReader],
        budget_ms: u64,
        now_ms: impl Fn() -> u64,
    ) -> ScanResult {
        run_all(readers, &[], &[], budget_ms, now_ms)
    }

    /// Every reader this port has: the store-first ones over their own roots, then
    /// the process-first ones over the process table.
    ///
    /// One `claimed` set spans both passes, so a session a store reader already
    /// reported keeps its source and a pid is never handed out twice. `table` is
    /// passed in rather than read here: the coordinator stays pure, and a test can
    /// drive the process pass with a fake table.
    pub fn run_all(
        readers: &[&dyn StoreReader],
        process_readers: &[&dyn ProcessReader],
        table: &[crate::agent_process::ProcessInfo],
        budget_ms: u64,
        now_ms: impl Fn() -> u64,
    ) -> ScanResult {
        let started = now_ms();
        let mut result = ScanResult::default();
        let mut claimed: HashSet<String> = HashSet::new();
        let mut exhausted = false;

        for reader in readers {
            for root in reader.roots() {
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
                for hit in reader.read(&root) {
                    if claimed.insert(hit.session_id.clone()) {
                        result.hits.push(hit);
                    }
                }
            }
        }

        // The process-first pass. Only processes of a source that has a reader are
        // asked about, and a process whose cwd could not be read is skipped: a
        // guessed directory would put a card in a folder the user does not have.
        for reader in process_readers {
            for process in table {
                if !exhausted && now_ms().saturating_sub(started) > budget_ms {
                    exhausted = true;
                }
                if exhausted {
                    result
                        .unfinished
                        .push((reader.source(), process.pid.to_string()));
                    continue;
                }
                if process.cwd.is_none()
                    || !crate::agent_process::is_session_process(reader.source(), process)
                {
                    continue;
                }
                for hit in reader.read_process(process) {
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
        /// What this reader says it looks at; defaults to the table's own roots.
        roots: Option<Vec<String>>,
    }

    impl Fake {
        fn new(source: &'static str, table: Vec<(&'static str, Vec<DiscoveryHit>)>) -> Self {
            Self {
                source,
                table,
                calls: std::cell::RefCell::new(Vec::new()),
                roots: None,
            }
        }

        fn looking_at(source: &'static str, roots: &[&str]) -> Self {
            Self {
                roots: Some(roots.iter().map(|root| (*root).to_owned()).collect()),
                ..Self::new(source, Vec::new())
            }
        }
    }

    impl StoreReader for Fake {
        fn source(&self) -> &'static str {
            self.source
        }

        fn roots(&self) -> Vec<String> {
            self.roots.clone().unwrap_or_else(|| {
                self.table
                    .iter()
                    .map(|(root, _)| (*root).to_owned())
                    .collect()
            })
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

    /// Each reader is walked over **its own** roots. Claude keeps a store per
    /// config root; a single-database agent keeps one. A caller holding one list
    /// would re-read that database once per config root.
    #[test]
    fn each_reader_is_walked_over_its_own_roots() {
        let claude = Fake::new(
            "claude",
            vec![
                (r"/r1", vec![hit("a", "claude", "/w")]),
                (r"/r2", vec![hit("b", "claude", "/w")]),
            ],
        );
        let store = Fake::new("codex", vec![(r"/data", vec![hit("c", "codex", "/w")])]);

        let readers: Vec<&dyn StoreReader> = vec![&claude, &store];
        let result = scan::run(&readers, scan::DEFAULT_BUDGET_MS, || 0);

        assert_eq!(ids(&result), vec!["a", "b", "c"]);
        assert_eq!(*claude.calls.borrow(), vec![r"/r1", r"/r2"]);
        assert_eq!(
            *store.calls.borrow(),
            vec![r"/data"],
            "a single-store reader must not be read once per config root"
        );
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

        let result = scan::run(&readers, scan::DEFAULT_BUDGET_MS, || 0);

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

        let result = scan::run(&readers, scan::DEFAULT_BUDGET_MS, || 0);
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

        let result = scan::run(&readers, 100, || {
            let call = clock.get();
            clock.set(call + 1);
            if call < 2 { 0 } else { 10_000 }
        });

        assert_eq!(ids(&result), vec!["a"]);
        // Readers are walked reader-major, so the second reader's one root is
        // what the deadline costs.
        assert_eq!(
            result.unfinished,
            vec![("codex", r"/r2".to_owned())],
            "the reader that did not reach its root is named"
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

        let result = scan::run(&readers, 50, || {
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
        let result = scan::run(&readers, scan::DEFAULT_BUDGET_MS, || 0);
        assert!(result.hits.is_empty());
        assert!(result.unfinished.is_empty());
    }

    #[test]
    fn a_reader_that_looks_nowhere_reads_nothing() {
        // A reader that looks nowhere reads nothing.
        let idle = Fake::looking_at("claude", &[]);
        let readers: Vec<&dyn StoreReader> = vec![&idle];
        let result = scan::run(&readers, scan::DEFAULT_BUDGET_MS, || 0);
        assert!(result.hits.is_empty());
        assert!(idle.calls.borrow().is_empty());
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

