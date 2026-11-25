import { describe, expect, test } from 'bun:test';
import {
  DEFAULT_SOUND_CONTROLS,
  minutesToTime,
  soundControlsFromSettings,
  timeToMinutes,
} from './sound-controls';

describe('soundControlsFromSettings', () => {
  test('a fresh document keeps the Swift defaults', () => {
    expect(soundControlsFromSettings({})).toEqual(DEFAULT_SOUND_CONTROLS);
  });

  test('written keys win and bad values are ignored', () => {
    const controls = soundControlsFromSettings({
      soundEnabled: true,
      soundVolume: 140,
      soundTaskComplete: false,
      quietHoursStart: 'bad',
    });
    expect(controls.enabled).toBe(true);
    expect(controls.volume).toBe(100);
    expect(controls.taskComplete).toBe(false);
    expect(controls.quietHoursStart).toBe(22 * 60);
  });
});

describe('quiet hours inputs', () => {
  test('minutes round-trip through a clock label', () => {
    expect(minutesToTime(22 * 60)).toBe('22:00');
    expect(minutesToTime(8 * 60 + 30)).toBe('08:30');
    expect(timeToMinutes('22:00')).toBe(22 * 60);
    expect(timeToMinutes('bad')).toBeNull();
    expect(timeToMinutes('25:00')).toBeNull();
  });
});
