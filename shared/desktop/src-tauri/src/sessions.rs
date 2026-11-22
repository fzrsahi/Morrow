//! Session reduction backed by `morrow-core::reduce` (`reduceEvent`): the
//! reducer, then the attention policy's intents where the reducer used to
//! put them, projecting `SessionSnapshot` into the panel's `Session` rows.
//! The shell's outlets for those intents — a sound, a completion card — are
//! still to come (plan §5.2, Phase 3); the sessions and the cards are what it
//! runs today.

use std::collections::HashMap;

use morrow_core::intake::SourceCatalog;
use morrow_core::reduce::{
    AgentStatus, HookEvent, ReduceContext, SessionSnapshot, SideEffect, Timestamp, reduce_event,
};
use morrow_protocol::HookPayload;
use serde::Serialize;

/// The seven words a card can show (Swift `SessionCardStatus`). The word
/// carries the meaning; the colour only repeats it. Serialized snake_case so
/// the frontend can key its colour table directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CardStatus {
    NeedsYou,
    Working,
    Thinking,
    Done,
    Idle,
    Stopped,
    Error,
}

impl CardStatus {
    /// Sort tier: what needs you, then what is working, then the rest.
    fn tier(self) -> u8 {
        match self {
            Self::NeedsYou => 0,
            Self::Working | Self::Thinking => 1,
            Self::Done | Self::Idle | Self::Stopped | Self::Error => 2,
        }
    }
}

/// `SessionCardStatus(session:)`: an idle card reads DONE once it has a prompt
/// or a reply, STOPPED when the user interrupted, ERROR on a failed turn.
pub fn card_status(s: &SessionSnapshot) -> CardStatus {
    match s.status {
        AgentStatus::WaitingApproval | AgentStatus::WaitingQuestion => CardStatus::NeedsYou,
        AgentStatus::Running => CardStatus::Working,
        AgentStatus::Processing => CardStatus::Thinking,
        AgentStatus::Idle => {
            if s.last_turn_failed {
                CardStatus::Error
            } else if s.interrupted {
                CardStatus::Stopped
            } else if s.last_assistant_message.is_some()
                || s.last_user_prompt.is_some()
                || !s.recent_messages.is_empty()
            {
                CardStatus::Done
            } else {
                CardStatus::Idle
            }
        }
    }
}

/// What the panel shows of one session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Session {
    pub session_id: String,
    /// `_source`, or `claude` without one (Swift `SessionSnapshot.source`).
    pub source: String,
    /// `hook_event_name` of the latest hook.
    pub last_event: String,
    /// The last path component of `cwd`, or empty.
    pub cwd_name: String,
    /// Milliseconds since the Unix epoch, when the latest hook arrived.
    pub updated_at: u64,
    /// The spelled-out status the card shows.
    pub status: CardStatus,
    /// Running tool and its one-line description; empty when none.
    pub current_tool: String,
    pub tool_description: String,
    pub last_user_prompt: String,
    pub last_assistant_message: String,
    pub model: String,
    /// The full working directory; `cwd_name` is its last component.
    pub cwd: String,
    pub term_app: String,
    pub tty_path: String,
    pub cli_pid: Option<i32>,
}

#[derive(Debug, Default)]
pub struct Sessions {
    inner: morrow_core::reduce::Sessions,
    last_events: HashMap<String, String>,
    sources: SourceCatalog,
    home_directory: String,
}

impl Sessions {
    /// Creates a session store whose reducer ignores `home_directory` as a
    /// bare project `cwd`.
    pub fn with_home(home_directory: impl Into<String>) -> Self {
        Self {
            home_directory: home_directory.into(),
            ..Self::default()
        }
    }

