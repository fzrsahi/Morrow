// #199 — the Settings window's pure logic. The sidebar follows the port-only
// pages of the #settings canvas; the settings themselves live in the existing
// ~/.codeisland/settings.json, read and written by the shell's settings_* commands.

import type { DisplayMode, SessionDensity, SessionFilter } from './panel';
import type { CapabilityRow } from './types';
import { STRINGS, translate, BASE_LANGUAGE } from './strings';

/** #202 — the shell emits the settings document to both webviews after a write. */
export const SETTINGS_CHANGED_EVENT = 'settings-changed';

export type SettingsPageId =
  | 'general'
  | 'appearance'
  | 'mascots'
  | 'sound'
  | 'shortcuts'
  | 'agents'
  | 'workspace'
  | 'music'
  | 'floating-lyrics'
  | 'prayer'
  | 'behavior'
  | 'buddy'
  | 'hooks'
  | 'remote'
  | 'capabilities'
  | 'about';

export type SettingsNavItem = {
  id: SettingsPageId;
  label: string;
  icon: string;
  available: boolean;
  /** #239 AC 3 — why a page has no controls, in the user's words. Required for
   *  every `available: false` row and shown *on* the page: a sidebar entry that
   *  is greyed out says "you cannot go there", not "there is nothing there
   *  yet", and the difference is the whole point of the criterion. */
  reason?: string;
};

/** #239 AC 4 — the label a Settings row draws.
 *
 *  Ten of the eleven pages have a key in the shared resource (the macOS table
 *  calls them the same thing); `capabilities` is port-only — macOS shows that
 *  table nowhere — so it keeps its literal. Measured: `capabilities` is the only
 *  id missing from `en.json`, and a test pins that it stays the only one.
 */
export function pageLabel(item: SettingsNavItem, language: string = BASE_LANGUAGE): string {
  if (item.id in (STRINGS[language] ?? STRINGS[BASE_LANGUAGE])) return translate(item.id, language as never);
  return item.label;
}

/** The default global hotkey, as `hotkey.rs` registers it on a fresh install.
 *  One place, because the Shortcuts page must not be the second copy. */
export const DEFAULT_HOTKEY = 'Super+Space';

/** One row of `shortcut_list`: an action the shell can actually grab. */
export type ShortcutBinding = {
  action: string;
  label: string;
  /** The chord on a fresh install; `null` for an action that starts unbound. */
  default: string | null;
  /** The effective chord, or `null` when the user cleared it. */
  chord: string | null;
};

const MODIFIERS: Record<string, string> = {
  super: 'Super',
  meta: 'Super',
  cmd: 'Super',
  command: 'Super',
  ctrl: 'Ctrl',
  control: 'Ctrl',
  alt: 'Alt',
  option: 'Alt',
  shift: 'Shift',
};
const MODIFIER_ORDER = ['Super', 'Ctrl', 'Alt', 'Shift'];

/** KeyboardEvent.key names that need their case preserved (Space is not SPACE). */
const NAMED_KEYS: Record<string, string> = {
  space: 'Space',
  backspace: 'Backspace',
  delete: 'Delete',
  enter: 'Enter',
  home: 'Home',
  end: 'End',
  pageup: 'PageUp',
  pagedown: 'PageDown',
  arrowup: 'ArrowUp',
  arrowdown: 'ArrowDown',
  arrowleft: 'ArrowLeft',
  arrowright: 'ArrowRight',
};

function canonicalKey(raw: string): string {
  return NAMED_KEYS[raw.toLowerCase()] ?? raw.toUpperCase();
}

/** One chord written two ways (`ctrl+a`, `Ctrl+A`) is one binding. Needed for
 *  conflicts: a hand-edited settings file must not slip past the badge. */
