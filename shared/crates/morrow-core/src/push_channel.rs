//! Building the one HTTP request a channel sends
//! (`macos/Sources/MorrowCore/PushChannel.swift`).
//!
//! #238 AC 1, fourth of four. The rendering is in [`crate::push`], the decision in
//! [`crate::push_policy`], the answer in [`crate::push_transport`]; this turns a message
//! into the bytes a service expects. Nothing is sent here.
//!
//! Each builder exists because a service is peculiar in one way that a "generic webhook"
//! would get wrong:
//!
//! - **Bark** relays through APNs, whose payload limit counts bytes for title, subtitle
//!   and group together, so the body is cut in **bytes** — a thousand CJK characters
//!   would be 3–4 KB. A blocking push uses `timeSensitive`, which breaks through Focus.
//! - **ntfy** must receive JSON at the server **root**: POSTed to `/<topic>`, it
//!   publishes the whole document as the text of a new message and still answers 200.
//!   So a topic at the end of the server field is taken out — it is the topic when Topic
//!   is empty, a repeat when equal, and a contradiction the user has to resolve when not.
//! - **DingTalk** signs over `"<timestampMillis>\n<secret>"`, **Feishu** keys the MAC with
//!   that string and signs nothing — the inversion is real and documented by both.
//! - **Slack** reads `<…>` as link syntax and `*`/`_` as emphasis, so the text is escaped
//!   and mrkdwn is off: a command must survive as typed.
//! - **Telegram** counts UTF-16 units, and the title shares the limit with the body
//!   after the tags are parsed.

use base64::Engine as _;
use serde_json::{Map, Value, json};

use crate::push::{PushEventKind, PushMessage, truncated, truncated_utf8, truncated_utf16};
use crate::push_policy::PushChannelKind;
use crate::push_transport::PushHttpRequest;

impl PushChannelKind {
    /// Hosted default for the services that have one; the rest take a webhook URL.
    pub fn default_endpoint(self) -> &'static str {
        match self {
            Self::Bark => "https://api.day.app",
            Self::Ntfy => "https://ntfy.sh",
            Self::Telegram => "https://api.telegram.org",
            Self::Dingtalk | Self::Feishu | Self::Wecom | Self::Slack => "",
        }
    }

    /// A team chat: everyone in the group reads what is posted, so a new channel sends
    /// headlines only.
    pub fn is_group_chat(self) -> bool {
        matches!(
            self,
            Self::Dingtalk | Self::Feishu | Self::Wecom | Self::Slack
        )
    }
}

/// Why a channel cannot send yet — shown inline in settings, and reported by a test send
/// instead of making a request that is bound to fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PushConfigProblem {
    MissingDeviceKey,
    MissingTopic,
    MissingWebhook,
    MissingBotToken,
    MissingChatId,
    InvalidUrl,
    /// ntfy: the server field already ends in a topic, and Topic names another one.
    NtfyTopicMismatch,
}

/// One channel's settings: a flat record, because the fields mean different things per
/// kind and a flat record keeps the stored JSON and the settings bindings simple.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PushChannelConfig {
    pub kind: PushChannelKind,
    pub enabled: bool,
    pub events: Vec<PushEventKind>,
    /// Bark / ntfy server, Telegram API base, or the full webhook URL. May carry
    /// `user:password@` for a self-hosted server behind basic auth.
    pub endpoint: String,
    /// Bark device key, ntfy topic, Telegram `chat_id`.
    pub target: String,
    /// ntfy access token, Telegram bot token.
    pub token: String,
    /// DingTalk "加签" secret, Feishu signature secret.
    pub secret: String,
    /// Bark only.
    pub group: String,
    pub icon: String,
    pub sound: String,
    /// ntfy priority (1–5) for pushes that block an agent.
    pub priority: i64,
    /// Commands, replies, error text and question options. Off, a push says only who,
    /// which project and what happened. Defaults off for team chats.
    pub include_details: bool,
}

/// The priority a fresh channel starts at.
pub const DEFAULT_PRIORITY: i64 = 4;

impl PushChannelConfig {
    pub fn new(kind: PushChannelKind) -> Self {
        Self {
            kind,
            enabled: false,
            events: PushEventKind::default_selection().to_vec(),
            endpoint: String::new(),
            target: String::new(),
            token: String::new(),
            secret: String::new(),
            group: String::new(),
            icon: String::new(),
            sound: String::new(),
            priority: DEFAULT_PRIORITY,
            include_details: !kind.is_group_chat(),
        }
    }

    /// The first reason this channel cannot send, or `None` when it can.
    pub fn problem(&self) -> Option<PushConfigProblem> {
        let probe = PushMessage {
            kind: PushEventKind::Completion,
            session_id: String::new(),
            title: String::new(),
            headline: String::new(),
            body: String::new(),
            blocks_agent: false,
        };
        build_request(&probe, self, 0.0).err()
    }

    pub fn is_configured(&self) -> bool {
        self.problem().is_none()
    }

    /// Whether this channel takes a push of `kind` right now.
    pub fn accepts(&self, kind: PushEventKind) -> bool {
        self.enabled && self.events.contains(&kind) && self.is_configured()
    }

    /// One config per kind, in `all()` order. A missing or unreadable entry comes back as
    /// a default (disabled), so a corrupt setting disables pushes rather than crashing.
    pub fn decode_list(json: &str) -> Vec<Self> {
        let entries: Vec<Value> = serde_json::from_str(json).unwrap_or_default();
        let mut decoded: Vec<Self> = Vec::new();
        for entry in entries {
            if let Ok(config) = serde_json::from_value::<Self>(entry) {
                if !decoded.iter().any(|known| known.kind == config.kind) {
                    decoded.push(config);
                }
            }
        }
        PushChannelKind::all()
            .into_iter()
            .map(|kind| {
                decoded
                    .iter()
                    .find(|config| config.kind == kind)
                    .cloned()
                    .unwrap_or_else(|| Self::new(kind))
            })
            .collect()
    }

