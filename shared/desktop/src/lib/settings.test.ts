import { describe, expect, test } from 'bun:test';

import {
  DEFAULT_GESTURES,
  DEFAULT_HOTKEY,
  appearanceFromSettings,
  canonicalChord,
  chordFromEvent,
  conflictingBinding,
  describeCapability,
  gesturesFromSettings,
  sessionDensityFromSettings,
  sessionFilterFromSettings,
  SETTINGS_KEYS,
  SETTINGS_NAV,
} from './settings';
import type { ShortcutBinding } from './settings';
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
      language: 'system',
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

  test('#462: the language key matches Swift and survives a reload', () => {
    expect(SETTINGS_KEYS.language).toBe('appLanguage');
    expect(appearanceFromSettings(null).language).toBe('system');
    expect(appearanceFromSettings({ appLanguage: 'de' }).language).toBe('de');
    expect(appearanceFromSettings({ appLanguage: 42 }).language).toBe('system');
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
      'remote',
      'capabilities',
      'about',
    ]);
    for (const id of [
      'hooks',
      'remote',
      'capabilities',
      'general',
      'appearance',
      'prayer',
      'about',
      'agents',
    ]) {
      expect(SETTINGS_NAV.flatMap((group) => group.items).find((item) => item.id === id)?.available).toBe(
        true,
      );
    }
  });
});

// #239 AC 3 — a Settings page with no controls is a statement about the port, so
// it has to carry its argument. These tests are the difference between "not
// built" and "hidden": a greyed-out row with no reason reads as a permission
// problem, and the next person cannot tell whether it is a bug.
describe('the pages that have no controls say why', () => {
  const items = SETTINGS_NAV.flatMap((group) => group.items);

  test('every unavailable page carries a reason and every available one does not', () => {
    for (const item of items) {
      if (item.available) {
        expect(item.reason, `${item.id} is built but still argues`).toBeUndefined();
      } else {
        expect(item.reason, `${item.id} has no controls and no reason`).toBeTruthy();
      }
    }
  });

  test('a reason names something specific, not "not implemented"', () => {
    // A placeholder would satisfy the type and defeat the criterion.
    for (const item of items.filter((entry) => !entry.available)) {
      const reason = item.reason ?? '';
      expect(reason.length, `${item.id}: ${reason}`).toBeGreaterThan(60);
      expect(reason.toLowerCase()).not.toMatch(/^(not implemented|todo|coming soon|unsupported)\.?$/);
    }
  });

  test('two pages are still empty and eleven are built', () => {
    // Measured, not assumed: #239 counted seven empty pages; Sound (#241),
    // Shortcuts (#239), Prayer/Behavior (#307/#376), Remote (#448), General's
    // language picker (#462) and Agents (#465) are built, so two remain.
    expect(items.filter((entry) => !entry.available).map((entry) => entry.id)).toEqual([
      'mascots',
      'music',
    ]);
    expect(items.filter((entry) => entry.available).length).toBe(11);
  });

  test('the shortcut the page prints is the one the shell registers', () => {
    // Two copies of a binding drift; `hotkey.rs` names this string once.
    expect(DEFAULT_HOTKEY).toBe('Super+Space');
  });
});

// Parity #3 — both gestures are switches, not fixed behaviour.
describe('gesture settings', () => {
  test('a fresh install leaves both gestures on, as on macOS', () => {
    expect(gesturesFromSettings(null)).toEqual(DEFAULT_GESTURES);
    expect(gesturesFromSettings(null)).toEqual({ scrollVolume: true, swipeTabs: true });
  });

  test('each gesture can be turned off independently', () => {
    expect(gesturesFromSettings({ notchScrollVolumeControl: false })).toEqual({
      scrollVolume: false,
      swipeTabs: true,
    });
    expect(gesturesFromSettings({ notchScrollTabSwitch: false })).toEqual({
      scrollVolume: true,
      swipeTabs: false,
    });
  });

  test('malformed values fall back to the default', () => {
    expect(gesturesFromSettings({ notchScrollVolumeControl: 'off', notchScrollTabSwitch: 0 })).toEqual(
      DEFAULT_GESTURES,
    );
  });

  test('the keys match Swift SettingsKey so the two apps can share a value', () => {
    expect(SETTINGS_KEYS.scrollVolume).toBe('notchScrollVolumeControl');
    expect(SETTINGS_KEYS.swipeTabs).toBe('notchScrollTabSwitch');
  });
});

describe('editable shortcut bindings', () => {
  const rows: ShortcutBinding[] = [
    { action: 'toggleIsland', label: 'Show / hide the island', default: DEFAULT_HOTKEY, chord: DEFAULT_HOTKEY },
    { action: 'openSettings', label: 'Open Settings', default: null, chord: 'Ctrl+K' },
  ];

  test('a chord written two ways is still one binding', () => {
    expect(canonicalChord('Super+Space')).toBe('Super+Space');
    expect(canonicalChord('ctrl + a')).toBe('Ctrl+A');
    expect(canonicalChord('SHIFT+CTRL+K')).toBe('Ctrl+Shift+K');
    expect(canonicalChord('Ctrl')).toBeNull();
    expect(canonicalChord('Ctrl+')).toBeNull();
    expect(canonicalChord('Ctrl+A+B')).toBeNull();
  });

  test('recording takes a chord, never a bare key or Escape', () => {
    expect(
      chordFromEvent({ key: ' ', ctrlKey: false, altKey: false, shiftKey: false, metaKey: true }),
    ).toBe('Super+Space');
    expect(
      chordFromEvent({ key: 'k', ctrlKey: true, altKey: false, shiftKey: false, metaKey: false }),
    ).toBe('Ctrl+K');
    expect(
      chordFromEvent({ key: 'K', ctrlKey: false, altKey: false, shiftKey: true, metaKey: true }),
    ).toBe('Super+Shift+K');
    expect(
      chordFromEvent({ key: 'k', ctrlKey: false, altKey: false, shiftKey: false, metaKey: false }),
    ).toBeNull();
    expect(
      chordFromEvent({ key: 'Control', ctrlKey: true, altKey: false, shiftKey: false, metaKey: false }),
    ).toBeNull();
    expect(
      chordFromEvent({ key: 'Escape', ctrlKey: true, altKey: false, shiftKey: false, metaKey: false }),
    ).toBeNull();
  });

  test('the same chord on two actions badges both rows', () => {
    const clash = [{ ...rows[1], chord: 'super+space' }, rows[0]];
    expect(conflictingBinding(rows[0], rows)).toBeNull();
    expect(conflictingBinding(clash[0], clash)?.action).toBe('toggleIsland');
    expect(conflictingBinding(clash[1], clash)?.action).toBe('openSettings');
  });

  test('a cleared binding never conflicts', () => {
    const cleared: ShortcutBinding[] = [{ ...rows[0], chord: null }, { ...rows[1], chord: null }];
    expect(conflictingBinding(cleared[0], cleared)).toBeNull();
    expect(conflictingBinding(cleared[1], cleared)).toBeNull();
  });
});
