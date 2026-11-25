//! Per-display island settings, ported from
//! `macos/Sources/MorrowCore/MultiDisplayConfiguration.swift`.
//!
//! The plan is pure: given the displays the OS reports and the stored document,
//! decide which island each screen gets. The shell owns the enumeration and the
//! window; this module owns the keys, the migration and the clamps so a
//! reconnect cannot strand the user with no visible island.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Swift `IslandDisplayMode.rawValue`, the two values the shared strings name.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum IslandDisplayMode {
    #[serde(rename = "notch")]
    Notch,
    #[serde(rename = "dynamicIsland")]
    DynamicIsland,
}

impl IslandDisplayMode {
    pub fn raw_value(self) -> &'static str {
        match self {
            Self::Notch => "notch",
            Self::DynamicIsland => "dynamicIsland",
        }
    }

    /// Swift `IslandDisplayMode.resolved(from:)`.
    pub fn resolved(raw: Option<&str>) -> Self {
        match raw {
            Some("dynamicIsland") => Self::DynamicIsland,
            _ => Self::Notch,
        }
    }
}

/// Swift `DynamicIslandTopOffset`.
pub mod dynamic_island_top_offset {
    pub const MIN: f64 = 0.0;
    pub const MAX: f64 = 40.0;
    pub const STEP: f64 = 1.0;
    pub const DEFAULT: f64 = 10.0;

    /// A non-finite offset is the default, not a stranded window off-screen.
    pub fn clamp(offset: f64) -> f64 {
        if !offset.is_finite() {
            return DEFAULT;
        }
        offset.clamp(MIN, MAX)
    }
}

/// Swift `DisplayConfiguration`: one screen's island presentation.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DisplayConfiguration {
    pub enabled: bool,
    pub display_mode: IslandDisplayMode,
    pub dynamic_island_top_offset: f64,
}

impl DisplayConfiguration {
    pub fn new(enabled: bool, display_mode: IslandDisplayMode, top_offset: f64) -> Self {
        Self {
            enabled,
            display_mode,
            dynamic_island_top_offset: dynamic_island_top_offset::clamp(top_offset),
        }
    }

    /// Swift's memberwise decoder: absent fields keep their defaults.
    pub fn from_value(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        Some(Self::new(
            object
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            IslandDisplayMode::resolved(
                object.get("displayMode").and_then(Value::as_str),
            ),
            object
                .get("dynamicIslandTopOffset")
                .and_then(Value::as_f64)
                .unwrap_or(dynamic_island_top_offset::DEFAULT),
        ))
    }
}

/// Swift `DisplayDescriptor`: platform-agnostic, `hasNotch` is always false off
/// Apple hardware but stays in the shape so the two shells share the JSON.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayDescriptor {
    pub id: String,
    pub index: usize,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub has_notch: bool,
    pub is_builtin: bool,
    pub is_primary: bool,
}

impl DisplayDescriptor {
    pub fn resolution_label(&self) -> String {
        format!("{}×{}", self.width, self.height)
    }

    pub fn default_mode(&self) -> IslandDisplayMode {
        if self.has_notch {
            IslandDisplayMode::Notch
        } else {
            IslandDisplayMode::DynamicIsland
        }
    }

    pub fn default_configuration(&self, enabled: bool) -> DisplayConfiguration {
        DisplayConfiguration::new(
            enabled,
            self.default_mode(),
            dynamic_island_top_offset::DEFAULT,
        )
    }
}

/// Swift `DisplayConfigurationCodec`: the JSON string behind
/// `SettingsKey.displayConfigurations`.
pub mod codec {
    use super::*;

    pub fn decode(raw: Option<&str>) -> Option<BTreeMap<String, DisplayConfiguration>> {
        let trimmed = raw?.trim();
        if trimmed.is_empty() {
            return None;
        }
        let value: Value = serde_json::from_str(trimmed).ok()?;
        let object = value.as_object()?;
        // A bad entry is dropped, not fatal to the other displays (Swift's
        // `LossyDisplayConfigurationEntry`).
        let decoded: BTreeMap<_, _> = object
            .iter()
            .filter_map(|(id, entry)| DisplayConfiguration::from_value(entry).map(|c| (id.clone(), c)))
            .collect();
        if decoded.is_empty() {
            None
        } else {
            Some(decoded)
        }
    }

    pub fn encode(configurations: &BTreeMap<String, DisplayConfiguration>) -> String {
        serde_json::to_string(configurations).unwrap_or_else(|_| "{}".to_owned())
    }
}

/// Swift `DisplayConfigurationPlanner`.
pub mod planner {
    use super::*;

