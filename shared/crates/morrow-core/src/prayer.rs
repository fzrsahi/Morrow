//! The pure calculation core of the prayer feature: `JulianDayCalculator.swift`,
//! `PrayerCalculationMethod.swift`, `PrayerTimeCalculator.swift`,
//! `HijriCalendarCalculator.swift` and `DailyPrayerSchedule.swift` in
//! `macos/Sources/MorrowCore`.
//!
//! Nothing here reads a clock, a time zone database or the network. The
//! observer's UTC offset, the day and the coordinates are passed in, so every
//! function is deterministic. `PrayerTimeCalculator::calculate` returns the
//! same eight milestones Swift's `DailyPrayerSchedule` carries; a milestone
//! holds both the local clock minutes Swift computed and the absolute epoch
//! instant those `DateComponents` name at the injected offset.
//!
//! Every case of the Swift matrix lives in
//! `shared/fixtures/prayer/expected.json`; the integration test
//! `tests/prayer_conformance.rs` checks all 180 of them.

use serde::{Deserialize, Serialize};

/// `Coordinates` in `PrayerCalculationMethod.swift`: decimal degrees.
/// `f64` is neither `Eq` nor `Hash`, so this mirrors Swift's `Equatable` only.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct Coordinates {
    pub latitude: f64,
    pub longitude: f64,
}

impl Coordinates {
    pub fn new(latitude: f64, longitude: f64) -> Self {
        Self {
            latitude,
            longitude,
        }
    }
}

/// `YearMonthDay` in `PrayerCalculationMethod.swift`: a calendar day with no
/// time zone attached. Derives the same lexicographic ordering as Swift's
/// `Comparable` conformance.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct YearMonthDay {
    pub year: i32,
    pub month: i32,
    pub day: i32,
}

impl YearMonthDay {
    pub fn new(year: i32, month: i32, day: i32) -> Self {
        Self { year, month, day }
    }
}

/// `PrayerCalculationMethod` in `PrayerCalculationMethod.swift`. The serde
/// names are the Swift enum's raw values.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum PrayerCalculationMethod {
    #[serde(rename = "kemenag")]
    Kemenag,
    #[serde(rename = "nuLFPBNU")]
    NuLfpbnu,
    #[serde(rename = "muhammadiyah")]
    Muhammadiyah,
}

impl PrayerCalculationMethod {
    pub const ALL: [Self; 3] = [Self::Kemenag, Self::NuLfpbnu, Self::Muhammadiyah];

    /// `displayName` in Swift.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Kemenag => "Kemenag RI",
            Self::NuLfpbnu => "Nahdlatul Ulama (LF PBNU)",
            Self::Muhammadiyah => "Muhammadiyah (MTT)",
        }
    }

    /// `fajrAngle` in Swift: Kemenag and NU use -20°, Muhammadiyah -18°.
    pub fn fajr_angle(self) -> f64 {
        match self {
            Self::Kemenag | Self::NuLfpbnu => 20.0,
            Self::Muhammadiyah => 18.0,
        }
    }

    /// `ishaAngle` in Swift: -18° for every method.
    pub fn isha_angle(self) -> f64 {
        18.0
    }

    /// `dhuhaAngle` in Swift: +4.5°.
    pub fn dhuha_angle(self) -> f64 {
        4.5
    }

    /// `dhuhrMinutesAfterTransit` in Swift: 2m, 1m, 0m.
    pub fn dhuhr_minutes_after_transit(self) -> f64 {
        match self {
            Self::Kemenag => 2.0,
            Self::NuLfpbnu => 1.0,
            Self::Muhammadiyah => 0.0,
        }
    }

    /// `usesCeilRounding` in Swift: only Kemenag rounds to nearest.
    pub fn uses_ceil_rounding(self) -> bool {
        match self {
            Self::Kemenag => false,
            Self::NuLfpbnu | Self::Muhammadiyah => true,
        }
    }
}

/// `PrayerType` in `PrayerCalculationMethod.swift`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum PrayerType {
    #[serde(rename = "imsak")]
    Imsak,
    #[serde(rename = "subuh")]
    Subuh,
    #[serde(rename = "terbit")]
    Terbit,
    #[serde(rename = "dhuha")]
    Dhuha,
    #[serde(rename = "dzuhur")]
    Dzuhur,
    #[serde(rename = "ashar")]
    Ashar,
    #[serde(rename = "maghrib")]
    Maghrib,
    #[serde(rename = "isya")]
    Isya,
}

impl PrayerType {
    /// The order of `allMilestones` in Swift (and of the fixture's `order`).
    pub const ALL: [Self; 8] = [
        Self::Imsak,
        Self::Subuh,
        Self::Terbit,
        Self::Dhuha,
        Self::Dzuhur,
        Self::Ashar,
        Self::Maghrib,
        Self::Isya,
    ];
    /// The order of `fivePrayers` in Swift.
    pub const FARDH: [Self; 5] = [
        Self::Subuh,
        Self::Dzuhur,
        Self::Ashar,
        Self::Maghrib,
        Self::Isya,
    ];

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Imsak => "Imsak",
            Self::Subuh => "Fajr",
            Self::Terbit => "Sunrise",
            Self::Dhuha => "Duha",
            Self::Dzuhur => "Dhuhr",
            Self::Ashar => "Asr",
            Self::Maghrib => "Maghrib",
            Self::Isya => "Isha",
        }
    }

    /// `isFardh` in Swift: Imsak, Terbit and Duha are milestones, not prayers.
    pub fn is_fardh(self) -> bool {
        matches!(
            self,
            Self::Subuh | Self::Dzuhur | Self::Ashar | Self::Maghrib | Self::Isya
        )
    }
}

