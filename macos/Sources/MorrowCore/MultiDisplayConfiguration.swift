import Foundation

/// Per-display presentation settings persisted across display reconnects.
public struct DisplayConfiguration: Codable, Equatable, Sendable {
    public var enabled: Bool
    public var displayMode: IslandDisplayMode
    public var dynamicIslandTopOffset: Double

    public init(
        enabled: Bool,
        displayMode: IslandDisplayMode,
        dynamicIslandTopOffset: Double = DynamicIslandTopOffset.defaultValue
    ) {
        self.enabled = enabled
        self.displayMode = displayMode
        self.dynamicIslandTopOffset = DynamicIslandTopOffset.clamp(dynamicIslandTopOffset)
    }

    private enum CodingKeys: String, CodingKey {
        case enabled
        case displayMode
        case dynamicIslandTopOffset
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let enabled = try container.decodeIfPresent(Bool.self, forKey: .enabled) ?? true
        let modeRaw = try container.decodeIfPresent(String.self, forKey: .displayMode)
        let topOffset = try container.decodeIfPresent(Double.self, forKey: .dynamicIslandTopOffset)
            ?? DynamicIslandTopOffset.defaultValue

        self.init(
            enabled: enabled,
            displayMode: IslandDisplayMode.resolved(from: modeRaw),
            dynamicIslandTopOffset: topOffset
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(enabled, forKey: .enabled)
        try container.encode(displayMode.rawValue, forKey: .displayMode)
        try container.encode(DynamicIslandTopOffset.clamp(dynamicIslandTopOffset), forKey: .dynamicIslandTopOffset)
    }
}

/// Platform-agnostic descriptor of a connected screen used by settings UI,
/// persistence planning, and multi-display tests.
public struct DisplayDescriptor: Identifiable, Equatable, Sendable {
    /// Stable hardware-backed identifier: `"builtin"`, persistent display UUID string,
    /// or `"display_<cgID>"` fallback.
    public let id: String
    /// Zero-based position matching `NSScreen.screens` order.
    public let index: Int
    public let name: String
    public let width: Int
    public let height: Int
    public let hasNotch: Bool
    public let isBuiltin: Bool
    public let isPrimary: Bool

    public init(
        id: String,
        index: Int,
        name: String,
        width: Int,
        height: Int,
        hasNotch: Bool,
        isBuiltin: Bool,
        isPrimary: Bool
    ) {
        self.id = id
        self.index = index
        self.name = name
        self.width = width
        self.height = height
        self.hasNotch = hasNotch
        self.isBuiltin = isBuiltin
        self.isPrimary = isPrimary
    }

    public var resolutionLabel: String {
        "\(width)×\(height)"
    }

    public static func defaultMode(hasNotch: Bool) -> IslandDisplayMode {
        hasNotch ? .notch : .dynamicIsland
    }

    public static func defaultConfiguration(
        hasNotch: Bool,
        enabled: Bool = false,
        topOffset: Double = DynamicIslandTopOffset.defaultValue
    ) -> DisplayConfiguration {
        DisplayConfiguration(
            enabled: enabled,
            displayMode: defaultMode(hasNotch: hasNotch),
            dynamicIslandTopOffset: topOffset
        )
    }
}

/// UTF-8 JSON codec shared by `@AppStorage(SettingsKey.displayConfigurations)`
/// and `SettingsManager` so SwiftUI and AppKit read/write the identical format.
public enum DisplayConfigurationCodec {
    private struct LossyDisplayConfigurationEntry: Decodable {
        let value: DisplayConfiguration?

        init(from decoder: Decoder) throws {
            self.value = try? DisplayConfiguration(from: decoder)
        }
    }

    public static func decode(_ raw: String?) -> [String: DisplayConfiguration]? {
        guard let raw else { return nil }
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, let data = trimmed.data(using: .utf8) else {
            return nil
        }
        return decode(data: data)
    }

