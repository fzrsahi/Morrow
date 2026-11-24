//! Folding a reply's block structure into running text for line-capped previews
//! (`macos/Sources/MorrowCore/MarkdownPreviewText.swift`).
//!
//! A preview cut to one line cannot show a table or a code block — it can only show
//! their syntax: `## `, `- [ ]`, `| --- |`, ```` ```swift ````. Every block is reduced
//! to its words instead: headings lose their `#`, list items join with ` · `, table
//! rows become "cell, cell", a code block shows its first line.
//!
//! What this **does not** carry is styling. macOS returns an `AttributedString`
//! because the notch draws a bold heading and a code-styled run; the card in the port
//! draws plain text, so this returns `String`. If a surface ever wants the bold back,
//! the segments here are the place to grow — not the parser, which already tags runs
//! (`markdown::MarkdownInline`).

use crate::markdown::{
    self, Checkbox, MarkdownBlock, MarkdownCodeBlock, MarkdownList, MarkdownTable,
};

/// Joins list items, table rows and — in single-line mode — blocks.
pub const SEPARATOR: &str = " · ";

/// Bytes of text outside code fences worth flattening. A preview shows a few lines at
/// most — the island caps rows at five, the Codex live bar 160 characters — while
/// flattening parses and inline-renders the whole reply: ~120 ms for a 139 KB Codex
/// output, redone for every streamed chunk.
pub const SOURCE_BUDGET: usize = 8 * 1024;

/// Bytes in all, code included. Code only contributes its first line to a preview, so
/// it does not count against [`SOURCE_BUDGET`] — a long sample cannot hide the text
/// after it — but an unclosed fence must still end.
pub const SOURCE_HARD_LIMIT: usize = 64 * 1024;

/// The preview of `text`, everything on one line when `single_line`.
pub fn plain(text: &str, single_line: bool) -> String {
    let blocks = markdown::parse(text);
    join(&segments(&blocks, single_line), single_line)
}

/// The head of `text` a preview is built from: whole lines up to the budget, then as
/// much of the next line as fits. Short replies come back unchanged.
pub fn preview_source(text: &str) -> String {
    if text.len() <= SOURCE_BUDGET {
        return text.to_owned();
    }
    let mut prose = 0usize;
    let mut total = 0usize;
    let mut fence: Option<markdown::Fence> = None;
    let mut offset = 0usize;
    loop {
        let rest = &text[offset..];
        let (line, after) = match rest.find('\n') {
            Some(newline) => (&rest[..newline], &rest[newline + 1..]),
            None => (rest, ""),
        };
        let bytes = line.len();
        let mut is_prose = false;
        if let Some(open) = &fence {
            if markdown::closes_fence(line, open) {
                fence = None;
            }
        } else if let Some(open) = markdown::opening_fence(line) {
            fence = Some(open);
        } else {
            is_prose = true;
        }
        let allowance = if is_prose {
            (SOURCE_BUDGET - prose).min(SOURCE_HARD_LIMIT.saturating_sub(total))
        } else {
            SOURCE_HARD_LIMIT.saturating_sub(total)
        };
        if bytes > allowance {
            return format!("{}{}", &text[..offset], head_of(line, allowance));
        }
        if is_prose {
            prose += bytes;
        }
        total += bytes + 1;
        if after.is_empty() {
            break;
        }
        offset = text.len() - after.len();
    }
    text.to_owned()
}

/// As much of `line` as fits in `allowance` bytes, never splitting a character.
fn head_of(line: &str, allowance: usize) -> &str {
    let mut end = 0;
    for (index, ch) in line.char_indices() {
        if index + ch.len_utf8() > allowance {
            return &line[..end];
        }
        end = index + ch.len_utf8();
    }
    line
}

// MARK: - Segments

/// One segment per block, already inline-rendered. Empty blocks (a lone `#` mid-stream,
/// a thematic break) produce nothing.
fn segments(blocks: &[MarkdownBlock], single_line: bool) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    for block in blocks {
        match block {
            MarkdownBlock::Heading { text, .. } => result.push(inline(text, true)),
            MarkdownBlock::Paragraph(text) => result.push(inline(text, single_line)),
            MarkdownBlock::List(list) => result.push(join_with(&list_items(list), SEPARATOR)),
            MarkdownBlock::Code(code) => result.push(code_summary(code)),
            MarkdownBlock::Table(table) => {
                result.push(join_with(&table_rows(table), SEPARATOR));
            }
            MarkdownBlock::Quote(inner) => result.extend(segments(inner, single_line)),
            MarkdownBlock::ThematicBreak => continue,
        }
    }
    result.retain(|segment| !is_blank(segment));
    result
}

