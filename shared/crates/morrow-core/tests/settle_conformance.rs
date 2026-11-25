//! `AttentionPolicy.settle` on every case of `fixtures/core/settle.json`
//! (exported from Swift by `macos/Tests/MorrowCoreTests/CoreContractJSONExporter.swift`
//! out of `AttentionPolicyTests`' settle rows): the decision Swift settles
//! for an intents/context pair must be the one morrow-core settles.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde_json::Value;

use morrow_core::attention::{
    AttentionContext, AttentionIntents, AttentionPushKind, Completion, CompletionPresentation,
    CompletionStyle, FollowUpIntent, FollowUpReminderKind, Push, SoundHold, SoundPreferences,
    settle, settle_completion,
};

mod contracts;

use contracts::decision_document;

/// Every case of the bundle; a change in the bundle shows here.
const CASES: usize = 20;

fn bundle() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../shared/contract/fixtures/core/settle.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).expect("settle.json")
}

// MARK: - Reading a case

fn push_kind(value: &Value) -> AttentionPushKind {
    match value["type"].as_str().expect("a push kind") {
        "error" => AttentionPushKind::Error,
        "completion" => AttentionPushKind::Completion {
            is_subagent: value["is_subagent"] == true,
            interrupted: value["interrupted"] == true,
        },
        "approval" => AttentionPushKind::Approval,
        "question" => AttentionPushKind::Question,
        "displayOnlyWait" => AttentionPushKind::DisplayOnlyWait,
        other => panic!("push kind {other}"),
    }
}

fn follow_up(value: &Value) -> Option<FollowUpIntent> {
    Some(match value.get("type")?.as_str()? {
        "trackCompletion" => FollowUpIntent::TrackCompletion {
            turn_failed: value["turn_failed"] == true,
        },
        "restartWait" => FollowUpIntent::RestartWait(
            FollowUpReminderKind::from_raw_value(value["kind"].as_str().expect("a reminder kind"))
                .unwrap_or_else(|| panic!("reminder kind {}", value["kind"])),
        ),
        other => panic!("follow-up kind {other}"),
    })
}

fn intents(value: &Value) -> AttentionIntents {
    AttentionIntents {
        sound: value["sound"].as_str().map(str::to_owned),
        completion: (!value["completion"].is_null()).then(|| Completion {
            turn_failed: value["completion"]["turn_failed"] == true,
        }),
        push: (!value["push"].is_null()).then(|| Push {
            kind: push_kind(&value["push"]["kind"]),
            smart_suppressible: value["push"]["smart_suppressible"] == true,
        }),
        follow_up: follow_up(&value["follow_up"]),
    }
}

fn context(value: &Value) -> AttentionContext {
    let audible_cues = value["audible_cues"]
        .as_array()
        .expect("audible cues")
        .iter()
        .map(|cue| cue.as_str().expect("a cue").to_owned())
        .collect::<BTreeSet<_>>();
    let completion_style = match value["completion_style"]
        .as_str()
        .expect("a completion style")
    {
        "expand" => CompletionStyle::Expand,
        "glance" => CompletionStyle::Glance,
        "off" => CompletionStyle::Off,
        other => panic!("completion style {other}"),
    };
    AttentionContext {
        sounds: SoundPreferences {
            enabled: value["sound_enabled"] == true,
            audible_cues,
        },
        sound_hold: SoundHold {
            quiet_hours: value["quiet_hours"] == true,
            mute_when_away: value["mute_when_away"] == true,
            scene_quiet: value["scene_quiet"] == true,
        },
        completion_style,
        push_enabled: value["push_enabled"] == true,
        session_in_front: value["session_in_front"] == true,
    }
}

// MARK: - Running the bundle

#[test]
fn every_settle_case_matches_swift() {
    let bundle = bundle();
    assert_eq!(bundle["contract"], "settle");
    let cases = bundle["cases"].as_array().unwrap();
    let mut failures = Vec::new();
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let got = decision_document(&settle(
            &intents(&case["intents"]),
            &context(&case["context"]),
        ));
        if got != case["decision"] {
            failures.push(format!(
                "{name}:\n  rust:  {got}\n  swift: {}",
                case["decision"]
            ));
        }
    }
    println!(
        "settle: {}/{} cases",
        cases.len() - failures.len(),
        cases.len()
    );
    assert!(
        failures.is_empty(),
        "{} of {} cases differ from Swift:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
    assert_eq!(cases.len(), CASES);
}

#[test]
fn settle_completion_keeps_the_shell_rule() {
    let completion = Completion { turn_failed: false };
    assert_eq!(
        settle_completion(None, CompletionStyle::Expand, false),
        None
    );
    assert_eq!(
        settle_completion(Some(&completion), CompletionStyle::Off, true),
        Some(CompletionPresentation::StatusOnly)
    );
    assert_eq!(
        settle_completion(Some(&completion), CompletionStyle::Glance, false),
        Some(CompletionPresentation::Glance)
    );
    assert_eq!(
        settle_completion(Some(&completion), CompletionStyle::Expand, false),
        Some(CompletionPresentation::Card)
    );
    // The shell asks Smart Suppress when the card shows, not at decision time.
    assert_eq!(
        settle_completion(Some(&completion), CompletionStyle::Expand, true),
        Some(CompletionPresentation::Glance)
    );
}
