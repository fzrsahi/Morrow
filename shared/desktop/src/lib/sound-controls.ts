// #465 — the Sound page's keys and defaults, spelled as Swift `SettingsKey`s.
// The shell's `attention::sound_preferences` reads the same document; this
// module is the Settings side of that contract.

export const SOUND_KEYS = {
  enabled: 'soundEnabled',
  volume: 'soundVolume',
  sessionStart: 'soundSessionStart',
  taskComplete: 'soundTaskComplete',
  taskError: 'soundTaskError',
  approvalNeeded: 'soundApprovalNeeded',
  promptSubmit: 'soundPromptSubmit',
  boot: 'soundBoot',
  quietHoursEnabled: 'quietHoursEnabled',
  quietHoursStart: 'quietHoursStart',
  quietHoursEnd: 'quietHoursEnd',
  autoMuteWhenAway: 'autoMuteWhenAway',
} as const;

export type SoundControls = {
  enabled: boolean;
  volume: number;
  sessionStart: boolean;
  taskComplete: boolean;
  taskError: boolean;
  approvalNeeded: boolean;
  promptSubmit: boolean;
  boot: boolean;
  quietHoursEnabled: boolean;
  quietHoursStart: number;
  quietHoursEnd: number;
  autoMuteWhenAway: boolean;
};

export const DEFAULT_SOUND_CONTROLS: SoundControls = {
  enabled: false,
  volume: 50,
  sessionStart: true,
  taskComplete: true,
  taskError: true,
  approvalNeeded: true,
  promptSubmit: false,
  boot: true,
  quietHoursEnabled: false,
  quietHoursStart: 22 * 60,
  quietHoursEnd: 8 * 60,
  autoMuteWhenAway: true,
};

export type SoundToggle =
  | 'sessionStart'
  | 'taskComplete'
  | 'taskError'
  | 'approvalNeeded'
  | 'promptSubmit'
  | 'boot';

/** The switches in `SoundManager.eventSounds` plus AppLaunch. */
export const SOUND_EVENTS: ReadonlyArray<{ field: SoundToggle; label: string }> = [
  { field: 'sessionStart', label: 'Session start' },
  { field: 'taskComplete', label: 'Round complete' },
  { field: 'taskError', label: 'Turn failed' },
  { field: 'approvalNeeded', label: 'Approval needed' },
  { field: 'promptSubmit', label: 'Prompt submitted' },
  { field: 'boot', label: 'App launch' },
];

const BOOLEAN_FIELDS = [
  'enabled',
  'sessionStart',
  'taskComplete',
  'taskError',
  'approvalNeeded',
  'promptSubmit',
  'boot',
  'quietHoursEnabled',
  'autoMuteWhenAway',
] as const;

function object(value: unknown): Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

/** An absent key means the shipped default; a malformed one is ignored. */
export function soundControlsFromSettings(value: unknown): SoundControls {
  const stored = object(value);
  const next = { ...DEFAULT_SOUND_CONTROLS };
  for (const field of BOOLEAN_FIELDS) {
    const raw = stored[SOUND_KEYS[field]];
    if (typeof raw === 'boolean') next[field] = raw;
  }
  const volume = stored[SOUND_KEYS.volume];
  if (typeof volume === 'number' && Number.isFinite(volume)) {
    next.volume = Math.max(0, Math.min(100, Math.round(volume)));
  }
  for (const field of ['quietHoursStart', 'quietHoursEnd'] as const) {
    const raw = stored[SOUND_KEYS[field]];
    if (typeof raw === 'number' && Number.isFinite(raw)) {
      next[field] = Math.max(0, Math.min(24 * 60 - 1, Math.round(raw)));
    }
  }
  return next;
}

/** `22:00` from minutes since midnight, for the two quiet-hours inputs. */
export function minutesToTime(minutes: number): string {
  const capped = Math.max(0, Math.min(24 * 60 - 1, Math.round(minutes)));
  const hour = Math.floor(capped / 60);
  return `${String(hour).padStart(2, '0')}:${String(capped % 60).padStart(2, '0')}`;
}

export function timeToMinutes(value: string): number | null {
  const match = /^(\d{1,2}):(\d{2})$/.exec(value.trim());
  if (!match) return null;
  const hour = Number(match[1]);
  const minute = Number(match[2]);
  if (hour > 23 || minute > 59) return null;
  return hour * 60 + minute;
}
