//! The attention policy (`AttentionPolicy`, `macos/Sources/MorrowCore/AttentionPolicy.swift`)
//! and the sound routing it reads (`EventSoundRouting.swift`): what a moment
//! calls for on its own — a sound, a completion card, a push, a follow-up
//! reminder — given the session it happened to.
//!
//! [`intents`] and [`settle`] are the ports; their rules have rows in
//! `fixtures/core/attention.json` and `fixtures/core/settle.json`, both
//! exported from Swift. The shell reads the settings and the scene and hands
//! [`settle`] the facts; [`settle_completion`] remains the shell's earlier
//! completion-only projection, which asks Smart Suppress as the card shows.

use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::Value;

use crate::reduce::{HookEventFacts, SessionSnapshot};

/// `SoundCue`: the sound cues the policy speaks in, as `SoundManager` keys
/// its sounds and the per-event switches.
pub mod sound_cue {
    /// An approval or a question needs the user ("approval needed").
    pub const REQUEST: &str = "PermissionRequest";
    pub const SESSION_START: &str = "SessionStart";
    pub const PROMPT: &str = "UserPromptSubmit";
    /// A turn finished.
    pub const TURN_END: &str = "Stop";
    /// The app was launched by the user.
    pub const LAUNCH: &str = "AppLaunch";
}

/// `EventSoundRouting.turnFailed`: the cue for a turn that died on an error.
/// The name follows Claude Code's hook.
pub const TURN_FAILED: &str = "StopFailure";

/// `EventSoundRouting.soundEvent(rawEventName:normalizedEventName:)`: the cue
/// a hook event rings, or `None` when it stays silent.
///
/// Sounds mark turn boundaries, not steps inside a turn: a failed tool call
/// is routine. `StopFailure` is only visible before `EventNormalizer` folds
/// it onto `Stop`, which is why the raw name comes along.
pub fn sound_event(raw_event_name: &str, normalized_event_name: &str) -> Option<String> {
    if normalized_event_name == "PostToolUseFailure" {
        return None;
    }
    // The launch chime's cue is the app's own: a hook event that happens to
    // carry its name rings nothing.
    if normalized_event_name == sound_cue::LAUNCH {
        return None;
    }
    if raw_event_name == "StopFailure" || raw_event_name == "stop_failure" {
        return Some(TURN_FAILED.to_owned());
    }
    Some(normalized_event_name.to_owned())
}

/// Something that happened which may deserve the user's attention.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttentionMoment {
    /// A hook event, as the reducer applied it.
    HookEvent(HookEventFacts),
    /// The island queued an approval. `first_of_burst`: no other approval was
    /// visibly waiting (a dismissed one does not count).
    ApprovalQueued { first_of_burst: bool },
    /// The island queued a question. `smart_suppressible`: false for Codex
    /// Desktop's app-server question, whose card and push have never been
    /// held back by Smart Suppress.
    QuestionQueued {
        first_of_burst: bool,
        smart_suppressible: bool,
    },
    /// An agent got blocked on the user somewhere the island cannot answer
    /// for it (`DisplayOnlyWait`).
    RequestRaisedElsewhere,
    /// A session began waiting display-only on `kind`. `announce`: a live
    /// wait from a source the island can never own, rather than a launch
    /// rebuild, a backfill, or a terminal prompt the island may still queue.
    DisplayOnlyWaitBegan {
        kind: FollowUpReminderKind,
        announce: bool,
    },
    /// A turn began in a source the hook reducer never sees.
    TurnStarted { new_session: bool },
    /// A turn ended in a source the hook reducer never sees.
    TurnEnded { end: TurnEnd, host: TurnHost },
    /// A follow-up reminder came due and the island itself delivers it.
    ReminderDue {
        kind: FollowUpReminderKind,
        turn_failed: bool,
    },
    /// The user launched the app (a launch at login stays silent before it
    /// gets here).
    AppLaunched,
}

