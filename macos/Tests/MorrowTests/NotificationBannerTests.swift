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

    func testChargingBannerKeepsSemanticColorsWhenAutoAlbumThemeActive() {
        let defaults = UserDefaults.standard
        let prevSync = defaults.object(forKey: SettingsKey.syncThemeWithAlbumArt)
        let prevColor = defaults.object(forKey: SettingsKey.musicHUDProgressColor)
        let nowPlaying = NowPlayingManager.shared
        let prevPlaying = nowPlaying.isPlaying
        let prevArtwork = nowPlaying.artworkImage
        let prevPalette = nowPlaying.albumPalette
        defer {
            if let prevSync { defaults.set(prevSync, forKey: SettingsKey.syncThemeWithAlbumArt) } else { defaults.removeObject(forKey: SettingsKey.syncThemeWithAlbumArt) }
            if let prevColor { defaults.set(prevColor, forKey: SettingsKey.musicHUDProgressColor) } else { defaults.removeObject(forKey: SettingsKey.musicHUDProgressColor) }
            nowPlaying.isPlaying = prevPlaying
            nowPlaying.artworkImage = prevArtwork
            nowPlaying.albumPalette = prevPalette
        }

        defaults.set(true, forKey: SettingsKey.syncThemeWithAlbumArt)
        defaults.set(MusicProgressColor.album.rawValue, forKey: SettingsKey.musicHUDProgressColor)
        nowPlaying.isPlaying = true
        nowPlaying.artworkImage = NSImage(size: NSSize(width: 10, height: 10))
        let pinkAlbum = AlbumArtPalette(
            primary: Color(red: 1.0, green: 0.10, blue: 0.50),
            secondary: Color(red: 0.50, green: 0.20, blue: 0.90),
            spectrum: RetroSpectrumPalette(base: .pink, mid: .purple, peak: .white)
        )
        nowPlaying.albumPalette = pinkAlbum

        // MusicProgressColor honors syncThemeWithAlbumArt / album
        XCTAssertEqual(MusicProgressColor.cyan.color, pinkAlbum.primary)
        XCTAssertEqual(MusicProgressColor.album.color, pinkAlbum.primary)

        // Battery/charging states stay strictly semantic (green / amber / red), never album-colored
        let charging = PowerBannerState.charging(kind: .magSafe, watts: 140, timeToFullMinutes: 20)
        let full = PowerBannerState.fullyCharged(kind: .magSafe)
        let onHold = PowerBannerState.onHold(kind: .magSafe)
        let low = PowerBannerState.lowBattery(percent: 15)

        XCTAssertEqual(MagSafeChargingBanner.semanticColor(for: charging), charging.accentColor)
        XCTAssertEqual(MagSafeChargingBanner.semanticColor(for: full), full.accentColor)
        XCTAssertEqual(MagSafeChargingBanner.semanticColor(for: onHold), onHold.accentColor)
        XCTAssertEqual(MagSafeChargingBanner.semanticColor(for: low), low.accentColor)
        XCTAssertNotEqual(MagSafeChargingBanner.semanticColor(for: charging), pinkAlbum.primary)
    }

    func testCyberSystemGaugeBarBatteryColorAndIconCoverAllPowerStates() {
        let full100 = SystemMetrics(
            batteryLevel: 100,
            isCharging: false,
            isACPowered: true,
            powerBannerState: .fullyCharged(kind: .magSafe)
        )
        XCTAssertEqual(CyberSystemGaugeBar.batteryColor(for: full100), PowerBannerState.fullyCharged(kind: .magSafe).accentColor)
        XCTAssertEqual(CyberSystemGaugeBar.batteryIcon(for: full100), "battery.100")

        let full99 = SystemMetrics(
            batteryLevel: 99,
            isCharging: false,
            isACPowered: true,
            powerBannerState: .fullyCharged(kind: .usbC)
        )
        XCTAssertEqual(CyberSystemGaugeBar.batteryColor(for: full99), PowerBannerState.fullyCharged(kind: .magSafe).accentColor)
        XCTAssertEqual(CyberSystemGaugeBar.batteryIcon(for: full99), "battery.100")

        let onHold = SystemMetrics(
            batteryLevel: 80,
            isCharging: false,
            isACPowered: true,
            powerBannerState: .onHold(kind: .magSafe)
        )
        XCTAssertEqual(CyberSystemGaugeBar.batteryColor(for: onHold), PowerBannerState.onHold(kind: .magSafe).accentColor)
        XCTAssertEqual(CyberSystemGaugeBar.batteryIcon(for: onHold), "battery.75")

        let low = SystemMetrics(
            batteryLevel: 15,
            isCharging: false,
            isACPowered: false,
            powerBannerState: .lowBattery(percent: 15)
        )
        XCTAssertEqual(CyberSystemGaugeBar.batteryColor(for: low), PowerBannerState.lowBattery(percent: 15).accentColor)
        XCTAssertEqual(CyberSystemGaugeBar.batteryIcon(for: low), "battery.25")

        let charging = SystemMetrics(
            batteryLevel: 50,
            isCharging: true,
            isACPowered: true,
            powerBannerState: .charging(kind: .magSafe, watts: 140, timeToFullMinutes: 20)
        )
        XCTAssertEqual(CyberSystemGaugeBar.batteryColor(for: charging), PowerBannerState.charging(kind: .magSafe, watts: nil, timeToFullMinutes: nil).accentColor)
        XCTAssertEqual(CyberSystemGaugeBar.batteryIcon(for: charging), "battery.100.bolt")

        let dischargingNormal = SystemMetrics(
            batteryLevel: 65,
            isCharging: false,
            isACPowered: false,
            powerBannerState: .charging(kind: .magSafe, watts: nil, timeToFullMinutes: nil)
        )
        XCTAssertEqual(CyberSystemGaugeBar.batteryColor(for: dischargingNormal), Color.white.opacity(0.8))
        XCTAssertEqual(CyberSystemGaugeBar.batteryIcon(for: dischargingNormal), "battery.75")

        let dischargingFull = SystemMetrics(
            batteryLevel: 100,
            isCharging: false,
            isACPowered: false,
            powerBannerState: .charging(kind: .magSafe, watts: nil, timeToFullMinutes: nil)
        )
        XCTAssertEqual(CyberSystemGaugeBar.batteryColor(for: dischargingFull), Color.white.opacity(0.8))
        XCTAssertEqual(CyberSystemGaugeBar.batteryIcon(for: dischargingFull), "battery.100")
    }
}
