//! Claude plan-limit core: the limit model, the `/api/oauth/usage` response
//! parser and the refresh policy (`ClaudeQuota.swift`, `ClaudeQuotaClient.swift`,
//! `ClaudeQuotaScheduler.swift` in `macos/Sources/MorrowCore`). Fixture values
//! come from `macos/Tests/MorrowCoreTests/ClaudeQuota{,Scheduler}Tests.swift`.
//!
//! Pure: no HTTP, no keychain, no wall clock. The caller hands in the response
//! bytes, its status and every instant. Reset dates are read with
//! [`crate::time::parse_iso8601`], the same reader `ClaudeUsageScanner` uses —
//! Swift's `ClaudeQuotaSnapshot.date(_:)` tries a fractional and a plain
//! `ISO8601DateFormatter`, both of which that parser covers.
//!
//! The network half (bearer header, ephemeral session, redirect refusal) and
//! the credential read stay in the platform shells; what is here is the value
//! semantics they interpret.

use std::fmt;

use serde::Serialize;
use serde::ser::SerializeStruct;
use serde_json::Value;

use crate::reduce::Timestamp;
use crate::time::parse_iso8601;

/// `ClaudeQuotaLimit.Kind`: one rate-limit window from Anthropic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Rolling 5-hour window.
    Session,
    /// 7-day window across all models.
    WeeklyAll,
    /// 7-day window scoped to one model.
    WeeklyScoped,
}

impl Kind {
    /// Swift `Kind.allCases`, in display order (5h, weekly, weekly/model).
    pub const ALL: [Self; 3] = [Self::Session, Self::WeeklyAll, Self::WeeklyScoped];

    /// Swift `Kind.rawValue`: "session", "weekly_all", "weekly_scoped".
    pub fn raw_value(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::WeeklyAll => "weekly_all",
            Self::WeeklyScoped => "weekly_scoped",
        }
    }

    /// Swift `Kind(rawValue:)`.
    pub fn from_raw_value(raw: &str) -> Option<Self> {
        match raw {
            "session" => Some(Self::Session),
            "weekly_all" => Some(Self::WeeklyAll),
            "weekly_scoped" => Some(Self::WeeklyScoped),
            _ => None,
        }
    }

    /// Swift `Kind.windowSeconds` (`TimeInterval`).
    pub fn window_seconds(self) -> f64 {
        match self {
            Self::Session => 5.0 * 3600.0,
            Self::WeeklyAll | Self::WeeklyScoped => 7.0 * 86_400.0,
        }
    }
}

/// `ClaudeQuotaLimit.Pace.Tone`: how usage compares with spending the window
/// evenly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    Neutral,
    Ahead,
    Behind,
}

/// `ClaudeQuotaLimit.Pace`: the pace readout for a live window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pace {
    /// Used share minus elapsed share, in percentage points (`+` = burning
    /// faster than even pace).
    pub points: f64,
    /// `points` as window time: how much budget is used ahead of schedule.
    pub duration: f64,
    /// Percent the window reaches at reset if the current rate holds.
    pub projected_percent: f64,
    /// Time until 100% at the current rate; `None` if the window resets first
    /// or is already at its limit.
    pub exhausts_in: Option<f64>,
    pub tone: Tone,
}

/// `ClaudeQuotaLimit.Level`: severity bucket for colouring.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Normal,
    Warning,
    Critical,
}

/// `ClaudeQuotaLimit`: one window's numbers from Anthropic's usage endpoint.
#[derive(Clone, Debug, PartialEq)]
pub struct Limit {
    pub kind: Kind,
    /// 0…100; may exceed 100 when the account is over its limit.
    pub percent: f64,
    /// Server-side severity, e.g. "normal" / "warning" / "critical". Free-form.
    pub severity: String,
    pub resets_at: Option<Timestamp>,
    /// Model display name for [`Kind::WeeklyScoped`], otherwise `None`.
    pub scope_label: Option<String>,
}

impl Limit {
    /// Swift `paceMinElapsed`: below this share elapsed the readout is hidden.
    pub const PACE_MIN_ELAPSED: f64 = 0.10;
    /// Swift `paceNeutralPoints`: within this many points, pace is neutral.
    pub const PACE_NEUTRAL_POINTS: f64 = 5.0;

    pub fn new(
        kind: Kind,
        percent: f64,
        severity: &str,
        resets_at: Option<Timestamp>,
        scope_label: Option<String>,
    ) -> Self {
        Self {
            kind,
            percent,
            severity: severity.to_owned(),
            resets_at,
            scope_label,
        }
    }

