import XCTest
@testable import MorrowCore

final class MultiDisplayConfigurationTests: XCTestCase {
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
        id: "EXT-UUID-27",
        index: 1,
        name: "Studio Display",
        width: 2560,
        height: 1440,
        hasNotch: false,
        isBuiltin: false,
        isPrimary: false
    )

    private let unseenClamshellMonitor = DisplayDescriptor(
        id: "EXT-UUID-NEW",
        index: 0,
        name: "LG UltraFine",
        width: 3840,
        height: 2160,
        hasNotch: false,
        isBuiltin: false,
        isPrimary: true
    )

    func testDisplayConfigurationCodecRoundTripAndClamping() throws {
        let original: [String: DisplayConfiguration] = [
            "builtin": DisplayConfiguration(enabled: true, displayMode: .notch, dynamicIslandTopOffset: 10),
            "EXT-UUID-27": DisplayConfiguration(enabled: true, displayMode: .dynamicIsland, dynamicIslandTopOffset: 99),
        ]
        let encoded = DisplayConfigurationCodec.encode(original)
        let decoded = try XCTUnwrap(DisplayConfigurationCodec.decode(encoded))
        XCTAssertEqual(decoded["builtin"], DisplayConfiguration(enabled: true, displayMode: .notch, dynamicIslandTopOffset: 10))
        XCTAssertEqual(decoded["EXT-UUID-27"], DisplayConfiguration(enabled: true, displayMode: .dynamicIsland, dynamicIslandTopOffset: 40))

        XCTAssertNil(DisplayConfigurationCodec.decode(nil))
        XCTAssertNil(DisplayConfigurationCodec.decode(""))
        XCTAssertNil(DisplayConfigurationCodec.decode("   "))
        XCTAssertNil(DisplayConfigurationCodec.decode("not-json"))
    }

    func testDisplayConfigurationCorruptJSONFallsBackSafely() throws {
        let json = """
        {
          "builtin": {
            "displayMode": "invalid_mode",
            "dynamicIslandTopOffset": -12
          }
        }
        """
        let decoded = try XCTUnwrap(DisplayConfigurationCodec.decode(json))
        let config = try XCTUnwrap(decoded["builtin"])
        XCTAssertTrue(config.enabled)
        XCTAssertEqual(config.displayMode, .notch)
        XCTAssertEqual(config.dynamicIslandTopOffset, 0)
    }

    func testLegacyMigrationRespectsDisplayChoiceAndSmartDefaults() {
        let displays = [macBook, external27]

        let autoMigrated = DisplayConfigurationPlanner.migrateLegacy(
            displays: displays,
            displayChoice: "auto",
            legacyMode: .dynamicIsland,
            legacyTopOffset: 18
        )
        XCTAssertEqual(autoMigrated["builtin"], DisplayConfiguration(enabled: true, displayMode: .dynamicIsland, dynamicIslandTopOffset: 18))
        XCTAssertEqual(autoMigrated["EXT-UUID-27"], DisplayConfiguration(enabled: false, displayMode: .dynamicIsland, dynamicIslandTopOffset: 10))

        let screen1Migrated = DisplayConfigurationPlanner.migrateLegacy(
            displays: displays,
            displayChoice: "screen_1",
            legacyMode: .notch,
            legacyTopOffset: 10
        )
        XCTAssertEqual(screen1Migrated["builtin"]?.enabled, false)
        XCTAssertEqual(screen1Migrated["builtin"]?.displayMode, .notch)
        XCTAssertEqual(screen1Migrated["EXT-UUID-27"]?.enabled, true)
        XCTAssertEqual(screen1Migrated["EXT-UUID-27"]?.displayMode, .notch)
    }

    func testTwoDisplayIndependentModeAndEnablementSwapping() {
        let displays = [macBook, external27]
        let configs: [String: DisplayConfiguration] = [
            "builtin": DisplayConfiguration(enabled: true, displayMode: .notch, dynamicIslandTopOffset: 10),
            "EXT-UUID-27": DisplayConfiguration(enabled: true, displayMode: .dynamicIsland, dynamicIslandTopOffset: 22),
        ]
        XCTAssertEqual(DisplayConfigurationPlanner.primaryEnabledDisplayID(displays: displays, configs: configs), "builtin")

        let swapped: [String: DisplayConfiguration] = [
            "builtin": DisplayConfiguration(enabled: false, displayMode: .dynamicIsland, dynamicIslandTopOffset: 16),
            "EXT-UUID-27": DisplayConfiguration(enabled: true, displayMode: .notch, dynamicIslandTopOffset: 10),
        ]
        XCTAssertEqual(DisplayConfigurationPlanner.primaryEnabledDisplayID(displays: displays, configs: swapped), "EXT-UUID-27")
    }

    func testUnplugAndReplugRetainsExternalDisplayConfiguration() {
        let twoDisplays = [macBook, external27]
        let stored: [String: DisplayConfiguration] = [
            "builtin": DisplayConfiguration(enabled: true, displayMode: .notch, dynamicIslandTopOffset: 10),
            "EXT-UUID-27": DisplayConfiguration(enabled: true, displayMode: .dynamicIsland, dynamicIslandTopOffset: 24),
        ]

        // Unplug external monitor -> only macBook connected, stored retains EXT-UUID-27
        let unplugged = DisplayConfigurationPlanner.resolvedConfigurations(
            stored: stored,
            displays: [macBook],
            displayChoice: "auto",
            legacyMode: .notch,
            legacyTopOffset: 10
        )
        XCTAssertFalse(unplugged.didMigrate)
        XCTAssertEqual(unplugged.configs["EXT-UUID-27"], stored["EXT-UUID-27"])

        // Replug external monitor -> EXT-UUID-27 restored with exact previous config
        let replugged = DisplayConfigurationPlanner.resolvedConfigurations(
            stored: unplugged.configs,
            displays: twoDisplays,
            displayChoice: "auto",
            legacyMode: .notch,
            legacyTopOffset: 10
        )
        XCTAssertFalse(replugged.didMigrate)
        XCTAssertEqual(replugged.configs["EXT-UUID-27"], stored["EXT-UUID-27"])
    }

    func testUnseenClamshellMonitorFallsBackWhileExplicitAllOffIsRespected() {
        let storedMacBookOnly: [String: DisplayConfiguration] = [
            "builtin": DisplayConfiguration(enabled: true, displayMode: .notch, dynamicIslandTopOffset: 10)
        ]

        // Clamshell on unseen external monitor -> initializes unseen monitor as enabled
        let clamshell = DisplayConfigurationPlanner.resolvedConfigurations(
            stored: storedMacBookOnly,
            displays: [unseenClamshellMonitor],
            displayChoice: "auto",
            legacyMode: .dynamicIsland,
            legacyTopOffset: 14
        )
        XCTAssertTrue(clamshell.didMigrate)
        XCTAssertEqual(clamshell.configs["EXT-UUID-NEW"]?.enabled, true)
        XCTAssertEqual(clamshell.configs["EXT-UUID-NEW"]?.displayMode, .dynamicIsland)
        XCTAssertEqual(clamshell.configs["EXT-UUID-NEW"]?.dynamicIslandTopOffset, 14)
        XCTAssertEqual(clamshell.configs["builtin"]?.enabled, true)

        // Explicit all-off on connected displays -> respected
        let explicitAllOff: [String: DisplayConfiguration] = [
            "builtin": DisplayConfiguration(enabled: false, displayMode: .notch, dynamicIslandTopOffset: 10),
            "EXT-UUID-27": DisplayConfiguration(enabled: false, displayMode: .dynamicIsland, dynamicIslandTopOffset: 10),
        ]
        let resolvedAllOff = DisplayConfigurationPlanner.resolvedConfigurations(
            stored: explicitAllOff,
            displays: [macBook, external27],
            displayChoice: "auto",
            legacyMode: .notch,
            legacyTopOffset: 10
        )
        XCTAssertFalse(resolvedAllOff.didMigrate)
        XCTAssertNil(DisplayConfigurationPlanner.primaryEnabledDisplayID(displays: [macBook, external27], configs: resolvedAllOff.configs))
    }

    func testResilientPerEntryDecodingPreservesValidSiblingsAndRejectsEmptyOrInvalidJSON() throws {
        let partialPoisonJSON = """
        {
          "builtin": {
            "enabled": true,
            "displayMode": "notch",
            "dynamicIslandTopOffset": 12
          },
          "EXT-CORRUPT": {
            "enabled": "not_a_bool"
          },
          "EXT-ARRAY": [1, 2, 3],
          "EXT-SCALAR": 42
        }
        """
        let decoded = try XCTUnwrap(DisplayConfigurationCodec.decode(partialPoisonJSON))
        XCTAssertEqual(decoded.count, 1)
        XCTAssertEqual(
            decoded["builtin"],
            DisplayConfiguration(enabled: true, displayMode: .notch, dynamicIslandTopOffset: 12)
        )
        XCTAssertNil(decoded["EXT-CORRUPT"])

        XCTAssertNil(DisplayConfigurationCodec.decode(#"{"EXT-CORRUPT":{"enabled":"not_a_bool"}}"#))
        XCTAssertNil(DisplayConfigurationCodec.decode("{}"))
        XCTAssertNil(DisplayConfigurationCodec.decode("[1,2,3]"))
        XCTAssertNil(DisplayConfigurationCodec.decode("null"))
    }
}
