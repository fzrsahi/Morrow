import AppKit
import XCTest
@testable import Morrow
import MorrowCore

@MainActor
final class MultiDisplaySettingsTests: XCTestCase {
    private var sandbox: DefaultsSandbox!

    private let macBook = DisplayDescriptor(
        id: "builtin",
        index: 0,
        name: "Built-in Retina Display",
        width: 1512,
        height: 982,
        hasNotch: true,
        isBuiltin: true,
        isPrimary: true
    )

    private let external27 = DisplayDescriptor(
        id: "EXT-27",
        index: 1,
        name: "Studio Display",
        width: 2560,
        height: 1440,
        hasNotch: false,
        isBuiltin: false,
        isPrimary: false
    )

    override func setUp() async throws {
        try await super.setUp()
        _ = NSApplication.shared
        sandbox = DefaultsSandbox(keys: DefaultsSandbox.panelKeys)
        UserDefaults.standard.removeObject(forKey: SettingsKey.islandEnabled)
        UserDefaults.standard.removeObject(forKey: SettingsKey.displayConfigurations)
        UserDefaults.standard.removeObject(forKey: SettingsKey.displayChoice)
        UserDefaults.standard.removeObject(forKey: SettingsKey.islandDisplayMode)
        UserDefaults.standard.removeObject(forKey: SettingsKey.dynamicIslandTopOffset)
    }

    override func tearDown() async throws {
        StatusItemController.shared.displaysProvider = { ScreenDetector.descriptors() }
        sandbox?.restore()
        L10n.shared.language = "system"
        try await super.tearDown()
    }

    func testMasterIslandEnabledDefaultsTrueAndToggles() {
        XCTAssertTrue(SettingsManager.shared.islandEnabled)
        SettingsManager.shared.islandEnabled = false
        XCTAssertFalse(SettingsManager.shared.islandEnabled)
        SettingsManager.shared.islandEnabled = true
        XCTAssertTrue(SettingsManager.shared.islandEnabled)
    }

    func testLegacyMigrationAndPerDisplayModeMirroring() {
        UserDefaults.standard.set("auto", forKey: SettingsKey.displayChoice)
        UserDefaults.standard.set(IslandDisplayMode.dynamicIsland.rawValue, forKey: SettingsKey.islandDisplayMode)
        UserDefaults.standard.set(18.0, forKey: SettingsKey.dynamicIslandTopOffset)

        let displays = [macBook, external27]
        let migrated = SettingsManager.shared.displayConfigurations(for: displays)
        XCTAssertEqual(migrated["builtin"]?.enabled, true)
        XCTAssertEqual(migrated["builtin"]?.displayMode, .dynamicIsland)
        XCTAssertEqual(migrated["builtin"]?.dynamicIslandTopOffset, 18)
        XCTAssertEqual(migrated["EXT-27"]?.enabled, false)
        XCTAssertEqual(migrated["EXT-27"]?.displayMode, .dynamicIsland)
        XCTAssertNotNil(SettingsManager.shared.rawDisplayConfigurations)

        // Switch Display 1 to Notch and enable Display 2 as Dynamic Island (24pt)
        SettingsManager.shared.setDisplayConfigurations([
            "builtin": DisplayConfiguration(enabled: true, displayMode: .notch, dynamicIslandTopOffset: 10),
            "EXT-27": DisplayConfiguration(enabled: true, displayMode: .dynamicIsland, dynamicIslandTopOffset: 24),
        ], displays: displays)

        XCTAssertEqual(SettingsManager.shared.islandDisplayMode, .notch)
        XCTAssertEqual(SettingsManager.shared.dynamicIslandTopOffset, 10)

        // Disable Display 1 so Display 2 becomes primary enabled display -> mirrors Display 2
        SettingsManager.shared.setDisplayConfiguration(
            DisplayConfiguration(enabled: false, displayMode: .notch, dynamicIslandTopOffset: 10),
            for: "builtin",
            displays: displays
        )
        XCTAssertEqual(SettingsManager.shared.islandDisplayMode, .dynamicIsland)
        XCTAssertEqual(SettingsManager.shared.dynamicIslandTopOffset, 24)
    }