    /// Swift `elapsedFraction(now:)`: share of the window already gone,
    /// clamped to 0…1. `None` when the server gave no reset time.
    pub fn elapsed_fraction(&self, now: Timestamp) -> Option<f64> {
        let resets_at = self.resets_at?;
        let remaining = resets_at.0 - now.0;
        let elapsed = 1.0 - remaining / self.kind.window_seconds();
        Some(elapsed.clamp(0.0, 1.0))
    }

    /// Swift `hasReset(now:)`: the window rolled over, so these numbers belong
    /// to the previous one.
    pub fn has_reset(&self, now: Timestamp) -> bool {
        self.resets_at.is_some_and(|resets_at| resets_at <= now)
    }

    /// Swift `paceDelta(now:)`: used share minus elapsed share. Falls back to
    /// the used share with no reset time so windows stay comparable.
    pub fn pace_delta(&self, now: Timestamp) -> f64 {
        let used = self.percent / 100.0;
        match self.elapsed_fraction(now) {
            Some(elapsed) => used - elapsed,
            None => used,
        }
    }

    /// Swift `pace(now:)`: `None` without a reset time, this early in the
    /// window, or once it has reset.
    pub fn pace(&self, now: Timestamp) -> Option<Pace> {
        if self.resets_at.is_none() || self.has_reset(now) {
            return None;
        }
        let elapsed = self.elapsed_fraction(now)?;
        if elapsed < Self::PACE_MIN_ELAPSED {
            return None;
        }
        let points = self.percent - elapsed * 100.0;
        // Swift `Double.rounded()` is `.toNearestOrEven`.
        let rounded = points.round_ties_even();
        let tone = if rounded.abs() <= Self::PACE_NEUTRAL_POINTS {
            Tone::Neutral
        } else if rounded > 0.0 {
            Tone::Ahead
        } else {
            Tone::Behind
        };
        let projected_percent = self.percent / elapsed;
        let exhausts_in = if self.percent < 100.0 && projected_percent > 100.0 {
            // Percent per second so far; the rest of the budget at that rate.
            let rate = self.percent / (elapsed * self.kind.window_seconds());
            Some((100.0 - self.percent) / rate)
        } else {
            None
        };
        Some(Pace {
            points,
            duration: points / 100.0 * self.kind.window_seconds(),
            projected_percent,
            exhausts_in,
            tone,
        })
    }

    /// Swift `isOverLimit`.
    pub fn is_over_limit(&self) -> bool {
        self.percent >= 100.0
    }

    /// Swift `level`: tolerant of unknown server strings.
    pub fn level(&self) -> Level {
        if self.is_over_limit() {
            return Level::Critical;
        }
        match self.severity.to_lowercase().as_str() {
            "normal" | "" => Level::Normal,
            "warning" | "warn" | "elevated" => Level::Warning,
            _ => Level::Critical,
        }
    }
}

/// `ClaudeQuotaSnapshot`: the limits one fetch returned.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub limits: Vec<Limit>,
    pub fetched_at: Timestamp,
}

/// Swift `ClaudeQuotaSnapshot.ParseError`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// The body was not a JSON object.
    NotJson,
    /// The object carried no usable limit.
    NoLimits,
}

impl Snapshot {
    pub fn new(limits: Vec<Limit>, fetched_at: Timestamp) -> Self {
        Self { limits, fetched_at }
    }

    pub fn is_empty(&self) -> bool {
        self.limits.is_empty()
    }

    pub fn limit(&self, kind: Kind) -> Option<&Limit> {
        self.limits.iter().find(|limit| limit.kind == kind)
    }

    /// Swift `ordered`: display order, 5h → weekly → weekly (model).
    pub fn ordered(&self) -> Vec<Limit> {
        Kind::ALL
            .iter()
            .filter_map(|kind| self.limit(*kind).cloned())
            .collect()
    }

