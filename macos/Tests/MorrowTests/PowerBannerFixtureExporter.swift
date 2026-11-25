import Foundation
import XCTest

@testable import Morrow

/// `fixtures/core/power_banner.json` is what the macOS power code actually does
/// with a reading: the banner title and subtitle, the threshold chain, the
/// adapter-name classification and the network speed format. IOKit, colours and
/// glyphs stay in the shell.
///
/// `CODEISLAND_POWER_EXPORT_JSON=<fixtures dir>` rewrites the file; without it
/// this test only checks the file on disk is still what Swift produces. Every
/// expected value comes from calling the app's own code, not from a table
/// someone retyped out of a test.
final class PowerBannerFixtureExporter: XCTestCase {
    private struct ResolverCase {
        let name: String
        let kind: PowerSourceKind
        let isCharging: Bool
        let isAC: Bool
        let isCharged: Bool
        let batteryLevel: Int
        let adapterWatts: Int?
        let rawTimeToFullMinutes: Int?
    }

    private struct AdapterCase {
        let name: String
        let adapterName: String?
        let source: String?
        let description: String?
        let family: Int
        let magSafeConnected: Bool
    }

    private static func metadataCases() -> [(String, PowerBannerState)] {
        [
            ("charging_magsafe_140w_28m", .charging(kind: .magSafe, watts: 140, timeToFullMinutes: 28)),
            ("charging_usbc_watts_only", .charging(kind: .usbC, watts: 96, timeToFullMinutes: nil)),
            ("charging_over_an_hour", .charging(kind: .magSafe, watts: nil, timeToFullMinutes: 90)),
            ("charging_exactly_an_hour", .charging(kind: .usbC, watts: nil, timeToFullMinutes: 60)),
            ("charging_omits_zero_and_negative", .charging(kind: .magSafe, watts: 0, timeToFullMinutes: -1)),
            ("on_hold_usbc", .onHold(kind: .usbC)),
            ("fully_charged_magsafe", .fullyCharged(kind: .magSafe)),
            ("low_battery_15", .lowBattery(percent: 15)),
        ]
    }

    private static func resolverCases() -> [ResolverCase] {
        [
            ResolverCase(name: "charged_flag_at_80", kind: .magSafe, isCharging: false, isAC: true, isCharged: true, batteryLevel: 80, adapterWatts: nil, rawTimeToFullMinutes: nil),
            ResolverCase(name: "full_at_99_boundary", kind: .usbC, isCharging: false, isAC: true, isCharged: false, batteryLevel: 99, adapterWatts: 30, rawTimeToFullMinutes: 30),
            ResolverCase(name: "on_hold_at_98_ignores_watts_and_time", kind: .usbC, isCharging: false, isAC: true, isCharged: false, batteryLevel: 98, adapterWatts: 140, rawTimeToFullMinutes: 30),
            ResolverCase(name: "low_battery_at_20_boundary", kind: .magSafe, isCharging: false, isAC: false, isCharged: false, batteryLevel: 20, adapterWatts: nil, rawTimeToFullMinutes: nil),
            ResolverCase(name: "not_low_at_21", kind: .magSafe, isCharging: false, isAC: false, isCharged: false, batteryLevel: 21, adapterWatts: nil, rawTimeToFullMinutes: nil),
            ResolverCase(name: "full_battery_100_off_ac", kind: .magSafe, isCharging: false, isAC: false, isCharged: false, batteryLevel: 100, adapterWatts: nil, rawTimeToFullMinutes: nil),
            ResolverCase(name: "charging_when_is_charging", kind: .usbC, isCharging: true, isAC: true, isCharged: false, batteryLevel: 50, adapterWatts: 140, rawTimeToFullMinutes: 28),
            ResolverCase(name: "negative_time_to_full_dropped", kind: .magSafe, isCharging: true, isAC: true, isCharged: false, batteryLevel: 40, adapterWatts: nil, rawTimeToFullMinutes: -5),
            ResolverCase(name: "zero_time_to_full_dropped", kind: .magSafe, isCharging: true, isAC: true, isCharged: false, batteryLevel: 40, adapterWatts: nil, rawTimeToFullMinutes: 0),
            ResolverCase(name: "full_ignores_watts_and_time", kind: .magSafe, isCharging: false, isAC: true, isCharged: true, batteryLevel: 100, adapterWatts: 140, rawTimeToFullMinutes: 30),
        ]
    }

