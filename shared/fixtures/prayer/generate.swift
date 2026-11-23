// Prints the prayer fixture matrix as JSON on stdout, from the real Swift
// implementation. Redirect it over shared/fixtures/prayer/expected.json to
// re-verify the Rust numbers in morrow-core/tests/prayer_conformance.rs:
//
//   swiftc -O \
//     macos/Sources/MorrowCore/JulianDayCalculator.swift \
//     macos/Sources/MorrowCore/PrayerCalculationMethod.swift \
//     macos/Sources/MorrowCore/PrayerTimeCalculator.swift \
//     macos/Sources/MorrowCore/DailyPrayerSchedule.swift \
//     shared/fixtures/prayer/generate.swift -o /tmp/prayer-generate
//   /tmp/prayer-generate > shared/fixtures/prayer/expected.json
//
// The clock and the UTC offset are injected (fixed-offset TimeZone), so the
// output is deterministic and needs no location services or network.

import Foundation

@main
struct PrayerFixtureGenerator {
    struct City {
        let name: String
        let latitude: Double
        let longitude: Double
        let elevationMeters: Double
    }

    static let cities: [City] = [
        City(name: "Jakarta", latitude: -6.2088, longitude: 106.8456, elevationMeters: 10),
        City(name: "Singapore", latitude: 1.3521, longitude: 103.8198, elevationMeters: 15),
        City(name: "Mecca", latitude: 21.4225, longitude: 39.8262, elevationMeters: 277),
        City(name: "Oslo", latitude: 59.9139, longitude: 10.7522, elevationMeters: 23),
        City(name: "London", latitude: 51.5074, longitude: -0.1278, elevationMeters: 11),
    ]

    // Ramadan 1447 (18/19 Feb 2026) and the 2026 DST transitions are included.
    static let dates: [(year: Int, month: Int, day: Int)] = [
        (2026, 1, 15), (2026, 2, 18), (2026, 2, 19), (2026, 3, 8),
        (2026, 3, 20), (2026, 3, 29), (2026, 4, 15), (2026, 6, 21),
        (2026, 9, 27), (2026, 10, 25), (2026, 12, 21), (2026, 12, 31),
    ]

    static let methods: [PrayerCalculationMethod] = [.kemenag, .nuLFPBNU, .muhammadiyah]
    static let prayerTypes: [PrayerType] = [
        .imsak, .subuh, .terbit, .dhuha, .dzuhur, .ashar, .maghrib, .isya,
    ]

    /// Fixed offsets mirror the ones injected into the Rust conformance test.
    /// EU/UK summer time in 2026 runs 29 Mar through 25 Oct; on the transition
    /// days the post-transition offset is used.
    static func offsetSeconds(city: String, date: (year: Int, month: Int, day: Int)) -> Int {
        switch city {
        case "Jakarta": return 7 * 3600
        case "Singapore": return 8 * 3600
        case "Mecca": return 3 * 3600
        case "Oslo", "London":
            let summer = date.month > 3 || (date.month == 3 && date.day >= 29)
                || date.month < 10 || (date.month == 10 && date.day <= 25)
            if city == "Oslo" { return summer ? 2 * 3600 : 3600 }
            return summer ? 3600 : 0
        default: return 0
        }
    }

    static func clockAndEpoch(_ date: Date, calendar: Calendar) -> (Int, Int) {
        let parts = calendar.dateComponents([.hour, .minute], from: date)
        let minutes = (parts.hour ?? 0) * 60 + (parts.minute ?? 0)
        return (minutes, Int(date.timeIntervalSince1970.rounded()))
    }

    static func main() {
        var cases: [[String: Any]] = []

        for city in cities {
            for date in dates {
                for method in methods {
                    let offset = offsetSeconds(city: city.name, date: date)
                    let timeZone = TimeZone(secondsFromGMT: offset)!
                    var calendar = Calendar(identifier: .gregorian)
                    calendar.timeZone = timeZone

                    let schedule = PrayerTimeCalculator.calculate(
                        day: YearMonthDay(year: date.year, month: date.month, day: date.day),
                        coordinates: Coordinates(latitude: city.latitude, longitude: city.longitude),
                        elevationMeters: city.elevationMeters,
                        method: method,
                        timeZone: timeZone
                    )

                    var minutes: [Int] = []
                    var epoch: [Int] = []
                    for prayer in prayerTypes {
                        let (minuteValue, epochValue) = clockAndEpoch(
                            schedule.time(for: prayer), calendar: calendar
                        )
                        minutes.append(minuteValue)
                        epoch.append(epochValue)
                    }

                    cases.append([
                        "city": city.name,
                        "date": [date.year, date.month, date.day],
                        "method": method.rawValue,
                        "utc_offset_seconds": offset,
                        "minutes": minutes,
                        "epoch": epoch,
                    ])
                }
            }
        }

        let fixture: [String: Any] = [
            "order": prayerTypes.map(\.rawValue),
            "cities": cities.map { [
                "name": $0.name,
                "latitude": $0.latitude,
                "longitude": $0.longitude,
                "elevation_meters": $0.elevationMeters,
            ] },
            "dates": dates.map { [$0.year, $0.month, $0.day] },
            "methods": methods.map(\.rawValue),
            "cases": cases,
        ]

        let data = try! JSONSerialization.data(withJSONObject: fixture, options: [.sortedKeys])
        print(String(decoding: data, as: UTF8.self))
    }
}
