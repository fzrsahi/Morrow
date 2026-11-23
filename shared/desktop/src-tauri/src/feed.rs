//! The sessions, the held permission requests, and their way to the
//! frontend: hooks are reduced as they arrive, and the snapshot is pushed
//! through the frontend's `Channel` at most once a frame (≤60 Hz), whatever
//! the hook rate.

use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use morrow_core::attention::CompletionStyle;
use morrow_core::reduce::SideEffect;
use morrow_platform::process::Watch;
use morrow_platform::sound;
use morrow_protocol::HookPayload;
use serde::Serialize;
use tauri::ipc::Channel;

use crate::completion::{self, SnapshotCompletion};
use crate::permissions::{Card, Decision, Permissions};
use crate::prune::{Pid, SessionProcess, Watches};
use crate::sessions::{Session, Sessions};
use crate::transcripts::Tails;

/// What the frontend receives (`Snapshot` in `src/App.svelte`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    pub sessions: Vec<Session>,
    /// Held permission requests, oldest first.
    pub permissions: Vec<Card>,
    /// The last finished turn's reply, parsed into Markdown blocks by the
    /// core, and its settled presentation. `None` after the next prompt.
    pub completion: Option<SnapshotCompletion>,
}

/// One push per 1/60 s at most.
const FRAME: Duration = Duration::from_micros(16_667);
/// How often a visible, unlocked panel looks for new transcript bytes. The
/// feed still coalesces the resulting pushes to `FRAME`; a hidden or locked
/// panel does not wake at all (see the pusher below).
const TAIL_POLL: Duration = Duration::from_millis(200);

/// One liveness round per second: well inside the five seconds a dead CLI
/// has to disappear from the panel (#198).
const PRUNE_POLL: Duration = Duration::from_secs(1);

/// Where a decided sound goes (#241). Production leaves it `None` and the
/// platform plays it; a test installs a recorder, because "a locked screen is
/// silent" is only observable if something was asked to play.
pub type SoundOutlet = Arc<dyn Fn(&str) + Send + Sync>;

pub struct Feed {
    state: Mutex<State>,
    changed: Condvar,
    /// Wakes the pruner when a session appears; separate from the pusher's
    /// condvar so a single hook cannot wake only the wrong waiter.
    prune_changed: Condvar,
    sound: Option<SoundOutlet>,
}

impl Default for Feed {
    fn default() -> Self {
        Self {
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
            prune_changed: Condvar::new(),
            sound: None,
        }
    }
}

struct State {
    sessions: Sessions,
    permissions: Permissions,
    completion: Option<SnapshotCompletion>,
    watches: Watches,
    channel: Option<Channel<Snapshot>>,
    /// The frontend has not seen the latest snapshot.
    dirty: bool,
    /// The island window is showing (#197 hides it for fullscreen). Only a
    /// visible panel tails transcripts.
    visible: bool,
    /// #197: a locked screen suspends content probes without hiding the
    /// window. Independent of `visible` so fullscreen hiding is unchanged.
    probes_active: bool,
    tails: Tails,
    /// The scan policy's clock (#259): how often a visible panel may look for
    /// sessions, and the one place the gap is defined.
    discovery: morrow_core::scan_policy::ScanThrottle,
}

impl Default for State {
    fn default() -> Self {
        Self {
            sessions: Sessions::with_home(std::env::var("HOME").unwrap_or_default()),
            permissions: Permissions::default(),
            completion: None,
            watches: Watches::default(),
            channel: None,
            dirty: false,
            visible: true,
            probes_active: true,
            tails: Tails::default(),
            discovery: morrow_core::scan_policy::ScanThrottle::new(
                morrow_core::scan_policy::MINIMUM_GAP_MS,
            ),
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

impl State {
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            sessions: self.sessions.snapshot(),
            permissions: self.permissions.cards(),
            completion: self.completion.clone(),
        }
    }
}

impl Feed {
    /// A feed whose sounds go to `outlet` instead of the desktop (#241).
    #[cfg(test)]
    pub fn with_sound(outlet: SoundOutlet) -> Self {
        Self {
            sound: Some(outlet),
            ..Self::default()
        }
    }