fn table_rows(table: &MarkdownTable) -> Vec<String> {
    let mut rows: Vec<Vec<String>> = vec![table.header.clone()];
    rows.extend(table.rows.iter().cloned());
    rows.iter()
        .map(|row| {
            let cells: Vec<String> = row
                .iter()
                .filter(|cell| !is_blank(cell))
                .map(|cell| inline(cell, true))
                .collect();
            join_with(&cells, ", ")
        })
        .collect()
}

fn list_items(list: &MarkdownList) -> Vec<String> {
    let mut items = Vec::new();
    for (index, item) in list.items.iter().enumerate() {
        // An item's own paragraph and its nested list flow into one run:
        // "Parent · child · child".
        let mut text = join(&segments(&item.blocks, true), true);
        if let Some(box_) = item.checkbox {
            let mark = match box_ {
                Checkbox::Checked => "☑ ",
                Checkbox::Unchecked => "☐ ",
            };
            text = format!("{mark}{text}");
        }
        // Numbers carry meaning (steps, ranking); bullets do not.
        if list.is_ordered {
            text = format!("{}. {text}", list.start + index as i64);
        }
        if !is_blank(&text) {
            items.push(text);
        }
    }
    items
}

/// First non-empty line of the code, plus an ellipsis when more follows — enough to
/// recognise `npm test` without spilling a whole script into the preview.
fn code_summary(code: &MarkdownCodeBlock) -> String {
    let lines: Vec<&str> = code
        .code
        .split('\n')
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    match lines.len() {
        0 => String::new(),
        1 => lines[0].to_owned(),
        _ => format!("{} …", lines[0]),
    }
}

// MARK: - Joining

fn join(segments: &[String], single_line: bool) -> String {
    if !single_line {
        return join_with(segments, "\n");
    }
    let mut result = String::new();
    for segment in segments {
        if !result.is_empty() {
            // "Changes:" already introduces what follows; a dot after it reads as noise.
            let last = result.chars().last();
            result.push_str(if matches!(last, Some(':') | Some('：')) {
                " "
            } else {
                SEPARATOR
            });
        }
        result.push_str(segment);
    }
    result
}