/// `JulianDayCalculator.swift`: Meeus' *Astronomical Algorithms*, chapter 7.
/// The returned value is the Julian Day at 0h UT of the given Gregorian day.
pub struct JulianDay;

impl JulianDay {
    /// `JulianDayCalculator.julianDay(year:month:day:)`.
    ///
    /// `day` may be fractional; a month of 1 or 2 is rolled into the previous
    /// year as in the Swift source.
    pub fn calculate(year: i32, month: i32, day: f64) -> f64 {
        let (y, m) = if month <= 2 {
            (year - 1, month + 12)
        } else {
            (year, month)
        };
        let a = (y as f64 / 100.0).floor();
        let b = 2.0 - a + (a / 4.0).floor();
        (365.25 * (y + 4716) as f64).floor() + (30.6001 * (m + 1) as f64).floor() + day + b - 1524.5
    }
}

/// Free-function spelling of [`JulianDay::calculate`].
pub fn julian_day(year: i32, month: i32, day: f64) -> f64 {
    JulianDay::calculate(year, month, day)
}

/// One prayer milestone: the local clock Swift put in `DateComponents`, and
/// the absolute instant those components name at the injected UTC offset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrayerMoment {
    /// Minutes from local midnight, after Swift's `% 24` hour wrapping.
    pub local_minutes: i64,
    /// Seconds since the Unix epoch, for comparisons against an injected time.
    pub epoch_seconds: i64,
}

/// `PrayerTimeCalculator.swift`: a pure, stateless Meeus solar calculation.
pub struct PrayerTimeCalculator;

