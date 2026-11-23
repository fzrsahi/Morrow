//! What a push says, and how it is worded (`macos/Sources/MorrowCore/PushNotification.swift`).
//!
//! A push is rendered once, channel-neutral, and each channel decides how to map the
//! three parts onto its own fields. Two properties matter more than the layout:
//!
//! - **Nothing leaves the Mac unredacted.** Every line goes through the same credential
//!   redaction the notch uses, and a reply is cut *before* that pass rather than after
//!   it: running the patterns over each line of a 50 KB transcript to keep 200
//!   characters costs milliseconds per push.
//! - **A secret question never leaves at all.** `isSecret` renders "answer it on your
//!   Mac" — a third-party push server is further off-device than the Buddy screen.
//!
//! What is here is the rendering half of #238 AC 1. Channels, the delivery policy and
//! the transport are their own slices; `PushContent` is the piece the follow-up
//! reminders and `DisplayOnlyWait` were waiting for.

use std::fmt::Write as _;

use crate::reduce::hook_event::sanitized_summary;

/// The moments worth reaching for a phone over. One push per moment, not per hook.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PushEventKind {
    Permission,
    Question,
    Completion,
    Error,
    /// An approval or question still unanswered, or a finished turn nobody looked at.
    Reminder,
}

impl PushEventKind {
    /// Kinds that keep an agent blocked until someone answers: a channel maps this onto
    /// its own "break through" knob. A reminder's own urgency follows what it reminds
    /// about.
    pub fn blocks_agent(self) -> bool {
        matches!(self, Self::Permission | Self::Question | Self::Reminder)
    }

    /// The kinds offered as per-channel checkboxes, in display order.
    pub fn configurable() -> [Self; 5] {
        [
            Self::Permission,
            Self::Question,
            Self::Completion,
            Self::Error,
            Self::Reminder,
        ]
    }

    /// What a freshly configured channel pushes: everything.
    pub fn default_selection() -> [Self; 5] {
        Self::configurable()
    }

    /// Scannable marker at the front of the title — a lock screen truncates the rest, so
    /// the kind has to be readable from the first glyph.
    pub fn emoji(self) -> &'static str {
        match self {
            Self::Permission => "🔐",
            Self::Question => "❓",
            Self::Completion => "✅",
            Self::Error => "❌",
            Self::Reminder => "⏰",
        }
    }
}

/// Who a push is about, read off the session card rather than guessed from the hook.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PushSubject {
    pub session_id: String,
    /// Display name of the agent ("Claude", "Codex").
    pub agent: String,
    /// Project folder name.
    pub project: Option<String>,
    /// Remote host label for SSH sessions.
    pub host: Option<String>,
}

impl PushSubject {
    /// "Claude · vibe-notch", or "Claude · vibe-notch @ devbox" for a remote session.
    pub fn label(&self) -> String {
        let mut parts: Vec<&str> = vec![self.agent.trim()];
        if let Some(project) = self
            .project
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
        {
            parts.push(project);
        }
        let mut label = parts
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
        if let Some(host) = self
            .host
            .as_deref()
            .map(str::trim)
            .filter(|h| !h.is_empty())
        {
            label.push_str(" @ ");
            label.push_str(host);
        }
        label
    }
}

/// One question a push can carry, with its options.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PushQuestionItem {
    pub question: String,
    pub options: Vec<String>,
    /// Short label of the question, all a headline-only push shows.
    pub header: Option<String>,
}

/// Structured content, taken from the same queue entries the island renders, so the
/// phone and the notch never disagree about what is being asked.
#[derive(Clone, Debug, PartialEq)]
pub enum PushContent {
    /// `detail`: command, file path or other one-line summary of the call.
    Permission {
        tool: Option<String>,
        detail: Option<String>,
    },
    Question {
        items: Vec<PushQuestionItem>,
        /// The answer is sensitive: neither the question nor its options leave the Mac.
        is_secret: bool,
    },
    Completion {
        /// The last assistant reply of the turn.
        summary: Option<String>,
    },
    Error {
        /// Machine error class ("rate_limit").
        kind: Option<String>,
        /// Readable text.
        detail: Option<String>,
    },
    Reminder {
        /// What is still waiting, usually the original permission or question.
        pending: Option<Box<PushContent>>,
        /// Seconds since it started waiting.
        waiting_since: Option<f64>,
    },
    /// A display-only wait: the island holds nothing to answer, so the push says where
    /// to go. It is an approval or a question in every other respect — same kind, so the
    /// same checkboxes and the same dedupe.
    AnswerElsewhere {
        pending: Box<PushContent>,
        app: Option<String>,
    },
}