    /// Play one cue through the outlet, or the platform's player when there is
    /// none. A missing or failing player is a Capability the Settings table
    /// already states; the feed has nowhere to show it and must not stop.
    fn emit_sound(&self, cue: &str) {
        if let Some(outlet) = &self.sound {
            outlet(cue);
            return;
        }
        if let Some(player) = sound::player_on_path() {
            let _ = sound::play(player, cue);
        }
    }

    /// Reduces one hook into the sessions.
    pub fn record(&self, payload: &HookPayload) {
        let now = now_ms();
        // #241: the cues are collected under the lock and played after it is
        // released — starting a process is not something the feed's other
        // waiters should queue behind.
        let mut cues: Vec<String> = Vec::new();
        let mut state = self.lock();
        let applied = state.sessions.apply_with_intents(payload, now);
        for effect in &applied.effects {
            match effect {
                SideEffect::RemoveSession { session_id } => {
                    state.permissions.drop_session(session_id);
                    state.tails.remove(session_id);
                    state.watches.forget(session_id);
                }
                // #241: a locked screen is silent, and the cue is dropped
                // rather than queued for the unlock. `probes_active` is the
                // shell's lock state (#197), the same one the probes obey.
                SideEffect::PlaySound(cue) if state.probes_active => cues.push(cue.clone()),
                _ => {}
            }
        }
        let event_name = payload
            .as_object()
            .get("hook_event_name")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        // A new turn retires the old card; a turn end offers a new one. The
        // policy's intent (not the raw event name) decides whether it exists.
        if event_name == "UserPromptSubmit" || event_name == "SessionStart" {
            state.completion = None;
        } else if let Some(completion) = applied.intents.completion.as_ref() {
            let session_id = payload
                .as_object()
                .get("session_id")
                .and_then(|value| value.as_str())
                .unwrap_or("default");
            if let Some(session) = state
                .sessions
                .snapshot()
                .into_iter()
                .find(|s| s.session_id == session_id)
            {
                state.completion = Some(SnapshotCompletion {
                    session_id: session.session_id,
                    updated_at: session.updated_at,
                    turn_failed: completion.turn_failed,
                    presentation: completion::presentation(
                        Some(completion),
                        CompletionStyle::Expand,
                        false,
                    ),
                    blocks: completion::blocks(&session.last_assistant_message),
                });
            }
        }
        state.dirty = true;
        drop(state);
        for cue in &cues {
            self.emit_sound(cue);
        }
        self.changed.notify_one();
        self.prune_changed.notify_one();
    }

    /// The window was hidden or shown (#197). Hiding forgets every attachment
    /// so showing backfills from the tail instead of replaying a stale offset.
    pub fn set_visible(&self, visible: bool) {
        let mut state = self.lock();
        if state.visible == visible {
            return;
        }
        state.visible = visible;
        if visible {
            state.dirty = true;
        } else {
            state.tails = Tails::default();
        }
        self.changed.notify_one();
    }

    /// #197: suspend content probes on a locked screen without touching the
    /// window. As with hiding, the attachments go: resuming backfills from the
    /// tail instead of replaying a stale offset.
    pub fn set_probes_active(&self, active: bool) {
        let mut state = self.lock();
        if state.probes_active == active {
            return;
        }
        state.probes_active = active;
        if !active {
            state.tails = Tails::default();
        }
    }

    /// Read whatever the visible panel's sessions have appended. Called by
    /// the pusher on its timeout, never from a hidden or locked panel; hooks
    /// still arrive and are reduced regardless of visibility.
    pub fn poll_transcripts(&self) {
        let mut state = self.lock();
        if !state.visible || !state.probes_active {
            return;
        }
        let paths = state.sessions.transcript_paths();
        let live: HashSet<String> = paths.iter().map(|(id, _)| id.clone()).collect();
        let mut changed = false;
        for (session_id, path) in paths {
            if let Some(delta) = state.tails.attach(&session_id, &path) {
                changed |= state.sessions.apply_transcript_delta(&session_id, &delta);
            }
            if let Some(delta) = state.tails.poll(&session_id) {
                changed |= state.sessions.apply_transcript_delta(&session_id, &delta);
            }
        }
        state.tails.retain(|session_id| live.contains(session_id));
        if changed {
            state.dirty = true;
            self.changed.notify_one();
        }
    }

