//! Offline reverse-geocoding catalog, ported from `OfflineCityDatabase.swift`
//! in `macos/Sources/MorrowCore`.
//!
//! As in Swift, the catalog is fixed data: no clock, time zone database,
//! filesystem or network is consulted. [`OfflineCity::utc_offset_seconds`]
//! replaces Swift's `OfflineCity.timeZone` with the same Indonesian longitude
//! band fallback, expressed as a fixed offset in seconds (the IANA identifier
//! is still stored for display).

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::Coordinates;

/// `OfflineCity` in `OfflineCityDatabase.swift`: one manually selectable or
/// reverse-geocoded city. `String` mirrors Swift's `String`; `f64` mirrors
/// `Double`, so this is `PartialEq` but not `Eq` (like [`Coordinates`]).
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct OfflineCity {
    pub id: String,
    pub name: String,
    pub province_or_country: String,
    pub coordinates: Coordinates,
    pub time_zone_identifier: String,
    pub elevation_meters: f64,
}

impl OfflineCity {
    /// The Swift memberwise `init(id:name:provinceOrCountry:coordinates:
    /// timeZoneIdentifier:elevationMeters:)`, with latitude/longitude expanded
    /// for the catalog literals below.
    fn city(
        id: &'static str,
        name: &'static str,
        province_or_country: &'static str,
        latitude: f64,
        longitude: f64,
        time_zone_identifier: &'static str,
        elevation_meters: f64,
    ) -> Self {
        Self {
            id: id.to_owned(),
            name: name.to_owned(),
            province_or_country: province_or_country.to_owned(),
            coordinates: Coordinates::new(latitude, longitude),
            time_zone_identifier: time_zone_identifier.to_owned(),
            elevation_meters,
        }
    }

    /// The offset Swift's `OfflineCity.timeZone` falls back to when the IANA
    /// identifier cannot be resolved:
    ///
    /// - WIB (UTC+7): longitude < 115°
    /// - WITA (UTC+8): 115° <= longitude < 125°
    /// - WIT (UTC+9): longitude >= 125°
    ///
    /// Returned in seconds so no time zone database (or `TimeZone.current`)
    /// is needed. The band rule applies to every city, including the global
    /// ones, exactly as the Swift fallback would.
    pub fn utc_offset_seconds(&self) -> i32 {
        let lon = self.coordinates.longitude;
        let offset_hours = if lon < 115.0 {
            7
        } else if lon < 125.0 {
            8
        } else {
            9
        };
        offset_hours * 3600
    }
}

/// The full catalog from Swift's `OfflineCityDatabase.allCities`, transcribed
/// verbatim (ids, names, coordinates, time zone identifiers and elevations).
/// Lazily built once so callers can borrow a stable slice.
static ALL_CITIES: OnceLock<Vec<OfflineCity>> = OnceLock::new();

/// Swift's `OfflineCityDatabase.allCities`.
pub fn all_cities() -> &'static [OfflineCity] {
    ALL_CITIES.get_or_init(build_catalog).as_slice()
}