impl PrayerTimeCalculator {
    /// `PrayerTimeCalculator.calculate(day:coordinates:elevationMeters:method:timeZone:)`.
    ///
    /// `utc_offset_seconds` replaces `timeZone.secondsFromGMT()` in Swift;
    /// the caller supplies it so no time zone database or current date is
    /// consulted here.
    pub fn calculate(
        day: YearMonthDay,
        coordinates: Coordinates,
        elevation_meters: f64,
        method: PrayerCalculationMethod,
        utc_offset_seconds: i32,
    ) -> DailyPrayerSchedule {
        let lat_rad = d2r(coordinates.latitude);
        let lon_deg = coordinates.longitude;
        let tz_hours = utc_offset_seconds as f64 / 3600.0;

        // 1. Julian Day at 12h UT (the Swift source uses the integer day).
        let jd = julian_day(day.year, day.month, day.day as f64);
        let t = (jd - 2_451_545.0) / 36_525.0;

        // 2. Solar coordinates (Meeus).
        let l0 = normalize_degrees(280.46646 + 36_000.769_83 * t + 0.0003032 * t * t);
        let m = normalize_degrees(357.52911 + 35_999.050_29 * t - 0.0001537 * t * t);
        let m_rad = d2r(m);
        let c = (1.914602 - 0.004817 * t - 0.000014 * t * t) * m_rad.sin()
            + (0.019993 - 0.000101 * t) * (2.0 * m_rad).sin()
            + 0.000289 * (3.0 * m_rad).sin();

        let sun_true = l0 + c;
        let omega = 125.04 - 1934.136 * t;
        let lambda = sun_true - 0.00569 - 0.00478 * d2r(omega).sin();

        let eps0 = 23.439291 - 0.0130042 * t - 0.00000016 * t * t + 0.000000504 * t * t * t;
        let eps = eps0 + 0.00256 * d2r(omega).cos();

        let eps_rad = d2r(eps);
        let lambda_rad = d2r(lambda);

        let sin_delta = eps_rad.sin() * lambda_rad.sin();
        let delta_rad = sin_delta.asin();
        let cos_delta = delta_rad.cos();
        let ra_deg = normalize_degrees(r2d(
            (eps_rad.cos() * lambda_rad.sin()).atan2(lambda_rad.cos())
        ));

        // Equation of Time in minutes.
        let mut diff = l0 - ra_deg;
        while diff > 180.0 {
            diff -= 360.0;
        }
        while diff < -180.0 {
            diff += 360.0;
        }
        let eot_minutes = diff * 4.0;

        // Solar transit (Istiwa) in local hours.
        let transit_hours = 12.0 + tz_hours - lon_deg / 15.0 - eot_minutes / 60.0;

        // Hour angle H in hours for a given solar altitude.
        let hour_angle = |altitude_deg: f64| -> f64 {
            let cos_h =
                (d2r(altitude_deg).sin() - lat_rad.sin() * sin_delta) / (lat_rad.cos() * cos_delta);
            r2d(cos_h.clamp(-1.0, 1.0).acos()) / 15.0
        };

        // Subuh & Imsak.
        let h_subuh = hour_angle(-method.fajr_angle());
        let mut subuh_hours = transit_hours - h_subuh;
        if method == PrayerCalculationMethod::Kemenag {
            subuh_hours += 2.0 / 60.0; // +2m ihtiyati
        }
        let imsak_hours = subuh_hours - 10.0 / 60.0;

        // Sunrise (Terbit).
        let (sunrise_alt, sunrise_adjust_mins) = match method {
            PrayerCalculationMethod::Kemenag => {
                (-1.0, -2.0 - kemenag_elevation_minutes(elevation_meters))
            }
            PrayerCalculationMethod::NuLfpbnu => (-0.8333 - apparent_dip(elevation_meters), 0.0),
            PrayerCalculationMethod::Muhammadiyah => (-0.8333, 0.0),
        };
        let terbit_hours = transit_hours - hour_angle(sunrise_alt) + sunrise_adjust_mins / 60.0;

        // Dhuha (+4.5°).
        let mut dhuha_hours = transit_hours - hour_angle(method.dhuha_angle());
        if method == PrayerCalculationMethod::Kemenag {
            dhuha_hours += 2.0 / 60.0;
        }

        // Dzuhur.
        let mut dzuhur_hours = transit_hours + method.dhuhr_minutes_after_transit() / 60.0;
        if method == PrayerCalculationMethod::Kemenag {
            dzuhur_hours += 2.0 / 60.0; // +2m ihtiyati
        }

        // Ashar (Shafi'i shadow multiplier m = 1).
        let asr_alt_deg = r2d((1.0 / (1.0 + (lat_rad - delta_rad).abs().tan())).atan());
        let mut ashar_hours = transit_hours + hour_angle(asr_alt_deg);
        if method == PrayerCalculationMethod::Kemenag {
            ashar_hours += 2.0 / 60.0;
        }

        // Maghrib.
        let (sunset_alt, sunset_adjust_mins) = match method {
            PrayerCalculationMethod::Kemenag => {
                (-1.0, 2.0 + kemenag_elevation_minutes(elevation_meters))
            }
            PrayerCalculationMethod::NuLfpbnu => (-0.8333 - apparent_dip(elevation_meters), 0.0),
            PrayerCalculationMethod::Muhammadiyah => (-0.8333, 0.0),
        };
        let maghrib_hours = transit_hours + hour_angle(sunset_alt) + sunset_adjust_mins / 60.0;

        // Isya.
        let mut isya_hours = transit_hours + hour_angle(-method.isha_angle());
        if method == PrayerCalculationMethod::Kemenag {
            isya_hours += 2.0 / 60.0;
        }

        // Build the moments. Swift feeds `hour = (totalMinutes / 60) % 24`
        // and `minute = totalMinutes % 60` into a Calendar, so a total past
        // midnight wraps to the same day ( Oslo's summer Isha, for instance).
        let jd_epoch = julian_day(1970, 1, 1.0);
        let day_start_epoch =
            ((jd - jd_epoch) * 86_400.0).round() as i64 - utc_offset_seconds as i64;
        let to_moment = |hours: f64| {
            let total = round_or_ceil(hours * 60.0, method);
            let local_minutes = swift_clock_minutes(total);
            PrayerMoment {
                local_minutes,
                epoch_seconds: day_start_epoch + local_minutes * 60,
            }
        };

        DailyPrayerSchedule {
            day,
            method,
            utc_offset_seconds,
            imsak: to_moment(imsak_hours),
            subuh: to_moment(subuh_hours),
            terbit: to_moment(terbit_hours),
            dhuha: to_moment(dhuha_hours),
            dzuhur: to_moment(dzuhur_hours),
            ashar: to_moment(ashar_hours),
            maghrib: to_moment(maghrib_hours),
            isya: to_moment(isya_hours),
        }
    }
}

fn d2r(degrees: f64) -> f64 {
    degrees * std::f64::consts::PI / 180.0
}

fn r2d(radians: f64) -> f64 {
    radians * 180.0 / std::f64::consts::PI
}

/// `normalizeDegrees` in Swift: `truncatingRemainder` then wrap negatives.
fn normalize_degrees(degrees: f64) -> f64 {
    let mut d = degrees % 360.0;
    if d < 0.0 {
        d += 360.0;
    }
    d
}

/// `apparentDip(elevationMeters:)` in Swift: `0.02933 * sqrt(max(0, h))`.
fn apparent_dip(elevation_meters: f64) -> f64 {
    0.02933 * elevation_meters.max(0.0).sqrt()
}

/// `kemenagElevationMinutes(_:)` in Swift's elevation lookup table.
fn kemenag_elevation_minutes(elevation_meters: f64) -> f64 {
    match elevation_meters {
        e if e < 250.0 => 0.0,
        e if e < 700.0 => 1.0,
        e if e < 1000.0 => 2.0,
        e if e < 1500.0 => 3.0,
        e if e < 2000.0 => 4.0,
        e if e < 2500.0 => 5.0,
        _ => 6.0,
    }
}

/// `Int(round(x))` for Kemenag, `Int(ceil(x))` for NU and Muhammadiyah.
/// Rust's `f64::round` rounds halves away from zero, like Swift's `round`.
fn round_or_ceil(minutes: f64, method: PrayerCalculationMethod) -> i64 {
    if method.uses_ceil_rounding() {
        minutes.ceil() as i64
    } else {
        minutes.round() as i64
    }
}

