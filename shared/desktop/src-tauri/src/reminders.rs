//! The follow-up reminders the island sends itself (#238 AC 9).
//!
//! The schedule lives in `morrow_core::follow_up`; this owns the part that is not a
//! state machine — what a reminder says, and which card it is about. It used to live in
//! the webview (`src/lib/reminders.ts`), which meant a hidden or minimised panel stopped
//! reminding, and the policy could only be tested inside a browser.
//!
//! Two wording rules, both from the macOS controller: a question and an approval open
//! differently, and the body starts with the session so a stack of notifications is
//! scannable.

use std::collections::{BTreeMap, BTreeSet};

use morrow_core::follow_up::{FollowUpKind, FollowUpReminderScheduler, Origin};
use morrow_core::reduce::Timestamp;
use serde::Serialize;

/// One `notify_send` call, paired with the card it reminds about.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Reminder {
    pub id: u64,
    pub title: String,
    pub body: String,
}

/// What the island knows about one held request, as far as a reminder needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaitingCard {
    pub id: u64,
    pub session_id: String,
    /// The question text for a question card; empty for an approval.
    pub question: String,
    /// One line of the tool input, so the body can name what is being asked about.
    pub summary: String,
    pub tool_name: String,
}

impl WaitingCard {
    fn is_question(&self) -> bool {
        !self.question.is_empty()
    }

    fn reminder(&self) -> Reminder {
        let detail = if !self.question.is_empty() {
            self.question.trim()
        } else if !self.summary.trim().is_empty() {
            self.summary.trim()
        } else {
            self.tool_name.trim()
        };
        Reminder {
            id: self.id,
            title: if self.is_question() {
                "Morrow · Question waiting".to_owned()
            } else {
                "Morrow · Approval waiting".to_owned()
            },
            body: if detail.is_empty() {
                self.session_id.clone()
            } else {
                format!("{} · {detail}", self.session_id)
            },
        }
    }
}

/// The scheduler plus the cards its reminders are about.
#[derive(Debug)]
pub struct ReminderBook {
    scheduler: FollowUpReminderScheduler,
    /// Cards by request id, so a reminder can be worded after the wait it names.
    cards: BTreeMap<u64, WaitingCard>,
}

impl Default for ReminderBook {
    fn default() -> Self {
        // Off until the setting says otherwise: a fresh install must not start
        // notifying before the user has chosen an interval.
        Self {
            scheduler: FollowUpReminderScheduler::new(None),
            cards: BTreeMap::new(),
        }
    }
}

impl ReminderBook {
    pub fn set_interval(&mut self, seconds: Option<f64>) {
        self.scheduler.set_interval(seconds);
    }

    /// Reconcile with the held requests and deliver whatever is due.
    ///
    /// `held_back` is the lock screen, quiet hours, or a Notifications capability that
    /// is not `Supported`: the schedule keeps running, and the first eligible tick
    /// catches the owed reminder up.
    pub fn tick(
        &mut self,
        cards: &[WaitingCard],
        held_back: bool,
        now: Timestamp,
    ) -> Vec<Reminder> {
        self.cards = cards.iter().map(|card| (card.id, card.clone())).collect();
        let mut approvals = BTreeMap::new();
        let mut questions = BTreeMap::new();
        for card in cards {
            // The request id is the card's: a request that is answered and replaced
            // under the same session is a new wait, and is timed from scratch.
            let request = card.id.to_string();
            if card.is_question() {
                questions.insert(card.session_id.clone(), request);
            } else {
                approvals.insert(card.session_id.clone(), request);
            }
        }
        self.scheduler
            .sync_requests(FollowUpKind::Approval, &approvals, now);
        self.scheduler
            .sync_requests(FollowUpKind::Question, &questions, now);
        self.scheduler
            .collect_due(now, held_back)
            .iter()
            .filter_map(|reminder| self.content(reminder))
            .collect()
    }

    /// Reconcile the sessions waiting somewhere the island cannot answer (#238 AC 3).
    ///
    /// The island's own queue is reconciled by [`Self::tick`]; these are the waits it
    /// only mirrors, and they are a separate origin on purpose: an entry that changes
    /// hands keeps its timing instead of starting over.
    pub fn sync_display_only(
        &mut self,
        kind: FollowUpKind,
        waiting: &BTreeSet<String>,
        held_by_island: &BTreeSet<String>,
        now: Timestamp,
    ) {
        self.scheduler
            .sync_origin_keeping(kind, Origin::DisplayOnly, waiting, held_by_island, now);
    }

