// Mirrors the DTOs in src-tauri/src/feed.rs and src-tauri/src/hooks.rs.
// Render and input only: all decisions stay in the shell.

export type Session = {
  session_id: string;
  source: string;
  last_event: string;
  cwd_name: string;
  updated_at: number;
};

export type Card = {
  id: number;
  session_id: string;
  source: string;
  tool_name: string;
  summary: string;
};

export type Snapshot = { sessions: Session[]; permissions: Card[] };
export type Decision = 'allow' | 'deny';

export type HooksStatus = {
  codeisland_dir: string;
  script_installed: boolean;
  bridge_installed: boolean;
  hooks_installed: boolean;
  settings: string;
  settings_exists: boolean;
};

export type TargetStatus = {
  source: string;
  name: string;
  detected: boolean;
  installed: boolean;
  path: string;
};

export type OpencodeStatus = {
  detected: boolean;
  plugin_installed: boolean;
  registered: boolean;
  config: string;
};

export type KimiStatus = { detected: boolean; installed: boolean; config: string };

export type YamlStatus = {
  source: string;
  name: string;
  detected: boolean;
  installed: boolean;
  config: string;
  refusal: string | null;
};

export type HooksSnapshot = {
  status: HooksStatus;
  targets: TargetStatus[];
  opencode: OpencodeStatus;
  kimi: KimiStatus;
  yaml: YamlStatus[];
  version: string | null;
};

export type CapabilityRow = {
  feature: string;
  capability: { Supported: null } | { Degraded: string } | { Unsupported: string };
};

export type AutostartStatus = { installed: boolean; path: string };
