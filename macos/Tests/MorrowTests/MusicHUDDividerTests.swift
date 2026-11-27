import AppKit
import SwiftUI
import XCTest
@testable import Morrow

@MainActor
final class MusicHUDDividerTests: XCTestCase {
    private func render(_ view: some View) throws -> CGImage {
        let host = view
            .frame(width: 680)
            .background(Color.black)
        return try XCTUnwrap(OffscreenRender.rasterize(host, scale: 1))
    }

    private func pixelBrightness(in image: CGImage, x: Int, y: Int) throws -> Int {
        XCTAssertGreaterThanOrEqual(x, 0)
        XCTAssertLessThan(x, image.width)
        XCTAssertGreaterThanOrEqual(y, 0)
        XCTAssertLessThan(y, image.height)

        let data = try XCTUnwrap(image.dataProvider?.data)
        let ptr = try XCTUnwrap(CFDataGetBytePtr(data))
        let bytesPerPixel = image.bitsPerPixel / 8
        let offset = y * image.bytesPerRow + x * bytesPerPixel
        let r = Int(ptr[offset])
        let g = Int(ptr[offset + 1])
        let b = Int(ptr[offset + 2])
        return max(r, max(g, b))
    }

    private func findDividerColumn(in image: CGImage, xRange: ClosedRange<Int>, sampleY: Int) throws -> Int {
        var bestX = xRange.lowerBound
        var bestBrightness = -1
        for x in xRange {
            let value = try pixelBrightness(in: image, x: x, y: sampleY)
            if value > bestBrightness {
                bestBrightness = value
                bestX = x
            }
        }
        return bestX
    }

    func testModernCleanMusicHUDRendersFadedVerticalDivider() throws {
        let nowPlaying = NowPlayingManager()
        let appState = AppState()
        let image = try render(ModernCleanMusicHUDContentView(nowPlaying: nowPlaying, appState: appState))

        // .frame(height: 114) + .padding(.top, 4) + .padding(.bottom, 16) => HStack center at y = 4 + 57 = 61.
        let centerY = 61
        // .padding(.horizontal, 18) + left block 255 + HStack spacing 14 = 287.
        let dividerX = try findDividerColumn(in: image, xRange: 285...289, sampleY: centerY)

        let centerBrightness = try pixelBrightness(in: image, x: dividerX, y: centerY)
        let leftGapBrightness = try pixelBrightness(in: image, x: dividerX - 4, y: centerY)
        let rightGapBrightness = try pixelBrightness(in: image, x: dividerX + 4, y: centerY)

        XCTAssertGreaterThanOrEqual(centerBrightness, 20, "Modern Clean vertical divider missing at x=\(dividerX)")
        XCTAssertLessThanOrEqual(leftGapBrightness, 2, "Expected blank gap left of divider")
        XCTAssertLessThanOrEqual(rightGapBrightness, 2, "Expected blank gap right of divider")

        // Verify top and bottom gradient fade ends near clear.
        let topEdgeBrightness = try pixelBrightness(in: image, x: dividerX, y: 2)
        let bottomEdgeBrightness = try pixelBrightness(in: image, x: dividerX, y: image.height - 3)
        XCTAssertLessThanOrEqual(topEdgeBrightness, 4, "Expected top edge of Modern Clean divider to fade out")
        XCTAssertLessThanOrEqual(bottomEdgeBrightness, 4, "Expected bottom edge of Modern Clean divider to fade out")
    }

    func testRetroCyberMusicHUDRendersVerticalDivider() throws {
        let nowPlaying = NowPlayingManager()
        let appState = AppState()
        let image = try render(RetroCyberMusicHUDContentView(nowPlaying: nowPlaying, appState: appState))

        let centerY = 61
        // .padding(.horizontal, 18) + left block 255 + HStack spacing 16 = 289.
        let dividerX = try findDividerColumn(in: image, xRange: 287...291, sampleY: centerY)

        let leftGapBrightness = try pixelBrightness(in: image, x: dividerX - 4, y: centerY)
        let rightGapBrightness = try pixelBrightness(in: image, x: dividerX + 4, y: centerY)
        XCTAssertLessThanOrEqual(leftGapBrightness, 2, "Expected blank gap left of divider")
        XCTAssertLessThanOrEqual(rightGapBrightness, 2, "Expected blank gap right of divider")

        for y in stride(from: centerY - 30, through: centerY + 30, by: 10) {
            let brightness = try pixelBrightness(in: image, x: dividerX, y: y)
            XCTAssertGreaterThanOrEqual(brightness, 20, "Retro Cyber vertical divider missing at x=\(dividerX), y=\(y)")
        }
    }
}
