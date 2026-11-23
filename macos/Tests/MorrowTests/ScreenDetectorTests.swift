import AppKit
import XCTest
@testable import Morrow
import MorrowCore

final class ScreenDetectorTests: XCTestCase {
    func testAutoPreferredIndexPrefersBuiltInNotchScreenOverExternalDisplays() {
        let candidates = [
            ScreenDetector.Candidate(
                frame: NSRect(x: 0, y: 0, width: 1512, height: 982),
                hasNotch: true,
                isMain: true
            ),
            ScreenDetector.Candidate(
                frame: NSRect(x: 1512, y: 0, width: 1920, height: 1080),
                hasNotch: false,
                isMain: false
            )
        ]

        // Even when an active app window is on the external display (monitor 2),
        // the island stays anchored to the built-in notch display (index 0) like BoringNotch.
        let index = ScreenDetector.autoPreferredIndex(
            candidates: candidates,
            activeWindowBounds: NSRect(x: 1800, y: 100, width: 1000, height: 800)
        )

        XCTAssertEqual(index, 0)
    }

    func testAutoPreferredIndexPrefersBuiltInScreenWhenNotchScreenExists() {
        let candidates = [
            ScreenDetector.Candidate(
                frame: NSRect(x: 0, y: 0, width: 1512, height: 982),
                hasNotch: true,
                isMain: false
            ),
            ScreenDetector.Candidate(
                frame: NSRect(x: 1512, y: 0, width: 1920, height: 1080),
                hasNotch: false,
                isMain: true
            )
        ]

        let index = ScreenDetector.autoPreferredIndex(
            candidates: candidates,
            activeWindowBounds: nil
        )

        XCTAssertEqual(index, 0)
    }

    func testAutoPreferredIndexFallsBackToMainScreenWhenNoBuiltInScreenExists() {
        let candidates = [
            ScreenDetector.Candidate(
                frame: NSRect(x: 0, y: 0, width: 1920, height: 1080),
                hasNotch: false,
                isMain: false
            ),
            ScreenDetector.Candidate(
                frame: NSRect(x: 1920, y: 0, width: 1920, height: 1080),
                hasNotch: false,
                isMain: true
            )
        ]

        let index = ScreenDetector.autoPreferredIndex(
            candidates: candidates,
            activeWindowBounds: nil
        )

        XCTAssertEqual(index, 1)
    }

    func testDescriptorsAndIdentifiersForConnectedScreens() {
        let screens = NSScreen.screens
        let descriptors = ScreenDetector.descriptors(from: screens)
        XCTAssertEqual(descriptors.count, screens.count)

        let ids = descriptors.map(\.id)
        XCTAssertEqual(Set(ids).count, ids.count, "display identifiers must be unique")
        for (index, descriptor) in descriptors.enumerated() {
            XCTAssertFalse(descriptor.id.isEmpty)
            XCTAssertEqual(descriptor.index, index)
            XCTAssertEqual(descriptor.isPrimary, index == 0)
        }

        if let firstScreen = screens.first {
            let identifier = ScreenDetector.displayIdentifier(for: firstScreen)
            XCTAssertFalse(identifier.isEmpty)
            if ScreenDetector.isBuiltinScreen(firstScreen) {
                XCTAssertEqual(identifier, "builtin")
            }
        }
    }

    @MainActor
    func testScreenMonitorInitializesWithInjectedDisplays() {
        let sample = [
            DisplayDescriptor(
                id: "builtin",
                index: 0,
                name: "Built-in",
                width: 1512,
                height: 982,
                hasNotch: true,
                isBuiltin: true,
                isPrimary: true
            )
        ]
        let monitor = ScreenMonitor(displays: sample, observeSystemChanges: false)
        XCTAssertEqual(monitor.displays, sample)
    }

    func testStableIdentifierAvoidsBuiltinCollisionAndVolatileDisplayIDOnReplug() {
        let builtinID = ScreenDetector.stableIdentifier(
            isBuiltin: true,
            uuidString: "UUID-BUILTIN",
            vendor: 1552,
            model: 41000,
            serial: 0,
            unit: 0,
            localizedName: "Built-in Retina Display",
            width: 1512,
            height: 982
        )
        let externalNotchID = ScreenDetector.stableIdentifier(
            isBuiltin: false,
            uuidString: nil,
            vendor: 4268,
            model: 16896,
            serial: 987654,
            unit: 2,
            localizedName: "External Notch Monitor",
            width: 3840,
            height: 2160
        )
        XCTAssertEqual(builtinID, "builtin")
        XCTAssertNotEqual(externalNotchID, "builtin")
        XCTAssertEqual(externalNotchID, "hw_4268_16896_987654")

        // Even without UUID or serial, fallback uses stable vendor/model/unit instead of runtime CGDirectDisplayID.
        let serialLessID = ScreenDetector.stableIdentifier(
            isBuiltin: false,
            uuidString: nil,
            vendor: 4268,
            model: 16897,
            serial: 0,
            unit: 3,
            localizedName: "Studio Display",
            width: 2560,
            height: 1440
        )
        XCTAssertFalse(serialLessID.hasPrefix("display_"))
        XCTAssertEqual(serialLessID, "hw_4268_16897_u3_Studio Display_2560x1440")
    }
}
