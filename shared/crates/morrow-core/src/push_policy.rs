//! Whether a push goes out at all (`macos/Sources/MorrowCore/PushPolicy.swift`).
//!
//! #238 AC 1, second of four. The rendering is in [`crate::push`]; this is the part that
//! decides, and every rule here exists because a push was once wrong in a way nobody
//! would notice:
//!
//! - **Presence beats recency.** A locked or dark screen means away no matter how recent
//!   the last keystroke was (⌃⌘Q locks instantly), and Smart Suppress only counts while
//!   someone is there: a locked Mac keeps reporting the last frontmost app, which is
//!   exactly when the push matters most.
//! - **A completion after an error is the same turn ending.** The error already told the
//!   story, so the completion inside its shadow is dropped as a duplicate.
//! - **Volume never silences an approval.** Finished turns, errors and reminders past the
//!   global cap are dropped — a flood of them says less than one — but an agent blocked
//!   on a person is never refused for volume.
//!
//! Nothing here reads a clock or the network: presence, time and the channel set are
//! injected, so a policy case is a table entry rather than a timing test.

use crate::push::PushEventKind;
use crate::reduce::{SessionSnapshot, subsession};
use serde::Serialize;

/// Is anyone at this machine? Every signal is readable without a privacy permission; the
/// app gathers them, this decides.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PushPresence {
    pub screen_locked: bool,
    pub screen_saver_running: bool,
    pub displays_asleep: bool,
    /// False when another user owns the console (fast user switching).
    pub session_on_console: bool,
    /// Seconds since the last keyboard / mouse / trackpad event.
    pub idle_seconds: f64,
}

impl PushPresence {
    /// A present user at an awake, unlocked console.
    pub fn present(idle_seconds: f64) -> Self {
        Self {
            session_on_console: true,
            idle_seconds,
            ..Self::default()
        }
    }

    /// A locked or dark screen means away however recent the last input was; otherwise,
    /// quiet hands for `idle_threshold` seconds.
    pub fn is_away(&self, idle_threshold: f64) -> bool {
        if self.screen_locked
            || self.screen_saver_running
            || self.displays_asleep
            || !self.session_on_console
        {
            return true;
        }
        self.idle_seconds >= idle_threshold.max(0.0)
    }
}

/// Why a push did not go out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PushSkipReason {
    /// "Only when I'm away" is on and the user is at the machine.
    UserPresent,
    /// The user is here and Smart Suppress kept the island quiet because the agent's own
    /// terminal is in front.
    SmartSuppressed,
    /// A subagent's turn ended; the parent's completion is the one that matters.
    Subagent,
    /// The user stopped the turn themselves (Esc / Ctrl-C).
    Interrupted,
    /// Same session, same kind, moments ago.
    Duplicate,
    /// The cap on pushes that can wait was reached. Approvals and questions are never
    /// refused for volume.
    RateLimited,
    /// No enabled, configured channel takes this kind.
    NoChannel,
    /// A reminder fired after its request was already answered.
    NothingPending,
}

/// What the gate needs to know.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PushGateInput {
    pub kind: PushEventKind,
    pub only_when_away: bool,
    pub idle_threshold: f64,
    pub presence: PushPresence,
    pub smart_suppressed: bool,
    pub is_subagent: bool,
    pub interrupted: bool,
}

/// `None` means send. Channel selection and dedupe come after this.
pub fn evaluate(input: &PushGateInput) -> Option<PushSkipReason> {
    // Only a finished turn has these: an approval from a subagent is still an approval.
    if input.kind == PushEventKind::Completion {
        if input.is_subagent {
            return Some(PushSkipReason::Subagent);
        }
        if input.interrupted {
            return Some(PushSkipReason::Interrupted);
        }
    }
    let away = input.presence.is_away(input.idle_threshold);
    if input.only_when_away && !away {
        return Some(PushSkipReason::UserPresent);
    }
    if !away && input.smart_suppressed {
        return Some(PushSkipReason::SmartSuppressed);
    }
    None
}

/// A channel the shell has checked for this event. Kept generic so `morrow-core`
/// does not need to know the channel implementations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PushChannelInput {
    pub enabled: bool,
    pub accepts_event: bool,
}

impl PushChannelInput {
    pub fn accepts(enabled: bool, accepts_event: bool) -> Self {
        Self {
            enabled,
            accepts_event,
        }
    }
}

