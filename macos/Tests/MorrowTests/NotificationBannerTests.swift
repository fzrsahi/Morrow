import XCTest
import SwiftUI
import AppKit
@testable import Morrow
import MorrowCore

@MainActor
final class NotificationBannerTests: XCTestCase {

    // MARK: - BannerHoverHoldTimer

    func testHoverHoldAutoDismissesAfterDeadlineWhenNotHovered() {
        let timer = BannerHoverHoldTimer(defaultDuration: 0.08)
        let exp = expectation(description: "Auto-dismiss fires")

        timer.start { exp.fulfill() }
        wait(for: [exp], timeout: 0.5)
        XCTAssertFalse(timer.isHovered)
        XCTAssertNil(timer.deadline)
    }

    func testHoverHoldPausesWhileHoveredPastDeadlineThenDismissesOnUnhover() {
        let timer = BannerHoverHoldTimer(defaultDuration: 0.06)
        var dismissed = false

        timer.start { dismissed = true }
        timer.setHovered(true, isVisible: true) { dismissed = true }

        let waitPastDeadline = expectation(description: "Wait past initial 0.06s deadline")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.12) { waitPastDeadline.fulfill() }
        wait(for: [waitPastDeadline], timeout: 0.5)

        XCTAssertFalse(dismissed, "Banner must stay held while hovered past deadline")

        timer.setHovered(false, isVisible: true) { dismissed = true }
        XCTAssertTrue(dismissed, "Banner must dismiss immediately when unhovered after deadline elapsed")
    }

    func testHoverHoldWaitsRemainingTimeWhenUnhoveredBeforeDeadline() {
        let timer = BannerHoverHoldTimer(defaultDuration: 0.16)
        var dismissed = false

        timer.start { dismissed = true }
        timer.setHovered(true, isVisible: true) { dismissed = true }
        timer.setHovered(false, isVisible: true) { dismissed = true }

        XCTAssertFalse(dismissed, "Banner must wait remaining deadline when unhovered early")

        let exp = expectation(description: "Remaining deadline expires")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.24) { exp.fulfill() }
        wait(for: [exp], timeout: 0.6)
        XCTAssertTrue(dismissed)
    }

    // MARK: - PowerBannerState Metadata

    func testPowerBannerStateMetadataAndIcons() {
        let charging = PowerBannerState.charging(kind: .magSafe, watts: 140, timeToFullMinutes: 28)
        XCTAssertEqual(charging.title, "Charging")
        XCTAssertEqual(charging.subtitle, "MagSafe · 140W · 28m left")
        XCTAssertEqual(charging.modernIconName, "bolt.fill")
        XCTAssertEqual(charging.pixelGlyph, .bolt)

        let onHold = PowerBannerState.onHold(kind: .usbC)
        XCTAssertEqual(onHold.title, "Charging On Hold")
        XCTAssertEqual(onHold.subtitle, "USB-C · Optimized charging")
        XCTAssertEqual(onHold.modernIconName, "pause.fill")
        XCTAssertEqual(onHold.pixelGlyph, .power)

        let full = PowerBannerState.fullyCharged(kind: .magSafe)
        XCTAssertEqual(full.title, "Fully Charged")
        XCTAssertEqual(full.subtitle, "MagSafe · 100% charged")
        XCTAssertEqual(full.modernIconName, "checkmark")
        XCTAssertEqual(full.pixelGlyph, .checkmark)

        let low = PowerBannerState.lowBattery(percent: 15)
        XCTAssertEqual(low.title, "Low Battery")
        XCTAssertEqual(low.subtitle, "Connect power adapter")
        XCTAssertEqual(low.modernIconName, "battery.25")
        XCTAssertEqual(low.pixelGlyph, .power)
    }

    // MARK: - PrayerTimeManager Preview State & Theme Rendering

    func testTriggerTestBannerSetsAndDismissClearsPreviewPrayerType() {
        let manager = PrayerTimeManager.shared
        manager.triggerTestBanner(prayer: .subuh)
        XCTAssertTrue(manager.showMiniPeekBanner)
        XCTAssertEqual(manager.previewPrayerType, .subuh)

        manager.dismissMiniPeekBanner()
        XCTAssertFalse(manager.showMiniPeekBanner)
        XCTAssertNil(manager.previewPrayerType)
    }

    func testChargingAndPrayerBannersRenderInModernAndPixelThemes() {
        let defaults = UserDefaults.standard
        let previousTheme = defaults.string(forKey: SettingsKey.islandUITheme)
        defer {
            if let previousTheme {
                defaults.set(previousTheme, forKey: SettingsKey.islandUITheme)
            } else {
                defaults.removeObject(forKey: SettingsKey.islandUITheme)
            }
        }

        for theme in [IslandUITheme.modern.rawValue, IslandUITheme.pixel.rawValue] {
            defaults.set(theme, forKey: SettingsKey.islandUITheme)

            let chargingHost = NSHostingView(rootView: MagSafeChargingBanner(batteryLevel: 54, displayMode: .notch))
            chargingHost.frame = NSRect(x: 0, y: 0, width: 380, height: 70)
            chargingHost.layoutSubtreeIfNeeded()
            XCTAssertGreaterThan(chargingHost.fittingSize.width, 0)

            let prayerHost = NSHostingView(rootView: PrayerMiniPeekBannerView(manager: PrayerTimeManager.shared, displayMode: .notch) {})
            prayerHost.frame = NSRect(x: 0, y: 0, width: 500, height: 150)
            prayerHost.layoutSubtreeIfNeeded()
            XCTAssertGreaterThan(prayerHost.fittingSize.width, 0)
        }
    }
}
