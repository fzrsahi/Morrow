//! Permission requests the panel holds until the user answers (plan §3):
//! one card per held hook, answered with the bytes the macOS island sends
//! Claude Code (`AppState.approvePermission` / `denyPermission`), which the
//! bridge prints verbatim.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc::Sender;

use morrow_protocol::HookPayload;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The longest tool summary a card shows, in characters.
const SUMMARY_CHARS: usize = 120;

/// Whether the island holds `payload` for an answer. `TODO(#93)`: HookIntake
/// in morrow-core decides (aliases, questions); E2E-1 serves Claude Code's
/// `PermissionRequest`. A hook Cursor ran is never held: Cursor reads its
/// stdout against its own schema (shared/contract/hook-edge-protocol.md).
pub fn waits(payload: &HookPayload) -> bool {
    let cursor = payload
        .get("cursor_version")
        .and_then(Value::as_str)
        .is_some_and(|v| !v.trim().is_empty());
    if cursor {
        return false;
    }
    // AskUserQuestion arrives as a PermissionRequest, so it was already held.
    // #188 adds Notification questions (a top-level string `question`), the
    // other shape `QuestionPayload.from(event:)` recognises on macOS.
    payload.hook_event_name() == "PermissionRequest" || notification_question(payload).is_some()
}

fn notification_question(payload: &HookPayload) -> Option<&str> {
    if payload.hook_event_name() != "Notification" {
        return None;
    }
    payload.get("question").and_then(Value::as_str)
}

/// One option of a question card (`QuestionBar`'s OptionRow).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct QuestionOption {
    pub label: String,
    pub description: String,
}

/// The question a card shows. Single choice only in #188 — #189/#190 add
/// typing and the multi-question wizard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct QuestionCard {
    pub question: String,
    pub header: String,
    pub options: Vec<QuestionOption>,
    /// How the hook was answered: AskUserQuestion needs `updatedInput.answers`,
    /// a Notification question needs a flat `answer`.
    pub from_permission: bool,
}

/// Parse the hook into the question the panel draws, if it is one.
fn question(payload: &HookPayload) -> Option<QuestionCard> {
    let option = |label: &str| QuestionOption {
        label: label.to_owned(),
        description: String::new(),
    };
    if let Some(text) = notification_question(payload) {
        let options = payload
            .get("options")
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_str).map(option).collect())
            .unwrap_or_default();
        return Some(QuestionCard {
            question: text.to_owned(),
            header: String::new(),
            options,
            from_permission: false,
        });
    }
    if payload.hook_event_name() != "PermissionRequest" {
        return None;
    }
    let input = payload.get("tool_input");
    if let Some(questions) = input
        .and_then(|input| input.get("questions"))
        .and_then(Value::as_array)
    {
        let first = questions.first()?;
        let text = first
            .get("question")
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())?;
        let header = first
            .get("header")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let options = first
            .get("options")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .map(|value| QuestionOption {
                        label: value
                            .get("label")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                        description: value
                            .get("description")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        return Some(QuestionCard {
            question: text.to_owned(),
            header,
            options,
            from_permission: true,
        });
    }
    // Legacy shape: `question` plus an array of string options.
    let text = input
        .and_then(|input| input.get("question"))
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())?;
    let options = input
        .and_then(|input| input.get("options"))
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_str).map(option).collect())
        .unwrap_or_default();
    Some(QuestionCard {
        question: text.to_owned(),
        header: String::new(),
        options,
        from_permission: true,
    })
}

/// Swift `handleAskUserQuestion`: answers are keyed by the question's text,
/// never its header; a repeated text gets `_2`, `_3`, … so each stays distinct.
fn answer_keys(questions: &[Value]) -> Vec<String> {
    let mut used = BTreeSet::new();
    questions
        .iter()
        .map(|question| {
            let base = question
                .get("question")
                .and_then(Value::as_str)
                .unwrap_or("Question");
            if used.insert(base.to_owned()) {
                return base.to_owned();
            }
            let mut suffix = 2;
            loop {
                let key = format!("{base}_{suffix}");
                if used.insert(key.clone()) {
                    return key;
                }
                suffix += 1;
            }
        })
        .collect()
}

