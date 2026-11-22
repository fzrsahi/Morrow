//! The sessions, the held permission requests, and their way to the
//! frontend: hooks are reduced as they arrive, and the snapshot is pushed
//! through the frontend's `Channel` at most once a frame (≤60 Hz), whatever
//! the hook rate.

use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use morrow_core::reduce::SideEffect;
use morrow_platform::process::Watch;
use morrow_protocol::HookPayload;
use serde::Serialize;
use tauri::ipc::Channel;

use crate::permissions::{Card, Decision, Permissions};
use crate::prune::{SessionProcess, Watches};
use crate::sessions::{Session, Sessions};

/// What the frontend receives (`Snapshot` in `src/App.svelte`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    pub sessions: Vec<Session>,
    /// Held permission requests, oldest first.
    pub permissions: Vec<Card>,
}

/// One push per 1/60 s at most.
const FRAME: Duration = Duration::from_micros(16_667);

/// One liveness round per second: well inside the five seconds a dead CLI
/// has to disappear from the panel (#198).
const PRUNE_POLL: Duration = Duration::from_secs(1);

#[derive(Default)]
pub struct Feed {
    state: Mutex<State>,
    changed: Condvar,
    /// Wakes the pruner when a session appears; separate from the pusher's
    /// condvar so a single hook cannot wake only the wrong waiter.
    prune_changed: Condvar,
}

struct State {
    sessions: Sessions,
    permissions: Permissions,
    watches: Watches,
    channel: Option<Channel<Snapshot>>,
    /// The frontend has not seen the latest snapshot.
    dirty: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            sessions: Sessions::with_home(std::env::var("HOME").unwrap_or_default()),
            permissions: Permissions::default(),
            watches: Watches::default(),
            channel: None,
            dirty: false,
        }
    }
}

impl State {
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            sessions: self.sessions.snapshot(),
            permissions: self.permissions.cards(),
        }
    }
}

impl Feed {
    /// Reduces one hook into the sessions.
    pub fn record(&self, payload: &HookPayload) {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
        let mut state = self.lock();
        for effect in state.sessions.apply(payload, now_ms) {
            if let SideEffect::RemoveSession { session_id } = effect {
                state.permissions.drop_session(&session_id);
                state.watches.forget(&session_id);
            }
        }
        state.dirty = true;
        self.changed.notify_one();
        self.prune_changed.notify_one();
    }

    /// Holds `payload` as a permission card; its decision arrives on the
    /// receiver.
    pub fn hold(&self, payload: &HookPayload) -> (u64, Receiver<Decision>) {
        let (answer, decision) = mpsc::channel();
        let id = self.change(|state| state.permissions.hold(payload, answer));
        (id, decision)
    }

    /// The user's answer to card `id`; false if it is no longer held.
    pub fn answer(&self, id: u64, decision: Decision) -> bool {
        self.change(|state| state.permissions.answer(id, decision))
    }

    /// Card `id`'s hook is gone.
    pub fn drop_request(&self, id: u64) {
        self.change(|state| state.permissions.drop_request(id));
    }

    fn change<T>(&self, f: impl FnOnce(&mut State) -> T) -> T {
        let mut state = self.lock();
        let result = f(&mut state);
        state.dirty = true;
        self.changed.notify_one();
        result
    }

    /// Sends snapshots to `channel` from now on, the current one first. A
    /// reloaded frontend subscribes again and replaces the old channel.
    pub fn subscribe(&self, channel: Channel<Snapshot>) {
        let mut state = self.lock();
        state.channel = Some(channel);
        state.dirty = true;
        self.changed.notify_one();
    }

