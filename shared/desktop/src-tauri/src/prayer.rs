//! The user-visible prayer slice (#307): settings, the next prayer and its
//! countdown, and one OS notification per fardh milestone. The astronomical
//! work stays in `morrow_core::prayer`; this module only reads the settings
//! file, chooses a city and hands the shell's `notify-send` adapter a message.
//!
//! Every calculation helper takes an injected `now`. Only the commands (and
//! their `SystemTime::now()` call) touch the real clock, so the tests can pin
//! a day and check the shape without a desktop or a notification daemon.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use morrow_core::prayer::{
    DailyPrayerSchedule, HijriCalendarSource, OfflineCity, PrayerCalculationMethod,
    PrayerTimeCalculator, PrayerType, PrayerWakePolicy, YearMonthDay, cities,
};
use morrow_platform::notify::notify;
use morrow_platform::runner::Runner;
use morrow_platform::settings;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, State};

use crate::HooksSite;
use crate::settings::SETTINGS_CHANGED_EVENT;

/// The four keys this slice owns, spelled like their Swift `SettingsKey`s.
pub const KEY_CITY: &str = "prayerManualCityId";
pub const KEY_METHOD: &str = "prayerCalculationMethod";
pub const KEY_CORRECTION: &str = "prayerCorrectionMinutes";
pub const KEY_HIJRI: &str = "prayerHijriCalendarSource";

const KEYS: [&str; 4] = [KEY_CITY, KEY_METHOD, KEY_CORRECTION, KEY_HIJRI];

/// What the Settings page edits, with the task's fallbacks for missing values.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PrayerSettings {
    pub city_id: String,
    pub method: PrayerCalculationMethod,
    pub correction_minutes: i32,
    pub hijri_source: HijriCalendarSource,
}

/// One row of the expanded card.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PrayerTimeView {
    pub key: String,
    pub name: String,
    /// Local `HH:MM` at the selected city's fixed UTC offset.
    pub time: String,
    pub epoch_seconds: i64,
}

/// The card's whole view model. `None` from `prayer_next` means the settings
/// document has no prayer keys yet: the frontend renders no card at all
/// rather than inventing a time for a city the user never chose.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PrayerSnapshot {
    pub city_id: String,
    pub city_name: String,
    pub method: String,
    pub correction_minutes: i32,
    pub hijri: String,
    pub times: Vec<PrayerTimeView>,
    pub next: Option<PrayerTimeView>,
    pub in_minutes: i64,
    pub urgent: bool,
}

/// A city row for the Settings picker.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CityOption {
    pub id: String,
    pub name: String,
    pub province: String,
}

/// The de-duplication policy lives across `prayer_tick` calls in the shell.
#[derive(Debug, Default)]
pub struct PrayerWakeState(pub Mutex<PrayerWakePolicy>);