/// Everything the one push decision needs, with the clock and the channel state
/// injected. The shell builds this from settings, presence and its dedupe/limiter.
#[derive(Clone, Debug)]
pub struct PushDecisionInput<'a> {
    pub gate: PushGateInput,
    pub channels: &'a [PushChannelInput],
    /// False when the reminder's request has already been answered.
    pub pending: bool,
    pub now: f64,
}

/// The single decision point: all nine skip reasons can come out of here, and
/// the global volume cap is enforced for the kinds that may be dropped.
pub fn decide(input: &PushDecisionInput<'_>, limiter: &PushRateLimiter) -> Option<PushSkipReason> {
    if let Some(reason) = evaluate(&input.gate) {
        return Some(reason);
    }
    if input.gate.kind == PushEventKind::Reminder && !input.pending {
        return Some(PushSkipReason::NothingPending);
    }
    if !input
        .channels
        .iter()
        .any(|channel| channel.enabled && channel.accepts_event)
    {
        return Some(PushSkipReason::NoChannel);
    }
    if !must_deliver(input.gate.kind) && limiter.next_slot(input.now) > input.now {
        return Some(PushSkipReason::RateLimited);
    }
    None
}

/// One moment, not one hook: the same session and kind inside the window is one push.
#[derive(Clone, Debug)]
pub struct PushDeduplicator {
    /// Same session + same kind (+ same request) inside this many seconds is one moment.
    pub window: f64,
    /// A completion right after an error in the same session is the same turn ending.
    pub error_shadow: f64,
    /// How long a push is remembered at all.
    pub memory: f64,
    last_sent: std::collections::BTreeMap<String, f64>,
}

impl Default for PushDeduplicator {
    fn default() -> Self {
        Self {
            window: 60.0,
            error_shadow: 60.0,
            memory: 3_600.0,
            last_sent: std::collections::BTreeMap::new(),
        }
    }
}

impl PushDeduplicator {
    /// When a push of `kind` for `session_id` was last admitted, within `memory`.
    pub fn last_sent(&self, kind: PushEventKind, session_id: &str) -> Option<f64> {
        self.last_sent
            .get(&slot_key(kind, session_id, None))
            .copied()
    }

    /// `None` = admitted, and recorded at `now`.
    ///
    /// `request_key` names which approval or question this is (a tool call id, or a
    /// fingerprint of what is asked); `None` for kinds about the session as a whole.
    pub fn admit(
        &mut self,
        kind: PushEventKind,
        session_id: &str,
        request_key: Option<&str>,
        now: f64,
    ) -> Option<PushSkipReason> {
        self.prune(now);
        let slot = slot_key(kind, session_id, request_key);
        if let Some(last) = self.last_sent.get(&slot) {
            if now - last < self.window {
                return Some(PushSkipReason::Duplicate);
            }
        }
        if kind == PushEventKind::Completion {
            if let Some(error) =
                self.last_sent
                    .get(&slot_key(PushEventKind::Error, session_id, None))
            {
                if now - error < self.error_shadow {
                    return Some(PushSkipReason::Duplicate);
                }
            }
        }
        self.last_sent.insert(slot, now);
        None
    }

    /// The request was answered: its next push is news, not a repeat.
    pub fn forget(&mut self, kind: PushEventKind, session_id: &str, request_key: Option<&str>) {
        self.last_sent
            .remove(&slot_key(kind, session_id, request_key));
    }

    /// Bounded by time rather than by session count.
    fn prune(&mut self, now: f64) {
        let horizon = self.window.max(self.error_shadow).max(self.memory);
        self.last_sent.retain(|_, sent| now - *sent < horizon);
    }
}

fn slot_key(kind: PushEventKind, session_id: &str, request_key: Option<&str>) -> String {
    let kind = kind_name(kind);
    match request_key.filter(|key| !key.is_empty()) {
        Some(request) => format!("{kind}|{session_id}|{request}"),
        None => format!("{kind}|{session_id}"),
    }
}

fn kind_name(kind: PushEventKind) -> &'static str {
    match kind {
        PushEventKind::Permission => "permission",
        PushEventKind::Question => "question",
        PushEventKind::Completion => "completion",
        PushEventKind::Error => "error",
        PushEventKind::Reminder => "reminder",
    }
}

/// At most `count` sends in any rolling `seconds`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PushRateWindow {
    pub count: usize,
    pub seconds: f64,
}