    /// The list as the settings file stores it: keys sorted, `events` as names.
    pub fn encode_list(configs: &[Self]) -> String {
        serde_json::to_string(configs).unwrap_or_default()
    }
}

/// Every kind, in the order the settings page lists them.
impl PushChannelKind {
    pub fn all() -> [Self; 7] {
        [
            Self::Bark,
            Self::Ntfy,
            Self::Dingtalk,
            Self::Feishu,
            Self::Wecom,
            Self::Slack,
            Self::Telegram,
        ]
    }
}

impl serde::Serialize for PushChannelConfig {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let kind = match self.kind {
            PushChannelKind::Bark => "bark",
            PushChannelKind::Ntfy => "ntfy",
            PushChannelKind::Dingtalk => "dingtalk",
            PushChannelKind::Feishu => "feishu",
            PushChannelKind::Wecom => "wecom",
            PushChannelKind::Slack => "slack",
            PushChannelKind::Telegram => "telegram",
        };
        let mut events: Vec<&str> = self.events.iter().map(|kind| kind_name(*kind)).collect();
        events.sort_unstable();
        let mut out = serializer.serialize_struct("PushChannelConfig", 12)?;
        out.serialize_field("kind", kind)?;
        out.serialize_field("enabled", &self.enabled)?;
        out.serialize_field("events", &events)?;
        out.serialize_field("endpoint", &self.endpoint)?;
        out.serialize_field("target", &self.target)?;
        out.serialize_field("token", &self.token)?;
        out.serialize_field("secret", &self.secret)?;
        out.serialize_field("group", &self.group)?;
        out.serialize_field("icon", &self.icon)?;
        out.serialize_field("sound", &self.sound)?;
        out.serialize_field("priority", &self.priority)?;
        out.serialize_field("includeDetails", &self.include_details)?;
        out.end()
    }
}

fn kind_name(kind: PushEventKind) -> &'static str {
    match kind {
        PushEventKind::Permission => "permission",
        PushEventKind::Question => "question",
        PushEventKind::Completion => "completion",
        PushEventKind::Error => "error",
        PushEventKind::Reminder => "reminder",
    }
}

fn event_kind(name: &str) -> Option<PushEventKind> {
    Some(match name {
        "permission" => PushEventKind::Permission,
        "question" => PushEventKind::Question,
        "completion" => PushEventKind::Completion,
        "error" => PushEventKind::Error,
        "reminder" => PushEventKind::Reminder,
        _ => return None,
    })
}

impl<'de> serde::Deserialize<'de> for PushChannelConfig {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw: Value = Value::deserialize(deserializer)?;
        let kind = match raw.get("kind").and_then(|kind| kind.as_str()) {
            Some("bark") => PushChannelKind::Bark,
            Some("ntfy") => PushChannelKind::Ntfy,
            Some("dingtalk") => PushChannelKind::Dingtalk,
            Some("feishu") => PushChannelKind::Feishu,
            Some("wecom") => PushChannelKind::Wecom,
            Some("slack") => PushChannelKind::Slack,
            Some("telegram") => PushChannelKind::Telegram,
            // A kind this build does not know: the entry is skipped, not the list.
            _ => return Err(serde::de::Error::custom("unknown channel kind")),
        };
        let mut config = Self::new(kind);
        let text = |key: &str| {
            raw.get(key)
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_owned()
        };
        config.enabled = raw
            .get("enabled")
            .and_then(|value| value.as_bool())
            .unwrap_or(config.enabled);
        if let Some(events) = raw.get("events").and_then(|value| value.as_array()) {
            let known: Vec<PushEventKind> = events
                .iter()
                .filter_map(|event| event.as_str())
                .filter_map(event_kind)
                .collect();
            if !known.is_empty() {
                config.events = known;
            }
        }
        config.endpoint = text("endpoint");
        config.target = text("target");
        config.token = text("token");
        config.secret = text("secret");
        config.group = text("group");
        config.icon = text("icon");
        config.sound = text("sound");
        if let Some(priority) = raw.get("priority").and_then(|value| value.as_i64()) {
            config.priority = priority;
        }
        if let Some(details) = raw.get("includeDetails").and_then(|value| value.as_bool()) {
            config.include_details = details;
        }
        Ok(config)
    }
}

// MARK: - The request

/// The user agent every request carries, so a self-hosted server's log says who called.
pub const USER_AGENT: &str = "Morrow-Push/1.0";
/// Footer on group-chat bots, so DingTalk / Feishu "custom keyword" security can be
/// satisfied with the keyword "CodeIsland".
pub const KEYWORD_FOOTER: &str = "— Morrow";

/// APNs counts bytes for title, subtitle, group and bark-server's own fields together.
const BARK_BODY_BYTES: usize = 2_500;
const BARK_TITLE_LIMIT: usize = 120;
const BARK_SUBTITLE_LIMIT: usize = 200;
/// ntfy turns longer messages into attachments (default limit 4096 bytes).
const NTFY_MESSAGE_BYTES: usize = 3_800;
/// WeCom text content: at most 2048 UTF-8 bytes.
const WECOM_CONTENT_BYTES: usize = 2_048;
/// Telegram: 1–4096 characters after entity parsing, counted in UTF-16 units.
const TELEGRAM_TEXT_LIMIT: usize = 3_900;
const CHAT_TEXT_LIMIT: usize = 4_000;
const SLACK_TEXT_LIMIT: usize = 3_000;

