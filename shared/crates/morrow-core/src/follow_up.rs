//! The follow-up reminder schedule: one nudge after another for an approval, a question
//! or a finished turn nobody looked at
//! (`macos/Sources/MorrowCore/FollowUpReminderScheduler.swift`).
//!
//! No timers and no clock. Every mutation takes `now`, and the owner arms a single
//! wake-up for [`FollowUpReminderScheduler::next_wake`] — so with the feature off, or
//! nothing waiting, there is no state and nothing scheduled.
//!
//! The port's frontend used to own this (`src/lib/reminders.ts`), which meant the
//! schedule lived in the webview: a hidden panel stopped reminding, and the policy
//! could not be tested on an injected clock without a browser. #238 AC 9 moves it here.
//!
//! Two flavours of item, and the difference is not cosmetic:
//!
//! - **Synced** (approvals, questions) — the owner reports the whole set that is waiting
//!   and entries appear and disappear with it. A silenced or exhausted entry stays until
//!   its item stops waiting, so a later sync cannot restart the reminders for the very
//!   same request.
//! - **Tracked** (completions) — started by an event; a newer event for the same session
//!   restarts it, and it is dropped once done.
//!
//! Holding back (locked screen, quiet hours) does not pause the clock: an item that comes
//! due while held is marked owed and delivered as one catch-up when the hold ends. Missed
//! attempts collapse into that single delivery rather than being spent silently.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::reduce::Timestamp;

/// The interval the panel has always used: one minute, the shortest enabled choice of
/// macOS's `intervalChoices` [0, 1, 2, 3, 5].
pub const DEFAULT_INTERVAL_SECONDS: f64 = 60.0;

/// What a follow-up reminder is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FollowUpKind {
    Approval,
    Question,
    Completion,
}

impl FollowUpKind {
    /// Approvals and questions block the agent, so they are worth nagging about a few
    /// times; a finished turn blocks nothing and gets one nudge.
    pub fn max_attempts(self) -> u32 {
        match self {
            Self::Approval | Self::Question => 3,
            Self::Completion => 1,
        }
    }

    /// Approvals → questions → completions, so the caller can take the head as "the
    /// most urgent".
    fn rank(self) -> u8 {
        match self {
            Self::Approval => 0,
            Self::Question => 1,
            Self::Completion => 2,
        }
    }
}

/// Where the waiting item lives, and so what a reminder can do about it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Origin {
    /// The island holds it: its card can be shown again.
    Island,
    /// A display-only wait (Cowork, Cursor's in-IDE question, a terminal prompt
    /// announced by a Notification hook). It can only be answered in the app that asked,
    /// so a reminder chimes and hints but has no card to open.
    DisplayOnly,
}

/// How a reminder reached the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Delivery {
    /// Delivered at its due time.
    OnTime,
    /// Came due while reminders were held back. Nothing played; a [`Delivery::CatchUp`]
    /// follows if the item is still waiting when the hold ends.
    Deferred,
    /// The held-back reminder, delivered as soon as the hold ended.
    CatchUp,
}

/// One follow-up reminder, handed to whoever reacts to it.
#[derive(Clone, Debug, PartialEq)]
pub struct FollowUpReminder {
    pub kind: FollowUpKind,
    pub session_id: String,
    /// 1-based. For [`Delivery::Deferred`] it is the attempt that is owed.
    pub attempt: u32,
    pub max_attempts: u32,
    /// When the item started waiting (request arrival / turn completion).
    pub waiting_since: Timestamp,
    pub delivery: Delivery,
    pub origin: Origin,
    /// The queued request this reminder is about, when the island holds one; `None` for
    /// display-only waits and finished turns.
    pub request_id: Option<String>,
    /// A reminder the island kept to itself because the user seemed to be in front of
    /// the item. Nothing played locally; a channel to somewhere else still gets it.
    pub locally_suppressed: bool,
    /// A completion whose turn ended on an error, so no channel may present it as work
    /// that finished.
    pub turn_failed: bool,
}

impl FollowUpReminder {
    /// No further reminder will follow for this item.
    pub fn is_final(&self) -> bool {
        self.delivery != Delivery::Deferred && self.attempt >= self.max_attempts
    }