/// How a turn ended in a source that bypasses the hook reducer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnEnd {
    Completed,
    /// On an error: a failed AiWork stream, a Cowork turn whose result is an
    /// error.
    Failed,
    /// The user stopped it: Claude Desktop's Stop button, AiWork's
    /// `stream.aborted`.
    Stopped,
}

/// Where a turn that bypasses the hook reducer ran.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnHost {
    /// Claude Desktop (Cowork tasks), read from its session store.
    ClaudeDesktop,
    /// An AiWork / Agentix daemon (`sessions.watch`).
    Aiwork,
}

/// `FollowUpReminderKind`: what a session was waiting for when its reminders
/// started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FollowUpReminderKind {
    Approval,
    Question,
    Completion,
}

impl FollowUpReminderKind {
    /// The case name the fixtures and the frontend use.
    pub fn raw_value(self) -> &'static str {
        match self {
            Self::Approval => "approval",
            Self::Question => "question",
            Self::Completion => "completion",
        }
    }

    pub fn from_raw_value(value: &str) -> Option<Self> {
        match value {
            "approval" => Some(Self::Approval),
            "question" => Some(Self::Question),
            "completion" => Some(Self::Completion),
            _ => None,
        }
    }
}

/// What the rules read about the session a moment happened to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AttentionSubject {
    /// The session is a child another agent spawned, working for its parent:
    /// it names one (`SessionSnapshot.parent_session_id`), or the app just
    /// folded its card into its parent's.
    pub is_child: bool,
    /// The user stopped the session's turn (Esc / Ctrl-C).
    pub interrupted: bool,
}

impl AttentionSubject {
    /// As `session` stands; a session the island no longer holds is nobody's
    /// child.
    pub fn from_session(session: Option<&SessionSnapshot>) -> Self {
        Self {
            is_child: session.is_some_and(|s| s.parent_session_id.is_some()),
            interrupted: session.is_some_and(|s| s.interrupted),
        }
    }
}

/// What a push is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttentionPushKind {
    /// The turn ended on an error.
    Error,
    /// The turn finished. A subagent's or an interrupted one is offered to
    /// the push gate all the same: it turns both away and says why.
    Completion {
        is_subagent: bool,
        interrupted: bool,
    },
    Approval,
    Question,
    /// A display-only wait began: answer it where it was asked.
    DisplayOnlyWait,
}

/// Follow-up reminder bookkeeping a moment starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FollowUpIntent {
    /// One nudge for this finished turn if nobody looks at it.
    TrackCompletion { turn_failed: bool },
    /// A display-only wait began: its reminders start over.
    RestartWait(FollowUpReminderKind),
}

