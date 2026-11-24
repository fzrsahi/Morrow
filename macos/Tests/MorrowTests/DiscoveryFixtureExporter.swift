import Foundation
import XCTest

import MorrowCore
@testable import Morrow

/// `fixtures/discovery/` is what the discovery readers do with a store the
/// platform handed them: the pure half of `ClaudeDiscovery`, `CursorDiscovery`
/// and `GeminiDiscovery`, plus `ProjectDirEncoding` (#245).
///
/// `CODEISLAND_DISCOVERY_EXPORT_JSON=<fixtures dir>` rewrites the file instead
/// of checking it. Every expected value is produced by calling the app's own
/// code, so the file records what Swift does, not what a test hopes.
final class DiscoveryFixtureExporter: XCTestCase {
    /// A Cursor agent transcript: four turns, so the "last three" rule shows.
    static let cursorTranscript = """
    {"role":"user","message":{"content":[{"type":"text","text":"Add a filter bar"}]}}
    {"role":"assistant","message":{"content":[{"type":"text","text":"Added the bar"}]}}
    {"role":"user","message":{"content":"Narrow it to open issues"}}
    {"role":"assistant","message":{"content":[{"type":"text","text":"Filtered"}]}}
    """

    /// A Gemini session file: `sessionId`, `model`, and a `messages` array whose
    /// entries carry a `type` and a `content`.
    static let geminiSession = """
    {
      "sessionId": "gem-1",
      "model": "gemini-2.5-pro",
      "messages": [
        {"type": "user", "content": "Where is the router?"},
        {"type": "gemini", "content": "In src/router.ts"},
        {"type": "user", "content": [{"type": "text", "text": "Open it"}]},
        {"type": "gemini", "content": "Opened"}
      ]
    }
    """

    /// A [CC] transcript whose first assistant row has an empty model: the
    /// reader skips it and finds nothing.
    static let claudeTranscriptEmpty = """
    {"type":"user","message":{"content":"Hi"}}
    {"type":"assistant","message":{"model":"","content":[{"type":"text","text":"Hello"}]}}
    {"type":"assistant","message":{"model":"","content":[{"type":"text","text":"Later"}]}}
    """

    /// The same shape with a model on the second assistant row, so the "first
    /// row that has one" rule is visible.
    static let claudeTranscriptNamed = """
    {"type":"user","message":{"content":"Hi"}}
    {"type":"assistant","message":{"model":"","content":[{"type":"text","text":"Hello"}]}}
    {"type":"assistant","message":{"model":"model-x","content":[{"type":"text","text":"Later"}]}}
    """

    /// Paths whose encoding the project-directory name depends on.
    static let paths = [
        "/Users/dev/projects/morrow",
        "/Users/dev/My Project",
        "/Users/dev/naïve",
        "C:\\Users\\dev\\morrow",
        "/",
    ]

    static func document() throws -> [String: Any] {
        [
            "cursor": [
                "transcript": cursorTranscript,
                "expected": [
                    "messages": messages(CursorDiscovery.recentMessages(fromTranscript: cursorTranscript)),
                ],
            ],
            "gemini": [
                "session": geminiSession,
                "expected": [
                    "session_id": text(GeminiDiscovery.sessionId(fromTranscript: geminiSession)),
                    "model": text(GeminiDiscovery.model(fromTranscript: geminiSession)),
                    "messages": messages(GeminiDiscovery.recentMessages(fromTranscript: geminiSession)),
                ],
            ],
            "claude": [
                "empty_transcript": claudeTranscriptEmpty,
                "empty_expected": text(ClaudeDiscovery.model(fromTranscript: claudeTranscriptEmpty)),
                "named_transcript": claudeTranscriptNamed,
                "named_expected": text(ClaudeDiscovery.model(fromTranscript: claudeTranscriptNamed)),
            ],
            "project_dir": paths.map { path in
                [
                    "path": path,
                    "encoded": path.claudeProjectDirEncoded(),
                    "app_encoded": path.appProjectDirEncoded(),
                ]
            },
        ]
    }

    /// A `String?` as JSON wants it. Typed on purpose: `Any?` boxes the optional
    /// and a nil comes out as `""` instead of `null`.
    private static func text(_ value: String?) -> Any {
        value.map { $0 as Any } ?? NSNull()
    }

    private static func messages(_ list: [ChatMessage]) -> [[String: Any]] {
        list.map { ["is_user": $0.isUser, "text": $0.text] }
    }

    func testExportedDiscoveryFixturesMatchSwift() throws {
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
            .appendingPathComponent("shared/contract/fixtures/discovery/readers.json")

        guard let exportDir = ProcessInfo.processInfo.environment["CODEISLAND_DISCOVERY_EXPORT_JSON"]
        else {
            // The Rust side reads the fixture; here it only has to be current.
            let onDisk = try Data(contentsOf: path)
            XCTAssertEqual(
                String(decoding: onDisk, as: UTF8.self),
                String(decoding: out, as: UTF8.self),
                "fixtures/discovery/readers.json is stale: re-export with CODEISLAND_DISCOVERY_EXPORT_JSON"
            )
            return
        }

        let dir = URL(fileURLWithPath: exportDir, isDirectory: true)
            .appendingPathComponent("discovery", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        try out.write(to: dir.appendingPathComponent("readers.json"))
        throw XCTSkip("exported the discovery reader fixtures to \(exportDir)/discovery")
    }
}
