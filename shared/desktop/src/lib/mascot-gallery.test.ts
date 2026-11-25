import { describe, expect, test } from 'bun:test';
import {
  DEFAULT_MASCOT_SOURCE,
  MASCOT_GALLERY,
  clampMascotSpeed,
  gallerySprite,
  mascotFromSettings,
  mascotSpeedLabel,
} from './mascot-gallery';

describe('mascot gallery', () => {
  test('copies all 33 entries with unique sources', () => {
    expect(MASCOT_GALLERY).toHaveLength(33);
    expect(new Set(MASCOT_GALLERY.map((entry) => entry.source)).size).toBe(33);
    expect(MASCOT_GALLERY[0]).toMatchObject({ name: 'Clawd', source: 'claude' });
    expect(MASCOT_GALLERY[32]).toMatchObject({ name: 'Ghost', source: 'ghost' });
  });

  test('an unported body falls back to the Clawd sprite', () => {
    expect(gallerySprite(MASCOT_GALLERY[0])).toBe('claude');
    expect(gallerySprite(MASCOT_GALLERY.find((e) => e.source === 'ghost')!)).toBe('claude');
    expect(gallerySprite(MASCOT_GALLERY.find((e) => e.source === 'droid')!)).toBe('factory');
  });
});

describe('mascot settings', () => {
  test('speed is clamped to the slider steps', () => {
    expect(clampMascotSpeed(-10)).toBe(0);
    expect(clampMascotSpeed(301)).toBe(300);
    expect(clampMascotSpeed(37)).toBe(25);
    expect(mascotSpeedLabel(0)).toBe('Off');
    expect(mascotSpeedLabel(150)).toBe('1.5×');
  });

  test('a stored source wins and a bad one uses the default', () => {
    expect(mascotFromSettings({ mascotspeed: 5, mascotSpeed: 50, defaultSource: 'kimi' })).toEqual({
      speed: 50,
      source: 'kimi',
    });
    expect(mascotFromSettings({ defaultSource: 12 })).toEqual({
      speed: 100,
      source: DEFAULT_MASCOT_SOURCE,
    });
  });
});
