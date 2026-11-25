import AppKit
import SwiftUI
import XCTest
@testable import Morrow
import MorrowCore

/// #44: with AI Agents switched off the idle notch must still carry the
/// Morrow mascot, and agent sessions must no longer drive the collapsed bar.
@MainActor
final class IdleMascotTests: XCTestCase {
    private let suiteName = "IdleMascotTests"

    override func setUp() async throws {
        try await super.setUp()
        _ = NSApplication.shared
        MascotAnimationGate.shared.setPanelVisible(false)
        let suite = UserDefaults(suiteName: suiteName)
        suite?.removePersistentDomain(forName: suiteName)
    }

    override func tearDown() async throws {
        UserDefaults(suiteName: suiteName)?.removePersistentDomain(forName: suiteName)
        MascotAnimationGate.shared.setPanelVisible(true)
        try await super.tearDown()
    }

    func testAgentsDisabledResolvesToMorrowMascot() {
        XCTAssertEqual(
            IdleMascotSource.resolved(agentsEnabled: false, defaultSource: "codex"),
            IdleMascotSource.morrow
        )
    }

    func testAgentsEnabledKeepsConfiguredIdleMascot() {
        XCTAssertEqual(
            IdleMascotSource.resolved(agentsEnabled: true, defaultSource: "codex"),
            "codex"
        )
        XCTAssertEqual(
            IdleMascotSource.resolved(agentsEnabled: true, defaultSource: "  "),
            SettingsDefaults.defaultSource
        )
    }

    func testMorrowMascotDrawsVisiblePixels() throws {
        let mascot = MascotView(source: IdleMascotSource.morrow, status: .idle, size: 27)
            .environment(\.mascotAnimationsActive, false)
            .environment(\.mascotStaticTime, 1.0)
        let image = try XCTUnwrap(OffscreenRender.rasterize(mascot, scale: 2))
        XCTAssertGreaterThan(noriCoverage(image).opaque, 100, "Nori must be drawn at 27pt")
    }

    /// Nori is the #44 choice: its dark body and amber eyes must still read at
    /// the 20pt size used by the collapsed pill, without the SVG preview rect.
    func testNoriDrawsVisiblePixelsAt20Points() throws {
        let mascot = MascotView(source: IdleMascotSource.morrow, status: .idle, size: 20)
            .environment(\.mascotAnimationsActive, false)
            .environment(\.mascotStaticTime, 1.0)
        let image = try XCTUnwrap(OffscreenRender.rasterize(mascot, scale: 4))
        let coverage = noriCoverage(image)
        XCTAssertGreaterThan(coverage.opaque, 500, "Nori must be visible at 20pt")
        XCTAssertGreaterThan(coverage.body, 300, "Nori's navy/indigo body must survive at 20pt")
        XCTAssertGreaterThan(coverage.eyes, 20, "Nori's amber eyes must stay visible at 20pt")
        XCTAssertTrue(coverage.cornerAlpha.allSatisfy { $0 <= 16 },
                      "the SVG preview's dark backing rect must not be drawn")
    }

    /// Default behaviour stays intact: agents on keeps the configured idle mascot.
    func testAgentsEnabledUsesConfiguredIdleMascot() throws {
        let suite = try XCTUnwrap(UserDefaults(suiteName: suiteName))
        suite.set(true, forKey: SettingsKey.agentsEnabled)
        suite.set(false, forKey: SettingsKey.hideWhenNoSession)

        let image = try renderPanel(state: AppState(), suite: suite)
        XCTAssertGreaterThan(brightPixels(image), 100)
        try save(image, named: "before-agents-on")
    }

