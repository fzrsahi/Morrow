import { describe, expect, test } from 'bun:test';

import {
  appearanceFromSettings,
  describeCapability,
  sessionDensityFromSettings,
  sessionFilterFromSettings,
  SETTINGS_KEYS,
  SETTINGS_NAV,
} from './settings';
import type { CapabilityRow } from './types';

describe('capability table', () => {
  test('every Limited or Not supported row explains itself', () => {
    const rows: CapabilityRow[] = [
      {
        feature: 'Panel anchoring',
        capability: { Degraded: 'GNOME does not let apps pin a panel above other windows' },
      },
      {
        feature: 'Fullscreen detection',
        capability: { Unsupported: 'no portable signal on Linux' },
      },
      { feature: 'Now Playing', capability: { Supported: null } },
    ];

    const shown = rows.map(describeCapability);
    expect(shown.map((row) => row.label)).toEqual(['Limited', 'Not supported', 'Supported']);
    expect(shown[0].reason).toBe('GNOME does not let apps pin a panel above other windows');
    expect(shown[1].reason).toBe('no portable signal on Linux');
    expect(shown[2].reason).toBeNull();
    for (const row of shown.filter((row) => row.label !== 'Supported')) {
      expect(row.reason?.trim()).toBeTruthy();
    }
  });

  test('a missing reason still produces an explanation, never "null"', () => {
    const shown = describeCapability({ feature: 'Tray icon', capability: { Degraded: '' } });
    expect(shown.label).toBe('Limited');
    expect(shown.reason).toBeTruthy();
    expect(shown.reason).not.toBe('null');
  });
});

describe('appearance settings', () => {
  test('a fresh install uses the documented defaults', () => {
    expect(appearanceFromSettings(null)).toEqual({
      theme: 'pixel',
      fontSize: 11,
      accent: 'cyan',
      animationSpeed: 1,
      islandShape: 'floating-pill',
      displayMode: 'dynamicIsland',
      position: 'Top centre · primary monitor',
      expandOnHover: false,
      collapseOnLeave: false,
    });
  });

  test('stored values are read from the existing settings document', () => {
    const appearance = appearanceFromSettings({
      theme: 'modern',
      fontSize: 14,
      islandShape: 'floating-pill',
      islandDisplayMode: 'notch',
      expandOnHover: true,
      sound: '8bit',
    });
    expect(appearance.theme).toBe('modern');
    expect(appearance.fontSize).toBe(14);
    expect(appearance.islandShape).toBe('floating-pill');
    expect(appearance.displayMode).toBe('notch');
    expect(appearance.expandOnHover).toBe(true);
    expect(appearance.accent).toBe('cyan');
  });

  test('unknown or malformed stored values fall back to defaults', () => {
    const appearance = appearanceFromSettings({
      theme: 'neon',
      fontSize: 999,
      accent: 12,
      animationSpeed: 'slow',
      islandShape: 'square',
      expandOnHover: 'yes',
    });
    expect(appearance.theme).toBe('pixel');
    expect(appearance.fontSize).toBe(11);
    expect(appearance.accent).toBe('cyan');
    expect(appearance.animationSpeed).toBe(1);
    expect(appearance.islandShape).toBe('floating-pill');
    expect(appearance.displayMode).toBe('dynamicIsland');
    expect(appearance.expandOnHover).toBe(false);
  });

  test('the keys are top-level keys of ~/.codeisland/settings.json', () => {
    expect(SETTINGS_KEYS.theme).toBe('theme');
    expect(SETTINGS_KEYS.fontSize).toBe('fontSize');
    expect(SETTINGS_KEYS.islandShape).toBe('islandShape');
    // Mirrors Swift SettingsKey.islandDisplayMode / IslandDisplayMode.rawValue.
    expect(SETTINGS_KEYS.displayMode).toBe('islandDisplayMode');
  });

  test('a pre-existing islandShape setting migrates to the Swift mode names', () => {
    expect(appearanceFromSettings({ islandShape: 'floating-pill' }).displayMode).toBe(
      'dynamicIsland',
    );
    expect(appearanceFromSettings({ islandShape: 'edge-tab' }).displayMode).toBe('notch');
  });

  test('the session filter is a top-level key written by the panel', () => {
    expect(SETTINGS_KEYS.sessionFilter).toBe('sessionFilter');
    expect(SETTINGS_KEYS.sessionDensity).toBe('sessionDensity');
  });

  test('a fresh install groups nothing and uses compact rows', () => {
    expect(sessionFilterFromSettings(null)).toBe('all');
    expect(sessionDensityFromSettings(null)).toBe('compact');
  });

  test('the stored filter and density survive a reload', () => {
    expect(sessionFilterFromSettings({ sessionFilter: 'sta', sessionDensity: 'comfortable' })).toBe(
      'sta',
    );
    expect(sessionDensityFromSettings({ sessionFilter: 'sta', sessionDensity: 'comfortable' })).toBe(
      'comfortable',
    );
    expect(sessionFilterFromSettings({ sessionFilter: 'agt' })).toBe('agt');
  });

  test('malformed stored values fall back instead of breaking the panel', () => {
    expect(sessionFilterFromSettings({ sessionFilter: 'unknown' })).toBe('all');
    expect(sessionDensityFromSettings({ sessionDensity: 42 })).toBe('compact');
  });

  test('the port-only pages are reachable from the sidebar', () => {
    expect(SETTINGS_NAV.flatMap((group) => group.items.map((item) => item.id))).toEqual([
      'general',
      'appearance',
      'mascots',
      'sound',
      'shortcuts',
      'agents',
      'music',
      'prayer',
      'behavior',
      'hooks',
      'capabilities',
      'about',
    ]);
    for (const id of ['hooks', 'capabilities', 'appearance', 'prayer', 'about']) {
      expect(SETTINGS_NAV.flatMap((group) => group.items).find((item) => item.id === id)?.available).toBe(
        true,
      );
    }
  });
});
