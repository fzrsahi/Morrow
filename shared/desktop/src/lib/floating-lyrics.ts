// #465 — the Floating Synced Lyrics controls from
// `SettingsView.swift:2738-2943`. The raw values are Swift enum raw values, so
// a settings document written by macOS opens unchanged on this shell.

export const FLOATING_LYRIC_KEYS = {
  preset: 'floatingLyricPreset',
  fontSize: 'floatingLyricFontSize',
  fontDesign: 'floatingLyricFontDesign',
  iconStyle: 'floatingLyricIconStyle',
  textColor: 'floatingLyricTextColor',
  verticalGap: 'floatingLyricVerticalGap',
  autoHideOnHover: 'floatingLyricAutoHideOnHover',
  syncOffset: 'lyricSyncOffset',
} as const;

export const LYRIC_PRESETS = [
  'pureText',
  'capsule',
  'frostedGlass',
  'neonHud',
  'seamlessGlass',
  'retroPixel',
  'liquidAurora',
  'minimalBraun',
  'tapeReel',
  'vinylGroove',
  'fluidKaraoke',
  'connectedChin',
  'bilateralWings',
] as const;
export const LYRIC_FONT_DESIGNS = ['rounded', 'monospaced', 'system', 'serif', 'pixel'] as const;
export const LYRIC_ICON_STYLES = [
  'spinningVinyl',
  'albumTurntable',
  'dancingWave',
  'pulsingNote',
  'retroCassette',
  'studioMic',
  'hapticBubble',
  'none',
] as const;
export const LYRIC_TEXT_COLORS = ['album', 'white', 'cyan', 'pink', 'green', 'amber'] as const;

export type FloatingLyrics = {
  preset: string;
  fontSize: number;
  fontDesign: string;
  iconStyle: string;
  textColor: string;
  verticalGap: number;
  autoHideOnHover: boolean;
  syncOffset: number;
};

export const DEFAULT_FLOATING_LYRICS: FloatingLyrics = {
  preset: 'pureText',
  fontSize: 10,
  fontDesign: 'rounded',
  iconStyle: 'spinningVinyl',
  textColor: 'white',
  verticalGap: 2.5,
  autoHideOnHover: true,
  syncOffset: 0,
};

function choose<T extends string>(value: unknown, allowed: readonly T[], fallback: T): T {
  return allowed.includes(value as T) ? (value as T) : fallback;
}

function finite(value: unknown, fallback: number, min: number, max: number): number {
  return typeof value === 'number' && Number.isFinite(value)
    ? Math.max(min, Math.min(max, value))
    : fallback;
}

export function floatingLyricsFromSettings(value: unknown): FloatingLyrics {
  const stored = value !== null && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
  const autoHide = stored[FLOATING_LYRIC_KEYS.autoHideOnHover];
  return {
    preset: choose(stored[FLOATING_LYRIC_KEYS.preset], LYRIC_PRESETS, DEFAULT_FLOATING_LYRICS.preset),
    fontSize: finite(stored[FLOATING_LYRIC_KEYS.fontSize], 10, 8, 32),
    fontDesign: choose(
      stored[FLOATING_LYRIC_KEYS.fontDesign],
      LYRIC_FONT_DESIGNS,
      DEFAULT_FLOATING_LYRICS.fontDesign,
    ),
    iconStyle: choose(
      stored[FLOATING_LYRIC_KEYS.iconStyle],
      LYRIC_ICON_STYLES,
      DEFAULT_FLOATING_LYRICS.iconStyle,
    ),
    textColor: choose(
      stored[FLOATING_LYRIC_KEYS.textColor],
      LYRIC_TEXT_COLORS,
      DEFAULT_FLOATING_LYRICS.textColor,
    ),
    verticalGap: finite(stored[FLOATING_LYRIC_KEYS.verticalGap], 2.5, 0, 20),
    autoHideOnHover:
      typeof autoHide === 'boolean' ? autoHide : DEFAULT_FLOATING_LYRICS.autoHideOnHover,
    syncOffset: finite(stored[FLOATING_LYRIC_KEYS.syncOffset], 0, -30, 30),
  };
}