/// The `DateComponents` fields Swift writes: `(total / 60) % 24` hours and
/// `total % 60` minutes, with integer truncation toward zero in both steps.
fn swift_clock_minutes(total_minutes: i64) -> i64 {
    (total_minutes / 60 % 24) * 60 + total_minutes % 60
}

/// `DailyPrayerSchedule.swift`: eight milestones for one day and their
/// next-prayer queries. All comparisons use the absolute `epoch_seconds`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DailyPrayerSchedule {
    pub day: YearMonthDay,
    pub method: PrayerCalculationMethod,
    /// The offset the caller injected; no time zone is read from the system.
    pub utc_offset_seconds: i32,
    pub imsak: PrayerMoment,
    pub subuh: PrayerMoment,
    pub terbit: PrayerMoment,
    pub dhuha: PrayerMoment,
    pub dzuhur: PrayerMoment,
    pub ashar: PrayerMoment,
    pub maghrib: PrayerMoment,
    pub isya: PrayerMoment,
}

impl DailyPrayerSchedule {
    /// Midnight of `day` at the injected UTC offset, as epoch seconds.
    pub fn day_start_epoch(&self) -> i64 {
        let jd = julian_day(self.day.year, self.day.month, self.day.day as f64);
        ((jd - julian_day(1970, 1, 1.0)) * 86_400.0).round() as i64 - self.utc_offset_seconds as i64
    }

    /// `time(for:)` in Swift.
    pub fn time(&self, prayer: PrayerType) -> PrayerMoment {
        match prayer {
            PrayerType::Imsak => self.imsak,
            PrayerType::Subuh => self.subuh,
            PrayerType::Terbit => self.terbit,
            PrayerType::Dhuha => self.dhuha,
            PrayerType::Dzuhur => self.dzuhur,
            PrayerType::Ashar => self.ashar,
            PrayerType::Maghrib => self.maghrib,
            PrayerType::Isya => self.isya,
        }
    }

    /// `fivePrayers` in Swift.
    pub fn five_prayers(&self) -> [(PrayerType, PrayerMoment); 5] {
        PrayerType::FARDH.map(|prayer| (prayer, self.time(prayer)))
    }

    /// `allMilestones` in Swift.
    pub fn all_milestones(&self) -> [(PrayerType, PrayerMoment); 8] {
        PrayerType::ALL.map(|prayer| (prayer, self.time(prayer)))
    }

    /// `nextPrayer(at:tomorrowSubuh:)` in Swift.
    ///
    /// `at_epoch_seconds` is the injected "now" and `tomorrow_subuh` is
    /// tomorrow's Fajr epoch instant, used when today's milestones have all
    /// passed. Returns the milestone's epoch instant.
    pub fn next_prayer(
        &self,
        at_epoch_seconds: i64,
        tomorrow_subuh: Option<i64>,
    ) -> Option<(PrayerType, i64)> {
        let items: Vec<(PrayerType, i64)> = self
            .all_milestones()
            .into_iter()
            .map(|(prayer, moment)| (prayer, moment.epoch_seconds))
            .collect();
        next_milestone(&items, at_epoch_seconds, tomorrow_subuh)
    }

    /// `nextFardhPrayer(at:tomorrowSubuh:)` in Swift.
    pub fn next_fardh_prayer(
        &self,
        at_epoch_seconds: i64,
        tomorrow_subuh: Option<i64>,
    ) -> Option<(PrayerType, i64)> {
        let items: Vec<(PrayerType, i64)> = self
            .five_prayers()
            .into_iter()
            .map(|(prayer, moment)| (prayer, moment.epoch_seconds))
            .collect();
        next_milestone(&items, at_epoch_seconds, tomorrow_subuh)
    }

    /// `PrayerTimeManager.minutesUntilNextPrayer`: `Int(ceil((next - now)/60))`.
    pub fn minutes_until_next_prayer(
        &self,
        at_epoch_seconds: i64,
        tomorrow_subuh: Option<i64>,
    ) -> Option<i64> {
        let (_, at) = self.next_prayer(at_epoch_seconds, tomorrow_subuh)?;
        let seconds = at - at_epoch_seconds;
        Some(if seconds > 0 { (seconds + 59) / 60 } else { 0 })
    }

    /// `currentFardhPrayer(at:)` in Swift.
    pub fn current_fardh_prayer(&self, at_epoch_seconds: i64) -> PrayerType {
        if at_epoch_seconds < self.subuh.epoch_seconds {
            PrayerType::Isya // still night from the previous day
        } else if at_epoch_seconds < self.dzuhur.epoch_seconds {
            PrayerType::Subuh
        } else if at_epoch_seconds < self.ashar.epoch_seconds {
            PrayerType::Dzuhur
        } else if at_epoch_seconds < self.maghrib.epoch_seconds {
            PrayerType::Ashar
        } else if at_epoch_seconds < self.isya.epoch_seconds {
            PrayerType::Maghrib
        } else {
            PrayerType::Isya
        }
    }
}

/// `nextMilestone(in:after:tomorrowSubuh:)` in Swift.
fn next_milestone(
    items: &[(PrayerType, i64)],
    after: i64,
    tomorrow_subuh: Option<i64>,
) -> Option<(PrayerType, i64)> {
    items
        .iter()
        .find(|item| item.1 > after)
        .copied()
        .or_else(|| tomorrow_subuh.map(|at| (PrayerType::Subuh, at)))
}

