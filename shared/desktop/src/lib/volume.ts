// #195 — pure decisions for the volume HUD. The shell owns `music_volume*`
// (music.rs); this module never asks for the volume itself. It maps a reading
// to the canvas #volume meter and says when the HUD should be on screen.

import { isSupported } from './capabilities';
import type { CapabilityRow } from './types';

/** 12 segments: 0–7 cyan, 8–9 amber, 10–11 red (#volume notes). */
export const VOLUME_SEGMENTS = 12;

/** Canvas #volume: show the meter briefly after a change, then hide it. */
export const VOLUME_HUD_MS = 1500;

export type VolumeSegment = 'cyan' | 'amber' | 'red' | 'off';

/** Mirrors `morrow_core::music::Volume` (percent 0–100). */
export type Volume = { percent: number; muted: boolean };

function litCount(percent: number, muted: boolean): number {
  if (muted) return 0;
  const clamped = Math.min(100, Math.max(0, percent));
  return Math.round((clamped / 100) * VOLUME_SEGMENTS);
}

export function segmentColors(percent: number, muted = false): VolumeSegment[] {
  const lit = litCount(percent, muted);
  return Array.from({ length: VOLUME_SEGMENTS }, (_, i) => {
    if (i >= lit) return 'off';
    if (i < 8) return 'cyan';
    if (i < 10) return 'amber';
    return 'red';
  });
}

export function readout(volume: Volume): string {
  return volume.muted ? 'MUTE' : `${volume.percent}%`;
}

/** #239 AC 2 — the meter is now an input, not only a reading.
 *
 *  Clicking segment `index` asks for the level that lights exactly that many
 *  segments, which is the inverse of `litCount` and must stay one: a click that
 *  asked for 8 % on the first segment would light it and five of its neighbours
 *  read differently. The invariant is a test, not an observation. */
export function percentForSegment(index: number): number {
  const clamped = Math.min(VOLUME_SEGMENTS - 1, Math.max(0, Math.round(index)));
  return Math.round(((clamped + 1) / VOLUME_SEGMENTS) * 100);
}

/** Arrow-key step on the meter (5 %, like a desktop mixer). `null` for a key
 *  that is not the meter's business, so the caller can let it through. */
export function percentForKey(percent: number, key: string): number | null {
  const from = Math.min(100, Math.max(0, Math.round(percent)));
  switch (key) {
    case 'ArrowRight':
    case 'ArrowUp':
      return Math.min(100, from + 5);
    case 'ArrowLeft':
    case 'ArrowDown':
      return Math.max(0, from - 5);
    case 'Home':
      return 0;
    case 'End':
      return 100;
    default:
      return null;
  }
}

/** What a click on the meter does to the shell, in order.
 *
 *  Setting a level while muted also unmutes. Neither adapter clears the flag
 *  itself (`wpctl set-volume` and `IAudioEndpointVolume` both leave it alone),
 *  so without this the click would draw a dark meter over sound that stays
 *  silent — the user would click again, and again. */
export function setVolumePlan(muted: boolean, percent: number): { unmuteFirst: boolean; percent: number } {
  return { unmuteFirst: muted, percent: Math.min(100, Math.max(0, Math.round(percent))) };
}

/** A device without a volume adapter must not get a HUD that cannot answer a
 *  click. Only a plain `Supported` row counts; `Degraded` still hides it. */
export function volumeSupported(rows: CapabilityRow[]): boolean {
  return isSupported(rows, 'System volume');
}

/** Change detection and the 1.5 s dwell, on the webview's clock. The first
 *  reading seeds the baseline so a launch never pops the HUD. */
export class VolumeHudGate {
  private last: Volume | null = null;
  private until = Number.NEGATIVE_INFINITY;

  observe(next: Volume, now: number): boolean {
    const previous = this.last;
    this.last = { ...next };
    if (previous === null || !changed(previous, next)) return false;
    this.until = now + VOLUME_HUD_MS;
    return true;
  }

  /** Extend the dwell for a change we made ourselves (mute click): the
   *  follow-up `music_volume` read may race or fail. */
  show(now: number): void {
    this.until = now + VOLUME_HUD_MS;
  }

  visible(now: number): boolean {
    return now < this.until;
  }
}

function changed(a: Volume, b: Volume): boolean {
  return a.percent !== b.percent || a.muted !== b.muted;
}
