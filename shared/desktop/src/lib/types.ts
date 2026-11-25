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
  /** `live_reply` folded into one line of running text (#238 AC 4). */
  reply_preview?: string;
  model: string;
  /** Full cwd; `cwd_name` is only its last component. */
  cwd: string;
  term_app: string;
  tty_path: string;
  cli_pid: number | null;
  /** #187 — remote sessions have no local window; the jump icon is hidden. */
  is_remote: boolean;
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

/** #188 — one OptionRow of a Question card. */
export type QuestionOption = { label: string; description: string };

/** #190 — one step of a multi-question request. */
export type QuestionPrompt = {
  question: string;
  header: string;
  options: QuestionOption[];
  multi_select: boolean;
};

/** #188 — the question a card draws (AskUserQuestion or Notification). */
export type QuestionCard = {
  question: string;
  header: string;
  options: QuestionOption[];
  /** AskUserQuestion replies need `updatedInput.answers`; Notification does not. */
  from_permission: boolean;
  /** #190: true when this question accepts more than one pick. */
  multi_select: boolean;
  /** #190: every question in one request; empty for a single question. */
  prompts: QuestionPrompt[];
};

/** #462 — mirrors `WorkspaceStatus` in src-tauri/src/workspace.rs. Counts come
 *  from `git status --porcelain=v1`, commits from `git log`. */
export type GitCommit = { hash: string; subject: string };
export type WorkspaceStatus = {
  cwd: string;
  project: string;
  branch: string;
  isWorktree: boolean;
  staged: number;
  modified: number;
  untracked: number;
  commits: GitCommit[];
};

export type Card = {
  id: number;
  session_id: string;
  source: string;
  tool_name: string;
  summary: string;
  tool_input: ToolInput;
  /** #188: null on an approval card. */
  question: QuestionCard | null;
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

/** One row of the prayer card's expanded day list. */
export type PrayerTime = {
  key: string;
  name: string;
  /** Local `HH:MM` at the selected city's offset. */
  time: string;
  epoch_seconds: number;
};

/** #307 — mirrors `PrayerSnapshot` in src-tauri/src/prayer.rs. */
export type PrayerSnapshot = {
  city_id: string;
  city_name: string;
  method: string;
  correction_minutes: number;
  /** #462 — `compact` or `panoramic`, from `prayerCardDisplayMode`. */
  card_mode: string;
  hijri: string;
  times: PrayerTime[];
  next: PrayerTime | null;
  in_minutes: number;
  urgent: boolean;
};

export type Snapshot = {
  sessions: Session[];
  permissions: Card[];
  completion: Completion | null;
  /** Cards whose follow-up reminder already went out (#238 AC 9). */
  reminder_ids?: number[];
  /** #307 — absent until a prayer setting is saved; never fabricated. */
  prayer?: PrayerSnapshot | null;
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

/** Mirrors `SshHost` in src-tauri/src/remote.rs (morrow_core::remote::host). */
export type RemoteHost = {
  id: string;
  name: string;
  host: string;
  user: string;
  port: number | null;
  identity_file: string;
  auto_connect: boolean;
  auth_socket: string;
  cwd_filter: string;
  agents_run_as_other_users: boolean;
};

/** Mirrors the tagged `RemoteStatus` serialized by src-tauri/src/remote.rs. */
export type RemoteStatus =
  | { state: 'disconnected' }
  | { state: 'connecting' }
  | { state: 'connected' }
  | { state: 'failed'; message: string };

/** #241 — one row of the Sound page, from the shell's cue table. */
export type SoundCue = { name: string; file: string };