    /// A finished turn the user has not looked at yet.
    pub fn track_completion(&mut self, session_id: &str, turn_failed: bool, now: Timestamp) {
        self.scheduler
            .track_with(FollowUpKind::Completion, session_id, now, turn_failed);
    }

    /// Cards whose reminder already went out: the panel keeps a bell on them until the
    /// request is answered or dropped.
    pub fn hinted(&self) -> Vec<u64> {
        self.cards
            .iter()
            .filter(|(_, card)| {
                let kind = if card.is_question() {
                    FollowUpKind::Question
                } else {
                    FollowUpKind::Approval
                };
                self.scheduler.has_reminded(kind, &card.session_id)
            })
            .map(|(id, _)| *id)
            .collect()
    }

    fn content(&self, reminder: &morrow_core::follow_up::FollowUpReminder) -> Option<Reminder> {
        match (reminder.kind, reminder.origin) {
            // The island holds the request: the reminder opens the very card it is about.
            (FollowUpKind::Approval | FollowUpKind::Question, Origin::Island) => {
                let id: u64 = reminder.request_id.as_deref()?.parse().ok()?;
                self.cards.get(&id).map(WaitingCard::reminder)
            }
            // A display-only wait has no card of ours — nothing here can answer it — so
            // the reminder names the session and the session's own card says where to go.
            (FollowUpKind::Approval | FollowUpKind::Question, Origin::DisplayOnly) => {
                Some(Reminder {
                    id: 0,
                    title: if reminder.kind == FollowUpKind::Question {
                        "Morrow · Question waiting".to_owned()
                    } else {
                        "Morrow · Approval waiting".to_owned()
                    },
                    body: reminder.session_id.clone(),
                })
            }
            // A finished turn has no card either.
            (FollowUpKind::Completion, _) => Some(Reminder {
                id: 0,
                title: if reminder.turn_failed {
                    "Morrow · Turn failed".to_owned()
                } else {
                    "Morrow · Turn finished".to_owned()
                },
                body: reminder.session_id.clone(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: f64) -> Timestamp {
        Timestamp(seconds)
    }

    fn approval(id: u64, session: &str) -> WaitingCard {
        WaitingCard {
            id,
            session_id: session.to_owned(),
            question: String::new(),
            summary: "rm -rf build".to_owned(),
            tool_name: "Bash".to_owned(),
        }
    }

    fn question(id: u64, session: &str) -> WaitingCard {
        WaitingCard {
            id,
            session_id: session.to_owned(),
            question: "Which database?".to_owned(),
            summary: String::new(),
            tool_name: "AskUserQuestion".to_owned(),
        }
    }

    fn book() -> ReminderBook {
        let mut book = ReminderBook::default();
        book.set_interval(Some(60.0));
        book
    }

    #[test]
    fn nothing_is_sent_until_the_interval_says_so() {
        let mut book = book();
        assert!(book.tick(&[approval(1, "s1")], false, at(0.0)).is_empty());

        let due = book.tick(&[approval(1, "s1")], false, at(60.0));
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].id, 1);
        assert_eq!(due[0].title, "Morrow · Approval waiting");
        assert_eq!(due[0].body, "s1 · rm -rf build");
        assert_eq!(book.hinted(), vec![1]);
    }

    #[test]
    fn a_question_reads_differently_from_an_approval() {
        let mut book = book();
        book.tick(&[question(7, "s2")], false, at(0.0));
        let due = book.tick(&[question(7, "s2")], false, at(60.0));
        assert_eq!(due[0].title, "Morrow · Question waiting");
        assert_eq!(due[0].body, "s2 · Which database?");
    }

    #[test]
    fn an_answered_request_loses_its_bell() {
        let mut book = book();
        book.tick(&[approval(1, "s1")], false, at(0.0));
        book.tick(&[approval(1, "s1")], false, at(60.0));
        assert_eq!(book.hinted(), vec![1]);
        // Answered: the card is gone, so nothing is hinted and nothing is due.
        assert!(book.tick(&[], false, at(90.0)).is_empty());
        assert!(book.hinted().is_empty());
        assert!(book.tick(&[], false, at(1_000.0)).is_empty());
    }

    #[test]
    fn a_replaced_request_is_timed_from_scratch() {
        let mut book = book();
        book.tick(&[approval(1, "s1")], false, at(0.0));
        book.tick(&[approval(1, "s1")], false, at(60.0));
        // Same session, a new request: attempt one again, an interval from now.
        assert!(book.tick(&[approval(2, "s1")], false, at(70.0)).is_empty());
        let due = book.tick(&[approval(2, "s1")], false, at(130.0));
        assert_eq!(due[0].id, 2);
    }

    #[test]
    fn a_held_back_reminder_comes_once_when_the_hold_ends() {
        let mut book = book();
        book.tick(&[approval(1, "s1")], true, at(0.0));
        book.tick(&[approval(1, "s1")], true, at(60.0));
        assert!(book.tick(&[approval(1, "s1")], true, at(200.0)).is_empty());
        let catch_up = book.tick(&[approval(1, "s1")], false, at(300.0));
        assert_eq!(
            catch_up.len(),
            1,
            "one catch-up, not one per missed attempt"
        );
        assert_eq!(catch_up[0].id, 1);
    }

    /// #238 AC 3: a wait the island only mirrors still reminds, without a card to open.
    #[test]
    fn a_display_only_wait_reminds_without_a_card() {
        let mut book = book();
        let waiting = BTreeSet::from(["s7".to_owned()]);
        book.sync_display_only(FollowUpKind::Approval, &waiting, &BTreeSet::new(), at(0.0));
        assert!(book.tick(&[], false, at(30.0)).is_empty());
        let due = book.tick(&[], false, at(60.0));
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].title, "Morrow · Approval waiting");
        assert_eq!(due[0].body, "s7");
        assert_eq!(due[0].id, 0, "no card to open");

        // Each kind reconciles on its own, and a wait that ends takes its reminder with it.
        book.sync_display_only(
            FollowUpKind::Approval,
            &BTreeSet::new(),
            &BTreeSet::new(),
            at(70.0),
        );
        assert!(book.tick(&[], false, at(600.0)).is_empty());

        let question = BTreeSet::from(["s8".to_owned()]);
        book.sync_display_only(
            FollowUpKind::Question,
            &question,
            &BTreeSet::new(),
            at(600.0),
        );
        let due = book.tick(&[], false, at(660.0));
        assert_eq!(due[0].title, "Morrow · Question waiting");
    }