    private static func adapterCases() -> [AdapterCase] {
        [
            AdapterCase(name: "mag_safe_port_connected_wins", adapterName: "USB-C Power Adapter", source: nil, description: nil, family: 0xE000400A, magSafeConnected: true),
            AdapterCase(name: "mag_safe_in_name", adapterName: "MagSafe Power Adapter", source: nil, description: nil, family: 0xE000400A, magSafeConnected: false),
            AdapterCase(name: "mag_safe_in_description", adapterName: nil, source: nil, description: "MagSafe charger", family: 0xE0004007, magSafeConnected: false),
            AdapterCase(name: "source_used_when_name_missing", adapterName: nil, source: "USB-C Power Adapter", description: nil, family: 0, magSafeConnected: false),
            AdapterCase(name: "usb_in_name", adapterName: "USB Adapter", source: nil, description: nil, family: 0, magSafeConnected: false),
            AdapterCase(name: "pd_in_description", adapterName: nil, source: nil, description: "USB PD", family: 0, magSafeConnected: false),
            AdapterCase(name: "type_c_in_name", adapterName: "Type-C Charger", source: nil, description: nil, family: 0, magSafeConnected: false),
            AdapterCase(name: "family_e000400a", adapterName: nil, source: nil, description: nil, family: 0xE000400A, magSafeConnected: false),
            AdapterCase(name: "family_e0004007", adapterName: nil, source: nil, description: nil, family: 0xE0004007, magSafeConnected: false),
            AdapterCase(name: "unknown_family_defaults_mag_safe", adapterName: nil, source: nil, description: nil, family: 0, magSafeConnected: false),
            AdapterCase(name: "empty_name_does_not_fall_back_to_source", adapterName: "", source: "USB-C Power Adapter", description: nil, family: 0, magSafeConnected: false),
        ]
    }

    private static func speedCases() -> [(String, Double)] {
        [
            ("zero", 0),
            ("just_below_kilo", 999),
            ("one_kilo", 1024),
            ("two_kilo", 2048),
            ("quarter_mega", 262_144),
            ("one_mega", 1_048_576),
            ("one_and_half_mega", 1_572_864),
            ("ten_mega", 10_485_760),
            ("negative", -1),
        ]
    }

    private static func stateJSON(_ state: PowerBannerState) -> [String: Any] {
        var object: [String: Any]
        switch state {
        case .charging(let kind, let watts, let mins):
            object = ["tag": "charging", "kind": kind.rawValue]
            if let watts { object["watts"] = watts }
            if let mins { object["time_to_full"] = mins }
        case .onHold(let kind):
            object = ["tag": "onHold", "kind": kind.rawValue]
        case .fullyCharged(let kind):
            object = ["tag": "fullyCharged", "kind": kind.rawValue]
        case .lowBattery(let percent):
            object = ["tag": "lowBattery", "percent": percent]
        }
        object["title"] = state.title
        object["subtitle"] = state.subtitle
        return object
    }

    @MainActor private static func document() -> [String: Any] {
        let metadata = Self.metadataCases().map { name, state in
            ["name": name, "expected": Self.stateJSON(state)]
        }
        let resolved = Self.resolverCases().map { c in
            [
                "name": c.name,
                "kind": c.kind.rawValue,
                "is_charging": c.isCharging,
                "is_ac": c.isAC,
                "is_charged": c.isCharged,
                "battery_level": c.batteryLevel,
                "adapter_watts": c.adapterWatts as Any,
                "raw_time_to_full": c.rawTimeToFullMinutes as Any,
                "expected": Self.stateJSON(PowerBannerResolver.state(
                    kind: c.kind,
                    isCharging: c.isCharging,
                    isAC: c.isAC,
                    isCharged: c.isCharged,
                    batteryLevel: c.batteryLevel,
                    adapterWatts: c.adapterWatts,
                    rawTimeToFullMinutes: c.rawTimeToFullMinutes
                )),
            ]
        }
        let adapters = Self.adapterCases().map { c in
            [
                "name": c.name,
                "adapter_name": c.adapterName as Any,
                "source": c.source as Any,
                "description": c.description as Any,
                "family": c.family,
                "mag_safe_connected": c.magSafeConnected,
                "kind": PowerBannerResolver.adapterKind(
                    name: c.adapterName,
                    source: c.source,
                    description: c.description,
                    family: c.family,
                    magSafeConnected: c.magSafeConnected
                ).rawValue,
            ]
        }
        let speeds = Self.speedCases().map { name, bytes in
            ["name": name, "bytes_per_sec": bytes, "formatted": SystemMonitorManager.formatSpeed(bytes)]
        }
        return [
            "source": "macos/Sources/Morrow/SystemMonitorManager.swift",
            "metadata_cases": metadata,
            "resolver_cases": resolved,
            "adapter_cases": adapters,
            "speed_cases": speeds,
        ]
    }

    @MainActor func testExportedPowerBannerFixturesMatchSwift() throws {
        var out = try JSONSerialization.data(
            withJSONObject: Self.document(),
            options: [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        )
        out.append(0x0A)

        let path = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("shared/contract/fixtures/core/power_banner.json")

        guard let exportDir = ProcessInfo.processInfo.environment["CODEISLAND_POWER_EXPORT_JSON"]
        else {
            let onDisk = try Data(contentsOf: path)
            XCTAssertEqual(
                String(decoding: onDisk, as: UTF8.self),
                String(decoding: out, as: UTF8.self),
                "fixtures/core/power_banner.json is stale: re-export with CODEISLAND_POWER_EXPORT_JSON"
            )
            return
        }

        let dir = URL(fileURLWithPath: exportDir, isDirectory: true)
            .appendingPathComponent("core", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        try out.write(to: dir.appendingPathComponent("power_banner.json"))
        throw XCTSkip("exported the power banner fixtures to \(exportDir)/core")
    }
}