impl PrayerWakeState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Parse one value from the settings document, falling back per key. A corrupt
/// or mistyped value is treated exactly like a missing one; it is never
/// written back here.
pub fn prayer_settings_from_value(value: &Value) -> PrayerSettings {
    // Owned strings: a closure cannot name the borrow's lifetime, and every
    // value here is tiny.
    let text = |key: &str| -> Option<String> {
        value
            .as_object()
            .and_then(|object| object.get(key))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    let enumerated =
        |key: &str, fallback: &str| -> String { text(key).unwrap_or_else(|| fallback.to_owned()) };
    let method = serde_json::from_value::<PrayerCalculationMethod>(Value::String(enumerated(
        KEY_METHOD, "kemenag",
    )))
    .unwrap_or(PrayerCalculationMethod::Kemenag);
    let hijri_source =
        serde_json::from_value::<HijriCalendarSource>(Value::String(enumerated(KEY_HIJRI, "khgt")))
            .unwrap_or(HijriCalendarSource::Khgt);
    let correction_minutes = value
        .as_object()
        .and_then(|object| object.get(KEY_CORRECTION))
        .and_then(Value::as_i64)
        .map(|minutes| minutes.clamp(-120, 120) as i32)
        .unwrap_or(0);

    PrayerSettings {
        city_id: text(KEY_CITY).unwrap_or_else(|| "jakarta".to_owned()),
        method,
        correction_minutes,
        hijri_source,
    }
}

fn has_prayer_settings(value: &Value) -> bool {
    value
        .as_object()
        .is_some_and(|object| KEYS.iter().any(|key| object.contains_key(*key)))
}

/// Everything the view and the notifier need from one settings document and
/// one injected instant.
pub struct Prepared {
    settings: PrayerSettings,
    city: OfflineCity,
    day: YearMonthDay,
    schedule: DailyPrayerSchedule,
    correction_seconds: i64,
}

fn prepare(stored: &Value, now_epoch_seconds: i64) -> Option<Prepared> {
    if !has_prayer_settings(stored) {
        return None;
    }
    let settings = prayer_settings_from_value(stored);
    let city = cities::city_by_id(&settings.city_id).unwrap_or_else(cities::default_city);
    let offset = city.utc_offset_seconds();
    let day = YearMonthDay::from_epoch_seconds(now_epoch_seconds, offset);
    let schedule = PrayerTimeCalculator::calculate(
        day,
        city.coordinates,
        city.elevation_meters,
        settings.method,
        offset,
    );
    let correction_seconds = i64::from(settings.correction_minutes) * 60;
    Some(Prepared {
        settings,
        city,
        day,
        schedule,
        correction_seconds,
    })
}

fn format_clock(epoch_seconds: i64, utc_offset_seconds: i32) -> String {
    let minutes = (epoch_seconds + i64::from(utc_offset_seconds))
        .div_euclid(60)
        .rem_euclid(1440);
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

fn view(prayer: PrayerType, due: i64, offset: i32) -> PrayerTimeView {
    PrayerTimeView {
        key: prayer.key().to_owned(),
        name: prayer.display_name().to_owned(),
        time: format_clock(due, offset),
        epoch_seconds: due,
    }
}

/// Tomorrow's Fajr, for the hours after today's Isha.
fn tomorrow_subuh(prepared: &Prepared) -> i64 {
    let offset = prepared.city.utc_offset_seconds();
    let tomorrow =
        YearMonthDay::from_epoch_seconds(prepared.schedule.day_start_epoch() + 86_400, offset);
    let next = PrayerTimeCalculator::calculate(
        tomorrow,
        prepared.city.coordinates,
        prepared.city.elevation_meters,
        prepared.settings.method,
        offset,
    );
    next.subuh.epoch_seconds + prepared.correction_seconds
}

/// Pure view model for the island card. `None` when the user has not saved any
/// prayer setting yet.
pub fn prayer_snapshot(stored: &Value, now_epoch_seconds: i64) -> Option<PrayerSnapshot> {
    let prepared = prepare(stored, now_epoch_seconds)?;
    let offset = prepared.city.utc_offset_seconds();
    let times: Vec<PrayerTimeView> = prepared
        .schedule
        .five_prayers()
        .into_iter()
        .map(|(prayer, moment)| {
            view(
                prayer,
                moment.epoch_seconds + prepared.correction_seconds,
                offset,
            )
        })
        .collect();
    let next = times
        .iter()
        .find(|row| row.epoch_seconds > now_epoch_seconds)
        .cloned()
        .or_else(|| Some(view(PrayerType::Subuh, tomorrow_subuh(&prepared), offset)));
    let due = next.as_ref().map_or(0, |row| row.epoch_seconds);
    let seconds = due - now_epoch_seconds;
    let in_minutes = if seconds > 0 { (seconds + 59) / 60 } else { 0 };
    let hijri = morrow_core::prayer::HijriCalendarCalculator::calculate(
        prepared.day,
        prepared.settings.hijri_source,
        0,
    )
    .formatted_string();

    Some(PrayerSnapshot {
        city_id: prepared.city.id.clone(),
        city_name: prepared.city.name.clone(),
        method: prepared.settings.method.display_name().to_owned(),
        correction_minutes: prepared.settings.correction_minutes,
        hijri,
        times,
        next,
        in_minutes,
        urgent: in_minutes <= 15,
    })
}

/// Send the reminders that are due right now. The notification adapter is
/// injected so a test's fake `Runner` records calls instead of talking to a
/// desktop; the policy is injected so suspend/unlock replay cannot duplicate
/// a milestone or dump a backlog.
pub fn notify_due_prayers(
    prepared: &Prepared,
    policy: &mut PrayerWakePolicy,
    now_epoch_seconds: i64,
    runner: &dyn Runner,
) -> Result<usize, String> {
    let offset = prepared.city.utc_offset_seconds();
    let mut sent = 0;
    for prayer in PrayerType::FARDH {
        let due = prepared.schedule.time(prayer).epoch_seconds + prepared.correction_seconds;
        // Key the milestone by the day it belongs to, not the day we woke up
        // on: an Isha just before midnight replayed just after it is still the
        // same notification.
        let milestone_day = YearMonthDay::from_epoch_seconds(due, offset);
        if policy.due(milestone_day, prayer, due, now_epoch_seconds) {
            let row = view(prayer, due, offset);
            let body = format!("{} · {} · {}", row.name, row.time, prepared.city.name);
            notify(runner, "Prayer time", &body)?;
            sent += 1;
        }
    }
    Ok(sent)
}

fn now_epoch_seconds() -> Result<i64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .map_err(|error| error.to_string())
}

// --- Tauri commands -------------------------------------------------------

#[tauri::command]
pub fn prayer_settings_load(site: State<'_, HooksSite>) -> Result<PrayerSettings, String> {
    let stored = settings::load(&site.paths.codeisland_dir).map_err(|error| error.to_string())?;
    Ok(prayer_settings_from_value(&stored))
}

#[tauri::command]
pub fn prayer_settings_set(
    app: AppHandle,
    site: State<'_, HooksSite>,
    key: String,
    value: Value,
) -> Result<bool, String> {
    let changed = settings::set(&site.paths.codeisland_dir, &key, &value)
        .map_err(|error| error.to_string())?;
    // Same event as the Appearance page: the island adopts a new city or
    // method without a restart.
    if let Ok(stored) = settings::load(&site.paths.codeisland_dir) {
        let _ = app.emit(SETTINGS_CHANGED_EVENT, stored);
    }
    Ok(changed)
}

#[tauri::command]
pub fn prayer_cities() -> Vec<CityOption> {
    cities::all_cities()
        .iter()
        .map(|city| CityOption {
            id: city.id.clone(),
            name: city.name.clone(),
            province: city.province_or_country.clone(),
        })
        .collect()
}

#[tauri::command]
pub fn prayer_next(
    site: State<'_, HooksSite>,
    feed: State<'_, crate::feed::Feed>,
) -> Result<Option<PrayerSnapshot>, String> {
    let stored = match settings::load(&site.paths.codeisland_dir) {
        Ok(stored) => stored,
        Err(_) => return Ok(None),
    };
    let view = prayer_snapshot(&stored, now_epoch_seconds()?);
    feed.set_prayer(view.clone());
    Ok(view)
}

#[tauri::command]
pub fn prayer_tick(
    site: State<'_, HooksSite>,
    wake: State<'_, PrayerWakeState>,
    feed: State<'_, crate::feed::Feed>,
) -> Result<usize, String> {
    let stored = settings::load(&site.paths.codeisland_dir).map_err(|error| error.to_string())?;
    let now = now_epoch_seconds()?;
    // #307: the card travels in the feed snapshot, not only in this command's
    // return value. A tick refreshes it even when no notification is due.
    feed.set_prayer(prayer_snapshot(&stored, now));
    let Some(prepared) = prepare(&stored, now) else {
        return Ok(0);
    };
    let mut policy = wake
        .0
        .lock()
        .map_err(|_| "prayer wake state poisoned".to_owned())?;
    notify_due_prayers(
        &prepared,
        &mut policy,
        now,
        &morrow_platform::runner::SystemRunner,
    )
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    const DAY: YearMonthDay = YearMonthDay::new(2026, 1, 15);

    fn stored(city: &str, method: &str, minutes: i64, hijri: &str) -> Value {
        serde_json::json!({
            KEY_CITY: city,
            KEY_METHOD: method,
            KEY_CORRECTION: minutes,
            KEY_HIJRI: hijri,
        })
    }

    fn jakarta_schedule() -> DailyPrayerSchedule {
        let city = cities::city_by_id("jakarta").expect("the catalog has Jakarta");
        PrayerTimeCalculator::calculate(
            DAY,
            city.coordinates,
            city.elevation_meters,
            PrayerCalculationMethod::Kemenag,
            city.utc_offset_seconds(),
        )
    }

    #[derive(Default)]
    struct FakeRunner {
        calls: RefCell<Vec<(String, String)>>,
    }

    impl Runner for FakeRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
            assert_eq!(program, "notify-send");
            self.calls.borrow_mut().push((
                args[args.len() - 2].to_owned(),
                args[args.len() - 1].to_owned(),
            ));
            Ok(String::new())
        }
    }

    #[test]
    fn settings_fall_back_to_jakarta_kemenag_zero_and_khgt() {
        assert_eq!(
            prayer_settings_from_value(&serde_json::json!({})),
            PrayerSettings {
                city_id: "jakarta".into(),
                method: PrayerCalculationMethod::Kemenag,
                correction_minutes: 0,
                hijri_source: HijriCalendarSource::Khgt,
            }
        );
        let partial = serde_json::json!({ KEY_CITY: "surabaya", KEY_CORRECTION: 7 });
        let parsed = prayer_settings_from_value(&partial);
        assert_eq!(parsed.city_id, "surabaya");
        assert_eq!(parsed.correction_minutes, 7);
        assert_eq!(parsed.method, PrayerCalculationMethod::Kemenag);
        assert_eq!(parsed.hijri_source, HijriCalendarSource::Khgt);

        let bad = serde_json::json!({
            KEY_METHOD: "not-a-method",
            KEY_HIJRI: 42,
            KEY_CORRECTION: 9000,
        });
        let parsed = prayer_settings_from_value(&bad);
        assert_eq!(parsed.method, PrayerCalculationMethod::Kemenag);
        assert_eq!(parsed.hijri_source, HijriCalendarSource::Khgt);
        assert_eq!(parsed.correction_minutes, 120);
    }

    #[test]
    fn the_command_reads_a_real_home_settings_document() {
        // #307 review: the earlier tests fed in-memory Values, so the
        // HOME/codeisland_dir path the command actually uses was never
        // exercised. Here the settings are written to a temp HOME and read
        // back through the same platform helper the command calls.
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("a clock")
            .as_nanos();
        let home =
            std::env::temp_dir().join(format!("morrow-prayer-home-{}-{nanos}", std::process::id()));
        let codeisland = home.join(".codeisland");
        std::fs::create_dir_all(&codeisland).expect("a temp home");
        for (key, value) in [
            (KEY_CITY, serde_json::json!("jakarta")),
            (KEY_METHOD, serde_json::json!("kemenag")),
            (KEY_CORRECTION, serde_json::json!(0)),
            (KEY_HIJRI, serde_json::json!("khgt")),
        ] {
            morrow_platform::settings::set(&codeisland, key, &value).expect("a saved setting");
        }
        let stored = morrow_platform::settings::load(&codeisland).expect("a readable document");
        let view = prayer_snapshot(&stored, now_epoch_seconds().expect("a clock"))
            .expect("a saved city has a schedule");
        assert_eq!(view.city_id, "jakarta");
        assert_eq!(view.city_name, "Jakarta");
        assert_eq!(view.times.len(), 5);
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn no_prayer_settings_means_no_card() {
        assert!(prayer_snapshot(&serde_json::json!({ "theme": "pixel" }), 0).is_none());
    }

    #[test]
    fn snapshot_uses_the_city_offset_and_manual_correction() {
        let schedule = jakarta_schedule();
        let now = schedule.subuh.epoch_seconds - 60;
        let snapshot = prayer_snapshot(&stored("jakarta", "kemenag", 5, "khgt"), now)
            .expect("prayer settings are present");
        assert_eq!(snapshot.city_id, "jakarta");
        assert_eq!(snapshot.city_name, "Jakarta");
        assert_eq!(snapshot.correction_minutes, 5);
        assert_eq!(snapshot.times.len(), 5);
        assert_eq!(snapshot.times[0].key, "subuh");
        assert_eq!(
            snapshot.times[0].epoch_seconds,
            schedule.subuh.epoch_seconds + 300
        );
        assert!(!snapshot.hijri.is_empty());

        let next = snapshot.next.expect("a next prayer");
        assert_eq!(next.key, "subuh");
        assert_eq!(next.epoch_seconds, schedule.subuh.epoch_seconds + 300);
        assert_eq!(snapshot.in_minutes, 6);
        // Six minutes is inside the 15-minute "about to start" window.
        assert!(snapshot.urgent);
    }

    #[test]
    fn snapshot_warns_when_the_next_prayer_is_close() {
        let schedule = jakarta_schedule();
        let now = schedule.dzuhur.epoch_seconds - 10 * 60;
        let snapshot = prayer_snapshot(&stored("jakarta", "kemenag", 0, "neoMABIMS"), now)
            .expect("prayer settings are present");
        assert_eq!(
            snapshot.next.as_ref().map(|row| row.key.as_str()),
            Some("dzuhur")
        );
        assert_eq!(snapshot.in_minutes, 10);
        assert!(snapshot.urgent);
    }

    #[test]
    fn unknown_city_falls_back_without_a_fake_time() {
        let schedule = jakarta_schedule();
        let snapshot = prayer_snapshot(
            &stored("nowhere", "kemenag", 0, "khgt"),
            schedule.subuh.epoch_seconds - 60,
        )
        .expect("a city key is present");
        assert_eq!(snapshot.city_id, "jakarta");
        assert!(snapshot.next.is_some());
    }

    #[test]
    fn fake_runner_gets_one_notification_per_due_milestone() {
        let schedule = jakarta_schedule();
        let now = schedule.subuh.epoch_seconds;
        let prepared = prepare(&stored("jakarta", "kemenag", 0, "khgt"), now).expect("prepared");
        let runner = FakeRunner::default();
        let mut policy = PrayerWakePolicy::new();

        assert_eq!(
            notify_due_prayers(&prepared, &mut policy, now, &runner),
            Ok(1)
        );
        // A coalesced timer or an unlock replays the same instant: no second
        // notification, and no real `notify-send` call.
        assert_eq!(
            notify_due_prayers(&prepared, &mut policy, now, &runner),
            Ok(0)
        );
        let calls = runner.calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "Prayer time");
        // The notification uses the core's English display name ("Fajr"),
        // matching the English "Prayer time" title.
        assert!(calls[0].1.contains("Fajr"));
    }

    #[test]
    fn suspend_does_not_replay_the_day_s_backlog() {
        let schedule = jakarta_schedule();
        let now = schedule.subuh.epoch_seconds + 3 * 3600;
        let prepared = prepare(&stored("jakarta", "kemenag", 0, "khgt"), now).expect("prepared");
        let runner = FakeRunner::default();
        let mut policy = PrayerWakePolicy::new();

        assert_eq!(
            notify_due_prayers(&prepared, &mut policy, now, &runner),
            Ok(0)
        );
        assert!(runner.calls.borrow().is_empty());
    }
}
