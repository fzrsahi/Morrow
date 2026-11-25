// Parity gap #3 — pure decisions for wheel volume and swipe tabs on the island.
// The DOM forwards wheel deltas; this module owns the step, the swipe threshold
// and the tab cycle. Pointer dragging stays in `panel.ts` and App.svelte: a
// wheel event is never treated as a drag.

import type { PanelTab } from './panel';

/** The tabs `Header.svelte` draws, in the order a forward swipe visits them. */
export const PANEL_TABS: readonly PanelTab[] = [
  'agents',
  'workspace',
  'prayer',
  'music',
];

/** #465: turning the Agents feature off leaves one reachable tab. Both the
 *  header and the swipe cycle use this list, so a swipe cannot land on a tab
 *  the user cannot see. */
export function visiblePanelTabs(agentsEnabled: boolean): readonly PanelTab[] {
  return agentsEnabled ? PANEL_TABS : ['workspace', 'prayer', 'music'];
}

/** One wheel notch steps by the same five percent as the meter's arrow keys. */
export const WHEEL_VOLUME_STEP = 5;

/** Mirrors Swift `NotchNavigationManager`: 20 px before a swipe counts. */
export const SWIPE_THRESHOLD_PX = 20;

/** A wheel event after this quiet gap starts a new swipe gesture. */
export const SWIPE_IDLE_MS = 300;

const LINE_PX = 16;
const PAGE_PX = 400;

/** Normalise `WheelEvent.deltaMode` so a line or page notch can cross a threshold. */
export function wheelDeltaPx(delta: number, mode: number): number {
  if (!Number.isFinite(delta)) return 0;
  if (mode === 1) return delta * LINE_PX;
  if (mode === 2) return delta * PAGE_PX;
  return delta;
}

/** Scroll up (negative deltaY) raises the level, as on the macOS notch.
 *  `null` means "no gesture" so the caller can leave the event alone. */
export function wheelVolumePercent(percent: number, deltaY: number): number | null {
  if (!Number.isFinite(deltaY) || deltaY === 0) return null;
  if (!Number.isFinite(percent)) return null;
  const from = Math.min(100, Math.max(0, Math.round(percent)));
  return Math.min(100, Math.max(0, from + (deltaY < 0 ? WHEEL_VOLUME_STEP : -WHEEL_VOLUME_STEP)));
}

/** Cycle through the panel tabs. A swipe never lands nowhere: at the end it
 *  wraps to the first tab, which keeps the two-tab panel reversible. */
export function cycleTab(
  current: PanelTab,
  direction: 1 | -1,
  tabs: readonly PanelTab[] = PANEL_TABS,
): PanelTab {
  if (tabs.length === 0) return current;
  const index = tabs.indexOf(current);
  if (index < 0) return tabs[0];
  return tabs[(index + direction + tabs.length) % tabs.length];
}

/** Accumulates horizontal wheel deltas into one tab switch per gesture.
 *
 *  DOM wheel events have no trackpad phase, so a quiet gap ends a gesture.
 *  Momentum events arrive without a gap and cannot fire a second switch, and a
 *  reversal starts a fresh accumulation in the opposite direction. */
export class SwipeGate {
  private accumulated = 0;
  private latched: 1 | -1 | null = null;
  private lastAt = Number.NEGATIVE_INFINITY;

  /** `1` means swipe left / next tab, `-1` means swipe right / previous tab. */
  push(deltaX: number, now: number): 1 | -1 | null {
    if (!Number.isFinite(deltaX) || deltaX === 0 || !Number.isFinite(now)) return null;
    if (now - this.lastAt >= SWIPE_IDLE_MS) this.reset();
    this.lastAt = now;

    // deltaX is negative when swiping left; Swift inverts it for forward.
    const direction: 1 | -1 = deltaX < 0 ? 1 : -1;
    if (this.latched !== null) {
      if (direction === this.latched) return null;
      // The first event in the opposite direction starts the new gesture;
      // its delta is not counted, so momentum cannot flip back instantly.
      this.latched = null;
      this.accumulated = 0;
      return null;
    }

    this.accumulated += Math.abs(deltaX);
    if (this.accumulated < SWIPE_THRESHOLD_PX) return null;
    this.accumulated = 0;
    this.latched = direction;
    return direction;
  }

  reset(): void {
    this.accumulated = 0;
    this.latched = null;
    this.lastAt = Number.NEGATIVE_INFINITY;
  }
}