    /// Holds `payload` as a permission card; its decision arrives on the
    /// receiver.
    pub fn hold(&self, payload: &HookPayload) -> (u64, Receiver<Vec<u8>>) {
        let (answer, decision) = mpsc::channel();
        let id = self.change(|state| state.permissions.hold(payload, answer));
        (id, decision)
    }

    /// The user's answer to card `id`; false if it is no longer held.
    pub fn answer(&self, id: u64, decision: Decision) -> bool {
        self.change(|state| state.permissions.answer(id, decision))
    }

    /// #188: a question option was picked; its label is echoed as the answer.
    pub fn answer_question(&self, id: u64, pick: &str) -> bool {
        self.change(|state| state.permissions.answer_question(id, pick))
    }

    /// #190: one answer per question of a wizard request, in question order.
    pub fn answer_questions(&self, id: u64, answers: Vec<String>) -> bool {
        self.change(|state| state.permissions.answer_questions(id, answers))
    }

    /// #188: the question card was skipped, not answered.
    pub fn skip_question(&self, id: u64) -> bool {
        self.change(|state| state.permissions.skip_question(id))
    }

    /// Card `id`'s hook is gone.
    pub fn drop_request(&self, id: u64) {
        self.change(|state| state.permissions.drop_request(id));
    }

