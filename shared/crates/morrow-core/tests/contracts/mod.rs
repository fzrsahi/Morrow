//! The core contract's projections, as `macos/Tests/MorrowCoreTests/CoreContractJSONExporter.swift`
//! writes them: what the policy's intents look like in
//! `fixtures/core/attention.json` and in a reducer step of
//! `fixtures/core/reducer.json`.
//!
//! Shared by `attention_conformance.rs` and `reducer_conformance.rs`.
#![allow(dead_code)]

use serde_json::{Map, Value, json};

use morrow_core::attention::{
    AttentionDecision, AttentionIntents, AttentionPushKind, CompletionPresentation, FollowUpIntent,
};

fn push_kind(kind: AttentionPushKind) -> Value {
    match kind {
        AttentionPushKind::Error => json!({"type": "error"}),
        AttentionPushKind::Completion {
            is_subagent,
            interrupted,
        } => json!({
            "type": "completion",
            "is_subagent": is_subagent,
            "interrupted": interrupted,
        }),
        AttentionPushKind::Approval => json!({"type": "approval"}),
        AttentionPushKind::Question => json!({"type": "question"}),
        AttentionPushKind::DisplayOnlyWait => json!({"type": "displayOnlyWait"}),
    }
}

/// `CoreContractJSONExporter`'s `intents` document.
pub fn intents_document(intents: &AttentionIntents) -> Value {
    let mut doc = Map::new();
    doc.insert("sound".into(), json!(intents.sound));
    doc.insert(
        "completion".into(),
        match intents.completion {
            Some(completion) => json!({"turn_failed": completion.turn_failed}),
            None => Value::Null,
        },
    );
    doc.insert(
        "push".into(),
        match intents.push {
            Some(push) => json!({
                "kind": push_kind(push.kind),
                "smart_suppressible": push.smart_suppressible,
            }),
            None => Value::Null,
        },
    );
    doc.insert("follow_up".into(), follow_up_document(intents.follow_up));
    Value::Object(doc)
}

fn follow_up_document(follow_up: Option<FollowUpIntent>) -> Value {
    match follow_up {
        Some(FollowUpIntent::TrackCompletion { turn_failed }) => {
            json!({"type": "trackCompletion", "turn_failed": turn_failed})
        }
        Some(FollowUpIntent::RestartWait(kind)) => {
            json!({"type": "restartWait", "kind": kind.raw_value()})
        }
        None => Value::Null,
    }
}

/// `CoreContractJSONExporter`'s `decision` document in settle.json.
pub fn decision_document(decision: &AttentionDecision) -> Value {
    let mut doc = Map::new();
    doc.insert("sound".into(), json!(decision.sound));
    doc.insert(
        "completion".into(),
        match &decision.completion {
            Some(completion) => json!({
                "turn_failed": completion.turn_failed,
                "presentation": match completion.presentation {
                    CompletionPresentation::StatusOnly => "status_only",
                    CompletionPresentation::Glance => "glance",
                    CompletionPresentation::Card => "card",
                },
            }),
            None => Value::Null,
        },
    );
    doc.insert(
        "push".into(),
        match &decision.push {
            Some(push) => json!({
                "kind": push_kind(push.kind),
                "smart_suppressed": push.smart_suppressed,
            }),
            None => Value::Null,
        },
    );
    doc.insert("follow_up".into(), follow_up_document(decision.follow_up));
    Value::Object(doc)
}
