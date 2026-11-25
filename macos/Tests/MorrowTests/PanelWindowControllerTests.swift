import AppKit
import XCTest
@testable import Morrow
import MorrowCore

final class PanelWindowControllerTests: XCTestCase {
    func testScreenHopMotionUsesMoreVisibleTiming() {
        let motion = PanelWindowController.screenHopMotion()

        XCTAssertEqual(motion.outgoingOffset, 18)
        XCTAssertEqual(motion.incomingOffset, 30)
        XCTAssertEqual(motion.fadeOutDuration, 0.14, accuracy: 0.001)
        XCTAssertEqual(motion.incomingPauseDuration, 0.06, accuracy: 0.001)
        XCTAssertEqual(motion.fadeInDuration, 0.34, accuracy: 0.001)
    }

    func testScreenHopFramesRetractOldFrameAndDropIntoNewFrame() {
        let oldFrame = NSRect(x: 100, y: 820, width: 420, height: 180)
        let newFrame = NSRect(x: 1800, y: 900, width: 420, height: 180)

        let frames = PanelWindowController.screenHopFrames(
            oldFrame: oldFrame,
            newFrame: newFrame
        )

        XCTAssertEqual(frames.outgoing.origin.x, oldFrame.origin.x)
        XCTAssertEqual(frames.outgoing.origin.y, oldFrame.origin.y + 18)
        XCTAssertEqual(frames.outgoing.size, oldFrame.size)

        XCTAssertEqual(frames.incoming.origin.x, newFrame.origin.x)
        XCTAssertEqual(frames.incoming.origin.y, newFrame.origin.y + 30)
        XCTAssertEqual(frames.incoming.size, newFrame.size)
    }

    func testPerDisplayAnchoredYIndependentlyPositionsNotchAndDynamicIsland() {
        let display1NotchY = PanelWindowLayout.anchoredY(
            screenMaxY: 982,
            panelHeight: 32,
            displayMode: .notch,
            isFullscreenMode: false,
            topOffset: 10
        )
        let display2IslandY = PanelWindowLayout.anchoredY(
            screenMaxY: 1080,
            panelHeight: 25,
            displayMode: .dynamicIsland,
            isFullscreenMode: false,
            topOffset: 24
        )
        let display1NotchFullscreenY = PanelWindowLayout.anchoredY(
            screenMaxY: 982,
            panelHeight: 32,
            displayMode: .notch,
            isFullscreenMode: true,
            topOffset: 10
        )
        XCTAssertEqual(display1NotchY, 950)
        XCTAssertEqual(display1NotchFullscreenY, 950)
        XCTAssertEqual(display2IslandY, 1031)
    }

    func testPanelSlotPlannerHandlesZeroOneAndTwoDisplays() {
        let d1 = DisplayDescriptor(id: "builtin", index: 0, name: "Built-in", width: 1512, height: 982, hasNotch: true, isBuiltin: true, isPrimary: true)
        let d2 = DisplayDescriptor(id: "EXT-2", index: 1, name: "External", width: 1920, height: 1080, hasNotch: false, isBuiltin: false, isPrimary: false)

        let configs: [String: DisplayConfiguration] = [
            "builtin": DisplayConfiguration(enabled: true, displayMode: .notch),
            "EXT-2": DisplayConfiguration(enabled: true, displayMode: .dynamicIsland),
        ]

        let both = PanelSlotPlanner.plan(islandEnabled: true, displays: [d1, d2], configurations: configs)
        XCTAssertEqual(both.activeDisplayIDs, ["builtin", "EXT-2"])
        XCTAssertEqual(both.primaryCoordinatorID, "builtin")

        let onlySecondConfigs: [String: DisplayConfiguration] = [
            "builtin": DisplayConfiguration(enabled: false, displayMode: .notch),
            "EXT-2": DisplayConfiguration(enabled: true, displayMode: .dynamicIsland),
        ]
        let onlySecond = PanelSlotPlanner.plan(islandEnabled: true, displays: [d1, d2], configurations: onlySecondConfigs)
        XCTAssertEqual(onlySecond.activeDisplayIDs, ["EXT-2"])
        XCTAssertEqual(onlySecond.primaryCoordinatorID, "EXT-2")

        let masterOff = PanelSlotPlanner.plan(islandEnabled: false, displays: [d1, d2], configurations: configs)
        XCTAssertEqual(masterOff.activeDisplayIDs, [])
        XCTAssertNil(masterOff.primaryCoordinatorID)

        let noDisplays = PanelSlotPlanner.plan(islandEnabled: true, displays: [], configurations: configs)
        XCTAssertEqual(noDisplays.activeDisplayIDs, [])
        XCTAssertNil(noDisplays.primaryCoordinatorID)
    }

