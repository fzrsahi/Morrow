// #465 — the Behaviour page's keys, read from the same settings document the
// shell and macOS use. Defaults and raw key spellings mirror Swift's
// `SettingsKey` / `SettingsDefaults`; a value the shell cannot obey is still
// stored here, and the page says so rather than pretending it took effect.

import type { AmbientState, AmbientSupport } from './ambient';

/** Swift `SettingsKey` spellings. One table, so a control and its reader cannot drift. */
export const BEHAVIOR_KEYS = {
  hideInFullscreen: 'hideInFullscreen',
  hideWhenNoSession: 'hideWhenNoSession',
  smartSuppress: 'smartSuppress',
  collapseOnMouseLeave: 'collapseOnMouseLeave',
  hoverExpandDelay: 'hoverExpandDelay',
  autoCollapseAfterSessionJump: 'autoCollapseAfterSessionJump',
  autoExpandOnPermission: 'autoExpandOnPermission',
  autoExpandOnQuestion: 'autoExpandOnQuestion',
  autoExpandOnCompletion: 'autoExpandOnCompletion',
  completionNotificationStyle: 'completionNotificationStyle',
  pluginSessionMode: 'pluginSessionMode',
  sessionTimeout: 'sessionTimeout',
  rotationInterval: 'rotationInterval',
  maxToolHistory: 'maxToolHistory',
  autoApproveTools: 'autoApproveTools',
  autoApproveSources: 'autoApproveSources',
  excludedHookCwdSubstrings: 'excludedHookCwdSubstrings',
  claudeConfigDir: 'claude_config_dir',
  extraConfigDirs: 'extra_config_dirs_v1',
  webhookEnabled: 'webhookEnabled',
  webhookURL: 'webhookURL',
  webhookEventFilter: 'webhookEventFilter',
  allowHorizontalDrag: 'allowHorizontalDrag',
  avoidMenuBarIcons: 'avoidMenuBarIcons',
} as const;

export type CompletionStyle = 'expand' | 'glance' | 'off';
export type PluginSessionMode = 'separate' | 'merge' | 'hide';

export type BehaviorSettings = {
  hideInFullscreen: boolean;
  hideWhenNoSession: boolean;
  smartSuppress: boolean;
  collapseOnMouseLeave: boolean;
  hoverExpandDelay: number;
  autoCollapseAfterSessionJump: boolean;
  autoExpandOnPermission: boolean;
  autoExpandOnQuestion: boolean;
  autoExpandOnCompletion: boolean;
  completionNotificationStyle: CompletionStyle;
  pluginSessionMode: PluginSessionMode;
  sessionTimeout: number;
  rotationInterval: number;
  maxToolHistory: number;
  autoApproveTools: string;
  autoApproveSources: string;
  excludedHookCwdSubstrings: string;
  claudeConfigDir: string;
  extraConfigDirs: string;
  webhookEnabled: boolean;
  webhookURL: string;
  webhookEventFilter: string;
  allowHorizontalDrag: boolean;
  avoidMenuBarIcons: boolean;
};

export const DEFAULT_BEHAVIOR: BehaviorSettings = {
  hideInFullscreen: false,
  hideWhenNoSession: false,
  smartSuppress: true,
  collapseOnMouseLeave: true,
  hoverExpandDelay: 0.5,
  autoCollapseAfterSessionJump: false,
  autoExpandOnPermission: true,
  autoExpandOnQuestion: true,
  autoExpandOnCompletion: true,
  completionNotificationStyle: 'expand',
  pluginSessionMode: 'separate',
  sessionTimeout: 30,
  rotationInterval: 5,
  maxToolHistory: 20,
  autoApproveTools: '',
  autoApproveSources: '',
  excludedHookCwdSubstrings: '',
  claudeConfigDir: '',
  extraConfigDirs: '',
  webhookEnabled: false,
  webhookURL: '',
  webhookEventFilter: '',
  allowHorizontalDrag: false,
  avoidMenuBarIcons: true,
};

const COMPLETION_STYLES: readonly CompletionStyle[] = ['expand', 'glance', 'off'];
const SESSION_MODES: readonly PluginSessionMode[] = ['separate', 'merge', 'hide'];
const HOVER_RANGE = { min: 0.1, max: 1.0 } as const;

function object(value: unknown): Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

function flag(stored: Record<string, unknown>, key: keyof typeof BEHAVIOR_KEYS): boolean {
  const value = stored[BEHAVIOR_KEYS[key]];
  return typeof value === 'boolean' ? value : DEFAULT_BEHAVIOR[key] as boolean;
}

function text(stored: Record<string, unknown>, key: keyof typeof BEHAVIOR_KEYS): string {
  const value = stored[BEHAVIOR_KEYS[key]];
  return typeof value === 'string' ? value : DEFAULT_BEHAVIOR[key] as string;
}

