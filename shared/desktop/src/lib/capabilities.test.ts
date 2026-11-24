// Review finding: capability "Supported" was tested twice, once in volume.ts
// and once in App.svelte. One shared seam answers it for every feature.
import { describe, expect, test } from 'bun:test';
import { isSupported } from './capabilities';
import type { CapabilityRow } from './types';

const row = (feature: string, capability: CapabilityRow['capability']): CapabilityRow => ({
  feature,
  capability,
});

describe('isSupported (shared capability probe)', () => {
  test('a Supported row answers true for its feature', () => {
    expect(isSupported([row('Screen lock', { Supported: null })], 'Screen lock')).toBe(true);
  });

  test('Degraded and Unsupported rows are not Supported', () => {
    expect(isSupported([row('System volume', { Degraded: 'needs a mixer' })], 'System volume')).toBe(
      false,
    );
    expect(isSupported([row('System volume', { Unsupported: 'no wpctl' })], 'System volume')).toBe(
      false,
    );
  });

  test('a different feature or an empty table is not Supported', () => {
    expect(isSupported([row('Now Playing', { Supported: null })], 'Screen lock')).toBe(false);
    expect(isSupported([], 'Screen lock')).toBe(false);
  });
});