    /// Swift `ClaudeQuotaSnapshot.parse(_:fetchedAt:)`. Prefers the normalised
    /// `limits[]` array, then falls back to the legacy top-level
    /// `five_hour` / `seven_day*` objects; first entry per kind wins.
    pub fn parse(data: &[u8], fetched_at: Timestamp) -> Result<Self, ParseError> {
        let value: Value = serde_json::from_slice(data).map_err(|_| ParseError::NotJson)?;
        let Some(obj) = value.as_object() else {
            return Err(ParseError::NotJson);
        };

        let mut limits: Vec<Limit> = Vec::new();
        if let Some(raw) = obj.get("limits").and_then(Value::as_array) {
            for item in raw {
                let Some(item) = item.as_object() else {
                    continue;
                };
                let Some(kind) = item
                    .get("kind")
                    .and_then(Value::as_str)
                    .and_then(Kind::from_raw_value)
                else {
                    continue;
                };
                let Some(percent) = number(item.get("percent")) else {
                    continue;
                };
                // Swift: `model["display_name"] as? String ?? model["id"] as? String`.
                let scope_label = item
                    .get("scope")
                    .and_then(Value::as_object)
                    .and_then(|scope| scope.get("model"))
                    .and_then(Value::as_object)
                    .and_then(|model| {
                        model
                            .get("display_name")
                            .and_then(Value::as_str)
                            .or_else(|| model.get("id").and_then(Value::as_str))
                    })
                    .map(str::to_owned);
                limits.push(Limit {
                    kind,
                    percent,
                    severity: item
                        .get("severity")
                        .and_then(Value::as_str)
                        .unwrap_or("normal")
                        .to_owned(),
                    resets_at: date(item.get("resets_at")),
                    scope_label,
                });
            }
        }

        if limits.is_empty() {
            for (key, kind, label) in [
                ("five_hour", Kind::Session, None),
                ("seven_day", Kind::WeeklyAll, None),
                ("seven_day_opus", Kind::WeeklyScoped, Some("Opus")),
            ] {
                if let Some(limit) = legacy(obj, key, kind, label) {
                    limits.push(limit);
                }
            }
            if !limits.iter().any(|limit| limit.kind == Kind::WeeklyScoped) {
                if let Some(limit) =
                    legacy(obj, "seven_day_sonnet", Kind::WeeklyScoped, Some("Sonnet"))
                {
                    limits.push(limit);
                }
            }
        }

        // One entry per kind, first wins (Swift's `seen` set).
        let mut seen = Vec::new();
        limits.retain(|limit| {
            if seen.contains(&limit.kind) {
                false
            } else {
                seen.push(limit.kind);
                true
            }
        });
        if limits.is_empty() {
            return Err(ParseError::NoLimits);
        }
        Ok(Self::new(limits, fetched_at))
    }
}

/// Swift `ClaudeQuotaSnapshot.number(_:)`: an `NSNumber` or a numeric string.
/// A JSON boolean bridges to `NSNumber` too, so `true` reads as 1 and `false`
/// as 0 — matching Swift's `as? NSNumber` / `doubleValue`.
fn number(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(number) => number.as_f64(),
        Value::String(raw) => raw.parse().ok(),
        Value::Bool(flag) => Some(if *flag { 1.0 } else { 0.0 }),
        _ => None,
    }
}

/// Swift `ClaudeQuotaSnapshot.date(_:)`: an ISO 8601 string, fractional or
/// plain; the shared core parser covers both formatter attempts.
fn date(value: Option<&Value>) -> Option<Timestamp> {
    parse_iso8601(value?.as_str()?)
}

/// Swift `ClaudeQuotaSnapshot.parse` legacy fallback for one key.
fn legacy(
    obj: &serde_json::Map<String, Value>,
    key: &str,
    kind: Kind,
    label: Option<&str>,
) -> Option<Limit> {
    let item = obj.get(key)?.as_object()?;
    let percent = number(item.get("utilization"))?;
    Some(Limit::new(
        kind,
        percent,
        "normal",
        date(item.get("resets_at")),
        label.map(str::to_owned),
    ))
}

/// `ClaudeQuotaClientError`, without the transport half (there is no HTTP
/// here). Like Swift's `interpret`, both [`ParseError`] cases collapse into the
/// single `.parse` — only [`Snapshot::parse`] itself keeps them distinct.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuotaError {
    /// Swift `.unauthorized`: 401/403, or a known-expired credential.
    Unauthorized,
    /// Swift `.rateLimited`: 429.
    RateLimited,
    /// Swift `.http(status)`. Redirects are refused by the shell, so a 3xx
    /// surfaces here instead of being followed.
    Http(i32),
    /// Swift `.parse`: the body was not JSON or carried no usable limit.
    Parse,
}

impl From<ParseError> for QuotaError {
    fn from(_: ParseError) -> Self {
        Self::Parse
    }
}

/// Swift `ClaudeQuotaClient.interpret(data:response:now:)`, with `status`
/// injected (the core never sees an `HTTPURLResponse`).
pub fn interpret_response(
    data: &[u8],
    status: i32,
    fetched_at: Timestamp,
) -> Result<Snapshot, QuotaError> {
    match status {
        200..=299 => Snapshot::parse(data, fetched_at).map_err(Into::into),
        401 | 403 => Err(QuotaError::Unauthorized),
        429 => Err(QuotaError::RateLimited),
        _ => Err(QuotaError::Http(status)),
    }
}

