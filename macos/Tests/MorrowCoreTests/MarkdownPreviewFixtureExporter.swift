import XCTest
@testable import MorrowCore

/// `fixtures/core/markdown_preview.json` is what Swift's block reducer does with each
/// sample, so the Rust port is compared against values Swift produced rather than
/// against values someone retyped from a Swift test.
///
/// `CODEISLAND_MARKDOWN_EXPORT_JSON=<fixtures dir>` rewrites the file; without it this
/// test only checks the file on disk is still current.
final class MarkdownPreviewFixtureExporter: XCTestCase {
    private static let reply = """
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
    """

    private static func samples() -> [(String, String)] {
        let code: [String] = (1...200).map { "line \($0) of a long sample" }
        return [
            ("reply", reply),
            ("paragraph_breaks", "one\ntwo"),
            ("ordered_nested", "Steps:\n\n3. build\n   - debug\n4. test"),
            ("quote_and_break", "> **Note** restart\n\n---\n\ndone"),
            ("single_line_code", "Run:\n```\n  make  \n```"),
            ("empty_cells", "| a | | c |\n|---|---|---|\n| 1 | 2 | |"),
            ("streaming_table", "| a | b |\n|--"),
            ("streaming_fence", "Here:\n```py"),
            ("lone_hash", "##"),
            ("lone_bullet", "- "),
            ("empty", ""),
            ("inline_link", "- see [docs](https://example.dev)"),
            ("headings_and_code", "# Title\nuse `make`\n```\nnpm test\n```"),
            ("long_fence_then_prose", "Here is the fix:\n```swift\n\(code.joined(separator: "\n"))\n```\nThen run the tests."),
            ("unclosed_fence", "Output:\n```\n" + String(repeating: "x\n", count: 500)),
            ("one_huge_line", "Result: " + String(repeating: "word ", count: 5_000)),
        ]
    }

    private static func document() throws -> [String: Any] {
        let cases: [[String: Any]] = samples().map { name, text in
            [
                "name": name,
                "input": text,
                "single_line": String(MarkdownPreviewText.plain(text, singleLine: true)),
                "multi_line": String(MarkdownPreviewText.plain(text, singleLine: false)),
                "preview_source": MarkdownPreviewText.previewSource(text),
            ]
        }
        return [
            "source_budget": MarkdownPreviewText.sourceBudget,
            "source_hard_limit": MarkdownPreviewText.sourceHardLimit,
            "separator": MarkdownPreviewText.separator,
            "cases": cases,
        ]
    }

    func testExportedMarkdownPreviewFixturesMatchSwift() throws {
        var out = try JSONSerialization.data(
            withJSONObject: try Self.document(),
            options: [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        )
        out.append(0x0A)

        let path = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("shared/contract/fixtures/core/markdown_preview.json")

        guard let exportDir = ProcessInfo.processInfo.environment["CODEISLAND_MARKDOWN_EXPORT_JSON"]
        else {
            let onDisk = try Data(contentsOf: path)
            XCTAssertEqual(
                String(decoding: onDisk, as: UTF8.self),
                String(decoding: out, as: UTF8.self),
                "fixtures/core/markdown_preview.json is stale: re-export with CODEISLAND_MARKDOWN_EXPORT_JSON"
            )
            return
        }

        let dir = URL(fileURLWithPath: exportDir, isDirectory: true)
            .appendingPathComponent("core", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        try out.write(to: dir.appendingPathComponent("markdown_preview.json"))
        throw XCTSkip("exported the markdown preview fixtures to \(exportDir)/core")
    }
}
