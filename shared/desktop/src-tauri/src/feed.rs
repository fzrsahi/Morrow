//! The sessions, the held permission requests, and their way to the
//! frontend: hooks are reduced as they arrive, and the snapshot is pushed
//! through the frontend's `Channel` at most once a frame (≤60 Hz), whatever
//! the hook rate.

use std::collections::HashSet;
use std::path::Path;
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
use serde_json::Value;
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
    /// Cards whose follow-up reminder already went out (#238 AC 9): the panel keeps a
    /// bell on them until the request is answered or dropped.
    pub reminder_ids: Vec<u64>,
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

/// Where a due reminder goes. Production leaves it `None` and the desktop's
/// notifier posts it; a test installs a recorder, because "no later nag" is
/// only observable if something was asked to notify.
pub type NotifyOutlet = Arc<dyn Fn(&str, &str) + Send + Sync>;

pub struct Feed {
    state: Mutex<State>,
    changed: Condvar,
    /// Wakes the pruner when a session appears; separate from the pusher's
    /// condvar so a single hook cannot wake only the wrong waiter.
    prune_changed: Condvar,
    sound: Option<SoundOutlet>,
    /// See [`NotifyOutlet`].
    notify: Option<NotifyOutlet>,
    /// What the last scan read, kept **across** ticks: a cache built inside one
    /// scan would read everything and forget it (#284). On the `Feed` rather than
    /// in `State` because a scan runs outside the state lock and this is
    /// internally synchronised on its own.
    scan_cache: morrow_core::scan_policy::TextCache,
    /// #238 AC 9: the follow-up schedule, in the core. Off the `State` lock because the
    /// reminder thread ticks while hooks are being reduced.
    reminders: Mutex<crate::reminders::ReminderBook>,
}

impl Default for Feed {
    fn default() -> Self {
        Self {
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
            prune_changed: Condvar::new(),
            sound: None,
            notify: None,
            scan_cache: morrow_core::scan_policy::TextCache::new(
                morrow_core::scan_policy::SCAN_CACHE_CAPACITY,
            ),
            reminders: Mutex::new(crate::reminders::ReminderBook::default()),
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
    /// How often the session list may be rewritten (#238 AC 6).
    saving: morrow_core::session_store::SaveThrottle,
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
            saving: morrow_core::session_store::SaveThrottle::new(),
        }
    }
}

/// `~/.morrow`, unless `MORROW_SESSIONS_DIR` (or the old name) says otherwise.
fn sessions_directory(home: &Path) -> std::path::PathBuf {
    let override_dir = std::env::var("MORROW_SESSIONS_DIR")
        .ok()
        .or_else(|| std::env::var("CODEISLAND_SESSIONS_DIR").ok());
    morrow_core::session_store::directory(override_dir.as_deref(), home)
}

/// #341: the feature starts **off**. Only a positive stored interval arms it;
/// 0 or a missing key leaves every timer silent.
pub const REMINDER_INTERVAL_KEY: &str = "followUpReminderIntervalSeconds";

pub fn reminder_interval(stored: Option<&Value>) -> Option<f64> {
    stored
        .and_then(|value| value.get(REMINDER_INTERVAL_KEY))
        .and_then(Value::as_f64)
        .filter(|seconds| *seconds > 0.0)
}

/// `$CODEX_HOME` (a leading `~` expanded) or `~/.codex`, where `session_index.jsonl`
/// keeps the thread names.
fn codex_home(home: &Path) -> std::path::PathBuf {
    match std::env::var("CODEX_HOME") {
        Ok(raw) if !raw.trim().is_empty() => morrow_core::session_store::expand(raw.trim(), home),
        _ => home.join(".codex"),
    }
}

