//! AiWork's stream events, folded into a session snapshot
//! (`macos/Sources/MorrowCore/AiWorkWatchClient.swift` for the mapper,
//! `macos/Sources/Morrow/AppState+AiWorkWatch.swift` for the fold).
//!
//! AiWork is a daemon that streams JSON events over a socket; the socket is not ported
//! (it is a watch client, not a model), but the part that decides what the island shows
//! is pure and is what #238 AC 3 is about: the same event must produce the same status,
//! the same tool row, and so the same `AttentionMoment`.
//!
//! Three behaviours worth stating, because each exists to fix something that looked
//! wrong on a card:
//!
//! - **The status is whatever the caller says it is.** The daemon's event name does not
//!   decide `waitingApproval` — the watch client does, from the socket's own state. This
//!   fold only sets the fields beside it.
//! - **Text deltas accumulate.** Putting each fragment into `toolDescription` looks like
//!   noise and re-triggers the blur on every token, so fragments merge into
//!   `lastAssistantMessage` (last 800 characters) and the row shows a 160-character
//!   preview.
//! - **A tool result lingers.** The tool name stays, the description marks completion,
//!   and the status drops to `processing` until the next progress event arrives.
//!
//! The placeholder strings are English literals here. macOS routes them through `L10n`
//! (`reply_complete_placeholder` and friends, #322); the port has no shared string
//! table yet, so a translated build would need them wired, and that is stated rather
//! than hidden behind a call that cannot exist.

use serde_json::Value;

use crate::attention::{AttentionMoment, FollowUpReminderKind, TurnEnd, TurnHost};
use crate::display_only_wait;
use crate::follow_up::FollowUpKind;
use crate::reduce::{AgentStatus, ChatMessage, SessionSnapshot};

/// The assistant text the card keeps after a turn.
pub const ASSISTANT_TAIL_CHARS: usize = 800;
/// The preview the row shows while the turn streams.
pub const PREVIEW_CHARS: usize = 200;
/// How much of a preview fits in the process row's description.
pub const DESCRIPTION_CHARS: usize = 160;
/// How many tools the history keeps for AiWork.
pub const TOOL_HISTORY_LIMIT: usize = 20;

/// The stand-in line a finished turn leaves when it has no text of its own.
pub fn terminal_placeholder(event_name: &str) -> &'static str {
    match event_name {
        "stream.failed" => "Reply failed",
        "stream.aborted" => "Reply interrupted",
        _ => "Reply complete",
    }
}

/// `text` cut to `max` characters with an ellipsis, counted in characters so a preview
/// never splits a glyph.
pub fn truncate(text: &str, max: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max {
        return text.to_owned();
    }
    let mut out: String = chars[..max.saturating_sub(1)].iter().collect();
    out.push('…');
    out
}

fn string(data: Option<&Value>, key: &str) -> Option<String> {
    data?
        .get(key)?
        .as_str()
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn object<'a>(data: Option<&'a Value>, key: &str) -> Option<&'a Value> {
    data?.get(key).filter(|value| value.is_object())
}