/// What a moment calls for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AttentionIntents {
    /// The sound cue (a [`sound_cue`] or a hook event's name), or `None`.
    pub sound: Option<String>,
    /// The moment finished a turn the island announces.
    pub completion: Option<Completion>,
    pub push: Option<Push>,
    pub follow_up: Option<FollowUpIntent>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Completion {
    pub turn_failed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Push {
    pub kind: AttentionPushKind,
    /// Smart Suppress may hold it back while the user is at the machine.
    pub smart_suppressible: bool,
}

/// `AttentionPolicy.intents(for:subject:)`: what `moment` calls for, before
/// the user's settings and the room have their say.
pub fn intents(moment: &AttentionMoment, subject: AttentionSubject) -> AttentionIntents {
    let sound = |cue: &str| AttentionIntents {
        sound: Some(cue.to_owned()),
        ..AttentionIntents::default()
    };
    match moment {
        AttentionMoment::HookEvent(facts) => hook_event_intents(facts, subject),
        AttentionMoment::ApprovalQueued { first_of_burst } => {
            // The chime marks the start of a burst of approvals: three queued
            // back to back are one interruption, not three. The push is per
            // request; its dedupe tells replays apart.
            AttentionIntents {
                sound: first_of_burst.then(|| sound_cue::REQUEST.to_owned()),
                push: Some(Push {
                    kind: AttentionPushKind::Approval,
                    smart_suppressible: true,
                }),
                ..AttentionIntents::default()
            }
        }
        AttentionMoment::QuestionQueued {
            first_of_burst,
            smart_suppressible,
        } => AttentionIntents {
            sound: first_of_burst.then(|| sound_cue::REQUEST.to_owned()),
            push: Some(Push {
                kind: AttentionPushKind::Question,
                smart_suppressible: *smart_suppressible,
            }),
            ..AttentionIntents::default()
        },
        // Nothing is queued — the answer can only be given where it was
        // asked — so the sound is what says the agent is blocked.
        AttentionMoment::RequestRaisedElsewhere => sound(sound_cue::REQUEST),
        // Reminders start over for every wait that begins. Only a live wait
        // from a source the island can never own is pushed: a launch rebuild
        // or a backfill is old news, and a terminal `permission_prompt`
        // races its own PermissionRequest hook, whose richer push would lose
        // its dedupe slot to this one.
        AttentionMoment::DisplayOnlyWaitBegan { kind, announce } => AttentionIntents {
            push: announce.then_some(Push {
                kind: AttentionPushKind::DisplayOnlyWait,
                smart_suppressible: true,
            }),
            follow_up: Some(FollowUpIntent::RestartWait(*kind)),
            ..AttentionIntents::default()
        },
        AttentionMoment::TurnStarted { new_session } => {
            if subject.is_child {
                return AttentionIntents::default();
            }
            sound(if *new_session {
                sound_cue::SESSION_START
            } else {
                sound_cue::PROMPT
            })
        }
        AttentionMoment::TurnEnded { end, host } => {
            if subject.is_child {
                return AttentionIntents::default();
            }
            match end {
                TurnEnd::Completed => finished_turn_intents(false, subject),
                TurnEnd::Failed => finished_turn_intents(true, subject),
                // Claude Desktop's Stop button: the user is right there and
                // knows. No jingle, no completion card, no push, no
                // follow-up; the card just reads interrupted.
                TurnEnd::Stopped => match host {
                    TurnHost::ClaudeDesktop => AttentionIntents::default(),
                    // `stream.aborted` has always popped its card (it reads
                    // "interrupted"), like an Esc in a hook-driven agent —
                    // but it rings nothing, pushes nothing and is not
                    // followed up.
                    TurnHost::Aiwork => AttentionIntents {
                        completion: Some(Completion { turn_failed: false }),
                        ..AttentionIntents::default()
                    },
                },
            }
        }
        // The same sound as the moment it reminds of: a turn that died rings
        // the error jingle again, not "done".
        AttentionMoment::ReminderDue { kind, turn_failed } => match kind {
            FollowUpReminderKind::Approval | FollowUpReminderKind::Question => {
                sound(sound_cue::REQUEST)
            }
            FollowUpReminderKind::Completion => sound(if *turn_failed {
                TURN_FAILED
            } else {
                sound_cue::TURN_END
            }),
        },
        AttentionMoment::AppLaunched => sound(sound_cue::LAUNCH),
    }
}

fn hook_event_intents(facts: &HookEventFacts, subject: AttentionSubject) -> AttentionIntents {
    // A session another agent spawned works for that agent's turn: its own
    // start, prompt, turn end and failure are the parent's business, not the
    // user's. Its approvals and questions arrive as their own moments and
    // still alert.
    if subject.is_child {
        return AttentionIntents::default();
    }
    // A conversation held somewhere else — a Hermes gateway chat, the API
    // server, a background run: its card follows along, but a bot answering
    // on Telegram isn't news at the desk.
    if facts.held_elsewhere {
        return AttentionIntents::default();
    }

    let cue = sound_event(&facts.raw_event_name, &facts.event_name);
    let mut intents = AttentionIntents::default();
    // Only an event that moved the session itself rings: not one dropped as
    // stale, filed under a subagent, or ending the session. A turn the user
    // stopped ends without a chime: it did not complete, and the user knows —
    // its card still pops.
    if facts.applied_to_session && !(facts.ended_turn && subject.interrupted) {
        intents.sound = cue.clone();
    }
    if facts.ended_turn {
        // The normalizer folds StopFailure onto Stop; only the raw name says
        // the turn died (same rule as its sound).
        let turn_failed = cue.as_deref() == Some(TURN_FAILED);
        intents.completion = Some(Completion { turn_failed });
        intents.follow_up = follow_up(turn_failed, subject);
    }
    // A turn that stopped on an error pushes the error instead of a
    // completion: one push for one failed turn. Only the session's own turn,
    // though: a subagent's failure filed under the session (or a stale one)
    // is not its error, and pushed under its name it would shadow the
    // session's own completion push a moment later.
    if facts.turn_error.is_some() {
        if facts.applied_to_session {
            intents.push = Some(Push {
                kind: AttentionPushKind::Error,
                smart_suppressible: true,
            });
        }
    } else if facts.ended_turn {
        intents.push = Some(Push {
            kind: AttentionPushKind::Completion {
                is_subagent: facts.subagent_turn,
                interrupted: subject.interrupted,
            },
            smart_suppressible: true,
        });
    }
    intents
}

/// A turn that finished in a source the hook reducer never sees. One the user
/// stopped ends without a chime there too.
fn finished_turn_intents(turn_failed: bool, subject: AttentionSubject) -> AttentionIntents {
    AttentionIntents {
        sound: (!subject.interrupted).then(|| {
            if turn_failed {
                TURN_FAILED
            } else {
                sound_cue::TURN_END
            }
            .to_owned()
        }),
        completion: Some(Completion { turn_failed }),
        push: Some(Push {
            kind: if turn_failed {
                AttentionPushKind::Error
            } else {
                AttentionPushKind::Completion {
                    is_subagent: false,
                    interrupted: subject.interrupted,
                }
            },
            smart_suppressible: true,
        }),
        follow_up: follow_up(turn_failed, subject),
    }
}

/// One nudge for a finished turn nobody looked at — unless the user stopped
/// it themselves: an interrupted turn is not news to anyone.
fn follow_up(turn_failed: bool, subject: AttentionSubject) -> Option<FollowUpIntent> {
    (!subject.interrupted).then_some(FollowUpIntent::TrackCompletion { turn_failed })
}

// MARK: Settling completions (AttentionPolicy.settle)

/// The three-way completion notification style (Settings › Behavior), as
/// `CompletionStyle` in Swift.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionStyle {
    Expand,
    Glance,
    Off,
}

