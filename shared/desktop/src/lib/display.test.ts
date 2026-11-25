import { describe, expect, test } from 'bun:test';
import {
  clampTopOffset,
  displayMetadata,
  displaySummary,
  displayTitle,
  TOP_OFFSET_DEFAULT,
  type DisplayDescriptor,
  type DisplaySnapshot,
} from './display';

const t = (key: string) => ({ display_numbered: 'Display %d', builtin_display: 'Built-in Display', notch: '(Notch)', island_enabled: 'Show Morrow Island' })[key] ?? key;

function display(overrides: Partial<DisplayDescriptor> = {}): DisplayDescriptor {
  return {
    id: 'eDP-1',
    index: 0,
    name: 'eDP-1',
    width: 1920,
    height: 1080,
    hasNotch: false,
    isBuiltin: true,
    isPrimary: true,
    ...overrides,
  };
}

function snapshot(overrides: Partial<DisplaySnapshot> = {}): DisplaySnapshot {
  return {
    displays: [display()],
    configs: { 'eDP-1': { enabled: true, displayMode: 'dynamicIsland', dynamicIslandTopOffset: 10 } },
    islandEnabled: true,
    displayChoice: 'auto',
    primaryEnabledDisplayId: 'eDP-1',
    readerReason: null,
    ...overrides,
  };
}

describe('top offset', () => {
  test('clamps to the Swift range and rounds', () => {
    expect(clampTopOffset(-5)).toBe(0);
    expect(clampTopOffset(99)).toBe(40);
    expect(clampTopOffset(12.6)).toBe(13);
    expect(clampTopOffset(Number.NaN)).toBe(TOP_OFFSET_DEFAULT);
  });
});

describe('display rows', () => {
  test('a built-in screen uses the shared label', () => {
    expect(displayTitle(display(), t)).toBe('Display 1 — Built-in Display');
    expect(displayMetadata(display(), t)).toBe('1920×1080 · Primary');
  });

  test('an external screen keeps its OS name', () => {
    expect(displayTitle(display({ id: 'HDMI-1', name: 'HDMI-1', isBuiltin: false, isPrimary: false, index: 1 }), t)).toBe('Display 2 — HDMI-1');
    expect(displayMetadata(display({ isPrimary: false }), t)).toBe('1920×1080');
  });

  test('the general summary names the enabled screens', () => {
    expect(displaySummary(snapshot(), t)).toBe('Built-in Display');
    expect(displaySummary(snapshot({ islandEnabled: false }), t)).toBe('Show Morrow Island: off');
    expect(displaySummary(snapshot({ configs: {} }), t)).toBe('All displays hidden');
    expect(displaySummary(snapshot({ displays: [], readerReason: 'no reader' }), t)).toBe('no reader');
  });
});
