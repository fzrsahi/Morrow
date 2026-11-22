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
  model: string;
  /** Full cwd; `cwd_name` is only its last component. */
  cwd: string;
  term_app: string;
  tty_path: string;
  cli_pid: number | null;
};

export type Card = {
  id: number;
  session_id: string;
  source: string;
  tool_name: string;
  summary: string;
};

/** Inline spans are parsed by `morrow_core::markdown`; the panel never parses
 *  Markdown itself. `spans` nests for `strong` / `emphasis`. */
export type InlineSpan = {
  kind: 'text' | 'code' | 'link' | 'strong' | 'emphasis';
  text: string;
  url?: string;
  spans?: InlineSpan[];
};

export type MarkdownListItem = {
  checkbox: 'unchecked' | 'checked' | null;
  blocks: MarkdownBlock[];
};

/** Mirrors the serialized `morrow_core::markdown::MarkdownBlock`. */
export type MarkdownBlock =
  | { kind: 'heading'; level: number; spans: InlineSpan[] }
  | { kind: 'paragraph'; spans: InlineSpan[] }
  | { kind: 'list'; ordered: boolean; start: number; items: MarkdownListItem[] }
  | { kind: 'code'; language: string | null; code: string; closed: boolean }
  | {
      kind: 'table';
      header: InlineSpan[][];
      alignments: ('automatic' | 'leading' | 'center' | 'trailing')[];
      rows: InlineSpan[][][];
    }
  | { kind: 'quote'; blocks: MarkdownBlock[] }
  | { kind: 'thematic_break' };

/** `AttentionDecision.completion` settled for the panel (style + Smart Suppress). */
export type Completion = {
  session_id: string;
  updated_at: number;
  turn_failed: boolean;
  presentation: 'status_only' | 'glance' | 'card';
  blocks: MarkdownBlock[];
};

export type Snapshot = {
  sessions: Session[];
  permissions: Card[];
  completion: Completion | null;
};
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
