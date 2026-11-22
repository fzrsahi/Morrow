//! Session reduction backed by `morrow-core::reduce` (`reduceEvent`): the
//! reducer, then the attention policy's intents where the reducer used to
//! put them, projecting `SessionSnapshot` into the panel's `Session` rows.
//! The shell's outlets for those intents — a sound, a completion card — are
//! still to come (plan §5.2, Phase 3); the sessions and the cards are what it
//! runs today.

use std::collections::HashMap;
use std::path::Path;

use morrow_core::attention::{
    AttentionIntents, AttentionMoment, AttentionSubject, intents as attention_intents,
};
use morrow_core::git::{self, Disk, DiskEntry, GitBranchInfo};
use morrow_core::intake::SourceCatalog;
use morrow_core::reduce::{
    AgentStatus, HookEvent, ReduceContext, SessionSnapshot, SideEffect, Timestamp,
    reduce_hook_event,
};
use morrow_core::transcript::ScanDelta;
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
    /// Newest assistant text from the session's transcript tail (#192). The
    /// frontend shows only this on the reply row; the hook fields above still
    /// own card status, tier order and the collapsed bar.
    pub live_reply: String,
    pub model: String,
    /// The full working directory; `cwd_name` is its last component.
    pub cwd: String,
    pub term_app: String,
    pub tty_path: String,
    pub cli_pid: Option<i32>,
    /// A remote session has no window on this machine; the card hides its jump
    /// icon (#187).
    pub is_remote: bool,
    /// Checked-out branch of `cwd`, resolved by `morrow_core::git::read`.
    pub git_branch: String,
    /// The checkout is a linked worktree; the card marks it with ⧉.
    pub git_is_worktree: bool,
    /// The agent's own checklist projected from `SessionSnapshot.agent_tasks`.
    pub tasks_done: usize,
    pub tasks_total: usize,
    pub task_active: String,
    pub subagent_count: usize,
    /// Idle-only recap from `SessionSnapshot::visible_recap`.
    pub recap: String,
}

/// The core reader walks `.git` without spawning `git`; the shell supplies the
/// disk it walks (`std::fs`), as a detached read would on macOS.
struct StdDisk;

impl Disk for StdDisk {
    fn entry(&self, path: &Path) -> Option<DiskEntry> {
        match std::fs::metadata(path) {
            Ok(meta) if meta.is_dir() => Some(DiskEntry::Directory),
            Ok(_) => std::fs::read_to_string(path).ok().map(DiskEntry::File),
            Err(_) => None,
        }
    }
}

/// What one hook did: its effects and the attention policy's intents.
/// A shell that settles the policy itself (the completion card's style and
/// Smart Suppress) needs both, as `reduce_hook_event`'s docs prescribe.
#[derive(Default)]
pub struct Applied {
    pub effects: Vec<SideEffect>,
    pub intents: AttentionIntents,
}

