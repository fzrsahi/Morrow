//! The HTTP side of a push, minus the network
//! (`macos/Sources/MorrowCore/PushTransport.swift`, and the interpreter at the tail of
//! `PushChannel.swift`).
//!
//! #238 AC 1, third of four. Nothing here opens a socket: the transport is a trait, and
//! everything that decides what an answer *meant* is pure — which is the only way a
//! delivery verdict can be tested without a Bark device or a Slack workspace.
//!
//! Four rules, each of which lost a push once:
//!
//! - **These services answer HTTP 200 for a rejected message**, so the body decides.
//!   Bark's `code`, DingTalk's `errcode`, Feishu's `code`, Telegram's `ok`.
//! - **A redirect is only followed within one host.** The body still carries the Bark
//!   device key, the ntfy topic or the chat's content, and the path of a webhook *is* a
//!   credential. An https → http hop would put all of it in clear.
//! - **The redirected POST stays a POST.** URLSession turns a POST answered with
//!   301/302/303 into a body-less GET, so a self-hosted Bark behind an http → https
//!   redirect receives an empty request and the push is lost with a misleading error.
//! - **A retry is only for a bad moment**, never for something that would fail the same
//!   way again, and a `Retry-After` is capped: an approval half a minute late is still
//!   useful, one ten minutes late mostly is not.

use crate::push::{PushEventKind, truncated};
use crate::push_policy::PushChannelKind;
use url::Url;

/// Raw outcome of one HTTP exchange, before the channel interprets the body.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PushTransportResponse {
    /// `None` when no HTTP response arrived (DNS, TLS, timeout).
    pub status_code: Option<u16>,
    pub body: String,
    /// The URL that finally answered, when redirects were followed.
    pub final_url: Option<String>,
    /// Set when no HTTP response arrived at all.
    pub error_description: Option<String>,
    /// Where the server redirected when that redirect was refused; `status_code` is then
    /// the 3xx itself.
    pub refused_redirect: Option<String>,
    /// The server's `Retry-After`, in seconds, when it sent one.
    pub retry_after: Option<f64>,
}

/// What the settings page shows after a test push.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PushDeliveryResult {
    pub ok: bool,
    /// `None` when no HTTP response arrived.
    pub status_code: Option<u16>,
    /// Server-side reason (`errcode`/`errmsg`, `code`/`msg`, `description`, …) or the
    /// transport error.
    pub message: String,
    /// The request was answered from a different URL: the user should update the saved
    /// address.
    pub redirected_to: Option<String>,
    /// The server redirected somewhere CodeIsland refuses to follow — credential-free,
    /// like `redirected_to`.
    pub redirect_refused_to: Option<String>,
}

impl PushDeliveryResult {
    /// The channel's verdict on a transport outcome.
    pub fn from_response(
        response: &PushTransportResponse,
        kind: PushChannelKind,
        request_url: &str,
    ) -> Self {
        let Some(status) = response.status_code else {
            return Self {
                ok: false,
                status_code: None,
                message: response
                    .error_description
                    .clone()
                    .unwrap_or_else(|| "no response".to_owned()),
                ..Self::default()
            };
        };
        if let Some(refused) = &response.refused_redirect {
            return Self {
                ok: false,
                status_code: Some(status),
                message: "redirect not followed".to_owned(),
                redirect_refused_to: Some(display_url(refused)),
                ..Self::default()
            };
        }
        let (ok, message) = interpret(kind, status, &response.body);
        let redirected_to = response
            .final_url
            .as_deref()
            .filter(|final_url| *final_url != request_url)
            .map(display_url);
        Self {
            ok,
            status_code: Some(status),
            message,
            redirected_to,
            redirect_refused_to: None,
        }
    }
}

/// The only thing that touches the network. A test injects a recorder, so no test ever
/// reaches a real push service.
pub trait PushTransport {
    fn send(&self, request: &PushHttpRequest) -> PushTransportResponse;
}

