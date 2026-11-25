// The Hooks page's labels come from here so the shell's status and the text it
// draws cannot drift. The page still owns every install decision.

import type {
  ConfigRootStatus,
  ExtraConfigDirStatus,
  HooksSnapshot,
  SocketHealth,
} from './types';

/** Hooks installed into a CLI are silent when nothing listens at the endpoint. */
export function socketLabel(health: SocketHealth): string {
  return health.listening
    ? `Listening at ${health.endpoint}`
    : `Not reachable: ${health.detail || 'unknown reason'}`;
}

export function coverage(snapshot: HooksSnapshot): { installed: number; detected: number } {
  const rows = [
    ...snapshot.targets.map((row) => ({ detected: row.detected, installed: row.installed })),
    { detected: snapshot.opencode.detected, installed: snapshot.opencode.plugin_installed },
    { detected: snapshot.kimi.detected, installed: snapshot.kimi.installed },
    ...snapshot.yaml.map((row) => ({ detected: row.detected, installed: row.installed })),
  ];
  return {
    installed: rows.filter((row) => row.installed).length,
    detected: rows.filter((row) => row.detected).length,
  };
}

export function configRootLabel(row: ConfigRootStatus): string {
  if (!row.ready) return 'Not detected';
  return row.installed ? 'Installed' : 'Not installed';
}

export function extraDirLabel(row: ExtraConfigDirStatus): string {
  if (row.problem) return row.problem;
  return row.installed ? 'Installed' : 'Not installed';
}

/** Extras are listed under their CLI, in the order the shell saved them. */
export function extraDirsFor(dirs: ExtraConfigDirStatus[], cli: string): ExtraConfigDirStatus[] {
  return dirs.filter((dir) => dir.cli === cli);
}

/** The shell command a root's Install/Remove button calls. */
export function configRootCommand(row: ConfigRootStatus): 'config_root_install' | 'config_root_uninstall' {
  return row.installed ? 'config_root_uninstall' : 'config_root_install';
}