/// The first and last 64 KB of a transcript: a rename lands at the end of the file, and a
/// session whose tail is one enormous tool result has its title only at the start.
fn read_head_and_tail(path: &Path) -> Option<(String, String)> {
    use std::io::{Read, Seek, SeekFrom};

    const WINDOW: u64 = 64 * 1024;
    let mut file = std::fs::File::open(path).ok()?;
    let size = file.metadata().ok()?.len();
    let window = size.min(WINDOW);
    let mut head = Vec::new();
    (&mut file).take(window).read_to_end(&mut head).ok()?;
    if size <= window {
        let text = String::from_utf8_lossy(&head).into_owned();
        return Some((text.clone(), text));
    }
    let mut tail = Vec::new();
    file.seek(SeekFrom::Start(size - window)).ok()?;
    file.read_to_end(&mut tail).ok()?;
    Some((
        String::from_utf8_lossy(&head).into_owned(),
        String::from_utf8_lossy(&tail).into_owned(),
    ))
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
            // Set by `Feed::snapshot`, which can see the scheduler.
            reminder_ids: Vec::new(),
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

    /// A feed whose reminders go to `outlet` instead of the desktop.
    #[cfg(test)]
    pub fn with_notify(outlet: NotifyOutlet) -> Self {
        Self {
            notify: Some(outlet),
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

    /// Post one due reminder through the outlet, or the platform's notifier.
    fn emit_notification(&self, title: &str, body: &str) {
        if let Some(outlet) = &self.notify {
            outlet(title, body);
            return;
        }
        let _ = crate::adapters::notify_send(title.to_owned(), body.to_owned());
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
                // #238 AC 9: a finished turn is nudged once, and its card is the hint.
                self.reminders
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .track_completion(
                        session_id,
                        completion.turn_failed,
                        morrow_core::reduce::Timestamp(now as f64 / 1000.0),
                    );
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

    /// The session behind card `id`, read while the card is still held. Anything
    /// that removes the card must capture this first: afterwards its id no
    /// longer resolves to a card, so there is no session to silence.
    fn held_card_session(state: &State, id: u64) -> Option<String> {
        state
            .permissions
            .cards()
            .into_iter()
            .find(|card| card.id == id)
            .map(|card| card.session_id)
    }

    /// Stop every reminder for the session a just-handled card belonged to.
    /// The scheduler rule existed in core but had no caller; this is it.
    fn silence_reminder_for(&self, session_id: &str) {
        self.reminders
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .silence_session(session_id);
    }

    /// The user's answer to card `id`; false if it is no longer held.
    pub fn answer(&self, id: u64, decision: Decision) -> bool {
        let (changed, session) = self.change(|state| {
            let session = Self::held_card_session(state, id);
            (state.permissions.answer(id, decision), session)
        });
        if changed {
            if let Some(session) = session {
                self.silence_reminder_for(&session);
            }
        }
        changed
    }

    /// #188: a question option was picked; its label is echoed as the answer.
    pub fn answer_question(&self, id: u64, pick: &str) -> bool {
        let (changed, session) = self.change(|state| {
            let session = Self::held_card_session(state, id);
            (state.permissions.answer_question(id, pick), session)
        });
        if changed {
            if let Some(session) = session {
                self.silence_reminder_for(&session);
            }
        }
        changed
    }

    /// #190: one answer per question of a wizard request, in question order.
    pub fn answer_questions(&self, id: u64, answers: Vec<String>) -> bool {
        let (changed, session) = self.change(|state| {
            let session = Self::held_card_session(state, id);
            (state.permissions.answer_questions(id, answers), session)
        });
        if changed {
            if let Some(session) = session {
                self.silence_reminder_for(&session);
            }
        }
        changed
    }

    /// #188: the question card was skipped, not answered.
    pub fn skip_question(&self, id: u64) -> bool {
        let (changed, session) = self.change(|state| {
            let session = Self::held_card_session(state, id);
            (state.permissions.skip_question(id), session)
        });
        if changed {
            if let Some(session) = session {
                self.silence_reminder_for(&session);
            }
        }
        changed
    }

    /// Card `id`'s hook is gone.
    pub fn drop_request(&self, id: u64) {
        let session = self.change(|state| {
            let session = Self::held_card_session(state, id);
            state.permissions.drop_request(id);
            session
        });
        if let Some(session) = session {
            self.silence_reminder_for(&session);
        }
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
    /// Put the session list a previous launch wrote back (#238 AC 6).
    ///
    /// Titles come from the store that named them: Claude's transcript carries them
    /// inline, Codex keeps a `session_index.jsonl` per root and `codex_index` is its
    /// text. Both are injected so this can be exercised without a real home.
    pub fn restore_sessions(&self, dir: &Path, codex_indexes: &[String]) -> usize {
        let rows = morrow_core::session_store::load(dir);
        if rows.is_empty() {
            return 0;
        }
        let mut state = self.lock();
        state
            .sessions
            .restore(rows, |row| match row.source.as_str() {
                "codex" => morrow_core::session_store::codex_thread_name_from_roots(
                    &codex_indexes.iter().map(String::as_str).collect::<Vec<_>>(),
                    &row.session_id,
                )
                .map(|title| morrow_core::session_store::ResolvedSessionTitle {
                    title,
                    source: morrow_core::session_store::SessionTitleSource::CodexThreadName,
                }),
                "claude" => {
                    let path = row.transcript_path.as_deref()?;
                    let (head, tail) = read_head_and_tail(Path::new(path))?;
                    morrow_core::session_store::claude_title(&head, &tail)
                }
                _ => None,
            })
    }

    /// One pass of the follow-up reminders (#238 AC 9): reconcile with the held
    /// requests, deliver what is due, and return the ids whose bell should stay up.
    ///
    /// `held_back` is the lock screen or a suspended probe (#197); quiet hours live in
    /// the frontend's settings and reach here through the same flag once they are read
    /// by the shell.
    pub fn tick_reminders(&self, held_back: bool, now_ms: u64) -> Vec<u64> {
        let held = self.lock().snapshot();
        let cards: Vec<crate::reminders::WaitingCard> = held
            .permissions
            .iter()
            .map(|card| crate::reminders::WaitingCard {
                id: card.id,
                session_id: card.session_id.clone(),
                question: card
                    .question
                    .as_ref()
                    .map(|question| question.question.clone())
                    .unwrap_or_default(),
                summary: card.summary.clone(),
                tool_name: card.tool_name.clone(),
            })
            .collect();
        let mut book = self
            .reminders
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let due = book.tick(
            &cards,
            held_back,
            morrow_core::reduce::Timestamp(now_ms as f64 / 1000.0),
        );
        for reminder in &due {
            // The island notifies from here, not the webview: a hidden panel has no
            // timer, and the reminder is about something the user cannot see.
            self.emit_notification(&reminder.title, &reminder.body);
        }
        book.hinted()
    }

    /// Arm the reminder interval: 0 or less turns the feature off.
    pub fn set_reminder_interval(&self, seconds: Option<f64>) {
        self.reminders
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .set_interval(seconds);
    }

    /// Write the session list out, at most once per save gap.
    pub fn save_sessions(&self, dir: &Path, now_ms: u64) -> bool {
        let mut state = self.lock();
        if !state.saving.due(now_ms) {
            return false;
        }
        let rows = state.sessions.persisted();
        morrow_core::session_store::save(dir, &rows);
        true
    }

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

    /// Asks the policy forever, woken by the stores themselves where the OS allows
    /// it (#286).
    ///
    /// The watcher is not a replacement for the clock: an append inside a file
    /// already on disk may not be reported (the fallback arm cannot see it, and
    /// inotify's `IN_MODIFY` is not a promise about a finished write). So the loop
    /// keeps its own tick, and a change only brings the next question forward.
    pub fn spawn_discovery(feed: Arc<Self>) {
        thread::Builder::new()
            .name("morrow-discovery".into())
            .spawn(move || {
                // #238 AC 6: what the last launch left behind, before the first
                // scan — a hook that arrives first wins, the file is only a memory.
                let home = std::path::PathBuf::from(feed.lock().sessions.home_directory());
                let sessions_dir = sessions_directory(&home);
                // Swift iterates every Codex root, not only $CODEX_HOME: a
                // thread renamed in another account's index keeps its title.
                let mut codex_roots = vec![codex_home(&home)];
                for entry in std::fs::read_dir(&home).into_iter().flatten().flatten() {
                    let path = entry.path();
                    let name = path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("");
                    if name.starts_with(".codex")
                        && path != codex_roots[0]
                        && path.join("session_index.jsonl").is_file()
                    {
                        codex_roots.push(path);
                    }
                }
                let codex_indexes: Vec<String> = codex_roots
                    .iter()
                    .filter_map(|root| {
                        std::fs::read_to_string(root.join("session_index.jsonl")).ok()
                    })
                    .collect();
                feed.restore_sessions(&sessions_dir, &codex_indexes);
                // #341: off until the user chooses an interval. The old
                // unconditional 60s default made the core's "off until set"
                // rule true only in unit tests.
                let stored = morrow_platform::settings::load(&sessions_dir).ok();
                feed.set_reminder_interval(reminder_interval(stored.as_ref()));
                let mut watcher = feed.watcher();
                loop {
                    // A watcher that reports a change wakes the loop early; one
                    // that does not, or one the OS refused, is a 250 ms sleep.
                    let moved = watcher
                        .as_mut()
                        .map(|watcher| watcher.wait(Duration::from_millis(250)))
                        .unwrap_or_else(|| {
                            thread::sleep(Duration::from_millis(250));
                            false
                        });
                    if !moved {
                        thread::sleep(Duration::from_millis(250));
                    }
                    let now = now_ms();
                    feed.tick_discovery(now);
                    feed.save_sessions(&sessions_dir, now);
                    // A locked screen suspends the probes (#197); a held-back reminder is
                    // reported once and caught up when it ends.
                    feed.tick_reminders(!feed.lock().probes_active, now);
                }
            })
            .expect("spawn the discovery thread");
    }

    /// An event source over every store a reader knows about, or `None` where the
    /// OS refuses — in which case the timer is the whole answer and discovery
    /// still works, only later.
    fn watcher(&self) -> Option<morrow_watch::Watcher> {
        let home = std::path::PathBuf::from(self.lock().sessions.home_directory());
        let config_dir = crate::hooks::claude_config_dir(
            &home,
            std::env::var("CLAUDE_CONFIG_DIR").ok().as_deref(),
        )
        .unwrap_or_else(|| home.join(".claude"));
        let roots = morrow_watch::default_roots(&home, &config_dir);
        morrow_watch::Watcher::new(&roots).ok()
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
        crate::discovery::scan_once(&config_dir, home, &self.scan_cache)
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
                        let channel = state.channel.clone();
                        let mut snapshot = state.snapshot();
                        drop(state);
                        snapshot.reminder_ids = feed.reminder_hints();
                        (snapshot, channel)
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
        let mut snapshot = self.lock().snapshot();
        snapshot.reminder_ids = self.reminder_hints();
        snapshot
    }

    /// The cards whose reminder already went out.
    fn reminder_hints(&self) -> Vec<u64> {
        self.reminders
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .hinted()
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

    #[test]
    fn reminders_start_off_until_an_interval_is_stored() {
        assert_eq!(reminder_interval(None), None);
        assert_eq!(reminder_interval(Some(&serde_json::json!({}))), None);
        assert_eq!(
            reminder_interval(Some(&serde_json::json!({ REMINDER_INTERVAL_KEY: 0 }))),
            None
        );
        assert_eq!(
            reminder_interval(Some(&serde_json::json!({ REMINDER_INTERVAL_KEY: 45 }))),
            Some(45.0)
        );
    }

    #[test]
    fn answering_a_card_silences_its_reminder() {
        let feed = Feed::default();
        feed.set_reminder_interval(Some(60.0));
        let (id, _receiver) = feed.hold(&hook(
            r#"{"session_id":"s1","hook_event_name":"PermissionRequest","tool_name":"Bash"}"#,
        ));
        let now = 1_000_000u64;
        assert!(
            feed.tick_reminders(false, now).is_empty(),
            "timer just started"
        );
        assert_eq!(
            feed.tick_reminders(false, now + 60_000).len(),
            1,
            "the wait is due"
        );
        assert!(feed.answer(id, Decision::Allow));
        assert!(
            feed.tick_reminders(false, now + 120_000).is_empty(),
            "an answered request must not chime again"
        );
    }

    /// Answering a card silences its whole session, not just the card that was
    /// removed. A second card still held for that session is the observable: if
    /// the session is not silenced, its reminder nags again a full interval later.
    /// Looking the session up by the answered card's id after the decision cannot
    /// work — the card is already out of `held`.
    #[test]
    fn answering_a_card_silences_later_nags_for_its_session() {
        let nags = Arc::new(Mutex::new(Vec::new()));
        let recorder = {
            let nags = Arc::clone(&nags);
            Arc::new(move |_title: &str, body: &str| nags.lock().unwrap().push(body.to_owned()))
        };
        let feed = Feed::with_notify(recorder);
        feed.set_reminder_interval(Some(60.0));

        // One session, two held requests: an approval and a question. Both carry
        // an active reminder once the interval passes; the approval is answered.
        let (approval, approval_rx) = feed.hold(&hook(
            r#"{"session_id":"s1","hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{"command":"rm -rf build"}}"#,
        ));
        let (question, question_rx) = feed.hold(&hook(
            r#"{"session_id":"s1","hook_event_name":"PermissionRequest","tool_input":{"questions":[{"question":"Which DB?","header":"DB","options":[{"label":"Postgres","description":""}],"multiSelect":false}]}}"#,
        ));
        let _keep_receivers = (approval_rx, question_rx);

        feed.tick_reminders(false, 0); // sync the requests; nothing due yet
        assert!(nags.lock().unwrap().is_empty());
        feed.tick_reminders(false, 60_000);
        assert_eq!(
            nags.lock().unwrap().len(),
            2,
            "both cards carry an active reminder"
        );
        let snapshot = feed.snapshot();
        assert!(snapshot.reminder_ids.contains(&approval));
        assert!(snapshot.reminder_ids.contains(&question));

        assert!(feed.answer(approval, Decision::Allow));
        nags.lock().unwrap().clear();

        // The question is still held and would be due again at 120 s. The answer
        // silenced the session, so there is no later nag.
        feed.tick_reminders(false, 120_000);
        assert!(
            nags.lock().unwrap().is_empty(),
            "no reminder may nag after the session's card was answered"
        );
    }

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
    /// #238 AC 6: the title comes from the store that names the session, not from the
    /// file — Claude's transcript carries it, Codex's index does.
    #[test]
    fn a_restored_session_asks_its_own_store_for_the_title() {
        use morrow_core::session_store;

        let root = std::env::temp_dir().join(format!(
            "morrow-titles-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let transcript = root.join("claude.jsonl");
        fs::write(
            &transcript,
            "{\"type\":\"ai-title\",\"aiTitle\":\"Suggested\"}\n{\"type\":\"custom-title\",\"customTitle\":\"Typed by me\"}\n",
        )
        .unwrap();

        let row = |id: &str, source: &str, path: Option<String>| session_store::PersistedSession {
            session_id: id.into(),
            source: source.into(),
            cwd: Some("/work/proj".into()),
            transcript_path: path,
            start_time: "2026-01-02T03:04:05Z".into(),
            last_activity: "2026-01-02T03:04:05Z".into(),
            ..session_store::PersistedSession::default()
        };
        session_store::save(
            &root,
            &[
                row(
                    "claude-1",
                    "claude",
                    Some(transcript.to_string_lossy().into_owned()),
                ),
                row("codex-1", "codex", None),
                // A provider that names its own sessions is not guessed at.
                row("cursor-1", "cursor", None),
            ],
        );
        let index = "{\"id\":\"codex-1\",\"thread_name\":\"Fix the flaky test\",\"updated_at\":\"2026-01-02T03:00:00Z\"}\n";

        let feed = Feed::default();
        assert_eq!(feed.restore_sessions(&root, &[index.to_owned()]), 3);
        let titles: Vec<(String, Option<String>)> = feed
            .lock()
            .sessions
            .persisted()
            .into_iter()
            .map(|row| (row.session_id, row.session_title))
            .collect();
        let title = |id: &str| {
            titles
                .iter()
                .find(|(row, _)| row == id)
                .and_then(|(_, title)| title.clone())
        };
        assert_eq!(title("claude-1"), Some("Typed by me".to_owned()));
        assert_eq!(title("codex-1"), Some("Fix the flaky test".to_owned()));
        assert_eq!(title("cursor-1"), None);

        // The save gap holds: a second call inside it writes nothing.
        assert!(feed.save_sessions(&root, 1_000));
        assert!(!feed.save_sessions(&root, 1_500));
        assert!(feed.save_sessions(&root, 3_000));

        let _ = fs::remove_dir_all(&root);
    }
}