    func testFullscreenGeometryDistinguishesIdenticalSideBySideDisplays() {
        // Two identical 1920x1080 monitors side by side, top edges aligned at y = 1080.
        let primaryFrame = CGRect(x: 0, y: 0, width: 1920, height: 1080)
        let secondaryFrame = CGRect(x: 1920, y: 0, width: 1920, height: 1080)
        let primaryMaxY: CGFloat = 1080

        // Fullscreen window on secondary display in CG global coordinates (origin top-left of primary)
        let secondaryCGBounds = CGRect(x: 1920, y: 0, width: 1920, height: 1080)
        XCTAssertFalse(
            PanelFullscreenGeometry.isWindowFullscreen(
                cgBounds: secondaryCGBounds,
                primaryScreenMaxY: primaryMaxY,
                targetScreenFrame: primaryFrame
            )
        )
        XCTAssertTrue(
            PanelFullscreenGeometry.isWindowFullscreen(
                cgBounds: secondaryCGBounds,
                primaryScreenMaxY: primaryMaxY,
                targetScreenFrame: secondaryFrame
            )
        )
    }

    func testCandidateWindowBoundsOnlyIncludesFrontmostAppWindows() {
        let ownPID: pid_t = 100
        let frontmostAppPID: pid_t = 200
        let backgroundAppPID: pid_t = 300

        let primaryFrame = CGRect(x: 0, y: 0, width: 1512, height: 982)
        let secondaryFrame = CGRect(x: 1512, y: 0, width: 2560, height: 1440)
        let primaryMaxY: CGFloat = 982

        let secondaryFullscreenCG = CGRect(x: 1512, y: -458, width: 2560, height: 1440)
        let windowList: [[String: Any]] = [
            // Morrow's own panel window (must be excluded)
            [
                kCGWindowOwnerPID as String: ownPID,
                kCGWindowLayer as String: 0,
                kCGWindowAlpha as String: 1.0,
                kCGWindowBounds as String: CGRect(x: 1512, y: -458, width: 2560, height: 1440).dictionaryRepresentation as Any,
            ],
            // Frontmost app fullscreen window on Display 2 (must be included)
            [
                kCGWindowOwnerPID as String: frontmostAppPID,
                kCGWindowLayer as String: 0,
                kCGWindowAlpha as String: 1.0,
                kCGWindowBounds as String: secondaryFullscreenCG.dictionaryRepresentation as Any,
            ],
            // Non-frontmost background app fullscreen window on Display 1 (must be excluded)
            [
                kCGWindowOwnerPID as String: backgroundAppPID,
                kCGWindowLayer as String: 0,
                kCGWindowAlpha as String: 1.0,
                kCGWindowBounds as String: CGRect(x: 0, y: 0, width: 1512, height: 982).dictionaryRepresentation as Any,
            ],
            // Non-zero layer overlay (must be excluded)
            [
                kCGWindowOwnerPID as String: frontmostAppPID,
                kCGWindowLayer as String: 25,
                kCGWindowAlpha as String: 1.0,
                kCGWindowBounds as String: CGRect(x: 0, y: 0, width: 1512, height: 982).dictionaryRepresentation as Any,
            ],
            // Zero-alpha hidden window (must be excluded)
            [
                kCGWindowOwnerPID as String: frontmostAppPID,
                kCGWindowLayer as String: 0,
                kCGWindowAlpha as String: 0.0,
                kCGWindowBounds as String: CGRect(x: 0, y: 0, width: 1512, height: 982).dictionaryRepresentation as Any,
            ],
        ]

        let candidates = PanelFullscreenGeometry.candidateWindowBounds(
            from: windowList,
            frontmostPID: frontmostAppPID,
            excludingPID: ownPID
        )
        XCTAssertEqual(candidates.count, 1)

        let primaryFullscreen = candidates.contains {
            PanelFullscreenGeometry.isWindowFullscreen(
                cgBounds: $0,
                primaryScreenMaxY: primaryMaxY,
                targetScreenFrame: primaryFrame
            )
        }
        let secondaryFullscreen = candidates.contains {
            PanelFullscreenGeometry.isWindowFullscreen(
                cgBounds: $0,
                primaryScreenMaxY: primaryMaxY,
                targetScreenFrame: secondaryFrame
            )
        }

        XCTAssertFalse(primaryFullscreen)
        XCTAssertTrue(secondaryFullscreen)
    }

