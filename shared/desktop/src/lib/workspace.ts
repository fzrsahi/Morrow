// #462 — the Workspace tab's settings and display rules. The git work happens
// in the shell; this file only decides what the snapshot may show and which of
// the five macOS keys this shell actually obeys.

import type { Theme } from './tokens';
import type { WorkspaceStatus } from './types';

/** The five keys Swift's `WorkspaceSettingsPage` writes, unchanged. */
export const WORKSPACE_KEYS = {
  enabled: 'workspaceEnabled',
  hudTheme: 'workspaceHUDTheme',
  syncAlbumArt: 'syncThemeWithAlbumArt',
  showProjectName: 'showProjectName',
  showGitBranch: 'showGitBranch',
} as const;

export type WorkspaceHudTheme = 'modern' | 'pixelRetro';
export type WorkspaceSettings = {
  enabled: boolean;
  hudTheme: WorkspaceHudTheme;
  syncAlbumArt: boolean;
  showProjectName: boolean;
  showGitBranch: boolean;
};

export const DEFAULT_WORKSPACE: WorkspaceSettings = {
  enabled: true,
  hudTheme: 'modern',
  syncAlbumArt: false,
  showProjectName: true,
  showGitBranch: true,
};

const HUD_THEMES: readonly WorkspaceHudTheme[] = ['modern', 'pixelRetro'];

function flag(value: unknown, fallback: boolean): boolean {
  return typeof value === 'boolean' ? value : fallback;
}

/** Malformed or missing values fall back to Swift's defaults, never to `true`
 *  for a switch a user turned off. */
export function workspaceFromSettings(value: unknown): WorkspaceSettings {
  const stored =
    value !== null && typeof value === 'object' && !Array.isArray(value)
      ? (value as Record<string, unknown>)
      : {};
  const theme = stored[WORKSPACE_KEYS.hudTheme];
  return {
    enabled: flag(stored[WORKSPACE_KEYS.enabled], DEFAULT_WORKSPACE.enabled),
    hudTheme: HUD_THEMES.includes(theme as WorkspaceHudTheme)
      ? (theme as WorkspaceHudTheme)
      : DEFAULT_WORKSPACE.hudTheme,
    syncAlbumArt: flag(stored[WORKSPACE_KEYS.syncAlbumArt], DEFAULT_WORKSPACE.syncAlbumArt),
    showProjectName: flag(
      stored[WORKSPACE_KEYS.showProjectName],
      DEFAULT_WORKSPACE.showProjectName,
    ),
    showGitBranch: flag(stored[WORKSPACE_KEYS.showGitBranch], DEFAULT_WORKSPACE.showGitBranch),
  };
}

/** Update one of the four live keys. Writes go to the settings file through
 *  `settings_set`; this only keeps the local page honest after a click. The
 *  album-art key has no switch on this shell, so it is not handled here. */
export function applyWorkspaceSetting(
  settings: WorkspaceSettings,
  key: string,
  value: unknown,
): WorkspaceSettings {
  if (key === WORKSPACE_KEYS.enabled && typeof value === 'boolean') {
    return { ...settings, enabled: value };
  }
  if (key === WORKSPACE_KEYS.hudTheme) {
    return {
      ...settings,
      hudTheme: value === 'pixelRetro' ? 'pixelRetro' : 'modern',
    };
  }
  if (key === WORKSPACE_KEYS.showProjectName && typeof value === 'boolean') {
    return { ...settings, showProjectName: value };
  }
  if (key === WORKSPACE_KEYS.showGitBranch && typeof value === 'boolean') {
    return { ...settings, showGitBranch: value };
  }
  return settings;
}

/** A workspace panel can choose its own 16-bit skin, as CyberWorkspaceView
 *  does: a global Pixel theme wins, otherwise this key decides. */
export function workspaceTheme(global: Theme, settings: WorkspaceSettings): Theme {
  return global === 'pixel' || settings.hudTheme === 'pixelRetro' ? 'pixel' : 'modern';
}

export function visibleProject(status: WorkspaceStatus, settings: WorkspaceSettings): string {
  return settings.showProjectName ? status.project : '';
}

export function visibleBranch(status: WorkspaceStatus, settings: WorkspaceSettings): string {
  return settings.showGitBranch ? status.branch : '';
}

/** One number for the badge, summed from the three real git counts. */
export function changeCount(status: WorkspaceStatus): number {
  return status.staged + status.modified + status.untracked;
}

export type WorkspaceState =
  | { kind: 'loading' }
  | { kind: 'error'; message: string }
  | { kind: 'no-session' }
  | { kind: 'not-repo' }
  | { kind: 'ready'; status: WorkspaceStatus };

/** Every state the deck can draw, in one place so it cannot render a stale
 *  snapshot under an error or present an empty repo as a failed read. */
export function workspaceState(
  status: WorkspaceStatus | null,
  loading: boolean,
  error: string | null,
  hasSession: boolean,
): WorkspaceState {
  if (loading) return { kind: 'loading' };
  if (error) return { kind: 'error', message: error };
  if (!hasSession) return { kind: 'no-session' };
  if (!status) return { kind: 'not-repo' };
  return { kind: 'ready', status };
}