    #[test]
    fn a_wait_handed_to_the_island_keeps_its_clock() {
        let mut book = book();
        let display = BTreeSet::from(["s7".to_owned()]);
        book.sync_display_only(FollowUpKind::Approval, &display, &BTreeSet::new(), at(0.0));
        // The island takes the request at t=30. The display-only mirror goes
        // empty, but the entry must survive for tick's island sync to adopt it.
        let island = BTreeSet::from(["s7".to_owned()]);
        book.sync_display_only(FollowUpKind::Approval, &BTreeSet::new(), &island, at(30.0));
        let due = book.tick(&[approval(1, "s7")], false, at(60.0));
        assert_eq!(
            due.len(),
            1,
            "the original 60s clock survived the handover at t=30"
        );
        assert_eq!(due[0].id, 1, "now a card-backed island reminder");
    }

    #[test]
    fn a_finished_turn_names_the_session_instead_of_a_card() {
        let mut book = book();
        book.track_completion("s9", false, at(0.0));
        let due = book.tick(&[], false, at(60.0));
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].title, "Morrow · Turn finished");
        assert_eq!(due[0].body, "s9");

        let mut book = self::book();
        book.track_completion("s9", true, at(0.0));
        assert_eq!(
            book.tick(&[], false, at(60.0))[0].title,
            "Morrow · Turn failed"
        );
    }

    #[test]
    fn the_feature_is_off_until_an_interval_is_chosen() {
        let mut book = ReminderBook::default();
        assert!(
            book.tick(&[approval(1, "s1")], false, at(10_000.0))
                .is_empty()
        );
        assert!(
            book.tick(&[approval(1, "s1")], false, at(20_000.0))
                .is_empty()
        );
    }

    #[test]
    fn an_answered_request_stops_being_reminded() {
        let mut book = book();
        book.tick(&[approval(1, "s1")], false, at(0.0));
        book.tick(&[approval(1, "s1")], false, at(60.0));
        // Answered and a different card takes its place: a new wait, timed afresh.
        assert!(book.tick(&[approval(2, "s1")], false, at(70.0)).is_empty());
        assert_eq!(book.tick(&[approval(2, "s1")], false, at(130.0)).len(), 1);
    }

    #[test]
    fn reminders_are_ordered_approvals_first() {
        let mut book = book();
        book.tick(&[question(2, "q"), approval(1, "a")], false, at(0.0));
        let due = book.tick(&[question(2, "q"), approval(1, "a")], false, at(60.0));
        let ids: Vec<u64> = due.iter().map(|reminder| reminder.id).collect();
        assert_eq!(ids, vec![1, 2]);
    }
}
