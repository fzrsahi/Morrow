// #197 — pure decisions for fullscreen, quiet hours and the lock screen.
// The polling loop in App.svelte calls the commands this module says to call;
// the shell owns the actual window hide/show.
import { describe, expect, test } from 'bun:test';
import { ambientView, pollPlan, type AmbientState, type AmbientSupport } from './ambient';

const windows: AmbientSupport = { fullscreenDetection: true, screenLock: true };
const linux: AmbientSupport = { fullscreenDetection: false, screenLock: true };
const awake: AmbientState = { fullscreen: false, locked: false, quiet: false };

describe('ambientView (#197)', () => {
  test('a fullscreen app hides the island where detection is supported', () => {
    expect(ambientView({ ...awake, fullscreen: true }, windows).hidden).toBe(true);
  });

  test('Linux leaves the island visible: there is no fullscreen probe behind the command', () => {
    expect(ambientView({ ...awake, fullscreen: true }, linux).hidden).toBe(false);
  });

  test('quiet hours turns on the moon', () => {
    expect(ambientView({ ...awake, quiet: true }, windows).quiet).toBe(true);
  });

  test('a locked screen suspends sound, animation and probes', () => {
    expect(ambientView({ ...awake, locked: true }, windows).suspended).toBe(true);
  });
});

describe('pollPlan (#197)', () => {
  test('Windows polls fullscreen, quiet hours and the lock while awake', () => {
    expect(pollPlan(ambientView(awake, windows), windows)).toEqual({
      fullscreen: true,
      quietHours: true,
      screenLocked: true,
      active: true,
    });
  });

  test('Linux never probes fullscreen or quiet hours', () => {
    expect(pollPlan(ambientView(awake, linux), linux)).toEqual({
      fullscreen: false,
      quietHours: false,
      screenLocked: true,
      active: true,
    });
  });

  test('a locked screen drops every content probe, keeping only the lock watch', () => {
    const locked = { ...awake, locked: true };
    expect(pollPlan(ambientView(locked, windows), windows)).toEqual({
      fullscreen: false,
      quietHours: false,
      screenLocked: true,
      active: true,
    });
  });

  test('polling stops when no adapter needs a probe', () => {
    const none: AmbientSupport = { fullscreenDetection: false, screenLock: false };
    expect(pollPlan(ambientView(awake, none), none).active).toBe(false);
  });
});