#[derive(Debug, Default)]
pub struct Sessions {
    inner: morrow_core::reduce::Sessions,
    last_events: HashMap<String, String>,
    /// The transcript path a hook reported for each session. Kept here rather
    /// than read from the reducer: `SessionStart` rebuilds its snapshot and
    /// drops the core's copy of the field.
    transcript_paths: HashMap<String, String>,
    /// Live transcript replies, keyed by session id, newest text only.
    live_replies: HashMap<String, String>,
    sources: SourceCatalog,
    home_directory: String,
    /// Last branch reading per session: `(cwd at the time, reading)`. A new
    /// cwd (or a Stop, where the turn may have switched branches) re-reads it.
    branches: HashMap<String, (String, Option<GitBranchInfo>)>,
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
    /// and returning the effects and the attention intents together.
    pub fn apply_with_intents(&mut self, payload: &HookPayload, now_ms: u64) -> Applied {
        let Some(event) = HookEvent::new(payload.as_object().clone(), &self.sources) else {
            return Applied::default();
        };
        let clock = || Timestamp(now_ms as f64 / 1000.0);
        let ctx = ReduceContext {
            clock: &clock,
            sources: &self.sources,
            home_directory: &self.home_directory,
            max_history: 50,
            reply_complete_placeholder: "Reply complete",
        };
        let reduction = reduce_hook_event(&mut self.inner, &event, &ctx);
        let session_id = event.session_id.as_deref().unwrap_or("default");
        let intents = attention_intents(
            &AttentionMoment::HookEvent(reduction.facts),
            AttentionSubject::from_session(self.inner.get(session_id)),
        );
        // The same placements as `reduce_event`: the sound (later in the
        // list) first, so the completion keeps its reducer-provided slot.
        let mut effects = reduction.effects;
        if let Some(sound) = &intents.sound {
            let at = reduction.slots.sound.unwrap_or(effects.len()).min(effects.len());
            effects.insert(at, SideEffect::PlaySound(sound.clone()));
        }
        if intents.completion.is_some() {
            let at = reduction
                .slots
                .completion
                .unwrap_or(effects.len())
                .min(effects.len());
            effects.insert(
                at,
                SideEffect::EnqueueCompletion {
                    session_id: session_id.to_owned(),
                },
            );
        }
        for effect in &effects {
            if let SideEffect::RemoveSession { session_id } = effect {
                self.remove(session_id);
            }
        }
        if let Some(snap) = self.inner.get(session_id) {
            let cwd = snap.cwd.clone().unwrap_or_default();
            let stale = self
                .branches
                .get(session_id)
                .map_or(true, |(seen, _)| seen != &cwd)
                || event.event_name == "Stop";
            if stale {
                let info = (!cwd.is_empty())
                    .then(|| git::read(&cwd, &StdDisk))
                    .flatten();
                self.branches.insert(session_id.to_owned(), (cwd, info));
            }
            self.last_events
                .insert(session_id.to_owned(), event.event_name);
            if let Some(path) = payload
                .as_object()
                .get("transcript_path")
                .and_then(serde_json::Value::as_str)
                .filter(|path| !path.is_empty())
            {
                self.transcript_paths
                    .insert(session_id.to_owned(), path.to_owned());
            }
        }
        Applied { effects, intents }
    }

    /// All emitted `SideEffect`s for the shell — the policy's `PlaySound` and
    /// `EnqueueCompletion` among them. Kept for the session tests; the feed
    /// uses `apply_with_intents` so it can settle the completion policy.
    #[cfg(test)]
    pub fn apply(&mut self, payload: &HookPayload, now_ms: u64) -> Vec<SideEffect> {
        self.apply_with_intents(payload, now_ms).effects
    }

    /// The transcript paths the hooks reported, for the shell's tailer.
    pub fn transcript_paths(&self) -> Vec<(String, String)> {
        self.transcript_paths
            .iter()
            .map(|(id, path)| (id.clone(), path.clone()))
            .collect()
    }

    /// Apply one parsed chunk of a session's transcript. Only the live reply
    /// changes here: a streaming assistant line is not a finished turn, so
    /// promoting it to `last_assistant_message` would move a working card to
    /// DONE and break #183's tier order.
    pub fn apply_transcript_delta(&mut self, session_id: &str, delta: &ScanDelta) -> bool {
        if !self.inner.contains_key(session_id) {
            return false;
        }
        let current = self.live_replies.get(session_id).cloned();
        let next = match delta
            .last_assistant_message
            .as_deref()
            .filter(|reply| !reply.trim().is_empty())
        {
            Some(reply) => Some(reply.to_owned()),
            None if delta.last_user_prompt.is_some() => None,
            None => current.clone(),
        };
        if next == current {
            return false;
        }
        match next {
            Some(reply) => {
                self.live_replies.insert(session_id.to_owned(), reply);
            }
            None => {
                self.live_replies.remove(session_id);
            }
        }
        true
    }

    /// Forget a session: the ✕ on its card (#185), a dead process (#198).
    /// Its last event, cached git branch, transcript path and live reply go
    /// with it so no stale label survives (#184/#192).
    pub fn remove(&mut self, session_id: &str) {
        self.inner.remove(session_id);
        self.last_events.remove(session_id);
        self.branches.remove(session_id);
        self.transcript_paths.remove(session_id);
        self.live_replies.remove(session_id);
    }

