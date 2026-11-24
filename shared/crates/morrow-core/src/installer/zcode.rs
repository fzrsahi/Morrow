//! ZCode's `config.json` (#234 batch 3b): a hooks object with a master switch.
//!
//! Every other target in this crate puts events straight under the top-level
//! `hooks` key. ZCode wraps them one level deeper, beside a switch that belongs to
//! the user:
//!
//! ```json
//! {"hooks": {"enabled": true,
//!            "events": {"PreToolUse": [{"hooks": [{"type": "command", …}]}]}}}
//! ```
//!
//! Two rules come with that shape, and both are the kind that fail silently:
//!
//! - **`enabled` is never flipped off.** It is the user's switch over *all* their
//!   hooks, including third-party ones they turned off on purpose. This installer
//!   seeds it when it is absent and leaves an explicit `false` alone: our hooks
//!   stay dormant rather than re-arming everything else.
//! - **A timeout is only written when it exceeds ZCode's own 60-second default.**
//!   Status hooks take the default; `PermissionRequest` needs the long one, or
//!   ZCode abandons the approval after a minute and pops its own dialog.
//!
//! `zcodeAllowedEvents` is a whitelist: an event outside it makes ZCode drop the
//! **whole** hooks config on load, not just that entry.

use serde_json::{Map, Value, json};

use super::claude_hooks::contains_our_hook;
use super::json_editor::{delete_top_level_key, set_top_level_value};
use super::settings::{object, same_document};

/// Events ZCode accepts. Anything else voids the file.
pub const ALLOWED_EVENTS: &[(&str, i64)] = &[
    ("SessionStart", 5),
    ("UserPromptSubmit", 5),
    ("PreToolUse", 5),
    // The one event that overrides the default: it must outlast the user's
    // thinking, not the tool call.
    ("PermissionRequest", 86_400),
    ("PostToolUse", 5),
    ("PostToolUseFailure", 5),
    ("Stop", 5),
];

/// ZCode's per-hook default, in seconds. A timeout at or below this changes
/// nothing and is left out.
pub const DEFAULT_TIMEOUT_SECONDS: i64 = 60;

/// What an install came to.
pub struct Outcome {
    pub changed: bool,
    pub text: String,
}

/// The `hooks` object with our entries in it, or `None` when the file does not
/// parse — a file we cannot read is a file we do not write.
fn merge(source: &str, command: &str) -> Option<Value> {
    let base = if source.trim().is_empty() {
        "{}\n"
    } else {
        source
    };
    let root = object(base)?;
    let mut hooks = root
        .get("hooks")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut events = strip_ours(hooks.get("events").and_then(Value::as_object));

    for (event, timeout) in ALLOWED_EVENTS {
        let mut hook = Map::new();
        hook.insert("type".to_owned(), json!("command"));
        hook.insert("command".to_owned(), json!(command));
        if *timeout > DEFAULT_TIMEOUT_SECONDS {
            hook.insert("timeout".to_owned(), json!(timeout));
        }
        let entries = events
            .entry((*event).to_owned())
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(list) = entries.as_array_mut() {
            list.push(json!({ "hooks": [Value::Object(hook)] }));
        }
    }

    // Seed the switch only when there is none: an explicit `false` is the user's
    // answer and stays.
    hooks
        .entry("enabled".to_owned())
        .or_insert_with(|| json!(true));
    hooks.insert("events".to_owned(), Value::Object(events));
    Some(Value::Object(hooks))
}

/// The same object without any entry of ours, and without event keys left empty.
fn strip_ours(events: Option<&Map<String, Value>>) -> Map<String, Value> {
    let mut kept = Map::new();
    for (event, value) in events.into_iter().flatten() {
        let Some(entries) = value.as_array() else {
            continue;
        };
        let cleaned: Vec<Value> = entries
            .iter()
            .filter(|entry| !contains_our_hook(entry))
            .cloned()
            .collect();
        if !cleaned.is_empty() {
            kept.insert(event.clone(), Value::Array(cleaned));
        }
    }
    kept
}

/// Install or repair. `None` when the file does not parse.
pub fn install(existing: Option<&str>, command: &str) -> Option<Outcome> {
    let source = existing.unwrap_or_default();
    let merged = merge(source, command)?;
    let text = if source.trim().is_empty() {
        // A file that is not there yet has no bytes to preserve.
        let mut document = Map::new();
        document.insert("hooks".to_owned(), merged);
        format!(
            "{}\n",
            serde_json::to_string_pretty(&Value::Object(document)).ok()?
        )
    } else {
        set_top_level_value(source, "hooks", &merged)?
    };
    let changed = !same_document(source, &text);
    Some(Outcome { changed, text })
}

/// Take our entries back out. `None` when the file does not parse, or when
/// nothing of ours was in it.
pub fn uninstall(existing: &str) -> Option<Outcome> {
    let root = object(existing)?;
    let hooks = root.get("hooks").and_then(Value::as_object).cloned()?;
    let events = strip_ours(hooks.get("events").and_then(Value::as_object));
    let had_ours = hooks
        .get("events")
        .and_then(Value::as_object)
        .is_some_and(|current| current != &events);
    if !had_ours {
        return None;
    }

    // Nothing of ours — or the user's — is left under `hooks`: drop the key
    // rather than leave scaffolding that says an installer was here.
    // An `enabled: false` is an answer the user gave, so it is not scaffolding to
    // delete — removing it would switch their hooks back on behind their back.
    // macOS drops the key unconditionally; this is the one place this port is
    // deliberately stricter.
    let flipped_off = hooks.get("enabled") == Some(&Value::Bool(false));
    let fully_ours = events.is_empty()
        && !flipped_off
        && hooks.keys().all(|key| key == "enabled" || key == "events");
    let text = if fully_ours {
        delete_top_level_key(existing, "hooks")?
    } else {
        let mut updated = hooks;
        updated.insert("events".to_owned(), Value::Object(events));
        set_top_level_value(existing, "hooks", &Value::Object(updated))?
    };
    Some(Outcome {
        changed: text != existing,
        text,
    })
}

