//! The Rust port of `MarkdownPreviewText` against values Swift produced
//! (`macos/Tests/MorrowCoreTests/MarkdownPreviewFixtureExporter.swift`).
//!
//! The unit tests in the module carry expectations copied out of the Swift test file;
//! this one compares against a generated artefact, so a change on the Swift side that
//! nobody mirrored fails here instead of on a card.

use morrow_core::markdown_preview::{
    SEPARATOR, SOURCE_BUDGET, SOURCE_HARD_LIMIT, plain, preview_source,
};
use serde_json::Value;

fn fixture() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contract/fixtures/core/markdown_preview.json"
    );
    let text = std::fs::read_to_string(path).expect("fixtures/core/markdown_preview.json");
    serde_json::from_str(&text).expect("valid JSON")
}

#[test]
fn the_budgets_are_the_ones_swift_uses() {
    let fixture = fixture();
    assert_eq!(
        fixture["source_budget"].as_u64(),
        Some(SOURCE_BUDGET as u64)
    );
    assert_eq!(
        fixture["source_hard_limit"].as_u64(),
        Some(SOURCE_HARD_LIMIT as u64)
    );
    assert_eq!(fixture["separator"].as_str(), Some(SEPARATOR));
}

#[test]
fn every_case_matches_swift() {
    let fixture = fixture();
    let cases = fixture["cases"].as_array().expect("cases");
    assert!(cases.len() >= 16, "{} cases", cases.len());

    for case in cases {
        let name = case["name"].as_str().expect("name");
        let input = case["input"].as_str().expect("input");

        let single = plain(input, true);
        assert_eq!(
            single,
            case["single_line"].as_str().expect("single_line"),
            "{name}: single-line preview"
        );

        let multi = plain(input, false);
        assert_eq!(
            multi,
            case["multi_line"].as_str().expect("multi_line"),
            "{name}: multi-line preview"
        );

        let source = preview_source(input);
        assert_eq!(
            source,
            case["preview_source"].as_str().expect("preview_source"),
            "{name}: preview source"
        );
    }
}

#[test]
fn the_visible_head_of_a_cut_preview_is_the_uncut_head() {
    // The fixture's big cases are cut; the first line the card draws must be the same
    // either way, which is the whole reason the cut may happen at all.
    let fixture = fixture();
    for case in fixture["cases"].as_array().expect("cases") {
        let name = case["name"].as_str().expect("name");
        let input = case["input"].as_str().expect("input");
        let source = preview_source(input);
        if source.len() == input.len() {
            continue;
        }
        let cut = plain(&source, true);
        let whole = plain(input, true);
        let shared: String = cut.chars().take(40).collect();
        assert!(
            whole.starts_with(&shared),
            "{name}: the cut preview reads differently from the start"
        );
    }
}
