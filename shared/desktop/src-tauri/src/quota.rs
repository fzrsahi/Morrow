//! #308 — the user-visible Claude quota slice.
//!
//! The model, parser and refresh policy live in `morrow_core::quota`; the
//! credential file and the usage request live in `morrow_platform::quota`.
//! This module is the IPC boundary: it turns their result into a view the
//! panel can draw, and keeps the one fetcher (and its last view) in Tauri state.
//!
//! Two rules matter here:
//! - No bearer token, and nothing derived from one, is ever part of a view.
//!   Every reason string is a fixed sentence or a string the platform has
//!   already scrubbed, and [`scrub`] is a second line of defence.
//! - A non-ok view has no `limits` at all. The panel must never invent a 0%
//!   bar for a state where we have no numbers.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use morrow_core::quota::{
    ChipMode, Credential, Kind, Level, QuotaError, Snapshot as CoreSnapshot, Tone,
};
use morrow_core::reduce::Timestamp;
use morrow_platform::quota::{FetchAction, QuotaFetcher};
use morrow_platform::runner::SystemRunner;
use morrow_platform::settings;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, State};

use crate::HooksSite;
use crate::settings::SETTINGS_CHANGED_EVENT;

/// Same key (and raw values) as Swift `SettingsKey.claudeQuotaChip`.
const CHIP_MODE_KEY: &str = "claudeQuotaChip";

const CREDENTIALS_UNREADABLE: &str = "Claude credentials could not be read.";
const NOT_CONNECTED: &str = "Sign in to Claude Code once to share your plan limits.";
const OFF: &str = "Plan limits are turned off in Settings.";
const SERVICE_ERROR: &str = "Claude's usage service returned an error.";
const UNEXPECTED_RESPONSE: &str = "Claude's usage service returned an unexpected response.";
const UNAUTHORIZED: &str = "Claude sign-in was rejected. Sign in again in Claude Code.";
const RATE_LIMITED: &str = "Claude is rate limiting requests. Try again shortly.";

/// The five wire states. `serde` gives exactly the strings the panel expects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotaStatus {
    Off,
    Unavailable,
    NotConnected,
    Unauthorized,
    RateLimited,
    Ok,
}

/// One window's numbers. Every field comes from the core model or an enum,
/// except `label`, which is a fixed phrase plus the server's model display name.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct QuotaLimitView {
    pub kind: String,
    pub percent: f64,
    pub level: String,
    pub pace_tone: Option<String>,
    /// Seconds since 1970, like the rest of the desktop DTOs.
    pub resets_at: Option<f64>,
    pub label: String,
}

/// What `quota_snapshot` sends to the webview. `limits` is `None` unless we
/// really have numbers — the card checks this before drawing a bar.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct QuotaSnapshotView {
    pub status: QuotaStatus,
    pub reason: String,
    pub limits: Option<Vec<QuotaLimitView>>,
}

impl QuotaSnapshotView {
    fn problem(status: QuotaStatus, reason: impl Into<String>) -> Self {
        Self {
            status,
            reason: reason.into(),
            limits: None,
        }
    }
}

/// The seam the IPC boundary needs: the command can be exercised with a fake
/// source instead of a real HOME and a real `curl` (#308 review).
pub trait QuotaSource: Send {
    fn read_credentials(&self) -> Result<Option<Credential>, String>;
    fn fetch_if_due(
        &mut self,
        now: Timestamp,
        credential: &Credential,
        wants_live: bool,
    ) -> FetchAction;
}

impl<R: morrow_platform::runner::Runner + Send> QuotaSource for QuotaFetcher<R> {
    fn read_credentials(&self) -> Result<Option<Credential>, String> {
        QuotaFetcher::read_credentials(self)
    }

    fn fetch_if_due(
        &mut self,
        now: Timestamp,
        credential: &Credential,
        wants_live: bool,
    ) -> FetchAction {
        QuotaFetcher::fetch_if_due(self, now, credential, wants_live)
    }
}

/// The fetcher owns the scheduler; the last view lets a throttled call repeat
/// the numbers we already have instead of showing a spinner or a fake zero.
pub struct QuotaState {
    source: Box<dyn QuotaSource>,
    last: Option<QuotaSnapshotView>,
}

pub type ManagedQuota = Mutex<QuotaState>;