/// One request, ready to send. Kept transport-agnostic so the shell can hand it to
/// whatever HTTP client the platform provides.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PushHttpRequest {
    pub url: String,
    pub method: &'static str,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl PushHttpRequest {
    pub fn post(url: impl Into<String>, body: impl Into<Vec<u8>>) -> Self {
        Self {
            url: url.into(),
            method: "POST",
            headers: Vec::new(),
            body: body.into(),
        }
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

/// Most of these services answer HTTP 200 for a rejected message and put the verdict in
/// the body, so the body decides, not the status alone.
pub fn interpret(kind: PushChannelKind, status_code: u16, body: &str) -> (bool, String) {
    const MESSAGE_LIMIT: usize = 300;
    let json: Option<serde_json::Value> = serde_json::from_str(body).ok();
    let text = body.trim();
    let http_ok = (200..300).contains(&status_code);
    let limited = |text: &str| truncated(text, MESSAGE_LIMIT);
    let fallback = || {
        (
            http_ok,
            limited(if text.is_empty() && http_ok {
                "ok"
            } else {
                text
            }),
        )
    };

    match kind {
        // {"code":200,"message":"success"}; errors reuse the shape.
        PushChannelKind::Bark => {
            let Some(json) = json else {
                return fallback();
            };
            let code = integer(json.get("code"));
            let message = json
                .get("message")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let ok = http_ok && code.is_none_or(|code| code == 200);
            (
                ok,
                limited(if message.is_empty() {
                    if ok { "ok" } else { text }
                } else {
                    message
                }),
            )
        }
        // Errors: {"code":40301,"http":403,"error":"forbidden"}.
        PushChannelKind::Ntfy => {
            if http_ok {
                return (true, "ok".to_owned());
            }
            let Some(json) = json else {
                return fallback();
            };
            let Some(error) = json.get("error").and_then(|value| value.as_str()) else {
                return fallback();
            };
            let code = integer(json.get("code"))
                .map(|code| format!(" ({code})"))
                .unwrap_or_default();
            (false, limited(&format!("{error}{code}")))
        }
        // {"errcode":0,"errmsg":"ok"}; DingTalk's docs show errcode as a string.
        PushChannelKind::Dingtalk | PushChannelKind::Wecom => {
            let Some(json) = json else {
                return fallback();
            };
            let Some(code) = integer(json.get("errcode")) else {
                return fallback();
            };
            let message = json
                .get("errmsg")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            if http_ok && code == 0 {
                return (
                    true,
                    limited(if message.is_empty() { "ok" } else { message }),
                );
            }
            (false, limited(&format!("errcode {code}: {message}")))
        }
        // {"code":0,"msg":"success"}; legacy {"StatusCode":0,"StatusMessage":…}.
        PushChannelKind::Feishu => {
            let Some(json) = json else {
                return fallback();
            };
            let Some(code) = integer(json.get("code")).or_else(|| integer(json.get("StatusCode")))
            else {
                return fallback();
            };
            let message = json
                .get("msg")
                .or_else(|| json.get("StatusMessage"))
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            if http_ok && code == 0 {
                return (
                    true,
                    limited(if message.is_empty() { "ok" } else { message }),
                );
            }
            (false, limited(&format!("code {code}: {message}")))
        }
        // 200 "ok"; errors are plain strings like "invalid_payload".
        PushChannelKind::Slack => fallback(),
        // {"ok":false,"error_code":400,"description":"Bad Request: chat not found"}
        PushChannelKind::Telegram => {
            let Some(json) = json else {
                return fallback();
            };
            let Some(ok) = json.get("ok").and_then(|value| value.as_bool()) else {
                return fallback();
            };
            if ok {
                return (true, "ok".to_owned());
            }
            let description = json
                .get("description")
                .and_then(|value| value.as_str())
                .unwrap_or(text);
            let code = integer(json.get("error_code"))
                .map(|code| format!("{code}: "))
                .unwrap_or_default();
            (false, limited(&format!("{code}{description}")))
        }
    }
}

fn integer(value: Option<&serde_json::Value>) -> Option<i64> {
    let value = value?;
    if let Some(number) = value.as_i64() {
        return Some(number);
    }
    if let Some(number) = value.as_f64() {
        return Some(number as i64);
    }
    value
        .as_str()
        .and_then(|text| text.trim().parse::<i64>().ok())
}

/// One more try for a push that must not get lost to a hiccup.
pub const RETRY_DELAY_SECONDS: f64 = 5.0;
/// A `Retry-After` longer than this is cut to it.
pub const RETRY_MAX_DELAY_SECONDS: f64 = 30.0;
/// Redirects followed before the trail is called a loop.
pub const MAX_REDIRECTS: usize = 5;

/// Approvals, questions and errors are worth a retry; a finished turn and a reminder are
/// not — by the time they arrive the moment is gone.
pub fn retries(kind: PushEventKind) -> bool {
    matches!(
        kind,
        PushEventKind::Permission | PushEventKind::Question | PushEventKind::Error
    )
}

/// Seconds to wait before the retry, or `None` when this outcome is not worth one.
///
/// A 5xx or a 429 is a bad moment; a 4xx, a rejected signature or a refused redirect
/// would fail the same way again. A refused redirect is never retried, whatever the code.
pub fn retry_delay(response: &PushTransportResponse) -> Option<f64> {
    if response.refused_redirect.is_some() {
        return None;
    }
    let Some(status) = response.status_code else {
        return Some(RETRY_DELAY_SECONDS);
    };
    if status == 429 {
        let hinted = response
            .retry_after
            .or_else(|| telegram_retry_after(&response.body))
            .unwrap_or(RETRY_DELAY_SECONDS);
        return Some(hinted.clamp(1.0, RETRY_MAX_DELAY_SECONDS));
    }
    (500..600).contains(&status).then_some(RETRY_DELAY_SECONDS)
}

/// Telegram's own hint: `{"parameters":{"retry_after":17}}`.
fn telegram_retry_after(body: &str) -> Option<f64> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    json.get("parameters")?.get("retry_after")?.as_f64()
}

