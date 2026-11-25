//! What a diagnostics export may take off the machine
//! (`macos/Sources/Morrow/DiagnosticsRedaction.swift`).
//!
//! A bug report's bundle ends up on a public issue, so the shell writes only
//! what Settings says it writes: settings and session structure, prompt
//! previews capped at 80 characters, and secrets replaced however deep they
//! are. This module is pure: the shell hands in the JSON it read and the
//! metadata it collected, and gets a bundle back.

use serde_json::{Map, Value};

/// Swift `DiagnosticsRedaction.redacted`.
pub const REDACTED: &str = "<redacted>";
/// Swift `DiagnosticsRedaction.promptPreviewLength`.
pub const PROMPT_PREVIEW_LENGTH: usize = 80;

const ENVIRONMENT_KEYS: &[&str] = &[
    "env",
    "environment",
    "envvars",
    "env_vars",
    "environmentvariables",
];
const SECRET_WORDS: &[&str] = &[
    "key",
    "keys",
    "apikey",
    "token",
    "tokens",
    "secret",
    "secrets",
    "password",
    "passwd",
    "passphrase",
    "auth",
    "authorization",
    "credential",
    "credentials",
    "cookie",
    "cookies",
    "bearer",
    "jwt",
];
const SECRET_FRAGMENTS: &[&str] = &[
    "apikey",
    "api_key",
    "api-key",
    "token",
    "secret",
    "password",
    "passwd",
    "authorization",
    "credential",
];

/// Swift `isEnvironmentKey`: an `env` block keeps its variable names.
pub fn is_environment_key(key: &str) -> bool {
    ENVIRONMENT_KEYS.contains(&key.to_lowercase().as_str())
}

/// Split `apiKeyHelper` into `api`, `key`, `helper` and `X-API-Key` into
/// `x`, `api`, `key`, the same way Swift's `words(of:)` does.
fn words(key: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut previous: Option<char> = None;
    for character in key.chars() {
        if !character.is_alphanumeric() {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
        } else {
            let boundary = character.is_uppercase()
                && previous.is_some_and(|p| p.is_lowercase() || p.is_ascii_digit())
                && !current.is_empty();
            if boundary {
                words.push(std::mem::take(&mut current));
            }
            current.push(character.to_lowercase().next().unwrap_or(character));
        }
        previous = Some(character);
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// Swift `isSecretKey`.
pub fn is_secret_key(key: &str) -> bool {
    let lowered = key.to_lowercase();
    if SECRET_FRAGMENTS.iter().any(|part| lowered.contains(part)) {
        return true;
    }
    words(key).iter().any(|word| SECRET_WORDS.contains(&word.as_str()))
}

/// The environment rule Swift applies: names survive, values do not.
fn redact_environment(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, _)| (key, Value::String(REDACTED.to_owned())))
                .collect(),
        ),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| match item {
                    Value::String(entry) => {
                        let value = entry.split_once('=').map_or(REDACTED, |(name, _)| name);
                        Value::String(format!("{value}={REDACTED}"))
                    }
                    other => redact_all(other),
                })
                .collect(),
        ),
        other => redact_all(other),
    }
}

/// Every string under a secret's key is replaced; numbers and flags stay.
fn redact_all(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, redact_all(value)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(redact_all).collect()),
        Value::String(_) => Value::String(REDACTED.to_owned()),
        other => other,
    }
}

/// A list where the value after a flag such as `--api-key` is replaced too.
fn redact_arguments(items: Vec<Value>) -> Vec<Value> {
    let mut result = Vec::with_capacity(items.len());
    let mut after_flag = false;
    for item in items {
        if after_flag {
            if item.is_string() {
                result.push(Value::String(REDACTED.to_owned()));
                after_flag = false;
                continue;
            }
            after_flag = false;
        }
        if let Value::String(flag) = &item {
            if flag.starts_with('-') && !flag.contains('=') {
                after_flag = is_secret_key(flag.trim_start_matches('-'));
            }
        }
        result.push(redact(item, None));
    }
    result
}