export function canonicalChord(value: string): string | null {
  const modifiers = new Set<string>();
  let key: string | null = null;
  for (const token of value.split('+').map((part) => part.trim()).filter(Boolean)) {
    const modifier = MODIFIERS[token.toLowerCase()];
    if (modifier) {
      modifiers.add(modifier);
      continue;
    }
    if (key !== null) return null;
    key = canonicalKey(token);
  }
  if (key === null) return null;
  return [...MODIFIER_ORDER.filter((modifier) => modifiers.has(modifier)), key].join('+');
}

/** The parts of a key event a chord recorder needs. */
export type ChordKeys = Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey'>;

/** Record a chord from a keydown. Bare keys, modifier-only presses and Escape
 *  are rejected: a global grab must never swallow an ordinary key. */
export function chordFromEvent(event: ChordKeys): string | null {
  const modifiers = [
    event.metaKey && 'Super',
    event.ctrlKey && 'Ctrl',
    event.altKey && 'Alt',
    event.shiftKey && 'Shift',
  ].filter((modifier): modifier is string => typeof modifier === 'string');
  if (modifiers.length === 0) return null;
  const key =
    event.key === ' '
      ? 'Space'
      : /^[a-z0-9]$/i.test(event.key) || /^F([1-9]|1[0-2])$/i.test(event.key)
        ? canonicalKey(event.key)
        : (NAMED_KEYS[event.key.toLowerCase()] ?? null);
  return key === null ? null : [...modifiers, key].join('+');
}

/** The other action bound to the same chord, for the conflict badge. */
export function conflictingBinding(
  row: ShortcutBinding,
  rows: ShortcutBinding[],
): ShortcutBinding | null {
  const chord = row.chord === null ? null : canonicalChord(row.chord);
  if (chord === null) return null;
  return (
    rows.find(
      (other) =>
        other.action !== row.action &&
        other.chord !== null &&
        canonicalChord(other.chord) === chord,
    ) ?? null
  );
}

export const SETTINGS_NAV: { group: string; items: SettingsNavItem[] }[] = [
  {
    group: 'System',
    items: [
      // #462: General is back. The language tag it writes is read by
      // `resolveLanguage` in the Settings shell, so the window really changes
      // language. The profile and onboarding reset are still macOS only.
      { id: 'general', label: 'General', icon: 'settings', available: true },
      { id: 'appearance', label: 'Appearance', icon: 'settings', available: true },
      // #465: the 33-entry gallery, preview status, speed and default source
      // are settings only — they need no paired-body reader.
      { id: 'mascots', label: 'Mascots', icon: 'agents', available: true },
      { id: 'sound', label: 'Sound', icon: 'volume', available: true },
      { id: 'shortcuts', label: 'Shortcuts', icon: 'workspace', available: true },
    ],
  },
  {
    group: 'Features',
    items: [
      // #465: the Agents page writes the same Swift keys the panel reads for
      // theme, font, density, row limit and card details.
      { id: 'agents', label: 'Agents', icon: 'agents', available: true },
      { id: 'workspace', label: 'Workspace', icon: 'workspace', available: true },
      {
        id: 'music',
        label: 'Music',
        icon: 'music',
        available: false,
        reason:
          'The player is fixed per platform (MPRIS on Linux, SMTC on Windows) and the deck already follows it. There is no provider choice to offer, and the level is set on the island meter itself (#372) rather than here.',
      },
      { id: 'prayer', label: 'Prayer times', icon: 'workspace', available: true },
      { id: 'floating-lyrics', label: 'Floating lyrics', icon: 'music', available: true },
    ],
  },
  {
    group: 'Morrow',
    items: [
      { id: 'behavior', label: 'Behavior', icon: 'workspace', available: true },
      { id: 'buddy', label: 'Buddy / ESP32', icon: 'agents', available: true },
      { id: 'hooks', label: 'Hooks', icon: 'checkmark', available: true },
      { id: 'remote', label: 'Remote', icon: 'workspace', available: true },
      { id: 'capabilities', label: 'Capabilities', icon: 'workspace', available: true },
      { id: 'about', label: 'About', icon: 'settings', available: true },
    ],
  },
];