/// How the island shows a finished turn — `CompletionPresentation` in Swift.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionPresentation {
    /// Completion style "off": the session card updates, nothing pops.
    StatusOnly,
    /// The collapsed bar lights its cyan check.
    Glance,
    /// The completion card opens with the reply.
    Card,
}

/// The completion half of `settle(_:context:)`: what the user's settings and
/// the room allow. Swift reads the settings and the frontmost app itself;
/// here the shell supplies them. A card Smart Suppress holds back becomes a
/// glance — the turn is still tracked, but the user is already looking at it.
pub fn settle_completion(
    completion: Option<&Completion>,
    style: CompletionStyle,
    session_in_front: bool,
) -> Option<CompletionPresentation> {
    let _ = completion?;
    Some(match style {
        CompletionStyle::Off => CompletionPresentation::StatusOnly,
        CompletionStyle::Glance => CompletionPresentation::Glance,
        CompletionStyle::Expand if session_in_front => CompletionPresentation::Glance,
        CompletionStyle::Expand => CompletionPresentation::Card,
    })
}

/// The user's standing sound choices — `SoundPreferences` in Swift.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SoundPreferences {
    /// The master switch for event sounds.
    pub enabled: bool,
    /// Cues whose own switch is on and which have a sound.
    pub audible_cues: BTreeSet<String>,
}