/// Swift `DiagnosticsRedaction.redact`: `value` with its secrets replaced,
/// `key` being the name it was found under.
pub fn redact(value: Value, key: Option<&str>) -> Value {
    if let Some(key) = key {
        if is_environment_key(key) {
            return redact_environment(value);
        }
        if is_secret_key(key) {
            return redact_all(value);
        }
    }
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let redacted = redact(value, Some(&key));
                    (key, redacted)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(redact_arguments(items)),
        Value::String(text) => Value::String(redact_secrets(&text)),
        other => other,
    }
}

/// Whether a character may appear inside a credential token.
fn token_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '~' | '+' | '/' | '=')
}

/// Replace the provider-key shapes Swift's regex table names. This is the
/// form-based half of `redactSecrets(in:)`; the key- and flag-based half is in
/// [`redact`]. Longest prefix first so a `github_pat_` is not cut at `ghp_`.
pub fn redact_secrets(text: &str) -> String {
    const PREFIXES: &[&str] = &[
        "github_pat_",
        "sk-",
        "ghp_",
        "gho_",
        "ghu_",
        "ghs_",
        "ghr_",
        "xox",
        "AKIA",
        "AIza",
    ];
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let rest = &text[index..];
        let matched = PREFIXES
            .iter()
            .find(|prefix| rest.starts_with(**prefix))
            .copied();
        if let Some(prefix) = matched {
            let after = index + prefix.len();
            let mut end = after;
            while end < bytes.len() && token_char(text[end..].chars().next().unwrap_or(' ')) {
                end += text[end..].chars().next().unwrap_or(' ').len_utf8();
            }
            let minimum = match prefix {
                "github_pat_" => 32,
                "sk-" | "xox" | "AIza" => 12,
                "AKIA" => 20,
                _ => 24,
            };
            if end - after >= minimum {
                out.push_str(REDACTED);
                index = end;
                continue;
            }
        }
        // `Authorization: Bearer …` and a password in a URL.
        let lowered = rest.to_lowercase();
        if lowered.starts_with("bearer ") || lowered.starts_with("basic ") {
            let space = rest.find(' ').unwrap_or(0);
            let after = space + 1;
            let mut end = after;
            while end < bytes.len() && token_char(text[end..].chars().next().unwrap_or(' ')) {
                end += text[end..].chars().next().unwrap_or(' ').len_utf8();
            }
            if end - after >= 8 {
                out.push_str(&rest[..after]);
                out.push_str(REDACTED);
                index += end;
                continue;
            }
        }
        let character = rest.chars().next().unwrap_or(' ');
        out.push(character);
        index += character.len_utf8();
    }
    out
}

/// Swift `promptPreview`: trimmed, first 80 characters, then redacted.
pub fn prompt_preview(prompt: &str) -> String {
    let head: String = prompt.trim().chars().take(PROMPT_PREVIEW_LENGTH).collect();
    redact_secrets(&head)
}

/// Swift `contentFree`: what was said becomes its shape, not its content.
/// Structure keys (`id`, `status`, `state`, `kind`, `type`, `source`) keep a
/// short, redacted value so a task list still says what it was doing.
fn content_free(value: Value, key: Option<&str>) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let free = content_free(value, Some(&key));
                    (key, free)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(|v| content_free(v, None)).collect()),
        Value::String(text) => {
            if key.is_some_and(|k| matches!(k, "id" | "status" | "state" | "kind" | "type" | "source")) {
                let head: String = text.chars().take(64).collect();
                Value::String(redact_secrets(&head))
            } else {
                Value::String(format!("<{} characters>", text.chars().count()))
            }
        }
        other => other,
    }
}

/// The session fields Swift keeps as structure (`sessionStructureKeys`).
const SESSION_STRUCTURE_KEYS: &[&str] = &[
    "sessionId",
    "cwd",
    "source",
    "model",
    "sessionTitleSource",
    "providerSessionId",
    "termApp",
    "ttyPath",
    "tmuxPane",
    "tmuxClientTty",
    "herdrPaneId",
    "herdrSocketPath",
    "cliPid",
    "cliStartTime",
    "startTime",
    "lastActivity",
    "transcriptPath",
    "parentSessionId",
];

