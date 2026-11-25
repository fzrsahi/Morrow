//! The global shortcuts (#101, L-007, #462): chords that reach a shell handler
//! wherever the user is. X11 grabs them directly. Wayland cannot, which is why
//! the capability table points at the desktop's own binding instead; a failed
//! grab is reported the same way, never fatal.
//!
//! Bindings live in the same `~/.codeisland/settings.json` as the other
//! Settings values. This module owns the action table and the registration; the
//! `shortcut_list`/`shortcut_set` commands in `settings.rs` own the IPC.

use std::path::Path;
use std::sync::RwLock;

use morrow_platform::capability::Capability;
use morrow_platform::settings;
use serde::Serialize;
use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// The default binding: unchanged from the fixed chord this module installed
/// before #462. A fresh install behaves exactly as it did then.
pub const DEFAULT: &str = "Super+Space";

/// Top-level key in the settings document.
pub const SETTINGS_KEY: &str = "shortcuts";

/// Only actions with a shell handler are editable. A row nothing obeys would be
/// a story in Settings, not a setting.
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    pub id: &'static str,
    pub label: &'static str,
    /// `None` means unbound on a fresh install.
    pub default: Option<&'static str>,
}

pub const ACTIONS: [Action; 2] = [
    Action {
        id: "toggleIsland",
        label: "Show / hide the island",
        default: Some(DEFAULT),
    },
    Action {
        id: "openSettings",
        label: "Open Settings",
        default: None,
    },
];

/// One row of the Shortcuts page. `chord: None` renders as the cleared state.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Binding {
    pub action: &'static str,
    pub label: &'static str,
    pub default: Option<&'static str>,
    pub chord: Option<String>,
}

/// What actually happened at the last install, for the Capability table. A
/// rebind replaces this instead of keeping the first attempt's answer.
static OUTCOME: RwLock<Option<Capability>> = RwLock::new(None);

pub fn outcome() -> Option<Capability> {
    OUTCOME.read().ok().and_then(|guard| guard.clone())
}

fn record(capability: Capability) {
    if let Ok(mut guard) = OUTCOME.write() {
        *guard = Some(capability);
    }
}

pub fn action(id: &str) -> Option<&'static Action> {
    ACTIONS.iter().find(|action| action.id == id)
}

/// The effective chord. A stored string wins; `null` and `""` clear a default;
/// a missing key or a malformed value falls back to the default.
pub fn chord(stored: &Value, id: &str) -> Option<String> {
    let fallback = action(id).and_then(|action| action.default);
    match stored
        .get(SETTINGS_KEY)
        .and_then(Value::as_object)
        .and_then(|rows| rows.get(id))
    {
        Some(Value::String(chord)) if !chord.trim().is_empty() => Some(chord.clone()),
        // An explicit null (or empty string) is the user's "clear", even when
        // the action has a default. Only a missing or malformed key falls back.
        Some(Value::String(_)) | Some(Value::Null) => None,
        _ => fallback.map(str::to_owned),
    }
}

/// The rows the Settings page renders, from the same document the grabber uses.
pub fn list(stored: &Value) -> Vec<Binding> {
    ACTIONS
        .iter()
        .map(|action| Binding {
            action: action.id,
            label: action.label,
            default: action.default,
            chord: chord(stored, action.id),
        })
        .collect()
}

/// Validate a chord exactly as the grabber will parse it. An empty value clears
/// the binding rather than being an error.
pub fn validate_chord(value: Option<String>) -> Result<Option<String>, String> {
    let chord = value.map(|value| value.trim().to_owned()).filter(|value| !value.is_empty());
    if let Some(chord) = &chord {
        chord
            .parse::<Shortcut>()
            .map_err(|error| format!("{chord} is not a shortcut: {error}"))?;
    }
    Ok(chord)
}

/// The document after one binding changes. Pure, so the write and its
/// minimal-diff shape are testable without an `AppHandle`.
pub fn with_binding(stored: &Value, id: &str, chord: Option<String>) -> Value {
    let mut document = stored.as_object().cloned().unwrap_or_default();
    let mut shortcuts = document
        .get(SETTINGS_KEY)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    match chord {
        Some(chord) => {
            shortcuts.insert(id.to_owned(), Value::String(chord));
        }
        None => {
            shortcuts.insert(id.to_owned(), Value::Null);
        }
    }
    document.insert(SETTINGS_KEY.to_owned(), Value::Object(shortcuts));
    Value::Object(document)
}

