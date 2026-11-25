//! The user-visible prayer slice (#307): settings, the next prayer and its
//! countdown, and one OS notification per fardh milestone. The astronomical
//! work stays in `morrow_core::prayer`; this module only reads the settings
//! file, chooses a city and hands the shell's `notify-send` adapter a message.
//!
//! #462 parity adds the settings macOS also exposes: the enable switch, card
//! layout, manual Hijri day offset, location mode, chime and a test banner.
//! Automatic location is deliberately absent: it needs a GeoClue/WinRT
//! permission flow (#443), so the only ready mode is the offline city.
//!
//! Every calculation helper takes an injected `now`. Only the commands (and
//! their `SystemTime::now()` call) touch the real clock, so the tests can pin
//! a day and check the shape without a desktop or a notification daemon.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use morrow_core::prayer::{
    DailyPrayerSchedule, HijriCalendarSource, OfflineCity, PrayerCalculationMethod,
    PrayerTimeCalculator, PrayerType, PrayerWakePolicy, YearMonthDay, cities,
};
use morrow_platform::notify::notify;
use morrow_platform::runner::Runner;
use morrow_platform::settings;
use morrow_platform::sound;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, State};

use crate::HooksSite;
use crate::feed::Feed;
use crate::settings::SETTINGS_CHANGED_EVENT;

/// The keys this slice owns, spelled like their Swift `SettingsKey`s.
pub const KEY_CITY: &str = "prayerManualCityId";
pub const KEY_METHOD: &str = "prayerCalculationMethod";
pub const KEY_CORRECTION: &str = "prayerCorrectionMinutes";
pub const KEY_HIJRI: &str = "prayerHijriCalendarSource";
// #462 parity keys. The first five match Swift; `prayerUsePreciseLocation` is
// not among them because it only has meaning with automatic location (#443).
pub const KEY_ENABLED: &str = "prayerEnabled";
pub const KEY_CARD_MODE: &str = "prayerCardDisplayMode";
pub const KEY_HIJRI_OFFSET: &str = "prayerHijriDayOffset";
pub const KEY_LOCATION_MODE: &str = "prayerLocationMode";
pub const KEY_SOUND_ENABLED: &str = "prayerSoundEnabled";

/// Only the original four keys make a card exist. A document with just an
/// enable toggle or a layout choice must not invent a time for a city the user
/// never chose.
const KEYS: [&str; 4] = [KEY_CITY, KEY_METHOD, KEY_CORRECTION, KEY_HIJRI];

/// The shared sound table has no prayer-specific WAV. Reuse its round-complete
/// chime for the notification and the preview so both play the same sound.
pub const PRAYER_CHIME_CUE: &str = "TaskRoundComplete";