    /// Reduces `payload` at `now_ms`, applying `RemoveSession` effects locally
    /// and returning all emitted `SideEffect`s for the shell — the policy's
    /// `PlaySound` and `EnqueueCompletion` among them.
    pub fn apply(&mut self, payload: &HookPayload, now_ms: u64) -> Vec<SideEffect> {
        let Some(event) = HookEvent::new(payload.as_object().clone(), &self.sources) else {
            return Vec::new();
        };
        let clock = || Timestamp(now_ms as f64 / 1000.0);
        let ctx = ReduceContext {
            clock: &clock,
            sources: &self.sources,
            home_directory: &self.home_directory,
            max_history: 50,
            reply_complete_placeholder: "Reply complete",
        };
        let effects = reduce_event(&mut self.inner, &event, &ctx);
        for effect in &effects {
            if let SideEffect::RemoveSession { session_id } = effect {
                self.inner.remove(session_id);
                self.last_events.remove(session_id);
            }
        }
        let card_id = event.session_id.as_deref().unwrap_or("default");
        if self.inner.contains_key(card_id) {
            self.last_events
                .insert(card_id.to_owned(), event.event_name);
        }
        effects
    }

    /// #185: the ✕ on a session card. The session is gone until a hook starts
    /// it again; its last event goes with it so no stale label survives.
    pub fn remove(&mut self, session_id: &str) {
        self.inner.remove(session_id);
        self.last_events.remove(session_id);
    }

    /// NEEDS YOU first, then working/thinking, then the rest; newest first
    /// inside a tier and by session id on a tie, so the order is stable.
    pub fn snapshot(&self) -> Vec<Session> {
        let mut sessions: Vec<Session> = self
            .inner
            .iter()
            .map(|(session_id, snap)| Session {
                session_id: session_id.clone(),
                source: snap.source.clone(),
                last_event: self
                    .last_events
                    .get(session_id)
                    .cloned()
                    .unwrap_or_default(),
                cwd_name: snap
                    .cwd
                    .as_deref()
                    .map(basename)
                    .unwrap_or_default()
                    .to_owned(),
                updated_at: (snap.last_activity.0 * 1000.0).round().max(0.0) as u64,
                status: card_status(snap),
                current_tool: snap.current_tool().unwrap_or_default().to_owned(),
                tool_description: or_empty(&snap.tool_description),
                last_user_prompt: or_empty(&snap.last_user_prompt),
                last_assistant_message: or_empty(&snap.last_assistant_message),
                model: or_empty(&snap.model),
                cwd: or_empty(&snap.cwd),
                term_app: or_empty(&snap.term_app),
                tty_path: or_empty(&snap.tty_path),
                cli_pid: snap.cli_pid,
            })
            .collect();
        sessions.sort_by(|a, b| {
            a.status
                .tier()
                .cmp(&b.status.tier())
                .then_with(|| b.updated_at.cmp(&a.updated_at))
                .then_with(|| a.session_id.cmp(&b.session_id))
        });
        sessions
    }
}

fn or_empty(value: &Option<String>) -> String {
    value.clone().unwrap_or_default()
}