/// Build the managed state from the real HOME already resolved for the hooks.
pub fn new_state(site: &HooksSite) -> ManagedQuota {
    Mutex::new(QuotaState {
        source: Box::new(QuotaFetcher::new(SystemRunner, site.paths.home.clone())),
        last: None,
    })
}

#[cfg(test)]
pub fn state_with_source(source: Box<dyn QuotaSource>) -> ManagedQuota {
    Mutex::new(QuotaState { source, last: None })
}

/// Wall clock, read only at the IPC boundary. The core and the platform are
/// deliberately clock-free.
fn clock_now() -> Timestamp {
    Timestamp(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs_f64())
            .unwrap_or(0.0),
    )
}

/// Remove a known secret from text that is about to be shown or logged. The
/// platform already does this for transport errors; doing it here too means a
/// future change there cannot leak a token through this command.
fn scrub(text: &str, token: &str) -> String {
    if token.is_empty() {
        text.to_owned()
    } else {
        text.replace(token, "<redacted>")
    }
}

fn level_name(level: Level) -> &'static str {
    match level {
        Level::Normal => "normal",
        Level::Warning => "warning",
        Level::Critical => "critical",
    }
}

fn tone_name(tone: Tone) -> &'static str {
    match tone {
        Tone::Neutral => "neutral",
        Tone::Ahead => "ahead",
        Tone::Behind => "behind",
    }
}

/// Swift `ClaudeQuotaFormat.label`: short names for the three windows.
fn label(limit: &morrow_core::quota::Limit) -> String {
    match limit.kind {
        Kind::Session => "5h".to_owned(),
        Kind::WeeklyAll => "Week".to_owned(),
        Kind::WeeklyScoped => limit
            .scope_label
            .clone()
            .unwrap_or_else(|| "Week".to_owned()),
    }
}

/// Pure core snapshot → wire view. No clock, network or secret is read here;
/// the caller supplies both the snapshot and the instant for pace.
fn view_from_snapshot(snapshot: &CoreSnapshot, now: Timestamp) -> QuotaSnapshotView {
    let limits = snapshot
        .ordered()
        .into_iter()
        .map(|limit| QuotaLimitView {
            kind: limit.kind.raw_value().to_owned(),
            percent: limit.percent,
            level: level_name(limit.level()).to_owned(),
            pace_tone: limit.pace(now).map(|pace| tone_name(pace.tone).to_owned()),
            resets_at: limit.resets_at.map(|at| at.0),
            label: label(&limit),
        })
        .collect();
    QuotaSnapshotView {
        status: QuotaStatus::Ok,
        reason: String::new(),
        limits: Some(limits),
    }
}

/// A core error becomes a fixed sentence — never the error's own text, and
/// never an HTTP body. Only the enum variant decides what the user is told.
fn view_from_error(error: &QuotaError) -> QuotaSnapshotView {
    match error {
        QuotaError::Unauthorized => {
            QuotaSnapshotView::problem(QuotaStatus::Unauthorized, UNAUTHORIZED)
        }
        QuotaError::RateLimited => {
            QuotaSnapshotView::problem(QuotaStatus::RateLimited, RATE_LIMITED)
        }
        QuotaError::Http(_) => QuotaSnapshotView::problem(QuotaStatus::Unavailable, SERVICE_ERROR),
        QuotaError::NotJson | QuotaError::NoLimits => {
            QuotaSnapshotView::problem(QuotaStatus::Unavailable, UNEXPECTED_RESPONSE)
        }
    }
}

fn mode_from_settings(stored: &Value) -> ChipMode {
    stored
        .get(CHIP_MODE_KEY)
        .and_then(Value::as_str)
        .and_then(ChipMode::from_raw_value)
        .unwrap_or(ChipMode::Auto)
}

/// The collapsed-island choice, read from the same document as every other
/// setting. A missing or malformed value means Swift's `auto` default.
#[tauri::command]
pub fn quota_settings_load(site: State<'_, HooksSite>) -> Result<String, String> {
    let stored = settings::load(&site.paths.codeisland_dir).map_err(|error| error.to_string())?;
    Ok(mode_from_settings(&stored).raw_value().to_owned())
}

