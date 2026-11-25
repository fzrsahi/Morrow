import XCTest
@testable import MorrowCore

final class IslandDisplayModeTests: XCTestCase {

    func testNotchWidthScaleBoundsAndClamping() {
        XCTAssertEqual(NotchWidthScale.min, 0)
        XCTAssertEqual(NotchWidthScale.max, 200)
        XCTAssertEqual(NotchWidthScale.step, 1)
        XCTAssertEqual(NotchWidthScale.clamp(-25), 0)
        XCTAssertEqual(NotchWidthScale.clamp(100), 100)
        XCTAssertEqual(NotchWidthScale.clamp(350), 200)
    }

    func testDisplayModeRawValuesAndSafeResolution() {
        XCTAssertEqual(IslandDisplayMode.allCases, [.notch, .dynamicIsland])
        XCTAssertEqual(IslandDisplayMode.notch.rawValue, "notch")
        XCTAssertEqual(IslandDisplayMode.dynamicIsland.rawValue, "dynamicIsland")
        XCTAssertEqual(IslandDisplayMode.resolved(from: "dynamicIsland"), .dynamicIsland)
        XCTAssertEqual(IslandDisplayMode.resolved(from: "notch"), .notch)
        XCTAssertEqual(IslandDisplayMode.resolved(from: "invalid"), .notch)
        XCTAssertEqual(IslandDisplayMode.resolved(from: nil), .notch)
    }

    func testFloatingYOffsetDetachesOnlyInDynamicIsland() {
        XCTAssertEqual(IslandDisplayMetrics.floatingYOffset(displayMode: .notch, isFullscreenMode: false), 0)
        XCTAssertEqual(IslandDisplayMetrics.floatingYOffset(displayMode: .notch, isFullscreenMode: true), 0)
        XCTAssertEqual(IslandDisplayMetrics.floatingYOffset(displayMode: .notch, isFullscreenMode: true, customTopOffset: 24), 0)
        XCTAssertEqual(IslandDisplayMetrics.floatingYOffset(displayMode: .dynamicIsland, isFullscreenMode: false), 10)
        XCTAssertEqual(IslandDisplayMetrics.floatingYOffset(displayMode: .dynamicIsland, isFullscreenMode: true), 10)

        // Custom top offset clamps cleanly to 0...40pt when Dynamic Island is active
        XCTAssertEqual(IslandDisplayMetrics.floatingYOffset(displayMode: .dynamicIsland, isFullscreenMode: false, customTopOffset: 24), 24)
        XCTAssertEqual(IslandDisplayMetrics.floatingYOffset(displayMode: .dynamicIsland, isFullscreenMode: true, customTopOffset: 24), 24)
        XCTAssertEqual(IslandDisplayMetrics.floatingYOffset(displayMode: .dynamicIsland, isFullscreenMode: false, customTopOffset: -5), 0)
        XCTAssertEqual(IslandDisplayMetrics.floatingYOffset(displayMode: .dynamicIsland, isFullscreenMode: false, customTopOffset: 80), 40)
        XCTAssertEqual(IslandDisplayMetrics.floatingYOffset(displayMode: .notch, isFullscreenMode: false, customTopOffset: 24), 0)
        XCTAssertEqual(IslandDisplayMetrics.floatingYOffset(displayMode: .dynamicIsland, isFullscreenMode: false, customTopOffset: 10, physicalNotchHeight: 32), 42)
        XCTAssertEqual(IslandDisplayMetrics.floatingYOffset(displayMode: .notch, isFullscreenMode: false, customTopOffset: 10, physicalNotchHeight: 32), 0)
    }

    func testIntersectsHardwareNotchClearsWhenFloatingBelowPhysicalNotch() {
        XCTAssertFalse(
            IslandDisplayMetrics.intersectsHardwareNotch(
                displayMode: .dynamicIsland,
                hasNotch: false,
                topOffset: 10,
                physicalNotchHeight: 32
            )
        )
        XCTAssertTrue(
            IslandDisplayMetrics.intersectsHardwareNotch(
                displayMode: .notch,
                hasNotch: true,
                topOffset: 40,
                physicalNotchHeight: 32
            )
        )
        XCTAssertTrue(
            IslandDisplayMetrics.intersectsHardwareNotch(
                displayMode: .dynamicIsland,
                hasNotch: true,
                topOffset: 10,
                physicalNotchHeight: 32
            )
        )
        XCTAssertTrue(
            IslandDisplayMetrics.intersectsHardwareNotch(
                displayMode: .dynamicIsland,
                hasNotch: true,
                topOffset: 31,
                physicalNotchHeight: 32
            )
        )
        XCTAssertFalse(
            IslandDisplayMetrics.intersectsHardwareNotch(
                displayMode: .dynamicIsland,
                hasNotch: true,
                topOffset: 32,
                physicalNotchHeight: 32
            )
        )
        XCTAssertFalse(
            IslandDisplayMetrics.intersectsHardwareNotch(
                displayMode: .dynamicIsland,
                hasNotch: true,
                topOffset: 40,
                physicalNotchHeight: 32
            )
        )
    }

