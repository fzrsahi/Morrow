// #195 — the pure seams of the volume HUD. The shell owns `music_volume*`;
// this module only decides what the 12-segment meter draws, when the HUD is
// visible, and whether the System volume adapter exists on this machine.
import { describe, expect, test } from 'bun:test';
import {
  VOLUME_HUD_MS,
  VolumeHudGate,
  readout,
  segmentColors,
  volumeSupported,
  type Volume,
} from './volume';
import type { CapabilityRow } from './types';

const volume = (percent: number, muted = false): Volume => ({ percent, muted });

describe('the 12-segment meter', () => {
  test('62 percent lights seven cyan segments', () => {
    // Canvas: Math.round(62 / 100 * 12) = 7 lit cells.
    const colors = segmentColors(62);
    expect(colors).toHaveLength(12);
    expect(colors.slice(0, 7).every((color) => color === 'cyan')).toBe(true);
    expect(colors.slice(7).every((color) => color === 'off')).toBe(true);
  });

  test('segments 8 and 9 are amber, 10 and 11 are red', () => {
    const colors = segmentColors(100);
    expect(colors[7]).toBe('cyan');
    expect(colors[8]).toBe('amber');
    expect(colors[9]).toBe('amber');
    expect(colors[10]).toBe('red');
    expect(colors[11]).toBe('red');
  });

  test('mute lights nothing at all', () => {
    expect(segmentColors(100, true).every((color) => color === 'off')).toBe(true);
  });

  test('a reading outside 0–100 cannot light more or fewer segments', () => {
    expect(segmentColors(-5)).toEqual(segmentColors(0));
    expect(segmentColors(140)).toEqual(segmentColors(100));
  });
});

describe('the mono readout', () => {
  test('an unmuted reading prints its percent', () => {
    expect(readout(volume(62))).toBe('62%');
  });

  test('mute prints MUTE instead of a number', () => {
    expect(readout(volume(40, true))).toBe('MUTE');
  });
});

describe('the HUD only appears after a volume change', () => {
  test('the first reading is a baseline, not a change', () => {
    const gate = new VolumeHudGate();
    expect(gate.observe(volume(62), 1000)).toBe(false);
    expect(gate.visible(1000)).toBe(false);
  });

  test('an unchanged reading does not pop the HUD', () => {
    const gate = new VolumeHudGate();
    gate.observe(volume(62), 1000);
    expect(gate.observe(volume(62), 1400)).toBe(false);
    expect(gate.visible(1400)).toBe(false);
  });

  test('a new percent shows the HUD for about 1.5 s', () => {
    const gate = new VolumeHudGate();
    gate.observe(volume(62), 1000);
    expect(gate.observe(volume(80), 2000)).toBe(true);
    expect(VOLUME_HUD_MS).toBe(1500);
    expect(gate.visible(2000)).toBe(true);
    expect(gate.visible(3499)).toBe(true);
    expect(gate.visible(3500)).toBe(false);
  });

  test('muting is a volume change too', () => {
    const gate = new VolumeHudGate();
    gate.observe(volume(62), 1000);
    expect(gate.observe(volume(62, true), 2000)).toBe(true);
    expect(gate.visible(2000)).toBe(true);
  });

  test('a change while the HUD is up restarts its dwell', () => {
    const gate = new VolumeHudGate();
    gate.observe(volume(62), 1000);
    gate.observe(volume(80), 2000);
    gate.observe(volume(40), 2600);
    expect(gate.visible(3499)).toBe(true);
    expect(gate.visible(4100)).toBe(false);
  });

  test('an explicit show keeps the HUD up after our own mute click', () => {
    const gate = new VolumeHudGate();
    gate.observe(volume(62), 1000);
    gate.show(2000);
    expect(gate.visible(3499)).toBe(true);
  });
});

describe('the System volume capability', () => {
  const row = (feature: string, capability: CapabilityRow['capability']): CapabilityRow => ({
    feature,
    capability,
  });

  test('Supported enables the HUD', () => {
    expect(volumeSupported([row('System volume', { Supported: null })])).toBe(true);
  });

  test('an Unsupported or Degraded adapter disables it', () => {
    expect(volumeSupported([row('System volume', { Unsupported: 'no wpctl' })])).toBe(false);
    expect(volumeSupported([row('System volume', { Degraded: 'needs a mixer' })])).toBe(false);
  });

  test('a machine with no capability row has no volume HUD', () => {
    expect(volumeSupported([])).toBe(false);
    expect(volumeSupported([row('Now Playing', { Supported: null })])).toBe(false);
  });
});