fn basename(path: &str) -> &str {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use morrow_protocol::decode_payload;

    fn hook(json: &str) -> HookPayload {
        decode_payload(json.as_bytes()).unwrap()
    }

    #[test]
    fn removing_a_session_keeps_the_others_and_their_order() {
        let mut sessions = Sessions::default();
        sessions.apply(
            &hook(r#"{"session_id":"a","hook_event_name":"SessionStart","cwd":"/home/u/a"}"#),
            10,
        );
        sessions.apply(
            &hook(r#"{"session_id":"b","hook_event_name":"PreToolUse","cwd":"/home/u/b"}"#),
            20,
        );
        sessions.remove("a");
        let rows = sessions.snapshot();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].session_id, "b");
        assert_eq!(rows[0].last_event, "PreToolUse");
    }

    #[test]
    fn the_latest_hook_updates_the_session_and_preserves_prior_cwd() {
        let mut sessions = Sessions::default();
        sessions.apply(
            &hook(r#"{"session_id":"a","hook_event_name":"SessionStart","cwd":"/home/u/proj/"}"#),
            10,
        );
        sessions.apply(
            &hook(r#"{"session_id":"a","hook_event_name":"PreToolUse","_source":"codex"}"#),
            20,
        );
        assert_eq!(
            sessions.snapshot(),
            vec![Session {
                session_id: "a".into(),
                source: "codex".into(),
                last_event: "PreToolUse".into(),
                cwd_name: "proj".into(),
                updated_at: 20,
                status: CardStatus::Working,
                current_tool: String::new(),
                tool_description: String::new(),
                last_user_prompt: String::new(),
                last_assistant_message: String::new(),
                model: String::new(),
                cwd: "/home/u/proj/".into(),
                term_app: String::new(),
                tty_path: String::new(),
                cli_pid: None,
            }]
        );
    }

    #[test]
    fn sessions_come_newest_first_and_default_to_claude() {
        let mut sessions = Sessions::default();
        sessions.apply(
            &hook(r#"{"session_id":"old","hook_event_name":"Stop","cwd":"/srv/api"}"#),
            1,
        );
        sessions.apply(
            &hook(r#"{"session_id":"new","hook_event_name":"PreToolUse","cwd":"/home/u/web/"}"#),
            2,
        );
        let snapshot = sessions.snapshot();
        let rows: Vec<_> = snapshot
            .iter()
            .map(|s| {
                (
                    s.session_id.as_str(),
                    s.source.as_str(),
                    s.cwd_name.as_str(),
                )
            })
            .collect();
        assert_eq!(rows, [("new", "claude", "web"), ("old", "claude", "api")]);
    }

    #[test]
    fn session_end_emits_remove_session_and_clears_the_card() {
        let mut sessions = Sessions::default();
        sessions.apply(
            &hook(r#"{"session_id":"a","hook_event_name":"SessionStart","cwd":"/srv/api"}"#),
            10,
        );
        let effects = sessions.apply(
            &hook(r#"{"session_id":"a","hook_event_name":"SessionEnd"}"#),
            20,
        );
        assert_eq!(
            effects,
            vec![SideEffect::RemoveSession {
                session_id: "a".into()
            }]
        );
        assert!(sessions.snapshot().is_empty());
    }

    #[test]
    fn permission_request_preserves_project_cwd_and_normalizes_source() {
        let mut sessions = Sessions::with_home("/home/u");
        sessions.apply(
            &hook(r#"{"session_id":"a","hook_event_name":"SessionStart","cwd":"/home/u/proj"}"#),
            10,
        );
        sessions.apply(
            &hook(
                r#"{"session_id":"a","hook_event_name":"PermissionRequest","cwd":"/home/u","_source":"anti-gravity"}"#,
            ),
            20,
        );
        let snap = sessions.inner.get("a").expect("session a");
        assert_eq!(snap.status, AgentStatus::Idle);
        assert_eq!(snap.source, "antigravity");
        assert_eq!(snap.cwd.as_deref(), Some("/home/u/proj"));

        // A permission_prompt notification moves status to WaitingApproval.
        sessions.apply(
            &hook(
                r#"{"session_id":"a","hook_event_name":"Notification","notification_type":"permission_prompt"}"#,
            ),
            30,
        );
        let snap = sessions.inner.get("a").expect("session a");
        assert_eq!(snap.status, AgentStatus::WaitingApproval);
    }

    #[test]
    fn basename_of_odd_paths() {
        assert_eq!(basename(""), "");
        assert_eq!(basename("/"), "");
        assert_eq!(basename("proj"), "proj");
        assert_eq!(basename("/a/b//"), "b");
    }

    #[test]
    fn snapshot_carries_the_fields_the_session_card_renders() {
        let mut sessions = Sessions::with_home("/home/u");
        sessions.apply(
            &hook(r#"{"session_id":"a","hook_event_name":"SessionStart","cwd":"/home/u/web-app","model":"claude-sonnet-4","_term_app":"kitty","_tty":"/dev/pts/3","_ppid":4242}"#),
            10,
        );
        sessions.apply(
            &hook(r#"{"session_id":"a","hook_event_name":"UserPromptSubmit","prompt":"Add filters to the dashboard page"}"#),
            11,
        );
        sessions.apply(
            &hook(r#"{"session_id":"a","hook_event_name":"PreToolUse","tool_name":"Edit","tool_input":{"file_path":"src/components/Dashboard.tsx"}}"#),
            12,
        );

        let rows = sessions.snapshot();
        let row = &rows[0];
        assert_eq!(row.session_id, "a");
        assert_eq!(row.status, CardStatus::Working);
        assert_eq!(row.current_tool, "Edit");
        assert!(
            row.tool_description.contains("Dashboard.tsx"),
            "description: {:?}",
            row.tool_description
        );
        assert_eq!(row.last_user_prompt, "Add filters to the dashboard page");
        assert_eq!(row.last_assistant_message, "");
        assert_eq!(row.model, "claude-sonnet-4");
        assert_eq!(row.cwd, "/home/u/web-app");
        assert_eq!(row.cwd_name, "web-app");
        assert_eq!(row.term_app, "kitty");
        assert_eq!(row.tty_path, "/dev/pts/3");
        assert_eq!(row.cli_pid, Some(4242));
    }

    #[test]
    fn a_finished_turn_carries_the_last_reply_and_reads_done() {
        let mut sessions = Sessions::with_home("/home/u");
        sessions.apply(
            &hook(r#"{"session_id":"a","hook_event_name":"UserPromptSubmit","prompt":"Ship it"}"#),
            10,
        );
        sessions.apply(
            &hook(r#"{"session_id":"a","hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"npm test"}}"#),
            11,
        );
        sessions.apply(
            &hook(r#"{"session_id":"a","hook_event_name":"Stop","last_assistant_message":"Shipped; all tests pass."}"#),
            12,
        );

        let row = &sessions.snapshot()[0];
        assert_eq!(row.status, CardStatus::Done);
        assert_eq!(row.last_user_prompt, "Ship it");
        assert_eq!(row.last_assistant_message, "Shipped; all tests pass.");
        assert_eq!(row.current_tool, "");
    }

    #[test]
    fn card_status_maps_every_agent_state_and_idle_reason() {
        let mut s = SessionSnapshot::new(Timestamp(1.0));
        s.status = AgentStatus::WaitingApproval;
        assert_eq!(card_status(&s), CardStatus::NeedsYou);
        s.status = AgentStatus::WaitingQuestion;
        assert_eq!(card_status(&s), CardStatus::NeedsYou);
        s.status = AgentStatus::Running;
        assert_eq!(card_status(&s), CardStatus::Working);
        s.status = AgentStatus::Processing;
        assert_eq!(card_status(&s), CardStatus::Thinking);
        s.status = AgentStatus::Idle;
        assert_eq!(card_status(&s), CardStatus::Idle);
        s.last_user_prompt = Some("hello".into());
        assert_eq!(card_status(&s), CardStatus::Done);
        s.last_user_prompt = None;
        s.interrupted = true;
        assert_eq!(card_status(&s), CardStatus::Stopped);
        s.interrupted = false;
        s.last_turn_failed = true;
        assert_eq!(card_status(&s), CardStatus::Error);
    }

    #[test]
    fn needs_you_first_then_working_newest_first_within_each_tier() {
        let mut sessions = Sessions::default();
        sessions.apply(
            &hook(r#"{"session_id":"needs","hook_event_name":"Notification","notification_type":"permission_prompt"}"#),
            5,
        );
        sessions.apply(
            &hook(r#"{"session_id":"working","hook_event_name":"PreToolUse","tool_name":"Bash"}"#),
            10,
        );
        sessions.apply(
            &hook(r#"{"session_id":"thinking","hook_event_name":"UserPromptSubmit","prompt":"hi"}"#),
            20,
        );
        sessions.apply(
            &hook(r#"{"session_id":"done","hook_event_name":"SessionStart"}"#),
            30,
        );
        sessions.apply(
            &hook(r#"{"session_id":"done","hook_event_name":"UserPromptSubmit","prompt":"hi"}"#),
            31,
        );
        sessions.apply(&hook(r#"{"session_id":"done","hook_event_name":"Stop"}"#), 32);
        sessions.apply(
            &hook(r#"{"session_id":"idle","hook_event_name":"SessionStart"}"#),
            40,
        );

        let rows = sessions.snapshot();
        let order: Vec<_> = rows.iter().map(|s| s.session_id.as_str()).collect();
        assert_eq!(order, ["needs", "thinking", "working", "idle", "done"]);
    }
}