/// Register (or re-register) the stored chords. Called once at startup and
/// again after every `shortcut_set`, so a change takes effect without a restart.
pub fn install(app: &AppHandle, dir: &Path) {
    let stored = settings::load(dir).unwrap_or(Value::Null);
    let _ = app.global_shortcut().unregister_all();
    let mut registered = 0;
    let mut failure: Option<String> = None;

    for binding in list(&stored) {
        let Some(chord) = binding.chord else { continue };
        let shortcut = match chord.parse::<Shortcut>() {
            Ok(shortcut) => shortcut,
            Err(error) => {
                failure.get_or_insert(format!("{chord} is not a shortcut: {error}"));
                continue;
            }
        };
        let result = match binding.action {
            "toggleIsland" => app.global_shortcut().on_shortcut(shortcut, |app, _, event| {
                if event.state() == ShortcutState::Pressed {
                    crate::tray::toggle_island(app);
                }
            }),
            "openSettings" => app.global_shortcut().on_shortcut(shortcut, |app, _, event| {
                if event.state() == ShortcutState::Pressed {
                    if let Err(error) = crate::settings_window::open_settings(app.clone()) {
                        eprintln!("morrow-app: cannot open Settings from shortcut ({error})");
                    }
                }
            }),
            _ => continue,
        };
        match result {
            Ok(()) => registered += 1,
            Err(error) => {
                failure.get_or_insert(format!(
                    "could not grab {chord}: {error}; bind `morrow toggle` in the desktop instead"
                ));
            }
        }
    }

    record(match failure {
        Some(reason) => Capability::Degraded(reason),
        None if registered == 0 => Capability::Degraded(
            "no global shortcut is bound; bind one in Settings".to_owned(),
        ),
        None => Capability::Supported,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_fresh_install_keeps_the_one_binding_there_was_before_462() {
        let rows = list(&Value::Null);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].action, "toggleIsland");
        assert_eq!(rows[0].chord.as_deref(), Some("Super+Space"));
        assert_eq!(rows[0].default, Some(DEFAULT));
        assert_eq!(rows[1].action, "openSettings");
        assert_eq!(rows[1].chord, None);
        assert_eq!(rows[1].default, None);
    }

    #[test]
    fn a_stored_chord_overrides_the_default_and_null_clears_it() {
        let stored = json!({ "shortcuts": { "toggleIsland": null, "openSettings": "Ctrl+K" } });
        let rows = list(&stored);
        assert_eq!(rows[0].chord, None);
        assert_eq!(rows[1].chord.as_deref(), Some("Ctrl+K"));
        // The default is still reported so Settings can explain a cleared row.
        assert_eq!(rows[0].default, Some(DEFAULT));
    }

    #[test]
    fn a_conflicting_chord_shows_up_on_both_rows_of_the_list() {
        let stored = json!({ "shortcuts": { "toggleIsland": "Ctrl+K", "openSettings": "Ctrl+K" } });
        let chords: Vec<_> = list(&stored).into_iter().map(|row| row.chord).collect();
        assert_eq!(chords, vec![Some("Ctrl+K".into()), Some("Ctrl+K".into())]);
    }

    #[test]
    fn validation_accepts_tauri_chords_and_rejects_garbage() {
        assert_eq!(validate_chord(Some("  Super+Space ".into())).unwrap().as_deref(), Some("Super+Space"));
        assert_eq!(validate_chord(Some("Ctrl+Shift+A".into())).unwrap().as_deref(), Some("Ctrl+Shift+A"));
        assert_eq!(validate_chord(Some("".into())).unwrap(), None);
        assert_eq!(validate_chord(None).unwrap(), None);
        assert!(validate_chord(Some("Ctrl+".into())).is_err());
        assert!(validate_chord(Some("NotAKey+AlsoWrong".into())).is_err());
    }

    #[test]
    fn one_change_leaves_every_other_setting_alone() {
        let stored = json!({ "theme": "modern", "shortcuts": { "toggleIsland": "Super+Space" } });
        let next = with_binding(&stored, "openSettings", Some("Ctrl+K".into()));
        assert_eq!(next["theme"], "modern");
        assert_eq!(next["shortcuts"]["toggleIsland"], "Super+Space");
        assert_eq!(next["shortcuts"]["openSettings"], "Ctrl+K");

        let cleared = with_binding(&next, "toggleIsland", None);
        assert_eq!(cleared["shortcuts"]["toggleIsland"], Value::Null);
        assert_eq!(cleared["shortcuts"]["openSettings"], "Ctrl+K");
        assert_eq!(cleared["theme"], "modern");
    }

    #[test]
    fn an_unknown_action_is_not_a_setting() {
        assert!(action("toggleIsland").is_some());
        assert!(action("nonsense").is_none());
    }
}