/// One persisted session as exported: last prompt previewed, replies and
/// titles replaced by their length.
pub fn redact_persisted_session(session: &Value) -> Value {
    let Some(map) = session.as_object() else {
        return content_free(session.clone(), None);
    };
    let mut out = Map::new();
    for (key, value) in map {
        let exported = if key == "lastUserPrompt" {
            Value::String(prompt_preview(value.as_str().unwrap_or_default()))
        } else if SESSION_STRUCTURE_KEYS.contains(&key.as_str()) {
            redact(value.clone(), Some(key))
        } else {
            content_free(value.clone(), Some(key))
        };
        out.insert(key.clone(), exported);
    }
    Value::Object(out)
}

/// The whole bundle: metadata, settings and redacted sessions, in one object
/// the shell can write as pretty JSON.
pub fn bundle(metadata: Value, settings: &Value, sessions: &[Value]) -> Value {
    let mut root = Map::new();
    root.insert("metadata".to_owned(), redact(metadata, None));
    root.insert(
        "settings".to_owned(),
        redact(settings.clone(), None),
    );
    root.insert(
        "sessions".to_owned(),
        Value::Array(sessions.iter().map(redact_persisted_session).collect()),
    );
    Value::Object(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_secret_key_hides_its_value_but_not_its_shape() {
        let config = json!({ "apiKey": "sk-abcdefgh1234", "maxTokens": 8192, "enabled": true });
        let redacted = redact(config, None);
        assert_eq!(redacted["apiKey"], REDACTED);
        assert_eq!(redacted["maxTokens"], 8192);
        assert_eq!(redacted["enabled"], true);
    }

    #[test]
    fn an_env_block_keeps_names_and_loses_values() {
        let config = json!({ "env": { "HOME": "/Users/me", "TOKEN": "abc" } });
        assert_eq!(
            redact(config, None)["env"],
            json!({ "HOME": REDACTED, "TOKEN": REDACTED })
        );
    }

    #[test]
    fn a_flag_takes_the_value_after_it() {
        let args = json!(["node", "run", "--api-key", "abcd1234", "--port", "3000"]);
        assert_eq!(
            redact(args, None),
            json!(["node", "run", "--api-key", REDACTED, "--port", "3000"])
        );
    }

    #[test]
    fn provider_keys_are_replaced_in_text() {
        assert_eq!(redact_secrets("key sk-abcdefgh12345 here"), format!("key {REDACTED} here"));
        assert_eq!(redact_secrets("Bearer abcdefgh1234"), format!("Bearer {REDACTED}"));
        assert_eq!(redact_secrets("no secret here"), "no secret here");
    }

    #[test]
    fn a_prompt_is_trimmed_capped_and_redacted() {
        let long = format!("  {}sk-abcdefgh12345{}  ", "x".repeat(40), "x".repeat(60));
        let preview = prompt_preview(&long);
        assert!(preview.starts_with("xxxx"));
        assert!(preview.contains(REDACTED));
        assert!(!preview.contains("sk-"));
    }

    #[test]
    fn a_session_keeps_structure_and_drops_replies() {
        let session = json!({
            "sessionId": "abc12345",
            "source": "claude",
            "cwd": "/tmp/project",
            "lastUserPrompt": "hello",
            "lastReply": "a long reply",
            "subagentCount": 2
        });
        let exported = redact_persisted_session(&session);
        assert_eq!(exported["sessionId"], "abc12345");
        assert_eq!(exported["lastUserPrompt"], "hello");
        assert_eq!(exported["lastReply"], "<12 characters>");
        assert_eq!(exported["subagentCount"], 2);
    }

    #[test]
    fn a_bundle_has_three_sections() {
        let built = bundle(json!({ "appVersion": "0.1.0" }), &json!({}), &[json!({})]);
        assert_eq!(built["metadata"]["appVersion"], "0.1.0");
        assert!(built.get("settings").is_some());
        assert_eq!(built["sessions"].as_array().map(Vec::len), Some(1));
    }
}