    func testRestoreIslandIfAllDisplaysOffEnablesPreferredDisplay() {
        let displays = [macBook, external27]
        SettingsManager.shared.islandEnabled = false
        SettingsManager.shared.setDisplayConfigurations([
            "builtin": DisplayConfiguration(enabled: false, displayMode: .notch, dynamicIslandTopOffset: 10),
            "EXT-27": DisplayConfiguration(enabled: false, displayMode: .dynamicIsland, dynamicIslandTopOffset: 10),
        ], displays: displays)

        SettingsManager.shared.restoreIslandIfAllDisplaysOff(displays: displays)
        XCTAssertTrue(SettingsManager.shared.islandEnabled)

        let restored = SettingsManager.shared.displayConfigurations(for: displays)
        XCTAssertEqual(restored["builtin"]?.enabled, true)
        XCTAssertEqual(restored["EXT-27"]?.enabled, false)
    }

    func testStatusItemMenuCheckmarkTracksIslandEnabledAndRecoversInOneClick() {
        let controller = StatusItemController.shared
        let displays = [macBook, external27]
        controller.displaysProvider = { displays }
        let menu = controller.statusMenuForTesting

        SettingsManager.shared.islandEnabled = true
        SettingsManager.shared.setDisplayConfigurations([
            "builtin": DisplayConfiguration(enabled: true, displayMode: .notch, dynamicIslandTopOffset: 10),
            "EXT-27": DisplayConfiguration(enabled: false, displayMode: .dynamicIsland, dynamicIslandTopOffset: 10),
        ], displays: displays)
        controller.menuNeedsUpdate(menu)
        XCTAssertEqual(menu.items.first?.state, .on)

        SettingsManager.shared.islandEnabled = false
        controller.menuNeedsUpdate(menu)
        XCTAssertEqual(menu.items.first?.state, .off)

        // When master islandEnabled is true but all connected displays are disabled,
        // checkmark must show .off and one click must recover the preferred display.
        SettingsManager.shared.islandEnabled = true
        SettingsManager.shared.setDisplayConfigurations([
            "builtin": DisplayConfiguration(enabled: false, displayMode: .notch, dynamicIslandTopOffset: 10),
            "EXT-27": DisplayConfiguration(enabled: false, displayMode: .dynamicIsland, dynamicIslandTopOffset: 10),
        ], displays: displays)
        controller.menuNeedsUpdate(menu)
        XCTAssertEqual(menu.items.first?.state, .off)

        controller.performToggleIslandEnabled()
        controller.menuNeedsUpdate(menu)
        XCTAssertTrue(SettingsManager.shared.islandEnabled)
        XCTAssertEqual(SettingsManager.shared.displayConfigurations(for: displays)["builtin"]?.enabled, true)
        XCTAssertEqual(menu.items.first?.state, .on)

        // Clicking while active disables islandEnabled; clicking again restores it.
        controller.performToggleIslandEnabled()
        XCTAssertFalse(SettingsManager.shared.islandEnabled)
        controller.menuNeedsUpdate(menu)
        XCTAssertEqual(menu.items.first?.state, .off)

        controller.performToggleIslandEnabled()
        XCTAssertTrue(SettingsManager.shared.islandEnabled)
        controller.menuNeedsUpdate(menu)
        XCTAssertEqual(menu.items.first?.state, .on)
    }

    func testDisplaySettingsSummaryFormatter() {
        L10n.shared.language = "en"
        let displays = [macBook, external27]
        let configs: [String: DisplayConfiguration] = [
            "builtin": DisplayConfiguration(enabled: true, displayMode: .notch, dynamicIslandTopOffset: 10),
            "EXT-27": DisplayConfiguration(enabled: true, displayMode: .dynamicIsland, dynamicIslandTopOffset: 14),
        ]

        XCTAssertEqual(
            DisplaySettingsSummaryFormatter.summary(islandEnabled: true, displays: displays, configs: configs, l10n: L10n.shared),
            "2 Displays · Notch, Dynamic Island"
        )
        XCTAssertEqual(
            DisplaySettingsSummaryFormatter.summary(islandEnabled: false, displays: displays, configs: configs, l10n: L10n.shared),
            "Hidden on all displays"
        )
        XCTAssertEqual(
            DisplaySettingsSummaryFormatter.metadataLine(display: macBook, l10n: L10n.shared),
            "1512×982 · Primary"
        )
        XCTAssertEqual(
            DisplaySettingsSummaryFormatter.metadataLine(display: external27, l10n: L10n.shared),
            "2560×1440 · External"
        )
    }
}
