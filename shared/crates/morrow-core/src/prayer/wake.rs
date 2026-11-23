//! When a prayer reminder is actually due, and when it is already too late.
//!
//! A desktop suspends: the process wakes with several milestones apparently
//! "past due" at once. This policy keeps the small set of milestones it has
//! already announced, so a replay can never send a second notification for the
//! same one, and it refuses milestones older than a short window so a wake-up
//! hours later does not dump the whole day's backlog on the user. Nothing here
//! reads a clock, sends a notification or holds OS state — the caller injects
//! the current time.

use crate::prayer::{PrayerType, YearMonthDay};

/// A reminder is only worth sending this long after its milestone.
pub const DEFAULT_WINDOW_SECONDS: i64 = 15 * 60;

/// Two days of five fardh prayers. Older keys are dropped, so a long-running
/// shell cannot accumulate one entry per day forever.
const MAX_TRACKED: usize = 10;

/// One fardh milestone of one calendar day: the identity of a notification.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PrayerMilestone {
    pub day: YearMonthDay,
    pub prayer: PrayerType,
}

/// The de-duplication and backlog rule, with an injected clock.
#[derive(Clone, Debug, Default)]
pub struct PrayerWakePolicy {
    /// `(milestone, instant we sent it)`, oldest first.
    fired: Vec<(PrayerMilestone, i64)>,
}

impl PrayerWakePolicy {
    pub fn new() -> Self {
        Self::default()
    }

    /// Decide whether this milestone should be announced *now*, remembering
    /// the decision so a repeated check (suspend/unlock, timer coalescing)
    /// cannot send it twice.
    ///
    /// Returns false before the milestone and once it is older than
    /// `window_seconds`. Keys are pruned by that same window and capped, so
    /// neither a replay nor a long session grows the tracked set.
    pub fn should_fire(
        &mut self,
        day: YearMonthDay,
        prayer: PrayerType,
        due_epoch_seconds: i64,
        now_epoch_seconds: i64,
        window_seconds: i64,
    ) -> bool {
        self.fired.retain(|(_, at)| {
            *at <= now_epoch_seconds && now_epoch_seconds - *at <= window_seconds
        });
        if self.fired.len() > MAX_TRACKED {
            self.fired.drain(..self.fired.len() - MAX_TRACKED);
        }

        let milestone = PrayerMilestone { day, prayer };
        if self.fired.iter().any(|(sent, _)| *sent == milestone) {
            return false;
        }
        if now_epoch_seconds < due_epoch_seconds {
            return false;
        }
        if now_epoch_seconds - due_epoch_seconds > window_seconds {
            return false;
        }

        self.fired.push((milestone, now_epoch_seconds));
        true
    }

    /// [`Self::should_fire`] with the ordinary reminder window.
    pub fn due(
        &mut self,
        day: YearMonthDay,
        prayer: PrayerType,
        due_epoch_seconds: i64,
        now_epoch_seconds: i64,
    ) -> bool {
        self.should_fire(
            day,
            prayer,
            due_epoch_seconds,
            now_epoch_seconds,
            DEFAULT_WINDOW_SECONDS,
        )
    }

    /// How many milestones are still remembered. Mainly for tests and for
    /// callers that want to assert the set cannot grow without bound.
    pub fn tracked_count(&self) -> usize {
        self.fired.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: YearMonthDay = YearMonthDay::new(2026, 1, 15);

    #[test]
    fn one_notification_per_milestone() {
        let mut policy = PrayerWakePolicy::new();
        assert!(policy.due(DAY, PrayerType::Subuh, 1000, 1000));
        assert!(!policy.due(DAY, PrayerType::Subuh, 1000, 1001));
        assert!(!policy.due(DAY, PrayerType::Subuh, 1000, 1010));
        assert_eq!(policy.tracked_count(), 1);

        // A different prayer of the same day, and the same prayer tomorrow.
        assert!(policy.due(DAY, PrayerType::Dzuhur, 2000, 2000));
        assert!(policy.due(
            YearMonthDay::new(2026, 1, 16),
            PrayerType::Subuh,
            3000,
            3000
        ));
        assert_eq!(policy.tracked_count(), 3);
    }

    #[test]
    fn unlock_does_not_replay_a_milestone_already_sent() {
        let mut policy = PrayerWakePolicy::new();
        assert!(policy.due(DAY, PrayerType::Maghrib, 1000, 1000));
        // The machine slept, then woke while the milestone is still inside the
        // window: the key is remembered, so this is not a second notification.
        assert!(!policy.due(DAY, PrayerType::Maghrib, 1000, 1400));
        assert_eq!(policy.tracked_count(), 1);
    }

    #[test]
    fn old_backlog_is_dropped_instead_of_accumulating() {
        let mut policy = PrayerWakePolicy::new();
        // Five hours late: neither an old milestone nor a replay of one is due.
        assert!(!policy.due(DAY, PrayerType::Isya, 1000, 19_000));
        assert!(!policy.due(DAY, PrayerType::Subuh, 1000, 19_000));
        assert_eq!(policy.tracked_count(), 0);

        // A milestone in the future waits, and does not enter the tracked set.
        assert!(!policy.due(DAY, PrayerType::Subuh, 1000, 999));
        assert_eq!(policy.tracked_count(), 0);
    }

    #[test]
    fn tracked_keys_stay_bounded_across_many_days() {
        let mut policy = PrayerWakePolicy::new();
        for day in 1..=30 {
            let at = day as i64 * 86_400;
            let date = YearMonthDay::new(2026, 2, day as i32);
            for (index, prayer) in PrayerType::FARDH.iter().enumerate() {
                // Milestones are due in the same order they are checked, well
                // inside the window after `at`.
                let due = at + index as i64;
                assert!(policy.due(date, *prayer, due, at + index as i64));
                assert!(policy.tracked_count() <= MAX_TRACKED);
            }
        }
    }

    #[test]
    fn custom_window_controls_how_late_a_reminder_can_be() {
        let mut policy = PrayerWakePolicy::new();
        assert!(!policy.should_fire(DAY, PrayerType::Subuh, 1000, 1600, 60));
        assert!(policy.should_fire(DAY, PrayerType::Subuh, 1000, 1600, 700));
    }
}