    /// Swift `legacyTargetIndex`: a `screen_<n>` choice, then `main`, then the
    /// notched screen, then the built-in screen, then the first one.
    pub fn legacy_target_index(choice: &str, displays: &[DisplayDescriptor]) -> Option<usize> {
        if displays.is_empty() {
            return None;
        }
        if let Some(raw) = choice.strip_prefix("screen_") {
            if let Ok(index) = raw.parse::<usize>() {
                if index < displays.len() {
                    return Some(index);
                }
            }
        }
        if choice == "main" {
            return Some(0);
        }
        displays
            .iter()
            .position(|display| display.has_notch)
            .or_else(|| displays.iter().position(|display| display.is_builtin))
            .or(Some(0))
    }

    /// Swift `migrateLegacy`: one enabled island on the legacy target, every
    /// other connected screen off.
    pub fn migrate_legacy(
        displays: &[DisplayDescriptor],
        choice: &str,
        legacy_mode: IslandDisplayMode,
        legacy_top_offset: f64,
    ) -> BTreeMap<String, DisplayConfiguration> {
        let target = legacy_target_index(choice, displays).unwrap_or(0);
        displays
            .iter()
            .enumerate()
            .map(|(index, display)| {
                let config = if index == target {
                    DisplayConfiguration::new(true, legacy_mode, legacy_top_offset)
                } else {
                    display.default_configuration(false)
                };
                (display.id.clone(), config)
            })
            .collect()
    }

    /// Swift `resolvedConfigurations`: migrate when there is no stored map,
    /// preserve unknown stored keys on a clamshell/external-monitor change, and
    /// initialize newly seen displays as disabled.
    pub fn resolved_configurations(
        stored: Option<&BTreeMap<String, DisplayConfiguration>>,
        displays: &[DisplayDescriptor],
        choice: &str,
        legacy_mode: IslandDisplayMode,
        legacy_top_offset: f64,
    ) -> (BTreeMap<String, DisplayConfiguration>, bool) {
        let Some(stored) = stored else {
            if displays.is_empty() {
                return (BTreeMap::new(), false);
            }
            return (
                migrate_legacy(displays, choice, legacy_mode, legacy_top_offset),
                true,
            );
        };
        if displays.is_empty() {
            return (stored.clone(), false);
        }
        if !displays.iter().any(|display| stored.contains_key(&display.id)) {
            let mut merged = stored.clone();
            for (id, config) in
                migrate_legacy(displays, choice, legacy_mode, legacy_top_offset)
            {
                merged.insert(id, config);
            }
            return (merged, true);
        }
        let mut merged = stored.clone();
        let mut changed = false;
        for display in displays {
            if !merged.contains_key(&display.id) {
                merged.insert(display.id.clone(), display.default_configuration(false));
                changed = true;
            }
        }
        (merged, changed)
    }

    /// Swift `primaryEnabledDisplayID`: the first connected enabled screen.
    pub fn primary_enabled_display_id(
        displays: &[DisplayDescriptor],
        configs: &BTreeMap<String, DisplayConfiguration>,
    ) -> Option<String> {
        displays
            .iter()
            .find(|display| configs.get(&display.id).is_some_and(|c| c.enabled))
            .map(|display| display.id.clone())
    }
}

/// Parse `xrandr --query` into connected screens, in enumeration order.
pub fn parse_xrandr(output: &str) -> Vec<DisplayDescriptor> {
    let mut displays = Vec::new();
    for line in output.lines() {
        if !line.contains(" connected") {
            continue;
        }
        let mut fields = line.split_whitespace();
        let Some(name) = fields.next() else { continue };
        let is_primary = line.split_whitespace().any(|field| field == "primary");
        let geometry = fields
            .find(|field| {
                let bytes = field.as_bytes();
                field.contains('x')
                    && field.contains('+')
                    && bytes.first().is_some_and(u8::is_ascii_digit)
            })
            .and_then(|field| {
                let size = field.split('+').next().unwrap_or_default();
                let mut parts = size.split('x');
                let width = parts.next()?.parse::<u32>().ok()?;
                let height = parts.next()?.parse::<u32>().ok()?;
                Some((width, height))
            });
        let Some((width, height)) = geometry else { continue };
        let is_builtin = name.starts_with("eDP") || name.starts_with("LVDS");
        displays.push(DisplayDescriptor {
            id: name.to_owned(),
            index: displays.len(),
            name: name.to_owned(),
            width,
            height,
            has_notch: false,
            is_builtin,
            is_primary,
        });
    }
    displays
}

