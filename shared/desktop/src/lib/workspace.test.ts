import { describe, expect, test } from 'bun:test';
import type { WorkspaceStatus } from './types';
import {
  DEFAULT_WORKSPACE,
  WORKSPACE_KEYS,
  applyWorkspaceSetting,
  changeCount,
  visibleBranch,
  visibleProject,
  workspaceFromSettings,
  workspaceState,
  workspaceTheme,
} from './workspace';

const status: WorkspaceStatus = {
  cwd: '/work/morrow',
  project: 'morrow',
  branch: 'feat/workspace',
  isWorktree: false,
  staged: 2,
  modified: 3,
  untracked: 1,
  commits: [{ hash: 'a1b2c3d', subject: 'fix: keep status' }],
};

describe('workspace settings', () => {
  test('reads the same five keys Swift writes', () => {
    expect(Object.values(WORKSPACE_KEYS)).toEqual([
      'workspaceEnabled',
      'workspaceHUDTheme',
      'syncThemeWithAlbumArt',
      'showProjectName',
      'showGitBranch',
    ]);
    expect(workspaceFromSettings({})).toEqual(DEFAULT_WORKSPACE);
  });

  test('honours an explicit false and a valid HUD theme', () => {
    const got = workspaceFromSettings({
      [WORKSPACE_KEYS.enabled]: false,
      [WORKSPACE_KEYS.hudTheme]: 'pixelRetro',
      [WORKSPACE_KEYS.syncAlbumArt]: true,
      [WORKSPACE_KEYS.showProjectName]: false,
      [WORKSPACE_KEYS.showGitBranch]: false,
    });
    expect(got).toEqual({
      enabled: false,
      hudTheme: 'pixelRetro',
      syncAlbumArt: true,
      showProjectName: false,
      showGitBranch: false,
    });
  });

  test('a click updates its own field and nothing else', () => {
    const next = applyWorkspaceSetting(
      DEFAULT_WORKSPACE,
      WORKSPACE_KEYS.showGitBranch,
      false,
    );
    expect(next).toEqual({ ...DEFAULT_WORKSPACE, showGitBranch: false });
    expect(
      applyWorkspaceSetting(DEFAULT_WORKSPACE, WORKSPACE_KEYS.hudTheme, 'pixelRetro').hudTheme,
    ).toBe('pixelRetro');
    expect(applyWorkspaceSetting(DEFAULT_WORKSPACE, 'unknown', true)).toBe(DEFAULT_WORKSPACE);
  });

  test('malformed values fall back rather than becoming true', () => {
    const got = workspaceFromSettings({
      [WORKSPACE_KEYS.enabled]: 'yes',
      [WORKSPACE_KEYS.hudTheme]: 'neon',
      [WORKSPACE_KEYS.showGitBranch]: 1,
    });
    expect(got).toEqual(DEFAULT_WORKSPACE);
  });

  test('global Pixel wins over a Modern HUD choice', () => {
    expect(workspaceTheme('pixel', DEFAULT_WORKSPACE)).toBe('pixel');
    expect(workspaceTheme('modern', DEFAULT_WORKSPACE)).toBe('modern');
    expect(workspaceTheme('modern', { ...DEFAULT_WORKSPACE, hudTheme: 'pixelRetro' })).toBe(
      'pixel',
    );
  });
});

describe('workspace display', () => {
  test('the two visibility switches hide only their own row', () => {
    expect(visibleProject(status, DEFAULT_WORKSPACE)).toBe('morrow');
    expect(visibleBranch(status, DEFAULT_WORKSPACE)).toBe('feat/workspace');
    const hidden = { ...DEFAULT_WORKSPACE, showProjectName: false, showGitBranch: false };
    expect(visibleProject(status, hidden)).toBe('');
    expect(visibleBranch(status, hidden)).toBe('');
  });

  test('the badge counts real rows, not commits', () => {
    expect(changeCount(status)).toBe(6);
    expect(changeCount({ ...status, staged: 0, modified: 0, untracked: 0 })).toBe(0);
  });

  test('state has a distinct answer for loading, error, empty and ready', () => {
    expect(workspaceState(status, true, null, true)).toEqual({ kind: 'loading' });
    expect(workspaceState(status, false, 'git failed', true)).toEqual({
      kind: 'error',
      message: 'git failed',
    });
    expect(workspaceState(null, false, null, false)).toEqual({ kind: 'no-session' });
    expect(workspaceState(null, false, null, true)).toEqual({ kind: 'not-repo' });
    expect(workspaceState(status, false, null, true)).toEqual({ kind: 'ready', status });
  });

  test('a stale snapshot is not shown under a newer error', () => {
    expect(workspaceState(status, false, 'git failed', true).kind).toBe('error');
  });
});