/// `ClaudeOAuthCredential`, as Claude Code stores it. The token is a secret:
/// [`fmt::Debug`] and `serde::Serialize` deliberately redact/omit it so it can
/// never reach a log line or a persisted state blob.
#[derive(Clone, PartialEq)]
pub struct Credential {
    /// Bearer token. Never format or serialize this value.
    pub access_token: String,
    pub subscription_type: Option<String>,
    pub expires_at: Option<Timestamp>,
}

impl fmt::Debug for Credential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Credential")
            .field("access_token", &"<redacted>")
            .field("subscription_type", &self.subscription_type)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

impl Serialize for Credential {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("Credential", 2)?;
        // No `access_token` field: serialized state contains no secret.
        state.serialize_field("subscription_type", &self.subscription_type)?;
        state.serialize_field("expires_at", &self.expires_at.map(|at| at.0))?;
        state.end()
    }
}

impl Credential {
    /// Swift `ClaudeCredentialStore.parse(_:)`: the
    /// `{"claudeAiOauth":{…}}` shape in the keychain / `.credentials.json`.
    pub fn parse(data: &[u8]) -> Option<Self> {
        let value: Value = serde_json::from_slice(data).ok()?;
        let oauth = value.get("claudeAiOauth")?.as_object()?;
        let access_token = oauth
            .get("accessToken")
            .and_then(Value::as_str)
            .filter(|token| !token.is_empty())?;
        Some(Self {
            access_token: access_token.to_owned(),
            subscription_type: oauth
                .get("subscriptionType")
                .and_then(Value::as_str)
                .map(str::to_owned),
            expires_at: oauth
                .get("expiresAt")
                .and_then(Value::as_f64)
                .map(|milliseconds| Timestamp(milliseconds / 1000.0)),
        })
    }

    /// Swift `fetch(using:)`: a known-expired token is never sent; it is
    /// reported as `unauthorized` before any request.
    pub fn is_expired(&self, now: Timestamp) -> bool {
        self.expires_at.is_some_and(|expires_at| expires_at <= now)
    }
}

/// `ClaudeQuotaScheduler.Config`: all intervals in seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SchedulerConfig {
    pub debounce: f64,
    pub throttle: f64,
    pub idle_floor: f64,
    pub expand_stale: f64,
    pub backoff_base: f64,
    pub backoff_max: f64,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            debounce: 15.0,
            throttle: 60.0,
            idle_floor: 600.0,
            expand_stale: 60.0,
            backoff_base: 60.0,
            backoff_max: 900.0,
        }
    }
}

/// `ClaudeQuotaScheduler`: pure refresh policy. Stop events are coalesced,
/// throttled, and backed off on failure; a rejected token stops background
/// fetches until the user expands again and a retry succeeds (Swift clears
/// `needsLogin` in `recordSuccess()`, and the monitor rebuilds the scheduler
/// when the feature is turned off). No timer or clock lives here.
#[derive(Clone, Debug, PartialEq)]
pub struct Scheduler {
    pub config: SchedulerConfig,
    pub last_fetch_at: Option<Timestamp>,
    pub last_stop_at: Option<Timestamp>,
    /// A Stop arrived after the last fetch started and hasn't been served.
    pub pending: bool,
    pub failures: u32,
    pub needs_login: bool,
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new(SchedulerConfig::default())
    }
}

impl Scheduler {
    pub fn new(config: SchedulerConfig) -> Self {
        Self {
            config,
            last_fetch_at: None,
            last_stop_at: None,
            pending: false,
            failures: 0,
            needs_login: false,
        }
    }

    pub fn record_stop(&mut self, now: Timestamp) {
        self.last_stop_at = Some(now);
        self.pending = true;
    }

    pub fn record_fetch_start(&mut self, now: Timestamp) {
        self.last_fetch_at = Some(now);
        self.pending = false;
    }

    pub fn record_success(&mut self) {
        self.failures = 0;
        self.needs_login = false;
    }

    /// `unauthorized` covers both a rejected token and a missing credential.
    pub fn record_failure(&mut self, unauthorized: bool) {
        if unauthorized {
            self.needs_login = true;
            self.failures = 0;
        } else {
            self.failures += 1;
        }
    }

    /// Swift `backoff`: exponential from `backoffBase`, capped at `backoffMax`;
    /// `None` when the last fetch succeeded.
    pub fn backoff(&self) -> Option<f64> {
        if self.failures == 0 {
            return None;
        }
        let exponent = (self.failures as f64 - 1.0).min(10.0);
        Some((self.config.backoff_base * 2f64.powf(exponent)).min(self.config.backoff_max))
    }

