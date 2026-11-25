// #465 — the Buddy (ESP32) page's settings slice. The wire protocol is
// `morrow-core::buddy` and stays in Rust; this module owns the keys Swift
// writes and the same brightness/orientation rules the frame preview uses.

export const BUDDY_KEYS = {
  bridgeEnabled: 'esp32BridgeEnabled',
  heartbeatSeconds: 'esp32HeartbeatSeconds',
  brightness: 'buddyScreenBrightnessPercent',
  orientation: 'buddyScreenOrientation',
  selectedIdentifier: 'selectedBuddyIdentifier',
  selectedName: 'selectedBuddyName',
} as const;

export type BuddySettings = {
  bridgeEnabled: boolean;
  heartbeatSeconds: number;
  brightness: number;
  orientation: 'up' | 'down';
  selectedIdentifier: string;
  selectedName: string;
};

export const DEFAULT_BUDDY: BuddySettings = {
  bridgeEnabled: false,
  heartbeatSeconds: 5,
  brightness: 70,
  orientation: 'up',
  selectedIdentifier: '',
  selectedName: '',
};

/** Swift's `clampedBrightnessPercent`: 10–100, NaN means the default. */
export function clampBrightness(value: number): number {
  if (!Number.isFinite(value)) return DEFAULT_BUDDY.brightness;
  return Math.max(10, Math.min(100, Math.round(value)));
}

export function buddyFromSettings(value: unknown): BuddySettings {
  const stored = value !== null && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
  const heartbeat = stored[BUDDY_KEYS.heartbeatSeconds];
  return {
    bridgeEnabled:
      typeof stored[BUDDY_KEYS.bridgeEnabled] === 'boolean'
        ? (stored[BUDDY_KEYS.bridgeEnabled] as boolean)
        : DEFAULT_BUDDY.bridgeEnabled,
    heartbeatSeconds:
      typeof heartbeat === 'number' && Number.isFinite(heartbeat) && heartbeat >= 1 && heartbeat <= 60
        ? heartbeat
        : DEFAULT_BUDDY.heartbeatSeconds,
    brightness:
      typeof stored[BUDDY_KEYS.brightness] === 'number'
        ? clampBrightness(stored[BUDDY_KEYS.brightness] as number)
        : DEFAULT_BUDDY.brightness,
    orientation: stored[BUDDY_KEYS.orientation] === 'down' ? 'down' : 'up',
    selectedIdentifier:
      typeof stored[BUDDY_KEYS.selectedIdentifier] === 'string'
        ? (stored[BUDDY_KEYS.selectedIdentifier] as string)
        : '',
    selectedName:
      typeof stored[BUDDY_KEYS.selectedName] === 'string'
        ? (stored[BUDDY_KEYS.selectedName] as string)
        : '',
  };
}
