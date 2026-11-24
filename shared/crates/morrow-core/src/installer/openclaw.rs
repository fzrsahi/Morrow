//! OpenClaw's registration (#234 batch 4b): the file alone is not an install.
//!
//! `~/.openclaw/codeisland-plugin/index.ts` is only half of it. The gateway loads
//! plugins a config names, so the plugin directory has to be registered in
//! `~/.openclaw/openclaw.json` before anything happens:
//!
//! ```json
//! {"plugins": {"load": {"paths": ["…/codeisland-plugin"]},
//!              "entries": {"codeisland": {"enabled": true}}}}
//! ```
//!
//! `openclaw.json` is **JSON5**. Comments are handled — the editor strips them, as it
//! does for every other file here — but anything else JSON5 allows (unquoted keys,
//! single-quoted strings, trailing commas) does not parse. When that happens the
//! plugin files still land and this refuses to touch the config: clobbering a file we
//! cannot read would throw away the user's own gateway settings, and a plugin that is
//! on disk but unregistered fails visibly instead of silently.

use serde_json::{Map, Value, json};

use super::settings::{object, same_document};

/// The entry name the gateway looks for.
pub const ENTRY: &str = "codeisland";

/// What an install came to.
pub struct Outcome {
    pub changed: bool,
    pub text: String,
}

/// The config with our plugin registered, or `None` when it does not parse.
pub fn register(existing: Option<&str>, plugin_dir: &str) -> Option<Outcome> {
    let source = existing.unwrap_or_default();
    let mut root = if source.trim().is_empty() {
        Map::new()
    } else {
        object(source)?
    };
    let mut plugins = root
        .get("plugins")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut load = plugins
        .get("load")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut paths: Vec<Value> = load
        .get("paths")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !paths.iter().any(|path| path.as_str() == Some(plugin_dir)) {
        paths.push(json!(plugin_dir));
    }
    load.insert("paths".to_owned(), Value::Array(paths));
    plugins.insert("load".to_owned(), Value::Object(load));

    let mut entries = plugins
        .get("entries")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut entry = entries
        .get(ENTRY)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    // Only ours, and only ever switching it on: `enabled` inside our own entry is
    // not the user's switch over anything else.
    entry.insert("enabled".to_owned(), json!(true));
    entries.insert(ENTRY.to_owned(), Value::Object(entry));
    plugins.insert("entries".to_owned(), Value::Object(entries));

    // Keep the user's comments and formatting everywhere outside `plugins`:
    // rewriting the whole document with to_string_pretty silently ate them.
    let text = if source.trim().is_empty() {
        root.insert("plugins".to_owned(), Value::Object(plugins));
        format!(
            "{}\n",
            serde_json::to_string_pretty(&Value::Object(root)).ok()?
        )
    } else {
        super::json_editor::set_top_level_value(source, "plugins", &Value::Object(plugins))?
    };
    let changed = !same_document(source, &text);
    Some(Outcome { changed, text })
}

/// The config with our path and entry taken back out, or `None` when nothing of
/// ours was in it.
pub fn unregister(existing: &str, plugin_dir: &str) -> Option<Outcome> {
    let root = object(existing)?;
    let mut plugins = root.get("plugins").and_then(Value::as_object)?.clone();
    let mut changed = false;

    if let Some(load) = plugins.get("load").and_then(Value::as_object).cloned() {
        let mut load = load;
        if let Some(paths) = load.get("paths").and_then(Value::as_array).cloned() {
            let kept: Vec<Value> = paths
                .into_iter()
                .filter(|path| {
                    // Exact path, not a substring: a user's own plugin whose
                    // name merely contains "codeisland-plugin" is not ours.
                    let ours = path.as_str() == Some(plugin_dir);
                    if ours {
                        changed = true;
                    }
                    !ours
                })
                .collect();
            if kept.is_empty() {
                load.remove("paths");
            } else {
                load.insert("paths".to_owned(), Value::Array(kept));
            }
            if load.is_empty() {
                plugins.remove("load");
            } else {
                plugins.insert("load".to_owned(), Value::Object(load));
            }
        }
    }

    if let Some(entries) = plugins.get("entries").and_then(Value::as_object).cloned() {
        if entries.contains_key(ENTRY) {
            let mut entries = entries;
            entries.remove(ENTRY);
            changed = true;
            if entries.is_empty() {
                plugins.remove("entries");
            } else {
                plugins.insert("entries".to_owned(), Value::Object(entries));
            }
        }
    }

    if !changed {
        return None;
    }
    let text = if plugins.is_empty() {
        super::json_editor::delete_top_level_key(existing, "plugins")?
    } else {
        super::json_editor::set_top_level_value(existing, "plugins", &Value::Object(plugins))?
    };
    Some(Outcome {
        changed: true,
        text,
    })
}

