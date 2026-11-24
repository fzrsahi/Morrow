// #199 — the Settings window's pure logic. The sidebar follows the port-only
// pages of the #settings canvas; the settings themselves live in the existing
// ~/.codeisland/settings.json, read and written by the shell's settings_* commands.

import type { DisplayMode, SessionDensity, SessionFilter } from './panel';
import type { CapabilityRow } from './types';

/** #202 — the shell emits the settings document to both webviews after a write. */
export const SETTINGS_CHANGED_EVENT = 'settings-changed';

export type SettingsPageId =
  | 'general'
  | 'appearance'
  | 'mascots'
  | 'sound'
  | 'shortcuts'
  | 'agents'
  | 'music'
  | 'behavior'
  | 'hooks'
  | 'capabilities'
  | 'about';

export type SettingsNavItem = {
  id: SettingsPageId;
  label: string;
  icon: string;
  available: boolean;
};

export const SETTINGS_NAV: { group: string; items: SettingsNavItem[] }[] = [
  {
    group: 'System',
    items: [
      { id: 'general', label: 'General', icon: 'settings', available: false },
      { id: 'appearance', label: 'Appearance', icon: 'settings', available: true },
      { id: 'mascots', label: 'Mascots', icon: 'agents', available: false },
      { id: 'sound', label: 'Sound', icon: 'volume', available: false },
      { id: 'shortcuts', label: 'Shortcuts', icon: 'workspace', available: false },
    ],
  },
  {
    group: 'Features',
    items: [
      { id: 'agents', label: 'Agents', icon: 'agents', available: false },
      { id: 'music', label: 'Music', icon: 'music', available: false },
    ],
  },
  {
    group: 'Morrow',
    items: [
      { id: 'behavior', label: 'Behavior', icon: 'workspace', available: true },
      { id: 'hooks', label: 'Hooks', icon: 'checkmark', available: true },
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
  sessionFilter: 'sessionFilter',
  sessionDensity: 'sessionDensity',
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
