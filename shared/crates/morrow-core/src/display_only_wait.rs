//! A wait the island mirrors but cannot answer
//! (`macos/Sources/MorrowCore/DisplayOnlyWait.swift`).
//!
//! Some agents block on the user somewhere the island has no channel into: Claude
//! Desktop's Cowork permission cards, Cursor's in-IDE question, AiWork's approval and
//! plan confirmation, a terminal's own prompt announced by a `Notification` hook, a
//! Codex Desktop thread flagged `waitingOnUserInput`. Each of them only sets the
//! session's **status**. Nothing is queued, so there is no card to answer — the session
//! card says where to go, and the reminder chimes without offering a button.
//!
//! Whether a session waits display-only is **read off the session, never remembered**: it
//! does for exactly as long as its status says it is blocked on the user and the island
//! holds no request for it. A request the island holds — queued, or queued and dismissed
//! — is the island's own business, and a second reminder path for it would double every
//! chime.
//!
//! Two things are injected rather than read, and both for the same reason: [`answer_place`]
//! takes the session, and [`fallback_content`] takes the question text, because the port's
//! `SessionSnapshot` has no `cursorPendingQuestion` field yet. Guessing it from
//! `tool_description` would decorate a wait with the step *before* it.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::aiwork::{progress_text, tool_description, tool_name};
use crate::follow_up::FollowUpKind;
use crate::push::{PushContent, PushQuestionItem};
use crate::reduce::{AgentStatus, SessionSnapshot};

/// Claude Desktop hosts Cowork tasks and its own Code-tab sessions.
pub const CLAUDE_DESKTOP_BUNDLE_ID: &str = "com.anthropic.claudefordesktop";

/// The follow-up kind a session is display-only waiting on, or `None`.
pub fn kind(status: AgentStatus, island_holds_request: bool) -> Option<FollowUpKind> {
    if island_holds_request {
        return None;
    }
    match status {
        AgentStatus::WaitingApproval => Some(FollowUpKind::Approval),
        AgentStatus::WaitingQuestion => Some(FollowUpKind::Question),
        AgentStatus::Idle | AgentStatus::Processing | AgentStatus::Running => None,
    }
}

/// Every session display-only waiting on `kind`.
pub fn session_ids(
    waiting_on: FollowUpKind,
    sessions: &BTreeMap<String, SessionSnapshot>,
    island_request_session_ids: &BTreeSet<String>,
) -> BTreeSet<String> {
    sessions
        .iter()
        .filter(|(session_id, session)| {
            let holds = island_request_session_ids.contains(*session_id);
            kind(session.status, holds) == Some(waiting_on)
        })
        .map(|(session_id, _)| session_id.clone())
        .collect()
}

/// Where the user has to go to answer, as a push names it: the app hosting the session,
/// or the terminal it runs in. `None` for an SSH session — its prompt sits in a terminal
/// on another machine — and when nothing is known.
///
/// The port's `SessionSnapshot` has no `terminalName`, and the macOS version is a 60-line
/// table of bundle ids. What is here is the part that is a decision rather than a lookup:
/// a remote session, Claude Desktop, and AiWork name themselves. A session in an unknown
/// terminal returns `None`, which a push renders as "Respond on your Mac." — the same
/// wording as when the app is unknown.
pub fn answer_place(session: &SessionSnapshot) -> Option<String> {
    if session.is_remote() {
        return None;
    }
    // The card badge says "Claude", which on a phone reads as the model.
    if session.term_bundle_id.as_deref() == Some(CLAUDE_DESKTOP_BUNDLE_ID) {
        return Some("Claude Desktop".to_owned());
    }
    // AiWork (formerly DTCoder): badge names match the Hooks settings.
    match session.source.as_str() {
        "aiwork" => Some("AiWork".to_owned()),
        "aiwork-cli" => Some("AiWork CLI".to_owned()),
        _ => None,
    }
}

