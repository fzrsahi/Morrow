// One answer to "is this capability Supported?" for every caller: the volume
// HUD and the island's ambient adapters must agree on what a row means.

import type { CapabilityRow } from './types';

/** Only a plain `Supported` row counts; `Degraded` and `Unsupported` do not. */
export function isSupported(rows: CapabilityRow[], feature: string): boolean {
  return rows.some((row) => row.feature === feature && 'Supported' in row.capability);
}