/// The Sound page's keys, spelled as Swift `SettingsKey`s. The shell reads them
/// from the same settings document every other page writes.
pub mod sound_setting {
    /// Master switch.
    pub const ENABLED: &str = "soundEnabled";
    /// Slider percent, 0–100.
    pub const VOLUME: &str = "soundVolume";
    /// Each cue with its own switch, in `SoundManager.eventSounds` order, plus
    /// the launch chime's own switch.
    pub const CUES: &[(&str, &str)] = &[
        ("SessionStart", "soundSessionStart"),
        ("TaskRoundComplete", "soundTaskComplete"),
        ("Stop", "soundTaskComplete"),
        ("StopFailure", "soundTaskError"),
        ("PermissionRequest", "soundApprovalNeeded"),
        ("UserPromptSubmit", "soundPromptSubmit"),
        ("AppLaunch", "soundBoot"),
    ];
}

/// Turn the Sound page's settings into what [`audible_sound`] reads. An absent
/// key means "on": the shell played every cue before the Sound page had
/// switches, so an upgrade must not go silent for someone who set nothing.
pub fn sound_preferences(settings: &Value) -> SoundPreferences {
    let enabled = settings
        .get(sound_setting::ENABLED)
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let audible_cues = sound_setting::CUES
        .iter()
        .filter(|(_, key)| settings.get(*key).and_then(Value::as_bool).unwrap_or(true))
        .map(|(cue, _)| (*cue).to_owned())
        .collect();
    SoundPreferences {
        enabled,
        audible_cues,
    }
}

/// The volume slider as a percent. A missing or malformed value keeps the
/// shell's old full-level playback, not macOS's 50 % default, for the same
/// reason [`sound_preferences`] defaults to "on".
pub fn sound_volume_percent(settings: &Value) -> u8 {
    u8::try_from(
        settings
            .get(sound_setting::VOLUME)
            .and_then(Value::as_u64)
            .unwrap_or(100),
    )
    .unwrap_or(100)
    .min(100)
}

/// Why event sounds may be held back right now — `SoundHold` in Swift. The
/// shell reads the clock and the scene and supplies the results.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SoundHold {
    /// Inside the user's quiet-hours window.
    pub quiet_hours: bool,
    /// "Mute when away" is on.
    pub mute_when_away: bool,
    /// The screen is locked or the screen saver runs.
    pub scene_quiet: bool,
}

/// The Sound page's quiet-hours window — Swift `SoundManager.quietHoursActive`.
/// `now_minutes` is minutes since midnight, supplied by the shell: the core
/// owns the overnight window rule, not the clock or the time zone.
pub fn quiet_hours_active(settings: &Value, now_minutes: i64) -> bool {
    let enabled = settings
        .get("quietHoursEnabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !enabled {
        return false;
    }
    let number = |key: &str, default: i64| {
        settings
            .get(key)
            .and_then(Value::as_i64)
            .unwrap_or(default)
            .clamp(0, 24 * 60 - 1)
    };
    let start = number("quietHoursStart", 22 * 60);
    let end = number("quietHoursEnd", 8 * 60);
    if start == end {
        return false;
    }
    // A start later than the end spans midnight.
    if start < end {
        now_minutes >= start && now_minutes < end
    } else {
        now_minutes >= start || now_minutes < end
    }
}

/// The user's settings and the room's facts [`settle`] reads —
/// `AttentionContext` in Swift.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttentionContext {
    pub sounds: SoundPreferences,
    pub sound_hold: SoundHold,
    pub completion_style: CompletionStyle,
    pub push_enabled: bool,
    /// The setting is on and the session's own terminal is in front.
    pub session_in_front: bool,
}