impl PushContent {
    pub fn kind(&self) -> PushEventKind {
        match self {
            Self::Permission { .. } => PushEventKind::Permission,
            Self::Question { .. } => PushEventKind::Question,
            Self::Completion { .. } => PushEventKind::Completion,
            Self::Error { .. } => PushEventKind::Error,
            Self::Reminder { .. } => PushEventKind::Reminder,
            Self::AnswerElsewhere { pending, .. } => pending.kind(),
        }
    }

    /// A reminder about a finished turn must not break through just because it is a
    /// reminder; one about an approval must.
    pub fn blocks_agent(&self) -> bool {
        match self {
            Self::Reminder { pending, .. } => {
                pending.as_ref().is_none_or(|pending| pending.blocks_agent())
            }
            Self::AnswerElsewhere { pending, .. } => pending.blocks_agent(),
            _ => self.kind().blocks_agent(),
        }
    }
}

/// Localized phrases. The core has no access to the app's string tables, so the caller
/// passes them in; [`PushStrings::english`] is what the tests use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PushStrings {
    pub permission: String,
    pub question: String,
    pub completion: String,
    pub error: String,
    pub reminder: String,
    /// `%d` = minutes waited.
    pub waiting_minutes: String,
    pub secret_question: String,
    /// `%d` = options left out.
    pub more_options: String,
    pub test_headline: String,
    pub test_body: String,
    /// `%@` = the app a display-only wait has to be answered in.
    pub answer_in: String,
    /// A display-only wait whose app is unknown.
    pub answer_on_mac: String,
}

impl PushStrings {
    pub fn english() -> Self {
        Self {
            permission: "Needs approval".to_owned(),
            question: "Has a question".to_owned(),
            completion: "Finished".to_owned(),
            error: "Stopped on an error".to_owned(),
            reminder: "Still waiting".to_owned(),
            waiting_minutes: "%d min".to_owned(),
            secret_question: "Sensitive prompt — answer it on your Mac.".to_owned(),
            more_options: "+%d more".to_owned(),
            test_headline: "Test notification".to_owned(),
            test_body: "If you can read this, CodeIsland can reach you here.".to_owned(),
            answer_in: "Respond in %@.".to_owned(),
            answer_on_mac: "Respond on your Mac.".to_owned(),
        }
    }
}

/// A rendered push, channel-neutral.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PushMessage {
    pub kind: PushEventKind,
    pub session_id: String,
    /// Emoji + who: "🔐 Claude · vibe-notch".
    pub title: String,
    /// What happened: "Needs approval: Bash".
    pub headline: String,
    /// Details, possibly several lines; may be empty.
    pub body: String,
    pub blocks_agent: bool,
}