/// The reply bytes for one or more question picks. The wrapper and key order
/// are Swift's `JSONSerialization` output, read from its golden tests.
pub fn question_reply(card: &Card, picks: &[(usize, String)]) -> Vec<u8> {
    let Some(card_question) = card.question.as_ref() else {
        return b"{}".to_vec();
    };
    let answer = |text: &str| serde_json::to_string(text).unwrap_or_else(|_| "\"\"".into());
    if !card_question.from_permission {
        let first = picks.first().map(|(_, answer)| answer.as_str()).unwrap_or("");
        return format!(
            r#"{{"hookSpecificOutput":{{"hookEventName":"Notification","answer":{}}}}}"#,
            answer(first)
        )
        .into_bytes();
    }
    let mut input = if card.tool_input.is_object() {
        card.tool_input.clone()
    } else {
        Value::Object(Map::new())
    };
    let questions = input
        .get("questions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let keys = answer_keys(&questions);
    let mut answers = Map::new();
    for (index, pick) in picks {
        if let Some(key) = keys.get(*index) {
            answers.insert(key.clone(), Value::String(pick.clone()));
        }
    }
    let updated = input.as_object_mut().expect("an object above");
    updated.insert("questions".into(), Value::Array(questions));
    updated.insert("answers".into(), Value::Object(answers));
    if let Some((_, first)) = picks.first() {
        updated.insert("answer".into(), Value::String(first.clone()));
    }
    let updated = serde_json::to_string(&updated).unwrap_or_else(|_| "{}".into());
    format!(
        r#"{{"hookSpecificOutput":{{"hookEventName":"PermissionRequest","decision":{{"behavior":"allow","updatedInput":{updated}}}}}}}"#
    )
    .into_bytes()
}

/// Skip answers a question but gives no pick: AskUserQuestion denies the tool
/// call, a Notification question sends the bare `hookSpecificOutput`.
pub fn question_skip(card: &Card) -> Vec<u8> {
    match card.question.as_ref() {
        Some(question) if !question.from_permission => {
            br#"{"hookSpecificOutput":{"hookEventName":"Notification"}}"#.to_vec()
        }
        _ => Decision::Deny.reply().to_vec(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    Allow,
    Deny,
}

impl Decision {
    /// The reply, byte for byte as Swift writes it.
    pub fn reply(self) -> &'static [u8] {
        match self {
            Self::Allow => {
                br#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}"#
            }
            Self::Deny => {
                br#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny"}}}"#
            }
        }
    }
}

/// What the panel shows of one held request (`Card` in `src/App.svelte`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Card {
    pub id: u64,
    pub session_id: String,
    pub source: String,
    pub tool_name: String,
    /// One line of the tool input: the command, the file, or its JSON.
    pub summary: String,
    /// The whole input, so the panel can draw the code / diff / MCP block
    /// and name the file in the title (#186).
    pub tool_input: Value,
    /// #188: present for AskUserQuestion / Notification questions; the panel
    /// draws a Question card instead of an approval one.
    pub question: Option<QuestionCard>,
}

#[derive(Debug, Default)]
pub struct Permissions {
    next_id: u64,
    /// Oldest first; each with the way to its hook's connection. What travels
    /// on the sender is the reply's bytes: a decision or `updatedInput.answers`.
    held: BTreeMap<u64, (Card, Sender<Vec<u8>>)>,
}

impl Permissions {
    /// Holds `payload` until one of the answer methods sends the reply to
    /// `answer`, or [`Self::drop_request`]. Returns the card's id.
    pub fn hold(&mut self, payload: &HookPayload, answer: Sender<Vec<u8>>) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        let text = |key| payload.get(key).and_then(Value::as_str).unwrap_or_default();
        let input = payload.get("tool_input");
        let card = Card {
            id,
            session_id: payload.session_id().to_owned(),
            source: payload.source().unwrap_or("claude").to_owned(),
            tool_name: text("tool_name").to_owned(),
            summary: summary(input),
            tool_input: input.cloned().unwrap_or(Value::Null),
            question: question(payload),
        };
        self.held.insert(id, (card, answer));
        id
    }

    /// The user's Approve / Deny: sent to the request's hook, card gone.
    /// False for a request no longer held (answered, or its hook gone).
    pub fn answer(&mut self, id: u64, decision: Decision) -> bool {
        match self.held.remove(&id) {
            Some((_, answer)) => answer.send(decision.reply().to_vec()).is_ok(),
            None => false,
        }
    }

    /// #188: a picked option sends `allow` with `updatedInput.answers` keyed
    /// by question text (or a flat `answer` for a Notification question).
    pub fn answer_question(&mut self, id: u64, pick: &str) -> bool {
        match self.held.remove(&id) {
            Some((card, answer)) => {
                answer.send(question_reply(&card, &[(0, pick.to_owned())])).is_ok()
            }
            None => false,
        }
    }

    /// #188: Skip closes a question without picking — macOS denies an
    /// AskUserQuestion and sends the bare output for a Notification one.
    pub fn skip_question(&mut self, id: u64) -> bool {
        match self.held.remove(&id) {
            Some((card, answer)) => answer.send(question_skip(&card)).is_ok(),
            None => false,
        }
    }

    /// Its hook is gone: the card goes, unanswered.
    pub fn drop_request(&mut self, id: u64) -> bool {
        self.held.remove(&id).is_some()
    }

    /// Its session ended: every pending card for `session_id` goes, unanswered.
    pub fn drop_session(&mut self, session_id: &str) {
        self.held
            .retain(|_, (card, _)| card.session_id != session_id);
    }

    /// Oldest first.
    pub fn cards(&self) -> Vec<Card> {
        self.held.values().map(|(card, _)| card.clone()).collect()
    }
}