/// `HijriCalendarSource` in `HijriCalendarCalculator.swift`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum HijriCalendarSource {
    #[serde(rename = "khgt")]
    Khgt,
    #[serde(rename = "neoMABIMS")]
    NeoMabims,
}

impl HijriCalendarSource {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Khgt => "KHGT (Global Tunggal)",
            Self::NeoMabims => "Takwim Kemenag (Neo-MABIMS)",
        }
    }
}

/// `HijriDate` in `HijriCalendarCalculator.swift`.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct HijriDate {
    pub day: i32,
    pub month: i32,
    pub year: i32,
    pub month_name: String,
}

impl HijriDate {
    pub fn new(day: i32, month: i32, year: i32) -> Self {
        Self {
            day,
            month,
            year,
            month_name: hijri_month_name(month),
        }
    }

    /// `formattedString` in Swift.
    pub fn formatted_string(&self) -> String {
        format!("{} {} {}", self.day, self.month_name, self.year)
    }
}

/// `HijriDate.name(forMonth:)` in Swift.
pub fn hijri_month_name(month: i32) -> String {
    match month {
        1 => "Muharram",
        2 => "Safar",
        3 => "Rabi' al-Awwal",
        4 => "Rabi' al-Thani",
        5 => "Jumada al-Ula",
        6 => "Jumada al-Akhirah",
        7 => "Rajab",
        8 => "Sha'ban",
        9 => "Ramadan",
        10 => "Shawwal",
        11 => "Dhu al-Qi'dah",
        12 => "Dhu al-Hijjah",
        _ => return format!("Month {month}"),
    }
    .to_owned()
}

/// `HijriCalendarCalculator.swift`: verified 1447–1451 H tables with an
/// astronomical fallback outside them.
pub struct HijriCalendarCalculator;

/// `(hYear, hMonth, gYear, gMonth, gDay)`, transcribed from Swift's
/// `makeBoundary` calls.
type Boundary = (i32, i32, i32, i32, i32);

const KHGT_BOUNDARIES: [Boundary; 60] = [
    // 1447 H
    (1447, 1, 2025, 6, 26),
    (1447, 2, 2025, 7, 26),
    (1447, 3, 2025, 8, 24),
    (1447, 4, 2025, 9, 23),
    (1447, 5, 2025, 10, 23),
    (1447, 6, 2025, 11, 21),
    (1447, 7, 2025, 12, 21),
    (1447, 8, 2026, 1, 20),
    (1447, 9, 2026, 2, 18),
    (1447, 10, 2026, 3, 20),
    (1447, 11, 2026, 4, 18),
    (1447, 12, 2026, 5, 18),
    // 1448 H
    (1448, 1, 2026, 6, 16),
    (1448, 2, 2026, 7, 15),
    (1448, 3, 2026, 8, 14),
    (1448, 4, 2026, 9, 12),
    (1448, 5, 2026, 10, 12),
    (1448, 6, 2026, 11, 10),
    (1448, 7, 2026, 12, 10),
    (1448, 8, 2027, 1, 9),
    (1448, 9, 2027, 2, 8),
    (1448, 10, 2027, 3, 9),
    (1448, 11, 2027, 4, 8),
    (1448, 12, 2027, 5, 7),
    // 1449 H
    (1449, 1, 2027, 6, 6),
    (1449, 2, 2027, 7, 5),
    (1449, 3, 2027, 8, 3),
    (1449, 4, 2027, 9, 2),
    (1449, 5, 2027, 10, 1),
    (1449, 6, 2027, 10, 31),
    (1449, 7, 2027, 11, 29),
    (1449, 8, 2027, 12, 29),
    (1449, 9, 2028, 1, 28),
    (1449, 10, 2028, 2, 26),
    (1449, 11, 2028, 3, 27),
    (1449, 12, 2028, 4, 26),
    // 1450 H
    (1450, 1, 2028, 5, 25),
    (1450, 2, 2028, 6, 24),
    (1450, 3, 2028, 7, 23),
    (1450, 4, 2028, 8, 21),
    (1450, 5, 2028, 9, 20),
    (1450, 6, 2028, 10, 19),
    (1450, 7, 2028, 11, 17),
    (1450, 8, 2028, 12, 17),
    (1450, 9, 2029, 1, 16),
    (1450, 10, 2029, 2, 14),
    (1450, 11, 2029, 3, 16),
    (1450, 12, 2029, 4, 15),
    // 1451 H
    (1451, 1, 2029, 5, 15),
    (1451, 2, 2029, 6, 13),
    (1451, 3, 2029, 7, 13),
    (1451, 4, 2029, 8, 11),
    (1451, 5, 2029, 9, 9),
    (1451, 6, 2029, 10, 9),
    (1451, 7, 2029, 11, 7),
    (1451, 8, 2029, 12, 6),
    (1451, 9, 2030, 1, 5),
    (1451, 10, 2030, 2, 4),
    (1451, 11, 2030, 3, 5),
    (1451, 12, 2030, 4, 4),
];