/// The request one channel would send for `message`.
pub fn build_request(
    message: &PushMessage,
    channel: &PushChannelConfig,
    now: f64,
) -> Result<PushHttpRequest, PushConfigProblem> {
    match channel.kind {
        PushChannelKind::Bark => bark(message, channel),
        PushChannelKind::Ntfy => ntfy(message, channel),
        PushChannelKind::Dingtalk => dingtalk(message, channel, now),
        PushChannelKind::Feishu => feishu(message, channel, now),
        PushChannelKind::Wecom => wecom(message, channel),
        PushChannelKind::Slack => slack(message, channel),
        PushChannelKind::Telegram => telegram(message, channel),
    }
}

fn bark(
    message: &PushMessage,
    channel: &PushChannelConfig,
) -> Result<PushHttpRequest, PushConfigProblem> {
    let server_text =
        non_empty(&channel.endpoint).unwrap_or_else(|| channel.kind.default_endpoint().to_owned());
    let server = PushEndpoint::parse(&server_text).ok_or(PushConfigProblem::InvalidUrl)?;
    let mut base = server.url.clone();
    let mut device_key = non_empty(&channel.target);
    if device_key.is_none() {
        // The Bark app hands out "https://api.day.app/<key>/" and people paste it whole.
        let components = PushEndpoint::path_components(&base);
        if let Some(last) = components.last().filter(|last| *last != "push") {
            device_key = Some(last.clone());
            base = PushEndpoint::replacing_path(&base, &components[..components.len() - 1]);
        }
    }
    let device_key = device_key.ok_or(PushConfigProblem::MissingDeviceKey)?;
    let mut components = PushEndpoint::path_components(&base);
    if components.last() == Some(&device_key) {
        // The key was pasted into the URL *and* typed into its own field.
        components.pop();
        base = PushEndpoint::replacing_path(&base, &components);
    }
    let url = if components.last().map(String::as_str) == Some("push") {
        base
    } else {
        components.push("push".to_owned());
        PushEndpoint::replacing_path(&base, &components)
    };

    let headline = truncated(&message.headline, BARK_SUBTITLE_LIMIT);
    let mut body = Map::new();
    body.insert("device_key".to_owned(), json!(device_key));
    body.insert(
        "title".to_owned(),
        json!(truncated(&message.title, BARK_TITLE_LIMIT)),
    );
    body.insert(
        "level".to_owned(),
        json!(if message.blocks_agent {
            "timeSensitive"
        } else {
            "active"
        }),
    );
    body.insert(
        "group".to_owned(),
        json!(non_empty(&channel.group).unwrap_or_else(|| "CodeIsland".to_owned())),
    );
    if message.body.is_empty() {
        // Bark requires a body; an empty one falls back to the headline.
        body.insert("body".to_owned(), json!(headline));
    } else {
        body.insert("subtitle".to_owned(), json!(headline));
        body.insert(
            "body".to_owned(),
            json!(truncated_utf8(&message.body, BARK_BODY_BYTES)),
        );
    }
    if let Some(icon) = non_empty(&channel.icon) {
        body.insert("icon".to_owned(), json!(icon));
    }
    if let Some(sound) = non_empty(&channel.sound) {
        body.insert("sound".to_owned(), json!(sound));
    }
    Ok(json_request(
        &url,
        &Value::Object(body),
        server.authorization.as_deref(),
    ))
}

fn ntfy(
    message: &PushMessage,
    channel: &PushChannelConfig,
) -> Result<PushHttpRequest, PushConfigProblem> {
    let server_text =
        non_empty(&channel.endpoint).unwrap_or_else(|| channel.kind.default_endpoint().to_owned());
    let server = PushEndpoint::parse(&server_text).ok_or(PushConfigProblem::InvalidUrl)?;
    let mut components = PushEndpoint::path_components(&server.url);
    let mut topic = non_empty(&channel.target);
    if let Some(last) = components.last().cloned() {
        match &topic {
            // "https://ntfy.sh/mytopic" pasted whole.
            None => topic = Some(last),
            Some(topic) if *topic != last => {
                return Err(PushConfigProblem::NtfyTopicMismatch);
            }
            Some(_) => {}
        }
        components.pop();
    }
    let topic = topic.ok_or(PushConfigProblem::MissingTopic)?;
    let urgent = channel.priority.clamp(1, 5);
    let priority = if message.blocks_agent {
        urgent
    } else {
        urgent.min(3)
    };
    let body = json!({
        "topic": topic,
        "title": message.title,
        "message": truncated_utf8(&message.text(), NTFY_MESSAGE_BYTES),
        "priority": priority,
    });
    let authorization = match non_empty(&channel.token) {
        Some(token) if token.to_lowercase().starts_with("bearer ") => Some(token),
        Some(token) => Some(format!("Bearer {token}")),
        None => server.authorization.clone(),
    };
    let url = PushEndpoint::replacing_path(&server.url, &components);
    Ok(json_request(&url, &body, authorization.as_deref()))
}