export const SETTINGS_KEYS = {
  theme: 'theme',
  fontSize: 'fontSize',
  accent: 'accent',
  animationSpeed: 'animationSpeed',
  islandShape: 'islandShape',
  /** Same key and raw values as Swift `SettingsKey.islandDisplayMode`. */
  displayMode: 'islandDisplayMode',
  position: 'position',
  expandOnHover: 'expandOnHover',
  collapseOnLeave: 'collapseOnLeave',
  /** #185: the filter choice and row density live in the same document. */
  /** #462: same key as Swift `SettingsKey.appLanguage`; `system` resolves to one. */
  language: 'appLanguage',
  sessionFilter: 'sessionFilter',
  sessionDensity: 'sessionDensity',
  /** Parity #3 — the same keys as Swift `SettingsKey.notchScroll*`. */
  scrollVolume: 'notchScrollVolumeControl',
  swipeTabs: 'notchScrollTabSwitch',
  /** #307 — the four keys owned by the Prayer settings page. */
  prayerCityId: 'prayerManualCityId',
  prayerMethod: 'prayerCalculationMethod',
  prayerCorrectionMinutes: 'prayerCorrectionMinutes',
  prayerHijriSource: 'prayerHijriCalendarSource',
  /** #462 — the same names as Swift `SettingsKey`s, minus precise coordinates
   *  and last-known location: those only exist for automatic location (#443). */
  prayerEnabled: 'prayerEnabled',
  prayerCardMode: 'prayerCardDisplayMode',
  prayerHijriOffset: 'prayerHijriDayOffset',
  prayerLocationMode: 'prayerLocationMode',
  prayerSoundEnabled: 'prayerSoundEnabled',
} as const;

export type AppearanceSettings = {
  theme: 'pixel' | 'modern';
  fontSize: 11 | 12 | 14 | 16;
  accent: string;
  animationSpeed: number;
  /** Legacy #199 key, kept readable for existing settings documents. */
  islandShape: 'edge-tab' | 'floating-pill';
  displayMode: DisplayMode;
  position: string;
  expandOnHover: boolean;
  collapseOnLeave: boolean;
  /** #239 AC 4: a stored tag, or `system` to follow the desktop. #462: written by General. */
  language: string;
};

export const DEFAULT_APPEARANCE: AppearanceSettings = {
  theme: 'pixel',
  fontSize: 11,
  accent: 'cyan',
  animationSpeed: 1,
  islandShape: 'floating-pill',
  // No hardware notch on Windows/Linux: match Swift's non-notch island.
  displayMode: 'dynamicIsland',
  position: 'Top centre · primary monitor',
  expandOnHover: false,
  collapseOnLeave: false,
  language: 'system',
};

const FONT_SIZES = [11, 12, 14, 16] as const;