/// The first of `command`, `file_path`, `path`, `url`, `pattern` that is a
/// string, else the input's JSON; one line, at most [`SUMMARY_CHARS`].
fn summary(input: Option<&Value>) -> String {
    let Some(input) = input else {
        return String::new();
    };
    let text = ["command", "file_path", "path", "url", "pattern"]
        .iter()
        .find_map(|key| input.get(key).and_then(Value::as_str))
        .map_or_else(|| input.to_string(), str::to_owned);
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() <= SUMMARY_CHARS {
        return line;
    }
    let mut cut: String = line.chars().take(SUMMARY_CHARS - 1).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;
    use morrow_protocol::decode_payload;
    use std::sync::mpsc::{self, TryRecvError};

    fn hook(json: &str) -> HookPayload {
        decode_payload(json.as_bytes()).unwrap()
    }

    const BASH: &str = r#"{"session_id":"s1","hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{"command":"rm -rf build\n  && make"}}"#;
    /// AskUserQuestion arrives as a PermissionRequest. The two questions share
    /// their text on purpose — Swift's dedup golden uses these very strings.
    const ASK_TWO: &str = r#"{"session_id":"s","hook_event_name":"PermissionRequest","tool_name":"AskUserQuestion","tool_input":{"questions":[{"question":"重复的问题","header":"偏好","options":[{"label":"A","description":""}]},{"question":"重复的问题","header":"其他","options":[{"label":"D","description":""}]}]}}"#;
    const ASK_NO_HEADER: &str = r#"{"session_id":"s","hook_event_name":"PermissionRequest","tool_name":"AskUserQuestion","tool_input":{"questions":[{"question":"没有 header","options":[{"label":"B","description":""}]}]}}"#;
    const NOTIFY_QUESTION: &str = r#"{"session_id":"s","hook_event_name":"Notification","message":"Claude needs your input","question":"Continue?","options":["Yes","No"]}"#;

    /// A held question card, as the panel sees it.
    fn question_card(json: &str) -> Card {
        let mut permissions = Permissions::default();
        let (tx, _rx) = mpsc::channel();
        permissions.hold(&hook(json), tx);
        permissions.cards().remove(0)
    }

    #[test]
    fn only_a_permission_request_outside_cursor_waits() {
        assert!(waits(&hook(BASH)));
        assert!(!waits(&hook(
            r#"{"session_id":"s","hook_event_name":"PreToolUse"}"#
        )));
        assert!(!waits(&hook(
            r#"{"session_id":"s","hook_event_name":"PermissionRequest","cursor_version":"1.2"}"#
        )));
        assert!(waits(&hook(
            r#"{"session_id":"s","hook_event_name":"PermissionRequest","cursor_version":" "}"#
        )));
    }

    #[test]
    fn questions_wait_but_plain_notifications_do_not() {
        assert!(waits(&hook(ASK_TWO)), "AskUserQuestion is a PermissionRequest");
        assert!(waits(&hook(NOTIFY_QUESTION)));
        assert!(!waits(&hook(
            r#"{"session_id":"s","hook_event_name":"Notification","message":"idle_prompt"}"#
        )));
        assert!(!waits(&hook(
            r#"{"session_id":"s","hook_event_name":"Notification","question":{"text":"not a string"}}"#
        )));
    }

    /// Ported from Swift `AppStateQuestionFlowTests.testDuplicateQuestionTextGetsDedupedKeys`:
    /// answer keys are the question text byte for byte (headers never take part),
    /// and a repeated text gets `_2`, `_3`, … so every pick stays addressable.
    #[test]
    fn question_answers_are_keyed_by_question_text_like_macos() {
        let reply = String::from_utf8(question_reply(
            &question_card(ASK_TWO),
            &[(0, "A".into()), (1, "D".into())],
        ))
        .unwrap();
        assert_eq!(
            reply,
            r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow","updatedInput":{"questions":[{"question":"重复的问题","header":"偏好","options":[{"label":"A","description":""}]},{"question":"重复的问题","header":"其他","options":[{"label":"D","description":""}]}],"answers":{"重复的问题":"A","重复的问题_2":"D"},"answer":"A"}}}}"#
        );
    }

    /// Ported from Swift `testAnswerKeyUsesQuestionTextRegardlessOfHeader`.
    #[test]
    fn an_answer_key_ignores_the_header() {
        let reply = String::from_utf8(question_reply(
            &question_card(ASK_NO_HEADER),
            &[(0, "B".into())],
        ))
        .unwrap();
        assert!(reply.contains(r#""answers":{"没有 header":"B"}"#), "{reply}");
        assert!(!reply.contains(r#""header":"#), "{reply}");
    }

    #[test]
    fn a_pick_reaches_the_hook_and_drops_the_question_card() {
        let mut permissions = Permissions::default();
        let (tx, rx) = mpsc::channel();
        let id = permissions.hold(&hook(ASK_NO_HEADER), tx);
        assert!(permissions.cards()[0].question.is_some());
        assert!(permissions.answer_question(id, "B"));
        assert_eq!(
            rx.try_recv(),
            Ok(question_reply(
                &question_card(ASK_NO_HEADER),
                &[(0, "B".into())]
            ))
        );
        assert!(permissions.cards().is_empty());
        assert!(!permissions.answer_question(id, "B"), "answered once");
    }

    #[test]
    fn skipping_a_question_goes_through_the_same_held_hook() {
        let mut permissions = Permissions::default();
        let (tx, rx) = mpsc::channel();
        let id = permissions.hold(&hook(NOTIFY_QUESTION), tx);
        assert!(permissions.skip_question(id));
        assert_eq!(
            rx.try_recv(),
            Ok(question_skip(&question_card(NOTIFY_QUESTION)))
        );
        assert!(permissions.cards().is_empty());
    }

    #[test]
    fn skipping_a_permission_question_denies_the_tool_call_like_macos() {
        assert_eq!(
            question_skip(&question_card(ASK_TWO)),
            Decision::Deny.reply(),
            "AppState.skipQuestion answers an AskUserQuestion with deny"
        );
    }

    #[test]
    fn a_notification_question_replies_its_answer_or_nothing() {
        let card = question_card(NOTIFY_QUESTION);
        assert_eq!(
            String::from_utf8(question_reply(&card, &[(0, "Yes".into())])).unwrap(),
            r#"{"hookSpecificOutput":{"hookEventName":"Notification","answer":"Yes"}}"#
        );
        assert_eq!(
            String::from_utf8(question_skip(&card)).unwrap(),
            r#"{"hookSpecificOutput":{"hookEventName":"Notification"}}"#
        );
    }

    #[test]
    fn a_held_request_is_a_card_until_approved() {
        let mut permissions = Permissions::default();
        let (tx, rx) = mpsc::channel();
        let id = permissions.hold(&hook(BASH), tx);
        assert_eq!(
            permissions.cards(),
            vec![Card {
                id,
                session_id: "s1".into(),
                source: "claude".into(),
                tool_name: "Bash".into(),
                summary: "rm -rf build && make".into(),
                tool_input: serde_json::json!({"command": "rm -rf build\n  && make"}),
                question: None,
            }]
        );
        assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));

        assert!(permissions.answer(id, Decision::Allow));
        assert_eq!(rx.try_recv(), Ok(Decision::Allow.reply().to_vec()));
        assert!(permissions.cards().is_empty());
        assert!(!permissions.answer(id, Decision::Deny), "answered once");
    }

    #[test]
    fn deny_reaches_only_its_own_request() {
        let mut permissions = Permissions::default();
        let (tx1, rx1) = mpsc::channel();
        let (tx2, rx2) = mpsc::channel();
        let first = permissions.hold(&hook(BASH), tx1);
        let second = permissions.hold(&hook(BASH), tx2);
        assert_ne!(first, second);

        assert!(permissions.answer(second, Decision::Deny));
        assert_eq!(rx2.try_recv(), Ok(Decision::Deny.reply().to_vec()));
        assert_eq!(rx1.try_recv(), Err(TryRecvError::Empty));
        let ids: Vec<u64> = permissions.cards().iter().map(|c| c.id).collect();
        assert_eq!(ids, [first]);
    }

    #[test]
    fn a_gone_hook_drops_its_card_unanswered() {
        let mut permissions = Permissions::default();
        let (tx, rx) = mpsc::channel();
        let id = permissions.hold(&hook(BASH), tx);
        assert!(permissions.drop_request(id));
        assert!(permissions.cards().is_empty());
        assert_eq!(rx.try_recv(), Err(TryRecvError::Disconnected));
        assert!(!permissions.answer(id, Decision::Allow));
        assert!(!permissions.drop_request(id));
    }

    #[test]
    fn dropping_a_session_drops_only_its_cards() {
        let mut permissions = Permissions::default();
        let (tx1, rx1) = mpsc::channel();
        let (tx2, rx2) = mpsc::channel();
        let other =
            hook(r#"{"session_id":"s2","hook_event_name":"PermissionRequest","tool_name":"Bash"}"#);
        permissions.hold(&hook(BASH), tx1);
        let kept = permissions.hold(&other, tx2);

        permissions.drop_session("s1");
        assert_eq!(rx1.try_recv(), Err(TryRecvError::Disconnected));
        assert_eq!(rx2.try_recv(), Err(TryRecvError::Empty));
        assert_eq!(
            permissions.cards().iter().map(|c| c.id).collect::<Vec<_>>(),
            [kept]
        );
    }

    /// The replies as the macOS island writes them, read from its source:
    /// a change on either side fails here. `TODO(#86)`: the path moves with
    /// the monorepo layout.
    #[test]
    fn replies_are_the_macos_islands_bytes() {
        let swift = include_str!("../../../../macos/Sources/Morrow/AppState.swift");
        for decision in [Decision::Allow, Decision::Deny] {
            let reply = std::str::from_utf8(decision.reply()).unwrap();
            assert!(swift.contains(&format!("#\"{reply}\"#")), "{reply}");
        }
        assert_eq!(
            serde_json::from_str::<Decision>(r#""deny""#).unwrap(),
            Decision::Deny
        );
    }

    #[test]
    fn summaries_are_one_short_line() {
        let input = |json: &str| serde_json::from_str::<Value>(json).unwrap();
        assert_eq!(summary(None), "");
        assert_eq!(
            summary(Some(&input(r#"{"file_path":"/a/b.rs","content":"x"}"#))),
            "/a/b.rs"
        );
        assert_eq!(
            summary(Some(&input(r#"{"query":"q"}"#))),
            r#"{"query":"q"}"#
        );
        let long = summary(Some(&input(&format!(
            r#"{{"command":"{}"}}"#,
            "x".repeat(300)
        ))));
        assert_eq!(long.chars().count(), SUMMARY_CHARS);
        assert!(long.ends_with('…'));
    }
}