    /// #185: the user removed a session from its card. Its held cards go too,
    /// and the next snapshot carries the shorter list.
    pub fn dismiss(&self, session_id: &str) {
        self.change(|state| {
            state.sessions.remove(session_id);
            state.permissions.drop_session(session_id);
        });
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
    ///
    /// **Attach is when discovery runs** (#261). A panel that just appeared has
    /// nothing to show and every session an agent already has is invisible until
    /// its next hook, so the scan happens here rather than on a timer: the store
    /// is read when somebody is looking, and not otherwise.
    pub fn subscribe(&self, channel: Channel<Snapshot>) {
        {
            let mut state = self.lock();
            state.channel = Some(channel);
            state.dirty = true;
            self.changed.notify_one();
        }
        self.backfill();
    }

    /// One discovery tick: scan, if the policy says this is when (#248).
    ///
    /// A **hidden or locked panel does not scan at all** — `probes_active` is the
    /// frontend's lock watch (#197), and this is the same gate the transcript
    /// tailer uses. That is the discovery half of what the policy promises: an
    /// unattended panel costs nothing.
    ///
    /// A visible one scans at most once per `MINIMUM_GAP_MS`, and the gap lives in
    /// `scan_policy` rather than here so there is one number, not two.
    ///
    /// This is a **poll**, not an event watcher: macOS reacts to FSEvents, and
    /// Linux/Windows have no watch in this shell yet. The cost is bounded by the
    /// gap and, for the stores a reader can re-read, by `ScanCache` — which the
    /// readers do not use yet, which is why the gap is five seconds and not one.
    pub fn tick_discovery(&self, now_ms: u64) -> usize {
        self.tick_discovery_with(now_ms, |_home| self.scan_stores())
    }

    /// `tick_discovery` with the scan injected, so a test drives the policy
    /// without reading a real `~`.
    pub fn tick_discovery_with(
        &self,
        now_ms: u64,
        scan: impl FnOnce(&str) -> morrow_core::discovery::scan::ScanResult,
    ) -> usize {
        let allowed = {
            let mut state = self.lock();
            if !state.probes_active || !state.visible {
                // Forget the clock: coming back to a visible panel scans at once
                // rather than waiting out a gap that started while it was hidden.
                state.discovery.reset();
                return 0;
            }
            match state.discovery.decide(true, now_ms) {
                morrow_core::scan_policy::Decision::ScanNow => {
                    state.discovery.note_scan_started(now_ms);
                    true
                }
                _ => false,
            }
        };
        if !allowed {
            return 0;
        }
        self.backfill_with(scan)
    }

    /// Polls the policy forever. One second of granularity is enough for a
    /// five-second gap, and it costs a mutex read when nothing is due.
    pub fn spawn_discovery(feed: Arc<Self>) {
        thread::Builder::new()
            .name("morrow-discovery".into())
            .spawn(move || {
                loop {
                    thread::sleep(Duration::from_millis(250));
                    feed.tick_discovery(now_ms());
                }
            })
            .expect("spawn the discovery thread");
    }

    /// One discovery scan, integrated as ordinary sessions.
    ///
    /// Deliberately outside the lock while it reads the stores — a scan walks
    /// `~/.claude/projects` and reads tails — and inside it only to insert. A hook
    /// that arrives during the scan wins: `insert_discovered` refuses a session
    /// that already exists, so a WORKING card is never taken back to IDLE by a
    /// row the scan read a moment ago.
    ///
    /// Returns how many sessions were added, which is what the test asserts.
    pub fn backfill(&self) -> usize {
        self.backfill_with(|home| self.scan_stores_in(std::path::Path::new(home)))
    }

    /// The real stores, from the home `backfill_with` was given.
    fn scan_stores(&self) -> morrow_core::discovery::scan::ScanResult {
        let home = self.lock().sessions.home_directory();
        self.scan_stores_in(std::path::Path::new(&home))
    }

    fn scan_stores_in(&self, home: &std::path::Path) -> morrow_core::discovery::scan::ScanResult {
        // Claude Code's config dir comes from the installer's own ladder (#258),
        // so discovery reads the folder the hooks were written into.
        let config_dir = crate::hooks::claude_config_dir(
            home,
            std::env::var("CLAUDE_CONFIG_DIR").ok().as_deref(),
        )
        .unwrap_or_else(|| home.join(".claude"));
        crate::discovery::scan_once(&config_dir, home)
    }

    /// One discovery scan, integrated as ordinary sessions.
    ///
    /// Deliberately outside the lock while it reads the stores — a scan walks
    /// `~/.claude/projects`, reads tails and opens SQLite — and inside it only to
    /// insert. A hook that arrives during the scan wins: `insert_discovered`
    /// refuses a session that already exists, so a WORKING card is never taken
    /// back to IDLE by a row the scan read a moment ago.
    ///
    /// Returns how many sessions were added, which is what the test asserts.
    pub fn backfill_with(
        &self,
        scan: impl FnOnce(&str) -> morrow_core::discovery::scan::ScanResult,
    ) -> usize {
        let home = {
            let state = self.lock();
            state.sessions.home_directory()
        };
        let mut result = scan(&home);
        // #267: a discovered session without a pid is a card that can never be
        // pruned. The table is read here, once, and only when something needs it.
        crate::discovery::correlate_pids(&mut result.hits);
        let mut state = self.lock();
        let mut added = 0;
        for hit in &result.hits {
            if state.sessions.insert_discovered(hit) {
                added += 1;
            }
        }
        if added > 0 {
            state.dirty = true;
            // A discovered session with a pid deserves the same liveness watch a
            // hook-born one gets (#198); one without keeps its card until the
            // store stops changing, which is what `Watches` already does for a
            // session that has no capturable pid.
            self.prune_changed.notify_one();
            self.changed.notify_one();
        }
        added
    }

    fn prune_with(
        &self,
        capture: impl FnMut(Pid) -> Option<Watch>,
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
            state.tails.remove(id);
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
                            if state.visible && state.probes_active {
                                // Only a showing, unlocked panel wakes on its
                                // own: that wake is the transcript poll. A
                                // hidden or locked one waits for a hook or a
                                // subscribe instead.
                                let (next, elapsed) = feed
                                    .changed
                                    .wait_timeout(state, TAIL_POLL)
                                    .unwrap_or_else(PoisonError::into_inner);
                                state = next;
                                if elapsed.timed_out() {
                                    drop(state);
                                    feed.poll_transcripts();
                                    state = feed.lock();
                                }
                            } else {
                                state = feed
                                    .changed
                                    .wait(state)
                                    .unwrap_or_else(PoisonError::into_inner);
                            }
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

    /// The current snapshot (the pusher uses `State::snapshot` directly).
    #[cfg(test)]
    pub fn snapshot(&self) -> Snapshot {
        self.lock().snapshot()
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
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};

    /// The AC of #261, at the feed: a panel that has just appeared shows the
    /// sessions the agent already has, **before** any hook arrives.
    #[test]
    fn backfill_shows_sessions_before_any_hook_arrives() {
        let feed = Feed::default();
        assert!(feed.snapshot().sessions.is_empty(), "nothing to show yet");

        let added = feed.backfill_with(|_home| morrow_core::discovery::scan::ScanResult {
            hits: vec![
                discovered("found-a", "/home/u/a"),
                discovered("found-b", "/home/u/b"),
            ],
            ..Default::default()
        });

        assert_eq!(added, 2);
        let rows = feed.snapshot().sessions;
        let mut ids: Vec<&str> = rows.iter().map(|row| row.session_id.as_str()).collect();
        ids.sort();
        assert_eq!(ids, vec!["found-a", "found-b"]);
        assert!(
            rows.iter().all(|row| row.last_event.is_empty()),
            "a discovered session has no hook event, and the card must not claim one"
        );
    }

    /// A hook that lands during the scan owns the card: the scan must not take a
    /// WORKING session back to DONE.
    #[test]
    fn a_hook_that_lands_first_survives_the_backfill() {
        let feed = Feed::default();
        feed.record(
            &decode_payload(
                br#"{"session_id":"found-a","hook_event_name":"PreToolUse","cwd":"/home/u/a","tool_name":"Bash"}"#,
            )
            .unwrap(),
        );

        let added = feed.backfill_with(|_home| morrow_core::discovery::scan::ScanResult {
            hits: vec![discovered("found-a", "/home/u/elsewhere")],
            ..Default::default()
        });

        assert_eq!(added, 0, "the discovered row is the older witness");
        let rows = feed.snapshot().sessions;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].last_event, "PreToolUse");
    }