/// Parse the PowerShell line shape `name|WxH|True|builtin`, one screen per line.
pub fn parse_powershell_screens(output: &str) -> Vec<DisplayDescriptor> {
    let mut displays = Vec::new();
    for line in output.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let fields: Vec<_> = line.split('|').collect();
        if fields.len() < 3 {
            continue;
        }
        let mut size = fields[1].split('x');
        let (Some(width), Some(height)) = (size.next().and_then(|v| v.parse::<u32>().ok()), size.next().and_then(|v| v.parse::<u32>().ok())) else {
            continue;
        };
        displays.push(DisplayDescriptor {
            id: fields[0].to_owned(),
            index: displays.len(),
            name: fields[0].to_owned(),
            width,
            height,
            has_notch: false,
            is_builtin: fields.get(3).is_some_and(|v| v.trim().eq_ignore_ascii_case("true")),
            is_primary: fields[2].trim().eq_ignore_ascii_case("true"),
        });
    }
    displays
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display(id: &str, index: usize, primary: bool) -> DisplayDescriptor {
        DisplayDescriptor {
            id: id.to_owned(),
            index,
            name: id.to_owned(),
            width: 1920,
            height: 1080,
            has_notch: false,
            is_builtin: id == "builtin",
            is_primary: primary,
        }
    }

    #[test]
    fn top_offset_is_clamped_like_swift() {
        assert_eq!(dynamic_island_top_offset::clamp(-4.0), 0.0);
        assert_eq!(dynamic_island_top_offset::clamp(99.0), 40.0);
        assert_eq!(dynamic_island_top_offset::clamp(f64::NAN), 10.0);
    }

    #[test]
    fn the_legacy_target_prefers_a_screen_choice_then_main() {
        let displays = vec![display("a", 0, true), display("b", 1, false)];
        assert_eq!(planner::legacy_target_index("screen_1", &displays), Some(1));
        assert_eq!(planner::legacy_target_index("screen_9", &displays), Some(0));
        assert_eq!(planner::legacy_target_index("main", &displays), Some(0));
        assert_eq!(planner::legacy_target_index("auto", &displays), Some(0));
    }

    #[test]
    fn migration_enables_one_display_and_disables_the_rest() {
        let displays = vec![display("a", 0, true), display("b", 1, false)];
        let configs = planner::migrate_legacy(
            &displays,
            "main",
            IslandDisplayMode::DynamicIsland,
            12.0,
        );
        assert!(configs["a"].enabled);
        assert_eq!(configs["a"].display_mode, IslandDisplayMode::DynamicIsland);
        assert_eq!(configs["a"].dynamic_island_top_offset, 12.0);
        assert!(!configs["b"].enabled);
    }

    #[test]
    fn a_new_display_keeps_stored_keys_and_stays_off() {
        let displays = vec![display("a", 0, true), display("b", 1, false)];
        let mut stored = BTreeMap::new();
        stored.insert("a".to_owned(), display("a", 0, true).default_configuration(true));
        let (configs, changed) = planner::resolved_configurations(
            Some(&stored),
            &displays,
            "auto",
            IslandDisplayMode::Notch,
            10.0,
        );
        assert!(changed);
        assert!(configs.contains_key("a"));
        assert!(!configs["b"].enabled);
        assert_eq!(
            planner::primary_enabled_display_id(&displays, &configs).as_deref(),
            Some("a")
        );
    }

    #[test]
    fn a_clamshell_change_preserves_stored_keys_and_enables_a_legacy_target() {
        let displays = vec![display("external", 0, true)];
        let mut stored = BTreeMap::new();
        stored.insert("builtin".to_owned(), display("builtin", 0, false).default_configuration(true));
        let (configs, changed) = planner::resolved_configurations(
            Some(&stored),
            &displays,
            "main",
            IslandDisplayMode::Notch,
            10.0,
        );
        assert!(changed);
        assert!(configs["builtin"].enabled);
        assert!(configs["external"].enabled);
    }

    #[test]
    fn the_codec_drops_bad_entries_and_round_trips() {
        let raw = r#"{"a":{"enabled":true,"displayMode":"dynamicIsland","dynamicIslandTopOffset":44},"b":"bad"}"#;
        let decoded = codec::decode(Some(raw)).expect("a map");
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded["a"].dynamic_island_top_offset, 40.0);
        let encoded = codec::encode(&decoded);
        assert!(encoded.contains("dynamicIsland"));
        assert_eq!(codec::decode(Some("")).is_none(), true);
    }

    #[test]
    fn xrandr_parses_connected_screens_only() {
        let output = "Screen 0: minimum 320 x 200, current 3840 x 1080\neDP-1 connected primary 1920x1080+0+0 344mm x 194mm\n   1920x1080 60.00*\nHDMI-1 connected 1600x900+1920+0\nDP-1 disconnected (normal left inverted right x axis y axis)";
        let displays = parse_xrandr(output);
        assert_eq!(displays.len(), 2);
        assert_eq!(displays[0].id, "eDP-1");
        assert!(displays[0].is_primary);
        assert!(displays[0].is_builtin);
        assert_eq!(displays[1].resolution_label(), "1600×900");
    }

    #[test]
    fn powershell_screens_parse_the_shared_line_shape() {
        let output = "\\\\.\\DISPLAY1|1920x1080|True|False\n\\\\.\\DISPLAY2|2560x1440|False|True";
        let displays = parse_powershell_screens(output);
        assert_eq!(displays.len(), 2);
        assert!(displays[0].is_primary);
        assert!(displays[1].is_builtin);
    }
}