fn build_catalog() -> Vec<OfflineCity> {
    vec![
        // DKI Jakarta & Bodetabek
        OfflineCity::city(
            "jakarta",
            "Jakarta",
            "DKI Jakarta",
            -6.2088,
            106.8456,
            "Asia/Jakarta",
            8.0,
        ),
        OfflineCity::city(
            "bogor",
            "Bogor",
            "Jawa Barat",
            -6.5971,
            106.8060,
            "Asia/Jakarta",
            265.0,
        ),
        OfflineCity::city(
            "depok",
            "Depok",
            "Jawa Barat",
            -6.4025,
            106.7942,
            "Asia/Jakarta",
            90.0,
        ),
        OfflineCity::city(
            "tangerang",
            "Tangerang",
            "Banten",
            -6.1783,
            106.6319,
            "Asia/Jakarta",
            18.0,
        ),
        OfflineCity::city(
            "tangerang_selatan",
            "Tangerang Selatan",
            "Banten",
            -6.2889,
            106.7179,
            "Asia/Jakarta",
            40.0,
        ),
        OfflineCity::city(
            "bekasi",
            "Bekasi",
            "Jawa Barat",
            -6.2383,
            106.9756,
            "Asia/Jakarta",
            19.0,
        ),
        // Jawa Barat & Banten
        OfflineCity::city(
            "bandung",
            "Bandung",
            "Jawa Barat",
            -6.9175,
            107.6191,
            "Asia/Jakarta",
            708.0,
        ),
        OfflineCity::city(
            "serang",
            "Serang",
            "Banten",
            -6.1104,
            106.1607,
            "Asia/Jakarta",
            20.0,
        ),
        OfflineCity::city(
            "cirebon",
            "Cirebon",
            "Jawa Barat",
            -6.7320,
            108.5523,
            "Asia/Jakarta",
            8.0,
        ),
        OfflineCity::city(
            "sukabumi",
            "Sukabumi",
            "Jawa Barat",
            -6.9277,
            106.9300,
            "Asia/Jakarta",
            584.0,
        ),
        OfflineCity::city(
            "tasikmalaya",
            "Tasikmalaya",
            "Jawa Barat",
            -7.3274,
            108.2207,
            "Asia/Jakarta",
            351.0,
        ),
        // Jawa Tengah & DIY
        OfflineCity::city(
            "semarang",
            "Semarang",
            "Jawa Tengah",
            -6.9667,
            110.4167,
            "Asia/Jakarta",
            5.0,
        ),
        OfflineCity::city(
            "yogyakarta",
            "Yogyakarta",
            "DI Yogyakarta",
            -7.7956,
            110.3695,
            "Asia/Jakarta",
            113.0,
        ),
        OfflineCity::city(
            "surakarta",
            "Surakarta (Solo)",
            "Jawa Tengah",
            -7.5755,
            110.8243,
            "Asia/Jakarta",
            105.0,
        ),
        OfflineCity::city(
            "magelang",
            "Magelang",
            "Jawa Tengah",
            -7.4706,
            110.2178,
            "Asia/Jakarta",
            380.0,
        ),
        OfflineCity::city(
            "purwokerto",
            "Purwokerto",
            "Jawa Tengah",
            -7.4243,
            109.2302,
            "Asia/Jakarta",
            84.0,
        ),
        OfflineCity::city(
            "pekalongan",
            "Pekalongan",
            "Jawa Tengah",
            -6.8886,
            109.6753,
            "Asia/Jakarta",
            8.0,
        ),
        OfflineCity::city(
            "tegal",
            "Tegal",
            "Jawa Tengah",
            -6.8694,
            109.1402,
            "Asia/Jakarta",
            5.0,
        ),
        // Jawa Timur
        OfflineCity::city(
            "surabaya",
            "Surabaya",
            "Jawa Timur",
            -7.2575,
            112.7521,
            "Asia/Jakarta",
            5.0,
        ),
        OfflineCity::city(
            "malang",
            "Malang",
            "Jawa Timur",
            -7.9666,
            112.6326,
            "Asia/Jakarta",
            444.0,
        ),
        OfflineCity::city(
            "kediri",
            "Kediri",
            "Jawa Timur",
            -7.8480,
            112.0178,
            "Asia/Jakarta",
            67.0,
        ),
        OfflineCity::city(
            "madiun",
            "Madiun",
            "Jawa Timur",
            -7.6298,
            111.5239,
            "Asia/Jakarta",
            65.0,
        ),
        OfflineCity::city(
            "banyuwangi",
            "Banyuwangi",
            "Jawa Timur",
            -8.2192,
            114.3691,
            "Asia/Jakarta",
            15.0,
        ),
        OfflineCity::city(
            "jember",
            "Jember",
            "Jawa Timur",
            -8.1724,
            113.7007,
            "Asia/Jakarta",
            89.0,
        ),
        // Sumatera
        OfflineCity::city(
            "banda_aceh",
            "Banda Aceh",
            "Aceh",
            5.5483,
            95.3238,
            "Asia/Jakarta",
            10.0,
        ),
        OfflineCity::city(
            "medan",
            "Medan",
            "Sumatera Utara",
            3.5952,
            98.6722,
            "Asia/Jakarta",
            26.0,
        ),
        OfflineCity::city(
            "padang",
            "Padang",
            "Sumatera Barat",
            -0.9471,
            100.4172,
            "Asia/Jakarta",
            10.0,
        ),
        OfflineCity::city(
            "bukittinggi",
            "Bukittinggi",
            "Sumatera Barat",
            -0.3056,
            100.3692,
            "Asia/Jakarta",
            930.0,
        ),
        OfflineCity::city(
            "pekanbaru",
            "Pekanbaru",
            "Riau",
            0.5071,
            101.4478,
            "Asia/Jakarta",
            12.0,
        ),
        OfflineCity::city(
            "batam",
            "Batam",
            "Kepulauan Riau",
            1.1301,
            104.0529,
            "Asia/Jakarta",
            10.0,
        ),
        OfflineCity::city(
            "tanjungpinang",
            "Tanjungpinang",
            "Kepulauan Riau",
            0.9167,
            104.4500,
            "Asia/Jakarta",
            15.0,
        ),
        OfflineCity::city(
            "jambi",
            "Jambi",
            "Jambi",
            -1.6101,
            103.6131,
            "Asia/Jakarta",
            15.0,
        ),
        OfflineCity::city(
            "palembang",
            "Palembang",
            "Sumatera Selatan",
            -2.9909,
            104.7565,
            "Asia/Jakarta",
            8.0,
        ),
        OfflineCity::city(
            "bengkulu",
            "Bengkulu",
            "Bengkulu",
            -3.8004,
            102.2655,
            "Asia/Jakarta",
            15.0,
        ),
        OfflineCity::city(
            "bandar_lampung",
            "Bandar Lampung",
            "Lampung",
            -5.4500,
            105.2667,
            "Asia/Jakarta",
            20.0,
        ),
        OfflineCity::city(
            "pangkalpinang",
            "Pangkalpinang",
            "Bangka Belitung",
            -2.1333,
            106.1167,
            "Asia/Jakarta",
            15.0,
        ),
        // Kalimantan
        OfflineCity::city(
            "pontianak",
            "Pontianak",
            "Kalimantan Barat",
            0.0000,
            109.3333,
            "Asia/Jakarta",
            3.0,
        ),
        OfflineCity::city(
            "palangkaraya",
            "Palangka Raya",
            "Kalimantan Tengah",
            -2.2167,
            113.9167,
            "Asia/Jakarta",
            10.0,
        ),
        OfflineCity::city(
            "banjarmasin",
            "Banjarmasin",
            "Kalimantan Selatan",
            -3.3194,
            114.5908,
            "Asia/Makassar",
            2.0,
        ),
        OfflineCity::city(
            "banjarbaru",
            "Banjarbaru",
            "Kalimantan Selatan",
            -3.4400,
            114.8300,
            "Asia/Makassar",
            25.0,
        ),
        OfflineCity::city(
            "samarinda",
            "Samarinda",
            "Kalimantan Timur",
            -0.5022,
            117.1536,
            "Asia/Makassar",
            10.0,
        ),
        OfflineCity::city(
            "balikpapan",
            "Balikpapan",
            "Kalimantan Timur",
            -1.2379,
            116.8529,
            "Asia/Makassar",
            15.0,
        ),
        OfflineCity::city(
            "ikn",
            "Nusantara (IKN)",
            "Kalimantan Timur",
            -0.9700,
            116.7000,
            "Asia/Makassar",
            40.0,
        ),
        OfflineCity::city(
            "tarakan",
            "Tarakan",
            "Kalimantan Utara",
            3.3000,
            117.6333,
            "Asia/Makassar",
            10.0,
        ),
        // Bali & Nusa Tenggara
        OfflineCity::city(
            "denpasar",
            "Denpasar",
            "Bali",
            -8.6705,
            115.2126,
            "Asia/Makassar",
            15.0,
        ),
        OfflineCity::city(
            "mataram",
            "Mataram",
            "Nusa Tenggara Barat",
            -8.5833,
            116.1167,
            "Asia/Makassar",
            25.0,
        ),
        OfflineCity::city(
            "bima",
            "Bima",
            "Nusa Tenggara Barat",
            -8.4600,
            118.7300,
            "Asia/Makassar",
            15.0,
        ),
        OfflineCity::city(
            "kupang",
            "Kupang",
            "Nusa Tenggara Timur",
            -10.1772,
            123.6070,
            "Asia/Makassar",
            25.0,
        ),
        OfflineCity::city(
            "labuan_bajo",
            "Labuan Bajo",
            "Nusa Tenggara Timur",
            -8.4964,
            119.8877,
            "Asia/Makassar",
            20.0,
        ),
        // Sulawesi
        OfflineCity::city(
            "makassar",
            "Makassar",
            "Sulawesi Selatan",
            -5.1477,
            119.4327,
            "Asia/Makassar",
            15.0,
        ),
        OfflineCity::city(
            "manado",
            "Manado",
            "Sulawesi Utara",
            1.4748,
            124.8428,
            "Asia/Makassar",
            20.0,
        ),
        OfflineCity::city(
            "palu",
            "Palu",
            "Sulawesi Tengah",
            -0.8917,
            119.8707,
            "Asia/Makassar",
            10.0,
        ),
        OfflineCity::city(
            "kendari",
            "Kendari",
            "Sulawesi Tenggara",
            -3.9985,
            122.5126,
            "Asia/Makassar",
            15.0,
        ),
        OfflineCity::city(
            "gorontalo",
            "Gorontalo",
            "Gorontalo",
            0.5435,
            123.0568,
            "Asia/Makassar",
            15.0,
        ),
        OfflineCity::city(
            "mamuju",
            "Mamuju",
            "Sulawesi Barat",
            -2.6748,
            118.8885,
            "Asia/Makassar",
            15.0,
        ),
        // Maluku & Papua
        OfflineCity::city(
            "ambon",
            "Ambon",
            "Maluku",
            -3.6554,
            128.1908,
            "Asia/Jayapura",
            12.0,
        ),
        OfflineCity::city(
            "ternate",
            "Ternate",
            "Maluku Utara",
            0.7833,
            127.3667,
            "Asia/Jayapura",
            15.0,
        ),
        OfflineCity::city(
            "jayapura",
            "Jayapura",
            "Papua",
            -2.5489,
            140.7197,
            "Asia/Jayapura",
            25.0,
        ),
        OfflineCity::city(
            "sorong",
            "Sorong",
            "Papua Barat Daya",
            -0.8762,
            131.2558,
            "Asia/Jayapura",
            15.0,
        ),
        OfflineCity::city(
            "manokwari",
            "Manokwari",
            "Papua Barat",
            -0.8615,
            134.0620,
            "Asia/Jayapura",
            20.0,
        ),
        OfflineCity::city(
            "merauke",
            "Merauke",
            "Papua Selatan",
            -8.4932,
            140.4019,
            "Asia/Jayapura",
            8.0,
        ),
        OfflineCity::city(
            "timika",
            "Timika",
            "Papua Tengah",
            -4.5467,
            136.8837,
            "Asia/Jayapura",
            35.0,
        ),
        // Global Capitals & Holy Cities
        OfflineCity::city(
            "makkah",
            "Makkah",
            "Arab Saudi",
            21.4225,
            39.8262,
            "Asia/Riyadh",
            277.0,
        ),
        OfflineCity::city(
            "madinah",
            "Madinah",
            "Arab Saudi",
            24.5247,
            39.5692,
            "Asia/Riyadh",
            608.0,
        ),
        OfflineCity::city(
            "kuala_lumpur",
            "Kuala Lumpur",
            "Malaysia",
            3.1390,
            101.6869,
            "Asia/Kuala_Lumpur",
            22.0,
        ),
        OfflineCity::city(
            "singapore",
            "Singapura",
            "Singapura",
            1.3521,
            103.8198,
            "Asia/Singapore",
            15.0,
        ),
        OfflineCity::city(
            "tokyo",
            "Tokyo",
            "Jepang",
            35.6762,
            139.6503,
            "Asia/Tokyo",
            44.0,
        ),
        OfflineCity::city(
            "london",
            "London",
            "Britania Raya",
            51.5074,
            -0.1278,
            "Europe/London",
            11.0,
        ),
        OfflineCity::city(
            "new_york",
            "New York",
            "Amerika Serikat",
            40.7128,
            -74.0060,
            "America/New_York",
            10.0,
        ),
        OfflineCity::city(
            "cairo",
            "Kairo",
            "Mesir",
            30.0444,
            31.2357,
            "Africa/Cairo",
            23.0,
        ),
        OfflineCity::city(
            "istanbul",
            "Istanbul",
            "Turki",
            41.0082,
            28.9784,
            "Europe/Istanbul",
            39.0,
        ),
    ]
}