    /// No sessions left to watch.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// NEEDS YOU first, then working/thinking, then the rest; newest first
    /// inside a tier and by session id on a tie, so the order is stable.
    pub fn snapshot(&self) -> Vec<Session> {
        let mut sessions: Vec<Session> = self
            .inner
            .iter()
            .map(|(session_id, snap)| {
                let branch = self
                    .branches
                    .get(session_id)
                    .and_then(|(_, info)| info.as_ref());
                let tasks = &snap.agent_tasks;
                Session {
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
                    live_reply: self
                        .live_replies
                        .get(session_id)
                        .cloned()
                        .unwrap_or_default(),
                    model: or_empty(&snap.model),
                    cwd: or_empty(&snap.cwd),
                    term_app: or_empty(&snap.term_app),
                    tty_path: or_empty(&snap.tty_path),
                    cli_pid: snap.cli_pid,
                    is_remote: snap.is_remote(),
                    git_branch: branch.map_or_else(String::new, |i| i.branch.clone()),
                    git_is_worktree: branch.is_some_and(|i| i.is_worktree),
                    tasks_done: tasks.completed_count(),
                    tasks_total: tasks.items.len(),
                    task_active: tasks
                        .current()
                        .map_or_else(String::new, |task| task.progress_label().to_owned()),
                    subagent_count: snap.subagents.len(),
                    recap: snap
                        .visible_recap()
                        .map_or_else(String::new, |r| r.text.clone()),
                }
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
                live_reply: String::new(),
                model: String::new(),
                cwd: "/home/u/proj/".into(),
                term_app: String::new(),
                tty_path: String::new(),
                cli_pid: None,
                is_remote: false,
                git_branch: String::new(),
                git_is_worktree: false,
                tasks_done: 0,
                tasks_total: 0,
                task_active: String::new(),
                subagent_count: 0,
                recap: String::new(),
            }]
        );
    }

    #[test]
    fn a_remote_session_is_marked_so_its_card_hides_the_jump_icon() {
        let mut sessions = Sessions::default();
        sessions.apply(
            &hook(
                r#"{"session_id":"r","hook_event_name":"SessionStart","cwd":"/srv/app","_remote_host_id":"devbox"}"#,
            ),
            10,
        );
        assert!(sessions.snapshot()[0].is_remote);
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
    fn removing_a_session_drops_its_hook_history_too() {
        let mut sessions = Sessions::default();
        sessions.apply(
            &hook(r#"{"session_id":"a","hook_event_name":"PreToolUse"}"#),
            10,
        );
        sessions.remove("a");
        assert!(sessions.snapshot().is_empty());
        // A new session with the same id starts with no last event.
        sessions.apply(&hook(r#"{"session_id":"a","hook_event_name":"Stop"}"#), 20);
        assert_eq!(sessions.snapshot()[0].last_event, "Stop");
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
    fn a_transcript_delta_carries_the_reply_without_changing_the_hook_card() {
        use morrow_core::transcript::ScanDelta;

        let mut sessions = Sessions::with_home("/home/u");
        sessions.apply(
            &hook(r#"{"session_id":"a","hook_event_name":"SessionStart","transcript_path":"/tmp/a.jsonl"}"#),
            10,
        );
        assert_eq!(sessions.snapshot()[0].live_reply, "");

        let mut delta = ScanDelta::default();
        delta.last_user_prompt = Some("Add filters".into());
        assert!(
            !sessions.apply_transcript_delta("a", &delta),
            "a prompt alone shows nothing"
        );

        delta.last_assistant_message = Some("Adding filters".into());
        assert!(sessions.apply_transcript_delta("a", &delta));
        let row = &sessions.snapshot()[0];
        assert_eq!(row.live_reply, "Adding filters");
        // #183 owns the hook-derived card: status, prompt, reply and time are
        // untouched by tail progress, so tier ordering stays the same.
        assert_eq!(row.status, CardStatus::Idle);
        assert_eq!(row.last_user_prompt, "");
        assert_eq!(row.last_assistant_message, "");
        assert_eq!(row.updated_at, 10);
        assert!(
            !sessions.apply_transcript_delta("a", &delta),
            "unchanged text is not a change"
        );

        delta.last_assistant_message = Some("Wired the bar".into());
        sessions.apply_transcript_delta("a", &delta);
        assert_eq!(sessions.snapshot()[0].live_reply, "Wired the bar");

        // A new turn supersedes the old reply, and a gone session forgets it.
        let mut next_turn = ScanDelta::default();
        next_turn.last_user_prompt = Some("Ship it".into());
        sessions.apply_transcript_delta("a", &next_turn);
        assert_eq!(sessions.snapshot()[0].live_reply, "");
        sessions.apply(
            &hook(r#"{"session_id":"a","hook_event_name":"SessionEnd"}"#),
            20,
        );
        assert!(sessions.snapshot().is_empty());
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
    fn context_fields_follow_the_core_snapshot_and_refresh_on_cwd_change() {
        use std::time::{SystemTime, UNIX_EPOCH};

        use morrow_core::agent_tasks::{AgentTaskItem, AgentTaskList, AgentTaskStatus};
        use morrow_core::reduce::{AgentStatus, SubagentState, Timestamp};
        use morrow_core::session_recap::SessionRecap;

        // Two real checkouts: a primary one and a linked worktree, so the
        // branch comes from the core reader and the worktree mark is honest.
        let root = std::env::temp_dir().join(format!(
            "morrow-184-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        let primary = root.join("primary");
        let primary_git = primary.join(".git");
        std::fs::create_dir_all(&primary_git).unwrap();
        std::fs::write(primary_git.join("HEAD"), "ref: refs/heads/feat/dashboard\n").unwrap();
        let worktree = root.join("linked");
        let worktree_meta = root.join(".git/worktrees/wt");
        std::fs::create_dir_all(&worktree_meta).unwrap();
        std::fs::write(
            worktree_meta.join("HEAD"),
            "ref: refs/heads/exp/feature-store\n",
        )
        .unwrap();
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::write(
            worktree.join(".git"),
            format!("gitdir: {}\n", worktree_meta.display()),
        )
        .unwrap();

        let mut sessions = Sessions::default();
        sessions.apply(
            &hook(&format!(
                r#"{{"session_id":"a","hook_event_name":"SessionStart","cwd":"{}"}}"#,
                primary.display()
            )),
            10,
        );
        let snap = sessions.inner.get_mut("a").expect("session a");
        snap.status = AgentStatus::Idle;
        snap.agent_tasks = AgentTaskList::restored(
            vec![
                AgentTaskItem {
                    id: "1".into(),
                    task_id: None,
                    create_op_id: None,
                    title: "Wire filters".into(),
                    active_form: Some("Wiring filters".into()),
                    status: AgentTaskStatus::Completed,
                },
                AgentTaskItem {
                    id: "2".into(),
                    task_id: None,
                    create_op_id: None,
                    title: "Ship it".into(),
                    active_form: Some("Shipping it".into()),
                    status: AgentTaskStatus::InProgress,
                },
            ],
            None,
        );
        snap.subagents.insert(
            "sub-1".into(),
            SubagentState::new("sub-1", "general-purpose", Timestamp(10.0)),
        );
        snap.recap = Some(SessionRecap::new("Wired the filters", Timestamp(9.0)));

        let row = &sessions.snapshot()[0];
        assert_eq!(row.git_branch, "feat/dashboard");
        assert!(!row.git_is_worktree);
        assert_eq!(row.tasks_done, 1);
        assert_eq!(row.tasks_total, 2);
        assert_eq!(row.task_active, "Shipping it");
        assert_eq!(row.subagent_count, 1);
        assert_eq!(row.recap, "Wired the filters");

        // A new cwd on the same session re-reads the branch through the core.
        sessions.apply(
            &hook(&format!(
                r#"{{"session_id":"a","hook_event_name":"PreToolUse","tool_name":"Bash","cwd":"{}"}}"#,
                worktree.display()
            )),
            20,
        );
        let row = &sessions.snapshot()[0];
        assert_eq!(row.git_branch, "exp/feature-store");
        assert!(row.git_is_worktree);

        std::fs::remove_dir_all(&root).unwrap();
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
            &hook(
                r#"{"session_id":"thinking","hook_event_name":"UserPromptSubmit","prompt":"hi"}"#,
            ),
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
        sessions.apply(
            &hook(r#"{"session_id":"done","hook_event_name":"Stop"}"#),
            32,
        );
        sessions.apply(
            &hook(r#"{"session_id":"idle","hook_event_name":"SessionStart"}"#),
            40,
        );

        let rows = sessions.snapshot();
        let order: Vec<_> = rows.iter().map(|s| s.session_id.as_str()).collect();
        assert_eq!(order, ["needs", "thinking", "working", "idle", "done"]);
    }
}