const NEO_MABIMS_BOUNDARIES: [Boundary; 60] = [
    // 1447 H (+1 day on 1 Muharram, 1 Rabiul Awal, 1 Ramadan, 1 Syawal)
    (1447, 1, 2025, 6, 27),
    (1447, 2, 2025, 7, 26),
    (1447, 3, 2025, 8, 25),
    (1447, 4, 2025, 9, 23),
    (1447, 5, 2025, 10, 23),
    (1447, 6, 2025, 11, 21),
    (1447, 7, 2025, 12, 21),
    (1447, 8, 2026, 1, 20),
    (1447, 9, 2026, 2, 19),
    (1447, 10, 2026, 3, 21),
    (1447, 11, 2026, 4, 19),
    (1447, 12, 2026, 5, 18),
    // 1448 H
    (1448, 1, 2026, 6, 16),
    (1448, 2, 2026, 7, 16),
    (1448, 3, 2026, 8, 15),
    (1448, 4, 2026, 9, 13),
    (1448, 5, 2026, 10, 13),
    (1448, 6, 2026, 11, 11),
    (1448, 7, 2026, 12, 11),
    (1448, 8, 2027, 1, 10),
    (1448, 9, 2027, 2, 8),
    (1448, 10, 2027, 3, 10),
    (1448, 11, 2027, 4, 8),
    (1448, 12, 2027, 5, 8),
    // 1449 H
    (1449, 1, 2027, 6, 6),
    (1449, 2, 2027, 7, 6),
    (1449, 3, 2027, 8, 4),
    (1449, 4, 2027, 9, 3),
    (1449, 5, 2027, 10, 2),
    (1449, 6, 2027, 11, 1),
    (1449, 7, 2027, 11, 30),
    (1449, 8, 2027, 12, 30),
    (1449, 9, 2028, 1, 29),
    (1449, 10, 2028, 2, 27),
    (1449, 11, 2028, 3, 28),
    (1449, 12, 2028, 4, 27),
    // 1450 H
    (1450, 1, 2028, 5, 26),
    (1450, 2, 2028, 6, 25),
    (1450, 3, 2028, 7, 24),
    (1450, 4, 2028, 8, 22),
    (1450, 5, 2028, 9, 21),
    (1450, 6, 2028, 10, 20),
    (1450, 7, 2028, 11, 18),
    (1450, 8, 2028, 12, 18),
    (1450, 9, 2029, 1, 17),
    (1450, 10, 2029, 2, 15),
    (1450, 11, 2029, 3, 17),
    (1450, 12, 2029, 4, 16),
    // 1451 H
    (1451, 1, 2029, 5, 16),
    (1451, 2, 2029, 6, 14),
    (1451, 3, 2029, 7, 14),
    (1451, 4, 2029, 8, 12),
    (1451, 5, 2029, 9, 10),
    (1451, 6, 2029, 10, 10),
    (1451, 7, 2029, 11, 8),
    (1451, 8, 2029, 12, 7),
    (1451, 9, 2030, 1, 6),
    (1451, 10, 2030, 2, 5),
    (1451, 11, 2030, 3, 6),
    (1451, 12, 2030, 4, 5),
];

fn boundary_jd(boundary: Boundary) -> f64 {
    let (_, _, year, month, day) = boundary;
    julian_day(year, month, day as f64)
}

impl HijriCalendarCalculator {
    /// `HijriCalendarCalculator.calculate(day:source:manualOffsetDays:)`.
    pub fn calculate(
        day: YearMonthDay,
        source: HijriCalendarSource,
        manual_offset_days: i32,
    ) -> HijriDate {
        let target_jd = julian_day(day.year, day.month, day.day as f64) + manual_offset_days as f64;
        let table: &[Boundary] = match source {
            HijriCalendarSource::Khgt => &KHGT_BOUNDARIES[..],
            HijriCalendarSource::NeoMabims => &NEO_MABIMS_BOUNDARIES[..],
        };

        // The table spans 1447-01-01 .. 1451-12-30; Swift estimates its end as
        // the last month-start + 30 days.
        let first_jd = boundary_jd(table[0]);
        let end_table_jd = boundary_jd(table[table.len() - 1]) + 30.0;
        if (first_jd..end_table_jd).contains(&target_jd) {
            let mut matched_index = 0;
            for (i, entry) in table.iter().enumerate() {
                if target_jd >= boundary_jd(*entry) {
                    matched_index = i;
                } else {
                    break;
                }
            }
            let (h_year, h_month, _, _, _) = table[matched_index];
            let day_of_month = (target_jd - boundary_jd(table[matched_index])) as i32 + 1;
            return HijriDate::new(day_of_month, h_month, h_year);
        }

        calculate_algorithmic(target_jd, source)
    }
}

