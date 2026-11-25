// #307 — the prayer card has no DOM test harness in this repo (bun test only
// covers .ts), so the decision that drives the .svelte file is tested here:
// nothing rendered when the shell has no data, countdown and warning when it
// does. PrayerCard.svelte is a thin renderer of `prayerCardModel`.
import { describe, expect, test } from 'bun:test';
import {
  DEFAULT_PRAYER_SETTINGS,
  PRAYER_WARNING_MINUTES,
  countdownLabel,
  prayerCardModel,
  prayerSettingsFromSettings,
} from './prayer';
import { SETTINGS_KEYS } from './settings';
import type { PrayerSnapshot } from './types';

function snapshot(overrides: Partial<PrayerSnapshot> = {}): PrayerSnapshot {
  return {
    city_id: 'jakarta',
    city_name: 'Jakarta',
    method: 'Kemenag RI',
    correction_minutes: 0,
    card_mode: 'compact',
    hijri: '1 Ramadan 1447',
    times: [
      { key: 'subuh', name: 'Fajr', time: '04:27', epoch_seconds: 1000 },
      { key: 'maghrib', name: 'Maghrib', time: '18:18', epoch_seconds: 9000 },
    ],
    next: { key: 'subuh', name: 'Fajr', time: '04:27', epoch_seconds: 1000 },
    in_minutes: 42,
    urgent: false,
    ...overrides,
  };
}

describe('PrayerCard (#307)', () => {
  test('nothing is rendered when the shell has no prayer data', () => {
    expect(prayerCardModel(null)).toBeNull();
    expect(prayerCardModel(undefined)).toBeNull();
    expect(prayerCardModel(snapshot({ next: null }))).toBeNull();
    expect(prayerCardModel(snapshot({ times: [] }))).toBeNull();
  });

  test('a malformed countdown never shows a fake time', () => {
    expect(prayerCardModel(snapshot({ in_minutes: Number.NaN }))).toBeNull();
    expect(prayerCardModel(snapshot({ next: { ...snapshot().next!, time: '' } }))).toBeNull();
  });

  test('available data gives a countdown and the day\'s times', () => {
    const model = prayerCardModel(snapshot());
    expect(model).not.toBeNull();
    expect(model!.nextName).toBe('Fajr');
    expect(model!.nextTime).toBe('04:27');
    expect(model!.countdown).toBe('42 min');
    expect(model!.warning).toBeNull();
    expect(model!.times.map((row) => row.time)).toEqual(['04:27', '18:18']);
    expect(model!.hijri).toBe('1 Ramadan 1447');
  });

  test('an imminent prayer is flagged', () => {
    const model = prayerCardModel(snapshot({ in_minutes: 10, urgent: true }))!;
    expect(model.countdown).toBe('10 min');
    expect(model.warning).toBe(`Within ${PRAYER_WARNING_MINUTES} minutes`);
  });

  test('long countdowns are hours, not three-digit minutes', () => {
    expect(countdownLabel(0)).toBe('now');
    expect(countdownLabel(1)).toBe('1 min');
    expect(countdownLabel(60)).toBe('1 hr');
    expect(countdownLabel(95)).toBe('1 hr 35 min');
  });

  test('the settings page reads and validates its four original keys', () => {
    expect(prayerSettingsFromSettings(null)).toEqual(DEFAULT_PRAYER_SETTINGS);
    expect(
      prayerSettingsFromSettings({
        [SETTINGS_KEYS.prayerCityId]: 'surabaya',
        [SETTINGS_KEYS.prayerMethod]: 'nuLFPBNU',
        [SETTINGS_KEYS.prayerCorrectionMinutes]: 3,
        [SETTINGS_KEYS.prayerHijriSource]: 'neoMABIMS',
      }),
    ).toEqual({
      ...DEFAULT_PRAYER_SETTINGS,
      cityId: 'surabaya',
      method: 'nuLFPBNU',
      correctionMinutes: 3,
      hijriSource: 'neoMABIMS',
    });
    expect(
      prayerSettingsFromSettings({
        [SETTINGS_KEYS.prayerMethod]: 'bogus',
        [SETTINGS_KEYS.prayerCorrectionMinutes]: 9999,
      }),
    ).toEqual({ ...DEFAULT_PRAYER_SETTINGS, correctionMinutes: 120 });
  });

  test('#462 parity keys keep the card on for old documents and bound new values', () => {
    // No parity keys means the pre-#462 document: same card, no chime.
    const legacy = prayerSettingsFromSettings({
      [SETTINGS_KEYS.prayerCityId]: 'jakarta',
    });
    expect(legacy.enabled).toBe(true);
    expect(legacy.cardMode).toBe('compact');
    expect(legacy.hijriDayOffset).toBe(0);
    expect(legacy.locationMode).toBe('manual');
    expect(legacy.soundEnabled).toBe(false);

    const full = prayerSettingsFromSettings({
      [SETTINGS_KEYS.prayerEnabled]: false,
      [SETTINGS_KEYS.prayerCardMode]: 'panoramic',
      [SETTINGS_KEYS.prayerHijriOffset]: 5,
      [SETTINGS_KEYS.prayerLocationMode]: 'automatic',
      [SETTINGS_KEYS.prayerSoundEnabled]: true,
    });
    expect(full.enabled).toBe(false);
    expect(full.cardMode).toBe('panoramic');
    expect(full.hijriDayOffset).toBe(2);
    // #443: no automatic location adapter exists, so it cannot be selected.
    expect(full.locationMode).toBe('manual');
    expect(full.soundEnabled).toBe(true);

    const mistyped = prayerSettingsFromSettings({
      [SETTINGS_KEYS.prayerEnabled]: 'yes',
      [SETTINGS_KEYS.prayerCardMode]: 'huge',
      [SETTINGS_KEYS.prayerHijriOffset]: -8,
      [SETTINGS_KEYS.prayerSoundEnabled]: 1,
    });
    expect(mistyped).toEqual({ ...DEFAULT_PRAYER_SETTINGS, hijriDayOffset: -2 });
  });

  test('the snapshot carries the card layout the island opens with', () => {
    expect(prayerCardModel(snapshot())!.cardMode).toBe('compact');
    expect(prayerCardModel(snapshot({ card_mode: 'panoramic' }))!.cardMode).toBe('panoramic');
    expect(prayerCardModel(snapshot({ card_mode: 'bogus' }))!.cardMode).toBe('compact');
  });
});
