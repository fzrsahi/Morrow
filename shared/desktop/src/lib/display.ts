// #465 — the shape the shell's `display_snapshot` command returns, and the
// pure formatting the Settings rows share. Field names mirror Swift's
// `DisplayDescriptor` / `DisplayConfiguration` / `DynamicIslandTopOffset`.

import type { DisplayMode } from './panel';

export type DisplayDescriptor = {
  id: string;
  index: number;
  name: string;
  width: number;
  height: number;
  hasNotch: boolean;
  isBuiltin: boolean;
  isPrimary: boolean;
};

export type DisplayConfiguration = {
  enabled: boolean;
  displayMode: DisplayMode;
  dynamicIslandTopOffset: number;
};

export type DisplaySnapshot = {
  displays: DisplayDescriptor[];
  configs: Record<string, DisplayConfiguration>;
  islandEnabled: boolean;
  displayChoice: string;
  primaryEnabledDisplayId: string | null;
  readerReason: string | null;
};

export const TOP_OFFSET_MIN = 0;
export const TOP_OFFSET_MAX = 40;
export const TOP_OFFSET_DEFAULT = 10;

/** Swift `DynamicIslandTopOffset.clamp`. */
export function clampTopOffset(value: number): number {
  if (!Number.isFinite(value)) return TOP_OFFSET_DEFAULT;
  return Math.max(TOP_OFFSET_MIN, Math.min(TOP_OFFSET_MAX, Math.round(value)));
}

/** Swift `DisplayConfigRow.displayTitle`. */
export function displayTitle(display: DisplayDescriptor, t: (key: string) => string): string {
  const numbered = t('display_numbered').replace('%d', String(display.index + 1));
  const name = display.isBuiltin ? t('builtin_display') : display.name;
  return `${numbered} — ${name}`;
}

/** The caption under a row: resolution and which screen is primary. */
export function displayMetadata(display: DisplayDescriptor, t: (key: string) => string): string {
  const parts = [`${display.width}×${display.height}`];
  if (display.isPrimary) parts.push('Primary');
  if (display.hasNotch) parts.push(t('notch'));
  return parts.join(' · ');
}

/** Swift `DisplaySettingsSummaryFormatter.summary`, in one sentence. */
export function displaySummary(snapshot: DisplaySnapshot, t: (key: string) => string): string {
  if (!snapshot.islandEnabled) return `${t('island_enabled')}: off`;
  if (snapshot.displays.length === 0) return snapshot.readerReason ?? 'No displays detected';
  const enabled = snapshot.displays.filter((d) => snapshot.configs[d.id]?.enabled);
  const names = enabled.map((d) => (d.isBuiltin ? t('builtin_display') : d.name));
  return names.length ? names.join(', ') : 'All displays hidden';
}