    func testCenterGapCollapsesDeadSpaceInDynamicIslandOnNonNotchScreens() {
        // Notch mode preserves simulated notch scaling
        XCTAssertEqual(
            IslandDisplayMetrics.centerGapWidth(notchWidth: 185, scalePercent: 100, hasNotch: false, displayMode: .notch),
            185
        )
        XCTAssertEqual(
            IslandDisplayMetrics.centerGapWidth(notchWidth: 200, scalePercent: 50, hasNotch: false, displayMode: .notch),
            100
        )

        // Dynamic Island on non-notch display (MacBook Air / external monitor / Windows) removes 185pt void
        XCTAssertEqual(
            IslandDisplayMetrics.centerGapWidth(notchWidth: 185, scalePercent: 100, hasNotch: false, displayMode: .dynamicIsland),
            16
        )
        XCTAssertEqual(
            IslandDisplayMetrics.centerGapWidth(notchWidth: 185, scalePercent: -10, hasNotch: false, displayMode: .dynamicIsland),
            8
        )
        XCTAssertEqual(
            IslandDisplayMetrics.centerGapWidth(notchWidth: 185, scalePercent: 300, hasNotch: false, displayMode: .dynamicIsland),
            32
        )
    }

    func testCenterGapPreservesPhysicalNotchClearanceWhenHardwareNotchPresent() {
        XCTAssertEqual(
            IslandDisplayMetrics.centerGapWidth(notchWidth: 185, scalePercent: 80, hasNotch: true, displayMode: .dynamicIsland),
            185
        )
        XCTAssertEqual(
            IslandDisplayMetrics.centerGapWidth(notchWidth: 185, scalePercent: 150, hasNotch: true, displayMode: .dynamicIsland),
            277.5
        )
    }

    func testRestingWidthFormsBalancedPillWhenNonNotchDynamicIslandIdle() {
        XCTAssertEqual(
            IslandDisplayMetrics.restingWidth(notchWidth: 185, scalePercent: 100, hasNotch: false, displayMode: .dynamicIsland),
            116
        )
        XCTAssertEqual(
            IslandDisplayMetrics.restingWidth(notchWidth: 185, scalePercent: 0, hasNotch: false, displayMode: .dynamicIsland),
            96
        )
        XCTAssertEqual(
            IslandDisplayMetrics.restingWidth(notchWidth: 185, scalePercent: 200, hasNotch: false, displayMode: .dynamicIsland),
            136
        )
        // Notched display still spans physical notch; notch mode matches centerGapWidth
        XCTAssertEqual(
            IslandDisplayMetrics.restingWidth(notchWidth: 185, scalePercent: 100, hasNotch: true, displayMode: .dynamicIsland),
            185
        )
        XCTAssertEqual(
            IslandDisplayMetrics.restingWidth(notchWidth: 185, scalePercent: 100, hasNotch: false, displayMode: .notch),
            185
        )
    }

    func testPillShapeKindFollowsSpecSection4() {
        XCTAssertEqual(
            IslandDisplayMetrics.pillShapeKind(isExpanded: false, isExtendedPill: false),
            .capsule
        )
        XCTAssertEqual(
            IslandDisplayMetrics.pillShapeKind(isExpanded: false, isExtendedPill: true),
            .roundedRectangle(cornerRadius: 18.0)
        )
        XCTAssertEqual(
            IslandDisplayMetrics.pillShapeKind(isExpanded: true, isExtendedPill: false),
            .roundedRectangle(cornerRadius: 24.0)
        )
        XCTAssertEqual(
            IslandDisplayMetrics.pillShapeKind(isExpanded: true, isExtendedPill: true),
            .roundedRectangle(cornerRadius: 24.0)
        )
    }