    /// #248: the policy decides when a discovery tick may scan. A hidden or locked
    /// panel does not scan at all — that is the whole point of the gate.
    #[test]
    fn a_hidden_or_locked_panel_never_scans() {
        let feed = Feed::default();
        let scans = std::sync::Arc::new(Mutex::new(0usize));
        let counted = Arc::clone(&scans);

        // Locked: `probes_active` false.
        feed.set_probes_active(false);
        assert_eq!(
            feed.tick_discovery_with(0, move |_home| {
                *counted.lock().unwrap() += 1;
                morrow_core::discovery::scan::ScanResult::default()
            }),
            0
        );
        assert_eq!(*scans.lock().unwrap(), 0, "a locked panel must not scan");

        // Visible but not showing.
        feed.set_probes_active(true);
        feed.set_visible(false);
        let counted = Arc::clone(&scans);
        assert_eq!(
            feed.tick_discovery_with(0, move |_home| {
                *counted.lock().unwrap() += 1;
                morrow_core::discovery::scan::ScanResult::default()
            }),
            0
        );
        assert_eq!(*scans.lock().unwrap(), 0, "a hidden panel must not scan");
    }

    /// A visible panel scans once per gap, not once per tick: the tick runs four
    /// times a second.
    #[test]
    fn a_visible_panel_scans_once_per_gap() {
        let feed = Feed::default();
        let scans = Arc::new(Mutex::new(0usize));
        let gap = morrow_core::scan_policy::MINIMUM_GAP_MS;

        for now in [0, 100, 200, 400, gap - 1] {
            let counted = Arc::clone(&scans);
            feed.tick_discovery_with(now, move |_home| {
                *counted.lock().unwrap() += 1;
                morrow_core::discovery::scan::ScanResult::default()
            });
        }
        assert_eq!(
            *scans.lock().unwrap(),
            1,
            "five ticks inside one gap, one scan"
        );

        let counted = Arc::clone(&scans);
        feed.tick_discovery_with(gap, move |_home| {
            *counted.lock().unwrap() += 1;
            morrow_core::discovery::scan::ScanResult::default()
        });
        assert_eq!(*scans.lock().unwrap(), 2, "the gap has passed");
    }