    public static func decode(data: Data?) -> [String: DisplayConfiguration]? {
        guard let data, !data.isEmpty,
              let entries = try? JSONDecoder().decode([String: LossyDisplayConfigurationEntry].self, from: data) else {
            return nil
        }
        let decoded = entries.compactMapValues(\.value)
        return decoded.isEmpty ? nil : decoded
    }

    public static func encode(_ configurations: [String: DisplayConfiguration]) -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        guard let data = try? encoder.encode(configurations),
              let string = String(data: data, encoding: .utf8) else {
            return "{}"
        }
        return string
    }
}

/// Pure migration, resolution, and primary-display helpers for multi-display settings.
public enum DisplayConfigurationPlanner {
    public static func legacyTargetIndex(
        displayChoice: String,
        displays: [DisplayDescriptor]
    ) -> Int? {
        guard !displays.isEmpty else { return nil }

        if displayChoice.hasPrefix("screen_"),
           let index = Int(displayChoice.dropFirst(7)),
           index >= 0,
           index < displays.count {
            return index
        }

        if displayChoice == "main" {
            return 0
        }

        if let notchIndex = displays.firstIndex(where: \.hasNotch) {
            return notchIndex
        }

        if let builtinIndex = displays.firstIndex(where: \.isBuiltin) {
            return builtinIndex
        }

        return 0
    }

    public static func migrateLegacy(
        displays: [DisplayDescriptor],
        displayChoice: String,
        legacyMode: IslandDisplayMode,
        legacyTopOffset: Double
    ) -> [String: DisplayConfiguration] {
        guard !displays.isEmpty else { return [:] }
        let targetIndex = legacyTargetIndex(displayChoice: displayChoice, displays: displays) ?? 0
        var result: [String: DisplayConfiguration] = [:]
        for (index, display) in displays.enumerated() {
            if index == targetIndex {
                result[display.id] = DisplayConfiguration(
                    enabled: true,
                    displayMode: legacyMode,
                    dynamicIslandTopOffset: legacyTopOffset
                )
            } else {
                result[display.id] = DisplayDescriptor.defaultConfiguration(
                    hasNotch: display.hasNotch,
                    enabled: false
                )
            }
        }
        return result
    }

    public static func resolvedConfigurations(
        stored: [String: DisplayConfiguration]?,
        displays: [DisplayDescriptor],
        displayChoice: String,
        legacyMode: IslandDisplayMode,
        legacyTopOffset: Double
    ) -> (configs: [String: DisplayConfiguration], didMigrate: Bool) {
        guard let stored else {
            guard !displays.isEmpty else { return ([:], false) }
            let migrated = migrateLegacy(
                displays: displays,
                displayChoice: displayChoice,
                legacyMode: legacyMode,
                legacyTopOffset: legacyTopOffset
            )
            return (migrated, true)
        }

        guard !displays.isEmpty else {
            return (stored, false)
        }

        let hasKnownConnectedDisplay = displays.contains { stored[$0.id] != nil }
        if !hasKnownConnectedDisplay {
            // Clamshell / first-time external monitor with no previously seen display connected:
            // preserve prior stored keys while initializing the unseen display(s) so the user
            // is never stranded with 0 visible islands.
            var merged = stored
            let fallback = migrateLegacy(
                displays: displays,
                displayChoice: displayChoice,
                legacyMode: legacyMode,
                legacyTopOffset: legacyTopOffset
            )
            for (key, value) in fallback {
                merged[key] = value
            }
            return (merged, true)
        }

        var merged = stored
        var didChange = false
        for display in displays where merged[display.id] == nil {
            merged[display.id] = DisplayDescriptor.defaultConfiguration(
                hasNotch: display.hasNotch,
                enabled: false
            )
            didChange = true
        }
        return (merged, didChange)
    }

    public static func primaryEnabledDisplayID(
        displays: [DisplayDescriptor],
        configs: [String: DisplayConfiguration]
    ) -> String? {
        displays.first { configs[$0.id]?.enabled == true }?.id
    }
}