/// `Retry-After` as delay-seconds or as an HTTP date.
///
/// The date form is rare (a 429 from a CDN rather than a chat service) but it exists, and
/// treating it as "no hint" would retry immediately into the same limit.
pub fn retry_after_seconds(value: &str, now: f64) -> Option<f64> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if let Ok(seconds) = value.parse::<f64>() {
        return Some(seconds.max(0.0));
    }
    let at = parse_http_date(value)?;
    Some((at - now).max(0.0))
}

/// `EEE, dd MMM yyyy HH:mm:ss GMT` — the one format a `Retry-After` may carry.
fn parse_http_date(value: &str) -> Option<f64> {
    let parts: Vec<&str> = value.split_whitespace().collect();
    // "Wed, 21 Oct 2015 07:28:00 GMT"
    if parts.len() != 6 || !parts[5].eq_ignore_ascii_case("GMT") {
        return None;
    }
    let day: i64 = parts[1].parse().ok()?;
    let month = match parts[2].to_ascii_lowercase().as_str() {
        "jan" => 1,
        "feb" => 2,
        "mar" => 3,
        "apr" => 4,
        "may" => 5,
        "jun" => 6,
        "jul" => 7,
        "aug" => 8,
        "sep" => 9,
        "oct" => 10,
        "nov" => 11,
        "dec" => 12,
        _ => return None,
    };
    let year: i64 = parts[3].parse().ok()?;
    let clock: Vec<&str> = parts[4].split(':').collect();
    if clock.len() != 3 {
        return None;
    }
    let hour: i64 = clock[0].parse().ok()?;
    let minute: i64 = clock[1].parse().ok()?;
    let second: i64 = clock[2].parse().ok()?;
    if !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let days = crate::time::days_from_civil(year, month, day);
    Some((days * 86_400 + hour * 3600 + minute * 60 + second) as f64)
}

/// Why a redirect was not followed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedirectRefusal {
    TooManyHops,
    UnsupportedScheme,
    /// https → http: the credentials and the body would travel in clear.
    Downgrade,
    /// Another host: the body carries the device key, the topic or the chat's content,
    /// and the path may be a credential.
    OtherHost,
}

impl RedirectRefusal {
    /// Whether the user can fix it by saving the new address themselves.
    pub fn names_a_new_address(self) -> bool {
        matches!(self, Self::Downgrade | Self::OtherHost)
    }
}

/// Why a hop from `original` to `target` is not followed; `None` follows it.
///
/// Only same-host hops are followed — http → https, trailing-slash and path moves, the
/// ones a self-hosted server sends.
pub fn redirect_refusal(
    original: Option<&str>,
    target: &str,
    redirect_count: usize,
) -> Option<RedirectRefusal> {
    if redirect_count >= MAX_REDIRECTS {
        return Some(RedirectRefusal::TooManyHops);
    }
    let scheme = scheme_of(target)?;
    if scheme != "https" && scheme != "http" {
        return Some(RedirectRefusal::UnsupportedScheme);
    }
    let original_scheme = original.and_then(scheme_of);
    if original_scheme.as_deref() == Some("https") && scheme == "http" {
        return Some(RedirectRefusal::Downgrade);
    }
    // A redirect is only followed inside the same authority. `Url` does the
    // parsing: credentials, IPv6 literals and an explicit port were all misread
    // by the old `split(':')` surgery (#360).
    let target_authority = authority(target);
    let original_authority = original.and_then(authority);
    if target_authority.as_ref().map(|a| a.0.as_str())
        != original_authority.as_ref().map(|a| a.0.as_str())
        || target_authority.as_ref().map(|a| a.1) != original_authority.as_ref().map(|a| a.1)
    {
        return Some(RedirectRefusal::OtherHost);
    }
    None
}