/// Whether this config already names our plugin and switches it on.
pub fn is_registered(existing: &str, plugin_dir: &str) -> bool {
    let Some(root) = object(existing) else {
        return false;
    };
    let plugins = root.get("plugins").and_then(Value::as_object);
    let path_listed = plugins
        .and_then(|plugins| plugins.get("load"))
        .and_then(|load| load.get("paths"))
        .and_then(Value::as_array)
        .is_some_and(|paths| paths.iter().any(|path| path.as_str() == Some(plugin_dir)));
    let enabled = plugins
        .and_then(|plugins| plugins.get("entries"))
        .and_then(|entries| entries.get(ENTRY))
        .and_then(|entry| entry.get("enabled"))
        .and_then(Value::as_bool)
        == Some(true);
    path_listed && enabled
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIR: &str = "/home/dev/.openclaw/codeisland-plugin";

    fn parsed(text: &str) -> Value {
        serde_json::from_str(text).expect("valid JSON")
    }

    #[test]
    fn a_fresh_config_gets_a_path_and_an_enabled_entry() {
        let outcome = register(None, DIR).expect("a plan");
        let root = parsed(&outcome.text);
        assert_eq!(root["plugins"]["load"]["paths"][0], DIR);
        assert_eq!(root["plugins"]["entries"]["codeisland"]["enabled"], true);
        assert!(is_registered(&outcome.text, DIR));
    }

    #[test]
    fn the_users_own_plugins_survive() {
        let source = r#"{
            "gateway": {"port": 18789},
            "plugins": {"load": {"paths": ["/some/other"]}, "entries": {"other": {"enabled": true}}}
        }"#;
        let outcome = register(Some(source), DIR).expect("a plan");
        let root = parsed(&outcome.text);
        // Their path is still first, ours appended.
        assert_eq!(root["plugins"]["load"]["paths"][0], "/some/other");
        assert_eq!(root["plugins"]["load"]["paths"][1], DIR);
        assert_eq!(root["plugins"]["entries"]["other"]["enabled"], true);
        assert_eq!(root["gateway"]["port"], 18789);
    }

    #[test]
    fn registering_twice_does_not_list_the_path_twice() {
        let first = register(None, DIR).expect("a plan");
        let second = register(Some(&first.text), DIR).expect("a plan");
        assert!(!second.changed);
        assert_eq!(
            parsed(&second.text)["plugins"]["load"]["paths"]
                .as_array()
                .expect("paths")
                .len(),
            1
        );
    }

    #[test]
    fn unregister_takes_out_only_ours() {
        let source = r#"{"plugins": {"load": {"paths": ["/some/other"]}, "entries": {"other": {"enabled": true}}}}"#;
        let registered = register(Some(source), DIR).expect("a plan");
        let back = unregister(&registered.text, DIR).expect("a plan");
        let root = parsed(&back.text);
        assert_eq!(root["plugins"]["load"]["paths"][0], "/some/other");
        assert!(root["plugins"]["entries"]["codeisland"].is_null());
        assert_eq!(root["plugins"]["entries"]["other"]["enabled"], true);
        assert!(!is_registered(&back.text, DIR));
    }

    #[test]
    fn a_plugins_object_that_was_only_ours_is_deleted() {
        let registered = register(None, DIR).expect("a plan");
        let back = unregister(&registered.text, DIR).expect("a plan");
        assert!(parsed(&back.text).get("plugins").is_none());
    }

    #[test]
    fn a_file_we_cannot_parse_is_never_touched() {
        // Comments survive — the editor strips them. Unquoted keys do not, and a
        // gateway config we clobbered is the user's settings gone.
        assert!(register(Some("{ // my gateway\n}"), DIR).is_some());
        let json5 = "{ gateway: { port: 18789 },\n}\n";
        assert!(register(Some(json5), DIR).is_none());
        assert!(unregister(json5, DIR).is_none());
        assert!(!is_registered(json5, DIR));
    }

    #[test]
    fn a_foreign_path_that_merely_contains_the_plugin_name_is_not_ours() {
        let source = r#"{"plugins":{"load":{"paths":["/home/dev/codeisland-plugin-backup"]},"entries":{"codeisland":{"enabled":true}}}}"#;
        assert!(!is_registered(source, DIR), "the path must match exactly");
        let back = unregister(source, DIR).expect("our entry is still removed");
        let root = parsed(&back.text);
        assert_eq!(
            root["plugins"]["load"]["paths"][0],
            "/home/dev/codeisland-plugin-backup"
        );
    }

    #[test]
    fn registration_keeps_the_users_comments_outside_plugins() {
        let source = "{\n  // the user's own note\n  \"gateway\": {\"port\": 18789},\n  \"plugins\": {}\n}\n";
        let outcome = register(Some(source), DIR).expect("a plan");
        assert!(
            outcome.text.contains("the user's own note"),
            "{}",
            outcome.text
        );
    }

    #[test]
    fn unregister_of_a_config_with_nothing_of_ours_says_so() {
        let source = r#"{"plugins": {"entries": {"other": {"enabled": true}}}}"#;
        assert!(unregister(source, DIR).is_none());
    }
}