fn non_empty(text: Option<String>) -> Option<String> {
    text.map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

/// The tool display block, wherever the daemon put it: top level, `_meta`, or
/// `_meta.agentix`.
fn tool_display(data: Option<&Value>) -> Option<&Value> {
    let direct = object(data, "agentix.tool_display.v1");
    if direct.is_some() {
        return direct;
    }
    let meta = object(data, "_meta")?;
    if let Some(value) = meta.get("agentix.tool_display.v1") {
        return Some(value);
    }
    let agentix = meta.get("agentix")?;
    agentix
        .get("tool_display")
        .or_else(|| agentix.get("tool_display.v1"))
}

/// The short tool name for the row.
pub fn tool_name(data: Option<&Value>) -> Option<String> {
    if let Some(display) = tool_display(data) {
        if let Some(title) = string(Some(display), "title") {
            // Prefer the short name when the title is a long "name: detail".
            if let Some(name) = string(Some(display), "tool_name").or_else(|| string(data, "name"))
            {
                return Some(name);
            }
            return Some(title);
        }
        if let Some(name) = string(Some(display), "tool_name") {
            return Some(name);
        }
    }
    string(data, "name")
        .or_else(|| string(data, "title"))
        .or_else(|| string(data, "tool"))
        .or_else(|| string(data, "tool_name"))
}

/// One line of detail for the process row.
pub fn tool_description(data: Option<&Value>) -> Option<String> {
    if let Some(display) = tool_display(data) {
        if let Some(title) = string(Some(display), "title") {
            return Some(truncate(&title, DESCRIPTION_CHARS));
        }
        if let Some(Value::Object(fields)) = display.get("primary_fields") {
            let mut parts: Vec<String> = fields
                .iter()
                .filter_map(|(key, value)| {
                    let text = value.as_str().filter(|text| !text.is_empty())?;
                    Some(format!("{key}={text}"))
                })
                .collect();
            if !parts.is_empty() {
                parts.sort();
                return Some(truncate(&parts.join(" "), DESCRIPTION_CHARS));
            }
        }
    }
    let input = object(data, "input");
    for key in ["command", "query", "path"] {
        if let Some(text) = string(input, key) {
            return Some(truncate(&text, DESCRIPTION_CHARS));
        }
    }
    tool_name(data).map(|name| truncate(&name, 80))
}

/// The progress line: the event's own text, or the readable summary's signal.
pub fn progress_text(data: Option<&Value>) -> Option<String> {
    if let Some(text) = string(data, "text") {
        return Some(truncate(&text, DESCRIPTION_CHARS));
    }
    let signal =
        object(data, "readable_summary").and_then(|summary| string(Some(summary), "signal"))?;
    Some(truncate(&signal, DESCRIPTION_CHARS))
}

/// The label beside the process row while a turn streams.
pub fn progress_label(data: Option<&Value>, event_name: &str) -> String {
    if let Some(phase) = string(data, "phase") {
        return compact_phase_label(&phase);
    }
    match event_name {
        "stream.thinking_delta" => "thinking",
        "stream.text_delta" => "reply",
        "stream.progress" => "progress",
        "stream.fallback_started" => "fallback",
        "stream.ack" => "ack",
        _ => "working",
    }
    .to_owned()
}

/// A phase name shrunk to what fits beside the row.
pub fn compact_phase_label(phase: &str) -> String {
    match phase {
        "context_build" => "context",
        "llm_request" | "llm_streaming" => "llm",
        "tool_execution" => "tool",
        "post_processing" => "post",
        "provider_waiting" => "waiting",
        "provider_retrying" => "retry",
        _ => return truncate(&phase.replace('_', " "), 24),
    }
    .to_owned()
}

/// The user prompt a `stream.started` carries, from the daemon's `display_text` or the
/// usual aliases.
pub fn user_prompt(data: Option<&Value>) -> Option<String> {
    let session = object(data, "session");
    let session_value = session;
    let candidates = [
        string(data, "display_text"),
        string(data, "prompt"),
        string(data, "text"),
        string(data, "message"),
        string(data, "user_message"),
        session_value.and_then(|session| string(Some(session), "title")),
    ];
    for text in candidates.into_iter().flatten() {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return Some(truncate(trimmed, 200));
        }
    }
    None
}

/// `AiWorkStatusMapper.status(forEventName:)`: the daemon's event name decides
/// the status when it names a wait or a turn end.
fn status_for_event_name(name: &str) -> Option<AgentStatus> {
    match name {
        "stream.approval_required" | "stream.plan_confirmation_required" => {
            Some(AgentStatus::WaitingApproval)
        }
        "stream.question_required" => Some(AgentStatus::WaitingQuestion),
        "stream.tool_call"
        | "stream.tool_result"
        | "stream.started"
        | "stream.progress"
        | "stream.thinking_delta"
        | "stream.text_delta"
        | "stream.ack"
        | "stream.fallback_started"
        | "stream.approval_resolved"
        | "stream.plan_confirmation_resolved"
        | "stream.question_resolved" => Some(AgentStatus::Processing),
        "stream.completed" | "stream.failed" | "stream.aborted" => Some(AgentStatus::Idle),
        _ => None,
    }
}