    /// Swift `nextFireTime(wantsLive:now:)`: the earliest background fetch, or
    /// `None` when nothing is on screen or a login is needed.
    pub fn next_fire_time(&self, wants_live: bool, now: Timestamp) -> Option<Timestamp> {
        if !wants_live || self.needs_login {
            return None;
        }
        let mut candidate = match (self.pending, self.last_stop_at) {
            (true, Some(last_stop_at)) => {
                let mut candidate = last_stop_at.0 + self.config.debounce;
                if let Some(last_fetch_at) = self.last_fetch_at {
                    candidate = candidate.max(last_fetch_at.0 + self.config.throttle);
                }
                candidate
            }
            _ => match self.last_fetch_at {
                Some(last_fetch_at) => last_fetch_at.0 + self.config.idle_floor,
                None => now.0,
            },
        };
        if let Some(last_fetch_at) = self.last_fetch_at {
            if let Some(backoff) = self.backoff() {
                candidate = candidate.max(last_fetch_at.0 + backoff);
            }
        }
        Some(Timestamp(candidate))
    }

    /// Swift `shouldFetchOnExpand(now:)`: user-initiated fetches ignore the
    /// login pause (the user may have signed in again) but still honour
    /// backoff and the stale window.
    pub fn should_fetch_on_expand(&self, now: Timestamp) -> bool {
        let Some(last_fetch_at) = self.last_fetch_at else {
            return true;
        };
        let age = now.0 - last_fetch_at.0;
        match self.backoff() {
            Some(backoff) => age >= backoff,
            None => age >= self.config.expand_stale,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Swift fixture: `ISO8601DateFormatter().date(from: "2026-09-03T15:00:00Z")`.
    fn now() -> Timestamp {
        parse_iso8601("2026-09-03T15:00:00Z").unwrap()
    }

    fn at(seconds: f64) -> Timestamp {
        Timestamp(now().0 + seconds)
    }

    /// Trimmed copy of a real `/api/oauth/usage` response (Max plan, 2026-09-03)
    /// from `ClaudeQuotaTests.fixture`.
    const FIXTURE: &str = r#"{"five_hour":{"utilization":3.0,"resets_at":"2026-09-03T19:50:00.017533+00:00"},
     "seven_day":{"utilization":30.0,"resets_at":"2026-09-04T00:00:00.017553+00:00"},
     "seven_day_opus":null,
     "limits":[
       {"kind":"session","group":"session","percent":3,"severity":"normal","resets_at":"2026-09-03T19:50:00.017533+00:00","scope":null,"is_active":false},
       {"kind":"weekly_all","group":"weekly","percent":30,"severity":"normal","resets_at":"2026-09-04T00:00:00.017553+00:00","scope":null,"is_active":false},
       {"kind":"weekly_scoped","group":"weekly","percent":48,"severity":"normal","resets_at":"2026-09-04T00:00:00.017765+00:00","scope":{"model":{"id":null,"display_name":"Fable"},"surface":null},"is_active":true},
       {"kind":"something_new","percent":1}
     ]}"#;

    #[test]
    fn parses_normalised_limits_and_ignores_unknown_kinds() {
        let snapshot = Snapshot::parse(FIXTURE.as_bytes(), now()).unwrap();
        assert_eq!(snapshot.limits.len(), 3);
        assert!(!snapshot.is_empty());
        assert_eq!(
            snapshot
                .ordered()
                .iter()
                .map(|limit| limit.kind)
                .collect::<Vec<_>>(),
            vec![Kind::Session, Kind::WeeklyAll, Kind::WeeklyScoped]
        );
        let scoped = snapshot.limit(Kind::WeeklyScoped).unwrap();
        assert_eq!(scoped.percent, 48.0);
        assert_eq!(scoped.scope_label.as_deref(), Some("Fable"));
        assert_eq!(scoped.level(), Level::Normal);
        let resets = snapshot.limit(Kind::Session).unwrap().resets_at.unwrap();
        let plain = parse_iso8601("2026-09-03T19:50:00Z").unwrap();
        assert!((resets.0 - plain.0).abs() < 0.1);
    }

    #[test]
    fn falls_back_to_legacy_fields_when_limits_missing() {
        let json = r#"{"five_hour":{"utilization":12.5,"resets_at":"2026-09-03T19:50:00Z"},
         "seven_day":{"utilization":70,"resets_at":null},
         "seven_day_sonnet":{"utilization":5,"resets_at":null}}"#;
        let snapshot = Snapshot::parse(json.as_bytes(), now()).unwrap();
        assert_eq!(snapshot.limit(Kind::Session).unwrap().percent, 12.5);
        assert_eq!(snapshot.limit(Kind::WeeklyAll).unwrap().percent, 70.0);
        assert_eq!(snapshot.limit(Kind::WeeklyAll).unwrap().resets_at, None);
        assert_eq!(
            snapshot
                .limit(Kind::WeeklyScoped)
                .unwrap()
                .scope_label
                .as_deref(),
            Some("Sonnet")
        );
    }