    func testAllowsNotchLyricLayoutDisablesFallbackOnNonNotchDynamicIsland() {
        XCTAssertFalse(IslandDisplayMetrics.allowsNotchLyricLayout(displayMode: .dynamicIsland, hasNotch: false))
        XCTAssertTrue(IslandDisplayMetrics.allowsNotchLyricLayout(displayMode: .dynamicIsland, hasNotch: true))
        XCTAssertTrue(IslandDisplayMetrics.allowsNotchLyricLayout(displayMode: .notch, hasNotch: false))
        XCTAssertTrue(IslandDisplayMetrics.allowsNotchLyricLayout(displayMode: .notch, hasNotch: true))
    }

    func testShouldShowInlineLyricsOverridesWhenAgentActiveOrToastVisible() {
        XCTAssertTrue(
            IslandDisplayMetrics.shouldShowInlineLyrics(
                displayMode: .dynamicIsland,
                hasNotch: false,
                lyricsEligible: true,
                hasActiveOrWaitingAgent: false,
                isToastOnly: false
            )
        )
        XCTAssertFalse(
            IslandDisplayMetrics.shouldShowInlineLyrics(
                displayMode: .dynamicIsland,
                hasNotch: false,
                lyricsEligible: true,
                hasActiveOrWaitingAgent: true,
                isToastOnly: false
            )
        )
        XCTAssertFalse(
            IslandDisplayMetrics.shouldShowInlineLyrics(
                displayMode: .dynamicIsland,
                hasNotch: false,
                lyricsEligible: true,
                hasActiveOrWaitingAgent: false,
                isToastOnly: true
            )
        )
        XCTAssertFalse(
            IslandDisplayMetrics.shouldShowInlineLyrics(
                displayMode: .dynamicIsland,
                hasNotch: true,
                lyricsEligible: true,
                hasActiveOrWaitingAgent: false,
                isToastOnly: false
            )
        )
        XCTAssertFalse(
            IslandDisplayMetrics.shouldShowInlineLyrics(
                displayMode: .notch,
                hasNotch: false,
                lyricsEligible: true,
                hasActiveOrWaitingAgent: false,
                isToastOnly: false
            )
        )
    }

    func testInlineLyricsWidthStaysWithinSpecBoundsAndHardCaps() {
        let shortW = IslandDisplayMetrics.inlineLyricsWidth(displayedText: "Yeah", screenWidth: 1440, scalePercent: 100)
        XCTAssertEqual(shortW, 180, accuracy: 0.5)

        let mediumW = IslandDisplayMetrics.inlineLyricsWidth(displayedText: "Dancing in the moonlight", screenWidth: 1440, scalePercent: 100)
        XCTAssertGreaterThan(mediumW, 180)
        XCTAssertLessThanOrEqual(mediumW, 380)

        let longW = IslandDisplayMetrics.inlineLyricsWidth(
            displayedText: "This is a very long lyric line that should definitely hit the upper width cap cleanly",
            screenWidth: 1440,
            scalePercent: 100
        )
        XCTAssertEqual(longW, 380, accuracy: 0.5)

        let scaledMaxW = IslandDisplayMetrics.inlineLyricsWidth(
            displayedText: "This is a very long lyric line that should definitely hit the upper width cap cleanly",
            screenWidth: 1440,
            scalePercent: 200
        )
        XCTAssertEqual(scaledMaxW, 400, accuracy: 0.5)

        let narrowScreenW = IslandDisplayMetrics.inlineLyricsWidth(
            displayedText: "This is a very long lyric line that should definitely hit the upper width cap cleanly",
            screenWidth: 800,
            scalePercent: 100
        )
        XCTAssertEqual(narrowScreenW, 280, accuracy: 0.5)

        let cjkW = IslandDisplayMetrics.inlineLyricsWidth(displayedText: "夜空ノムコウ", screenWidth: 1440, scalePercent: 100)
        XCTAssertGreaterThanOrEqual(cjkW, 180)
        XCTAssertLessThanOrEqual(cjkW, 380)
    }