fn reminder_kind(kind: FollowUpKind) -> FollowUpReminderKind {
    match kind {
        FollowUpKind::Approval => FollowUpReminderKind::Approval,
        FollowUpKind::Question => FollowUpReminderKind::Question,
        FollowUpKind::Completion => FollowUpReminderKind::Completion,
    }
}

/// The missing producer for #238 AC 3: fold one AiWork frame and return the
/// `AttentionMoment` a display-only wait / turn transition deserves. Pure — a
/// transcript fixture drives it without a socket or a desktop (#355/#357).
pub fn reduce_aiwork_frame(
    snapshot: &mut SessionSnapshot,
    event_name: &str,
    data: Option<&Value>,
    island_holds_request: bool,
    new_session: bool,
) -> Option<AttentionMoment> {
    let before_status = snapshot.status;
    let status = status_for_event_name(event_name).unwrap_or(before_status);
    apply_stream_event(snapshot, event_name, data, status);

    let before = display_only_wait::kind(before_status, island_holds_request);
    let after = display_only_wait::kind(snapshot.status, island_holds_request);
    if let Some(after_kind) = after {
        if Some(after_kind) != before {
            return Some(AttentionMoment::DisplayOnlyWaitBegan {
                kind: reminder_kind(after_kind),
                announce: true,
            });
        }
    }
    match event_name {
        "stream.started" => Some(AttentionMoment::TurnStarted { new_session }),
        "stream.completed" => Some(AttentionMoment::TurnEnded {
            end: TurnEnd::Completed,
            host: TurnHost::Aiwork,
        }),
        "stream.failed" => Some(AttentionMoment::TurnEnded {
            end: TurnEnd::Failed,
            host: TurnHost::Aiwork,
        }),
        "stream.aborted" => Some(AttentionMoment::TurnEnded {
            end: TurnEnd::Stopped,
            host: TurnHost::Aiwork,
        }),
        _ => None,
    }
}

/// Fold one stream event into `snapshot`.
///
/// `status` is the caller's: the watch client derives it from the socket's own state, and
/// the event name never overrides it except for `stream.tool_result`, which marks
/// completion by dropping back to `processing`.
pub fn apply_stream_event(
    snapshot: &mut SessionSnapshot,
    event_name: &str,
    data: Option<&Value>,
    status: AgentStatus,
) {
    snapshot.status = status;
    match event_name {
        "stream.started" => {
            // Mirror Claude's UserPromptSubmit: keep the user turn in recentMessages so
            // an idle card still shows the chat line after the turn completes.
            if let Some(prompt) = user_prompt(data).filter(|prompt| !prompt.is_empty()) {
                snapshot.last_user_prompt = Some(prompt.clone());
                let repeated = snapshot
                    .recent_messages
                    .last()
                    .is_some_and(|last| last.is_user && last.text == prompt);
                if !repeated {
                    snapshot.add_recent_message(ChatMessage::new(true, &prompt));
                }
            }
            snapshot.last_assistant_message = None;
            snapshot.set_current_tool(None);
            snapshot.tool_description = None;
        }
        "stream.tool_call" => {
            snapshot.set_current_tool(tool_name(data));
            snapshot.tool_description = tool_description(data);
            if let Some(tool) = snapshot.current_tool().map(str::to_owned) {
                let description = snapshot.tool_description.clone();
                snapshot.record_tool(
                    &tool,
                    description.as_deref(),
                    true,
                    None,
                    TOOL_HISTORY_LIMIT,
                    snapshot.last_activity,
                );
            }
        }
        "stream.progress"
        | "stream.thinking_delta"
        | "stream.text_delta"
        | "stream.ack"
        | "stream.fallback_started" => {
            snapshot.set_current_tool(Some(progress_label(data, event_name)));
            match progress_text(data) {
                Some(text) => {
                    if event_name == "stream.text_delta" || event_name == "stream.thinking_delta" {
                        // Deltas are tiny fragments: accumulate, then show a trailing
                        // preview. Each fragment in the row would blur on every token.
                        let existing = snapshot.last_assistant_message.clone().unwrap_or_default();
                        let merged = format!("{existing}{text}");
                        snapshot.last_assistant_message =
                            Some(tail_chars(&merged, ASSISTANT_TAIL_CHARS));
                        let preview = tail_chars(&merged, PREVIEW_CHARS);
                        let preview = preview.trim();
                        snapshot.tool_description = Some(if preview.is_empty() {
                            snapshot.current_tool().unwrap_or_default().to_owned()
                        } else {
                            truncate(preview, DESCRIPTION_CHARS)
                        });
                    } else {
                        snapshot.tool_description = Some(text);
                    }
                }
                None => {
                    if snapshot.tool_description.is_none() {
                        snapshot.tool_description = snapshot.current_tool().map(str::to_owned);
                    }
                }
            }
        }
        "stream.tool_result" => {
            // Brief linger: the tool name stays, the description marks completion.
            if snapshot.current_tool().is_none() {
                snapshot.set_current_tool(tool_name(data));
            }
            snapshot.tool_description =
                tool_description(data).or_else(|| snapshot.current_tool().map(str::to_owned));
            snapshot.status = AgentStatus::Processing;
        }
        "stream.approval_required" | "stream.plan_confirmation_required" => {
            snapshot.set_current_tool(
                tool_name(data)
                    .or_else(|| string(data, "command"))
                    .or_else(|| Some("approval".to_owned())),
            );
            snapshot.tool_description = string(data, "command")
                .or_else(|| string(data, "reason"))
                .or_else(|| snapshot.tool_description.clone());
        }
        "stream.question_required" => {
            snapshot.set_current_tool(Some("question".to_owned()));
            snapshot.tool_description = string(data, "message")
                .or_else(|| progress_text(data))
                .or_else(|| Some("waiting for answer".to_owned()));
        }
        "stream.approval_resolved"
        | "stream.plan_confirmation_resolved"
        | "stream.question_resolved" => {
            snapshot.set_current_tool(Some("working".to_owned()));
            snapshot.tool_description = None;
        }
        "stream.completed" | "stream.failed" | "stream.aborted" => {
            finalize_turn_messages(snapshot, data, event_name);
            snapshot.set_current_tool(None);
            snapshot.tool_description = None;
        }
        _ => {}
    }
}

