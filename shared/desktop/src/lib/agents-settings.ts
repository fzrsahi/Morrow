// #465 — the Agents Settings page's keys. The names and defaults mirror Swift
// `SettingsKey` / `SettingsDefaults`; the panel obeys the ones it can render
// (theme, font, row limit, density and card details) and the page says which
// keys are only stored for macOS compatibility.

import { COMFORTABLE_SESSION_LIMIT, VISIBLE_SESSION_LIMIT } from './session-list';
import type { SessionDensity } from './panel';

export const AGENT_KEYS = {
  agentsEnabled: 'agentsEnabled',
  maxVisibleSessions: 'maxVisibleSessions',
  sessionListDensity: 'sessionListDensity',
  contentFontSize: 'contentFontSize',
  aiMessageLines: 'aiMessageLines',
  showAgentDetails: 'showAgentDetails',
  showToolStatus: 'showToolStatus',
  showTaskProgress: 'showTaskProgress',
  showSessionRecap: 'showSessionRecap',
  showModelLabel: 'showModelLabel',
  showUsageStats: 'showUsageStats',
  showClaudeQuota: 'showClaudeQuota',
  claudeQuotaChip: 'claudeQuotaChip',
  agentsHUDTheme: 'agentsHUDTheme',
} as const;

export type AgentsHUDTheme = 'pixelRetro' | 'modern';
export type QuotaChipMode = 'off' | 'auto' | 'session' | 'weeklyAll' | 'weeklyScoped';
export type ReplyLines = 0 | 1 | 2 | 3 | 5;

export type AgentsSettings = {
  agentsEnabled: boolean;
  maxVisibleSessions: number;
  sessionListDensity: SessionDensity;
  contentFontSize: 11 | 12 | 14 | 16;
  aiMessageLines: ReplyLines;
  showAgentDetails: boolean;
  showToolStatus: boolean;
  showTaskProgress: boolean;
  showSessionRecap: boolean;
  showModelLabel: boolean;
  showUsageStats: boolean;
  showClaudeQuota: boolean;
  claudeQuotaChip: QuotaChipMode;
  agentsHUDTheme: AgentsHUDTheme;
};

export const DEFAULT_AGENTS: AgentsSettings = {
  agentsEnabled: true,
  maxVisibleSessions: 5,
  sessionListDensity: 'comfortable',
  contentFontSize: 11,
  aiMessageLines: 1,
  showAgentDetails: false,
  showToolStatus: true,
  showTaskProgress: true,
  showSessionRecap: true,
  showModelLabel: false,
  showUsageStats: true,
  showClaudeQuota: false,
  claudeQuotaChip: 'auto',
  agentsHUDTheme: 'pixelRetro',
};

const FONT_SIZES = [11, 12, 14, 16] as const;
const REPLY_LINES = [0, 1, 2, 3, 5] as const;
const SESSION_LIMITS = [3, 5, 8, 10, 99] as const;
const DENSITIES = ['comfortable', 'compact'] as const;
const THEMES = ['pixelRetro', 'modern'] as const;
const CHIP_MODES = ['off', 'auto', 'session', 'weeklyAll', 'weeklyScoped'] as const;

function object(value: unknown): Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

function flag(stored: Record<string, unknown>, key: keyof typeof AGENT_KEYS): boolean {
  const value = stored[AGENT_KEYS[key]];
  return typeof value === 'boolean' ? value : (DEFAULT_AGENTS[key] as boolean);
}

function oneOf<T extends string | number>(
  stored: Record<string, unknown>,
  key: keyof typeof AGENT_KEYS,
  allowed: readonly T[],
): T {
  const value = stored[AGENT_KEYS[key]];
  return allowed.includes(value as T) ? (value as T) : (DEFAULT_AGENTS[key] as T);
}

/** The stored key is Swift's `sessionListDensity`; the panel calls it `SessionDensity`. */
export function agentsFromSettings(value: unknown): AgentsSettings {
  const stored = object(value);
  return {
    agentsEnabled: flag(stored, 'agentsEnabled'),
    maxVisibleSessions: oneOf(stored, 'maxVisibleSessions', SESSION_LIMITS),
    sessionListDensity: oneOf(stored, 'sessionListDensity', DENSITIES),
    contentFontSize: oneOf(stored, 'contentFontSize', FONT_SIZES),
    aiMessageLines: oneOf(stored, 'aiMessageLines', REPLY_LINES),
    showAgentDetails: flag(stored, 'showAgentDetails'),
    showToolStatus: flag(stored, 'showToolStatus'),
    showTaskProgress: flag(stored, 'showTaskProgress'),
    showSessionRecap: flag(stored, 'showSessionRecap'),
    showModelLabel: flag(stored, 'showModelLabel'),
    showUsageStats: flag(stored, 'showUsageStats'),
    showClaudeQuota: flag(stored, 'showClaudeQuota'),
    claudeQuotaChip: oneOf(stored, 'claudeQuotaChip', CHIP_MODES),
    agentsHUDTheme: oneOf(stored, 'agentsHUDTheme', THEMES),
  };
}

/**
 * How many rows the scroller exposes at once. An untouched setting keeps the
 * canvas' density-based viewport; a written value (including Unlimited/99)
 * wins, as in Swift. The footer still counts everything beyond the viewport.
 */
export function sessionViewportLimit(density: SessionDensity, maxVisibleSessions: number): number {
  if (maxVisibleSessions >= 99) return Number.MAX_SAFE_INTEGER;
  if (maxVisibleSessions === DEFAULT_AGENTS.maxVisibleSessions) {
    return density === 'compact' ? VISIBLE_SESSION_LIMIT : COMFORTABLE_SESSION_LIMIT;
  }
  return Math.max(1, maxVisibleSessions);
}

/** The subset of Agents switches the session cards actually render. */
export type CardDetails = {
  showDetails: boolean;
  showToolStatus: boolean;
  showTaskProgress: boolean;
  showRecap: boolean;
  showModelLabel: boolean;
  replyLines: ReplyLines;
};

/** The Agents tab has its own theme switch; it uses the same two panel themes. */
export function agentsTheme(theme: AgentsHUDTheme): 'pixel' | 'modern' {
  return theme === 'modern' ? 'modern' : 'pixel';
}

/** CSS line clamp: 0 means Swift's Unlimited, which needs no clamp at all. */
export function replyLineClamp(lines: ReplyLines): number {
  return lines === 0 ? 0 : Math.max(1, lines);
}