/// Lower-cased host and explicit port, with a scheme's implicit default
/// normalised away so `http→https` on the same host is still the same authority.
fn authority(url: &str) -> Option<(String, Option<u16>)> {
    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_lowercase();
    let port = parsed.port().filter(|port| {
        Some(*port)
            != match parsed.scheme() {
                "http" => Some(80),
                "https" => Some(443),
                _ => None,
            }
    });
    Some((host, port))
}

fn scheme_of(url: &str) -> Option<String> {
    let (scheme, _) = url.split_once("://")?;
    Some(scheme.to_lowercase())
}

/// Credential-free, token-free form of a URL for the settings page.
///
/// The path of a webhook (Slack, Telegram) or the query (DingTalk, WeCom) *is* the
/// credential, so only scheme, host and port are shown.
pub fn display_url(url: &str) -> String {
    let Ok(parsed) = Url::parse(url) else {
        return format!("{}://…", scheme_of(url).unwrap_or_default());
    };
    let scheme = parsed.scheme();
    let host = parsed.host_str().unwrap_or_default();
    match parsed.port() {
        Some(port) => format!("{scheme}://{host}:{port}/…"),
        None => format!("{scheme}://{host}/…"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(status: Option<u16>, body: &str) -> PushTransportResponse {
        PushTransportResponse {
            status_code: status,
            body: body.to_owned(),
            ..PushTransportResponse::default()
        }
    }

    #[test]
    fn a_rejected_message_can_still_answer_200() {
        // Bark: the body decides.
        let empty = interpret(
            PushChannelKind::Bark,
            200,
            r#"{"code":200,"message":"success"}"#,
        );
        assert_eq!(empty, (true, "success".to_owned()));
        let rejected = interpret(
            PushChannelKind::Bark,
            200,
            r#"{"code":400,"message":"device token invalid"}"#,
        );
        assert!(!rejected.0);
        assert_eq!(rejected.1, "device token invalid");

        // DingTalk: errcode as a number or a string.
        assert_eq!(
            interpret(
                PushChannelKind::Dingtalk,
                200,
                r#"{"errcode":0,"errmsg":"ok"}"#
            ),
            (true, "ok".to_owned())
        );
        assert_eq!(
            interpret(
                PushChannelKind::Dingtalk,
                200,
                r#"{"errcode":"310000","errmsg":"sign not match"}"#
            ),
            (false, "errcode 310000: sign not match".to_owned())
        );

        // Feishu: current and legacy shapes.
        assert!(
            interpret(
                PushChannelKind::Feishu,
                200,
                r#"{"code":0,"msg":"success"}"#
            )
            .0
        );
        assert_eq!(
            interpret(
                PushChannelKind::Feishu,
                200,
                r#"{"StatusCode":19021,"StatusMessage":"sign match fail"}"#
            ),
            (false, "code 19021: sign match fail".to_owned())
        );

        // Telegram: `ok` false with a description.
        assert_eq!(
            interpret(
                PushChannelKind::Telegram,
                400,
                r#"{"ok":false,"error_code":400,"description":"Bad Request: chat not found"}"#
            ),
            (false, "400: Bad Request: chat not found".to_owned())
        );
        assert_eq!(
            interpret(PushChannelKind::Telegram, 200, r#"{"ok":true}"#),
            (true, "ok".to_owned())
        );
    }

    #[test]
    fn ntfy_and_slack_fall_back_and_reports_the_body() {
        assert_eq!(
            interpret(PushChannelKind::Ntfy, 200, ""),
            (true, "ok".to_owned())
        );
        assert_eq!(
            interpret(
                PushChannelKind::Ntfy,
                403,
                r#"{"code":40301,"error":"forbidden"}"#
            ),
            (false, "forbidden (40301)".to_owned())
        );
        assert_eq!(
            interpret(PushChannelKind::Slack, 400, "invalid_payload"),
            (false, "invalid_payload".to_owned())
        );
        // No body and not OK: the message says nothing rather than inventing one.
        assert_eq!(
            interpret(PushChannelKind::Slack, 500, ""),
            (false, String::new())
        );
    }

    #[test]
    fn an_error_message_is_bounded() {
        let long = "x".repeat(600);
        let (ok, message) = interpret(PushChannelKind::Slack, 500, &long);
        assert!(!ok);
        assert!(
            message.chars().count() <= 300,
            "{} chars",
            message.chars().count()
        );
        assert!(message.ends_with('…'));
    }

    #[test]
    fn no_response_is_a_failure_with_the_transports_own_words() {
        let mut no_answer = response(None, "");
        no_answer.error_description = Some("timeout".to_owned());
        let result = PushDeliveryResult::from_response(
            &no_answer,
            PushChannelKind::Bark,
            "https://api.day.app/x",
        );
        assert!(!result.ok);
        assert_eq!(result.status_code, None);
        assert_eq!(result.message, "timeout");

        let silent = PushDeliveryResult::from_response(
            &response(None, ""),
            PushChannelKind::Bark,
            "https://api.day.app/x",
        );
        assert_eq!(silent.message, "no response");
    }

    #[test]
    fn a_followed_redirect_is_reported_without_the_credential() {
        let mut moved = response(Some(200), r#"{"code":200}"#);
        moved.final_url = Some("https://new.example.com/hook/SECRET".to_owned());
        let result = PushDeliveryResult::from_response(
            &moved,
            PushChannelKind::Bark,
            "https://api.day.app/x",
        );
        assert!(result.ok);
        assert_eq!(
            result.redirected_to.as_deref(),
            Some("https://new.example.com/…")
        );

        // Answering from the same URL is not a redirect to report.
        let mut same = response(Some(200), r#"{"code":200}"#);
        same.final_url = Some("https://api.day.app/x".to_owned());
        assert_eq!(
            PushDeliveryResult::from_response(
                &same,
                PushChannelKind::Bark,
                "https://api.day.app/x"
            )
            .redirected_to,
            None
        );
    }

    #[test]
    fn a_refused_redirect_says_where_and_is_never_ok() {
        let mut refused = response(Some(302), "");
        refused.refused_redirect = Some("http://elsewhere.example.com/path".to_owned());
        let result = PushDeliveryResult::from_response(
            &refused,
            PushChannelKind::Slack,
            "https://hooks.slack.com/x",
        );
        assert!(!result.ok);
        assert_eq!(result.status_code, Some(302));
        assert_eq!(result.message, "redirect not followed");
        assert_eq!(
            result.redirect_refused_to.as_deref(),
            Some("http://elsewhere.example.com/…")
        );
    }

    #[test]
    fn only_a_same_host_hop_is_followed() {
        // A trailing-slash or path move on the same host: fine.
        assert_eq!(
            redirect_refusal(Some("https://example.com/a"), "https://example.com/a/b", 0),
            None
        );
        // Another host carries the device key and the chat content elsewhere.
        assert_eq!(
            redirect_refusal(Some("https://example.com/a"), "https://evil.example/b", 0),
            Some(RedirectRefusal::OtherHost)
        );
        // https → http puts the body in clear.
        assert_eq!(
            redirect_refusal(Some("https://example.com/a"), "http://example.com/a", 0),
            Some(RedirectRefusal::Downgrade)
        );
        // http → https is the upgrade a self-hosted server sends.
        assert_eq!(
            redirect_refusal(Some("http://example.com/a"), "https://example.com/a", 0),
            None
        );
        assert_eq!(
            redirect_refusal(Some("https://example.com/a"), "ftp://example.com/a", 0),
            Some(RedirectRefusal::UnsupportedScheme)
        );
        assert_eq!(
            redirect_refusal(
                Some("https://example.com/a"),
                "https://example.com/b",
                MAX_REDIRECTS
            ),
            Some(RedirectRefusal::TooManyHops)
        );
        assert!(RedirectRefusal::Downgrade.names_a_new_address());
        assert!(!RedirectRefusal::TooManyHops.names_a_new_address());
    }

    #[test]
    fn ipv6_and_explicit_ports_are_parsed_as_authorities() {
        assert_eq!(
            display_url("http://[::1]:8080/a?token=secret"),
            "http://[::1]:8080/…"
        );
        assert_eq!(
            redirect_refusal(Some("https://[::1]:8443/a"), "https://[::1]:8443/b", 0),
            None
        );
        assert_eq!(
            redirect_refusal(Some("https://[::1]:8443/a"), "https://[::1]:9443/b", 0),
            Some(RedirectRefusal::OtherHost)
        );
        // Credentials are not part of the host; a change there is still the
        // same authority, as it was before.
        assert_eq!(
            redirect_refusal(
                Some("https://u:p@example.com/a"),
                "https://example.com/b",
                0
            ),
            None
        );
        // The scheme's implicit port counts: https:443 → http:80 is a hop to
        // a different authority (and is caught as a downgrade first).
        assert_eq!(
            redirect_refusal(Some("http://example.com/a"), "https://example.com/b", 0),
            None
        );
    }

    #[test]
    fn a_displayed_url_keeps_only_the_scheme_host_and_port() {
        assert_eq!(
            display_url("https://api.day.app/DEVICEKEY/secret"),
            "https://api.day.app/…"
        );
        assert_eq!(
            display_url("https://hooks.slack.com:8443/services/T/B/X"),
            "https://hooks.slack.com:8443/…"
        );
        assert_eq!(
            display_url("https://user:pass@example.com/path"),
            "https://example.com/…"
        );
        assert_eq!(
            display_url("https://oapi.dingtalk.com/robot/send?access_token=abc"),
            "https://oapi.dingtalk.com/…"
        );
    }

    #[test]
    fn a_retry_is_only_for_a_bad_moment() {
        assert!(retries(PushEventKind::Permission));
        assert!(retries(PushEventKind::Question));
        assert!(retries(PushEventKind::Error));
        assert!(!retries(PushEventKind::Completion));
        assert!(!retries(PushEventKind::Reminder));

        // No response, a 5xx: worth one try.
        assert_eq!(retry_delay(&response(None, "")), Some(RETRY_DELAY_SECONDS));
        assert_eq!(
            retry_delay(&response(Some(503), "")),
            Some(RETRY_DELAY_SECONDS)
        );
        // A 4xx would fail the same way again.
        assert_eq!(retry_delay(&response(Some(400), "")), None);
        assert_eq!(retry_delay(&response(Some(200), "")), None);
        // A refused redirect is not a hiccup.
        let mut refused = response(Some(503), "");
        refused.refused_redirect = Some("https://evil.example/".to_owned());
        assert_eq!(retry_delay(&refused), None);
    }

    #[test]
    fn a_429_waits_what_the_server_asked_within_reason() {
        let mut throttled = response(Some(429), "");
        throttled.retry_after = Some(17.0);
        assert_eq!(retry_delay(&throttled), Some(17.0));

        // Telegram puts it in the body.
        let telegram = response(Some(429), r#"{"ok":false,"parameters":{"retry_after":9}}"#);
        assert_eq!(retry_delay(&telegram), Some(9.0));

        // No hint at all: the default.
        assert_eq!(
            retry_delay(&response(Some(429), "")),
            Some(RETRY_DELAY_SECONDS)
        );

        // Ten minutes is cut: an approval that late is mostly useless.
        let mut long = response(Some(429), "");
        long.retry_after = Some(600.0);
        assert_eq!(retry_delay(&long), Some(RETRY_MAX_DELAY_SECONDS));

        // A zero-second hint still waits a minimum, or the retry is immediate.
        let mut zero = response(Some(429), "");
        zero.retry_after = Some(0.0);
        assert_eq!(retry_delay(&zero), Some(1.0));
    }

    #[test]
    fn retry_after_reads_seconds_or_an_http_date() {
        assert_eq!(retry_after_seconds("17", 0.0), Some(17.0));
        assert_eq!(retry_after_seconds("  0 ", 0.0), Some(0.0));
        assert_eq!(retry_after_seconds("", 0.0), None);
        assert_eq!(retry_after_seconds("soon", 0.0), None);

        // 2015-10-21T07:28:00Z is 1_445_412_480.
        let now = 1_445_412_400.0;
        assert_eq!(
            retry_after_seconds("Wed, 21 Oct 2015 07:28:00 GMT", now),
            Some(80.0)
        );
        // A date in the past is no wait, not a negative one.
        assert_eq!(
            retry_after_seconds("Wed, 21 Oct 2015 07:28:00 GMT", 1_445_412_500.0),
            Some(0.0)
        );
    }

    #[test]
    fn a_request_is_a_post_with_headers() {
        let request = PushHttpRequest::post("https://api.day.app/key", "{\"body\":\"hi\"}")
            .header("Content-Type", "application/json");
        assert_eq!(request.method, "POST");
        assert_eq!(request.headers.len(), 1);
        assert_eq!(request.body, b"{\"body\":\"hi\"}".to_vec());
    }
}