/// Swift's `OfflineCityDatabase.closestCity(to:)`: the nearest catalog city by
/// great-circle distance. Starts from [`default_city`] with an infinite
/// minimum, exactly as the Swift loop does.
pub fn closest_city(coordinates: Coordinates) -> OfflineCity {
    let mut closest = default_city();
    let mut min_distance = f64::INFINITY;

    for city in all_cities() {
        let dist = haversine_distance_kilometers(coordinates, city.coordinates);
        if dist < min_distance {
            min_distance = dist;
            closest = city.clone();
        }
    }
    closest
}

/// Swift's `OfflineCityDatabase.city(byId:)`.
pub fn city_by_id(id: &str) -> Option<OfflineCity> {
    all_cities().iter().find(|city| city.id == id).cloned()
}

/// Case-insensitive name lookup for the Settings picker (an addition beyond
/// Swift, where only `city(byId:)` exists).
pub fn city_by_name(name: &str) -> Option<OfflineCity> {
    all_cities()
        .iter()
        .find(|city| city.name.eq_ignore_ascii_case(name))
        .cloned()
}

/// Swift's `OfflineCityDatabase.defaultCity`: Jakarta.
pub fn default_city() -> OfflineCity {
    OfflineCity::city(
        "jakarta",
        "Jakarta",
        "DKI Jakarta",
        -6.2088,
        106.8456,
        "Asia/Jakarta",
        8.0,
    )
}