/// Whether event sounds are held back for now — `holdsSounds(_:)` in Swift.
pub fn holds_sounds(hold: &SoundHold) -> bool {
    hold.quiet_hours || (hold.mute_when_away && hold.scene_quiet)
}

/// The sound that plays — `audibleSound(_:sounds:hold:)` in Swift. A cue with
/// no sound of its own is never audible, however the switches are set.
pub fn audible_sound(
    cue: Option<&str>,
    sounds: &SoundPreferences,
    hold: &SoundHold,
) -> Option<String> {
    let cue = cue?;
    if !sounds.enabled || !sounds.audible_cues.contains(cue) || holds_sounds(hold) {
        return None;
    }
    Some(cue.to_owned())
}

/// What a settled turn does — `AttentionDecision` in Swift.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AttentionDecision {
    pub sound: Option<String>,
    pub completion: Option<CompletionDecision>,
    pub push: Option<PushDecision>,
    pub follow_up: Option<FollowUpIntent>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompletionDecision {
    pub turn_failed: bool,
    pub presentation: CompletionPresentation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PushDecision {
    pub kind: AttentionPushKind,
    pub smart_suppressed: bool,
}

/// `AttentionPolicy.settle(_:context:)`: what the user's settings and the room
/// allow. Smart Suppress answers for a push here; a completion card asks when
/// it is about to show, at once or from the queue, so style always wins.
pub fn settle(intents: &AttentionIntents, context: &AttentionContext) -> AttentionDecision {
    let mut decision = AttentionDecision {
        sound: audible_sound(
            intents.sound.as_deref(),
            &context.sounds,
            &context.sound_hold,
        ),
        ..AttentionDecision::default()
    };
    if let Some(completion) = &intents.completion {
        decision.completion = Some(CompletionDecision {
            turn_failed: completion.turn_failed,
            presentation: match context.completion_style {
                CompletionStyle::Off => CompletionPresentation::StatusOnly,
                CompletionStyle::Glance => CompletionPresentation::Glance,
                CompletionStyle::Expand => CompletionPresentation::Card,
            },
        });
    }
    if let Some(push) = intents.push.filter(|_| context.push_enabled) {
        decision.push = Some(PushDecision {
            kind: push.kind,
            smart_suppressed: push.smart_suppressible && context.session_in_front,
        });
    }
    decision.follow_up = match intents.follow_up {
        // Follow-ups only chase completions the user asked to hear about.
        Some(FollowUpIntent::TrackCompletion { .. })
            if context.completion_style == CompletionStyle::Off =>
        {
            None
        }
        follow_up => follow_up,
    };
    decision
}

#[cfg(test)]
mod quiet_hours_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn quiet_hours_are_off_unless_the_switch_is_on() {
        assert!(!quiet_hours_active(&json!({}), 23 * 60));
        assert!(!quiet_hours_active(&json!({ "quietHoursEnabled": false }), 23 * 60));
    }

    #[test]
    fn an_overnight_window_spans_midnight() {
        let settings = json!({
            "quietHoursEnabled": true,
            "quietHoursStart": 22 * 60,
            "quietHoursEnd": 8 * 60,
        });
        assert!(quiet_hours_active(&settings, 23 * 60));
        assert!(quiet_hours_active(&settings, 2 * 60));
        assert!(!quiet_hours_active(&settings, 12 * 60));
    }

    #[test]
    fn a_same_day_window_does_not_wrap() {
        let settings = json!({
            "quietHoursEnabled": true,
            "quietHoursStart": 9 * 60,
            "quietHoursEnd": 17 * 60,
        });
        assert!(quiet_hours_active(&settings, 10 * 60));
        assert!(!quiet_hours_active(&settings, 20 * 60));
        assert!(!quiet_hours_active(&json!({
            "quietHoursEnabled": true,
            "quietHoursStart": 480,
            "quietHoursEnd": 480,
        }), 12 * 60));
    }
}