/// `calculateAlgorithmic(targetJD:source:)`: the Swift source ignores `source`
/// here and always anchors at 1 Muharram 1447 H (KHGT, JD 2460852.5).
fn calculate_algorithmic(target_jd: f64, _source: HijriCalendarSource) -> HijriDate {
    let epoch_jd = 2_460_852.5;
    let synodic_month = 29.530588853;
    let elapsed_days = target_jd - epoch_jd;
    let elapsed_months = (elapsed_days / synodic_month).floor();
    let total_months = 1447 * 12 + elapsed_months as i64;

    let h_year = total_months / 12;
    let h_month = total_months % 12 + 1;

    let approx_month_start_jd = epoch_jd + elapsed_months * synodic_month;
    let mut day_of_month = (target_jd - approx_month_start_jd) as i32 + 1;
    day_of_month = day_of_month.clamp(1, 30);

    HijriDate::new(day_of_month, h_month as i32, h_year as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swift_method_constants() {
        for method in PrayerCalculationMethod::ALL {
            assert_eq!(method.isha_angle(), 18.0);
            assert_eq!(method.dhuha_angle(), 4.5);
        }
        let kemenag = PrayerCalculationMethod::Kemenag;
        assert_eq!(kemenag.display_name(), "Kemenag RI");
        assert_eq!(kemenag.fajr_angle(), 20.0);
        assert_eq!(kemenag.dhuhr_minutes_after_transit(), 2.0);
        assert!(!kemenag.uses_ceil_rounding());

        let nu = PrayerCalculationMethod::NuLfpbnu;
        assert_eq!(nu.display_name(), "Nahdlatul Ulama (LF PBNU)");
        assert_eq!(nu.fajr_angle(), 20.0);
        assert_eq!(nu.dhuhr_minutes_after_transit(), 1.0);
        assert!(nu.uses_ceil_rounding());

        let muh = PrayerCalculationMethod::Muhammadiyah;
        assert_eq!(muh.display_name(), "Muhammadiyah (MTT)");
        assert_eq!(muh.fajr_angle(), 18.0);
        assert_eq!(muh.dhuhr_minutes_after_transit(), 0.0);
        assert!(muh.uses_ceil_rounding());

        assert_eq!(serde_json::to_string(&kemenag).unwrap(), "\"kemenag\"");
        assert_eq!(serde_json::to_string(&nu).unwrap(), "\"nuLFPBNU\"");
        assert_eq!(serde_json::to_string(&muh).unwrap(), "\"muhammadiyah\"");

        assert_eq!(PrayerType::ALL.len(), 8);
        assert_eq!(PrayerType::FARDH.len(), 5);
        assert!(
            PrayerType::ALL
                .iter()
                .all(|p| p.is_fardh() == PrayerType::FARDH.contains(p))
        );
        assert_eq!(PrayerType::Terbit.display_name(), "Sunrise");
    }

    #[test]
    fn julian_day_matches_meeus_reference() {
        assert_eq!(julian_day(2000, 1, 1.0), 2_451_544.5);
        assert_eq!(julian_day(1970, 1, 1.0), 2_440_587.5);
        assert_eq!(julian_day(2026, 1, 15.0), 2_461_055.5);
        // January and February roll into the previous year, month + 12.
        assert_eq!(julian_day(2026, 2, 1.0), julian_day(2025, 14, 1.0));
    }

    #[test]
    fn elevation_helpers_match_swift_table() {
        let cases = [
            (0.0, 0.0),
            (249.0, 0.0),
            (250.0, 1.0),
            (699.0, 1.0),
            (700.0, 2.0),
            (999.0, 2.0),
            (1000.0, 3.0),
            (1499.0, 3.0),
            (1500.0, 4.0),
            (1999.0, 4.0),
            (2000.0, 5.0),
            (2499.0, 5.0),
            (2500.0, 6.0),
            (10_000.0, 6.0),
        ];
        for (elevation, expected) in cases {
            assert_eq!(
                kemenag_elevation_minutes(elevation),
                expected,
                "{elevation}"
            );
        }
        assert_eq!(apparent_dip(100.0), 0.02933 * 10.0);
        assert_eq!(apparent_dip(-5.0), 0.0);
    }

    #[test]
    fn date_components_wrap_like_swift_calendar() {
        assert_eq!(swift_clock_minutes(1172), 1172); // 19:32
        assert_eq!(swift_clock_minutes(1445), 5); // 00:05, not 24:05
        assert_eq!(swift_clock_minutes(1500), 60); // 01:00
        assert_eq!(swift_clock_minutes(-12), -12); // minute: -12
        assert_eq!(swift_clock_minutes(-65), -65); // hour: -1, minute: -5
    }

    #[test]
    fn hijri_month_start_29_and_30_day_months() {
        let khgt = HijriCalendarSource::Khgt;
        // KHGT 1 Muharram 1447 = 2025-06-26, 1 Safar = 2025-07-26 (30 days).
        assert_eq!(
            HijriCalendarCalculator::calculate(YearMonthDay::new(2025, 6, 26), khgt, 0),
            HijriDate::new(1, 1, 1447)
        );
        assert_eq!(
            HijriCalendarCalculator::calculate(YearMonthDay::new(2025, 7, 25), khgt, 0),
            HijriDate::new(30, 1, 1447)
        );
        assert_eq!(
            HijriCalendarCalculator::calculate(YearMonthDay::new(2025, 7, 26), khgt, 0),
            HijriDate::new(1, 2, 1447)
        );
        // 1 Safar -> 1 Rabi' al-Awwal = 2025-08-24, a 29-day month.
        assert_eq!(
            HijriCalendarCalculator::calculate(YearMonthDay::new(2025, 8, 23), khgt, 0),
            HijriDate::new(29, 2, 1447)
        );
        assert_eq!(
            HijriCalendarCalculator::calculate(YearMonthDay::new(2025, 8, 24), khgt, 0),
            HijriDate::new(1, 3, 1447)
        );
    }

    #[test]
    fn hijri_ramadan_differs_between_sources_and_manual_offset() {
        // KHGT: 1 Ramadan 1447 = 2026-02-18, Neo-MABIMS: 2026-02-19.
        assert_eq!(
            HijriCalendarCalculator::calculate(
                YearMonthDay::new(2026, 2, 18),
                HijriCalendarSource::Khgt,
                0
            ),
            HijriDate::new(1, 9, 1447)
        );
        // Neo-MABIMS starts Sha'ban on 2026-01-20 and Ramadan on 2026-02-19,
        // so 2026-02-18 is 30 Sha'ban — a full month in this table (KHGT's
        // Sha'ban is the 29-day one, ending 2026-02-17).
        assert_eq!(
            HijriCalendarCalculator::calculate(
                YearMonthDay::new(2026, 2, 18),
                HijriCalendarSource::NeoMabims,
                0
            ),
            HijriDate::new(30, 8, 1447)
        );
        assert_eq!(
            HijriCalendarCalculator::calculate(
                YearMonthDay::new(2026, 2, 19),
                HijriCalendarSource::NeoMabims,
                0
            ),
            HijriDate::new(1, 9, 1447)
        );
        assert_eq!(
            HijriCalendarCalculator::calculate(
                YearMonthDay::new(2026, 2, 18),
                HijriCalendarSource::Khgt,
                1
            ),
            HijriDate::new(2, 9, 1447)
        );
        assert_eq!(
            HijriDate::new(1, 9, 1447).formatted_string(),
            "1 Ramadan 1447"
        );
    }

    #[test]
    fn hijri_table_boundary_falls_back_to_algorithmic() {
        // 1451-12-01 KHGT starts 2030-04-04; last table JD + 30 is 2462625.5.
        assert_eq!(
            HijriCalendarCalculator::calculate(
                YearMonthDay::new(2030, 4, 4),
                HijriCalendarSource::Khgt,
                0
            ),
            HijriDate::new(1, 12, 1451)
        );
        // 2030-05-04 is exactly the end of the tabular range: algorithmic.
        assert_eq!(
            HijriCalendarCalculator::calculate(
                YearMonthDay::new(2030, 5, 4),
                HijriCalendarSource::Khgt,
                0
            ),
            HijriDate::new(2, 1, 1452)
        );
        assert_eq!(
            HijriCalendarCalculator::calculate(
                YearMonthDay::new(2024, 1, 1),
                HijriCalendarSource::Khgt,
                0
            ),
            HijriDate::new(20, 6, 1445)
        );
    }

    #[test]
    fn next_prayer_countdown_and_current_period() {
        let schedule = PrayerTimeCalculator::calculate(
            YearMonthDay::new(2026, 1, 15),
            Coordinates::new(-6.2088, 106.8456),
            10.0,
            PrayerCalculationMethod::Kemenag,
            25_200,
        );
        // Values from shared/fixtures/prayer/expected.json, Jakarta 2026-01-15.
        let subuh = schedule.subuh.epoch_seconds;
        let dzuhur = schedule.dzuhur.epoch_seconds;
        let isya = schedule.isya.epoch_seconds;
        assert_eq!(schedule.subuh.local_minutes, 267);
        assert_eq!(schedule.maghrib.local_minutes, 1098);

        assert_eq!(
            schedule.next_prayer(subuh - 60, None),
            Some((PrayerType::Subuh, subuh))
        );
        assert_eq!(
            schedule.minutes_until_next_prayer(subuh - 60, None),
            Some(1)
        );
        assert_eq!(
            schedule.next_prayer(subuh, None),
            Some((PrayerType::Terbit, schedule.terbit.epoch_seconds))
        );
        assert_eq!(schedule.current_fardh_prayer(subuh - 1), PrayerType::Isya);
        assert_eq!(schedule.current_fardh_prayer(subuh), PrayerType::Subuh);
        assert_eq!(schedule.current_fardh_prayer(dzuhur), PrayerType::Dzuhur);
        assert_eq!(schedule.current_fardh_prayer(isya), PrayerType::Isya);

        let tomorrow = isya + 10_000;
        assert_eq!(
            schedule.next_prayer(isya, Some(tomorrow)),
            Some((PrayerType::Subuh, tomorrow))
        );
        assert_eq!(
            schedule.next_fardh_prayer(isya, Some(tomorrow)),
            Some((PrayerType::Subuh, tomorrow))
        );
        assert_eq!(
            schedule.minutes_until_next_prayer(isya, Some(tomorrow)),
            Some(167)
        );
    }

    #[test]
    fn summer_isha_wraps_to_same_day_like_datecomponents() {
        // Swift's toDate uses hour % 24, so Oslo's 25h Isha lands at 01:21.
        let schedule = PrayerTimeCalculator::calculate(
            YearMonthDay::new(2026, 6, 21),
            Coordinates::new(59.9139, 10.7522),
            23.0,
            PrayerCalculationMethod::Kemenag,
            7_200,
        );
        assert_eq!(schedule.isya.local_minutes, 81);
        assert_eq!(
            schedule.isya.epoch_seconds,
            schedule.day_start_epoch() + 81 * 60
        );
    }
}
