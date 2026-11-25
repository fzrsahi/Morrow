//! #465 — the shell half of the per-display island settings.
//!
//! Mirrors Swift's `GeneralPage` / `DisplayConfigRow`: one row per connected
//! screen with an enable switch, a presentation mode and a top offset. The
//! planner and the codec are pure in `morrow-core`; this module reads the
//! settings document, enumerates screens through the platform adapter and
//! writes the same `displayConfigurations` key Swift persists.

use std::collections::BTreeMap;

use morrow_core::display::{
    codec, dynamic_island_top_offset, planner, DisplayConfiguration, DisplayDescriptor,
    IslandDisplayMode,
};
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State};

use crate::HooksSite;

pub const CONFIGS_KEY: &str = "displayConfigurations";
pub const ENABLED_KEY: &str = "islandEnabled";
pub const CHOICE_KEY: &str = "displayChoice";

/// One read of the Settings page: connected screens and their resolved rows.
#[derive(Debug, Serialize)]
pub struct DisplaySnapshot {
    pub displays: Vec<DisplayDescriptor>,
    pub configs: BTreeMap<String, DisplayConfiguration>,
    pub island_enabled: bool,
    pub display_choice: String,
    pub primary_enabled_display_id: Option<String>,
    /// Why the list is empty: no enumerator on this host, not "no screens".
    pub reader_reason: Option<String>,
}

fn resolved(site: &HooksSite) -> DisplaySnapshot {
    let stored = morrow_platform::settings::load(&site.paths.codeisland_dir).unwrap_or_default();
    let displays = morrow_platform::display::connected_displays(&morrow_platform::runner::SystemRunner);
    let reader_reason = if displays.is_empty() {
        Some("No display reader is available on this host.".to_owned())
    } else {
        None
    };
    let legacy_mode = IslandDisplayMode::resolved(
        stored.get("islandDisplayMode").and_then(Value::as_str),
    );
    let legacy_offset = stored
        .get("dynamicIslandTopOffset")
        .and_then(Value::as_f64)
        .unwrap_or(dynamic_island_top_offset::DEFAULT);
    let choice = stored
        .get(CHOICE_KEY)
        .and_then(Value::as_str)
        .unwrap_or("auto")
        .to_owned();
    let (configs, _) = planner::resolved_configurations(
        codec::decode(stored.get(CONFIGS_KEY).and_then(Value::as_str)).as_ref(),
        &displays,
        &choice,
        legacy_mode,
        legacy_offset,
    );
    let primary_enabled_display_id =
        planner::primary_enabled_display_id(&displays, &configs);
    DisplaySnapshot {
        displays,
        configs,
        island_enabled: stored
            .get(ENABLED_KEY)
            .and_then(Value::as_bool)
            .unwrap_or(true),
        display_choice: choice,
        primary_enabled_display_id,
        reader_reason,
    }
}

/// Write the resolved rows back and tell both windows about the change.
fn save_configs(
    app: &AppHandle,
    site: &HooksSite,
    config_rows: BTreeMap<String, DisplayConfiguration>,
) -> Result<bool, String> {
    let changed = morrow_platform::settings::set(
        &site.paths.codeisland_dir,
        CONFIGS_KEY,
        &Value::String(codec::encode(&config_rows)),
    )
    .map_err(|error| error.to_string())?;
    if let Ok(stored) = morrow_platform::settings::load(&site.paths.codeisland_dir) {
        let _ = app.emit(crate::settings::SETTINGS_CHANGED_EVENT, stored);
    }
    Ok(changed)
}

#[tauri::command]
pub fn display_snapshot(site: State<'_, HooksSite>) -> DisplaySnapshot {
    resolved(&site)
}

/// Swift's `DisplayConfigRow`: one screen's switch, mode and top offset.
#[tauri::command]
pub fn display_set(
    app: AppHandle,
    site: State<'_, HooksSite>,
    id: String,
    enabled: bool,
    display_mode: String,
    top_offset: f64,
) -> Result<bool, String> {
    let mut snapshot = resolved(&site);
    let config = DisplayConfiguration::new(
        enabled,
        IslandDisplayMode::resolved(Some(&display_mode)),
        top_offset,
    );
    snapshot.configs.insert(id, config);
    save_configs(&app, &site, snapshot.configs)
}

/// Swift's `islandEnabled` switch. Turning it on keeps the "never zero islands"
/// rule: if every row is off, enable the legacy target.
#[tauri::command]
pub fn display_island_enabled(
    app: AppHandle,
    site: State<'_, HooksSite>,
    enabled: bool,
) -> Result<bool, String> {
    let mut snapshot = resolved(&site);
    if enabled
        && !snapshot
            .displays
            .iter()
            .any(|display| snapshot.configs.get(&display.id).is_some_and(|c| c.enabled))
    {
        let target = planner::legacy_target_index(&snapshot.display_choice, &snapshot.displays)
            .unwrap_or(0);
        if let Some(display) = snapshot.displays.get(target) {
            let mut config = display.default_configuration(true);
            config.display_mode =
                IslandDisplayMode::resolved(snapshot.configs.get(&display.id).map(|c| c.display_mode.raw_value()));
            snapshot.configs.insert(display.id.clone(), config);
        }
    }
    let changed = morrow_platform::settings::set(
        &site.paths.codeisland_dir,
        ENABLED_KEY,
        &json!(enabled),
    )
    .map_err(|error| error.to_string())?;
    save_configs(&app, &site, snapshot.configs)?;
    Ok(changed)
}

/// Top-centre of a monitor for an island of `width` physical pixels.
pub fn target_position(origin: (i32, i32), size: (u32, u32), width: u32) -> (i32, i32) {
    (origin.0 + (size.0.saturating_sub(width)) as i32 / 2, origin.1)
}

/// Move the single shell island to the chosen display. There is one window in
/// the Tauri shell, so selecting a display moves it rather than cloning it.
#[tauri::command]
pub fn display_activate(
    app: AppHandle,
    site: State<'_, HooksSite>,
    id: String,
) -> Result<bool, String> {
    let mut snapshot = resolved(&site);
    let Some(display) = snapshot.displays.iter().find(|row| row.id == id) else {
        return Err(format!("{id} is not a connected display"));
    };
    for (key, config) in snapshot.configs.iter_mut() {
        config.enabled = key == &id;
    }
    let available = app.available_monitors().map_err(|error| error.to_string())?;
    let monitor = available
        .iter()
        .find(|monitor| monitor.name() == Some(&display.name))
        .or_else(|| available.get(display.index))
        .cloned();
    if let (Some(window), Some(monitor)) = (app.get_webview_window(crate::island::LABEL), monitor) {
        let size = window.inner_size().map_err(|error| error.to_string())?;
        let (x, y) = target_position(
            (monitor.position().x, monitor.position().y),
            (monitor.size().width, monitor.size().height),
            size.width,
        );
        window
            .set_position(PhysicalPosition::new(x, y))
            .map_err(|error| error.to_string())?;
    }
    save_configs(&app, &site, snapshot.configs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_island_is_centred_on_the_chosen_display() {
        assert_eq!(target_position((1920, 0), (1600, 900), 240), (2600, 0));
    }

    #[test]
    fn a_display_row_accepts_the_swift_mode_spellings() {
        assert_eq!(
            IslandDisplayMode::resolved(Some("dynamicIsland")),
            IslandDisplayMode::DynamicIsland
        );
        assert_eq!(
            IslandDisplayMode::resolved(Some("nonsense")),
            IslandDisplayMode::Notch
        );
    }
}