    /// Coming back to a visible panel scans at once, rather than waiting out a gap
    /// that started while it was hidden.
    #[test]
    fn a_panel_that_comes_back_scans_at_once() {
        let feed = Feed::default();
        let scans = Arc::new(Mutex::new(0usize));

        let counted = Arc::clone(&scans);
        feed.tick_discovery_with(1_000, move |_home| {
            *counted.lock().unwrap() += 1;
            morrow_core::discovery::scan::ScanResult::default()
        });
        assert_eq!(*scans.lock().unwrap(), 1);

        feed.set_visible(false);
        feed.tick_discovery(1_100);
        feed.set_visible(true);

        let counted = Arc::clone(&scans);
        feed.tick_discovery_with(1_200, move |_home| {
            *counted.lock().unwrap() += 1;
            morrow_core::discovery::scan::ScanResult::default()
        });
        assert_eq!(
            *scans.lock().unwrap(),
            2,
            "the clock was forgotten while hidden"
        );
    }

    /// A store that is not there adds nothing and does not mark the feed dirty.
    #[test]
    fn an_empty_scan_changes_nothing() {
        let feed = Feed::default();
        assert_eq!(
            feed.backfill_with(|_home| morrow_core::discovery::scan::ScanResult::default()),
            0
        );
        assert!(feed.snapshot().sessions.is_empty());
    }

    fn discovered(session_id: &str, cwd: &str) -> morrow_core::discovery::DiscoveryHit {
        morrow_core::discovery::DiscoveryHit {
            session_id: session_id.to_owned(),
            source: "claude",
            cwd: cwd.to_owned(),
            modified_at: Some(1_000),
            transcript_path: Some(format!("/roots/{session_id}.jsonl")),
            ..morrow_core::discovery::DiscoveryHit::default()
        }
    }

