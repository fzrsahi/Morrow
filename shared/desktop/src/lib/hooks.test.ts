import { describe, expect, test } from 'bun:test';
import {
  configRootCommand,
  configRootLabel,
  coverage,
  extraDirLabel,
  extraDirsFor,
  socketLabel,
} from './hooks';
import type {
  ConfigRootStatus,
  ExtraConfigDirStatus,
  HooksSnapshot,
  SocketHealth,
} from './types';

const socket = (health: Partial<SocketHealth> = {}): SocketHealth => ({
  endpoint: '/tmp/morrow-island.sock',
  listening: true,
  detail: '',
  ...health,
});

const root = (fields: Partial<ConfigRootStatus> = {}): ConfigRootStatus => ({
  cli: 'claude',
  name: 'Claude Code',
  path: '/home/dev/.claude',
  ready: true,
  installed: false,
  ...fields,
});

const extra = (fields: Partial<ExtraConfigDirStatus> = {}): ExtraConfigDirStatus => ({
  id: 'claude:/home/dev/.claude-work',
  cli: 'claude',
  name: 'Claude Code',
  path: '/home/dev/.claude-work',
  enabled: true,
  ready: true,
  installed: false,
  problem: null,
  ...fields,
});

const snapshot = (fields: Partial<HooksSnapshot> = {}): HooksSnapshot => ({
  status: {
    codeisland_dir: '/home/dev/.codeisland',
    script_installed: false,
    bridge_installed: false,
    hooks_installed: false,
    settings: '/home/dev/.claude/settings.json',
    settings_exists: false,
  },
  targets: [],
  opencode: { detected: false, plugin_installed: false, registered: false, config: '' },
  kimi: { detected: false, installed: false, config: '' },
  yaml: [],
  socket: socket(),
  config_roots: [],
  extra_dirs: [],
  version: null,
  note: null,
  ...fields,
});

describe('socket health', () => {
  test('a listening endpoint is named without an error', () => {
    expect(socketLabel(socket())).toBe('Listening at /tmp/morrow-island.sock');
  });

  test('a failed probe says why and never claims the hooks work', () => {
    const label = socketLabel(socket({ listening: false, detail: 'another user owns the path' }));
    expect(label).toBe('Not reachable: another user owns the path');
    expect(label).not.toContain('Listening');
  });

  test('an empty detail is still a failure, not a blank row', () => {
    expect(socketLabel(socket({ listening: false }))).toBe('Not reachable: unknown reason');
  });
});

describe('config roots', () => {
  test('a missing root is not reported as merely uninstalled', () => {
    expect(configRootLabel(root({ ready: false }))).toBe('Not detected');
    expect(configRootLabel(root())).toBe('Not installed');
    expect(configRootLabel(root({ installed: true }))).toBe('Installed');
  });

  test('the button command follows the installed state', () => {
    expect(configRootCommand(root())).toBe('config_root_install');
    expect(configRootCommand(root({ installed: true }))).toBe('config_root_uninstall');
  });
});

describe('extra config dirs', () => {
  test('a problem replaces the install label', () => {
    expect(extraDirLabel(extra())).toBe('Not installed');
    expect(extraDirLabel(extra({ installed: true }))).toBe('Installed');
    expect(extraDirLabel(extra({ problem: "This is the CLI's primary config root" }))).toBe(
      "This is the CLI's primary config root",
    );
  });

  test('extras are listed under their own CLI', () => {
    const dirs = [extra(), extra({ cli: 'codex', id: 'codex:/work' }), extra({ id: 'claude:/two' })];
    expect(extraDirsFor(dirs, 'claude').map((dir) => dir.id)).toEqual([
      'claude:/home/dev/.claude-work',
      'claude:/two',
    ]);
    expect(extraDirsFor(dirs, 'grok')).toEqual([]);
  });
});

describe('coverage', () => {
  test('counts the same rows the page draws, including extras being out of scope', () => {
    expect(coverage(snapshot())).toEqual({ installed: 0, detected: 0 });
    expect(
      coverage(
        snapshot({
          targets: [{ source: 'codex', name: 'Codex', detected: true, installed: true, path: '' }],
          config_roots: [root({ installed: true })],
          extra_dirs: [extra({ installed: true })],
        }),
      ),
    ).toEqual({ installed: 1, detected: 1 });
  });
});