/// The channel families a configured push can belong to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PushChannelKind {
    Bark,
    Ntfy,
    Telegram,
    Dingtalk,
    Wecom,
    Feishu,
    Slack,
}

impl PushChannelKind {
    /// What the service allows one incoming webhook, with headroom: exceeding it gets
    /// messages rejected (Feishu, Slack) or the robot silenced for ten minutes
    /// (DingTalk). Personal channels have no limit worth throttling at this volume.
    pub fn rate_limits(self) -> Vec<PushRateWindow> {
        match self {
            Self::Dingtalk | Self::Wecom => vec![PushRateWindow {
                count: 18,
                seconds: 60.0,
            }],
            Self::Feishu => vec![
                PushRateWindow {
                    count: 90,
                    seconds: 60.0,
                },
                PushRateWindow {
                    count: 4,
                    seconds: 1.0,
                },
            ],
            Self::Slack => vec![PushRateWindow {
                count: 1,
                seconds: 1.0,
            }],
            Self::Bark | Self::Ntfy | Self::Telegram => Vec::new(),
        }
    }
}

/// Sliding-window limiter: when the next send fits every window.
#[derive(Clone, Debug)]
pub struct PushRateLimiter {
    pub windows: Vec<PushRateWindow>,
    sends: Vec<f64>,
}

impl PushRateLimiter {
    pub fn new(windows: Vec<PushRateWindow>) -> Self {
        Self {
            windows,
            sends: Vec::new(),
        }
    }

    /// The earliest moment at or after `now` the next send fits every window.
    pub fn next_slot(&self, now: f64) -> f64 {
        let mut slot = now;
        for window in self.windows.iter().filter(|window| window.count > 0) {
            let mut recent: Vec<f64> = self
                .sends
                .iter()
                .copied()
                .filter(|sent| now - sent < window.seconds)
                .collect();
            if recent.len() < window.count {
                continue;
            }
            recent.sort_by(|a, b| a.total_cmp(b));
            // Room opens when enough recent sends age out of the window.
            let frees = recent[recent.len() - window.count] + window.seconds;
            slot = slot.max(frees);
        }
        slot
    }

    pub fn record(&mut self, at: f64) {
        let horizon = self
            .windows
            .iter()
            .map(|window| window.seconds)
            .fold(0.0, f64::max);
        self.sends.retain(|sent| at - *sent < horizon);
        self.sends.push(at);
    }
}

/// Across all channels and sessions: finished turns, errors and reminders past it are
/// dropped — a flood of them says less than one.
pub const GLOBAL_RATE_WINDOW: PushRateWindow = PushRateWindow {
    count: 12,
    seconds: 60.0,
};

/// Approvals and questions keep an agent blocked until someone answers: never refused by
/// the global cap, and queued rather than dropped when a team chat's own limit is hit.
pub fn must_deliver(kind: PushEventKind) -> bool {
    matches!(kind, PushEventKind::Permission | PushEventKind::Question)
}

/// A push that can be dropped still waits this long for a team chat's slot — per-second
/// limits (Feishu, Slack) free up that fast.
pub const DROP_GRACE_SECONDS: f64 = 5.0;

/// The error a hook reports when a session stopped on one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PushSessionError {
    pub kind: Option<String>,
    pub detail: Option<String>,
}

/// `StopFailure` (Claude Code; Grok as `StopFailure` / `stop_failure`) and Copilot's
/// `errorOccurred` / `ErrorOccurred`.
///
/// The normalizer folds `StopFailure` onto Stop, so it is matched on the raw name.
/// A Copilot error that is `recoverable` did not stop the session and is not an error.
pub fn session_error(event_name: &str, raw: &serde_json::Value) -> Option<PushSessionError> {
    let text = |value: Option<&serde_json::Value>| {
        value
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    };
    match event_name {
        "StopFailure" | "stop_failure" => Some(PushSessionError {
            kind: text(raw.get("error")),
            detail: text(raw.get("last_assistant_message"))
                .or_else(|| text(raw.get("error_details")))
                .or_else(|| text(raw.get("message"))),
        }),
        "errorOccurred" | "ErrorOccurred" => {
            if raw.get("recoverable").and_then(|value| value.as_bool()) == Some(true) {
                return None;
            }
            let error = raw.get("error");
            Some(PushSessionError {
                kind: error.and_then(|error| text(error.get("name"))),
                detail: error
                    .and_then(|error| text(error.get("message")))
                    .or_else(|| text(raw.get("error"))),
            })
        }
        _ => None,
    }
}

