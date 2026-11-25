// Parity gap #3 — the pure wheel/swipe seams. No webview, no shell commands:
// these tests pin the step, the threshold and the tab cycle the island uses.
import { describe, expect, test } from 'bun:test';

import {
  PANEL_TABS,
  SWIPE_IDLE_MS,
  SWIPE_THRESHOLD_PX,
  SwipeGate,
  cycleTab,
  visiblePanelTabs,
  wheelDeltaPx,
  wheelVolumePercent,
} from './gestures';

describe('visible tabs (#465)', () => {
  test('disabling the Agents feature leaves only the Music tab to cycle', () => {
    expect(visiblePanelTabs(true)).toEqual(['agents', 'music']);
    expect(visiblePanelTabs(false)).toEqual(['music']);
    expect(cycleTab('music', 1, visiblePanelTabs(false))).toBe('music');
  });
});

describe('wheel volume', () => {
  test('scrolling up raises the level and scrolling down lowers it', () => {
    expect(wheelVolumePercent(50, -100)).toBe(55);
    expect(wheelVolumePercent(50, 100)).toBe(45);
  });

  test('the level stops at the ends of the meter', () => {
    expect(wheelVolumePercent(0, 100)).toBe(0);
    expect(wheelVolumePercent(100, -100)).toBe(100);
    expect(wheelVolumePercent(98, -100)).toBe(100);
    expect(wheelVolumePercent(2, 100)).toBe(0);
  });

  test('no wheel movement is not a volume gesture', () => {
    expect(wheelVolumePercent(50, 0)).toBeNull();
    expect(wheelVolumePercent(50, Number.NaN)).toBeNull();
    expect(wheelVolumePercent(Number.NaN, -100)).toBeNull();
  });

  test('line and page wheel modes reach the same thresholds', () => {
    expect(wheelDeltaPx(3, 0)).toBe(3);
    expect(wheelDeltaPx(3, 1)).toBe(48);
    expect(wheelDeltaPx(1, 2)).toBe(400);
    expect(wheelDeltaPx(Number.NaN, 1)).toBe(0);
  });
});

describe('tab cycle', () => {
  test('the list is the two tabs the header draws, in header order', () => {
    expect(PANEL_TABS).toEqual(['agents', 'music']);
  });

  test('a forward swipe visits the next tab and wraps at the end', () => {
    expect(cycleTab('agents', 1)).toBe('music');
    expect(cycleTab('music', 1)).toBe('agents');
  });

  test('a backward swipe reverses the same cycle', () => {
    expect(cycleTab('music', -1)).toBe('agents');
    expect(cycleTab('agents', -1)).toBe('music');
  });

  test('an unknown tab starts at the first tab', () => {
    expect(cycleTab('unknown' as never, 1)).toBe('agents');
    expect(cycleTab('agents', 1, [])).toBe('agents');
  });
});

describe('swipe gate', () => {
  test('a small horizontal movement does not switch tabs', () => {
    const gate = new SwipeGate();
    expect(gate.push(-(SWIPE_THRESHOLD_PX - 5), 0)).toBeNull();
    expect(gate.push(-1, 10)).toBeNull();
  });

  test('crossing the threshold swipes forward once per gesture', () => {
    const gate = new SwipeGate();
    expect(gate.push(-5, 0)).toBeNull();
    expect(gate.push(-(SWIPE_THRESHOLD_PX - 5), 10)).toBe(1);
    // Trackpad momentum keeps arriving; it must not flip through every tab.
    expect(gate.push(-40, 20)).toBeNull();
    expect(gate.push(-40, 30)).toBeNull();
  });

  test('a swipe to the right goes back', () => {
    const gate = new SwipeGate();
    expect(gate.push(SWIPE_THRESHOLD_PX, 100)).toBe(-1);
    expect(gate.push(SWIPE_THRESHOLD_PX, 110)).toBeNull();
  });

  test('reversing direction starts a new gesture', () => {
    const gate = new SwipeGate();
    expect(gate.push(-SWIPE_THRESHOLD_PX, 0)).toBe(1);
    expect(gate.push(SWIPE_THRESHOLD_PX, 10)).toBeNull();
    expect(gate.push(SWIPE_THRESHOLD_PX * 2, 20)).toBe(-1);
  });

  test('a quiet gap re-arms the gate for the next swipe', () => {
    const gate = new SwipeGate();
    expect(gate.push(-SWIPE_THRESHOLD_PX, 0)).toBe(1);
    // Momentum continues just under the idle gap, so it stays one gesture.
    expect(gate.push(-SWIPE_THRESHOLD_PX, SWIPE_IDLE_MS - 1)).toBeNull();
    expect(gate.push(-SWIPE_THRESHOLD_PX, SWIPE_IDLE_MS * 2 - 1)).toBe(1);
  });

  test('reset discards an unfinished gesture', () => {
    const gate = new SwipeGate();
    expect(gate.push(-(SWIPE_THRESHOLD_PX - 1), 0)).toBeNull();
    gate.reset();
    expect(gate.push(-1, 10)).toBeNull();
  });

  test('non-finite wheel data is ignored', () => {
    const gate = new SwipeGate();
    expect(gate.push(Number.NaN, 0)).toBeNull();
    expect(gate.push(-SWIPE_THRESHOLD_PX, Number.NaN)).toBeNull();
    expect(gate.push(0, 0)).toBeNull();
  });
});
