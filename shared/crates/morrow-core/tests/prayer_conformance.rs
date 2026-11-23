//! Every row of the prayer matrix from the Swift implementation.
//!
//! `shared/fixtures/prayer/expected.json` IS the output of
//! `shared/fixtures/prayer/generate.swift` (5 cities x 3 methods x 12 dates),
//! compiled against the real Swift sources with `swiftc -O` and redirected
//! verbatim into the fixture. The Rust calculator below must match those
//! numbers to the minute — the expectation never comes from the code it tests.

use morrow_core::prayer::{
    Coordinates, HijriCalendarCalculator, HijriCalendarSource, PrayerCalculationMethod,
    PrayerTimeCalculator, PrayerType, YearMonthDay, hijri_month_name,
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
struct HijriCase {
    date: (i32, i32, i32),
    source: HijriCalendarSource,
    day: i32,
    month: i32,
    year: i32,
    month_name: String,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Fixture {
    hijri: Vec<HijriCase>,
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

#[test]
fn swift_hijri_gold_matches_for_both_criteria_and_the_leap_month() {
    let fixture = fixture();
    assert_eq!(fixture.hijri.len(), 28, "14 dates x 2 criteria");
    for case in &fixture.hijri {
        let value = HijriCalendarCalculator::calculate(
            YearMonthDay::new(case.date.0, case.date.1, case.date.2),
            case.source,
            0,
        );
        let label = format!("{:?}/{:?}", case.date, case.source);
        assert_eq!(value.day, case.day, "{label} day");
        assert_eq!(value.month, case.month, "{label} month");
        assert_eq!(value.year, case.year, "{label} year");
        assert_eq!(value.month_name, case.month_name, "{label} name");
        assert_eq!(
            hijri_month_name(case.month),
            case.month_name,
            "{label} name function"
        );
    }

    // #296 AC: the named leap-year pair is a cross-language gold, not just a
    // Rust unit test — KHGT's 1448 Dhu al-Hijjah has 30 days.
    let last = fixture
        .hijri
        .iter()
        .find(|case| case.source == HijriCalendarSource::Khgt && case.date == (2027, 6, 5))
        .expect("named leap-year case");
    assert_eq!((last.day, last.month, last.year), (30, 12, 1448));
    assert!(
        last.note
            .as_deref()
            .unwrap_or("")
            .contains("KHGT leap month")
    );
    let next = fixture
        .hijri
        .iter()
        .find(|case| case.source == HijriCalendarSource::Khgt && case.date == (2027, 6, 6))
        .expect("day after the leap month");
    assert_eq!((next.day, next.month, next.year), (1, 1, 1449));
}
