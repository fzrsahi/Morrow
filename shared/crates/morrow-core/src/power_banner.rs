//! The pure rules behind the macOS charging banner
//! (`macos/Sources/Morrow/SystemMonitorManager.swift`).
//!
//! IOKit, the run loop and the view stay in the shell: this module starts where
//! a power-source reading ends. It decides which banner a reading means, how
//! the adapter is named, what the banner says and how a network speed reads.
//! Swift's `PowerBannerResolver`, `PowerBannerState` and `formatSpeed` are the
//! source of truth; `shared/contract/fixtures/core/power_banner.json` is what
//! they produced and is checked by `tests/power_banner_conformance.rs`.
//!
//! The semantic colours and pixel glyph are deliberately not ported: they are
//! SwiftUI `Color` values and a view's icon, not values a shell can consume.

/// `PowerSourceKind` in Swift: the adapter that is connected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PowerSourceKind {
    MagSafe,
    UsbC,
}

impl PowerSourceKind {
    /// The label Swift puts in the banner subtitle.
    pub fn label(self) -> &'static str {
        match self {
            Self::MagSafe => "MagSafe",
            Self::UsbC => "USB-C",
        }
    }

    /// The raw value in the Swift fixture.
    pub fn tag(self) -> &'static str {
        match self {
            Self::MagSafe => "magSafe",
            Self::UsbC => "usbC",
        }
    }

    /// Parse the Swift fixture's `kind` value.
    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "magSafe" => Some(Self::MagSafe),
            "usbC" => Some(Self::UsbC),
            _ => None,
        }
    }
}

/// `PowerBannerState` in Swift, without the view-only colour and glyph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PowerBannerState {
    Charging {
        kind: PowerSourceKind,
        watts: Option<i64>,
        time_to_full_minutes: Option<i64>,
    },
    OnHold {
        kind: PowerSourceKind,
    },
    FullyCharged {
        kind: PowerSourceKind,
    },
    LowBattery {
        percent: i64,
    },
}

impl PowerBannerState {
    /// The Swift `title`.
    pub fn title(&self) -> &'static str {
        match self {
            Self::Charging { .. } => "Charging",
            Self::OnHold { .. } => "Charging On Hold",
            Self::FullyCharged { .. } => "Fully Charged",
            Self::LowBattery { .. } => "Low Battery",
        }
    }

    /// The Swift `subtitle`: watts and time are shown only when positive, and
    /// a time at or above an hour is split into hours and minutes.
    pub fn subtitle(&self) -> String {
        match self {
            Self::Charging {
                kind,
                watts,
                time_to_full_minutes,
            } => {
                let mut parts = vec![kind.label().to_owned()];
                if let Some(watts) = watts {
                    if *watts > 0 {
                        parts.push(format!("{watts}W"));
                    }
                }
                if let Some(mins) = time_to_full_minutes {
                    if *mins > 0 {
                        let hours = mins / 60;
                        let minutes = mins % 60;
                        parts.push(if hours > 0 {
                            format!("{hours}h {minutes}m left")
                        } else {
                            format!("{minutes}m left")
                        });
                    }
                }
                parts.join(" · ")
            }
            Self::OnHold { kind } => format!("{} · Optimized charging", kind.label()),
            Self::FullyCharged { kind } => format!("{} · 100% charged", kind.label()),
            Self::LowBattery { .. } => "Connect power adapter".to_owned(),
        }
    }

    /// The tag Swift writes into the conformance fixture.
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Charging { .. } => "charging",
            Self::OnHold { .. } => "onHold",
            Self::FullyCharged { .. } => "fullyCharged",
            Self::LowBattery { .. } => "lowBattery",
        }
    }
}

/// The threshold chain in Swift's `PowerBannerResolver.state`.
///
/// `is_charged` and `battery_level` describe the battery, `is_ac` and
/// `is_charging` the power source. A non-positive estimated time is treated as
/// unknown, the same way Swift drops `rawTimeToFullMinutes <= 0`.
pub fn power_banner_state(
    kind: PowerSourceKind,
    is_charging: bool,
    is_ac: bool,
    is_charged: bool,
    battery_level: i64,
    adapter_watts: Option<i64>,
    raw_time_to_full_minutes: Option<i64>,
) -> PowerBannerState {
    let time_to_full = match raw_time_to_full_minutes {
        Some(minutes) if minutes > 0 => Some(minutes),
        _ => None,
    };
    if !is_charging && is_ac && (is_charged || battery_level >= 99) {
        PowerBannerState::FullyCharged { kind }
    } else if !is_charging && is_ac {
        PowerBannerState::OnHold { kind }
    } else if !is_charging && !is_ac && battery_level <= 20 {
        PowerBannerState::LowBattery {
            percent: battery_level,
        }
    } else {
        PowerBannerState::Charging {
            kind,
            watts: adapter_watts,
            time_to_full_minutes: time_to_full,
        }
    }
}

/// The adapter-name rules in Swift's `PowerBannerResolver.adapterKind`.
///
/// `mag_safe_connected` is the one IOKit fact that cannot be read from the
/// adapter dictionary, so the shell hands it in with the three strings and the
/// adapter family. An empty `name` does not fall back to `source`: Swift's
/// `??` only falls back on `nil`.
pub fn adapter_kind(
    name: Option<&str>,
    source: Option<&str>,
    description: Option<&str>,
    family: i64,
    magsafe_connected: bool,
) -> PowerSourceKind {
    let lower_name = name.or(source).unwrap_or("").to_lowercase();
    let lower_description = description.unwrap_or("").to_lowercase();
    let combined = format!("{lower_name} {lower_description}");
    if magsafe_connected || combined.contains("magsafe") {
        return PowerSourceKind::MagSafe;
    }
    if combined.contains("usb") || combined.contains("pd") || combined.contains("type-c") {
        return PowerSourceKind::UsbC;
    }
    if family == 0xE000400A || family == 0xE0004007 {
        PowerSourceKind::UsbC
    } else {
        PowerSourceKind::MagSafe
    }
}

/// Swift's `SystemMonitorManager.formatSpeed`.
pub fn format_speed(bytes_per_sec: f64) -> String {
    if bytes_per_sec >= 1024.0 * 1024.0 {
        format!("{:.1}M", bytes_per_sec / (1024.0 * 1024.0))
    } else if bytes_per_sec >= 1024.0 {
        format!("{:.0}K", bytes_per_sec / 1024.0)
    } else {
        format!("{}B", bytes_per_sec as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_charged_flag_wins_over_the_capacity_threshold() {
        let state = power_banner_state(PowerSourceKind::MagSafe, false, true, true, 80, None, None);
        assert_eq!(state, PowerBannerState::FullyCharged { kind: PowerSourceKind::MagSafe });
    }

    #[test]
    fn a_non_positive_time_is_unknown() {
        let state = power_banner_state(
            PowerSourceKind::UsbC,
            true,
            true,
            false,
            40,
            Some(96),
            Some(0),
        );
        assert_eq!(
            state,
            PowerBannerState::Charging {
                kind: PowerSourceKind::UsbC,
                watts: Some(96),
                time_to_full_minutes: None,
            }
        );
        assert_eq!(state.subtitle(), "USB-C · 96W");
    }

    #[test]
    fn an_empty_name_does_not_use_the_source() {
        assert_eq!(
            adapter_kind(Some(""), Some("USB-C Power Adapter"), None, 0, false),
            PowerSourceKind::MagSafe
        );
    }
}
