//! Deciding which sessions are gone (#198). A watch is captured once per pid,
//! then rechecked by identity, so a reused pid cannot keep a dead session on
//! the panel. No process is touched here: the adapters are injected.

use std::collections::HashMap;

use morrow_platform::process::Watch;

/// A session's CLI pid, as the feed knows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionProcess {
    pub session_id: String,
    pub pid: Option<i32>,
}

/// The watches held for the sessions currently on the panel.
#[derive(Debug, Default)]
pub struct Watches {
    held: HashMap<String, (i32, Watch)>,
}

impl Watches {
    /// Capture a watch for every new or changed pid, then return the ids
    /// whose recorded process is gone.
    pub fn reconcile(
        &mut self,
        sessions: &[SessionProcess],
        mut capture: impl FnMut(i32) -> Option<Watch>,
        mut alive: impl FnMut(Watch) -> bool,
    ) -> Vec<String> {
        for session in sessions {
            let Some(pid) = session.pid else { continue };
            if self
                .held
                .get(&session.session_id)
                .is_some_and(|(held_pid, _)| *held_pid == pid)
            {
                continue;
            }
            self.held.remove(&session.session_id);
            if let Some(watch) = capture(pid) {
                self.held.insert(session.session_id.clone(), (pid, watch));
            }
        }

        self.held
            .retain(|id, _| sessions.iter().any(|s| s.session_id == *id));
        let mut dead: Vec<String> = self
            .held
            .iter()
            .filter(|(_, (_, watch))| !alive(*watch))
            .map(|(id, _)| id.clone())
            .collect();
        dead.sort();
        dead
    }

    /// Drop the watch when its session ends, so the same id can be watched
    /// afresh if a CLI comes back.
    pub fn forget(&mut self, session_id: &str) {
        self.held.remove(session_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn watch(pid: i32, start_time: u64) -> Watch {
        Watch {
            pid: pid as i64,
            start_time,
        }
    }

    fn session(id: &str, pid: Option<i32>) -> SessionProcess {
        SessionProcess {
            session_id: id.to_owned(),
            pid,
        }
    }

    /// The fake machine: a pid names whichever start time it holds now.
    fn alive_in(current: &HashMap<i32, u64>, w: Watch) -> bool {
        current.get(&(w.pid as i32)) == Some(&w.start_time)
    }

    #[test]
    fn a_session_whose_process_is_gone_is_pruned() {
        let mut watches = Watches::default();
        let sessions = [session("a", Some(42))];
        let dead = watches.reconcile(&sessions, |pid| Some(watch(pid, 7)), |_| false);
        assert_eq!(dead, ["a"]);
    }

    #[test]
    fn a_reused_pid_does_not_keep_the_old_session_alive() {
        let mut watches = Watches::default();
        let sessions = [session("a", Some(42))];
        // The watch names the process we first saw, not just its pid.
        let mut current = HashMap::from([(42, 7)]);
        assert!(
            watches
                .reconcile(
                    &sessions,
                    |pid| Some(watch(pid, current[&pid])),
                    |w| alive_in(&current, w),
                )
                .is_empty()
        );

        // The kernel hands pid 42 to another process. The pid in the hook did
        // not change, so the watch must not be recaptured: its start time is
        // what reveals the swap.
        current.insert(42, 9);
        let mut captures = 0;
        let dead = watches.reconcile(
            &sessions,
            |pid| {
                captures += 1;
                Some(watch(pid, current[&pid]))
            },
            |w| alive_in(&current, w),
        );
        assert_eq!(dead, ["a"]);
        assert_eq!(captures, 0, "the same pid keeps the original watch");
    }

    #[test]
    fn a_new_pid_gets_a_fresh_watch() {
        let mut watches = Watches::default();
        let mut captured = Vec::new();
        {
            let mut capture = |pid: i32| {
                captured.push(pid);
                Some(watch(pid, 7))
            };
            assert!(
                watches
                    .reconcile(&[session("a", Some(42))], &mut capture, |_| true)
                    .is_empty()
            );
            assert!(
                watches
                    .reconcile(&[session("a", Some(42))], &mut capture, |_| true)
                    .is_empty()
            );
            assert!(
                watches
                    .reconcile(&[session("a", Some(99))], &mut capture, |w| w.pid == 99)
                    .is_empty()
            );
        }
        assert_eq!(captured, [42, 99], "one watch per pid, not per poll");
    }

    #[test]
    fn a_session_without_a_capturable_pid_is_left_alone() {
        let mut watches = Watches::default();
        let sessions = [session("a", Some(42)), session("b", None)];
        assert!(watches.reconcile(&sessions, |_| None, |_| false).is_empty());
        // Once a watch can be taken, the usual rule applies to that session.
        assert_eq!(
            watches.reconcile(&sessions, |pid| Some(watch(pid, 7)), |_| false),
            ["a"]
        );
    }

    #[test]
    fn forgetting_a_session_lets_a_returning_one_be_watched_afresh() {
        let mut watches = Watches::default();
        let sessions = [session("a", Some(42))];
        assert!(
            watches
                .reconcile(&sessions, |pid| Some(watch(pid, 7)), |_| true)
                .is_empty()
        );
        watches.forget("a");
        let mut captures = 0;
        assert!(
            watches
                .reconcile(
                    &sessions,
                    |pid| {
                        captures += 1;
                        Some(watch(pid, 7))
                    },
                    |_| true,
                )
                .is_empty()
        );
        assert_eq!(captures, 1);
    }

    #[test]
    fn a_session_that_left_the_panel_is_no_longer_watched() {
        let mut watches = Watches::default();
        assert!(
            watches
                .reconcile(
                    &[session("a", Some(42))],
                    |pid| Some(watch(pid, 7)),
                    |_| true
                )
                .is_empty()
        );
        assert!(
            watches
                .reconcile(&[], |pid| Some(watch(pid, 7)), |_| false)
                .is_empty()
        );
        let mut captures = 0;
        let dead = watches.reconcile(
            &[session("a", Some(42))],
            |pid| {
                captures += 1;
                Some(watch(pid, 7))
            },
            |_| false,
        );
        assert_eq!(dead, ["a"]);
        assert_eq!(captures, 1, "the old watch was dropped with the session");
    }
}
