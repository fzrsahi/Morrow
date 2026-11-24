//! MiniMax Code's plugin pack (#234 batch 3b): two files, not one key.
//!
//! `mcode` (`@minimax-ai/code`) has no `settings.json` `hooks` key at all. Shell
//! hooks are only loadable through a **plugin**, and a local plugin is a directory
//! under `~/.minimax/plugins/<name>/` that mcode discovers by scanning. It parses
//! the Claude-compatible layout:
//!
//! ```text
//! ~/.minimax/plugins/morrow/
//! ├── .claude-plugin/plugin.json   {name, displayName, version, description, hooks}
//! └── hooks/hooks.json             {hooks: {Event: [{hooks: [{type, command, timeout}]}]}}
//! ```
//!
//! Two rules that would otherwise be silent:
//!
//! - **Timeouts must be 1..=10 seconds.** mcode rejects the whole handler outside
//!   that range (`HOOK_SCHEMA_INVALID`), so Claude's day-long `PermissionRequest`
//!   ceiling would drop every approval without a word. `PermissionRequest` takes
//!   the 10-second maximum; the bridge fails open and mcode's own prompt takes
//!   over.
//! - **The manifest carries a marker**, so uninstall only ever removes a pack this
//!   installer wrote. A plugin someone else left at `plugins/morrow` is not ours.

use serde_json::{Map, Value, json};

/// Written into the manifest's `description`: proof of authorship, and the only
/// thing uninstall trusts.
pub const MANIFEST_MARKER: &str = "Morrow MiniMax Code plugin";

/// The plugin's directory name under `<root>/plugins`.
pub const PLUGIN_NAME: &str = "morrow";

/// mcode's ceiling for a hook timeout, in seconds. A larger number voids the
/// handler.
pub const MAX_TIMEOUT_SECONDS: i64 = 10;

/// mcode's event registry: eleven events, and **no `Notification`**, which is why
/// the sound for a waiting agent is driven by `PreToolUse` here rather than by the
/// notification hook the other targets install.
pub const EVENTS: &[(&str, i64)] = &[
    ("SessionStart", 5),
    ("UserPromptSubmit", 5),
    ("PreToolUse", 5),
    // Ten, not a day: mcode discards anything above its ceiling, and a discarded
    // `PermissionRequest` is an approval that never reaches the island.
    ("PermissionRequest", MAX_TIMEOUT_SECONDS),
    ("PostToolUse", 5),
    ("SubagentStart", 5),
    ("SubagentStop", 5),
    ("Stop", 5),
    ("PreCompact", 5),
    ("PostCompact", 5),
    ("SessionEnd", 5),
];

/// `.claude-plugin/plugin.json`.
pub fn manifest() -> Value {
    json!({
        "name": PLUGIN_NAME,
        "displayName": "Morrow",
        "version": "1.0.0",
        "description": MANIFEST_MARKER,
        "hooks": ["hooks/hooks.json"],
    })
}

/// `hooks/hooks.json`, for the bridge command given.
///
/// No `matcher`: mcode validates one only when it is present, so omitting it is
/// both accepted and less to be wrong about.
pub fn hooks_document(command: &str) -> Value {
    let mut events = Map::new();
    for (event, timeout) in EVENTS {
        events.insert(
            (*event).to_owned(),
            json!([{
                "hooks": [{
                    "type": "command",
                    "command": command,
                    "timeout": timeout,
                }],
            }]),
        );
    }
    json!({ "hooks": Value::Object(events) })
}

/// The two files, as text, with a trailing newline: `String(contentsOf:)` and
/// `serde_json` both cope without one, and a diff is easier to read with it.
pub fn files(command: &str) -> Vec<(&'static str, String)> {
    vec![
        (
            ".claude-plugin/plugin.json",
            format!(
                "{}\n",
                serde_json::to_string_pretty(&manifest()).unwrap_or_default()
            ),
        ),
        (
            "hooks/hooks.json",
            format!(
                "{}\n",
                serde_json::to_string_pretty(&hooks_document(command)).unwrap_or_default()
            ),
        ),
    ]
}

/// Whether this manifest text is a pack we wrote.
pub fn manifest_is_ours(existing: &str) -> bool {
    serde_json::from_str::<Value>(existing)
        .ok()
        .and_then(|manifest| {
            manifest
                .get("description")
                .and_then(|description| description.as_str())
                .map(|description| {
                    // ConfigInstaller.swift accepts both spellings: a pack
                    // written before the rename is still ours to repair and
                    // remove, not a foreign plugin to refuse.
                    description == MANIFEST_MARKER
                        || description == "CodeIsland MiniMax Code plugin"
                })
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMAND: &str = "~/.codeisland/codeisland-hook.sh --source minimax";

    #[test]
    fn the_manifest_names_the_pack_and_points_at_its_hooks() {
        let manifest = manifest();
        assert_eq!(manifest["name"], "morrow");
        assert_eq!(manifest["description"], MANIFEST_MARKER);
        assert_eq!(manifest["hooks"][0], "hooks/hooks.json");
    }

    #[test]
    fn every_timeout_is_within_mcodes_ceiling() {
        // One second too many voids the handler without a word, so this is the
        // assertion the whole module exists for.
        for (event, timeout) in EVENTS {
            assert!(
                (1..=MAX_TIMEOUT_SECONDS).contains(timeout),
                "{event} asks for {timeout}s, which mcode rejects"
            );
        }
        assert_eq!(
            hooks_document(COMMAND)["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"],
            MAX_TIMEOUT_SECONDS
        );
    }

    #[test]
    fn the_events_are_mcodes_own_and_carry_no_matcher() {
        let document = hooks_document(COMMAND);
        let events = document["hooks"].as_object().expect("events");
        assert_eq!(events.len(), EVENTS.len());
        assert!(
            !events.contains_key("Notification"),
            "mcode has no such hook"
        );
        assert!(events.contains_key("PostCompact"));
        assert!(events["PreToolUse"][0].get("matcher").is_none());
    }

    #[test]
    fn a_foreign_manifest_is_not_ours_to_delete() {
        assert!(manifest_is_ours(&manifest().to_string()));
        assert!(manifest_is_ours(
            r#"{"name":"morrow","description":"CodeIsland MiniMax Code plugin"}"#
        ));
        assert!(!manifest_is_ours(
            r#"{"name":"morrow","description":"someone else"}"#
        ));
        assert!(!manifest_is_ours("not json"));
    }

    #[test]
    fn both_files_are_written_at_the_paths_mcode_scans() {
        let files = files(COMMAND);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].0, ".claude-plugin/plugin.json");
        assert_eq!(files[1].0, "hooks/hooks.json");
        assert!(files[1].1.contains("codeisland"));
    }
}