fn dingtalk(
    message: &PushMessage,
    channel: &PushChannelConfig,
    now: f64,
) -> Result<PushHttpRequest, PushConfigProblem> {
    let webhook = non_empty(&channel.endpoint).ok_or(PushConfigProblem::MissingWebhook)?;
    let endpoint = PushEndpoint::parse(&webhook).ok_or(PushConfigProblem::InvalidUrl)?;
    let mut url = endpoint.url.clone();
    if let Some(secret) = non_empty(&channel.secret) {
        let timestamp = (now * 1000.0).round() as i64;
        let sign = PushSigning::dingtalk_signature(&secret, timestamp);
        url = PushEndpoint::setting_query(
            &url,
            &[
                ("timestamp".to_owned(), timestamp.to_string()),
                ("sign".to_owned(), PushSigning::form_url_encoded(&sign)),
            ],
        );
    }
    let content = truncated(&chat_text(message, true), CHAT_TEXT_LIMIT);
    let body = json!({"msgtype": "text", "text": {"content": content}});
    Ok(json_request(&url, &body, endpoint.authorization.as_deref()))
}

fn feishu(
    message: &PushMessage,
    channel: &PushChannelConfig,
    now: f64,
) -> Result<PushHttpRequest, PushConfigProblem> {
    let webhook = non_empty(&channel.endpoint).ok_or(PushConfigProblem::MissingWebhook)?;
    let endpoint = PushEndpoint::parse(&webhook).ok_or(PushConfigProblem::InvalidUrl)?;
    let text = truncated(
        &[message.text(), KEYWORD_FOOTER.to_owned()].join("\n"),
        CHAT_TEXT_LIMIT,
    );
    let paragraphs: Vec<Value> = text
        .split('\n')
        .map(|line| json!([{"tag": "text", "text": line}]))
        .collect();
    let mut body = json!({
        "msg_type": "post",
        "content": {"post": {"zh_cn": {"title": message.title, "content": paragraphs}}},
    });
    if let Some(secret) = non_empty(&channel.secret) {
        let timestamp = now.floor() as i64;
        body["timestamp"] = json!(timestamp.to_string());
        body["sign"] = json!(PushSigning::feishu_signature(&secret, timestamp));
    }
    Ok(json_request(
        &endpoint.url,
        &body,
        endpoint.authorization.as_deref(),
    ))
}

fn wecom(
    message: &PushMessage,
    channel: &PushChannelConfig,
) -> Result<PushHttpRequest, PushConfigProblem> {
    let webhook = non_empty(&channel.endpoint).ok_or(PushConfigProblem::MissingWebhook)?;
    let endpoint = PushEndpoint::parse(&webhook).ok_or(PushConfigProblem::InvalidUrl)?;
    let content = truncated_utf8(&chat_text(message, false), WECOM_CONTENT_BYTES);
    let body = json!({"msgtype": "text", "text": {"content": content}});
    Ok(json_request(
        &endpoint.url,
        &body,
        endpoint.authorization.as_deref(),
    ))
}

fn slack(
    message: &PushMessage,
    channel: &PushChannelConfig,
) -> Result<PushHttpRequest, PushConfigProblem> {
    let webhook = non_empty(&channel.endpoint).ok_or(PushConfigProblem::MissingWebhook)?;
    let endpoint = PushEndpoint::parse(&webhook).ok_or(PushConfigProblem::InvalidUrl)?;
    let text = truncated(&chat_text(message, false), SLACK_TEXT_LIMIT);
    let body = json!({"text": PushSigning::html_escaped(&text), "mrkdwn": false});
    Ok(json_request(
        &endpoint.url,
        &body,
        endpoint.authorization.as_deref(),
    ))
}

fn telegram(
    message: &PushMessage,
    channel: &PushChannelConfig,
) -> Result<PushHttpRequest, PushConfigProblem> {
    let mut token = non_empty(&channel.token).ok_or(PushConfigProblem::MissingBotToken)?;
    let chat_id = non_empty(&channel.target).ok_or(PushConfigProblem::MissingChatId)?;
    // BotFather's token is "123:ABC"; people also paste the URL form "bot123:ABC".
    if let Some(rest) = token.strip_prefix("bot") {
        if rest.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
            token = rest.to_owned();
        }
    }
    let api_text =
        non_empty(&channel.endpoint).unwrap_or_else(|| channel.kind.default_endpoint().to_owned());
    let api = PushEndpoint::parse(&api_text).ok_or(PushConfigProblem::InvalidUrl)?;
    let mut components = PushEndpoint::path_components(&api.url);
    components.push(format!("bot{token}"));
    components.push("sendMessage".to_owned());
    let url = PushEndpoint::replacing_path(&api.url, &components);
    // Title, newline and body share the limit: the tags and the entities escaping adds
    // count as nothing once parsed.
    let budget = TELEGRAM_TEXT_LIMIT
        .saturating_sub(message.title.encode_utf16().count() + 1)
        .max(100);
    let body_text = truncated_utf16(&message.text(), budget);
    let html = format!(
        "<b>{}</b>\n{}",
        PushSigning::html_escaped(&message.title),
        PushSigning::html_escaped(&body_text)
    );
    let body = json!({
        "chat_id": chat_id,
        "text": html,
        "parse_mode": "HTML",
        "link_preview_options": {"is_disabled": true},
    });
    Ok(json_request(&url, &body, api.authorization.as_deref()))
}