fn tail_chars(text: &str, count: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= count {
        return text.to_owned();
    }
    chars[chars.len() - count..].iter().collect()
}

/// The closing of a turn: keep what the assistant said, or leave a stand-in so an idle
/// card is not blank.
pub fn finalize_turn_messages(
    snapshot: &mut SessionSnapshot,
    data: Option<&Value>,
    event_name: &str,
) {
    let notice_summary =
        object(data, "terminal_notice").and_then(|notice| string(Some(notice), "summary"));
    let final_text = string(data, "final_text")
        .or_else(|| string(data, "text"))
        .or(notice_summary)
        .or(snapshot.last_assistant_message.clone());

    match non_empty(final_text) {
        Some(text) => {
            let clipped = truncate(&text, 200);
            snapshot.last_assistant_message = Some(tail_chars(&text, ASSISTANT_TAIL_CHARS));
            let last_is_user = snapshot
                .recent_messages
                .last()
                .is_some_and(|last| last.is_user);
            let same_text = snapshot
                .recent_messages
                .last()
                .is_some_and(|last| last.text == clipped);
            if last_is_user || !same_text {
                snapshot.add_recent_message(ChatMessage::new(false, &clipped));
            }
        }
        None => {
            let last_is_user = snapshot
                .recent_messages
                .last()
                .is_some_and(|last| last.is_user);
            if last_is_user {
                let placeholder = terminal_placeholder(event_name);
                snapshot.add_recent_message(ChatMessage::new(false, placeholder));
            }
        }
    }

    // A late attach that missed `stream.started` still leaves a visible idle preview.
    if snapshot.recent_messages.is_empty() {
        let prompt = snapshot
            .last_user_prompt
            .clone()
            .or_else(|| snapshot.session_title.clone())
            .or_else(|| user_prompt(data));
        if let Some(prompt) = prompt {
            snapshot
                .last_user_prompt
                .get_or_insert_with(|| prompt.clone());
            snapshot.add_recent_message(ChatMessage::new(true, &prompt));
            snapshot.add_recent_message(ChatMessage::new(false, terminal_placeholder(event_name)));
        }
    }
}

