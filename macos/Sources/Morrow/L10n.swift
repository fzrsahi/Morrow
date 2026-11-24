import Foundation
import Combine

final class L10n: ObservableObject {
    static let shared = L10n()

    @Published var language: String {
        didSet { UserDefaults.standard.set(language, forKey: SettingsKey.appLanguage) }
    }

    init() {
        self.language = UserDefaults.standard.string(forKey: SettingsKey.appLanguage) ?? "system"
    }

    var effectiveLanguage: String {
        if language == "system" {
            let preferred = Locale.preferredLanguages.first ?? "en"
            if preferred.hasPrefix("zh-Hant") || preferred.hasPrefix("zh-TW")
                || preferred.hasPrefix("zh-HK") || preferred.hasPrefix("zh-MO") { return "zh-Hant" }
            if preferred.hasPrefix("zh") { return "zh" }
            if preferred.hasPrefix("de") { return "de" }
            if preferred.hasPrefix("ja") { return "ja" }
            if preferred.hasPrefix("tr") { return "tr" }
            if preferred.hasPrefix("ko") { return "ko" }
            return "en"
        }
        return language
    }

    subscript(_ key: String) -> String {
        Self.strings[effectiveLanguage]?[key] ?? Self.strings["en"]?[key] ?? key
    }

    // MARK: - Translations

    static let strings: [String: [String: String]] = [
        "en": en,
        "zh": zh,
        "zh-Hant": zhHant,
        "de": de,
        "ja": ja,
        "ko": ko,
        "tr": tr,
    ]

    // The tables live in L10n+Strings.swift, generated from
    // shared/contract/strings/*.json (#239 AC 4). Editing a string means editing
    // that JSON and running ./scripts/ci/strings-codegen.sh.
}
