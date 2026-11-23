//! Every row of the prayer matrix from the Swift implementation.
//!
//! `shared/fixtures/prayer/expected.json` IS the output of
//! `shared/fixtures/prayer/generate.swift` (5 cities x 3 methods x 12 dates),
//! compiled against the real Swift sources with `swiftc -O` and redirected
//! verbatim into the fixture. The Rust calculator below must match those
//! numbers to the minute — the expectation never comes from the code it tests.

use morrow_core::prayer::{
    Coordinates, PrayerCalculationMethod, PrayerTimeCalculator, PrayerType, YearMonthDay,
};
use serde::Deserialize;

const FIXTURE: &str = include_str!("../../../fixtures/prayer/expected.json");

#[derive(Debug, Deserialize)]
struct City {
    name: String,
    latitude: f64,
    longitude: f64,
    elevation_meters: f64,
}

#[derive(Debug, Deserialize)]
struct Case {
    city: String,
    date: (i32, i32, i32),
    method: PrayerCalculationMethod,
    utc_offset_seconds: i32,
    minutes: Vec<i64>,
    epoch: Vec<i64>,
}

#[derive(Debug, Deserialize)]
struct Fixture {
    order: Vec<String>,
    cities: Vec<City>,
    dates: Vec<(i32, i32, i32)>,
    methods: Vec<String>,
    cases: Vec<Case>,
}

fn fixture() -> Fixture {
    serde_json::from_str(FIXTURE).expect("prayer fixture JSON")
}

#[test]
fn fixture_matrix_is_complete() {
    let fixture = fixture();
    assert_eq!(
        fixture.cases.len(),
        fixture.cities.len() * fixture.methods.len() * fixture.dates.len()
    );
    assert_eq!(fixture.cities.len(), 5);
    assert_eq!(fixture.methods.len(), 3);
    assert_eq!(fixture.dates.len(), 12);

    // The eight values in `minutes`/`epoch` follow `PrayerType::ALL`.
    let raw_order: Vec<String> = PrayerType::ALL
        .iter()
        .map(|prayer| {
            serde_json::to_string(prayer)
                .unwrap()
                .trim_matches('"')
                .to_owned()
        })
        .collect();
    assert_eq!(fixture.order, raw_order);
}

#[test]
fn swift_prayer_matrix_matches() {
    let fixture = fixture();
    for case in &fixture.cases {
        let city = fixture
            .cities
            .iter()
            .find(|city| city.name == case.city)
            .unwrap_or_else(|| panic!("unknown city {}", case.city));
        let schedule = PrayerTimeCalculator::calculate(
            YearMonthDay::new(case.date.0, case.date.1, case.date.2),
            Coordinates::new(city.latitude, city.longitude),
            city.elevation_meters,
            case.method,
            case.utc_offset_seconds,
        );

        let label = format!("{}/{:?}/{:?}", case.city, case.date, case.method);
        for (index, prayer) in PrayerType::ALL.iter().enumerate() {
            let moment = schedule.time(*prayer);
            assert_eq!(
                moment.local_minutes,
                case.minutes[index],
                "{label} {} local minutes",
                prayer.display_name()
            );
            assert_eq!(
                moment.epoch_seconds,
                case.epoch[index],
                "{label} {} epoch seconds",
                prayer.display_name()
            );
        }
    }
}

#[test]
fn summer_isha_wraps_to_the_same_local_day() {
    // Swift builds DateComponents with hour % 24, so a >24h Isha wraps.
    let fixture = fixture();
    let wrapped: Vec<&Case> = fixture
        .cases
        .iter()
        .filter(|case| {
            case.minutes[PrayerType::Isya as usize] < case.minutes[PrayerType::Maghrib as usize]
        })
        .collect();
    assert!(
        wrapped
            .iter()
            .any(|case| case.city == "Oslo" && case.date == (2026, 6, 21)),
        "fixture must keep the high-latitude wrap case"
    );
}