impl PushMessage {
    /// Headline and body, for channels without a subtitle slot.
    pub fn text(&self) -> String {
        [self.headline.as_str(), self.body.as_str()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// What "Send test" delivers, styled as an approval so the urgent path is what gets
    /// exercised.
    pub fn test(strings: &PushStrings) -> Self {
        Self {
            kind: PushEventKind::Permission,
            session_id: "codeisland-test".to_owned(),
            title: "🔔 Morrow".to_owned(),
            headline: strings.test_headline.clone(),
            body: strings.test_body.clone(),
            blocks_agent: true,
        }
    }
}

/// Commands and paths are recognisable from their first few hundred characters.
pub const DETAIL_LIMIT: usize = 300;
pub const QUESTION_LIMIT: usize = 300;
pub const OPTION_LIMIT: usize = 80;
pub const MAX_OPTIONS: usize = 9;
pub const MAX_QUESTIONS: usize = 4;
/// Default cap on the completion summary; the reader can change it.
pub const DEFAULT_SUMMARY_LIMIT: usize = 200;
/// Longest question label a headline-only push shows.
pub const HEADER_LIMIT: usize = 40;

/// Render `content` once, channel-neutral.
pub fn render(
    content: &PushContent,
    subject: &PushSubject,
    strings: &PushStrings,
    summary_limit: usize,
    include_details: bool,
    now: f64,
) -> PushMessage {
    let (headline, body) = lines(content, strings, summary_limit, include_details, now);
    let who = subject.label();
    PushMessage {
        kind: content.kind(),
        session_id: subject.session_id.clone(),
        title: if who.is_empty() {
            content.kind().emoji().to_owned()
        } else {
            format!("{} {who}", content.kind().emoji())
        },
        headline,
        body,
        blocks_agent: content.blocks_agent(),
    }
}

fn lines(
    content: &PushContent,
    strings: &PushStrings,
    summary_limit: usize,
    include_details: bool,
    now: f64,
) -> (String, String) {
    match content {
        PushContent::Permission { tool, detail } => {
            let tool_name = tool.as_deref().map(str::trim).unwrap_or_default();
            let headline = if tool_name.is_empty() {
                strings.permission.clone()
            } else {
                format!("{}: {tool_name}", strings.permission)
            };
            if !include_details {
                return (headline, String::new());
            }
            (
                headline,
                clean(
                    first_line(detail.as_deref()).as_deref(),
                    DETAIL_LIMIT,
                    false,
                )
                .unwrap_or_default(),
            )
        }
        PushContent::Question { items, is_secret } => {
            // The card never streams a secret prompt to a peripheral either; a push
            // server is further off-device than the Buddy screen.
            if *is_secret {
                return (strings.question.clone(), strings.secret_question.clone());
            }
            if !include_details {
                let labels: Vec<String> = items
                    .iter()
                    .filter_map(|item| clean(item.header.as_deref(), HEADER_LIMIT, false))
                    .collect();
                let headline = if labels.is_empty() {
                    strings.question.clone()
                } else {
                    format!("{}: {}", strings.question, labels.join(" · "))
                };
                return (headline, String::new());
            }
            (strings.question.clone(), question_body(items, strings))
        }
        PushContent::Completion { summary } => {
            if !include_details {
                return (strings.completion.clone(), String::new());
            }
            let limit = summary_limit.max(20);
            (
                strings.completion.clone(),
                clean(summary.as_deref(), limit, true).unwrap_or_default(),
            )
        }
        PushContent::Error { kind, detail } => {
            let kind = kind.as_deref().map(str::trim).unwrap_or_default();
            let headline = if kind.is_empty() {
                strings.error.clone()
            } else {
                format!("{} ({kind})", strings.error)
            };
            if !include_details {
                return (headline, String::new());
            }
            let limit = summary_limit.max(20);
            (
                headline,
                clean(detail.as_deref(), limit, true).unwrap_or_default(),
            )
        }
        PushContent::Reminder {
            pending,
            waiting_since,
        } => {
            let mut headline = strings.reminder.clone();
            if let Some(waiting_since) = waiting_since {
                let minutes = ((now - waiting_since) / 60.0) as i64;
                if minutes >= 1 {
                    headline.push_str(" · ");
                    headline.push_str(&strings.waiting_minutes.replace("%d", &minutes.to_string()));
                }
            }
            // A reminder about a reminder would only repeat the headline.
            let Some(pending) = pending.as_deref() else {
                return (headline, String::new());
            };
            if pending.kind() == PushEventKind::Reminder {
                return (headline, String::new());
            }
            let (inner_headline, inner_body) =
                lines(pending, strings, summary_limit, include_details, now);
            let body = [inner_headline, inner_body]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            (headline, body)
        }
        PushContent::AnswerElsewhere { pending, app } => {
            let (headline, inner_body) =
                lines(pending, strings, summary_limit, include_details, now);
            let place = app.as_deref().map(str::trim).unwrap_or_default();
            let where_to = if place.is_empty() {
                strings.answer_on_mac.clone()
            } else {
                strings.answer_in.replace("%@", place)
            };
            let body = [inner_body, where_to]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            (headline, body)
        }
    }
}

/// "Which database?\n1. Postgres\n2. SQLite" — numbered so the reader can answer "2" at
/// the Mac without re-reading the list. Several questions are prefixed "(1/2)".
fn question_body(items: &[PushQuestionItem], strings: &PushStrings) -> String {
    let mut lines: Vec<String> = Vec::new();
    for (index, item) in items.iter().take(MAX_QUESTIONS).enumerate() {
        let question = clean(Some(&item.question), QUESTION_LIMIT, true).unwrap_or_default();
        let prefix = if items.len() > 1 {
            format!("({}/{}) ", index + 1, items.len())
        } else {
            String::new()
        };
        if !question.is_empty() || !prefix.is_empty() {
            lines.push(format!("{prefix}{question}"));
        }
        let options: Vec<String> = item
            .options
            .iter()
            .filter_map(|option| clean(Some(option), OPTION_LIMIT, true))
            .collect();
        for (number, option) in options.iter().take(MAX_OPTIONS).enumerate() {
            lines.push(format!("{}. {option}", number + 1));
        }
        if options.len() > MAX_OPTIONS {
            lines.push(
                strings
                    .more_options
                    .replace("%d", &(options.len() - MAX_OPTIONS).to_string()),
            );
        }
    }
    lines.join("\n")
}

/// The first non-empty line of a permission detail, with "…" when more followed.
///
/// A command's first line says what it does; the rest is typically a heredoc body — a
/// file's contents, a script, a key — that has no business on a lock screen or in a
/// group chat.
pub fn first_line(detail: Option<&str>) -> Option<String> {
    let detail = detail?;
    let lines: Vec<&str> = detail.lines().collect();
    let index = lines.iter().position(|line| !line.trim().is_empty())?;
    let first = lines[index].trim();
    let more = lines[index + 1..]
        .iter()
        .any(|line| !line.trim().is_empty());
    Some(if more {
        format!("{first} …")
    } else {
        first.to_owned()
    })
}

/// Plain, redacted, bounded text. Line structure is kept — a reply's bullet list stays
/// readable — and every line runs through the notch's credential redaction before
/// anything leaves the machine.
///
/// `markdown` drops the syntax a lock screen would show literally, and only for prose:
/// in a command `**` is a glob and a backtick is a substitution.
pub fn clean(value: Option<&str>, limit: usize, markdown: bool) -> Option<String> {
    let value = value?;
    let mut lines: Vec<String> = Vec::new();
    for raw_line in precut(value, limit).lines() {
        let mut line = raw_line.trim().to_owned();
        if markdown {
            if line.starts_with("```") {
                continue;
            }
            line = line.trim_start_matches('#').to_owned();
            if let Some(rest) = line.strip_prefix("> ") {
                line = rest.to_owned();
            }
            line = line.replace("**", "").replace('`', "");
        }
        if let Some(redacted) = sanitized_summary(&line, usize::MAX) {
            lines.push(redacted);
        }
    }
    let joined = lines.join("\n");
    if joined.is_empty() {
        return None;
    }
    Some(truncated(&joined, limit))
}

/// A long reply is cut **before** the redaction pass, not after: running every pattern
/// over each line of a 50 KB reply to keep 200 characters costs milliseconds per push.
///
/// The cut is loose — redaction and Markdown stripping change lengths — and lands on a
/// line boundary, or failing that a word boundary, so a credential is never split into a
/// piece the patterns no longer recognise.
pub fn precut(text: &str, limit: usize) -> String {
    let budget = limit.max(1) * 2 + 256;
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= budget {
        return text.to_owned();
    }
    let head: String = chars[..budget].iter().collect();
    if let Some(newline) = head.rfind('\n') {
        if newline > 0 {
            return head[..newline].to_owned();
        }
    }
    if let Some(space) = head.rfind(char::is_whitespace) {
        if space > 0 {
            return head[..space].to_owned();
        }
    }
    head
}

/// Character-bounded, with an ellipsis so a cut is visible as a cut.
pub fn truncated(text: &str, limit: usize) -> String {
    if limit == 0 {
        return String::new();
    }
    let char_count = text.chars().count();
    if char_count <= limit {
        return text.to_owned();
    }
    if limit == 1 {
        return "…".to_owned();
    }
    let head: String = text.chars().take(limit - 1).collect();
    format!("{}…", head.trim_end())
}

/// UTF-8 byte bounded: WeCom and ntfy count bytes, and a CJK character is three.
pub fn truncated_utf8(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_owned();
    }
    let budget = max_bytes.saturating_sub('…'.len_utf8());
    let mut used = 0;
    let mut out = String::new();
    for ch in text.chars() {
        let size = ch.len_utf8();
        if used + size > budget {
            break;
        }
        used += size;
        out.push(ch);
    }
    out.push('…');
    out
}

/// UTF-16 code-unit bounded: Telegram counts these, and a family emoji is eight units —
/// it goes whole or not at all.
pub fn truncated_utf16(text: &str, limit: usize) -> String {
    if text.encode_utf16().count() <= limit {
        return text.to_owned();
    }
    let budget = limit.saturating_sub(1); // "…" is one unit
    let mut used = 0;
    let mut out = String::new();
    for ch in text.chars() {
        let size = ch.len_utf16();
        if used + size > budget {
            break;
        }
        used += size;
        out.push(ch);
    }
    format!("{}…", out.trim_end())
}

// MARK: - The one-line detail of a tool call

/// The part of a tool call a person needs to judge it remotely.
pub fn permission_detail(
    tool_input: Option<&serde_json::Value>,
    fallback: Option<&str>,
    cwd: Option<&str>,
) -> Option<String> {
    const COMMAND_KEYS: [&str; 5] = ["command", "cmd", "CommandLine", "commandLine", "script"];
    const PATH_KEYS: [&str; 7] = [
        "file_path",
        "filePath",
        "notebook_path",
        "AbsolutePath",
        "TargetFile",
        "target_file",
        "path",
    ];
    const OTHER_KEYS: [&str; 5] = ["url", "query", "pattern", "description", "prompt"];

    if let Some(input) = tool_input {
        if let Some(command) = first_text(input, &COMMAND_KEYS) {
            return Some(command);
        }
        if let Some(path) = first_text(input, &PATH_KEYS) {
            return Some(display_path(&path, cwd));
        }
        if let Some(other) = first_text(input, &OTHER_KEYS) {
            return Some(other);
        }
    }
    fallback
        .map(str::trim)
        .filter(|fallback| !fallback.is_empty())
        .map(str::to_owned)
}

/// Project-relative when the path is inside the session's cwd; home-relative paths are
/// the redaction pass's business (`/Users/name` → `~`).
pub fn display_path(path: &str, cwd: Option<&str>) -> String {
    let trimmed = path.trim();
    let Some(cwd) = cwd.map(str::trim).filter(|cwd| !cwd.is_empty()) else {
        return trimmed.to_owned();
    };
    let root = cwd.strip_suffix('/').unwrap_or(cwd);
    let mut prefix = String::new();
    let _ = write!(prefix, "{root}/");
    match trimmed.strip_prefix(&prefix) {
        Some(rest) => rest.to_owned(),
        None => trimmed.to_owned(),
    }
}

/// Strings, or string arrays (Codex passes argv as `["bash", "-lc", "…"]`).
fn first_text(input: &serde_json::Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        let Some(value) = input.get(*key) else {
            continue;
        };
        if let Some(text) = value.as_str() {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_owned());
            }
        } else if let Some(parts) = value.as_array() {
            let joined = parts
                .iter()
                .filter_map(|part| part.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let joined = joined.trim();
            if !joined.is_empty() {
                return Some(joined.to_owned());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn subject() -> PushSubject {
        PushSubject {
            session_id: "s1".to_owned(),
            agent: "Claude".to_owned(),
            project: Some("vibe-notch".to_owned()),
            host: None,
        }
    }

    fn render_english(content: &PushContent) -> PushMessage {
        render(content, &subject(), &PushStrings::english(), 200, true, 0.0)
    }

    fn permission(tool: Option<&str>, detail: Option<&str>) -> PushContent {
        PushContent::Permission {
            tool: tool.map(str::to_owned),
            detail: detail.map(str::to_owned),
        }
    }

    #[test]
    fn only_the_blocking_kinds_break_through() {
        assert!(PushEventKind::Permission.blocks_agent());
        assert!(PushEventKind::Question.blocks_agent());
        assert!(!PushEventKind::Completion.blocks_agent());
        assert!(!PushEventKind::Error.blocks_agent());
        assert_eq!(PushEventKind::configurable().len(), 5);
        assert_eq!(PushEventKind::default_selection().len(), 5);
        assert_eq!(PushEventKind::Permission.emoji(), "🔐");
        assert_eq!(PushEventKind::Reminder.emoji(), "⏰");
    }

    #[test]
    fn a_session_label_names_the_agent_the_project_and_the_host() {
        assert_eq!(subject().label(), "Claude · vibe-notch");
        let remote = PushSubject {
            host: Some("devbox".to_owned()),
            project: None,
            ..subject()
        };
        assert_eq!(remote.label(), "Claude @ devbox");
        let bare = PushSubject {
            agent: "  Codex ".to_owned(),
            project: Some("   ".to_owned()),
            host: Some("  ".to_owned()),
            ..subject()
        };
        assert_eq!(bare.label(), "Codex");
    }

    #[test]
    fn an_approval_names_the_tool_and_keeps_only_the_first_line_of_the_detail() {
        let message = render_english(&permission(
            Some("Bash"),
            Some("rm -rf build\nand more\nlines"),
        ));
        assert_eq!(message.title, "🔐 Claude · vibe-notch");
        assert_eq!(message.headline, "Needs approval: Bash");
        assert_eq!(message.body, "rm -rf build …");
        assert!(message.blocks_agent);

        let unnamed = render_english(&permission(None, None));
        assert_eq!(unnamed.headline, "Needs approval");
        assert_eq!(unnamed.body, "");
    }

    #[test]
    fn a_headline_only_push_drops_the_detail() {
        let message = render(
            &permission(Some("Bash"), Some("rm -rf build")),
            &subject(),
            &PushStrings::english(),
            200,
            false,
            0.0,
        );
        assert_eq!(message.headline, "Needs approval: Bash");
        assert_eq!(message.body, "");
    }

    #[test]
    fn a_question_numbers_its_options_and_counts_them() {
        let content = PushContent::Question {
            items: vec![PushQuestionItem {
                question: "Which database?".to_owned(),
                options: vec!["Postgres".to_owned(), "SQLite".to_owned()],
                header: None,
            }],
            is_secret: false,
        };
        let message = render_english(&content);
        assert_eq!(message.headline, "Has a question");
        assert_eq!(message.body, "Which database?\n1. Postgres\n2. SQLite");
    }

    #[test]
    fn several_questions_are_numbered_and_long_option_lists_are_summarised() {
        let many: Vec<String> = (1..=11).map(|index| format!("option {index}")).collect();
        let content = PushContent::Question {
            items: vec![
                PushQuestionItem {
                    question: "First?".to_owned(),
                    options: many,
                    header: None,
                },
                PushQuestionItem {
                    question: "Second?".to_owned(),
                    options: Vec::new(),
                    header: None,
                },
            ],
            is_secret: false,
        };
        let body = render_english(&content).body;
        assert!(body.starts_with("(1/2) First?"), "{body}");
        assert!(body.contains("9. option 9"), "{body}");
        assert!(!body.contains("option 10"), "only nine options shown");
        assert!(body.contains("+2 more"), "{body}");
        assert!(body.contains("(2/2) Second?"), "{body}");
    }

    #[test]
    fn a_secret_question_never_leaves_the_machine() {
        let content = PushContent::Question {
            items: vec![PushQuestionItem {
                question: "API key?".to_owned(),
                options: vec!["sk-123".to_owned()],
                header: None,
            }],
            is_secret: true,
        };
        let message = render_english(&content);
        assert_eq!(message.headline, "Has a question");
        assert_eq!(message.body, "Sensitive prompt — answer it on your Mac.");
        assert!(!message.body.contains("sk-123"));
        assert!(!message.body.contains("API key"));
    }

    #[test]
    fn a_headline_only_question_shows_the_headers_not_the_options() {
        let content = PushContent::Question {
            items: vec![PushQuestionItem {
                question: "Which database?".to_owned(),
                options: vec!["Postgres".to_owned()],
                header: Some("Auth method".to_owned()),
            }],
            is_secret: false,
        };
        let message = render(
            &content,
            &subject(),
            &PushStrings::english(),
            200,
            false,
            0.0,
        );
        assert_eq!(message.headline, "Has a question: Auth method");
        assert_eq!(message.body, "");
    }

    #[test]
    fn a_reminder_says_how_long_and_repeats_what_is_waiting() {
        let content = PushContent::Reminder {
            pending: Some(Box::new(permission(Some("Bash"), Some("rm -rf build")))),
            waiting_since: Some(0.0),
        };
        // Five minutes later.
        let message = render(
            &content,
            &subject(),
            &PushStrings::english(),
            200,
            true,
            300.0,
        );
        assert_eq!(message.headline, "Still waiting · 5 min");
        assert_eq!(message.body, "Needs approval: Bash\nrm -rf build");
        // An approval still blocks; a completion's reminder does not.
        assert!(message.blocks_agent);

        let finished = PushContent::Reminder {
            pending: Some(Box::new(PushContent::Completion {
                summary: Some("done".to_owned()),
            })),
            waiting_since: None,
        };
        assert!(!finished.blocks_agent());
        let quiet = render(
            &finished,
            &subject(),
            &PushStrings::english(),
            200,
            true,
            0.0,
        );
        assert_eq!(
            quiet.headline, "Still waiting",
            "under a minute says nothing"
        );
    }

    #[test]
    fn a_reminder_about_a_reminder_does_not_repeat_itself() {
        let nested = PushContent::Reminder {
            pending: Some(Box::new(PushContent::Reminder {
                pending: None,
                waiting_since: None,
            })),
            waiting_since: None,
        };
        assert_eq!(render_english(&nested).body, "");
    }

    #[test]
    fn a_display_only_wait_says_where_to_answer() {
        let content = PushContent::AnswerElsewhere {
            pending: Box::new(permission(Some("Bash"), Some("npm publish"))),
            app: Some("Cursor".to_owned()),
        };
        let message = render_english(&content);
        assert_eq!(message.headline, "Needs approval: Bash");
        assert_eq!(message.body, "npm publish\nRespond in Cursor.");
        assert_eq!(message.kind, PushEventKind::Permission);

        let unknown_app = PushContent::AnswerElsewhere {
            pending: Box::new(permission(None, None)),
            app: None,
        };
        assert!(
            render_english(&unknown_app)
                .body
                .ends_with("Respond on your Mac.")
        );
    }

    #[test]
    fn an_error_names_its_class_and_a_completion_keeps_the_summary_limit() {
        let error = render_english(&PushContent::Error {
            kind: Some("rate_limit".to_owned()),
            detail: Some("Too many requests".to_owned()),
        });
        assert_eq!(error.headline, "Stopped on an error (rate_limit)");
        assert_eq!(error.body, "Too many requests");

        let long = "word ".repeat(200);
        let completion = render(
            &PushContent::Completion {
                summary: Some(long),
            },
            &subject(),
            &PushStrings::english(),
            50,
            true,
            0.0,
        );
        assert_eq!(completion.headline, "Finished");
        assert!(completion.body.chars().count() <= 50, "{}", completion.body);
        assert!(completion.body.ends_with('…'));
    }

    #[test]
    fn markdown_syntax_is_dropped_for_prose_and_kept_for_commands() {
        let prose =
            clean(Some("## Heading\n**bold** and `code`\n> quoted"), 200, true).expect("cleaned");
        assert_eq!(prose, "Heading\nbold and code\nquoted");

        let command = clean(Some("grep '**/*.rs' && echo `date`"), 200, false).expect("cleaned");
        assert_eq!(command, "grep '**/*.rs' && echo `date`");
    }

    #[test]
    fn a_reply_is_cut_before_it_is_redacted_and_lands_on_a_line_boundary() {
        let text = "a che".to_owned() + &"x\n".repeat(5_000);
        let cut = precut(&text, 100);
        assert!(cut.len() < text.len());
        assert!(!cut.ends_with('\n'));
        assert!(text.starts_with(&cut));
        // Short text is its own pre-cut.
        assert_eq!(precut("short", 100), "short");
    }

    #[test]
    fn the_three_bounds_count_characters_bytes_and_units() {
        assert_eq!(truncated("abcdef", 4), "abc…");
        assert_eq!(truncated("abc", 10), "abc");
        assert_eq!(truncated("abc", 0), "");
        assert_eq!(truncated("abc", 1), "…");

        // Three-byte characters: a byte bound keeps as many as fit.
        let cjk = "日本語テキスト";
        let bounded = truncated_utf8(cjk, 10);
        assert!(bounded.len() <= 10, "{} bytes", bounded.len());
        assert!(bounded.ends_with('…'));

        // An emoji outside the BMP is two UTF-16 units and goes whole or not at all.
        let emoji = "ab😀cd";
        let bounded = truncated_utf16(emoji, 4);
        assert!(bounded.encode_utf16().count() <= 4, "{bounded}");
    }

    #[test]
    fn the_detail_of_a_tool_call_is_the_command_the_path_or_the_fallback() {
        let bash = json!({"command": "cargo test"});
        assert_eq!(
            permission_detail(Some(&bash), None, Some("/work/proj")),
            Some("cargo test".to_owned())
        );
        // Paths inside the project lose the prefix; others are left alone.
        let edit = json!({"file_path": "/work/proj/src/main.rs"});
        assert_eq!(
            permission_detail(Some(&edit), None, Some("/work/proj")),
            Some("src/main.rs".to_owned())
        );
        let outside = json!({"file_path": "/etc/hosts"});
        assert_eq!(
            permission_detail(Some(&outside), None, Some("/work/proj")),
            Some("/etc/hosts".to_owned())
        );
        // Nothing recognisable: the island's own description.
        let unknown = json!({"nothing": 1});
        assert_eq!(
            permission_detail(Some(&unknown), Some("Read a file"), None),
            Some("Read a file".to_owned())
        );
        assert_eq!(permission_detail(Some(&unknown), Some("  "), None), None);
    }

    #[test]
    fn argv_passed_as_an_array_joins_into_one_line() {
        let codex = json!({"command": ["bash", "-lc", "npm test"]});
        assert_eq!(
            permission_detail(Some(&codex), None, None),
            Some("bash -lc npm test".to_owned())
        );
    }

    #[test]
    fn a_test_message_takes_the_urgent_path() {
        let message = PushMessage::test(&PushStrings::english());
        assert_eq!(message.kind, PushEventKind::Permission);
        assert_eq!(message.session_id, "codeisland-test");
        assert_eq!(message.title, "🔔 Morrow");
        assert!(message.blocks_agent);
        assert_eq!(
            message.text(),
            "Test notification\nIf you can read this, CodeIsland can reach you here."
        );
    }

    #[test]
    fn an_empty_subject_leaves_only_the_kind_marker() {
        let anonymous = PushSubject {
            agent: String::new(),
            project: None,
            ..subject()
        };
        let message = render(
            &permission(None, None),
            &anonymous,
            &PushStrings::english(),
            200,
            true,
            0.0,
        );
        assert_eq!(message.title, "🔐");
    }
}