    func testBalancedTwinHardCapMatchesSpecSection2() {
        XCTAssertEqual(
            IslandDisplayMetrics.balancedTwinHardCap(
                isHoveringRightWing: false,
                hasActiveOrWaitingAgent: false,
                isMusicPlaying: true,
                hasPriorityAccessory: false
            ),
            160
        )
        XCTAssertEqual(
            IslandDisplayMetrics.balancedTwinHardCap(
                isHoveringRightWing: false,
                hasActiveOrWaitingAgent: false,
                isMusicPlaying: false,
                hasPriorityAccessory: true
            ),
            180
        )
        XCTAssertEqual(
            IslandDisplayMetrics.balancedTwinHardCap(
                isHoveringRightWing: false,
                hasActiveOrWaitingAgent: true,
                isMusicPlaying: false,
                hasPriorityAccessory: false
            ),
            260
        )
        XCTAssertEqual(
            IslandDisplayMetrics.balancedTwinHardCap(
                isHoveringRightWing: false,
                hasActiveOrWaitingAgent: true,
                isMusicPlaying: true,
                hasPriorityAccessory: false
            ),
            280
        )
        XCTAssertEqual(
            IslandDisplayMetrics.balancedTwinHardCap(
                isHoveringRightWing: true,
                hasActiveOrWaitingAgent: false,
                isMusicPlaying: true,
                hasPriorityAccessory: false
            ),
            320
        )
        XCTAssertEqual(
            IslandDisplayMetrics.balancedTwinHardCap(
                isHoveringRightWing: false,
                hasActiveOrWaitingAgent: false,
                isMusicPlaying: false,
                hasPriorityAccessory: false,
                hasQuotaChip: true
            ),
            260
        )
    }

    func testDynamicIslandPillCornerRadiiMatchCollapsedExtendedAndExpandedStates() {
        XCTAssertEqual(IslandDisplayMetrics.pillCornerRadius(height: 32, isExpanded: false, isExtendedPill: false), 16)
        XCTAssertEqual(IslandDisplayMetrics.pillCornerRadius(height: 18, isExpanded: false, isExtendedPill: false), 12)
        XCTAssertEqual(IslandDisplayMetrics.pillCornerRadius(height: 44, isExpanded: false, isExtendedPill: false), 22)
        XCTAssertEqual(IslandDisplayMetrics.pillCornerRadius(height: 58, isExpanded: false, isExtendedPill: true), 18)
        XCTAssertEqual(IslandDisplayMetrics.pillCornerRadius(height: 320, isExpanded: true, isExtendedPill: false), 24)
        XCTAssertEqual(IslandDisplayMetrics.pillCornerRadius(height: 80, isExpanded: true, isExtendedPill: true), 24)
    }

    func testPillOuterPaddingProtectsRoundedCapsInDynamicIslandMode() {
        XCTAssertEqual(
            IslandDisplayMetrics.outerPadding(wingWidth: 40, expanded: false, bilateral: false, displayMode: .dynamicIsland),
            13
        )
        XCTAssertEqual(
            IslandDisplayMetrics.outerPadding(wingWidth: 40, expanded: false, bilateral: false, displayMode: .notch),
            6
        )
        XCTAssertEqual(
            IslandDisplayMetrics.outerPadding(wingWidth: 60, expanded: false, bilateral: false, displayMode: .notch),
            8
        )
        XCTAssertEqual(
            IslandDisplayMetrics.outerPadding(wingWidth: 80, expanded: true, bilateral: false, displayMode: .dynamicIsland),
            14
        )
    }

    func testBalancedTwinWidthRemovesHollowGapAndScalesCleanly() {
        let idleMusicW = IslandDisplayMetrics.balancedTwinWidth(
            leftWidth: 26,
            rightWidth: 38,
            outerPadding: 13,
            dividerGap: 10,
            hasDivider: true,
            scalePercent: 100,
            maxAllowedWidth: 400
        )
        XCTAssertEqual(idleMusicW, 111, accuracy: 0.5)

        let defaultGapW = IslandDisplayMetrics.balancedTwinWidth(
            leftWidth: 26,
            rightWidth: 38
        )
        XCTAssertEqual(defaultGapW, 111, accuracy: 0.5)

        let thinkingW = IslandDisplayMetrics.balancedTwinWidth(
            leftWidth: 86,
            rightWidth: 44,
            outerPadding: 13,
            dividerGap: 10,
            hasDivider: true,
            scalePercent: 100,
            maxAllowedWidth: 400
        )
        XCTAssertEqual(thinkingW, 177, accuracy: 0.5)
    }
}