    /// One liveness round. The adapters are injected so the decision can be
    /// exercised without real processes; `prune_dead` supplies the shell's.
    fn prune_with(
        &self,
        capture: impl FnMut(i32) -> Option<Watch>,
        alive: impl FnMut(Watch) -> bool,
    ) {
        let mut state = self.lock();
        let processes: Vec<SessionProcess> = state
            .sessions
            .snapshot()
            .into_iter()
            .map(|s| SessionProcess {
                session_id: s.session_id,
                pid: s.cli_pid,
            })
            .collect();
        let dead = state.watches.reconcile(&processes, capture, alive);
        if dead.is_empty() {
            return;
        }
        for id in &dead {
            state.sessions.remove(id);
            state.permissions.drop_session(id);
            state.watches.forget(id);
        }
        state.dirty = true;
        self.changed.notify_one();
    }

    /// Drop sessions whose CLI process is gone, with their cards.
    pub fn prune_dead(&self) {
        self.prune_with(
            |pid| crate::adapters::process_watch(pid as i64),
            crate::adapters::process_alive,
        );
    }

    /// Poll liveness while there are sessions to watch, then park until a
    /// hook creates another one: no adapters are called for an empty panel.
    pub fn spawn_pruner(feed: Arc<Self>) {
        thread::Builder::new()
            .name("morrow-pruner".into())
            .spawn(move || {
                loop {
                    {
                        let state = feed.lock();
                        if state.sessions.is_empty() {
                            let _parked = feed
                                .prune_changed
                                .wait(state)
                                .unwrap_or_else(PoisonError::into_inner);
                            continue;
                        }
                    }
                    feed.prune_dead();
                    thread::sleep(PRUNE_POLL);
                }
            })
            .expect("spawn the pruner thread");
    }

    /// Pushes snapshots forever: waits for a change, sends, waits a frame.
    /// `on_sent` is told how many sessions and cards each push carried.
    pub fn spawn_pusher(feed: Arc<Self>, on_sent: impl Fn(usize, usize) + Send + 'static) {
        thread::Builder::new()
            .name("morrow-feed".into())
            .spawn(move || {
                loop {
                    let (snapshot, channel) = {
                        let mut state = feed.lock();
                        while !(state.dirty && state.channel.is_some()) {
                            state = feed
                                .changed
                                .wait(state)
                                .unwrap_or_else(PoisonError::into_inner);
                        }
                        state.dirty = false;
                        let snapshot = state.snapshot();
                        (snapshot, state.channel.clone())
                    };
                    let counts = (snapshot.sessions.len(), snapshot.permissions.len());
                    if let Some(channel) = channel {
                        match channel.send(snapshot) {
                            Ok(()) => on_sent(counts.0, counts.1),
                            Err(error) => eprintln!("morrow-app: push to the frontend: {error}"),
                        }
                    }
                    thread::sleep(FRAME);
                }
            })
            .expect("spawn the feed thread");
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use morrow_platform::process::Watch;
    use morrow_protocol::decode_payload;

    fn hook(json: &str) -> HookPayload {
        decode_payload(json.as_bytes()).unwrap()
    }

    #[test]
    fn a_dead_session_takes_its_cards_with_it() {
        let feed = Feed::default();
        for id in ["a", "b"] {
            let _ = feed.hold(&hook(&format!(
                r#"{{"session_id":"{id}","hook_event_name":"PermissionRequest","tool_name":"Bash"}}"#
            )));
            feed.record(&hook(&format!(
                r#"{{"session_id":"{id}","hook_event_name":"SessionStart","_ppid":{}}}"#,
                if id == "a" { 4242 } else { 9999 }
            )));
        }

        // Only pid 9999 is alive; "a" and its approval card must go.
        feed.prune_with(
            |pid| {
                Some(Watch {
                    pid: pid as i64,
                    start_time: 1,
                })
            },
            |w| w.pid == 9999,
        );

        let snap = feed.lock().snapshot();
        assert_eq!(
            snap.sessions
                .iter()
                .map(|s| s.session_id.as_str())
                .collect::<Vec<_>>(),
            ["b"]
        );
        assert_eq!(
            snap.permissions
                .iter()
                .map(|c| c.session_id.as_str())
                .collect::<Vec<_>>(),
            ["b"]
        );
    }
}