    @MainActor
    func testTwoDisplaysSwitchBetweenNotchAndDynamicIslandUpdatesPanelFramesEndToEnd() throws {
        _ = NSApplication.shared
        let sandbox = DefaultsSandbox(keys: DefaultsSandbox.allSettingsKeys)
        defer { sandbox.restore() }

        let d1 = DisplayDescriptor(
            id: "builtin",
            index: 0,
            name: "Built-in",
            width: 1512,
            height: 982,
            hasNotch: true,
            isBuiltin: true,
            isPrimary: true
        )
        let d2 = DisplayDescriptor(
            id: "EXT-27",
            index: 1,
            name: "External 27",
            width: 2560,
            height: 1440,
            hasNotch: false,
            isBuiltin: false,
            isPrimary: false
        )
        let f1 = CGRect(x: 0, y: 0, width: 1512, height: 982)
        let f2 = CGRect(x: 1512, y: 0, width: 2560, height: 1440)
        let backingScreen = NSScreen.screens.first ?? NSScreen()

        let appState = AppState()
        let controller = PanelWindowController(appState: appState)
        defer { controller.tearDownAllSlotsForTesting() }

        controller.screenEntriesProvider = {
            [
                PanelWindowController.ScreenEntry(descriptor: d1, screen: backingScreen, frameOverride: f1),
                PanelWindowController.ScreenEntry(descriptor: d2, screen: backingScreen, frameOverride: f2),
            ]
        }

        SettingsManager.shared.islandEnabled = true
        SettingsManager.shared.hideWhenNoSession = false
        SettingsManager.shared.hideInFullscreen = false
        SettingsManager.shared.setDisplayConfigurations([
            "builtin": DisplayConfiguration(enabled: true, displayMode: .notch, dynamicIslandTopOffset: 10),
            "EXT-27": DisplayConfiguration(enabled: true, displayMode: .dynamicIsland, dynamicIslandTopOffset: 14),
        ], displays: [d1, d2])

        controller.syncPanelsForTesting()

        let initialFrames = controller.activeSlotFramesForTesting
        XCTAssertEqual(Set(initialFrames.keys), ["builtin", "EXT-27"])
        let d1Initial = try XCTUnwrap(initialFrames["builtin"])
        let d2Initial = try XCTUnwrap(initialFrames["EXT-27"])
        XCTAssertEqual(d1Initial.maxY, f1.maxY, accuracy: 0.5)
        XCTAssertEqual(d2Initial.maxY, f2.maxY - 14, accuracy: 0.5)
        XCTAssertGreaterThanOrEqual(d2Initial.minX, f2.minX)

        // Flip both displays' modes and verify real panel frames follow.
        SettingsManager.shared.setDisplayConfiguration(
            DisplayConfiguration(enabled: true, displayMode: .dynamicIsland, dynamicIslandTopOffset: 20),
            for: "builtin",
            displays: [d1, d2]
        )
        SettingsManager.shared.setDisplayConfiguration(
            DisplayConfiguration(enabled: true, displayMode: .notch, dynamicIslandTopOffset: 14),
            for: "EXT-27",
            displays: [d1, d2]
        )
        controller.syncPanelsForTesting()

        let flippedFrames = controller.activeSlotFramesForTesting
        let d1Flipped = try XCTUnwrap(flippedFrames["builtin"])
        let d2Flipped = try XCTUnwrap(flippedFrames["EXT-27"])
        XCTAssertEqual(d1Flipped.maxY, f1.maxY - (32 + 20), accuracy: 0.5)
        XCTAssertEqual(d2Flipped.maxY, f2.maxY, accuracy: 0.5)
        XCTAssertEqual(controller.activeSlotModesForTesting["builtin"], .dynamicIsland)
        XCTAssertEqual(controller.activeSlotModesForTesting["EXT-27"], .notch)

        // Adjusting Top Offset on notched display keeps pill anchored below the 32pt physical notch.
        SettingsManager.shared.setDisplayConfiguration(
            DisplayConfiguration(enabled: true, displayMode: .dynamicIsland, dynamicIslandTopOffset: 8),
            for: "builtin",
            displays: [d1, d2]
        )
        controller.syncPanelsForTesting()
        let d1CompactOffset = try XCTUnwrap(controller.activeSlotFramesForTesting["builtin"])
        XCTAssertEqual(d1CompactOffset.maxY, f1.maxY - (32 + 8), accuracy: 0.5)

        // Disable Display 1 and verify only Display 2 slot remains.
        SettingsManager.shared.setDisplayConfiguration(
            DisplayConfiguration(enabled: false, displayMode: .dynamicIsland, dynamicIslandTopOffset: 20),
            for: "builtin",
            displays: [d1, d2]
        )
        controller.syncPanelsForTesting()
        XCTAssertEqual(Set(controller.activeSlotFramesForTesting.keys), ["EXT-27"])
    }
}
