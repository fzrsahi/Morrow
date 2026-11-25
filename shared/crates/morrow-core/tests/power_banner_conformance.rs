//! The Rust port of the pure half of `SystemMonitorManager` against values
//! Swift produced (`PowerBannerFixtureExporter.swift`). The unit tests in the
//! module carry a few hand-derived cases; here every expectation comes from
//! the app's own code, so a change on the Swift side that nobody mirrors fails
//! here instead of on a charging banner.

use morrow_core::power_banner::{
    adapter_kind, format_speed, power_banner_state, PowerBannerState, PowerSourceKind,
};
use serde_json::{Map, Value};

fn fixture() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contract/fixtures/core/power_banner.json"
    );
    let text = std::fs::read_to_string(path).expect("fixtures/core/power_banner.json");
    serde_json::from_str(&text).expect("valid JSON")
}

fn kind(tag: &str) -> PowerSourceKind {
    PowerSourceKind::from_tag(tag).expect("known kind")
}

/// The same shape the Swift exporter writes, so missing keys matter: a zero or
/// negative watt/time value is omitted by Swift and must be omitted here too.
fn state_json(state: &PowerBannerState) -> Value {
    let mut object = Map::new();
    match state {
        PowerBannerState::Charging {
            kind,
            watts,
            time_to_full_minutes,
        } => {
            object.insert("tag".into(), Value::from(state.tag()));
            object.insert("kind".into(), Value::from(kind.tag()));
            if let Some(watts) = watts {
                object.insert("watts".into(), Value::from(*watts));
            }
            if let Some(minutes) = time_to_full_minutes {
                object.insert("time_to_full".into(), Value::from(*minutes));
            }
        }
        PowerBannerState::OnHold { kind } | PowerBannerState::FullyCharged { kind } => {
            object.insert("tag".into(), Value::from(state.tag()));
            object.insert("kind".into(), Value::from(kind.tag()));
        }
        PowerBannerState::LowBattery { percent } => {
            object.insert("tag".into(), Value::from(state.tag()));
            object.insert("percent".into(), Value::from(*percent));
        }
    }
    object.insert("title".into(), Value::from(state.title()));
    object.insert("subtitle".into(), Value::from(state.subtitle()));
    Value::Object(object)
}

#[test]
fn banner_text_matches_swift() {
    let bundle = fixture();
    let cases = bundle["metadata_cases"].as_array().expect("metadata_cases");
    assert!(cases.len() >= 8, "{} cases", cases.len());

    for case in cases {
        let name = case["name"].as_str().expect("name");
        let expected = &case["expected"];
        let state = match expected["tag"].as_str().expect("tag") {
            "charging" => PowerBannerState::Charging {
                kind: kind(expected["kind"].as_str().expect("kind")),
                watts: expected.get("watts").and_then(Value::as_i64),
                time_to_full_minutes: expected.get("time_to_full").and_then(Value::as_i64),
            },
            "onHold" => PowerBannerState::OnHold {
                kind: kind(expected["kind"].as_str().expect("kind")),
            },
            "fullyCharged" => PowerBannerState::FullyCharged {
                kind: kind(expected["kind"].as_str().expect("kind")),
            },
            "lowBattery" => PowerBannerState::LowBattery {
                percent: expected["percent"].as_i64().expect("percent"),
            },
            other => panic!("{name}: unknown tag {other}"),
        };
        assert_eq!(state_json(&state), *expected, "{name}");
    }
}

#[test]
fn thresholds_match_swift() {
    let bundle = fixture();
    let cases = bundle["resolver_cases"].as_array().expect("resolver_cases");
    assert!(cases.len() >= 10, "{} cases", cases.len());

    for case in cases {
        let name = case["name"].as_str().expect("name");
        let state = power_banner_state(
            kind(case["kind"].as_str().expect("kind")),
            case["is_charging"].as_bool().expect("is_charging"),
            case["is_ac"].as_bool().expect("is_ac"),
            case["is_charged"].as_bool().expect("is_charged"),
            case["battery_level"].as_i64().expect("battery_level"),
            case.get("adapter_watts").and_then(Value::as_i64),
            case.get("raw_time_to_full").and_then(Value::as_i64),
        );
        assert_eq!(state_json(&state), case["expected"], "{name}");
    }
}

#[test]
fn adapter_classification_matches_swift() {
    let bundle = fixture();
    let cases = bundle["adapter_cases"].as_array().expect("adapter_cases");
    assert!(cases.len() >= 11, "{} cases", cases.len());

    for case in cases {
        let name = case["name"].as_str().expect("name");
        let actual = adapter_kind(
            case.get("adapter_name").and_then(Value::as_str),
            case.get("source").and_then(Value::as_str),
            case.get("description").and_then(Value::as_str),
            case["family"].as_i64().expect("family"),
            case["mag_safe_connected"]
                .as_bool()
                .expect("mag_safe_connected"),
        );
        assert_eq!(actual.tag(), case["kind"].as_str().expect("kind"), "{name}");
    }
}

#[test]
fn speed_format_matches_swift() {
    let bundle = fixture();
    let cases = bundle["speed_cases"].as_array().expect("speed_cases");
    assert!(cases.len() >= 9, "{} cases", cases.len());

    for case in cases {
        let name = case["name"].as_str().expect("name");
        assert_eq!(
            format_speed(case["bytes_per_sec"].as_f64().expect("bytes_per_sec")),
            case["formatted"].as_str().expect("formatted"),
            "{name}"
        );
    }
}
