//! #191 — the completion card's data. The core parses the last assistant
//! reply into blocks and inline spans; this module only maps them onto the
//! wire DTO the panel draws, and settles the attention policy's completion
//! intent with what this shell knows. No Markdown is parsed in the frontend.

use morrow_core::attention::{self, Completion};
use morrow_core::markdown::{self, MarkdownBlock, MarkdownInline};
use serde::Serialize;

pub use morrow_core::attention::{
    CompletionPresentation as Presentation, CompletionStyle as Style,
};

/// The reply, as tagged blocks. Mirrored in `src/lib/types.ts`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Block {
    Heading {
        level: usize,
        spans: Vec<MarkdownInline>,
    },
    Paragraph {
        spans: Vec<MarkdownInline>,
    },
    List {
        ordered: bool,
        start: i64,
        items: Vec<ListItem>,
    },
    Code {
        language: Option<String>,
        code: String,
        closed: bool,
    },
    Table {
        header: Vec<Vec<MarkdownInline>>,
        alignments: Vec<Alignment>,
        rows: Vec<Vec<Vec<MarkdownInline>>>,
    },
    Quote {
        blocks: Vec<Block>,
    },
    ThematicBreak,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ListItem {
    checkbox: Option<Checkbox>,
    blocks: Vec<Block>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Checkbox {
    Unchecked,
    Checked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Alignment {
    Automatic,
    Leading,
    Center,
    Trailing,
}

/// `Snapshot.completion`: the last finished turn, in the shape the panel
/// renders. `updated_at` is the identity of one card (one open per turn).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SnapshotCompletion {
    pub session_id: String,
    pub updated_at: u64,
    pub turn_failed: bool,
    pub presentation: Presentation,
    pub blocks: Vec<Block>,
}

fn spans(text: &str) -> Vec<MarkdownInline> {
    markdown::parse_inline(text)
}

fn convert_block(block: &MarkdownBlock) -> Block {
    match block {
        MarkdownBlock::Heading { level, text } => Block::Heading {
            level: *level,
            spans: spans(text),
        },
        MarkdownBlock::Paragraph(text) => Block::Paragraph { spans: spans(text) },
        MarkdownBlock::List(list) => Block::List {
            ordered: list.is_ordered,
            start: list.start,
            items: list
                .items
                .iter()
                .map(|item| ListItem {
                    checkbox: item.checkbox.map(|box_| match box_ {
                        markdown::Checkbox::Unchecked => Checkbox::Unchecked,
                        markdown::Checkbox::Checked => Checkbox::Checked,
                    }),
                    blocks: item.blocks.iter().map(convert_block).collect(),
                })
                .collect(),
        },
        MarkdownBlock::Code(code) => Block::Code {
            language: code.language.clone(),
            code: code.code.clone(),
            closed: code.is_closed,
        },
        MarkdownBlock::Table(table) => Block::Table {
            header: table.header.iter().map(|cell| spans(cell)).collect(),
            alignments: table
                .alignments
                .iter()
                .map(|a| match a {
                    markdown::MarkdownTableAlignment::Automatic => Alignment::Automatic,
                    markdown::MarkdownTableAlignment::Leading => Alignment::Leading,
                    markdown::MarkdownTableAlignment::Center => Alignment::Center,
                    markdown::MarkdownTableAlignment::Trailing => Alignment::Trailing,
                })
                .collect(),
            rows: table
                .rows
                .iter()
                .map(|row| row.iter().map(|cell| spans(cell)).collect())
                .collect(),
        },
        MarkdownBlock::Quote(blocks) => Block::Quote {
            blocks: blocks.iter().map(convert_block).collect(),
        },
        MarkdownBlock::ThematicBreak => Block::ThematicBreak,
    }
}

/// The core's blocks for a reply, ready for the panel. A turn with no reply
/// text gets the same placeholder the reducer uses for its session card.
pub fn blocks(reply: &str) -> Vec<Block> {
    let reply = if reply.trim().is_empty() {
        "Reply complete"
    } else {
        reply
    };
    markdown::parse(reply).iter().map(convert_block).collect()
}

/// Settle a completion intent the way macOS does: settings first, then Smart
/// Suppress. No intent (a child turn, a turn held elsewhere) means no card.
pub fn presentation(
    completion: Option<&Completion>,
    style: Style,
    session_in_front: bool,
) -> Presentation {
    attention::settle_completion(completion, style, session_in_front)
        .unwrap_or(Presentation::StatusOnly)
}

#[cfg(test)]
mod tests {
    use super::*;
    use morrow_core::attention::{AttentionMoment, AttentionSubject, TurnEnd, TurnHost, intents};
    use serde_json::json;

    #[test]
    fn core_markdown_becomes_tagged_blocks_with_parsed_inline_spans() {
        let blocks = blocks(
            "## Dashboard filters\n\
             Added a **date-range** and `useOrderFilters` bar.\n\n\
             - Presets: today\n\
             - Status chips are multi-select\n\n\
             ```ts\nconst filters = useOrderFilters();\n```\n\n\
             | Suite | Tests |\n| --- | --- |\n| orders | 28 |\n\n\
             > See [docs](https://example.com).",
        );
        assert_eq!(
            serde_json::to_value(&blocks).unwrap(),
            json!([
                {"kind": "heading", "level": 2, "spans": [{"kind": "text", "text": "Dashboard filters"}]},
                {"kind": "paragraph", "spans": [
                    {"kind": "text", "text": "Added a "},
                    {"kind": "strong", "spans": [{"kind": "text", "text": "date-range"}]},
                    {"kind": "text", "text": " and "},
                    {"kind": "code", "text": "useOrderFilters"},
                    {"kind": "text", "text": " bar."}
                ]},
                {"kind": "list", "ordered": false, "start": 1, "items": [
                    {"checkbox": null, "blocks": [{"kind": "paragraph", "spans": [{"kind": "text", "text": "Presets: today"}]}]},
                    {"checkbox": null, "blocks": [{"kind": "paragraph", "spans": [{"kind": "text", "text": "Status chips are multi-select"}]}]}
                ]},
                {"kind": "code", "language": "ts", "code": "const filters = useOrderFilters();", "closed": true},
                {"kind": "table", "header": [
                    [{"kind": "text", "text": "Suite"}], [{"kind": "text", "text": "Tests"}]
                ], "alignments": ["automatic", "automatic"], "rows": [
                    [[{"kind": "text", "text": "orders"}], [{"kind": "text", "text": "28"}]]
                ]},
                {"kind": "quote", "blocks": [{"kind": "paragraph", "spans": [
                    {"kind": "text", "text": "See "},
                    {"kind": "link", "text": "docs", "url": "https://example.com"},
                    {"kind": "text", "text": "."}
                ]}]}
            ])
        );
    }

    #[test]
    fn a_finished_turn_settles_to_a_card_unless_the_user_asked_for_less() {
        let intents = intents(
            &AttentionMoment::TurnEnded {
                end: TurnEnd::Completed,
                host: TurnHost::Aiwork,
            },
            AttentionSubject::default(),
        );
        assert_eq!(
            presentation(intents.completion.as_ref(), Style::Expand, false),
            Presentation::Card
        );
        assert_eq!(
            presentation(intents.completion.as_ref(), Style::Glance, false),
            Presentation::Glance
        );
        assert_eq!(
            presentation(intents.completion.as_ref(), Style::Off, false),
            Presentation::StatusOnly
        );
        // Smart Suppress keeps the card down when the session's terminal is
        // in front; macOS still tracks the turn and shows the glance later.
        assert_eq!(
            presentation(intents.completion.as_ref(), Style::Expand, true),
            Presentation::Glance
        );
    }

    #[test]
    fn a_child_turn_has_no_completion_to_settle() {
        let intents = intents(
            &AttentionMoment::TurnEnded {
                end: TurnEnd::Completed,
                host: TurnHost::Aiwork,
            },
            AttentionSubject {
                is_child: true,
                interrupted: false,
            },
        );
        assert_eq!(
            presentation(intents.completion.as_ref(), Style::Expand, false),
            Presentation::StatusOnly
        );
    }
}