/// Write one top-level key and broadcast the document, like `settings_set`.
#[tauri::command]
pub fn quota_settings_set(
    app: AppHandle,
    site: State<'_, HooksSite>,
    mode: String,
) -> Result<bool, String> {
    let mode =
        ChipMode::from_raw_value(&mode).ok_or_else(|| format!("unknown quota mode: {mode}"))?;
    let changed = settings::set(
        &site.paths.codeisland_dir,
        CHIP_MODE_KEY,
        &Value::String(mode.raw_value().to_owned()),
    )
    .map_err(|error| error.to_string())?;
    if let Ok(stored) = settings::load(&site.paths.codeisland_dir) {
        let _ = app.emit(SETTINGS_CHANGED_EVENT, stored);
    }
    Ok(changed)
}

/// Fetch (or repeat the last view) for the panel. Never returns a token.
#[tauri::command]
pub fn quota_snapshot(
    site: State<'_, HooksSite>,
    state: State<'_, ManagedQuota>,
) -> Result<QuotaSnapshotView, String> {
    let stored = settings::load(&site.paths.codeisland_dir).map_err(|error| error.to_string())?;
    let mode = mode_from_settings(&stored);
    let mut state = state
        .lock()
        .map_err(|_| "quota state is unavailable".to_owned())?;
    Ok(snapshot_with(&mut state, clock_now(), mode))
}

/// The command's whole decision, with the clock injected. Tests drive this with
/// a fake `QuotaSource`; no HOME, network or Tauri runtime is involved.
fn snapshot_with(state: &mut QuotaState, now: Timestamp, mode: ChipMode) -> QuotaSnapshotView {
    if mode == ChipMode::Off {
        // A setting the shell never reads is a lie. "off" must change what the
        // command returns, not only what Settings persists.
        return scrub_view(QuotaSnapshotView::problem(QuotaStatus::Off, OFF), "");
    }
    let credential = match state.source.read_credentials() {
        Ok(Some(credential)) => credential,
        Ok(None) => {
            return scrub_view(
                QuotaSnapshotView::problem(QuotaStatus::NotConnected, NOT_CONNECTED),
                "",
            );
        }
        // The reader's own error names a path, never a token; keep the
        // sentence fixed anyway so nothing from disk reaches the webview.
        Err(_) => {
            return scrub_view(
                QuotaSnapshotView::problem(QuotaStatus::Unavailable, CREDENTIALS_UNREADABLE),
                "",
            );
        }
    };

    let token = credential.access_token.as_str();
    let view = match state.source.fetch_if_due(now, &credential, true) {
        // Throttled or paused: the numbers on screen are the last ones we got.
        FetchAction::Skipped => {
            return scrub_view(
                state.last.clone().unwrap_or_else(|| {
                    QuotaSnapshotView::problem(QuotaStatus::Unavailable, SERVICE_ERROR)
                }),
                token,
            );
        }
        FetchAction::Fetched(Ok(snapshot)) => {
            let view = view_from_snapshot(&snapshot, now);
            state.last = Some(view.clone());
            view
        }
        // A transport/provider failure must not erase the last real numbers.
        FetchAction::Fetched(Err(error)) => return scrub_view(view_from_error(&error), token),
        FetchAction::Error(error) => {
            return scrub_view(
                QuotaSnapshotView::problem(QuotaStatus::Unavailable, error),
                token,
            );
        }
    };
    scrub_view(view, token)
}

/// Defence in depth: no reason string leaving this boundary may contain a
/// bearer token, whichever branch produced it.
fn scrub_view(mut view: QuotaSnapshotView, token: &str) -> QuotaSnapshotView {
    view.reason = scrub(&view.reason, token);
    view
}

#[cfg(test)]
mod tests {
    use morrow_core::quota::Limit;

    use super::*;

    const TOKEN: &str = "sk-ant-oat01-very-secret";

    fn now() -> Timestamp {
        Timestamp(1_800_000_000.0)
    }

    fn at(seconds: f64) -> Timestamp {
        Timestamp(now().0 + seconds)
    }

    fn limit(kind: Kind, severity: &str, percent: f64, resets_in: Option<f64>) -> Limit {
        Limit::new(kind, percent, severity, resets_in.map(at), None)
    }

    fn json(view: &QuotaSnapshotView) -> String {
        serde_json::to_string(view).expect("a serializable view")
    }

    /// A source with no HOME and no network: the same seam the command uses.
    struct FakeSource {
        credential: Option<Credential>,
        action: FetchAction,
        calls: std::cell::Cell<usize>,
    }

    impl FakeSource {
        fn credential() -> Credential {
            Credential {
                access_token: TOKEN.to_owned(),
                subscription_type: Some("max".to_owned()),
                expires_at: None,
            }
        }
    }