    /// The regression: `hideWhenNoSession` is on and a Claude session is still
    /// tracked, but the agents toggle is off. The notch must show the Morrow
    /// mascot, not an empty bar and not the Claude icon.
    func testAgentsDisabledShowsIdleMascotWithSessionAndHideEnabled() throws {
        let suite = try XCTUnwrap(UserDefaults(suiteName: suiteName))
        suite.set(false, forKey: SettingsKey.agentsEnabled)
        suite.set(true, forKey: SettingsKey.hideWhenNoSession)

        var session = SessionSnapshot()
        session.source = "claude"
        session.status = .running
        session.lastActivity = Date()
        let state = AppState()
        state.sessions["s1"] = session
        state.activeSessionId = "s1"
        state.refreshDerivedState()

        let withSession = try renderPanel(state: state, suite: suite)
        XCTAssertGreaterThan(brightPixels(withSession), 100,
                             "idle notch must keep the Morrow mascot when agents are off")
        try save(withSession, named: "after-agents-off")

        let empty = try renderPanel(state: AppState(), suite: suite)
        XCTAssertEqual(brightPixels(withSession), brightPixels(empty),
                       "hidden agent sessions must not change the idle notch")
    }

    private func save(_ image: CGImage, named name: String) throws {
        guard let dir = ProcessInfo.processInfo.environment["MORROW_IDLE_SCREENSHOT_DIR"], !dir.isEmpty else { return }
        let folder = URL(fileURLWithPath: dir, isDirectory: true)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        try OffscreenRender.writePNG(image, to: folder.appendingPathComponent("\(name).png").path)
    }

    private func renderPanel(state: AppState, suite: UserDefaults) throws -> CGImage {
        let panel = NotchPanelView(
            appState: state,
            hasNotch: true,
            notchHeight: 32,
            notchW: 185,
            screenWidth: 1512
        )
        .environment(\.mascotAnimationsActive, false)
        .environment(\.mascotStaticTime, 5.2)
        .defaultAppStorage(suite)
        return try OffscreenRender.hosted(panel, size: CGSize(width: 400, height: 64), scale: 2)
    }

    /// Opaque coverage, navy body, amber eye pixels and corner alphas of Nori.
    private func noriCoverage(_ image: CGImage) -> (opaque: Int, body: Int, eyes: Int, cornerAlpha: [Int]) {
        let width = image.width, height = image.height
        guard let context = CGContext(
            data: nil,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: width * 4,
            space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ), let data = context.data else { return (0, 0, 0, []) }
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
        let pixels = data.assumingMemoryBound(to: UInt8.self)
        let unit = CGFloat(width) / 64
        var opaque = 0, body = 0, eyes = 0
        for y in 0..<height {
            let yUnit = CGFloat(y) / unit
            for x in 0..<width {
                let offset = (y * width + x) * 4
                let alpha = Int(pixels[offset + 3])
                guard alpha > 16 else { continue }
                opaque += 1
                let red = Int(pixels[offset])
                let green = Int(pixels[offset + 1])
                let blue = Int(pixels[offset + 2])
                if (30...110).contains(red), (30...120).contains(green),
                   (60...170).contains(blue), blue > red + 10 {
                    body += 1
                }
                if red > 180, green > 120, blue < 160, (20...30).contains(yUnit) {
                    eyes += 1
                }
            }
        }
        let corners = [(0, 0), (width - 1, 0), (0, height - 1), (width - 1, height - 1)]
        let cornerAlpha = corners.map { x, y in Int(pixels[(y * width + x) * 4 + 3]) }
        return (opaque, body, eyes, cornerAlpha)
    }

    /// The notch shell is black; Nori's amber accents read even at idle opacity.
    private func brightPixels(_ image: CGImage) -> Int {
        let width = image.width, height = image.height
        guard let context = CGContext(
            data: nil,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: width * 4,
            space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ), let data = context.data else { return 0 }
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
        let pixels = data.assumingMemoryBound(to: UInt8.self)
        var count = 0
        for offset in stride(from: 0, to: width * height * 4, by: 4) {
            let alpha = Int(pixels[offset + 3])
            let red = Int(pixels[offset])
            let green = Int(pixels[offset + 1])
            let blue = Int(pixels[offset + 2])
            if alpha > 16, red > 90, green > 90, blue > 90 { count += 1 }
        }
        return count
    }
}
