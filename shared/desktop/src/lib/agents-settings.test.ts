import { describe, expect, test } from 'bun:test';

import {
  AGENT_KEYS,
  DEFAULT_AGENTS,
  agentsFromSettings,
  agentsTheme,
  replyLineClamp,
  sessionViewportLimit,
} from './agents-settings';

describe('Agents settings', () => {
  test('a fresh install uses the Swift defaults', () => {
    expect(agentsFromSettings(null)).toEqual(DEFAULT_AGENTS);
    expect(agentsFromSettings({})).toEqual(DEFAULT_AGENTS);
  });

  test('stored keys use the same raw spellings as macOS', () => {
    const settings = agentsFromSettings({
      agentsEnabled: false,
      maxVisibleSessions: 8,
      sessionListDensity: 'compact',
      contentFontSize: 14,
      aiMessageLines: 3,
      showAgentDetails: true,
      showToolStatus: false,
      showTaskProgress: false,
      showSessionRecap: false,
      showModelLabel: true,
      showUsageStats: false,
      showClaudeQuota: true,
      claudeQuotaChip: 'weeklyScoped',
      agentsHUDTheme: 'modern',
    });
    expect(settings).toEqual({
      agentsEnabled: false,
      maxVisibleSessions: 8,
      sessionListDensity: 'compact',
      contentFontSize: 14,
      aiMessageLines: 3,
      showAgentDetails: true,
      showToolStatus: false,
      showTaskProgress: false,
      showSessionRecap: false,
      showModelLabel: true,
      showUsageStats: false,
      showClaudeQuota: true,
      claudeQuotaChip: 'weeklyScoped',
      agentsHUDTheme: 'modern',
    });
  });

  test('an unsupported choice falls back instead of breaking the panel', () => {
    const settings = agentsFromSettings({
      maxVisibleSessions: 7,
      sessionListDensity: 'roomy',
      contentFontSize: 30,
      aiMessageLines: 4,
      claudeQuotaChip: 'daily',
      agentsHUDTheme: 'neon',
      showTaskProgress: 'yes',
    });
    expect(settings.maxVisibleSessions).toBe(5);
    expect(settings.sessionListDensity).toBe('comfortable');
    expect(settings.contentFontSize).toBe(11);
    expect(settings.aiMessageLines).toBe(1);
    expect(settings.claudeQuotaChip).toBe('auto');
    expect(settings.agentsHUDTheme).toBe('pixelRetro');
    expect(settings.showTaskProgress).toBe(true);
  });

  test('the row viewport keeps the canvas density until the user chooses a limit', () => {
    expect(sessionViewportLimit('compact', 5)).toBe(4);
    expect(sessionViewportLimit('comfortable', 5)).toBe(2);
    expect(sessionViewportLimit('compact', 8)).toBe(8);
    expect(sessionViewportLimit('comfortable', 3)).toBe(3);
    expect(sessionViewportLimit('compact', 99)).toBeGreaterThan(1_000_000);
  });

  test('the card theme maps to the two panel themes and zero lines means unlimited', () => {
    expect(agentsTheme('modern')).toBe('modern');
    expect(agentsTheme('pixelRetro')).toBe('pixel');
    expect(replyLineClamp(0)).toBe(0);
    expect(replyLineClamp(3)).toBe(3);
  });

  test('the keys are top-level settings document keys', () => {
    expect(AGENT_KEYS.contentFontSize).toBe('contentFontSize');
    expect(AGENT_KEYS.sessionListDensity).toBe('sessionListDensity');
    expect(AGENT_KEYS.agentsHUDTheme).toBe('agentsHUDTheme');
    for (const key of Object.values(AGENT_KEYS)) expect(key).not.toContain('.');
  });
});
