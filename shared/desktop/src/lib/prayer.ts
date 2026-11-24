// #307 — the prayer card's view model and the Prayer settings page's values.
// The shell owns the calculation and the settings file; this module only
// validates what came back and turns it into words the card can draw. Keeping
// that decision out of the .svelte file makes it testable with the repo's bun
// test harness (which has no DOM), and keeps a malformed answer from ever
// drawing a made-up time.

import { SETTINGS_KEYS } from './settings';
import type { PrayerSnapshot } from './types';

export const PRAYER_METHODS = [
  { value: 'kemenag', label: 'Kemenag RI' },
  { value: 'nuLFPBNU', label: 'Nahdlatul Ulama (LF PBNU)' },
  { value: 'muhammadiyah', label: 'Muhammadiyah (MTT)' },
] as const;

export const HIJRI_SOURCES = [
  { value: 'khgt', label: 'KHGT (Global Tunggal)' },
  { value: 'neoMABIMS', label: 'Takwim Kemenag (Neo-MABIMS)' },
] as const;

export const PRAYER_WARNING_MINUTES = 15;

export type PrayerCity = { id: string; name: string; province: string };

export type PrayerSettings = {
  cityId: string;
  method: string;
  correctionMinutes: number;
  hijriSource: string;
};

export const DEFAULT_PRAYER_SETTINGS: PrayerSettings = {
  cityId: 'jakarta',
  method: 'kemenag',
  correctionMinutes: 0,
  hijriSource: 'khgt',
};

const allowed = (value: unknown, options: readonly { value: string }[], fallback: string) =>
  options.some((option) => option.value === value) ? (value as string) : fallback;

function object(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

/** Read the four prayer keys from the settings document, with the same
 *  fallbacks the shell uses. */
export function prayerSettingsFromSettings(value: unknown): PrayerSettings {
  const stored = object(value) ?? {};
  const cityId = stored[SETTINGS_KEYS.prayerCityId];
  const minutes = stored[SETTINGS_KEYS.prayerCorrectionMinutes];
  return {
    cityId: typeof cityId === 'string' && cityId ? cityId : DEFAULT_PRAYER_SETTINGS.cityId,
    method: allowed(stored[SETTINGS_KEYS.prayerMethod], PRAYER_METHODS, DEFAULT_PRAYER_SETTINGS.method),
    correctionMinutes:
      typeof minutes === 'number' && Number.isFinite(minutes)
        ? Math.max(-120, Math.min(120, Math.trunc(minutes)))
        : DEFAULT_PRAYER_SETTINGS.correctionMinutes,
    hijriSource: allowed(
      stored[SETTINGS_KEYS.prayerHijriSource],
      HIJRI_SOURCES,
      DEFAULT_PRAYER_SETTINGS.hijriSource,
    ),
  };
}

export type PrayerCardModel = {
  cityName: string;
  method: string;
  correctionMinutes: number;
  hijri: string;
  nextName: string;
  nextTime: string;
  countdown: string;
  warning: string | null;
  times: { key: string; name: string; time: string }[];
};

/** Compact countdown wording; the shell sends whole minutes. */
export function countdownLabel(minutes: number): string {
  if (minutes <= 0) return 'now';
  if (minutes < 60) return `${minutes} min`;
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return rest === 0 ? `${hours} hr` : `${hours} hr ${rest} min`;
}

/** The one place that decides whether the card has anything true to show.
 *  `null` means the card renders nothing at all — no placeholder clock. */
export function prayerCardModel(prayer: PrayerSnapshot | null | undefined): PrayerCardModel | null {
  if (!prayer || !prayer.next || !Array.isArray(prayer.times) || prayer.times.length === 0) {
    return null;
  }
  if (
    !prayer.next.time ||
    typeof prayer.in_minutes !== 'number' ||
    !Number.isFinite(prayer.in_minutes)
  ) {
    return null;
  }
  return {
    cityName: prayer.city_name || '',
    method: prayer.method || '',
    correctionMinutes: prayer.correction_minutes,
    hijri: prayer.hijri || '',
    nextName: prayer.next.name || '',
    nextTime: prayer.next.time,
    countdown: countdownLabel(prayer.in_minutes),
    warning: prayer.urgent ? `Within ${PRAYER_WARNING_MINUTES} minutes` : null,
    times: prayer.times.map((row) => ({ key: row.key, name: row.name, time: row.time })),
  };
}