/// What is being asked, from the session alone — for a wait whose source left nothing
/// richer (a Codex Desktop thread flag, a wait rebuilt at launch).
///
/// `question` is the session's own pending question when it has one: only fields owned by
/// the wait are read, because `current_tool` / `tool_description` can still describe the
/// step before it.
pub fn fallback_content(kind: FollowUpKind, question: Option<&str>) -> PushContent {
    match kind {
        FollowUpKind::Question => question_content(question),
        FollowUpKind::Approval | FollowUpKind::Completion => PushContent::Permission {
            tool: None,
            detail: None,
        },
    }
}

/// The four helpers above had definitions and tests but no caller: this is
/// the one place the shell uses to name where the answer must go and what the
/// push says. Richer sources win; `fallback_content` is the last resort (#355).
pub fn answer_elsewhere_content(
    session: &SessionSnapshot,
    kind: FollowUpKind,
    event_name: Option<&str>,
    data: Option<&Value>,
    cowork: Option<&CoworkAudit>,
    question: Option<&str>,
) -> (Option<PushContent>, Option<String>) {
    let content = cowork
        .and_then(content_for_cowork)
        .or_else(|| event_name.and_then(|name| content_for_aiwork_event(name, data)))
        .or_else(|| Some(fallback_content(kind, question)));
    (content, answer_place(session))
}

/// Claude Desktop Cowork: the card on top in Claude Desktop.
pub fn content_for_cowork(audit: &CoworkAudit) -> Option<PushContent> {
    let active = audit.active_permission.as_ref()?;
    match audit.phase {
        CoworkPhase::WaitingApproval => Some(PushContent::Permission {
            tool: non_empty(active.tool_name.as_deref()),
            detail: non_empty(active.detail.as_deref()),
        }),
        CoworkPhase::WaitingQuestion => Some(question_content(active.detail.as_deref())),
        CoworkPhase::Idle | CoworkPhase::Processing => None,
    }
}

/// AiWork: the stream event that opened the wait.
pub fn content_for_aiwork_event(name: &str, data: Option<&Value>) -> Option<PushContent> {
    match name {
        "stream.approval_required" | "stream.plan_confirmation_required" => {
            let detail = field(data, "command")
                .or_else(|| field(data, "reason"))
                .or_else(|| field(data, "message"))
                .or_else(|| tool_description(data));
            Some(PushContent::Permission {
                tool: tool_name(data),
                detail,
            })
        }
        "stream.question_required" => {
            let text = field(data, "message").or_else(|| progress_text(data));
            Some(question_content(text.as_deref()))
        }
        _ => None,
    }
}

/// The phase a Cowork task is in, as the app records it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoworkPhase {
    Idle,
    Processing,
    WaitingApproval,
    WaitingQuestion,
}

/// The permission card on top in Claude Desktop. Only the fields a reminder needs: what
/// is being asked, and how.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoworkAudit {
    pub phase: CoworkPhase,
    pub active_permission: Option<CoworkPermission>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoworkPermission {
    pub tool_name: Option<String>,
    pub detail: Option<String>,
}

/// A single question, or an empty one when its text is unknown — the push then only says
/// who is asking and where to answer.
fn question_content(text: Option<&str>) -> PushContent {
    PushContent::Question {
        items: non_empty(text)
            .map(|question| {
                vec![PushQuestionItem {
                    question,
                    options: Vec::new(),
                    header: None,
                }]
            })
            .unwrap_or_default(),
        is_secret: false,
    }
}

fn string(data: &Value, key: &str) -> Option<String> {
    data.get(key)?.as_str().map(str::to_owned)
}

