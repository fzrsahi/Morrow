// Mirrors the DTOs in src-tauri/src/feed.rs and src-tauri/src/hooks.rs.
// Render and input only: all decisions stay in the shell.

/** Mirrors `CardStatus` in src-tauri/src/sessions.rs (snake_case on the wire). */
export type CardStatus =
  | 'needs_you'
  | 'working'
  | 'thinking'
  | 'done'
  | 'idle'
  | 'stopped'
  | 'error';

export type Session = {
  session_id: string;
  source: string;
  last_event: string;
  cwd_name: string;
  updated_at: number;
  status: CardStatus;
  current_tool: string;
  tool_description: string;
  last_user_prompt: string;
  last_assistant_message: string;
  /** Newest text from the transcript tail; only the card's reply row shows it. */
  live_reply: string;
  model: string;
  /** Full cwd; `cwd_name` is only its last component. */
  cwd: string;
  term_app: string;
  tty_path: string;
  cli_pid: number | null;
  /** #184 — checked-out branch, empty when the cwd is not a git checkout. */
  git_branch: string;
  /** Linked worktree; the card appends ⧉ to the branch. */
  git_is_worktree: boolean;
  /** Agent's own checklist; `tasks_total === 0` hides the task row. */
  tasks_done: number;
  tasks_total: number;
  task_active: string;
  subagent_count: number;
  /** Idle-only turn recap, empty while the session is working. */
  recap: string;
};

/** The hook's `tool_input`, serialised by `serde_json::Value`. */
export type ToolInput = Record<string, unknown> | null;

export type Card = {
  id: number;
  session_id: string;
  source: string;
  tool_name: string;
  summary: string;
  tool_input: ToolInput;
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
  note: string | null;
};

export type CapabilityRow = {
  feature: string;
  capability: { Supported: null } | { Degraded: string } | { Unsupported: string };
};

export type AutostartStatus = { installed: boolean; path: string };
