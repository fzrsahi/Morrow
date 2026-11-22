//! The discovery readers against fixtures exported from Swift (#245).
//!
//! `shared/contract/fixtures/discovery/readers.json` is written by
//! `macos/Tests/MorrowTests/DiscoveryFixtureExporter.swift`, which calls the
//! app's own readers. Re-export with:
//!
//! ```text
//! cd macos && CODEISLAND_DISCOVERY_EXPORT_JSON=../shared/contract/fixtures \
//!   swift test --filter DiscoveryFixtureExporter
//! ```
//!
//! So this file asserts parity with Swift, not with a hand-written expectation.

use std::fs;

use morrow_core::discovery::{claude, cursor, extract_text_content, gemini, project_dir};
use serde_json::Value;

fn fixture() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../contract/fixtures/discovery/readers.json");
    let text =
        fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_json::from_str(&text).expect("a JSON fixture")
}

fn text_of(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

/// `[{is_user, text}]` as the fixture writes it.
fn messages(value: &Value) -> Vec<(bool, String)> {
    value
        .as_array()
        .expect("an array of messages")
        .iter()
        .map(|message| {
            (
                message["is_user"].as_bool().expect("is_user"),
                message["text"].as_str().expect("text").to_owned(),
            )
        })
        .collect()
}

#[test]
fn the_cursor_reader_matches_swift() {
    let fixture = fixture();
    let case = &fixture["cursor"];
    let transcript = case["transcript"].as_str().expect("a transcript");
    let expected = messages(&case["expected"]["messages"]);

    let actual: Vec<(bool, String)> = cursor::recent_messages(transcript)
        .into_iter()
        .map(|message| (message.is_user, message.text))
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn the_gemini_reader_matches_swift() {
    let fixture = fixture();
    let case = &fixture["gemini"];
    let session = case["session"].as_str().expect("a session file");

    assert_eq!(
        gemini::session_id(session),
        text_of(&case["expected"]["session_id"])
    );
    assert_eq!(gemini::model(session), text_of(&case["expected"]["model"]));
    let actual: Vec<(bool, String)> = gemini::recent_messages(session)
        .into_iter()
        .map(|message| (message.is_user, message.text))
        .collect();
    assert_eq!(actual, messages(&case["expected"]["messages"]));
}

#[test]
fn the_claude_model_reader_matches_swift() {
    let fixture = fixture();
    let case = &fixture["claude"];

    // An empty `model` is not a name: the reader skips it and finds nothing.
    let empty = case["empty_transcript"].as_str().expect("a transcript");
    assert_eq!(claude::model(empty), text_of(&case["empty_expected"]));

    let named = case["named_transcript"].as_str().expect("a transcript");
    assert_eq!(claude::model(named), text_of(&case["named_expected"]));
}

#[test]
fn the_project_directory_encoding_matches_swift() {
    let fixture = fixture();
    for case in fixture["project_dir"].as_array().expect("cases") {
        let path = case["path"].as_str().expect("a path");
        assert_eq!(
            project_dir::encoded(path),
            case["encoded"].as_str().expect("encoded"),
            "claudeProjectDirEncoded({path})"
        );
        assert_eq!(
            project_dir::app_encoded(path),
            case["app_encoded"].as_str().expect("app_encoded"),
            "appProjectDirEncoded({path})"
        );
    }
}

/// A store the platform could not read completely is not a crash: the readers
/// answer "nothing", which is what the shell renders as an empty panel.
#[test]
fn a_truncated_store_is_empty_rather_than_a_panic() {
    assert_eq!(claude::model("{\"type\":\"assistant\",\"mess"), None);
    assert!(cursor::recent_messages("{\"role\":\"user\"").is_empty());
    assert!(gemini::recent_messages("{\"sessionId\":").is_empty());
    assert_eq!(gemini::session_id("{\"sessionId\":"), None);
    assert!(cursor::recent_messages("").is_empty());
    assert!(gemini::recent_messages("").is_empty());
}

/// The message-text rule both Cursor and Gemini lean on: a bare string, the
/// `text` items, and the nested `output.text` a tool result uses.
#[test]
fn text_content_accepts_every_shape_a_store_writes() {
    let bare = serde_json::json!("plain");
    assert_eq!(extract_text_content(Some(&bare)), Some("plain".into()));

    let items = serde_json::json!([{"type": "text", "text": "from items"}]);
    assert_eq!(
        extract_text_content(Some(&items)),
        Some("from items".into())
    );

    let output = serde_json::json!([{"output": {"text": "from output"}}]);
    assert_eq!(
        extract_text_content(Some(&output)),
        Some("from output".into())
    );

    let empty = serde_json::json!("");
    assert_eq!(extract_text_content(Some(&empty)), None);
    assert_eq!(extract_text_content(None), None);
}