/// A trimmed field of `data`, or `None` for a blank one.
fn field(data: Option<&Value>, key: &str) -> Option<String> {
    data.and_then(|data| string(data, key))
        .and_then(|text| non_empty(Some(&text)))
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reduce::Timestamp;
    use serde_json::json;

    fn session(status: AgentStatus, source: &str) -> SessionSnapshot {
        let mut snapshot = SessionSnapshot::new(Timestamp(1.0));
        snapshot.status = status;
        snapshot.source = source.to_owned();
        snapshot
    }

    fn sessions(rows: &[(&str, AgentStatus)]) -> BTreeMap<String, SessionSnapshot> {
        rows.iter()
            .map(|(id, status)| ((*id).to_owned(), session(*status, "claude")))
            .collect()
    }

    #[test]
    fn only_a_blocked_status_is_a_display_only_wait() {
        assert_eq!(
            kind(AgentStatus::WaitingApproval, false),
            Some(FollowUpKind::Approval)
        );
        assert_eq!(
            kind(AgentStatus::WaitingQuestion, false),
            Some(FollowUpKind::Question)
        );
        assert_eq!(kind(AgentStatus::Running, false), None);
        assert_eq!(kind(AgentStatus::Processing, false), None);
        assert_eq!(kind(AgentStatus::Idle, false), None);
    }

    #[test]
    fn a_request_the_island_holds_is_not_a_display_only_wait() {
        // The island's own queue owns it: a second reminder path would double the chime.
        assert_eq!(kind(AgentStatus::WaitingApproval, true), None);
        assert_eq!(kind(AgentStatus::WaitingQuestion, true), None);
    }

    #[test]
    fn the_sessions_of_one_kind_are_read_off_their_status() {
        let all = sessions(&[
            ("a", AgentStatus::WaitingApproval),
            ("b", AgentStatus::WaitingQuestion),
            ("c", AgentStatus::Running),
            ("d", AgentStatus::WaitingApproval),
        ]);
        let island: BTreeSet<String> = BTreeSet::new();
        assert_eq!(
            session_ids(FollowUpKind::Approval, &all, &island),
            BTreeSet::from(["a".to_owned(), "d".to_owned()])
        );
        assert_eq!(
            session_ids(FollowUpKind::Question, &all, &island),
            BTreeSet::from(["b".to_owned()])
        );
        // One the island holds drops out of the display-only list.
        let held: BTreeSet<String> = BTreeSet::from(["a".to_owned()]);
        assert_eq!(
            session_ids(FollowUpKind::Approval, &all, &held),
            BTreeSet::from(["d".to_owned()])
        );
    }

    #[test]
    fn the_place_to_answer_is_the_hosting_app_or_nothing() {
        let mut claude_desktop = session(AgentStatus::WaitingApproval, "claude");
        claude_desktop.term_bundle_id = Some(CLAUDE_DESKTOP_BUNDLE_ID.to_owned());
        assert_eq!(
            answer_place(&claude_desktop).as_deref(),
            Some("Claude Desktop")
        );

        assert_eq!(
            answer_place(&session(AgentStatus::WaitingApproval, "aiwork")).as_deref(),
            Some("AiWork")
        );
        assert_eq!(
            answer_place(&session(AgentStatus::WaitingApproval, "aiwork-cli")).as_deref(),
            Some("AiWork CLI")
        );

        // An SSH session's prompt is in a terminal on another machine.
        let mut remote = session(AgentStatus::WaitingApproval, "claude");
        remote.remote_host_id = Some("host".to_owned());
        assert_eq!(answer_place(&remote), None);

        // A terminal the port does not name: the push says "on your Mac".
        assert_eq!(
            answer_place(&session(AgentStatus::WaitingApproval, "cursor")),
            None
        );
    }

    #[test]
    fn a_fallback_question_carries_the_text_it_was_given() {
        let content = fallback_content(FollowUpKind::Question, Some(" Which database? "));
        match content {
            PushContent::Question { items, is_secret } => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].question, "Which database?");
                assert!(!is_secret);
            }
            other => panic!("expected a question, got {other:?}"),
        }
    }

    #[test]
    fn a_fallback_question_without_text_asks_nothing_and_a_permission_never_has_one() {
        let PushContent::Question { items, .. } =
            fallback_content(FollowUpKind::Question, Some("  "))
        else {
            panic!("expected a question");
        };
        assert!(
            items.is_empty(),
            "the push says who and where, nothing more"
        );

        assert_eq!(
            fallback_content(FollowUpKind::Approval, Some("ignored")),
            PushContent::Permission {
                tool: None,
                detail: None
            }
        );
        assert_eq!(
            fallback_content(FollowUpKind::Completion, None),
            PushContent::Permission {
                tool: None,
                detail: None
            }
        );
    }

    #[test]
    fn a_cowork_card_reads_like_an_island_approval() {
        let audit = CoworkAudit {
            phase: CoworkPhase::WaitingApproval,
            active_permission: Some(CoworkPermission {
                tool_name: Some("Bash".to_owned()),
                detail: Some("rm -rf build".to_owned()),
            }),
        };
        assert_eq!(
            content_for_cowork(&audit),
            Some(PushContent::Permission {
                tool: Some("Bash".to_owned()),
                detail: Some("rm -rf build".to_owned()),
            })
        );

        let question = CoworkAudit {
            phase: CoworkPhase::WaitingQuestion,
            active_permission: Some(CoworkPermission {
                tool_name: None,
                detail: Some("Which database?".to_owned()),
            }),
        };
        match content_for_cowork(&question).expect("content") {
            PushContent::Question { items, .. } => {
                assert_eq!(items[0].question, "Which database?");
            }
            other => panic!("expected a question, got {other:?}"),
        }

        // Nothing on top, or a task that is not waiting: no push.
        assert_eq!(
            content_for_cowork(&CoworkAudit {
                phase: CoworkPhase::WaitingApproval,
                active_permission: None,
            }),
            None
        );
        for phase in [CoworkPhase::Idle, CoworkPhase::Processing] {
            assert_eq!(
                content_for_cowork(&CoworkAudit {
                    phase,
                    active_permission: Some(CoworkPermission {
                        tool_name: None,
                        detail: None,
                    }),
                }),
                None
            );
        }
    }

    #[test]
    fn answer_elsewhere_content_uses_the_richest_source_and_names_the_place() {
        let mut s = session(AgentStatus::WaitingApproval, "aiwork");
        s.term_bundle_id = Some("com.anthropic.claudefordesktop".to_owned());
        let (content, place) = answer_elsewhere_content(
            &s,
            FollowUpKind::Approval,
            Some("stream.approval_required"),
            Some(&json!({"command": "rm -rf build"})),
            None,
            None,
        );
        assert_eq!(place.as_deref(), Some("Claude Desktop"));
        assert!(matches!(content, Some(PushContent::Permission { .. })));

        // With no richer source, the fallback still gives the card a body and
        // an AiWork session still names its own host.
        let s = session(AgentStatus::WaitingQuestion, "aiwork");
        let (content, place) = answer_elsewhere_content(
            &s,
            FollowUpKind::Question,
            None,
            None,
            None,
            Some("Which database?"),
        );
        assert_eq!(place.as_deref(), Some("AiWork"));
        assert!(matches!(content, Some(PushContent::Question { .. })));
    }

    #[test]
    fn an_aiwork_approval_names_the_command_and_a_question_names_itself() {
        let approval = content_for_aiwork_event(
            "stream.approval_required",
            Some(&json!({"command": "npm publish", "tool_name": "Bash"})),
        );
        assert_eq!(
            approval,
            Some(PushContent::Permission {
                tool: Some("Bash".to_owned()),
                detail: Some("npm publish".to_owned()),
            })
        );

        let plan = content_for_aiwork_event(
            "stream.plan_confirmation_required",
            Some(&json!({"reason": "3 steps"})),
        );
        assert_eq!(
            plan,
            Some(PushContent::Permission {
                tool: None,
                detail: Some("3 steps".to_owned()),
            })
        );

        let question = content_for_aiwork_event(
            "stream.question_required",
            Some(&json!({"message": "Which one?"})),
        );
        match question.expect("content") {
            PushContent::Question { items, .. } => assert_eq!(items[0].question, "Which one?"),
            other => panic!("expected a question, got {other:?}"),
        }

        // No command, no reason, no message: the island's own description of the step.
        let described = content_for_aiwork_event(
            "stream.approval_required",
            Some(&json!({"input": {"command": "cargo test"}})),
        );
        assert_eq!(
            described,
            Some(PushContent::Permission {
                tool: None,
                detail: Some("cargo test".to_owned()),
            })
        );
    }

    #[test]
    fn any_other_aiwork_event_opens_no_wait() {
        assert_eq!(content_for_aiwork_event("stream.started", None), None);
        assert_eq!(content_for_aiwork_event("stream.text_delta", None), None);
        assert_eq!(
            content_for_aiwork_event("stream.approval_resolved", None),
            None
        );
    }
}