    impl QuotaSource for FakeSource {
        fn read_credentials(&self) -> Result<Option<Credential>, String> {
            Ok(self.credential.clone())
        }

        fn fetch_if_due(
            &mut self,
            _now: Timestamp,
            _credential: &Credential,
            _wants_live: bool,
        ) -> FetchAction {
            self.calls.set(self.calls.get() + 1);
            // A fake cannot move a stored action out; rebuild the equivalent.
            match &self.action {
                FetchAction::Skipped => FetchAction::Skipped,
                FetchAction::Fetched(Ok(snapshot)) => FetchAction::Fetched(Ok(snapshot.clone())),
                FetchAction::Fetched(Err(error)) => FetchAction::Fetched(Err(*error)),
                FetchAction::Error(error) => FetchAction::Error(error.clone()),
            }
        }
    }

    #[test]
    fn off_mode_changes_the_command_result_and_a_failure_keeps_the_last_numbers() {
        // #308 review: selecting `off` in Settings must be visible to the
        // command, and a transient failure must not erase a good cached view.
        let source = FakeSource {
            credential: Some(FakeSource::credential()),
            action: FetchAction::Fetched(Ok(CoreSnapshot {
                limits: vec![limit(Kind::Session, "normal", 12.0, None)],
                fetched_at: now(),
            })),
            calls: std::cell::Cell::new(0),
        };
        let managed = state_with_source(Box::new(source));
        let mut state = managed.lock().expect("state");
        assert_eq!(
            snapshot_with(&mut state, now(), ChipMode::Off).status,
            QuotaStatus::Off
        );

        let view = snapshot_with(&mut state, now(), ChipMode::Auto);
        assert_eq!(view.status, QuotaStatus::Ok);
        drop(state);

        // Rebuild the same state with a failing source while keeping the cached
        // view: a provider error must not clobber the last real numbers.
        let mut state = managed.lock().expect("state");
        state.source = Box::new(FakeSource {
            credential: Some(FakeSource::credential()),
            action: FetchAction::Error("sk-ant-oat01-very-secret failed".to_owned()),
            calls: std::cell::Cell::new(0),
        });
        let view = snapshot_with(&mut state, at(700.0), ChipMode::Auto);
        assert_eq!(view.status, QuotaStatus::Unavailable);
        assert!(!view.reason.contains(TOKEN), "token scrubbed on every path");
        assert!(state.last.is_some(), "the good snapshot survives a failure");
    }

    #[test]
    fn quota_snapshot_decision_runs_with_an_injected_fetcher() {
        // #308 review: the command's decision is exercised end to end with a
        // fake source — no HOME, no curl, no Tauri runtime.
        let source = FakeSource {
            credential: Some(FakeSource::credential()),
            action: FetchAction::Fetched(Ok(CoreSnapshot {
                limits: vec![limit(Kind::Session, "warning", 42.0, Some(1800.0))],
                fetched_at: now(),
            })),
            calls: std::cell::Cell::new(0),
        };
        let managed = state_with_source(Box::new(source));
        let mut state = managed.lock().expect("state");
        let view = snapshot_with(&mut state, now(), ChipMode::Auto);
        assert_eq!(view.status, QuotaStatus::Ok);
        assert_eq!(view.limits.as_ref().map(Vec::len), Some(1));
        assert_eq!(view.limits.as_ref().unwrap()[0].percent, 42.0);
        let wire = json(&view);
        assert!(!wire.contains(TOKEN), "token leaked into the view");
        drop(state);

        let managed = state_with_source(Box::new(FakeSource {
            credential: None,
            action: FetchAction::Skipped,
            calls: std::cell::Cell::new(0),
        }));
        let mut state = managed.lock().expect("state");
        let view = snapshot_with(&mut state, now(), ChipMode::Auto);
        assert_eq!(view.status, QuotaStatus::NotConnected);
        assert!(view.limits.is_none(), "no numbers without a credential");
        assert!(!json(&view).contains(TOKEN));
    }

    fn stored_mode(raw: &str) -> Value {
        let mut map = serde_json::Map::new();
        map.insert(CHIP_MODE_KEY.to_owned(), Value::String(raw.to_owned()));
        Value::Object(map)
    }