/// Whether a completion belongs to a subagent rather than the conversation the user
/// started: an explicit subagent id, or a Cursor Task card kept separate.
pub fn is_subagent_completion(
    agent_id: Option<&str>,
    session_id: &str,
    session: Option<&SessionSnapshot>,
) -> bool {
    if agent_id.is_some_and(|id| !id.is_empty()) {
        return true;
    }
    let Some(session) = session else {
        return false;
    };
    if !subsession::is_cursor_family_source(
        Some(session.source.as_str()),
        &crate::intake::session_snapshot::SourceCatalog::default(),
    ) {
        return false;
    }
    subsession::is_likely_cursor_task_card(
        session_id,
        session.provider_session_id.as_deref(),
        session.transcript_path.as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(kind: PushEventKind) -> PushGateInput {
        PushGateInput {
            kind,
            only_when_away: false,
            idle_threshold: 300.0,
            presence: PushPresence::present(0.0),
            smart_suppressed: false,
            is_subagent: false,
            interrupted: false,
        }
    }

    #[test]
    fn all_nine_skip_reasons_are_reachable_from_the_decision() {
        let now = 1_000.0;
        let channel = PushChannelInput::accepts(true, true);
        let open = PushRateLimiter::new(Vec::new());
        let decide_with =
            |gate, channels: &[PushChannelInput], pending, limiter: &PushRateLimiter| {
                decide(
                    &PushDecisionInput {
                        gate,
                        channels,
                        pending,
                        now,
                    },
                    limiter,
                )
            };

        let mut present = input(PushEventKind::Permission);
        present.only_when_away = true;
        assert_eq!(
            decide_with(present, &[channel], true, &open),
            Some(PushSkipReason::UserPresent)
        );
        let mut suppressed = input(PushEventKind::Permission);
        suppressed.smart_suppressed = true;
        assert_eq!(
            decide_with(suppressed, &[channel], true, &open),
            Some(PushSkipReason::SmartSuppressed)
        );
        let mut subagent = input(PushEventKind::Completion);
        subagent.is_subagent = true;
        assert_eq!(
            decide_with(subagent, &[channel], true, &open),
            Some(PushSkipReason::Subagent)
        );
        let mut interrupted = input(PushEventKind::Completion);
        interrupted.interrupted = true;
        assert_eq!(
            decide_with(interrupted, &[channel], true, &open),
            Some(PushSkipReason::Interrupted)
        );
        assert_eq!(
            decide_with(input(PushEventKind::Reminder), &[channel], false, &open),
            Some(PushSkipReason::NothingPending)
        );
        assert_eq!(
            decide_with(input(PushEventKind::Permission), &[], true, &open),
            Some(PushSkipReason::NoChannel)
        );
        assert_eq!(
            decide_with(
                input(PushEventKind::Permission),
                &[PushChannelInput::accepts(false, true)],
                true,
                &open
            ),
            Some(PushSkipReason::NoChannel)
        );

        let mut limited = PushRateLimiter::new(vec![PushRateWindow {
            count: 1,
            seconds: 60.0,
        }]);
        limited.record(now);
        assert_eq!(
            decide_with(input(PushEventKind::Completion), &[channel], true, &limited),
            Some(PushSkipReason::RateLimited)
        );
        // An approval is never refused for volume: it is the one that unblocks.
        assert_eq!(
            decide_with(input(PushEventKind::Permission), &[channel], true, &limited),
            None
        );

        // `Duplicate` still comes from the deduplicator, the other single entry point.
        let mut dedupe = PushDeduplicator::default();
        assert_eq!(
            dedupe.admit(PushEventKind::Permission, "s", Some("1"), now),
            None
        );
        assert_eq!(
            dedupe.admit(PushEventKind::Permission, "s", Some("1"), now + 1.0),
            Some(PushSkipReason::Duplicate)
        );
    }

    #[test]
    fn a_locked_or_dark_screen_means_away_however_recent_the_input() {
        let mut presence = PushPresence::present(0.0);
        assert!(!presence.is_away(300.0));
        presence.screen_locked = true;
        assert!(presence.is_away(300.0));
        presence.screen_locked = false;
        presence.displays_asleep = true;
        assert!(presence.is_away(300.0));
        presence.displays_asleep = false;
        presence.screen_saver_running = true;
        assert!(presence.is_away(300.0));
        presence.screen_saver_running = false;
        presence.session_on_console = false;
        assert!(presence.is_away(300.0), "another user owns the console");

        // Quiet hands for the threshold, and a negative threshold is zero.
        let quiet = PushPresence::present(300.0);
        assert!(quiet.is_away(300.0));
        assert!(PushPresence::present(1.0).is_away(-5.0));
    }

    #[test]
    fn only_when_away_needs_an_absent_user() {
        let mut away = input(PushEventKind::Permission);
        away.only_when_away = true;
        assert_eq!(evaluate(&away), Some(PushSkipReason::UserPresent));
        away.presence = PushPresence::present(900.0);
        assert_eq!(evaluate(&away), None);
    }

    #[test]
    fn smart_suppress_only_counts_while_someone_is_there() {
        let mut present = input(PushEventKind::Question);
        present.smart_suppressed = true;
        assert_eq!(evaluate(&present), Some(PushSkipReason::SmartSuppressed));

        // A locked Mac keeps reporting the last frontmost app: suppressing then would
        // drop the push exactly when it matters most.
        let mut locked = present;
        locked.presence.screen_locked = true;
        assert_eq!(evaluate(&locked), None);
    }

    #[test]
    fn a_subagent_or_an_interrupted_turn_does_not_push_but_an_approval_still_does() {
        let mut completion = input(PushEventKind::Completion);
        completion.is_subagent = true;
        assert_eq!(evaluate(&completion), Some(PushSkipReason::Subagent));

        let mut interrupted = input(PushEventKind::Completion);
        interrupted.interrupted = true;
        assert_eq!(evaluate(&interrupted), Some(PushSkipReason::Interrupted));

        // An approval from a subagent is still an approval: the answer is needed.
        let mut subagent_approval = input(PushEventKind::Permission);
        subagent_approval.is_subagent = true;
        assert_eq!(evaluate(&subagent_approval), None);
    }

    #[test]
    fn the_same_moment_twice_in_the_window_is_one_push() {
        let mut dedupe = PushDeduplicator::default();
        assert_eq!(
            dedupe.admit(PushEventKind::Question, "s1", Some("q1"), 0.0),
            None
        );
        assert_eq!(
            dedupe.admit(PushEventKind::Question, "s1", Some("q1"), 30.0),
            Some(PushSkipReason::Duplicate)
        );
        // Past the window it is news again.
        assert_eq!(
            dedupe.admit(PushEventKind::Question, "s1", Some("q1"), 61.0),
            None
        );
        // A request-keyed push is remembered under its key, not under the session.
        assert_eq!(dedupe.last_sent(PushEventKind::Question, "s1"), None);
        dedupe.admit(PushEventKind::Question, "s1", None, 62.0);
        assert_eq!(dedupe.last_sent(PushEventKind::Question, "s1"), Some(62.0));
    }

    #[test]
    fn a_different_request_of_the_same_session_is_news() {
        let mut dedupe = PushDeduplicator::default();
        dedupe.admit(PushEventKind::Permission, "s1", Some("tool-1"), 0.0);
        assert_eq!(
            dedupe.admit(PushEventKind::Permission, "s1", Some("tool-2"), 5.0),
            None,
            "the approval after one answered on the phone is a new wait"
        );
        // An answered request is a new wait too.
        dedupe.forget(PushEventKind::Permission, "s1", Some("tool-1"));
        assert_eq!(
            dedupe.admit(PushEventKind::Permission, "s1", Some("tool-1"), 6.0),
            None
        );
    }

    #[test]
    fn a_completion_inside_an_errors_shadow_is_the_same_turn_ending() {
        let mut dedupe = PushDeduplicator::default();
        dedupe.admit(PushEventKind::Error, "s1", None, 0.0);
        assert_eq!(
            dedupe.admit(PushEventKind::Completion, "s1", None, 30.0),
            Some(PushSkipReason::Duplicate)
        );
        // Another session is unaffected, and past the shadow it is news.
        assert_eq!(
            dedupe.admit(PushEventKind::Completion, "s2", None, 30.0),
            None
        );
        assert_eq!(
            dedupe.admit(PushEventKind::Completion, "s1", None, 61.0),
            None
        );
    }

    #[test]
    fn the_dedupe_table_is_bounded_by_time() {
        let mut dedupe = PushDeduplicator::default();
        dedupe.admit(PushEventKind::Error, "old", None, 0.0);
        dedupe.admit(PushEventKind::Error, "new", None, 4_000.0);
        // The 4000 s-old entry is past every horizon and is pruned on the next admit.
        dedupe.admit(PushEventKind::Error, "other", None, 4_001.0);
        assert_eq!(dedupe.last_sent(PushEventKind::Error, "old"), None);
        assert_eq!(dedupe.last_sent(PushEventKind::Error, "new"), Some(4_000.0));
    }

    #[test]
    fn a_rate_limiter_says_when_the_next_send_fits() {
        let mut limiter = PushRateLimiter::new(vec![PushRateWindow {
            count: 2,
            seconds: 60.0,
        }]);
        assert_eq!(limiter.next_slot(0.0), 0.0);
        limiter.record(0.0);
        assert_eq!(limiter.next_slot(10.0), 10.0);
        limiter.record(10.0);
        // Two sends inside the window: room opens when the first ages out.
        assert_eq!(limiter.next_slot(20.0), 60.0);
        assert_eq!(limiter.next_slot(61.0), 61.0);
    }

    #[test]
    fn every_window_must_fit_not_just_one() {
        // Feishu: 90 a minute and 4 a second.
        let mut limiter = PushRateLimiter::new(PushChannelKind::Feishu.rate_limits());
        for second in 0..4 {
            limiter.record(f64::from(second));
        }
        // The per-second window is full, the per-minute one is not.
        let slot = limiter.next_slot(4.0);
        assert!((4.0..=5.0).contains(&slot), "{slot}");
    }

    #[test]
    fn a_team_chat_is_throttled_and_a_personal_channel_is_not() {
        assert_eq!(
            PushChannelKind::Dingtalk.rate_limits(),
            vec![PushRateWindow {
                count: 18,
                seconds: 60.0
            }]
        );
        assert_eq!(PushChannelKind::Wecom.rate_limits().len(), 1);
        assert_eq!(PushChannelKind::Feishu.rate_limits().len(), 2);
        assert_eq!(PushChannelKind::Slack.rate_limits().len(), 1);
        for personal in [
            PushChannelKind::Bark,
            PushChannelKind::Ntfy,
            PushChannelKind::Telegram,
        ] {
            assert!(personal.rate_limits().is_empty(), "{personal:?}");
        }
    }

    #[test]
    fn volume_never_silences_an_approval() {
        assert!(must_deliver(PushEventKind::Permission));
        assert!(must_deliver(PushEventKind::Question));
        for droppable in [
            PushEventKind::Completion,
            PushEventKind::Error,
            PushEventKind::Reminder,
        ] {
            assert!(!must_deliver(droppable), "{droppable:?}");
        }
        assert_eq!(GLOBAL_RATE_WINDOW.count, 12);
        assert_eq!(DROP_GRACE_SECONDS, 5.0);
    }

    #[test]
    fn a_stop_failure_is_an_error_with_a_class_and_a_readable_detail() {
        let raw = serde_json::json!({
            "error": "rate_limit",
            "last_assistant_message": "API Error: Rate limit reached",
        });
        let error = session_error("StopFailure", &raw).expect("an error");
        assert_eq!(error.kind.as_deref(), Some("rate_limit"));
        assert_eq!(
            error.detail.as_deref(),
            Some("API Error: Rate limit reached")
        );

        // A hook of any other name is not an error.
        assert_eq!(session_error("Stop", &raw), None);
    }

    #[test]
    fn a_recoverable_copilot_error_did_not_stop_the_session() {
        let recoverable = serde_json::json!({
            "error": {"name": "NetworkError", "message": "retrying"},
            "recoverable": true,
        });
        assert_eq!(session_error("errorOccurred", &recoverable), None);

        let fatal = serde_json::json!({
            "error": {"name": "AuthError", "message": "bad token"},
        });
        let error = session_error("ErrorOccurred", &fatal).expect("an error");
        assert_eq!(error.kind.as_deref(), Some("AuthError"));
        assert_eq!(error.detail.as_deref(), Some("bad token"));
    }

    #[test]
    fn a_subagent_completion_is_recognised_by_its_id() {
        assert!(is_subagent_completion(Some("agent-1"), "s1", None));
        assert!(!is_subagent_completion(Some(""), "s1", None));
        assert!(!is_subagent_completion(None, "s1", None));
    }
}