    #[test]
    fn boolean_utilization_reads_as_nsnumber_like_swift() {
        // Swift `number(_:)` sees a JSON boolean as an `NSNumber`; true is 1,
        // false is 0. Both the normalised and legacy shapes must agree.
        let normalised = r#"{"limits":[
            {"kind":"session","percent":true},
            {"kind":"weekly_all","percent":false}
        ]}"#;
        let snapshot = Snapshot::parse(normalised.as_bytes(), now()).unwrap();
        assert_eq!(snapshot.limit(Kind::Session).unwrap().percent, 1.0);
        assert_eq!(snapshot.limit(Kind::WeeklyAll).unwrap().percent, 0.0);

        let legacy = r#"{"five_hour":{"utilization":true}}"#;
        let snapshot = Snapshot::parse(legacy.as_bytes(), now()).unwrap();
        assert_eq!(snapshot.limit(Kind::Session).unwrap().percent, 1.0);
    }

    #[test]
    fn parse_errors_are_distinct() {
        for (name, json, expected) in [
            ("garbage", "nope", ParseError::NotJson),
            ("empty object", "{}", ParseError::NoLimits),
            ("array root", "[]", ParseError::NotJson),
            (
                "unknown kinds only",
                r#"{"limits":[{"kind":"something_new","percent":1}]}"#,
                ParseError::NoLimits,
            ),
            (
                "missing percent",
                r#"{"limits":[{"kind":"session"}]}"#,
                ParseError::NoLimits,
            ),
        ] {
            assert_eq!(
                Snapshot::parse(json.as_bytes(), now()).unwrap_err(),
                expected,
                "{name}"
            );
        }
    }

    #[test]
    fn interpret_maps_status_codes() {
        for (name, body, status, expected) in [
            ("ok", FIXTURE, 200, None),
            ("ok at upper edge", FIXTURE, 299, None),
            ("unauthorized", "", 401, Some(QuotaError::Unauthorized)),
            ("forbidden", "", 403, Some(QuotaError::Unauthorized)),
            ("rate limited", "", 429, Some(QuotaError::RateLimited)),
            ("server error", "", 503, Some(QuotaError::Http(503))),
            // RedirectRefusal returns nil: a 3xx is surfaced, never followed.
            ("redirect refused", "", 302, Some(QuotaError::Http(302))),
            ("not JSON", "nope", 200, Some(QuotaError::Parse)),
            ("no limits", "{}", 200, Some(QuotaError::Parse)),
        ] {
            assert_eq!(
                interpret_response(body.as_bytes(), status, now()).err(),
                expected,
                "{name}"
            );
        }
    }

    #[test]
    fn interpret_collapses_both_parse_errors_to_parse() {
        // Swift `ClaudeQuotaClient.interpret` throws `.parse` for both
        // `ParseError.notJSON` and `ParseError.noLimits`; only the snapshot
        // parser itself keeps the two apart.
        for body in ["nope", "{}", r#"{"limits":[{"kind":"new","percent":1}]}"#] {
            assert_eq!(
                interpret_response(body.as_bytes(), 200, now()).unwrap_err(),
                QuotaError::Parse,
                "{body}"
            );
        }
    }

    #[test]
    fn elapsed_fraction_and_pace_delta_derive_from_reset_time() {
        for (name, percent, reset_in, expected_elapsed, expected_delta) in [
            ("20% elapsed", 40.0, Some(4.0 * 3600.0), Some(0.2), 0.2),
            ("past reset clamps", 40.0, Some(-60.0), Some(1.0), -0.6),
            ("no reset falls back to used", 40.0, None, None, 0.4),
        ] {
            let limit = Limit::new(Kind::Session, percent, "normal", reset_in.map(at), None);
            match (limit.elapsed_fraction(now()), expected_elapsed) {
                (Some(got), Some(want)) => assert!((got - want).abs() < 0.001, "{name}"),
                (None, None) => {}
                (got, want) => panic!("{name}: elapsed {got:?} != {want:?}"),
            }
            assert!(
                (limit.pace_delta(now()) - expected_delta).abs() < 0.001,
                "{name}"
            );
        }
    }

    #[test]
    fn pace_is_hidden_early_without_a_reset_or_after_rollover() {
        for (name, limit) in [
            (
                "12 minutes into 5h",
                Limit::new(
                    Kind::Session,
                    20.0,
                    "normal",
                    Some(at(5.0 * 3600.0 - 12.0 * 60.0)),
                    None,
                ),
            ),
            (
                "no reset time",
                Limit::new(Kind::WeeklyAll, 30.0, "normal", None, None),
            ),
            (
                "rolled over",
                Limit::new(Kind::WeeklyAll, 30.0, "normal", Some(at(-60.0)), None),
            ),
        ] {
            assert!(limit.pace(now()).is_none(), "{name}");
        }
        let live = Limit::new(Kind::WeeklyAll, 30.0, "normal", Some(at(60.0)), None);
        assert!(!live.has_reset(now()));
        assert!(live.pace(now()).is_some());
    }

    #[test]
    fn pace_matches_swift_fixtures() {
        // 72% used with 3h of 5h left → 40% elapsed → 32pp ahead.
        let ahead = Limit::new(Kind::Session, 72.0, "normal", Some(at(3.0 * 3600.0)), None);
        let pace = ahead.pace(now()).unwrap();
        assert!((pace.points - 32.0).abs() < 0.01);
        assert_eq!(pace.tone, Tone::Ahead);
        assert!((pace.duration - 96.0 * 60.0).abs() < 1.0);
        assert!((pace.projected_percent - 180.0).abs() < 0.01);
        assert!((pace.exhausts_in.unwrap() - 2800.0).abs() < 1.0);

        // 30% used with 52h of 168h left → 69% elapsed → 39pp behind.
        let behind = Limit::new(
            Kind::WeeklyAll,
            30.0,
            "normal",
            Some(at(52.0 * 3600.0)),
            None,
        );
        let pace = behind.pace(now()).unwrap();
        assert!((pace.points - (30.0 - 11_600.0 / 168.0)).abs() < 0.01);
        assert_eq!(pace.tone, Tone::Behind);
        assert!(pace.duration < 0.0);
        assert!((pace.projected_percent - 30.0 * 168.0 / 116.0).abs() < 0.01);
        assert_eq!(pace.exhausts_in, None);

        // 52% used at 50% elapsed → +2 → within ±5.
        let even = Limit::new(
            Kind::WeeklyAll,
            52.0,
            "normal",
            Some(at(84.0 * 3600.0)),
            None,
        );
        assert_eq!(even.pace(now()).unwrap().tone, Tone::Neutral);

        // Already at the limit: nothing left to run out.
        let over = Limit::new(Kind::Session, 100.0, "normal", Some(at(3600.0)), None);
        assert_eq!(over.pace(now()).unwrap().exhausts_in, None);
    }

    #[test]
    fn severity_levels_match_swift() {
        for (name, percent, severity, expected) in [
            ("normal", 10.0, "normal", Level::Normal),
            ("warning", 10.0, "warning", Level::Warning),
            ("unknown", 10.0, "exceeded", Level::Critical),
            ("over limit", 100.0, "normal", Level::Critical),
            ("empty", 10.0, "", Level::Normal),
            ("alias", 10.0, "WARN", Level::Warning),
        ] {
            assert_eq!(
                Limit::new(Kind::Session, percent, severity, None, None).level(),
                expected,
                "{name}"
            );
        }
    }

    #[test]
    fn credential_parse_and_token_hygiene() {
        let json = r#"{"claudeAiOauth":{"accessToken":"sk-ant-oat01-abc","refreshToken":"r","expiresAt":1788472382000,"subscriptionType":"max"}}"#;
        let credential = Credential::parse(json.as_bytes()).unwrap();
        assert_eq!(credential.access_token, "sk-ant-oat01-abc");
        assert_eq!(credential.subscription_type.as_deref(), Some("max"));
        assert_eq!(credential.expires_at.unwrap().0, 1_788_472_382.0);
        assert!(credential.is_expired(Timestamp(1_788_472_382.0)));
        assert!(!credential.is_expired(Timestamp(1_788_472_381.0)));
        assert!(Credential::parse(br#"{"claudeAiOauth":{"accessToken":""}}"#).is_none());

        let debug = format!("{credential:?}");
        let serialized = serde_json::to_string(&credential).unwrap();
        assert!(!debug.contains("sk-ant-oat01-abc"), "{debug}");
        assert!(!serialized.contains("sk-ant-oat01-abc"), "{serialized}");
        assert!(debug.contains("<redacted>"));
        assert!(!serialized.contains("access_token"));
    }

    // Scheduler fixtures: `ClaudeQuotaSchedulerTests`, t0 = 1_800_000_000.
    fn t0() -> Timestamp {
        Timestamp(1_800_000_000.0)
    }

    fn at_t0(seconds: f64) -> Timestamp {
        Timestamp(t0().0 + seconds)
    }

    #[test]
    fn first_live_tick_fires_immediately() {
        let scheduler = Scheduler::default();
        assert_eq!(scheduler.next_fire_time(true, t0()), Some(t0()));
        assert_eq!(scheduler.next_fire_time(false, t0()), None);
    }

    #[test]
    fn stops_within_debounce_coalesce_into_one_fetch() {
        let mut scheduler = Scheduler::default();
        scheduler.record_fetch_start(at_t0(-3600.0));
        scheduler.record_stop(at_t0(0.0));
        assert_eq!(
            scheduler.next_fire_time(true, at_t0(0.0)),
            Some(at_t0(15.0))
        );
        scheduler.record_stop(at_t0(12.0));
        assert_eq!(
            scheduler.next_fire_time(true, at_t0(12.0)),
            Some(at_t0(27.0))
        );
        scheduler.record_fetch_start(at_t0(27.0));
        assert!(!scheduler.pending);
        assert_eq!(
            scheduler.next_fire_time(true, at_t0(27.0)),
            Some(at_t0(27.0 + 600.0))
        );
    }

    #[test]
    fn stop_inside_throttle_gets_trailing_fetch() {
        let mut scheduler = Scheduler::default();
        scheduler.record_fetch_start(at_t0(0.0));
        scheduler.record_stop(at_t0(10.0));
        assert_eq!(
            scheduler.next_fire_time(true, at_t0(10.0)),
            Some(at_t0(60.0))
        );
        scheduler.record_stop(at_t0(50.0));
        assert_eq!(
            scheduler.next_fire_time(true, at_t0(50.0)),
            Some(at_t0(65.0))
        );
    }

    #[test]
    fn stops_are_remembered_while_nothing_is_on_screen() {
        let mut scheduler = Scheduler::default();
        scheduler.record_stop(at_t0(0.0));
        assert_eq!(scheduler.next_fire_time(false, at_t0(0.0)), None);
        assert_eq!(
            scheduler.next_fire_time(true, at_t0(100.0)),
            Some(at_t0(15.0))
        );
    }

    #[test]
    fn failures_back_off_exponentially_and_cap() {
        let mut scheduler = Scheduler::default();
        scheduler.record_fetch_start(at_t0(0.0));
        scheduler.record_failure(false);
        assert_eq!(scheduler.backoff(), Some(60.0));
        scheduler.record_stop(at_t0(1.0));
        assert_eq!(
            scheduler.next_fire_time(true, at_t0(1.0)),
            Some(at_t0(60.0))
        );
        for _ in 0..10 {
            scheduler.record_failure(false);
        }
        assert_eq!(scheduler.backoff(), Some(900.0));
        assert_eq!(
            scheduler.next_fire_time(true, at_t0(1.0)),
            Some(at_t0(900.0))
        );
        assert!(!scheduler.should_fetch_on_expand(at_t0(899.0)));
        assert!(scheduler.should_fetch_on_expand(at_t0(900.0)));
        scheduler.record_success();
        assert_eq!(scheduler.backoff(), None);
    }

    #[test]
    fn rejected_token_stops_background_fetches_but_expand_retries() {
        let mut scheduler = Scheduler::default();
        scheduler.record_fetch_start(at_t0(0.0));
        scheduler.record_failure(true);
        assert!(scheduler.needs_login);
        scheduler.record_stop(at_t0(5.0));
        assert_eq!(scheduler.next_fire_time(true, at_t0(5.0)), None);
        assert!(!scheduler.should_fetch_on_expand(at_t0(30.0)));
        assert!(scheduler.should_fetch_on_expand(at_t0(60.0)));
        scheduler.record_success();
        assert!(!scheduler.needs_login);
    }

    #[test]
    fn expand_refreshes_only_when_stale() {
        let mut scheduler = Scheduler::default();
        assert!(scheduler.should_fetch_on_expand(t0()));
        scheduler.record_fetch_start(t0());
        assert!(!scheduler.should_fetch_on_expand(at_t0(59.0)));
        assert!(scheduler.should_fetch_on_expand(at_t0(60.0)));
    }
}