/// What the Settings page edits, with the task's fallbacks for missing values.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PrayerSettings {
    pub city_id: String,
    pub method: PrayerCalculationMethod,
    pub correction_minutes: i32,
    pub hijri_source: HijriCalendarSource,
    // The card has always appeared once one of the four keys was saved. The
    // new switch defaults to on so existing documents keep their card.
    pub enabled: bool,
    pub card_mode: String,
    pub hijri_day_offset: i32,
    /// Always `manual` until #443 decides the automatic-location permission flow.
    pub location_mode: String,
    pub sound_enabled: bool,
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
    /// #462: `compact` or `panoramic`, so the island opens the card in the
    /// layout chosen in Settings without reading the document a second time.
    pub card_mode: String,
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
    let flag = |key: &str, fallback: bool| -> bool {
        value
            .as_object()
            .and_then(|object| object.get(key))
            .and_then(Value::as_bool)
            .unwrap_or(fallback)
    };
    let bounded = |key: &str, limit: i64| -> i32 {
        value
            .as_object()
            .and_then(|object| object.get(key))
            .and_then(Value::as_i64)
            .map(|days| days.clamp(-limit, limit) as i32)
            .unwrap_or(0)
    };
    let card_mode = if text(KEY_CARD_MODE).as_deref() == Some("panoramic") {
        "panoramic".to_owned()
    } else {
        "compact".to_owned()
    };
    // Nothing can resolve coordinates on Linux or Windows yet. Collapse any
    // stored "automatic" to manual so no part of the shell claims it works.
    let location_mode = text(KEY_LOCATION_MODE)
        .filter(|mode| mode == "manual")
        .unwrap_or_else(|| "manual".to_owned());

    PrayerSettings {
        city_id: text(KEY_CITY).unwrap_or_else(|| "jakarta".to_owned()),
        method,
        correction_minutes,
        hijri_source,
        enabled: flag(KEY_ENABLED, true),
        card_mode,
        hijri_day_offset: bounded(KEY_HIJRI_OFFSET, 2),
        location_mode,
        sound_enabled: flag(KEY_SOUND_ENABLED, false),
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
    // The switch suppresses the card and the reminders together: a disabled
    // prayer slice must not keep waking the notifier in the background.
    if !settings.enabled {
        return None;
    }
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
        prepared.settings.hijri_day_offset,
    )
    .formatted_string();

    Some(PrayerSnapshot {
        city_id: prepared.city.id.clone(),
        city_name: prepared.city.name.clone(),
        method: prepared.settings.method.display_name().to_owned(),
        correction_minutes: prepared.settings.correction_minutes,
        card_mode: prepared.settings.card_mode.clone(),
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

fn play_chime_default(cue: &str) {
    if let Some(player) = sound::player_on_path() {
        let _ = sound::play(player, cue);
    }
}

/// The outside-world inputs the prayer commands depend on: where the user's
/// settings live (derived from HOME at launch), the current clock, who posts
/// notifications and who plays the chime. The Tauri commands build this from
/// the real shell; tests inject a temp home, a pinned clock, a fake runner and
/// a recording chime so no desktop, clock or player is needed.
pub struct PrayerSite {
    codeisland_dir: PathBuf,
    now: Box<dyn Fn() -> Result<i64, String>>,
    runner: Box<dyn Runner>,
    chime: Box<dyn Fn(&str)>,
}

impl PrayerSite {
    /// The production wiring: the shell's resolved home, the system clock and
    /// the real desktop adapter.
    pub fn system(site: &HooksSite) -> Self {
        Self::new(
            site.paths.codeisland_dir.clone(),
            Box::new(now_epoch_seconds as fn() -> Result<i64, String>),
            Box::new(morrow_platform::runner::SystemRunner),
        )
    }

    fn new(
        codeisland_dir: PathBuf,
        now: Box<dyn Fn() -> Result<i64, String>>,
        runner: Box<dyn Runner>,
    ) -> Self {
        Self {
            codeisland_dir,
            now,
            runner,
            chime: Box::new(play_chime_default),
        }
    }

    fn now(&self) -> Result<i64, String> {
        (self.now)()
    }

    // --- command decision logic (shared by the Tauri wrappers and the tests) --

    fn settings_load(&self) -> Result<PrayerSettings, String> {
        let stored = settings::load(&self.codeisland_dir).map_err(|error| error.to_string())?;
        Ok(prayer_settings_from_value(&stored))
    }

    fn settings_set(&self, key: &str, value: &Value) -> Result<bool, String> {
        settings::set(&self.codeisland_dir, key, value).map_err(|error| error.to_string())
    }

    fn next(&self, feed: &Feed) -> Result<Option<PrayerSnapshot>, String> {
        let stored = match settings::load(&self.codeisland_dir) {
            Ok(stored) => stored,
            Err(_) => return Ok(None),
        };
        let view = prayer_snapshot(&stored, self.now()?);
        feed.set_prayer(view.clone());
        Ok(view)
    }

    fn tick(&self, wake: &PrayerWakeState, feed: &Feed) -> Result<usize, String> {
        let stored = settings::load(&self.codeisland_dir).map_err(|error| error.to_string())?;
        let now = self.now()?;
        // #307: the card travels in the feed snapshot, not only in this
        // command's return value. A tick refreshes it even when no
        // notification is due.
        feed.set_prayer(prayer_snapshot(&stored, now));
        let Some(prepared) = prepare(&stored, now) else {
            return Ok(0);
        };
        let mut policy = wake
            .0
            .lock()
            .map_err(|_| "prayer wake state poisoned".to_owned())?;
        let sent = notify_due_prayers(&prepared, &mut policy, now, self.runner.as_ref())?;
        // #462: one chime per tick that actually sent a reminder, matching
        // Swift's `playChimeIfConfigured` on the banner path.
        if sent > 0 && prepared.settings.sound_enabled {
            (self.chime)(PRAYER_CHIME_CUE);
        }
        Ok(sent)
    }

    /// The Settings page's Test button uses the same notifier as a real
    /// milestone. The desktop's "banner" is an OS notification (#307); there
    /// is no separate in-island banner state to trigger here.
    fn test_banner(&self) -> Result<(), String> {
        notify(
            self.runner.as_ref(),
            "WAKTU MAGHRIB TIBA",
            "Saatnya Menunaikan Ibadah Salat Maghrib",
        )
    }
}

// --- Tauri commands -------------------------------------------------------

#[tauri::command]
pub fn prayer_settings_set(
    app: AppHandle,
    site: State<'_, HooksSite>,
    key: String,
    value: Value,
) -> Result<bool, String> {
    let prayer_site = PrayerSite::system(&site);
    let changed = prayer_site.settings_set(&key, &value)?;
    // Same event as the Appearance page: the island adopts a new city or
    // method without a restart.
    if let Ok(stored) = settings::load(&prayer_site.codeisland_dir) {
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
    PrayerSite::system(&site).next(&feed)
}

#[tauri::command]
pub fn prayer_tick(
    site: State<'_, HooksSite>,
    wake: State<'_, PrayerWakeState>,
    feed: State<'_, crate::feed::Feed>,
) -> Result<usize, String> {
    PrayerSite::system(&site).tick(&wake, &feed)
}

/// #462: hear the same cue a real prayer reminder plays. A preview is a direct
/// action, so it reports a missing player instead of failing silently.
#[tauri::command]
pub fn prayer_preview_chime() -> Result<(), String> {
    match sound::player_on_path() {
        Some(player) => sound::play(player, PRAYER_CHIME_CUE),
        None => Err("no sound player on this machine".to_owned()),
    }
}

/// #462: the Settings page's "Test prayer notification" button.
#[tauri::command]
pub fn prayer_test_banner(site: State<'_, HooksSite>) -> Result<(), String> {
    PrayerSite::system(&site).test_banner()
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use morrow_core::prayer::HijriCalendarCalculator;

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

    #[derive(Default, Clone)]
    struct FakeRunner {
        // `Rc` lets the seam own a clone of the runner while the test keeps a
        // handle to inspect what was (not) sent.
        calls: Rc<RefCell<Vec<(String, String)>>>,
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
                enabled: true,
                card_mode: "compact".into(),
                hijri_day_offset: 0,
                location_mode: "manual".into(),
                sound_enabled: false,
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
    fn parity_keys_are_validated_and_backward_compatible() {
        // A document written before #462 has none of the new keys. It must
        // keep its card: enabled defaults on so persisted settings do not lose
        // the schedule they already had.
        let legacy = stored("jakarta", "kemenag", 0, "khgt");
        let parsed = prayer_settings_from_value(&legacy);
        assert!(parsed.enabled);
        assert_eq!(parsed.card_mode, "compact");
        assert_eq!(parsed.hijri_day_offset, 0);
        assert_eq!(parsed.location_mode, "manual");
        assert!(!parsed.sound_enabled);

        let full = serde_json::json!({
            KEY_ENABLED: false,
            KEY_CARD_MODE: "panoramic",
            KEY_HIJRI_OFFSET: 9,
            KEY_LOCATION_MODE: "automatic",
            KEY_SOUND_ENABLED: true,
        });
        let parsed = prayer_settings_from_value(&full);
        assert!(!parsed.enabled);
        assert_eq!(parsed.card_mode, "panoramic");
        assert_eq!(parsed.hijri_day_offset, 2);
        // #443: no automatic location adapter exists, so no stored value can
        // make the shell claim to use one.
        assert_eq!(parsed.location_mode, "manual");
        assert!(parsed.sound_enabled);

        let mistyped = serde_json::json!({
            KEY_ENABLED: "yes",
            KEY_CARD_MODE: "huge",
            KEY_HIJRI_OFFSET: -9,
            KEY_LOCATION_MODE: 42,
            KEY_SOUND_ENABLED: 1,
        });
        let parsed = prayer_settings_from_value(&mistyped);
        assert!(parsed.enabled);
        assert_eq!(parsed.card_mode, "compact");
        assert_eq!(parsed.hijri_day_offset, -2);
        assert_eq!(parsed.location_mode, "manual");
        assert!(!parsed.sound_enabled);
    }

    #[test]
    fn disabling_the_slice_stops_the_card_and_the_reminders() {
        let now = jakarta_schedule().subuh.epoch_seconds;
        let mut disabled = stored("jakarta", "kemenag", 0, "khgt");
        disabled[KEY_ENABLED] = serde_json::json!(false);
        assert!(prayer_snapshot(&disabled, now).is_none());
        assert!(prepare(&disabled, now).is_none());

        let mut enabled = disabled;
        enabled[KEY_ENABLED] = serde_json::json!(true);
        let prepared = prepare(&enabled, now).expect("enabled has a schedule");
        let runner = FakeRunner::default();
        let mut policy = PrayerWakePolicy::new();
        assert_eq!(
            notify_due_prayers(&prepared, &mut policy, now, &runner),
            Ok(1)
        );
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
    fn command_path_uses_the_injected_home_clock_and_fake_runner() {
        // #307: go through the exact decision logic the Tauri commands call,
        // with a temp home, a pinned clock and a fake notifier. No real clock,
        // no `notify-send`, no writes outside the temp dir.
        use std::time::{SystemTime, UNIX_EPOCH};

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("a clock")
            .as_nanos();
        let home =
            std::env::temp_dir().join(format!("morrow-prayer-cmd-{}-{nanos}", std::process::id()));
        let codeisland = home.join(".codeisland");
        std::fs::create_dir_all(&codeisland).expect("a temp home");

        // The injected home is the one the command reads settings from.
        for (key, value) in [
            (KEY_CITY, serde_json::json!("jakarta")),
            (KEY_METHOD, serde_json::json!("kemenag")),
            (KEY_CORRECTION, serde_json::json!(0)),
            (KEY_HIJRI, serde_json::json!("khgt")),
            (KEY_SOUND_ENABLED, serde_json::json!(true)),
        ] {
            settings::set(&codeisland, key, &value).expect("a saved setting");
        }

        let schedule = jakarta_schedule();
        // An injected, pinned clock: one minute before Fajr.
        let now = schedule.subuh.epoch_seconds - 60;
        let runner = FakeRunner::default();
        let chimes: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let recorded = chimes.clone();
        let mut site = PrayerSite::new(
            codeisland.clone(),
            Box::new(move || Ok::<i64, String>(now)),
            Box::new(runner.clone()),
        );
        // No player is started: the chime goes to the recording seam.
        site.chime = Box::new(move |cue| recorded.borrow_mut().push(cue.to_owned()));
        let feed = Feed::default();
        let wake = PrayerWakeState::new();

        // `prayer_next`'s path: load from the injected home, compute with the
        // injected clock, and publish the card into the feed snapshot.
        assert_eq!(
            site.settings_load().expect("settings load").city_id,
            "jakarta"
        );
        let snapshot = site.next(&feed).expect("prayer_next").expect("a card");
        assert_eq!(snapshot.city_name, "Jakarta");
        assert_eq!(snapshot.times.len(), 5);
        assert_eq!(
            snapshot.next.as_ref().map(|row| row.key.as_str()),
            Some("subuh")
        );
        assert_eq!(snapshot.in_minutes, 1);
        assert!(snapshot.urgent);
        assert_eq!(snapshot.card_mode, "compact");
        assert_eq!(feed.snapshot().prayer, Some(snapshot));

        // A tick before Fajr refreshes the card but sends nothing and is silent.
        assert_eq!(site.tick(&wake, &feed), Ok(0));
        assert!(runner.calls.borrow().is_empty());
        assert!(chimes.borrow().is_empty());

        // `prayer_tick` at the exact Fajr instant: one notification through the
        // fake runner, one chime, and the card refreshed to the next prayer.
        let due = schedule.subuh.epoch_seconds;
        let recorded = chimes.clone();
        let mut tick_site = PrayerSite::new(
            codeisland.clone(),
            Box::new(move || Ok::<i64, String>(due)),
            Box::new(runner.clone()),
        );
        tick_site.chime = Box::new(move |cue| recorded.borrow_mut().push(cue.to_owned()));
        assert_eq!(tick_site.tick(&wake, &feed), Ok(1));
        {
            let calls = runner.calls.borrow();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].0, "Prayer time");
            assert!(calls[0].1.contains("Fajr"));
        }
        assert_eq!(*chimes.borrow(), vec![PRAYER_CHIME_CUE.to_owned()]);
        let refreshed = feed.snapshot().prayer.expect("a card after the tick");
        assert_eq!(
            refreshed.next.as_ref().map(|row| row.key.as_str()),
            Some("dzuhur")
        );

        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn no_prayer_settings_means_no_card() {
        assert!(prayer_snapshot(&serde_json::json!({ "theme": "pixel" }), 0).is_none());
    }

    #[test]
    fn hijri_offset_and_card_mode_reach_the_snapshot() {
        let now = jakarta_schedule().subuh.epoch_seconds - 60;
        let mut configured = stored("jakarta", "kemenag", 0, "khgt");
        configured[KEY_HIJRI_OFFSET] = serde_json::json!(1);
        configured[KEY_CARD_MODE] = serde_json::json!("panoramic");
        let snapshot = prayer_snapshot(&configured, now).expect("a saved city has a card");
        assert_eq!(snapshot.card_mode, "panoramic");
        assert_eq!(
            snapshot.hijri,
            HijriCalendarCalculator::calculate(DAY, HijriCalendarSource::Khgt, 1)
                .formatted_string()
        );
        assert_ne!(
            snapshot.hijri,
            HijriCalendarCalculator::calculate(DAY, HijriCalendarSource::Khgt, 0)
                .formatted_string()
        );
    }

    #[test]
    fn test_banner_posts_the_same_notification_as_a_real_milestone() {
        let runner = FakeRunner::default();
        let site = PrayerSite::new(
            std::env::temp_dir(),
            Box::new(|| Err("the test banner never reads the clock".to_owned())),
            Box::new(runner.clone()),
        );
        assert_eq!(site.test_banner(), Ok(()));
        let calls = runner.calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "WAKTU MAGHRIB TIBA");
        assert_eq!(calls[0].1, "Saatnya Menunaikan Ibadah Salat Maghrib");
    }

    #[test]
    fn a_silent_slice_still_notifies_without_starting_a_player() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("a clock")
            .as_nanos();
        let home = std::env::temp_dir().join(format!(
            "morrow-prayer-silent-{}-{nanos}",
            std::process::id()
        ));
        let codeisland = home.join(".codeisland");
        std::fs::create_dir_all(&codeisland).expect("a temp home");
        for (key, value) in [
            (KEY_CITY, serde_json::json!("jakarta")),
            (KEY_METHOD, serde_json::json!("kemenag")),
            (KEY_CORRECTION, serde_json::json!(0)),
            (KEY_HIJRI, serde_json::json!("khgt")),
            (KEY_SOUND_ENABLED, serde_json::json!(false)),
        ] {
            settings::set(&codeisland, key, &value).expect("a saved setting");
        }
        let due = jakarta_schedule().subuh.epoch_seconds;
        let runner = FakeRunner::default();
        let chimes: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let recorded = chimes.clone();
        let mut site = PrayerSite::new(
            codeisland,
            Box::new(move || Ok::<i64, String>(due)),
            Box::new(runner.clone()),
        );
        site.chime = Box::new(move |cue| recorded.borrow_mut().push(cue.to_owned()));
        let feed = Feed::default();
        assert_eq!(site.tick(&PrayerWakeState::new(), &feed), Ok(1));
        assert_eq!(runner.calls.borrow().len(), 1);
        assert!(chimes.borrow().is_empty());
        std::fs::remove_dir_all(&home).ok();
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