    /// The same reminder, marked as kept off this machine.
    pub fn suppressed_locally(&self) -> Self {
        Self {
            locally_suppressed: true,
            ..self.clone()
        }
    }
}

/// One remembered item.
#[derive(Clone, Debug, PartialEq)]
struct Entry {
    waiting_since: Timestamp,
    /// Last delivery, or `waiting_since` before the first one: due times are anchored
    /// here so changing the interval re-times everything pending.
    anchor: Timestamp,
    delivered: u32,
    /// Came due while held back, waiting for the hold to end.
    owed: bool,
    /// Out of attempts, or silenced. Kept for synced entries so the same waiting item is
    /// not tracked from scratch again.
    done: bool,
    /// Created by `sync` rather than `track`.
    synced: bool,
    origin: Origin,
    request_id: Option<String>,
    turn_failed: bool,
}

impl Entry {
    fn new(now: Timestamp, synced: bool, origin: Origin) -> Self {
        Self {
            waiting_since: now,
            anchor: now,
            delivered: 0,
            owed: false,
            done: false,
            synced,
            origin,
            request_id: None,
            turn_failed: false,
        }
    }
}

/// Which item an entry belongs to.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key {
    pub kind: FollowUpKind,
    pub session_id: String,
}

impl Key {
    pub fn new(kind: FollowUpKind, session_id: impl Into<String>) -> Self {
        Self {
            kind,
            session_id: session_id.into(),
        }
    }
}

/// The timing state machine. See the module docs.
#[derive(Clone, Debug, Default)]
pub struct FollowUpReminderScheduler {
    /// Seconds between reminders; `None` means the feature is off.
    interval: Option<f64>,
    entries: BTreeMap<Key, Entry>,
}

impl FollowUpReminderScheduler {
    pub fn new(interval: Option<f64>) -> Self {
        Self {
            interval: normalize(interval),
            entries: BTreeMap::new(),
        }
    }

    pub fn interval(&self) -> Option<f64> {
        self.interval
    }

