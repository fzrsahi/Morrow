import { describe, expect, test } from 'bun:test';

import {
  BEHAVIOR_KEYS,
  DEFAULT_BEHAVIOR,
  behaviorFromSettings,
  completionStyleFromSettings,
  csvEntries,
  hoverDelayMs,
  islandHidden,
} from './behavior';

describe('behavior settings', () => {
  test('a fresh install uses the Swift defaults', () => {
    expect(behaviorFromSettings(null)).toEqual(DEFAULT_BEHAVIOR);
    expect(behaviorFromSettings({})).toEqual(DEFAULT_BEHAVIOR);
  });

  test('stored keys use the raw Swift spellings', () => {
    const settings = behaviorFromSettings({
      hideInFullscreen: true,
      smartSuppress: false,
      hoverExpandDelay: 0.8,
      pluginSessionMode: 'merge',
      sessionTimeout: 45,
      rotationInterval: 12,
      maxToolHistory: 50,
      autoApproveTools: 'Read, Edit',
      claude_config_dir: '/tmp/claude',
      extra_config_dirs_v1: '[]',
      webhookEnabled: true,
      webhookURL: 'http://localhost:9000/hook',
      allowHorizontalDrag: true,
      avoidMenuBarIcons: false,
    });
    expect(settings.hideInFullscreen).toBe(true);
    expect(settings.smartSuppress).toBe(false);
    expect(settings.hoverExpandDelay).toBe(0.8);
    expect(settings.pluginSessionMode).toBe('merge');
    expect(settings.sessionTimeout).toBe(45);
    expect(settings.rotationInterval).toBe(12);
    expect(settings.maxToolHistory).toBe(50);
    expect(settings.autoApproveTools).toBe('Read, Edit');
    expect(settings.claudeConfigDir).toBe('/tmp/claude');
    expect(settings.extraConfigDirs).toBe('[]');
    expect(settings.webhookEnabled).toBe(true);
    expect(settings.webhookURL).toBe('http://localhost:9000/hook');
    expect(settings.allowHorizontalDrag).toBe(true);
    expect(settings.avoidMenuBarIcons).toBe(false);
  });

  test('malformed and out-of-range values fall back to the documented default', () => {
    const settings = behaviorFromSettings({
      hideInFullscreen: 'yes',
      hoverExpandDelay: 12,
      pluginSessionMode: 'yolo',
      sessionTimeout: -1,
      rotationInterval: 0,
      maxToolHistory: 10_000,
      webhookURL: 42,
      claudeConfigDir: 7,
    });
    expect(settings.hideInFullscreen).toBe(false);
    expect(settings.hoverExpandDelay).toBe(0.5);
    expect(settings.pluginSessionMode).toBe('separate');
    expect(settings.sessionTimeout).toBe(30);
    expect(settings.rotationInterval).toBe(5);
    expect(settings.maxToolHistory).toBe(20);
    expect(settings.webhookURL).toBe('');
    expect(settings.claudeConfigDir).toBe('');
  });

  test('a written false on the legacy completion boolean migrates to off', () => {
    expect(completionStyleFromSettings({ autoExpandOnCompletion: false })).toBe('off');
    expect(completionStyleFromSettings({})).toBe('expand');
    expect(
      completionStyleFromSettings({
        autoExpandOnCompletion: false,
        completionNotificationStyle: 'glance',
      }),
    ).toBe('glance');
    expect(behaviorFromSettings({ autoExpandOnCompletion: false }).completionNotificationStyle).toBe(
      'off',
    );
  });

  test('comma-separated lists drop padding and empty entries', () => {
    expect(csvEntries(' Read, , Edit ,')).toEqual(['Read', 'Edit']);
    expect(csvEntries('')).toEqual([]);
  });

  test('the dwell is clamped to the same range as the Swift picker', () => {
    expect(hoverDelayMs(0.1)).toBe(100);
    expect(hoverDelayMs(1)).toBe(1000);
    expect(hoverDelayMs(0)).toBe(100);
    expect(hoverDelayMs(9)).toBe(1000);
    expect(hoverDelayMs(Number.NaN)).toBe(500);
  });

  test('the fullscreen switch needs a reader, and no-session hiding keeps an open panel', () => {
    const support = { fullscreenDetection: true, screenLock: false };
    const ambient = { fullscreen: true, locked: false, quiet: false };
    const behavior = { hideInFullscreen: true, hideWhenNoSession: true };
    expect(islandHidden({ ambient, support, sessionCount: 2, expanded: false, behavior })).toBe(true);
    expect(
      islandHidden({
        ambient,
        support: { ...support, fullscreenDetection: false },
        sessionCount: 2,
        expanded: false,
        behavior,
      }),
    ).toBe(false);
    expect(
      islandHidden({
        ambient: { ...ambient, fullscreen: false },
        support,
        sessionCount: 0,
        expanded: false,
        behavior,
      }),
    ).toBe(true);
    expect(
      islandHidden({
        ambient: { ...ambient, fullscreen: false },
        support,
        sessionCount: 0,
        expanded: true,
        behavior,
      }),
    ).toBe(false);
    expect(
      islandHidden({
        ambient,
        support,
        sessionCount: 0,
        expanded: false,
        behavior: { hideInFullscreen: false, hideWhenNoSession: false },
      }),
    ).toBe(false);
  });

  test('every key is a top-level settings document key, not a nested path', () => {
    for (const key of Object.values(BEHAVIOR_KEYS)) {
      expect(key).not.toContain('.');
      expect(key.length).toBeGreaterThan(0);
    }
    expect(BEHAVIOR_KEYS.hoverExpandDelay).toBe('hoverExpandDelay');
    expect(BEHAVIOR_KEYS.claudeConfigDir).toBe('claude_config_dir');
    expect(BEHAVIOR_KEYS.extraConfigDirs).toBe('extra_config_dirs_v1');
  });
});