/// What a session frame says about itself: cwd, title, model, client type, prompt, and
/// the readable summary's signal when nothing richer described the step.
pub fn apply_session_metadata(snapshot: &mut SessionSnapshot, data: Option<&Value>) {
    let Some(data) = data else {
        return;
    };
    let session = object(Some(data), "session");

    if let Some(cwd) = session
        .and_then(|session| string(Some(session), "cwd"))
        .or_else(|| string(Some(data), "cwd"))
        .filter(|cwd| !cwd.is_empty())
    {
        snapshot.cwd = Some(cwd);
    }
    if let Some(title) = session
        .and_then(|session| string(Some(session), "title"))
        .or_else(|| string(Some(data), "title"))
        .filter(|title| !title.is_empty())
    {
        snapshot.session_title = Some(title);
    }
    if let Some(model) = session
        .and_then(|session| string(Some(session), "model"))
        .or_else(|| string(Some(data), "model"))
        .filter(|model| !model.is_empty())
    {
        snapshot.model = Some(model);
    }
    if let Some(prompt) = string(Some(data), "prompt")
        .or_else(|| string(Some(data), "user_prompt"))
        .or_else(|| session.and_then(|session| string(Some(session), "last_user_prompt")))
        .filter(|prompt| !prompt.is_empty())
    {
        snapshot.last_user_prompt = Some(prompt.chars().take(240).collect());
    }
    if snapshot.tool_description.is_none() && snapshot.status != AgentStatus::Idle {
        if let Some(signal) = session
            .and_then(|session| object(Some(session), "readable_summary"))
            .and_then(|summary| string(Some(summary), "signal"))
            .filter(|signal| !signal.is_empty())
        {
            snapshot.tool_description = Some(truncate(&signal, DESCRIPTION_CHARS));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reduce::Timestamp;
    use serde_json::json;

    fn snapshot() -> SessionSnapshot {
        SessionSnapshot::new(Timestamp(1_000.0))
    }

    #[test]
    fn a_wait_or_turn_transition_produces_an_attention_moment() {
        let mut s = snapshot();
        assert_eq!(
            reduce_aiwork_frame(&mut s, "stream.approval_required", None, false, false),
            Some(AttentionMoment::DisplayOnlyWaitBegan {
                kind: FollowUpReminderKind::Approval,
                announce: true,
            })
        );
        // The second frame is the same wait: no second attention moment.
        assert_eq!(
            reduce_aiwork_frame(&mut s, "stream.approval_required", None, false, false),
            None
        );
        // A request the island already holds is not display-only.
        let mut held = snapshot();
        assert_eq!(
            reduce_aiwork_frame(&mut held, "stream.question_required", None, true, false),
            None
        );

        assert_eq!(
            reduce_aiwork_frame(&mut snapshot(), "stream.started", None, false, true),
            Some(AttentionMoment::TurnStarted { new_session: true })
        );
        assert_eq!(
            reduce_aiwork_frame(&mut snapshot(), "stream.completed", None, false, false),
            Some(AttentionMoment::TurnEnded {
                end: TurnEnd::Completed,
                host: TurnHost::Aiwork,
            })
        );
    }

    #[test]
    fn a_started_stream_carries_the_prompt_and_clears_the_previous_turn() {
        let mut s = snapshot();
        s.last_assistant_message = Some("old answer".to_owned());
        s.set_current_tool(Some("Bash".to_owned()));

        apply_stream_event(
            &mut s,
            "stream.started",
            Some(&json!({"display_text": "  fix the flaky test  "})),
            AgentStatus::Running,
        );
        assert_eq!(s.status, AgentStatus::Running);
        assert_eq!(s.last_user_prompt.as_deref(), Some("fix the flaky test"));
        assert_eq!(s.last_assistant_message, None);
        assert_eq!(s.current_tool(), None);
        assert!(s.recent_messages.last().is_some_and(|m| m.is_user));

        // The same prompt twice does not add a second chat line.
        let before = s.recent_messages.len();
        apply_stream_event(
            &mut s,
            "stream.started",
            Some(&json!({"display_text": "fix the flaky test"})),
            AgentStatus::Running,
        );
        assert_eq!(s.recent_messages.len(), before);
    }

    #[test]
    fn a_long_prompt_is_cut_to_two_hundred_characters() {
        let mut s = snapshot();
        let prompt = "x".repeat(400);
        apply_stream_event(
            &mut s,
            "stream.started",
            Some(&json!({"prompt": prompt})),
            AgentStatus::Running,
        );
        let stored = s.last_user_prompt.expect("a prompt");
        assert_eq!(stored.chars().count(), 200);
        assert!(stored.ends_with('…'));
    }

    #[test]
    fn a_tool_call_names_the_tool_and_records_it() {
        let mut s = snapshot();
        apply_stream_event(
            &mut s,
            "stream.tool_call",
            Some(&json!({
                "agentix.tool_display.v1": {"title": "Bash: run the tests", "tool_name": "Bash"},
                "input": {"command": "cargo test"},
            })),
            AgentStatus::Running,
        );
        assert_eq!(s.current_tool(), Some("Bash"));
        assert_eq!(s.tool_description.as_deref(), Some("Bash: run the tests"));
        assert_eq!(s.total_tool_call_count, 1);
    }

    #[test]
    fn a_tool_display_hidden_under_meta_is_still_found() {
        let data = json!({"_meta": {"agentix": {"tool_display": {"title": "Grep: find it"}}}});
        assert_eq!(
            tool_description(Some(&data)).as_deref(),
            Some("Grep: find it")
        );
        assert_eq!(tool_name(Some(&data)).as_deref(), Some("Grep: find it"));
        let nested = json!({"_meta": {"agentix.tool_display.v1": {"tool_name": "Read"}}});
        assert_eq!(tool_name(Some(&nested)).as_deref(), Some("Read"));
    }

    #[test]
    fn a_tool_without_a_display_falls_back_to_its_input() {
        let data = json!({"name": "Bash", "input": {"command": "ls -la"}});
        assert_eq!(tool_description(Some(&data)).as_deref(), Some("ls -la"));
        let query = json!({"name": "Grep", "input": {"query": "fn main"}});
        assert_eq!(tool_description(Some(&query)).as_deref(), Some("fn main"));
        let path = json!({"name": "Read", "input": {"path": "/tmp/a.rs"}});
        assert_eq!(tool_description(Some(&path)).as_deref(), Some("/tmp/a.rs"));
        // Nothing to describe: the tool's own name, shorter.
        let bare = json!({"name": "Bash"});
        assert_eq!(tool_description(Some(&bare)).as_deref(), Some("Bash"));
        assert_eq!(tool_description(None), None);
    }

    #[test]
    fn primary_fields_are_sorted_into_one_line() {
        let data = json!({
            "agentix.tool_display.v1": {
                "primary_fields": {"b": "two", "a": "one", "empty": ""}
            }
        });
        assert_eq!(
            tool_description(Some(&data)).as_deref(),
            Some("a=one b=two")
        );
    }

    #[test]
    fn text_deltas_accumulate_and_show_only_a_preview() {
        let mut s = snapshot();
        for fragment in ["hello ", "world"] {
            apply_stream_event(
                &mut s,
                "stream.text_delta",
                Some(&json!({"text": fragment})),
                AgentStatus::Running,
            );
        }
        assert_eq!(s.last_assistant_message.as_deref(), Some("hello world"));
        // The row shows the label, not the fragment, until there is text to preview.
        assert_eq!(s.current_tool(), Some("reply"));
        assert_eq!(s.tool_description.as_deref(), Some("hello world"));
    }

    #[test]
    fn the_assistant_text_keeps_its_tail_and_the_preview_is_shorter() {
        let mut s = snapshot();
        let fragment = "y".repeat(1_000);
        apply_stream_event(
            &mut s,
            "stream.text_delta",
            Some(&json!({"text": fragment})),
            AgentStatus::Running,
        );
        // `progress_text` cuts each fragment to 160 before it is merged, so one huge
        // delta arrives already truncated — the tail limit is for many small ones.
        assert_eq!(
            s.last_assistant_message
                .as_ref()
                .expect("text")
                .chars()
                .count(),
            DESCRIPTION_CHARS
        );
        assert_eq!(
            s.tool_description
                .as_ref()
                .expect("preview")
                .chars()
                .count(),
            DESCRIPTION_CHARS
        );

        // Fragments under the limit do accumulate past it.
        let mut s = snapshot();
        for _ in 0..10 {
            apply_stream_event(
                &mut s,
                "stream.text_delta",
                Some(&json!({"text": "z".repeat(100)})),
                AgentStatus::Running,
            );
        }
        assert_eq!(
            s.last_assistant_message
                .as_ref()
                .expect("text")
                .chars()
                .count(),
            ASSISTANT_TAIL_CHARS
        );
    }

    #[test]
    fn a_phase_names_the_row_instead_of_the_event() {
        let data = json!({"phase": "tool_execution"});
        assert_eq!(progress_label(Some(&data), "stream.progress"), "tool");
        assert_eq!(compact_phase_label("llm_streaming"), "llm");
        assert_eq!(compact_phase_label("something_else"), "something else");
        assert_eq!(
            compact_phase_label("a_very_long_phase_name_here"),
            "a very long phase name …"
        );
        // No phase: the event decides.
        assert_eq!(progress_label(None, "stream.thinking_delta"), "thinking");
        assert_eq!(progress_label(None, "stream.ack"), "ack");
        assert_eq!(progress_label(None, "stream.progress"), "progress");
        assert_eq!(progress_label(None, "stream.unknown"), "working");
    }

    #[test]
    fn an_approval_names_what_is_being_asked() {
        let mut s = snapshot();
        apply_stream_event(
            &mut s,
            "stream.approval_required",
            Some(&json!({"command": "rm -rf build"})),
            AgentStatus::WaitingApproval,
        );
        assert_eq!(s.status, AgentStatus::WaitingApproval);
        assert_eq!(s.current_tool(), Some("rm -rf build"));
        assert_eq!(s.tool_description.as_deref(), Some("rm -rf build"));

        // Nothing named: the row still says an approval is waiting.
        let mut bare = snapshot();
        apply_stream_event(
            &mut bare,
            "stream.plan_confirmation_required",
            Some(&json!({})),
            AgentStatus::WaitingApproval,
        );
        assert_eq!(bare.current_tool(), Some("approval"));
    }

    #[test]
    fn a_question_names_the_question_and_a_resolution_clears_it() {
        let mut s = snapshot();
        apply_stream_event(
            &mut s,
            "stream.question_required",
            Some(&json!({"message": "Which database?"})),
            AgentStatus::WaitingQuestion,
        );
        assert_eq!(s.current_tool(), Some("question"));
        assert_eq!(s.tool_description.as_deref(), Some("Which database?"));

        apply_stream_event(
            &mut s,
            "stream.question_resolved",
            None,
            AgentStatus::Running,
        );
        assert_eq!(s.current_tool(), Some("working"));
        assert_eq!(s.tool_description, None);
    }

    #[test]
    fn a_tool_result_lingers_and_drops_the_status_to_processing() {
        let mut s = snapshot();
        s.set_current_tool(Some("Bash".to_owned()));
        apply_stream_event(
            &mut s,
            "stream.tool_result",
            Some(&json!({"input": {"command": "cargo test"}})),
            AgentStatus::Running,
        );
        assert_eq!(s.current_tool(), Some("Bash"));
        assert_eq!(s.tool_description.as_deref(), Some("cargo test"));
        assert_eq!(s.status, AgentStatus::Processing);
    }

    #[test]
    fn a_finished_turn_keeps_the_answer_and_clears_the_tool_row() {
        let mut s = snapshot();
        s.set_current_tool(Some("reply".to_owned()));
        s.add_recent_message(ChatMessage::new(true, "ship it"));
        apply_stream_event(
            &mut s,
            "stream.completed",
            Some(&json!({"final_text": "Shipped the fix."})),
            AgentStatus::Idle,
        );
        assert_eq!(
            s.last_assistant_message.as_deref(),
            Some("Shipped the fix.")
        );
        assert_eq!(
            s.recent_messages.last().map(|m| m.text.as_str()),
            Some("Shipped the fix.")
        );
        assert_eq!(s.current_tool(), None);
        assert_eq!(s.tool_description, None);
        assert_eq!(s.status, AgentStatus::Idle);
    }

    #[test]
    fn a_turn_that_says_nothing_leaves_a_stand_in_line() {
        for (event, expected) in [
            ("stream.failed", "Reply failed"),
            ("stream.aborted", "Reply interrupted"),
            ("stream.completed", "Reply complete"),
        ] {
            let mut s = snapshot();
            s.add_recent_message(ChatMessage::new(true, "do the thing"));
            apply_stream_event(&mut s, event, Some(&json!({})), AgentStatus::Idle);
            assert_eq!(
                s.recent_messages.last().map(|m| m.text.as_str()),
                Some(expected),
                "{event}"
            );
        }
    }

    #[test]
    fn a_late_attach_still_leaves_a_visible_preview() {
        let mut s = snapshot();
        s.session_title = Some("Fix the flaky test".to_owned());
        apply_stream_event(&mut s, "stream.completed", None, AgentStatus::Idle);
        let texts: Vec<&str> = s.recent_messages.iter().map(|m| m.text.as_str()).collect();
        assert_eq!(texts, vec!["Fix the flaky test", "Reply complete"]);
        assert!(s.recent_messages[0].is_user);
    }

    #[test]
    fn session_metadata_fills_what_the_card_shows_but_never_overwrites_the_row() {
        let mut s = snapshot();
        s.status = AgentStatus::Running;
        apply_session_metadata(
            &mut s,
            Some(&json!({
                "session": {
                    "cwd": "/work/proj",
                    "title": "A title",
                    "model": "agentix-large",
                    "last_user_prompt": "from the session",
                    "readable_summary": {"signal": "reading files"},
                },
                "client_type": "gui",
            })),
        );
        assert_eq!(s.cwd.as_deref(), Some("/work/proj"));
        assert_eq!(s.session_title.as_deref(), Some("A title"));
        assert_eq!(s.model.as_deref(), Some("agentix-large"));
        assert_eq!(s.last_user_prompt.as_deref(), Some("from the session"));
        assert_eq!(s.tool_description.as_deref(), Some("reading files"));

        // A description that already describes the step is not replaced by a summary.
        let mut s = snapshot();
        s.status = AgentStatus::Running;
        s.tool_description = Some("cargo test".to_owned());
        apply_session_metadata(
            &mut s,
            Some(&json!({"session": {"readable_summary": {"signal": "reading files"}}})),
        );
        assert_eq!(s.tool_description.as_deref(), Some("cargo test"));

        // Nothing arrives while idle: a stale signal must not decorate an idle card.
        let mut s = snapshot();
        s.status = AgentStatus::Idle;
        apply_session_metadata(
            &mut s,
            Some(&json!({"session": {"readable_summary": {"signal": "reading files"}}})),
        );
        assert_eq!(s.tool_description, None);
    }

    #[test]
    fn a_prompt_from_the_session_object_is_read_when_the_top_level_has_none() {
        let mut s = snapshot();
        apply_session_metadata(
            &mut s,
            Some(&json!({"session": {"last_user_prompt": "nested"}})),
        );
        assert_eq!(s.last_user_prompt.as_deref(), Some("nested"));
        // …and the top level wins over it.
        apply_session_metadata(
            &mut s,
            Some(&json!({"prompt": "top", "session": {"last_user_prompt": "nested"}})),
        );
        assert_eq!(s.last_user_prompt.as_deref(), Some("top"));
    }

    #[test]
    fn truncation_counts_characters_and_never_splits_a_glyph() {
        assert_eq!(truncate("abc", 5), "abc");
        assert_eq!(truncate("abcdef", 4), "abc…");
        assert_eq!(truncate("ééééé", 3), "éé…");
    }

    #[test]
    fn an_unknown_event_only_sets_the_status_the_caller_gave() {
        let mut s = snapshot();
        s.set_current_tool(Some("Bash".to_owned()));
        apply_stream_event(&mut s, "stream.whatever", None, AgentStatus::Processing);
        assert_eq!(s.status, AgentStatus::Processing);
        assert_eq!(s.current_tool(), Some("Bash"));
    }
}