/// Whether every allowed event already carries one of our entries.
pub fn is_installed(existing: &str) -> bool {
    let Some(root) = object(existing) else {
        return false;
    };
    let events = root
        .get("hooks")
        .and_then(|hooks| hooks.get("events"))
        .and_then(Value::as_object);
    ALLOWED_EVENTS.iter().all(|(event, _)| {
        events
            .and_then(|events| events.get(*event))
            .and_then(Value::as_array)
            .is_some_and(|entries| entries.iter().any(contains_our_hook))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMAND: &str = "~/.codeisland/codeisland-hook.sh --source zcode";

    fn parsed(text: &str) -> Value {
        serde_json::from_str(super::super::json_editor::strip_json_comments(text).as_str())
            .expect("valid JSON")
    }

    #[test]
    fn the_events_go_under_hooks_events_beside_the_switch() {
        let outcome = install(None, COMMAND).expect("a plan");
        let root = parsed(&outcome.text);
        assert_eq!(root["hooks"]["enabled"], true);
        for (event, _) in ALLOWED_EVENTS {
            assert!(
                root["hooks"]["events"][event][0]["hooks"].is_array(),
                "{event} missing"
            );
        }
        // Not at the top level: a ZCode file with `hooks.PreToolUse` is a file
        // ZCode ignores.
        assert!(root["hooks"]["PreToolUse"].is_null());
        assert!(is_installed(&outcome.text));
    }

    #[test]
    fn a_timeout_is_written_only_where_it_beats_zcodes_default() {
        let root = parsed(&install(None, COMMAND).expect("a plan").text);
        assert!(
            root["hooks"]["events"]["PreToolUse"][0]["hooks"][0]
                .get("timeout")
                .is_none(),
            "five seconds is ZCode's own default and changes nothing"
        );
        assert_eq!(
            root["hooks"]["events"]["PermissionRequest"][0]["hooks"][0]["timeout"],
            86_400
        );
    }

    #[test]
    fn a_users_false_switch_is_never_flipped_back_on() {
        let source = "{\n  \"hooks\": {\n    \"enabled\": false\n  }\n}\n";
        let outcome = install(Some(source), COMMAND).expect("a plan");
        assert_eq!(parsed(&outcome.text)["hooks"]["enabled"], false);
    }

    #[test]
    fn everything_outside_the_hooks_key_survives_byte_for_byte() {
        let source = "{\n  // the user's own note\n  \"theme\": \"dark\",\n  \"hooks\": {}\n}\n";
        let outcome = install(Some(source), COMMAND).expect("a plan");
        assert!(
            outcome.text.contains("the user's own note"),
            "{}",
            outcome.text
        );
        assert!(outcome.text.contains("\"theme\": \"dark\""));
    }

    #[test]
    fn a_file_that_does_not_parse_is_never_touched() {
        assert!(install(Some("not json"), COMMAND).is_none());
        assert!(uninstall("not json").is_none());
        assert!(!is_installed("not json"));
    }

    #[test]
    fn a_second_install_changes_nothing() {
        let first = install(None, COMMAND).expect("a plan");
        let second = install(Some(&first.text), COMMAND).expect("a plan");
        assert!(!second.changed);
        assert_eq!(second.text, first.text);
    }

    #[test]
    fn uninstall_leaves_the_users_own_keys_and_drops_the_scaffolding() {
        let source = "{\n  \"theme\": \"dark\",\n  \"hooks\": {\n    \"enabled\": true\n  }\n}\n";
        let installed = install(Some(source), COMMAND).expect("a plan");
        let back = uninstall(&installed.text).expect("a plan");
        assert!(!back.text.contains("codeisland"), "{}", back.text);
        // Their own key is still there; the switch we seeded and the events we
        // wrote are not scaffolding worth keeping.
        let root = parsed(&back.text);
        assert_eq!(root["theme"], "dark");
        assert!(root.get("hooks").is_none(), "{}", back.text);
    }

    /// A switch the user turned **off** is an answer, not scaffolding: deleting it
    /// would re-arm every other hook they disabled.
    #[test]
    fn uninstall_keeps_an_explicit_off_switch() {
        let source = "{\n  \"hooks\": {\n    \"enabled\": false\n  }\n}\n";
        let installed = install(Some(source), COMMAND).expect("a plan");
        let back = uninstall(&installed.text).expect("a plan");
        assert_eq!(parsed(&back.text)["hooks"]["enabled"], false);
    }

    #[test]
    fn uninstall_of_a_file_with_nothing_of_ours_says_so() {
        let source = "{\n  \"hooks\": {\n    \"enabled\": true\n  }\n}\n";
        assert!(uninstall(source).is_none());
    }

    #[test]
    fn a_hooks_key_that_was_only_ever_ours_is_deleted_not_emptied() {
        let installed = install(None, COMMAND).expect("a plan");
        let back = uninstall(&installed.text).expect("a plan");
        let root = parsed(&back.text);
        assert!(root.get("hooks").is_none(), "{}", back.text);
    }
}