/// Swift's `OfflineCityDatabase.haversineDistanceKilometers(from:to:)`: the
/// spherical great-circle distance with an Earth radius of 6371 km.
pub fn haversine_distance_kilometers(from: Coordinates, to: Coordinates) -> f64 {
    let r = 6371.0_f64;
    let d_lat = (to.latitude - from.latitude) * std::f64::consts::PI / 180.0;
    let d_lon = (to.longitude - from.longitude) * std::f64::consts::PI / 180.0;
    let lat1 = from.latitude * std::f64::consts::PI / 180.0;
    let lat2 = to.latitude * std::f64::consts::PI / 180.0;

    let a = (d_lat / 2.0).sin() * (d_lat / 2.0).sin()
        + lat1.cos() * lat2.cos() * (d_lon / 2.0).sin() * (d_lon / 2.0).sin();
    let c = 2.0 * a.sqrt().atan2((1.0 - a).max(0.0).sqrt());
    r * c
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn catalog_is_full_and_ids_are_unique() {
        let cities = all_cities();
        assert_eq!(cities.len(), 71);
        let ids: HashSet<&str> = cities.iter().map(|city| city.id.as_str()).collect();
        assert_eq!(ids.len(), cities.len());
    }

    #[test]
    fn default_city_is_jakarta() {
        let city = default_city();
        assert_eq!(city.id, "jakarta");
        assert_eq!(city.name, "Jakarta");
        assert_eq!(city.province_or_country, "DKI Jakarta");
        assert_eq!(city.coordinates, Coordinates::new(-6.2088, 106.8456));
        assert_eq!(city.time_zone_identifier, "Asia/Jakarta");
        assert_eq!(city.elevation_meters, 8.0);
    }

    #[test]
    fn closest_city_matches_swift_catalog_coordinates() {
        // Coordinates are transcribed from the Swift catalog; feeding a
        // city's own coordinates must select that city (distance zero).
        let cases = [
            ("jakarta", -6.2088, 106.8456),
            ("makassar", -5.1477, 119.4327),
            ("jayapura", -2.5489, 140.7197),
            ("london", 51.5074, -0.1278),
            ("makkah", 21.4225, 39.8262),
        ];
        for (id, latitude, longitude) in cases {
            assert_eq!(
                closest_city(Coordinates::new(latitude, longitude)).id,
                id,
                "{id}"
            );
        }
    }

    #[test]
    fn city_by_id_and_case_insensitive_city_by_name() {
        assert_eq!(city_by_id("makkah").unwrap().name, "Makkah");
        assert!(city_by_id("not_a_city").is_none());

        for name in ["jakarta", "JAKARTA", "jAkArTa"] {
            assert_eq!(city_by_name(name).unwrap().id, "jakarta", "{name}");
        }
        assert_eq!(city_by_name("surakarta (SOLO)").unwrap().id, "surakarta");
        assert_eq!(city_by_name("NUSANTARA (ikn)").unwrap().id, "ikn");
        assert!(city_by_name("not_a_city").is_none());
    }

    #[test]
    fn utc_offset_seconds_uses_indonesian_longitude_bands() {
        // WIB: lon < 115 -> UTC+7 = 25200 seconds.
        assert_eq!(
            city_by_id("banda_aceh").unwrap().utc_offset_seconds(),
            25_200
        );
        assert_eq!(city_by_id("jakarta").unwrap().utc_offset_seconds(), 25_200);
        // WITA: 115 <= lon < 125 -> UTC+8 = 28800 seconds.
        assert_eq!(city_by_id("denpasar").unwrap().utc_offset_seconds(), 28_800);
        assert_eq!(city_by_id("makassar").unwrap().utc_offset_seconds(), 28_800);
        assert_eq!(city_by_id("manado").unwrap().utc_offset_seconds(), 28_800);
        // WIT: lon >= 125 -> UTC+9 = 32400 seconds.
        assert_eq!(city_by_id("ambon").unwrap().utc_offset_seconds(), 32_400);
        assert_eq!(city_by_id("jayapura").unwrap().utc_offset_seconds(), 32_400);

        // Exact band boundaries, including the `Asia/Jakarta` IANA string
        // being retained for display while the fallback drives the offset.
        let city_at =
            |longitude: f64| OfflineCity::city("x", "X", "X", 0.0, longitude, "Asia/Jakarta", 0.0);
        assert_eq!(city_at(114.9999).utc_offset_seconds(), 25_200);
        assert_eq!(city_at(115.0).utc_offset_seconds(), 28_800);
        assert_eq!(city_at(124.9999).utc_offset_seconds(), 28_800);
        assert_eq!(city_at(125.0).utc_offset_seconds(), 32_400);
    }

    #[test]
    fn haversine_is_zero_for_identical_points_and_symmetric() {
        let jakarta = Coordinates::new(-6.2088, 106.8456);
        let makkah = Coordinates::new(21.4225, 39.8262);
        assert_eq!(haversine_distance_kilometers(jakarta, jakarta), 0.0);
        let there = haversine_distance_kilometers(jakarta, makkah);
        let back = haversine_distance_kilometers(makkah, jakarta);
        assert!((there - back).abs() < 1e-9);
        assert!(there > 0.0);
    }
}