    #[test]
    fn a_snapshot_maps_every_level_and_only_real_pace() {
        let scoped = Limit::new(
            Kind::WeeklyScoped,
            100.0,
            // An unknown server string is critical; the token in it must not
            // survive the conversion.
            "exceeded sk-ant-oat01-very-secret",
            None,
            Some("Fable".to_owned()),
        );
        let snapshot = CoreSnapshot::new(
            vec![
                // 72% with 3h of the 5h window left → 40% elapsed → ahead.
                limit(Kind::Session, "normal", 72.0, Some(3.0 * 3600.0)),
                // Warning severity, no reset time → no pace.
                limit(Kind::WeeklyAll, "elevated", 70.0, None),
                scoped,
            ],
            now(),
        );

        let view = view_from_snapshot(&snapshot, now());
        assert_eq!(view.status, QuotaStatus::Ok);
        assert_eq!(view.reason, "");
        let limits = view.limits.as_ref().expect("ok carries limits");
        assert_eq!(
            limits.iter().map(|l| l.kind.as_str()).collect::<Vec<_>>(),
            ["session", "weekly_all", "weekly_scoped"]
        );
        assert_eq!(
            limits.iter().map(|l| l.level.as_str()).collect::<Vec<_>>(),
            ["normal", "warning", "critical"]
        );
        assert_eq!(
            limits.iter().map(|l| l.label.as_str()).collect::<Vec<_>>(),
            ["5h", "Week", "Fable"]
        );
        let paces = limits
            .iter()
            .map(|l| l.pace_tone.as_deref())
            .collect::<Vec<_>>();
        assert_eq!(paces, [Some("ahead"), None, None]);
        assert_eq!(limits[0].percent, 72.0);
        assert!((limits[0].resets_at.unwrap() - (now().0 + 3.0 * 3600.0)).abs() < 0.001);

        let wire = json(&view);
        assert!(!wire.contains(TOKEN), "{wire}");
        assert!(wire.contains("\"critical\""), "{wire}");
    }

    #[test]
    fn non_ok_states_have_no_limits_and_no_numbers() {
        for (status, reason) in [
            (QuotaStatus::Unavailable, CREDENTIALS_UNREADABLE),
            (QuotaStatus::NotConnected, NOT_CONNECTED),
            (QuotaStatus::Unauthorized, UNAUTHORIZED),
            (QuotaStatus::RateLimited, RATE_LIMITED),
        ] {
            let view = QuotaSnapshotView::problem(status, reason);
            let wire = json(&view);
            assert!(view.limits.is_none(), "{status:?}");
            assert!(!wire.contains("percent"), "{wire}");
            assert!(!wire.contains(TOKEN), "{wire}");
        }
    }

    #[test]
    fn error_views_are_fixed_sentences() {
        for (error, status) in [
            (QuotaError::Unauthorized, QuotaStatus::Unauthorized),
            (QuotaError::RateLimited, QuotaStatus::RateLimited),
            (QuotaError::Http(503), QuotaStatus::Unavailable),
            (QuotaError::NotJson, QuotaStatus::Unavailable),
            (QuotaError::NoLimits, QuotaStatus::Unavailable),
        ] {
            let view = view_from_error(&error);
            assert_eq!(view.status, status, "{error:?}");
            assert!(view.limits.is_none(), "{error:?}");
            assert!(view.reason.contains("Claude"), "{error:?}");
            assert!(!json(&view).contains(TOKEN), "{error:?}");
        }
    }

    #[test]
    fn scrub_removes_a_token_echoed_by_a_transport_error() {
        let message = format!("curl failed with header Authorization: Bearer {TOKEN}");
        let safe = scrub(&message, TOKEN);
        assert!(!safe.contains(TOKEN), "{safe}");
        assert!(safe.contains("<redacted>"), "{safe}");
        assert_eq!(scrub("no token here", ""), "no token here");
    }

    #[test]
    fn chip_mode_defaults_to_auto_and_round_trips_raw_values() {
        assert_eq!(mode_from_settings(&Value::Null), ChipMode::Auto);
        assert_eq!(
            mode_from_settings(&stored_mode("weeklyAll")),
            ChipMode::WeeklyAll
        );
        assert_eq!(mode_from_settings(&stored_mode("bogus")), ChipMode::Auto);
        assert_eq!(mode_from_settings(&stored_mode("off")), ChipMode::Off);
        for mode in ChipMode::ALL {
            assert_eq!(ChipMode::from_raw_value(mode.raw_value()), Some(mode));
        }
    }
}
