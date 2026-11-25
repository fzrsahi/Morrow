import { describe, expect, test } from 'bun:test';
import { DEFAULT_BUDDY, buddyFromSettings, clampBrightness } from './buddy-settings';

describe('buddy settings', () => {
  test('a fresh document keeps the Swift defaults', () => {
    expect(buddyFromSettings({})).toEqual(DEFAULT_BUDDY);
  });

  test('brightness is clamped to the firmware range', () => {
    expect(clampBrightness(0)).toBe(10);
    expect(clampBrightness(200)).toBe(100);
    expect(clampBrightness(Number.NaN)).toBe(70);
    expect(clampBrightness(69.6)).toBe(70);
  });

  test('stored keys win and bad values fall back', () => {
    const settings = buddyFromSettings({
      esp32BridgeEnabled: true,
      esp32HeartbeatSeconds: 99,
      buddyScreenBrightnessPercent: 0,
      buddyScreenOrientation: 'down',
      selectedBuddyName: 'Desk Buddy',
    });
    expect(settings.bridgeEnabled).toBe(true);
    expect(settings.heartbeatSeconds).toBe(5);
    expect(settings.brightness).toBe(10);
    expect(settings.orientation).toBe('down');
    expect(settings.selectedName).toBe('Desk Buddy');
  });
});