function object(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function text(value: unknown, fallback: string): string {
  return typeof value === 'string' && value.length > 0 ? value : fallback;
}

function flag(value: unknown, fallback: boolean): boolean {
  return typeof value === 'boolean' ? value : fallback;
}

function chosen<T extends string>(value: unknown, allowed: readonly T[], fallback: T): T {
  return allowed.includes(value as T) ? (value as T) : fallback;
}

/// The Swift display mode, migrated from the legacy #199 islandShape key when
/// the user has not written an islandDisplayMode yet.
export function displayModeFromSettings(stored: Record<string, unknown>): DisplayMode {
  const raw = stored[SETTINGS_KEYS.displayMode];
  if (raw === 'notch' || raw === 'dynamicIsland') return raw;
  const legacy = stored[SETTINGS_KEYS.islandShape];
  if (legacy === 'floating-pill') return 'dynamicIsland';
  if (legacy === 'edge-tab') return 'notch';
  return DEFAULT_APPEARANCE.displayMode;
}

/// Read the appearance values from the user's settings document. Unknown or
/// malformed values fall back to defaults instead of breaking the window.
export function appearanceFromSettings(value: unknown): AppearanceSettings {
  const stored = object(value) ?? {};
  const theme = stored[SETTINGS_KEYS.theme];
  const fontSize = stored[SETTINGS_KEYS.fontSize];
  const speed = stored[SETTINGS_KEYS.animationSpeed];
  const shape = stored[SETTINGS_KEYS.islandShape];
  return {
    theme: theme === 'pixel' || theme === 'modern' ? theme : DEFAULT_APPEARANCE.theme,
    fontSize: FONT_SIZES.includes(fontSize as (typeof FONT_SIZES)[number])
      ? (fontSize as AppearanceSettings['fontSize'])
      : DEFAULT_APPEARANCE.fontSize,
    accent: text(stored[SETTINGS_KEYS.accent], DEFAULT_APPEARANCE.accent),
    animationSpeed:
      typeof speed === 'number' && Number.isFinite(speed) && speed > 0
        ? speed
        : DEFAULT_APPEARANCE.animationSpeed,
    islandShape:
      shape === 'edge-tab' || shape === 'floating-pill'
        ? shape
        : DEFAULT_APPEARANCE.islandShape,
    displayMode: displayModeFromSettings(stored),
    position: text(stored[SETTINGS_KEYS.position], DEFAULT_APPEARANCE.position),
    language: text(stored[SETTINGS_KEYS.language], DEFAULT_APPEARANCE.language),
    expandOnHover: flag(stored[SETTINGS_KEYS.expandOnHover], DEFAULT_APPEARANCE.expandOnHover),
    collapseOnLeave: flag(
      stored[SETTINGS_KEYS.collapseOnLeave],
      DEFAULT_APPEARANCE.collapseOnLeave,
    ),
  };
}

const FILTERS = ['all', 'sta', 'agt'] as const;
const DENSITIES = ['compact', 'comfortable'] as const;

/// #185: the ALL/STA/AGT choice survives a restart; a bad value is ignored
/// rather than leaving the header with no selected filter.
export function sessionFilterFromSettings(value: unknown): SessionFilter {
  return chosen((object(value) ?? {})[SETTINGS_KEYS.sessionFilter], FILTERS, 'all');
}

/// #185: comfortable keeps #183's card rendering reachable for the tickets
/// that build on it; a fresh install uses the 34 pt compact rows.
export function sessionDensityFromSettings(value: unknown): SessionDensity {
  return chosen((object(value) ?? {})[SETTINGS_KEYS.sessionDensity], DENSITIES, 'compact');
}

/** Parity #3 — the two island gestures, both on by default like Swift's
 *  `SettingsDefaults.notchScroll*`. A written `false` is the off switch. */
export type GestureSettings = { scrollVolume: boolean; swipeTabs: boolean };

export const DEFAULT_GESTURES: GestureSettings = { scrollVolume: true, swipeTabs: true };

export function gesturesFromSettings(value: unknown): GestureSettings {
  const stored = object(value) ?? {};
  return {
    scrollVolume: flag(stored[SETTINGS_KEYS.scrollVolume], DEFAULT_GESTURES.scrollVolume),
    swipeTabs: flag(stored[SETTINGS_KEYS.swipeTabs], DEFAULT_GESTURES.swipeTabs),
  };
}

export type DescribedCapability = {
  label: 'Supported' | 'Limited' | 'Not supported';
  reason: string | null;
};

const NO_REASON = 'No reason reported by this platform.';

/// Rust sends `Capability` as an externally tagged enum: one key, optionally a
/// reason. A limitation without a usable reason still has to say something —
/// the table never renders the literal "null".
export function describeCapability(row: CapabilityRow): DescribedCapability {
  const [kind, raw] = Object.entries(row.capability)[0] ?? ['Supported', null];
  if (kind === 'Supported') return { label: 'Supported', reason: null };
  return {
    label: kind === 'Degraded' ? 'Limited' : 'Not supported',
    reason: typeof raw === 'string' && raw.trim() ? raw : NO_REASON,
  };
}
