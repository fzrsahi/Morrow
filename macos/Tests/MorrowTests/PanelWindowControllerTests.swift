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
        XCTAssertEqual(display1NotchY, 950)
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

    func testCandidateWindowBoundsDetectsNonFrontmostCrossDisplayFullscreenWindows() {
        let ownPID: pid_t = 100
        let display1FocusedAppPID: pid_t = 200
        let display2FullscreenAppPID: pid_t = 300

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
            // Focused app on Display 1 (not fullscreen)
            [
                kCGWindowOwnerPID as String: display1FocusedAppPID,
                kCGWindowLayer as String: 0,
                kCGWindowAlpha as String: 1.0,
                kCGWindowBounds as String: CGRect(x: 40, y: 40, width: 1000, height: 700).dictionaryRepresentation as Any,
            ],
            // Non-frontmost fullscreen app on Display 2 (must still be included)
            [
                kCGWindowOwnerPID as String: display2FullscreenAppPID,
                kCGWindowLayer as String: 0,
                kCGWindowAlpha as String: 1.0,
                kCGWindowBounds as String: secondaryFullscreenCG.dictionaryRepresentation as Any,
            ],
            // Non-zero layer overlay (must be excluded)
            [
                kCGWindowOwnerPID as String: display2FullscreenAppPID,
                kCGWindowLayer as String: 25,
                kCGWindowAlpha as String: 1.0,
                kCGWindowBounds as String: CGRect(x: 0, y: 0, width: 1512, height: 982).dictionaryRepresentation as Any,
            ],
            // Zero-alpha hidden window (must be excluded)
            [
                kCGWindowOwnerPID as String: display2FullscreenAppPID,
                kCGWindowLayer as String: 0,
                kCGWindowAlpha as String: 0.0,
                kCGWindowBounds as String: CGRect(x: 0, y: 0, width: 1512, height: 982).dictionaryRepresentation as Any,
            ],
        ]

        let candidates = PanelFullscreenGeometry.candidateWindowBounds(
            from: windowList,
            excludingPID: ownPID
        )
        XCTAssertEqual(candidates.count, 2)

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
}