fn join_with(segments: &[String], separator: &str) -> String {
    segments
        .iter()
        .filter(|segment| !segment.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(separator)
}

/// The block's words with their delimiters gone. Links keep their text.
fn inline(text: &str, single_line: bool) -> String {
    let source = if single_line {
        text.replace('\n', " ")
    } else {
        text.to_owned()
    };
    let mut out = String::new();
    flatten(&markdown::parse_inline(&source), &mut out);
    out
}

fn flatten(spans: &[markdown::MarkdownInline], out: &mut String) {
    for span in spans {
        match span {
            markdown::MarkdownInline::Text { text }
            | markdown::MarkdownInline::Code { text }
            | markdown::MarkdownInline::Link { text, .. } => out.push_str(text),
            markdown::MarkdownInline::Strong { spans }
            | markdown::MarkdownInline::Emphasis { spans } => flatten(spans, out),
        }
    }
}

fn is_blank(text: &str) -> bool {
    text.chars().all(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every expectation below is copied out of
    /// `macos/Tests/MorrowCoreTests/MarkdownPreviewTextTests.swift`, so a change on
    /// either side shows up as a failing test here rather than a card that quietly
    /// reads differently.
    const REPLY: &str = "\
## Summary
Fixed the **login** bug in `auth.ts`.

Changes:
- [x] token refresh checks expiry
- [ ] manual QA

| File | Lines |
|------|------:|
| auth.ts | 12 |

```bash
npm test
npm run lint
```
";

    #[test]
    fn a_single_line_preview_carries_no_markdown_syntax() {
        assert_eq!(
            plain(REPLY, true),
            "Summary · Fixed the login bug in auth.ts. · Changes: ☑ token refresh checks expiry · ☐ manual QA · File, Lines · auth.ts, 12 · npm test …"
        );
    }

    #[test]
    fn a_multi_line_preview_keeps_one_line_per_block() {
        assert_eq!(
            plain(REPLY, false),
            "Summary\nFixed the login bug in auth.ts.\nChanges:\n☑ token refresh checks expiry · ☐ manual QA\nFile, Lines · auth.ts, 12\nnpm test …"
        );
    }

    #[test]
    fn a_multi_line_preview_keeps_paragraph_breaks() {
        assert_eq!(plain("one\ntwo", false), "one\ntwo");
        assert_eq!(plain("one\ntwo", true), "one two");
    }

    #[test]
    fn ordered_lists_keep_their_numbers_and_nested_items_flow_inline() {
        assert_eq!(
            plain("Steps:\n\n3. build\n   - debug\n4. test", true),
            "Steps: 3. build · debug · 4. test"
        );
    }

    #[test]
    fn quotes_and_breaks_lose_their_markers() {
        assert_eq!(
            plain("> **Note** restart\n\n---\n\ndone", true),
            "Note restart · done"
        );
    }

    #[test]
    fn a_single_line_code_block_has_no_ellipsis() {
        assert_eq!(plain("Run:\n```\n  make  \n```", true), "Run: make");
    }

    #[test]
    fn empty_table_cells_are_skipped() {
        assert_eq!(
            plain("| a | | c |\n|---|---|---|\n| 1 | 2 | |", true),
            "a, c · 1, 2"
        );
    }

    #[test]
    fn streaming_fragments_preview_cleanly() {
        assert_eq!(plain("| a | b |\n|--", true), "a, b");
        assert_eq!(plain("Here:\n```py", true), "Here:");
        assert_eq!(plain("##", true), "");
        assert_eq!(plain("- ", true), "");
        assert_eq!(plain("", false), "");
    }

    #[test]
    fn inline_links_survive_as_their_text() {
        assert_eq!(plain("- see [docs](https://example.dev)", true), "see docs");
    }

    #[test]
    fn a_short_reply_is_its_own_preview_source() {
        assert_eq!(preview_source("one\ntwo"), "one\ntwo");
        assert_eq!(preview_source(""), "");
    }

    #[test]
    fn long_code_does_not_count_against_the_prose_budget() {
        // Verbatim from Swift: 1 500 code lines push the reply well past the prose
        // budget, and the paragraph after the fence must still arrive.
        let code: Vec<String> = (1..=1_500)
            .map(|line| format!("line {line} of a long sample"))
            .collect();
        let reply = format!(
            "Here is the fix:\n```swift\n{}\n```\nThen run the tests.",
            code.join("\n")
        );
        assert!(reply.len() > SOURCE_BUDGET, "{} bytes", reply.len());
        assert_eq!(
            plain(&preview_source(&reply), true),
            "Here is the fix: line 1 of a long sample … · Then run the tests."
        );
    }

    #[test]
    fn an_unclosed_fence_still_ends() {
        let unclosed = format!("Output:\n```\n{}", "x\n".repeat(100_000));
        let head = preview_source(&unclosed);
        assert!(head.len() <= SOURCE_HARD_LIMIT, "{} bytes", head.len());
        assert_eq!(plain(&head, true), "Output: x …");
    }

    #[test]
    fn one_huge_line_ends_at_the_prose_budget_and_cutting_twice_changes_nothing() {
        let one_line = format!("Result: {}", "word ".repeat(50_000));
        let cut = preview_source(&one_line);
        assert!(one_line.starts_with(&cut));
        assert_eq!(cut.len(), SOURCE_BUDGET);
        assert_eq!(preview_source(&cut), cut);
    }

    #[test]
    fn a_huge_reply_keeps_the_head_the_whole_reply_would_have() {
        // 139 KB Codex output at ~120 ms, redone for every streamed chunk: the cut
        // exists to keep the visible head identical while dropping the tail.
        let huge = huge_reply(300_000);
        let head: String = plain(&huge, true).chars().take(2_000).collect();
        let cut: String = plain(&preview_source(&huge), true)
            .chars()
            .take(2_000)
            .collect();
        assert_eq!(cut, head, "the visible head is unchanged");
        assert!(preview_source(&huge).len() < 16 * 1024);
    }

    fn huge_reply(bytes: usize) -> String {
        let mut parts: Vec<String> = Vec::new();
        let mut size = 0usize;
        let mut index = 0usize;
        while size < bytes {
            index += 1;
            let part = match index % 4 {
                0 => format!(
                    "## Section {index}\nSome **bold** text with `code` and a [link](https://example.dev/{index})."
                ),
                1 => format!("- item {index}\n- item with *emphasis*\n- [x] done"),
                2 => format!("```swift\nlet x{index} = 1\nprint(x{index})\n```"),
                _ => format!("| a | b |\n|---|---|\n| {index} | **{index}** |"),
            };
            size += part.len() + 2;
            parts.push(part);
        }
        parts.join("\n\n")
    }

    #[test]
    fn cutting_never_splits_a_character() {
        let source = format!("{}\nmore", "é".repeat(SOURCE_BUDGET));
        let head = preview_source(&source);
        assert!(head.is_char_boundary(head.len()));
        assert!(String::from_utf8(head.into_bytes()).is_ok());
    }
}
