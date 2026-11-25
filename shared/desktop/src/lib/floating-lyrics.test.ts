import { describe, expect, test } from 'bun:test';
import {
  DEFAULT_FLOATING_LYRICS,
  FLOATING_LYRIC_KEYS,
  floatingLyricsFromSettings,
} from './floating-lyrics';

describe('floating lyrics settings', () => {
  test('a fresh document keeps the Swift defaults', () => {
    expect(floatingLyricsFromSettings({})).toEqual(DEFAULT_FLOATING_LYRICS);
  });

  test('written Swift keys win and bad values fall back', () => {
    const settings = floatingLyricsFromSettings({
      [FLOATING_LYRIC_KEYS.preset]: 'neonHud',
      [FLOATING_LYRIC_KEYS.fontSize]: 999,
      [FLOATING_LYRIC_KEYS.iconStyle]: 'none',
      [FLOATING_LYRIC_KEYS.autoHideOnHover]: false,
      [FLOATING_LYRIC_KEYS.syncOffset]: -5,
    });
    expect(settings.preset).toBe('neonHud');
    expect(settings.fontSize).toBe(32);
    expect(settings.iconStyle).toBe('none');
    expect(settings.autoHideOnHover).toBe(false);
    expect(settings.syncOffset).toBe(-5);
  });

  test('an unknown enum value is not shipped to the renderer', () => {
    expect(floatingLyricsFromSettings({ floatingLyricPreset: 'madeUp' }).preset).toBe('pureText');
  });
});
