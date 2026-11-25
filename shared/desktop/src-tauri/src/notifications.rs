//! #465 — the notification-preview buttons and the real power banner.
//!
//! Swift's Settings page previews every Dynamic Island banner. The shell has
//! one island surface, so each button asks the shell to emit a preview event
//! that the island draws for three seconds. The charging previews use the
//! same pure `morrow_core::power_banner` rules as the live banner.

use morrow_core::power_banner::{power_banner_state, PowerBannerState, PowerSourceKind};
use serde_json::json;
use tauri::{AppHandle, Emitter};

pub const PREVIEW_EVENT: &str = "notification-preview";

/// The 19 buttons in `SettingsView.swift:2123-2341` (the issue counted 14 from
/// an earlier revision). Titles are copied verbatim.
pub const PREVIEWS: &[(&str, &str)] = &[
    ("magsafe", "Test MagSafe Charging"),
    ("prayer-maghrib", "Test Prayer (Maghrib)"),
    ("prayer-subuh", "Test Subuh"),
    ("prayer-dzuhur", "Test Dzuhur"),
    ("prayer-ashar", "Test Ashar"),
    ("prayer-isya", "Test Isya"),
    ("usbc", "Test USB-C Charging"),
    ("on-hold", "Test Charging On Hold"),
    ("fully-charged", "Test Fully Charged"),
    ("low-battery", "Test Low Battery"),
    ("volume-hud", "Test Volume HUD"),
    ("track-expand", "Test Track Auto-Expand"),
    ("mail-arrival", "Test Mail Arrival"),
    ("prayer-wing-idle", "Test Prayer Wing (Idle)"),
    ("prayer-wing-thinking", "Test Prayer Wing (Thinking)"),
    ("inline-lyrics", "Test Inline Lyrics"),
    ("approval-modal", "Test Approval Modal"),
    ("question-modal", "Test Question Modal"),
    ("quick-prompt", "Test Quick Prompt"),
];

/// Resolve a preview's rendered title, or `None` for an unknown id.
pub fn preview_title(id: &str) -> Option<&'static str> {
    PREVIEWS
        .iter()
        .find(|(key, _)| *key == id)
        .map(|(_, title)| *title)
}

#[tauri::command]
pub fn notification_preview(app: AppHandle, id: String) -> Result<bool, String> {
    let title = preview_title(&id)
        .ok_or_else(|| format!("{id} is not a notification preview"))?;
    app.emit(PREVIEW_EVENT, json!({ "id": id, "title": title }))
        .map_err(|error| error.to_string())?;
    Ok(true)
}

/// The charging state a desktop battery reading means. There is no MagSafe
/// on Windows or Linux, so the adapter is always reported as USB-C.
pub fn banner_from_battery(battery: morrow_platform::system_stats::Battery) -> PowerBannerState {
    power_banner_state(
        PowerSourceKind::UsbC,
        battery.charging,
        battery.ac_connected,
        battery.level >= 100,
        i64::from(battery.level),
        None,
        None,
    )
}

/// One live read of the battery, rendered with the core's rules and posted to
/// the same island banner the previews use.
#[tauri::command]
pub fn power_banner_preview(app: AppHandle) -> Result<String, String> {
    let mut reader = morrow_platform::system_stats::SystemReader::system();
    let battery = reader.read().battery;
    let banner = match battery {
        Some(battery) => banner_from_battery(battery),
        None => PowerBannerState::LowBattery { percent: 0 },
    };
    let title = format!("{} · {}", banner.title(), banner.subtitle());
    app.emit(PREVIEW_EVENT, json!({ "id": "power", "title": title.clone() }))
        .map_err(|error| error.to_string())?;
    Ok(title)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_preview_grid_covers_the_swift_buttons() {
        assert!(PREVIEWS.len() >= 14);
        assert_eq!(preview_title("magsafe"), Some("Test MagSafe Charging"));
        assert_eq!(preview_title("quick-prompt"), Some("Test Quick Prompt"));
        assert_eq!(preview_title("nobody"), None);
    }

    #[test]
    fn preview_ids_are_unique() {
        let mut ids: Vec<_> = PREVIEWS.iter().map(|(id, _)| *id).collect();
        ids.sort_unstable();
        assert_eq!(ids.len(), ids.iter().collect::<std::collections::BTreeSet<_>>().len());
    }

    #[test]
    fn a_charging_battery_uses_the_core_rules() {
        let battery = morrow_platform::system_stats::Battery {
            level: 54,
            charging: true,
            ac_connected: true,
        };
        assert_eq!(
            banner_from_battery(battery),
            PowerBannerState::Charging {
                kind: PowerSourceKind::UsbC,
                watts: None,
                time_to_full_minutes: None,
            }
        );
    }
}