    const REPLY: &str =
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Adding filters"}]}}"#;
    const NEXT_REPLY: &str =
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Wired the bar"}]}}"#;
    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "morrow-feed-192-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&dir).expect("scratch dir");
            Self(dir)
        }
        fn file(&self) -> PathBuf {
            self.0.join("transcript.jsonl")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn hook(json: &str) -> HookPayload {
        decode_payload(json.as_bytes()).unwrap()
    }

    fn append(path: &Path, line: &str) {
        writeln!(
            OpenOptions::new().append(true).open(path).expect("open"),
            "{line}"
        )
        .expect("append");
    }

    fn live_reply(feed: &Feed) -> String {
        feed.lock().sessions.snapshot()[0].live_reply.clone()
    }

    #[test]
    fn a_hidden_panel_does_not_tail_but_a_visible_one_does() {
        let scratch = Scratch::new();
        let path = scratch.file();
        fs::write(&path, format!("{REPLY}\n")).expect("write transcript");
        let feed = Feed::default();
        feed.record(&hook(&format!(
            r#"{{"session_id":"a","hook_event_name":"SessionStart","transcript_path":"{}"}}"#,
            path.display()
        )));
        feed.set_visible(false);
        append(&path, NEXT_REPLY);

        feed.poll_transcripts();
        assert_eq!(live_reply(&feed), "", "no reads while hidden");
        assert!(!feed.lock().tails.has("a"), "hiding drops the attachment");

        feed.set_visible(true);
        feed.poll_transcripts();
        assert_eq!(live_reply(&feed), "Wired the bar");

        // Hiding and showing again picks up whatever was appended meanwhile.
        feed.set_visible(false);
        append(
            &path,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Newest"}]}}"#,
        );
        feed.set_visible(true);
        feed.poll_transcripts();
        assert_eq!(live_reply(&feed), "Newest");

        // The session's own end tears the tail down, even while visible.
        feed.record(&hook(
            r#"{"session_id":"a","hook_event_name":"SessionEnd"}"#,
        ));
        assert!(feed.lock().sessions.snapshot().is_empty());
        assert!(!feed.lock().tails.has("a"));
    }

    #[test]
    fn suspending_probes_stops_tails_without_hiding_the_island() {
        let scratch = Scratch::new();
        let path = scratch.file();
        fs::write(&path, format!("{REPLY}\n")).expect("write transcript");
        let feed = Feed::default();
        feed.record(&hook(&format!(
            r#"{{"session_id":"a","hook_event_name":"SessionStart","transcript_path":"{}"}}"#,
            path.display()
        )));

        // A locked screen suspends content probes but leaves the window drawn.
        feed.set_probes_active(false);
        append(&path, NEXT_REPLY);
        feed.poll_transcripts();
        assert_eq!(live_reply(&feed), "", "no reads while probes are suspended");
        assert!(feed.lock().visible, "the island stays visible while locked");

        feed.set_probes_active(true);
        feed.poll_transcripts();
        assert_eq!(
            live_reply(&feed),
            "Wired the bar",
            "resuming picks up what was appended while locked"
        );
    }

    #[test]
    fn a_locked_screen_plays_no_sound_and_a_cue_is_not_queued() {
        let played = Arc::new(Mutex::new(Vec::new()));
        let recorder = {
            let played = Arc::clone(&played);
            Arc::new(move |cue: &str| played.lock().unwrap().push(cue.to_owned()))
        };
        let feed = Feed::with_sound(recorder);
        let prompt = hook(r#"{"session_id":"a","hook_event_name":"UserPromptSubmit"}"#);
        let stop = hook(r#"{"session_id":"a","hook_event_name":"Stop"}"#);

        // Awake: the turn's own cues ring.
        feed.record(&prompt);
        feed.record(&stop);
        assert_eq!(
            *played.lock().unwrap(),
            vec!["UserPromptSubmit".to_owned(), "Stop".to_owned()]
        );

        // Locked: the same cues arrive and nothing is played, and nothing is
        // waiting for the unlock either (#241, macOS `settle`'s lock rule).
        played.lock().unwrap().clear();
        feed.set_probes_active(false);
        feed.record(&prompt);
        feed.record(&stop);
        assert!(
            played.lock().unwrap().is_empty(),
            "a locked screen is silent"
        );

        // Unlocking does not replay what was dropped.
        feed.set_probes_active(true);
        assert!(played.lock().unwrap().is_empty(), "no cue is queued");

        // And the outlet is live again.
        feed.record(&prompt);
        assert_eq!(*played.lock().unwrap(), vec!["UserPromptSubmit".to_owned()]);
    }

    #[test]
    fn a_finished_turn_pushes_its_reply_as_core_blocks() {
        let feed = Feed::default();
        feed.record(&hook(
            r#"{"session_id":"a","hook_event_name":"Stop","last_assistant_message":"Done\n====\nAll tests pass."}"#,
        ));
        let snapshot = feed.snapshot();
        let completion = snapshot.completion.expect("completion");
        assert_eq!(completion.session_id, "a");
        assert_eq!(completion.updated_at, snapshot.sessions[0].updated_at);
        assert_eq!(
            completion.presentation,
            crate::completion::Presentation::Card
        );
        assert_eq!(completion.blocks.len(), 2);
        assert_eq!(
            serde_json::to_value(&completion.blocks[0]).unwrap()["kind"].as_str(),
            Some("heading")
        );
    }

    #[test]
    fn a_new_prompt_clears_the_last_completion() {
        let feed = Feed::default();
        feed.record(&hook(
            r#"{"session_id":"a","hook_event_name":"Stop","last_assistant_message":"Done."}"#,
        ));
        assert!(feed.snapshot().completion.is_some());
        feed.record(&hook(
            r#"{"session_id":"a","hook_event_name":"UserPromptSubmit","prompt":"again"}"#,
        ));
        assert!(feed.snapshot().completion.is_none());
    }

    #[test]
    fn a_dead_session_takes_its_cards_with_it() {
        let scratch = Scratch::new();
        let path = scratch.file();
        fs::write(&path, format!("{REPLY}\n")).expect("write transcript");
        let feed = Feed::default();
        for id in ["a", "b"] {
            let _ = feed.hold(&hook(&format!(
                r#"{{"session_id":"{id}","hook_event_name":"PermissionRequest","tool_name":"Bash"}}"#
            )));
            feed.record(&hook(&format!(
                r#"{{"session_id":"{id}","hook_event_name":"SessionStart","_ppid":{},"transcript_path":"{}"}}"#,
                if id == "a" { 4242 } else { 9999 },
                path.display()
            )));
        }
        feed.poll_transcripts();
        assert!(feed.lock().tails.has("a"), "the live session has a tail");

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
        assert!(!feed.lock().tails.has("a"), "pruning stops its tail too");
    }
}