/// Title, headline and body as lines — for chat bots with no title slot.
pub fn chat_text(message: &PushMessage, footer: bool) -> String {
    let mut lines = vec![message.title.clone(), message.text()];
    if footer {
        lines.push(KEYWORD_FOOTER.to_owned());
    }
    lines
        .into_iter()
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn json_request(url: &str, body: &Value, authorization: Option<&str>) -> PushHttpRequest {
    let mut request = PushHttpRequest::post(url, serde_json::to_vec(body).unwrap_or_default())
        .header("Content-Type", "application/json; charset=utf-8")
        .header("User-Agent", USER_AGENT);
    if let Some(authorization) = authorization {
        request = request.header("Authorization", authorization);
    }
    request
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

// MARK: - URLs

/// A parsed endpoint: the URL, plus the Basic credentials lifted out of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
    pub url: String,
    pub authorization: Option<String>,
}

/// User-typed URLs.
pub struct PushEndpoint;

impl PushEndpoint {
    /// Parses a user-typed URL: adds `https://` when the scheme is missing and moves
    /// `user:password@` out of the URL into a Basic `Authorization` value — sent
    /// explicitly rather than trusting the loader to answer a 401 with credentials it
    /// found in the URL.
    pub fn parse(raw: &str) -> Option<Endpoint> {
        let text = raw.trim();
        if text.is_empty() {
            return None;
        }
        let text = if text.contains("://") {
            text.to_owned()
        } else {
            format!("https://{text}")
        };
        let (scheme, rest) = text.split_once("://")?;
        if scheme.is_empty() || rest.is_empty() {
            return None;
        }
        let (authority, tail) = match rest.find(['/', '?', '#']) {
            Some(index) => (&rest[..index], &rest[index..]),
            None => (rest, ""),
        };
        let (credentials, host) = match authority.rsplit_once('@') {
            Some((credentials, host)) => (Some(credentials), host),
            None => (None, authority),
        };
        if host.is_empty() {
            return None;
        }
        let authorization =
            credentials.map(|credentials| format!("Basic {}", base64(credentials.as_bytes())));
        Some(Endpoint {
            url: format!("{scheme}://{host}{tail}"),
            authorization,
        })
    }

    /// The path segments of a URL, without the empty ones a trailing slash leaves.
    pub fn path_components(url: &str) -> Vec<String> {
        let (_, rest) = url.split_once("://").unwrap_or(("", url));
        let path = match rest.find(['?', '#']) {
            Some(index) => &rest[..index],
            None => rest,
        };
        let path = match path.find('/') {
            Some(index) => &path[index..],
            None => "",
        };
        path.split('/')
            .filter(|segment| !segment.is_empty())
            .map(str::to_owned)
            .collect()
    }

    /// The same URL with a different path.
    pub fn replacing_path(url: &str, components: &[String]) -> String {
        let (scheme, rest) = url.split_once("://").unwrap_or(("https", url));
        let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let authority = &rest[..authority_end];
        let tail = &rest[authority_end..];
        let query = tail.find(['?', '#']).map_or("", |index| &tail[index..]);
        let path = if components.is_empty() {
            String::new()
        } else {
            format!("/{}", components.join("/"))
        };
        format!("{scheme}://{authority}{path}{query}")
    }

    /// The same URL with `items` set in its query: an item already there is **replaced**,
    /// everything else is kept.
    ///
    /// Keeping the rest matters at the one call site: DingTalk's webhook URL carries its
    /// `access_token` in the query, and that token *is* the credential. Replacing the
    /// query instead of adding to it produced a request to a webhook that no longer
    /// knew who was calling.
    ///
    /// Values arrive already percent-encoded — a Base64 signature carries `+`, which a
    /// server decodes as a space if it is encoded by a generic encoder.
    pub fn setting_query(url: &str, items: &[(String, String)]) -> String {
        let (base, query) = match url.split_once('?') {
            Some((base, query)) => (base, query),
            None => (url, ""),
        };
        let replaced: Vec<&str> = items.iter().map(|(name, _)| name.as_str()).collect();
        let mut pairs: Vec<String> = query
            .split('&')
            .filter(|pair| !pair.is_empty())
            .filter(|pair| {
                let name = pair.split('=').next().unwrap_or_default();
                !replaced.contains(&name)
            })
            .map(str::to_owned)
            .collect();
        pairs.extend(items.iter().map(|(name, value)| format!("{name}={value}")));
        if pairs.is_empty() {
            return base.to_owned();
        }
        format!("{base}?{}", pairs.join("&"))
    }
}

/// Base64 for the Basic authorization header, standard alphabet with padding.
fn base64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

// MARK: - Signing

/// The two signatures a chat service asks for, and the encoding DingTalk's samples use.
pub struct PushSigning;

impl PushSigning {
    /// DingTalk "加签": `Base64(HMAC-SHA256(key: secret, message: "{ms}\n{secret}"))`.
    /// The caller URL-encodes it.
    pub fn dingtalk_signature(secret: &str, timestamp_millis: i64) -> String {
        let string_to_sign = format!("{timestamp_millis}\n{secret}");
        hmac_sha256(secret.as_bytes(), string_to_sign.as_bytes())
    }

    /// Feishu / Lark: `Base64(HMAC-SHA256(key: "{s}\n{secret}", message: empty))`. Note
    /// the inversion against DingTalk — Feishu keys the MAC with the string-to-sign and
    /// signs nothing.
    pub fn feishu_signature(secret: &str, timestamp_seconds: i64) -> String {
        let key = format!("{timestamp_seconds}\n{secret}");
        hmac_sha256(key.as_bytes(), b"")
    }

    /// `application/x-www-form-urlencoded` as Java's `URLEncoder` produces it — the
    /// encoding DingTalk's samples use.
    pub fn form_url_encoded(value: &str) -> String {
        let mut out = String::new();
        for byte in value.bytes() {
            match byte {
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'*' => {
                    out.push(byte as char);
                }
                b' ' => out.push('+'),
                other => out.push_str(&format!("%{other:02X}")),
            }
        }
        out
    }

    /// Slack's and Telegram's escaping: exactly `&`, `<`, `>`.
    pub fn html_escaped(text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    }
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC takes any key length");
    mac.update(message);
    base64(&mac.finalize().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message() -> PushMessage {
        PushMessage {
            kind: PushEventKind::Permission,
            session_id: "s1".to_owned(),
            title: "🔐 Claude · vibe".to_owned(),
            headline: "Needs approval: Bash".to_owned(),
            body: "rm -rf build".to_owned(),
            blocks_agent: true,
        }
    }

    fn channel(kind: PushChannelKind) -> PushChannelConfig {
        PushChannelConfig::new(kind)
    }

    fn body_json(request: &PushHttpRequest) -> Value {
        serde_json::from_slice(&request.body).expect("a JSON body")
    }

    #[test]
    fn only_the_hosted_services_have_a_default_endpoint() {
        assert_eq!(
            PushChannelKind::Bark.default_endpoint(),
            "https://api.day.app"
        );
        assert_eq!(PushChannelKind::Ntfy.default_endpoint(), "https://ntfy.sh");
        assert_eq!(
            PushChannelKind::Telegram.default_endpoint(),
            "https://api.telegram.org"
        );
        for webhook in [
            PushChannelKind::Dingtalk,
            PushChannelKind::Feishu,
            PushChannelKind::Wecom,
            PushChannelKind::Slack,
        ] {
            assert_eq!(webhook.default_endpoint(), "");
            assert!(webhook.is_group_chat());
        }
        assert!(!PushChannelKind::Bark.is_group_chat());
    }

    #[test]
    fn a_fresh_channel_is_off_and_a_team_chat_starts_without_details() {
        let personal = channel(PushChannelKind::Bark);
        assert!(!personal.enabled);
        assert!(personal.include_details);
        assert_eq!(personal.priority, DEFAULT_PRIORITY);
        assert_eq!(personal.events.len(), 5);

        let group = channel(PushChannelKind::Slack);
        assert!(!group.include_details, "everyone in the group reads it");
    }

    #[test]
    fn a_channel_that_cannot_send_says_why_before_trying() {
        assert_eq!(
            channel(PushChannelKind::Bark).problem(),
            Some(PushConfigProblem::MissingDeviceKey)
        );
        assert_eq!(
            channel(PushChannelKind::Ntfy).problem(),
            Some(PushConfigProblem::MissingTopic)
        );
        assert_eq!(
            channel(PushChannelKind::Slack).problem(),
            Some(PushConfigProblem::MissingWebhook)
        );
        assert_eq!(
            channel(PushChannelKind::Telegram).problem(),
            Some(PushConfigProblem::MissingBotToken)
        );
        assert!(!channel(PushChannelKind::Bark).is_configured());
        // `accepts` is false for any of those.
        assert!(!channel(PushChannelKind::Bark).accepts(PushEventKind::Permission));
    }

    #[test]
    fn a_bark_push_is_a_post_to_push_with_a_time_sensitive_level() {
        let mut config = channel(PushChannelKind::Bark);
        config.target = "device-key".to_owned();
        config.enabled = true;
        let request = build_request(&message(), &config, 0.0).expect("a request");
        assert_eq!(request.url, "https://api.day.app/push");
        assert_eq!(request.method, "POST");
        let body = body_json(&request);
        assert_eq!(body["device_key"], "device-key");
        assert_eq!(body["level"], "timeSensitive");
        assert_eq!(body["group"], "CodeIsland");
        assert_eq!(body["subtitle"], "Needs approval: Bash");
        assert_eq!(body["body"], "rm -rf build");
        // A non-blocking push does not break through Focus.
        let quiet = PushMessage {
            blocks_agent: false,
            ..message()
        };
        assert_eq!(
            body_json(&build_request(&quiet, &config, 0.0).unwrap())["level"],
            "active"
        );
    }

    #[test]
    fn a_bark_key_pasted_in_the_url_is_lifted_out_and_not_repeated() {
        let mut config = channel(PushChannelKind::Bark);
        config.endpoint = "https://api.day.app/KEY123".to_owned();
        let request = build_request(&message(), &config, 0.0).expect("a request");
        assert_eq!(request.url, "https://api.day.app/push");
        assert_eq!(body_json(&request)["device_key"], "KEY123");

        // Pasted in both places: one copy, not two path segments.
        config.endpoint = "https://api.day.app/KEY123".to_owned();
        config.target = "KEY123".to_owned();
        let request = build_request(&message(), &config, 0.0).expect("a request");
        assert_eq!(request.url, "https://api.day.app/push");
    }

    #[test]
    fn an_empty_bark_body_falls_back_to_the_headline() {
        let mut config = channel(PushChannelKind::Bark);
        config.target = "k".to_owned();
        let headline_only = PushMessage {
            body: String::new(),
            ..message()
        };
        let body = body_json(&build_request(&headline_only, &config, 0.0).unwrap());
        assert_eq!(body["body"], "Needs approval: Bash");
        assert!(body.get("subtitle").is_none());
    }

    #[test]
    fn ntfy_publishes_json_at_the_server_root_and_reads_a_pasted_topic() {
        let mut config = channel(PushChannelKind::Ntfy);
        config.endpoint = "https://ntfy.sh/mytopic".to_owned();
        let request = build_request(&message(), &config, 0.0).expect("a request");
        assert_eq!(
            request.url, "https://ntfy.sh",
            "JSON at the root, or ntfy publishes it as text"
        );
        assert_eq!(body_json(&request)["topic"], "mytopic");

        // The topic in its own field matches the one in the URL.
        config.target = "mytopic".to_owned();
        assert!(build_request(&message(), &config, 0.0).is_ok());

        // A different one is a contradiction only the user can resolve.
        config.target = "other".to_owned();
        assert_eq!(
            build_request(&message(), &config, 0.0).unwrap_err(),
            PushConfigProblem::NtfyTopicMismatch
        );
    }

    #[test]
    fn ntfy_priority_is_urgent_only_for_a_blocking_push() {
        let mut config = channel(PushChannelKind::Ntfy);
        config.target = "topic".to_owned();
        config.priority = 5;
        assert_eq!(
            body_json(&build_request(&message(), &config, 0.0).unwrap())["priority"],
            5
        );
        let quiet = PushMessage {
            blocks_agent: false,
            ..message()
        };
        assert_eq!(
            body_json(&build_request(&quiet, &config, 0.0).unwrap())["priority"],
            3
        );

        // Out of range is clamped to what the service accepts.
        config.priority = 99;
        assert_eq!(
            body_json(&build_request(&message(), &config, 0.0).unwrap())["priority"],
            5
        );
    }

    #[test]
    fn ntfy_token_becomes_a_bearer_header_unless_it_already_says_so() {
        let mut config = channel(PushChannelKind::Ntfy);
        config.target = "topic".to_owned();
        config.token = "tk_secret".to_owned();
        let request = build_request(&message(), &config, 0.0).unwrap();
        assert!(
            request
                .headers
                .contains(&("Authorization".to_owned(), "Bearer tk_secret".to_owned()))
        );

        config.token = "Bearer tk_secret".to_owned();
        let request = build_request(&message(), &config, 0.0).unwrap();
        assert!(
            request
                .headers
                .contains(&("Authorization".to_owned(), "Bearer tk_secret".to_owned()))
        );
    }

    #[test]
    fn a_signed_dingtalk_url_carries_the_timestamp_and_the_encoded_signature() {
        let mut config = channel(PushChannelKind::Dingtalk);
        config.endpoint = "https://oapi.dingtalk.com/robot/send?access_token=abc".to_owned();
        config.secret = "SECxyz".to_owned();
        let request = build_request(&message(), &config, 1_700_000_000.0).unwrap();
        assert!(
            request
                .url
                .starts_with("https://oapi.dingtalk.com/robot/send?")
        );
        assert!(
            request.url.contains("timestamp=1700000000000"),
            "{}",
            request.url
        );
        assert!(request.url.contains("&sign="), "{}", request.url);
        // The access token is a credential: it is on the URL the user typed, not dropped.
        assert!(request.url.contains("access_token=abc"));
        assert_eq!(body_json(&request)["msgtype"], "text");
        let content = body_json(&request)["text"]["content"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(
            content.ends_with(KEYWORD_FOOTER),
            "the keyword satisfies their security check"
        );
    }

    #[test]
    fn an_unsigned_dingtalk_request_leaves_the_url_alone() {
        let mut config = channel(PushChannelKind::Dingtalk);
        config.endpoint = "https://oapi.dingtalk.com/robot/send?access_token=abc".to_owned();
        let request = build_request(&message(), &config, 0.0).unwrap();
        assert_eq!(
            request.url,
            "https://oapi.dingtalk.com/robot/send?access_token=abc"
        );
    }

    #[test]
    fn feishu_posts_paragraphs_and_puts_the_signature_in_the_body() {
        let mut config = channel(PushChannelKind::Feishu);
        config.endpoint = "https://open.feishu.cn/open-apis/bot/v2/hook/xyz".to_owned();
        config.secret = "s3cret".to_owned();
        let request = build_request(&message(), &config, 1_700_000_000.0).unwrap();
        let body = body_json(&request);
        assert_eq!(body["msg_type"], "post");
        let paragraphs = body["content"]["post"]["zh_cn"]["content"]
            .as_array()
            .unwrap();
        assert!(paragraphs.len() >= 2);
        assert_eq!(body["timestamp"], "1700000000");
        assert!(body["sign"].as_str().is_some_and(|sign| !sign.is_empty()));
    }

    #[test]
    fn slack_escapes_what_it_would_read_as_markup_and_keeps_commands_literal() {
        let mut config = channel(PushChannelKind::Slack);
        config.endpoint = "https://hooks.slack.com/services/T/B/X".to_owned();
        let with_angle = PushMessage {
            headline: "grep <pattern> & more".to_owned(),
            body: String::new(),
            ..message()
        };
        let body = body_json(&build_request(&with_angle, &config, 0.0).unwrap());
        assert_eq!(body["mrkdwn"], false, "a command must survive as typed");
        let text = body["text"].as_str().unwrap();
        assert!(text.contains("&lt;pattern&gt;"), "{text}");
        assert!(text.contains("&amp;"), "{text}");
        assert!(!text.contains('<'));
    }

    #[test]
    fn wecom_stays_inside_its_byte_budget() {
        let mut config = channel(PushChannelKind::Wecom);
        config.endpoint = "https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=abc".to_owned();
        let long = PushMessage {
            body: "日本語".repeat(1_000),
            ..message()
        };
        let body = body_json(&build_request(&long, &config, 0.0).unwrap());
        let content = body["text"]["content"].as_str().unwrap();
        assert!(
            content.len() <= WECOM_CONTENT_BYTES,
            "{} bytes",
            content.len()
        );
    }

    #[test]
    fn telegram_builds_bot_token_url_with_html_and_no_link_preview() {
        let mut config = channel(PushChannelKind::Telegram);
        config.token = "123:ABC".to_owned();
        config.target = "42".to_owned();
        let request = build_request(&message(), &config, 0.0).unwrap();
        assert_eq!(
            request.url,
            "https://api.telegram.org/bot123:ABC/sendMessage"
        );
        let body = body_json(&request);
        assert_eq!(body["chat_id"], "42");
        assert_eq!(body["parse_mode"], "HTML");
        assert_eq!(body["link_preview_options"]["is_disabled"], true);
        let text = body["text"].as_str().unwrap();
        assert!(text.starts_with("<b>"), "{text}");
        assert!(text.contains("\nrm -rf build"), "{text}");
    }

    #[test]
    fn a_telegram_token_pasted_as_a_url_is_accepted() {
        let mut config = channel(PushChannelKind::Telegram);
        config.token = "bot123:ABC".to_owned();
        config.target = "42".to_owned();
        let request = build_request(&message(), &config, 0.0).unwrap();
        assert!(
            request.url.contains("/bot123:ABC/sendMessage"),
            "{}",
            request.url
        );
        assert!(!request.url.contains("botbot"));
    }

    #[test]
    fn a_basic_auth_url_is_lifted_out_of_the_url() {
        let endpoint = PushEndpoint::parse("user:pass@example.com/hook").expect("a URL");
        assert_eq!(endpoint.url, "https://example.com/hook");
        assert_eq!(
            endpoint.authorization.as_deref(),
            Some("Basic dXNlcjpwYXNz")
        );
        // No credentials: no header.
        let plain = PushEndpoint::parse("https://example.com/hook").expect("a URL");
        assert_eq!(plain.authorization, None);
        assert_eq!(PushEndpoint::parse("   "), None);
        assert_eq!(PushEndpoint::parse("https://"), None);
    }

    #[test]
    fn a_path_can_be_read_and_replaced_without_losing_the_query() {
        assert_eq!(
            PushEndpoint::path_components("https://example.com/a/b/c?x=1"),
            vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]
        );
        assert!(PushEndpoint::path_components("https://example.com").is_empty());
        assert_eq!(
            PushEndpoint::replacing_path("https://example.com/a/b", &["x".to_owned()]),
            "https://example.com/x"
        );
        assert_eq!(
            PushEndpoint::replacing_path("https://example.com/a?token=1", &[]),
            "https://example.com?token=1"
        );
        assert_eq!(
            PushEndpoint::setting_query(
                "https://example.com/a",
                &[("k".to_owned(), "v".to_owned())]
            ),
            "https://example.com/a?k=v"
        );
    }

    #[test]
    fn the_two_signatures_are_inverted_as_both_services_document() {
        // DingTalk: key is the secret, message is "<ms>\n<secret>".
        let dingtalk = PushSigning::dingtalk_signature("SECtest", 1_700_000_000_000);
        // Feishu: key is "<s>\n<secret>", message is empty.
        let feishu = PushSigning::feishu_signature("s3cret", 1_700_000_000);
        // Both are base64 of a 32-byte digest.
        assert_eq!(dingtalk.len(), 44);
        assert_eq!(feishu.len(), 44);
        assert!(dingtalk.ends_with('='));
        assert_ne!(dingtalk, feishu);
        // Stable for the same inputs, different for a different timestamp.
        assert_eq!(
            dingtalk,
            PushSigning::dingtalk_signature("SECtest", 1_700_000_000_000)
        );
        assert_ne!(
            dingtalk,
            PushSigning::dingtalk_signature("SECtest", 1_700_000_000_001)
        );
    }

    #[test]
    fn a_signature_is_url_encoded_the_way_dingtalks_samples_are() {
        assert_eq!(PushSigning::form_url_encoded("a b"), "a+b");
        assert_eq!(PushSigning::form_url_encoded("a/b+c="), "a%2Fb%2Bc%3D");
        assert_eq!(PushSigning::form_url_encoded("AZaz09-_.*"), "AZaz09-_.*");
        assert_eq!(PushSigning::form_url_encoded("=="), "%3D%3D");
    }

    #[test]
    fn the_config_survives_a_round_trip_and_an_unknown_kind_is_skipped() {
        let mut bark = channel(PushChannelKind::Bark);
        bark.enabled = true;
        bark.target = "key".to_owned();
        bark.events = vec![PushEventKind::Permission, PushEventKind::Error];
        let encoded = PushChannelConfig::encode_list(&[bark.clone()]);
        let decoded = PushChannelConfig::decode_list(&encoded);
        assert_eq!(decoded.len(), 7, "one per kind, always");
        let restored = decoded
            .iter()
            .find(|config| config.kind == PushChannelKind::Bark)
            .unwrap();
        assert_eq!(restored.target, "key");
        assert!(restored.enabled);
        assert_eq!(restored.events.len(), 2);

        // A corrupt entry costs that channel, not the list.
        let partial =
            r#"[{"kind":"nope","enabled":true},{"kind":"ntfy","enabled":true,"target":"t"}]"#;
        let decoded = PushChannelConfig::decode_list(partial);
        let ntfy = decoded
            .iter()
            .find(|config| config.kind == PushChannelKind::Ntfy)
            .unwrap();
        assert!(ntfy.enabled);
        assert_eq!(ntfy.target, "t");
        // Missing entries come back disabled.
        assert!(
            !decoded
                .iter()
                .find(|config| config.kind == PushChannelKind::Slack)
                .unwrap()
                .enabled
        );
        assert_eq!(PushChannelConfig::decode_list("not json").len(), 7);
    }

    #[test]
    fn accepts_needs_enabled_selected_and_configured() {
        let mut config = channel(PushChannelKind::Bark);
        config.target = "key".to_owned();
        assert!(
            !config.accepts(PushEventKind::Permission),
            "not enabled yet"
        );
        config.enabled = true;
        assert!(config.accepts(PushEventKind::Permission));
        config.events = vec![PushEventKind::Error];
        assert!(!config.accepts(PushEventKind::Permission));
        assert!(config.accepts(PushEventKind::Error));
        config.target.clear();
        assert!(
            !config.accepts(PushEventKind::Error),
            "a configured channel is required too"
        );
    }
}