    pub fn is_enabled(&self) -> bool {
        self.interval.is_some()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Items that can still produce a reminder.
    pub fn live_keys(&self) -> BTreeSet<Key> {
        self.entries
            .iter()
            .filter(|(_, entry)| !entry.done)
            .map(|(key, _)| key.clone())
            .collect()
    }

    /// Every remembered item, done or not.
    pub fn tracked_keys(&self) -> BTreeSet<Key> {
        self.entries.keys().cloned().collect()
    }

    /// An item came due while held back and is still waiting for the hold to end.
    pub fn has_owed(&self) -> bool {
        self.entries.values().any(|entry| entry.owed && !entry.done)
    }

    /// Whether a reminder for this item has already gone out: the panel keeps its bell
    /// until the request is answered or dropped.
    pub fn has_reminded(&self, kind: FollowUpKind, session_id: &str) -> bool {
        self.entries
            .get(&Key::new(kind, session_id))
            .is_some_and(|entry| entry.delivered > 0)
    }

    pub fn origin_of(&self, key: &Key) -> Option<Origin> {
        self.entries.get(key).map(|entry| entry.origin)
    }

    /// Turning the feature off drops everything; changing the interval re-times pending
    /// entries from their last delivery.
    pub fn set_interval(&mut self, interval: Option<f64>) {
        self.interval = normalize(interval);
        if self.interval.is_none() {
            self.entries.clear();
        }
    }

    /// Reconcile one synced kind against the sessions whose request the island holds,
    /// without naming the requests.
    pub fn sync_waiting(&mut self, kind: FollowUpKind, waiting: &BTreeSet<String>, now: Timestamp) {
        self.sync_origin(kind, Origin::Island, waiting, now);
    }

    /// Reconcile one synced kind against the island's queue: each waiting session and the
    /// request its card shows. A session whose request changed — the one it was reminded
    /// about was answered and the next took its place without the session ever leaving
    /// the queue — is a new wait: fresh timing, no attempts spent, not silenced.
    pub fn sync_requests(
        &mut self,
        kind: FollowUpKind,
        requests: &BTreeMap<String, String>,
        now: Timestamp,
    ) {
        let waiting: BTreeMap<String, Option<String>> = requests
            .iter()
            .map(|(session, request)| (session.clone(), Some(request.clone())))
            .collect();
        self.reconcile(kind, Origin::Island, &waiting, now);
    }

    /// Reconcile the entries of one kind and origin. Entries of the other origin are left
    /// alone: the island's queues are re-read on every change, a display-only wait only
    /// once a session's status has settled.
    ///
    /// A listed session that already has an entry of the other origin is the same wait
    /// changing hands (a terminal prompt the island then queued): the entry is adopted
    /// with its timing, attempts and silence.
    pub fn sync_origin(
        &mut self,
        kind: FollowUpKind,
        origin: Origin,
        waiting: &BTreeSet<String>,
        now: Timestamp,
    ) {
        let waiting: BTreeMap<String, Option<String>> = waiting
            .iter()
            .map(|session| (session.clone(), None))
            .collect();
        self.reconcile(kind, origin, &waiting, now);
    }

    fn reconcile(
        &mut self,
        kind: FollowUpKind,
        origin: Origin,
        waiting: &BTreeMap<String, Option<String>>,
        now: Timestamp,
    ) {
        if !self.is_enabled() {
            return;
        }
        self.entries.retain(|key, entry| {
            key.kind != kind || entry.origin != origin || waiting.contains_key(&key.session_id)
        });
        for (session_id, request_id) in waiting {
            let key = Key::new(kind, session_id.clone());
            match self.entries.get_mut(&key) {
                // A named request that differs from the one we remember is a new wait;
                // a wait handing over to/from an unnamed one is the same wait.
                Some(entry)
                    if !is_new_request(request_id.as_deref(), entry.request_id.as_deref()) =>
                {
                    entry.origin = origin;
                    if request_id.is_some() {
                        entry.request_id = request_id.clone();
                    }
                }
                _ => {
                    let mut entry = Entry::new(now, true, origin);
                    entry.request_id = request_id.clone();
                    self.entries.insert(key, entry);
                }
            }
        }
    }

    /// A new wait began for an item that may still be tracked from an earlier one whose
    /// end was never seen (the session left the wait and came back between two syncs).
    pub fn restart(
        &mut self,
        kind: FollowUpKind,
        session_id: &str,
        origin: Origin,
        now: Timestamp,
    ) {
        if !self.is_enabled() {
            return;
        }
        self.entries
            .insert(Key::new(kind, session_id), Entry::new(now, true, origin));
    }

    /// Start (or restart) an event-driven item.
    pub fn track(&mut self, kind: FollowUpKind, session_id: &str, now: Timestamp) {
        self.track_with(kind, session_id, now, false);
    }

    /// As [`Self::track`], with the turn that ended having failed.
    pub fn track_with(
        &mut self,
        kind: FollowUpKind,
        session_id: &str,
        now: Timestamp,
        turn_failed: bool,
    ) {
        if !self.is_enabled() {
            return;
        }
        let mut entry = Entry::new(now, false, Origin::Island);
        entry.turn_failed = turn_failed;
        self.entries.insert(Key::new(kind, session_id), entry);
    }

    /// Hand back a reminder `collect_due` just produced that reached nobody: the user was
    /// already looking at the item. The attempt is not spent and the item is not silenced
    /// — being in front of it once says nothing about the next time.
    ///
    /// Ignored when the entry has since moved on to another wait (answered, restarted, a
    /// newer request) or been silenced.
    pub fn postpone(&mut self, reminder: &FollowUpReminder, now: Timestamp) {
        if !self.is_enabled() {
            return;
        }
        let key = Key::new(reminder.kind, reminder.session_id.clone());
        let Some(entry) = self.entries.get_mut(&key) else {
            return;
        };
        if entry.waiting_since != reminder.waiting_since
            || entry.request_id != reminder.request_id
            || entry.delivered != reminder.attempt
        {
            return;
        }
        entry.delivered = reminder.attempt.saturating_sub(1);
        entry.owed = false;
        entry.done = false;
        entry.anchor = now;
    }

    /// Stop reminding about an item (seen, jumped to, handled elsewhere).
    pub fn silence(&mut self, key: &Key) {
        match self.entries.get_mut(key) {
            Some(entry) if entry.synced => {
                entry.done = true;
                entry.owed = false;
            }
            Some(_) => {
                self.entries.remove(key);
            }
            None => {}
        }
    }

    pub fn silence_session(&mut self, session_id: &str) {
        let keys: Vec<Key> = self
            .entries
            .keys()
            .filter(|key| key.session_id == session_id)
            .cloned()
            .collect();
        for key in keys {
            self.silence(&key);
        }
    }

    pub fn silence_kind(&mut self, kind: FollowUpKind) {
        let keys: Vec<Key> = self
            .entries
            .keys()
            .filter(|key| key.kind == kind)
            .cloned()
            .collect();
        for key in keys {
            self.silence(&key);
        }
    }

    pub fn reset(&mut self) {
        self.entries.clear();
    }

    /// Earliest moment a live, not-yet-owed entry comes due. Owed entries wait on the
    /// hold to end, not on the clock.
    pub fn next_wake(&self) -> Option<Timestamp> {
        let interval = self.interval?;
        self.entries
            .values()
            .filter(|entry| !entry.done && !entry.owed)
            .map(|entry| Timestamp(entry.anchor.0 + interval))
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }

    /// Advance to `now`. While `held_back`, due items become owed (reported once each as
    /// [`Delivery::Deferred`]); otherwise due and owed items are delivered and counted.
    ///
    /// The result is ordered approvals → questions → completions, then oldest first, so
    /// the caller can take the head as the most urgent.
    pub fn collect_due(&mut self, now: Timestamp, held_back: bool) -> Vec<FollowUpReminder> {
        let Some(interval) = self.interval else {
            return Vec::new();
        };
        // A tracked item's last reminder is kept until the next pass so the owner can
        // still postpone it; nothing else reads a done entry.
        self.entries.retain(|_, entry| entry.synced || !entry.done);

        let keys: Vec<Key> = self.entries.keys().cloned().collect();
        let mut out = Vec::new();
        for key in keys {
            let Some(entry) = self.entries.get(&key).cloned() else {
                continue;
            };
            if entry.done {
                continue;
            }
            let due = now.0 >= entry.anchor.0 + interval;
            if !entry.owed && !due {
                continue;
            }
            let attempt = entry.delivered + 1;
            let max_attempts = key.kind.max_attempts();
            if held_back {
                if entry.owed {
                    continue; // already reported
                }
                if let Some(entry) = self.entries.get_mut(&key) {
                    entry.owed = true;
                }
                out.push(FollowUpReminder {
                    kind: key.kind,
                    session_id: key.session_id.clone(),
                    attempt,
                    max_attempts,
                    waiting_since: entry.waiting_since,
                    delivery: Delivery::Deferred,
                    origin: entry.origin,
                    request_id: entry.request_id.clone(),
                    locally_suppressed: false,
                    turn_failed: entry.turn_failed,
                });
                continue;
            }
            out.push(FollowUpReminder {
                kind: key.kind,
                session_id: key.session_id.clone(),
                attempt,
                max_attempts,
                waiting_since: entry.waiting_since,
                delivery: if entry.owed {
                    Delivery::CatchUp
                } else {
                    Delivery::OnTime
                },
                origin: entry.origin,
                request_id: entry.request_id.clone(),
                locally_suppressed: false,
                turn_failed: entry.turn_failed,
            });
            if let Some(entry) = self.entries.get_mut(&key) {
                entry.delivered = attempt;
                entry.owed = false;
                if attempt >= max_attempts {
                    entry.done = true;
                } else {
                    entry.anchor = now;
                }
            }
        }
        out.sort_by(urgency);
        out
    }
}

/// Two named requests that differ are two waits. A wait without a name handing over to a
/// named one — or back — is the same wait changing hands.
fn is_new_request(request_id: Option<&str>, current: Option<&str>) -> bool {
    match (request_id, current) {
        (Some(new), Some(old)) => new != old,
        _ => false,
    }
}

fn urgency(a: &FollowUpReminder, b: &FollowUpReminder) -> std::cmp::Ordering {
    a.kind
        .rank()
        .cmp(&b.kind.rank())
        .then_with(|| a.waiting_since.0.total_cmp(&b.waiting_since.0))
        .then_with(|| a.session_id.cmp(&b.session_id))
}

fn normalize(interval: Option<f64>) -> Option<f64> {
    interval.filter(|interval| *interval > 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: f64) -> Timestamp {
        Timestamp(seconds)
    }

    fn waiting(sessions: &[&str]) -> BTreeSet<String> {
        sessions.iter().map(|s| (*s).to_owned()).collect()
    }

    fn one_approval() -> FollowUpReminderScheduler {
        let mut scheduler = FollowUpReminderScheduler::new(Some(60.0));
        scheduler.sync_waiting(FollowUpKind::Approval, &waiting(&["s1"]), at(0.0));
        scheduler
    }

    #[test]
    fn nothing_is_scheduled_while_the_feature_is_off() {
        let mut off = FollowUpReminderScheduler::new(None);
        off.sync_waiting(FollowUpKind::Approval, &waiting(&["s1"]), at(0.0));
        off.track(FollowUpKind::Completion, "s1", at(0.0));
        assert!(off.is_empty());
        assert!(off.next_wake().is_none());
        assert!(off.collect_due(at(10_000.0), false).is_empty());

        // A zero or negative interval is off, not "every instant".
        assert!(!FollowUpReminderScheduler::new(Some(0.0)).is_enabled());
        assert!(!FollowUpReminderScheduler::new(Some(-5.0)).is_enabled());
    }

    #[test]
    fn a_reminder_fires_at_the_interval_and_not_before() {
        let mut scheduler = one_approval();
        assert_eq!(scheduler.next_wake(), Some(at(60.0)));
        assert!(scheduler.collect_due(at(59.9), false).is_empty());

        let due = scheduler.collect_due(at(60.0), false);
        assert_eq!(due.len(), 1);
        let reminder = &due[0];
        assert_eq!(reminder.kind, FollowUpKind::Approval);
        assert_eq!(reminder.attempt, 1);
        assert_eq!(reminder.max_attempts, 3);
        assert_eq!(reminder.delivery, Delivery::OnTime);
        assert!(!reminder.is_final());
        // The next one is an interval after the delivery, not after the wait.
        assert_eq!(scheduler.next_wake(), Some(at(120.0)));
    }

    #[test]
    fn an_approval_is_nudged_three_times_and_then_left_alone() {
        let mut scheduler = one_approval();
        for (index, at_seconds) in [60.0, 120.0, 180.0].into_iter().enumerate() {
            let due = scheduler.collect_due(at(at_seconds), false);
            assert_eq!(due.len(), 1, "attempt {}", index + 1);
            assert_eq!(due[0].attempt, index as u32 + 1);
        }
        assert!(scheduler.collect_due(at(240.0), false).is_empty());
        assert_eq!(scheduler.live_keys().len(), 0);
        // The exhausted entry is kept: a later sync must not restart this wait.
        assert_eq!(scheduler.tracked_keys().len(), 1);
    }

    #[test]
    fn a_finished_turn_gets_one_nudge() {
        let mut scheduler = FollowUpReminderScheduler::new(Some(60.0));
        scheduler.track(FollowUpKind::Completion, "s1", at(0.0));
        let due = scheduler.collect_due(at(60.0), false);
        assert_eq!(due.len(), 1);
        assert!(due[0].is_final());
        assert!(scheduler.collect_due(at(120.0), false).is_empty());
        // A tracked entry that is done is dropped on the next pass, not kept forever.
        assert!(scheduler.is_empty());
    }

    #[test]
    fn an_answered_request_takes_its_reminder_with_it() {
        let mut scheduler = one_approval();
        scheduler.sync_waiting(FollowUpKind::Approval, &BTreeSet::new(), at(30.0));
        assert!(scheduler.is_empty());
        assert!(scheduler.next_wake().is_none());
    }

    #[test]
    fn a_new_request_for_the_same_session_starts_over() {
        let mut scheduler = FollowUpReminderScheduler::new(Some(60.0));
        scheduler.sync_requests(
            FollowUpKind::Approval,
            &BTreeMap::from([("s1".to_owned(), "req-1".to_owned())]),
            at(0.0),
        );
        assert_eq!(scheduler.collect_due(at(60.0), false)[0].attempt, 1);

        // Answered and replaced inside one sync: fresh timing, no attempts spent.
        scheduler.sync_requests(
            FollowUpKind::Approval,
            &BTreeMap::from([("s1".to_owned(), "req-2".to_owned())]),
            at(61.0),
        );
        assert!(scheduler.collect_due(at(100.0), false).is_empty());
        let due = scheduler.collect_due(at(121.0), false);
        assert_eq!(due[0].attempt, 1);
        assert_eq!(due[0].request_id.as_deref(), Some("req-2"));
    }

    #[test]
    fn an_unnamed_wait_handing_over_to_a_named_one_is_the_same_wait() {
        let mut scheduler = FollowUpReminderScheduler::new(Some(60.0));
        // A terminal prompt announced by a Notification hook: display-only, no request.
        scheduler.sync_origin(
            FollowUpKind::Approval,
            Origin::DisplayOnly,
            &waiting(&["s1"]),
            at(0.0),
        );
        assert_eq!(scheduler.collect_due(at(60.0), false)[0].attempt, 1);

        // The island queues it: same wait, so no second first attempt.
        scheduler.sync_requests(
            FollowUpKind::Approval,
            &BTreeMap::from([("s1".to_owned(), "req-1".to_owned())]),
            at(60.0),
        );
        assert_eq!(
            scheduler.origin_of(&Key::new(FollowUpKind::Approval, "s1")),
            Some(Origin::Island)
        );
        assert!(scheduler.collect_due(at(61.0), false).is_empty());
        let due = scheduler.collect_due(at(120.0), false);
        assert_eq!(due[0].attempt, 2);
        assert_eq!(due[0].request_id.as_deref(), Some("req-1"));
    }

    #[test]
    fn the_other_origin_is_left_alone_by_a_sync() {
        let mut scheduler = FollowUpReminderScheduler::new(Some(60.0));
        scheduler.sync_origin(
            FollowUpKind::Question,
            Origin::DisplayOnly,
            &waiting(&["s1"]),
            at(0.0),
        );
        // The island's queue for the same kind is empty and says nothing about the
        // display-only wait.
        scheduler.sync_waiting(FollowUpKind::Question, &BTreeSet::new(), at(1.0));
        assert_eq!(scheduler.live_keys().len(), 1);
        assert_eq!(
            scheduler.origin_of(&Key::new(FollowUpKind::Question, "s1")),
            Some(Origin::DisplayOnly)
        );
    }

    #[test]
    fn a_silenced_entry_is_not_restarted_by_the_next_sync() {
        let mut scheduler = FollowUpReminderScheduler::new(Some(60.0));
        scheduler.sync_requests(
            FollowUpKind::Approval,
            &BTreeMap::from([("s1".to_owned(), "req-1".to_owned())]),
            at(0.0),
        );
        scheduler.silence(&Key::new(FollowUpKind::Approval, "s1"));
        assert!(scheduler.collect_due(at(600.0), false).is_empty());
        scheduler.sync_requests(
            FollowUpKind::Approval,
            &BTreeMap::from([("s1".to_owned(), "req-1".to_owned())]),
            at(600.0),
        );
        assert!(scheduler.collect_due(at(1_200.0), false).is_empty());

        // A different request is a different wait, and is reminded again.
        scheduler.sync_requests(
            FollowUpKind::Approval,
            &BTreeMap::from([("s1".to_owned(), "req-2".to_owned())]),
            at(1_200.0),
        );
        assert_eq!(scheduler.collect_due(at(1_260.0), false).len(), 1);
    }

    #[test]
    fn silencing_a_tracked_item_drops_it_but_a_synced_one_stays_quiet() {
        let mut scheduler = FollowUpReminderScheduler::new(Some(60.0));
        scheduler.track(FollowUpKind::Completion, "session-a", at(0.0));
        scheduler.silence(&Key::new(FollowUpKind::Completion, "session-a"));
        assert!(scheduler.is_empty());

        scheduler.sync_waiting(FollowUpKind::Question, &waiting(&["a", "b"]), at(0.0));
        scheduler.silence_session("a");
        assert_eq!(scheduler.tracked_keys().len(), 2);
        assert_eq!(scheduler.live_keys().len(), 1);

        scheduler.silence_kind(FollowUpKind::Question);
        assert_eq!(scheduler.live_keys().len(), 0);
    }

    #[test]
    fn a_held_back_reminder_is_owed_once_and_caught_up_once() {
        let mut scheduler = one_approval();

        // Locked at the due moment: reported once as deferred, then quiet.
        let deferred = scheduler.collect_due(at(60.0), true);
        assert_eq!(deferred.len(), 1);
        assert_eq!(deferred[0].delivery, Delivery::Deferred);
        assert_eq!(deferred[0].attempt, 1);
        assert!(scheduler.collect_due(at(90.0), true).is_empty());
        assert!(scheduler.has_owed());
        // Owed entries wait on the hold, not on the clock.
        assert!(scheduler.next_wake().is_none());

        // The hold ends: one catch-up, and the clock restarts from here.
        let catch_up = scheduler.collect_due(at(300.0), false);
        assert_eq!(catch_up.len(), 1);
        assert_eq!(catch_up[0].delivery, Delivery::CatchUp);
        // A held-back reminder was never delivered, so `delivered` never moved: the
        // catch-up is the attempt that was owed, not the one after it.
        assert_eq!(catch_up[0].attempt, 1);
        assert!(!scheduler.has_owed());
        assert_eq!(scheduler.next_wake(), Some(at(360.0)));
    }

    #[test]
    fn missed_attempts_collapse_into_the_catch_up() {
        let mut scheduler = one_approval();
        scheduler.collect_due(at(60.0), true);
        // Held for ten minutes: three attempts would have been due.
        let catch_up = scheduler.collect_due(at(600.0), false);
        assert_eq!(catch_up.len(), 1, "one delivery, not three silent ones");
        assert_eq!(catch_up[0].attempt, 1);
        // The clock restarts from the catch-up: one more is still owed at 660.
        assert_eq!(scheduler.collect_due(at(659.0), false).len(), 0);
        assert_eq!(scheduler.collect_due(at(660.0), false)[0].attempt, 2);
    }

    #[test]
    fn postponing_does_not_spend_an_attempt() {
        let mut scheduler = one_approval();
        let due = scheduler.collect_due(at(60.0), false);
        assert_eq!(due[0].attempt, 1);
        assert_eq!(scheduler.next_wake(), Some(at(120.0)));

        scheduler.postpone(&due[0], at(60.0));
        assert!(scheduler.collect_due(at(119.0), false).is_empty());
        let again = scheduler.collect_due(at(120.0), false);
        assert_eq!(again[0].attempt, 1, "nothing was spent");
    }

    #[test]
    fn postponing_a_reminder_that_moved_on_is_ignored() {
        let mut scheduler = one_approval();
        let due = scheduler.collect_due(at(60.0), false);
        // Answered before the island decided the user was looking at it.
        scheduler.sync_waiting(FollowUpKind::Approval, &BTreeSet::new(), at(61.0));
        scheduler.postpone(&due[0], at(61.0));
        assert!(scheduler.is_empty());
    }

    #[test]
    fn turning_the_feature_off_drops_everything_and_back_on_starts_empty() {
        let mut scheduler = one_approval();
        scheduler.set_interval(None);
        assert!(scheduler.is_empty());
        assert_eq!(scheduler.next_wake(), None);
        scheduler.set_interval(Some(30.0));
        assert!(scheduler.is_empty(), "nothing is remembered across off");
        assert_eq!(scheduler.interval(), Some(30.0));
    }

    #[test]
    fn changing_the_interval_retimes_from_the_last_delivery() {
        let mut scheduler = FollowUpReminderScheduler::new(Some(60.0));
        scheduler.sync_waiting(FollowUpKind::Question, &waiting(&["s1"]), at(0.0));
        assert_eq!(scheduler.collect_due(at(60.0), false).len(), 1);
        scheduler.set_interval(Some(10.0));
        assert_eq!(scheduler.next_wake(), Some(at(70.0)));
        assert_eq!(scheduler.collect_due(at(70.0), false).len(), 1);
    }

    #[test]
    fn the_order_is_approvals_then_questions_then_completions_then_oldest() {
        let mut scheduler = FollowUpReminderScheduler::new(Some(60.0));
        scheduler.track(FollowUpKind::Completion, "oldest", at(0.0));
        scheduler.sync_waiting(FollowUpKind::Question, &waiting(&["q-new"]), at(30.0));
        scheduler.sync_waiting(FollowUpKind::Approval, &waiting(&["a-new"]), at(40.0));
        scheduler.track(FollowUpKind::Completion, "newest", at(50.0));

        // Every entry is due by 110: a-new at 100, q-new at 90, oldest at 60, newest at 110.
        let due = scheduler.collect_due(at(110.0), false);
        let order: Vec<(&str, FollowUpKind)> = due
            .iter()
            .map(|reminder| (reminder.session_id.as_str(), reminder.kind))
            .collect();
        assert_eq!(
            order,
            vec![
                ("a-new", FollowUpKind::Approval),
                ("q-new", FollowUpKind::Question),
                ("oldest", FollowUpKind::Completion),
                ("newest", FollowUpKind::Completion),
            ]
        );
    }

    #[test]
    fn a_failed_turn_says_so_in_every_reminder_it_produces() {
        let mut scheduler = FollowUpReminderScheduler::new(Some(60.0));
        scheduler.track_with(FollowUpKind::Completion, "s1", at(0.0), true);
        let due = scheduler.collect_due(at(60.0), false);
        assert!(due[0].turn_failed);
        // A newer turn that succeeded replaces the flag.
        scheduler.track(FollowUpKind::Completion, "s1", at(100.0));
        assert!(!scheduler.collect_due(at(160.0), false)[0].turn_failed);
    }

    #[test]
    fn a_reminder_kept_off_this_machine_still_exists_for_the_others() {
        let mut scheduler = one_approval();
        let due = scheduler.collect_due(at(60.0), false);
        let quiet = due[0].suppressed_locally();
        assert!(quiet.locally_suppressed);
        assert_eq!(quiet.attempt, due[0].attempt);
        assert!(!due[0].locally_suppressed);
    }

    #[test]
    fn only_the_last_attempt_after_the_last_is_final() {
        let mut scheduler = one_approval();
        scheduler.collect_due(at(60.0), true);
        let deferred = scheduler.collect_due(at(90.0), true);
        assert!(deferred.is_empty());
        // A deferred reminder is never final, even at the last attempt.
        let mut scheduler = FollowUpReminderScheduler::new(Some(60.0));
        scheduler.track(FollowUpKind::Completion, "s1", at(0.0));
        let owed = scheduler.collect_due(at(60.0), true);
        assert!(
            !owed[0].is_final(),
            "a completion owed is not a completion done"
        );
    }

    #[test]
    fn a_bell_stays_up_from_the_first_delivery_until_the_wait_ends() {
        let mut scheduler = one_approval();
        assert!(!scheduler.has_reminded(FollowUpKind::Approval, "s1"));
        scheduler.collect_due(at(60.0), false);
        assert!(scheduler.has_reminded(FollowUpKind::Approval, "s1"));
        scheduler.sync_waiting(FollowUpKind::Approval, &BTreeSet::new(), at(70.0));
        assert!(!scheduler.has_reminded(FollowUpKind::Approval, "s1"));
    }

    #[test]
    fn restart_forgets_attempts_and_silence() {
        let mut scheduler = one_approval();
        scheduler.collect_due(at(60.0), false);
        scheduler.silence(&Key::new(FollowUpKind::Approval, "s1"));
        scheduler.restart(FollowUpKind::Approval, "s1", Origin::Island, at(100.0));
        let due = scheduler.collect_due(at(160.0), false);
        assert_eq!(due[0].attempt, 1);
        assert_eq!(due[0].delivery, Delivery::OnTime);
    }
}
