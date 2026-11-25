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

// #462 — the remaining macOS controls that work without OS permissions.
export const PRAYER_CARD_MODES = [
  { value: 'compact', label: 'Compact' },
  { value: 'panoramic', label: 'Panoramic (expanded day list)' },
] as const;

export const PRAYER_LOCATION_MODES = [
  { value: 'manual', label: 'Manual city' },
  { value: 'automatic', label: 'Automatic (unavailable)' },
] as const;

/** #443 — shown beside the disabled Automatic option, not hidden in a tooltip. */
export const AUTOMATIC_LOCATION_REASON =
  'Automatic location needs an OS permission flow (GeoClue on Linux, WinRT on Windows) this shell does not have yet (#443). The offline city picker below is the supported path.';

/** Only manual can be selected. A stored "automatic" never claims to work. */
const AVAILABLE_LOCATION_MODES = [{ value: 'manual' }] as const;

export const PRAYER_WARNING_MINUTES = 15;

export type PrayerCity = { id: string; name: string; province: string };

export type PrayerSettings = {
  cityId: string;
  method: string;
  correctionMinutes: number;
  hijriSource: string;
  enabled: boolean;
  cardMode: (typeof PRAYER_CARD_MODES)[number]['value'];
  hijriDayOffset: number;
  locationMode: (typeof AVAILABLE_LOCATION_MODES)[number]['value'];
  soundEnabled: boolean;
};

export const DEFAULT_PRAYER_SETTINGS: PrayerSettings = {
  cityId: 'jakarta',
  method: 'kemenag',
  correctionMinutes: 0,
  hijriSource: 'khgt',
  // The card has always appeared once a prayer key was saved. Defaulting on
  // keeps that behaviour for documents written before #462.
  enabled: true,
  cardMode: 'compact',
  hijriDayOffset: 0,
  locationMode: 'manual',
  soundEnabled: false,
};

const allowed = <T extends string>(
  value: unknown,
  options: readonly { value: T }[],
  fallback: T,
): T => (options.some((option) => option.value === value) ? (value as T) : fallback);

function object(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

/** Read the prayer keys from the settings document, with the same fallbacks
 *  the shell uses. Values the shell cannot honour fall back instead of being
 *  shown as a working choice. */
export function prayerSettingsFromSettings(value: unknown): PrayerSettings {
  const stored = object(value) ?? {};
  const cityId = stored[SETTINGS_KEYS.prayerCityId];
  const minutes = stored[SETTINGS_KEYS.prayerCorrectionMinutes];
  const offset = stored[SETTINGS_KEYS.prayerHijriOffset];
  const enabled = stored[SETTINGS_KEYS.prayerEnabled];
  const soundEnabled = stored[SETTINGS_KEYS.prayerSoundEnabled];
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
    enabled: typeof enabled === 'boolean' ? enabled : DEFAULT_PRAYER_SETTINGS.enabled,
    cardMode: allowed(
      stored[SETTINGS_KEYS.prayerCardMode],
      PRAYER_CARD_MODES,
      DEFAULT_PRAYER_SETTINGS.cardMode,
    ),
    hijriDayOffset:
      typeof offset === 'number' && Number.isFinite(offset)
        ? Math.max(-2, Math.min(2, Math.trunc(offset)))
        : DEFAULT_PRAYER_SETTINGS.hijriDayOffset,
    locationMode: allowed(
      stored[SETTINGS_KEYS.prayerLocationMode],
      AVAILABLE_LOCATION_MODES,
      DEFAULT_PRAYER_SETTINGS.locationMode,
    ),
    soundEnabled:
      typeof soundEnabled === 'boolean' ? soundEnabled : DEFAULT_PRAYER_SETTINGS.soundEnabled,
  };
}

export type PrayerCardModel = {
  cityName: string;
  method: string;
  correctionMinutes: number;
  cardMode: (typeof PRAYER_CARD_MODES)[number]['value'];
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
    cardMode: prayer.card_mode === 'panoramic' ? 'panoramic' : 'compact',
    hijri: prayer.hijri || '',
    nextName: prayer.next.name || '',
    nextTime: prayer.next.time,
    countdown: countdownLabel(prayer.in_minutes),
    warning: prayer.urgent ? `Within ${PRAYER_WARNING_MINUTES} minutes` : null,
    times: prayer.times.map((row) => ({ key: row.key, name: row.name, time: row.time })),
  };
}