function integer(
  stored: Record<string, unknown>,
  key: keyof typeof BEHAVIOR_KEYS,
  min: number,
  max: number,
): number {
  const value = stored[BEHAVIOR_KEYS[key]];
  return typeof value === 'number' && Number.isInteger(value) && value >= min && value <= max
    ? value
    : (DEFAULT_BEHAVIOR[key] as number);
}

/** Swift's completionStyle migration: a written false on the old boolean means off. */
export function completionStyleFromSettings(stored: Record<string, unknown>): CompletionStyle {
  const raw = stored[BEHAVIOR_KEYS.completionNotificationStyle];
  if (COMPLETION_STYLES.includes(raw as CompletionStyle)) return raw as CompletionStyle;
  const legacy = stored[BEHAVIOR_KEYS.autoExpandOnCompletion];
  return legacy === false ? 'off' : 'expand';
}

/** Split a comma-separated Swift preference into clean, non-empty entries. */
export function csvEntries(value: string): string[] {
  return value
    .split(',')
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0);
}

/** Read every Behaviour key with Swift's defaults when a key is absent or wrong. */
export function behaviorFromSettings(value: unknown): BehaviorSettings {
  const stored = object(value);
  const rawDelay = stored[BEHAVIOR_KEYS.hoverExpandDelay];
  const mode = stored[BEHAVIOR_KEYS.pluginSessionMode];
  return {
    hideInFullscreen: flag(stored, 'hideInFullscreen'),
    hideWhenNoSession: flag(stored, 'hideWhenNoSession'),
    smartSuppress: flag(stored, 'smartSuppress'),
    collapseOnMouseLeave: flag(stored, 'collapseOnMouseLeave'),
    hoverExpandDelay:
      typeof rawDelay === 'number' &&
      Number.isFinite(rawDelay) &&
      rawDelay >= HOVER_RANGE.min &&
      rawDelay <= HOVER_RANGE.max
        ? rawDelay
        : DEFAULT_BEHAVIOR.hoverExpandDelay,
    autoCollapseAfterSessionJump: flag(stored, 'autoCollapseAfterSessionJump'),
    autoExpandOnPermission: flag(stored, 'autoExpandOnPermission'),
    autoExpandOnQuestion: flag(stored, 'autoExpandOnQuestion'),
    autoExpandOnCompletion: flag(stored, 'autoExpandOnCompletion'),
    completionNotificationStyle: completionStyleFromSettings(stored),
    pluginSessionMode: SESSION_MODES.includes(mode as PluginSessionMode)
      ? (mode as PluginSessionMode)
      : DEFAULT_BEHAVIOR.pluginSessionMode,
    sessionTimeout: integer(stored, 'sessionTimeout', 1, 24 * 60),
    rotationInterval: integer(stored, 'rotationInterval', 1, 3600),
    maxToolHistory: integer(stored, 'maxToolHistory', 1, 1000),
    autoApproveTools: text(stored, 'autoApproveTools'),
    autoApproveSources: text(stored, 'autoApproveSources'),
    excludedHookCwdSubstrings: text(stored, 'excludedHookCwdSubstrings'),
    claudeConfigDir: text(stored, 'claudeConfigDir'),
    extraConfigDirs: text(stored, 'extraConfigDirs'),
    webhookEnabled: flag(stored, 'webhookEnabled'),
    webhookURL: text(stored, 'webhookURL'),
    webhookEventFilter: text(stored, 'webhookEventFilter'),
    allowHorizontalDrag: flag(stored, 'allowHorizontalDrag'),
    avoidMenuBarIcons: flag(stored, 'avoidMenuBarIcons'),
  };
}

/** The dwell timer, in milliseconds, for a written `hoverExpandDelay` in seconds. */
export function hoverDelayMs(seconds: number): number {
  const value = Number.isFinite(seconds) ? seconds : DEFAULT_BEHAVIOR.hoverExpandDelay;
  return Math.round(Math.min(HOVER_RANGE.max, Math.max(HOVER_RANGE.min, value)) * 1000);
}

export type IslandVisibilityInput = {
  ambient: AmbientState;
  support: AmbientSupport;
  sessionCount: number;
  expanded: boolean;
  behavior: Pick<BehaviorSettings, 'hideInFullscreen' | 'hideWhenNoSession'>;
};

/**
 * Which settings may hide the OS window. `hideWhenNoSession` keeps a refresh
 * button reachable once the panel is open, and fullscreen hiding only applies
 * where the shell has a fullscreen reader at all (the ambient support row).
 */
export function islandHidden(input: IslandVisibilityInput): boolean {
  const fullscreen = input.support.fullscreenDetection && input.ambient.fullscreen;
  if (input.behavior.hideInFullscreen && fullscreen) return true;
  return input.behavior.hideWhenNoSession && input.sessionCount === 0 && !input.expanded;
}
