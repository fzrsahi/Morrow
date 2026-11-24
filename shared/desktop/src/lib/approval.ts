// #186 — pure view rules for the approval card. Ported from macOS
// `ApprovalCopy` (NotchPanelView.swift:4231-4326) and the English
// `approval_title_*` table (L10n.swift:617-626). The shell sends the tool name
// and its raw input; the card never shows those words verbatim.

import type { ToolInput } from './types';

export type ApprovalAction =
  | 'command'
  | 'change'
  | 'read'
  | 'search'
  | 'web'
  | 'mcp'
  | 'agent'
  | 'generic';

/** L10n.swift:617-626, English. Positional markers are kept as written there. */
export const APPROVAL_TITLE = {
  command: '%@ wants to run a command',
  change_file: '%1$@ wants to change %2$@',
  change: '%@ wants to change files',
  read_file: '%1$@ wants to read %2$@',
  read: '%@ wants to read files',
  search: '%@ wants to search the files',
  web: '%@ wants to access the web',
  mcp: '%1$@ wants to call %2$@',
  agent: '%@ wants to start a subagent',
  generic: '%@ wants to run',
} as const;

/** How long we keep the "answered elsewhere" card before dropping it. */
export const GONE_NOTICE_MS = 1500;
export const GONE_NOTICE =
  'Answered in the terminal. This request is no longer waiting.';

/** #approval-queue: Ctrl+Alt+… on Windows and Linux, not macOS' ⌥⌘. */
export const SHORTCUTS = {
  hide: 'Esc',
  deny: 'Ctrl+Alt+D',
  allow: 'Ctrl+Alt+A',
} as const;

export function shortcutHint(key: string, hotkeySupported: boolean): string | null {
  return hotkeySupported ? key : null;
}

export function queueLabel(position: number, total: number): string | null {
  return total > 1 ? `${position} of ${total}` : null;
}

/** `mcp__server__tool`, split; null for anything else. */
export function mcpParts(tool: string): { server: string; tool: string } | null {
  if (!tool.startsWith('mcp__')) return null;
  const rest = tool.slice('mcp__'.length);
  const separator = rest.indexOf('__');
  if (separator < 0) return null;
  const server = rest.slice(0, separator);
  const name = rest.slice(separator + 2);
  return server && name ? { server, tool: name } : null;
}

/** ToolNameDisplay.compact(tool, 32) — trim, then elide the middle. */
export function displayName(tool: string): string {
  const trimmed = tool.trim();
  if (trimmed.length <= 32) return trimmed;
  const suffixBudget = 18;
  const prefixCount = 32 - suffixBudget - 3;
  return `${trimmed.slice(0, prefixCount)}...${trimmed.slice(-suffixBudget)}`;
}

export function approvalAction(tool: string): ApprovalAction {
  const parts = mcpParts(tool);
  if (parts) return 'mcp';
  const name = tool.toLowerCase().replace(/-/g, '_');
  if (name === 'task' || name === 'agent' || name.includes('subagent') || name.includes('spawn_agent')) {
    return 'agent';
  }
  if (
    name.includes('webfetch') ||
    name.includes('websearch') ||
    name.includes('web_fetch') ||
    name.includes('web_search') ||
    name.includes('fetch_url')
  ) {
    return 'web';
  }
  // Before "change": Codex's write_stdin feeds a running command.
  if (
    name === 'bash' ||
    name === 'command' ||
    name.includes('shell') ||
    name.includes('exec_command') ||
    name.includes('write_stdin') ||
    name.includes('run_command') ||
    name.includes('execute_command') ||
    name.includes('terminal')
  ) {
    return 'command';
  }
  if (
    name.includes('edit') ||
    name.includes('write') ||
    name.includes('apply_patch') ||
    name.includes('replace') ||
    name.includes('create_file') ||
    name.includes('delete_file') ||
    name.includes('move_file')
  ) {
    return 'change';
  }
  if (
    name === 'read' ||
    name.includes('read_file') ||
    name.includes('view_file') ||
    name.includes('list_dir')
  ) {
    return 'read';
  }
  if (name === 'grep' || name === 'glob' || name === 'find' || name.includes('search')) {
    return 'search';
  }
  return 'generic';
}

function textField(input: ToolInput, keys: string[]): string | null {
  if (!input) return null;
  for (const key of keys) {
    const value = input[key];
    if (typeof value === 'string' && value) return value;
  }
  return null;
}

function fileName(input: ToolInput): string | null {
  const path = textField(input, ['file_path', 'notebook_path', 'path']);
  if (!path) return null;
  const name = path.split(/[\\/]/).pop();
  return name || null;
}

export function approvalTitle(tool: string, input: ToolInput): string {
  const name = displayName(tool);
  const format = (copy: string, ...args: string[]) =>
    copy.replace('%@', args[0]).replace('%1$@', args[0]).replace('%2$@', args[1] ?? '');
  switch (approvalAction(tool)) {
    case 'command':
      return format(APPROVAL_TITLE.command, name);
    case 'change': {
      const file = fileName(input);
      return file
        ? format(APPROVAL_TITLE.change_file, name, file)
        : format(APPROVAL_TITLE.change, name);
    }
    case 'read': {
      const file = fileName(input);
      return file
        ? format(APPROVAL_TITLE.read_file, name, file)
        : format(APPROVAL_TITLE.read, name);
    }
    case 'search':
      return format(APPROVAL_TITLE.search, name);
    case 'web':
      return format(APPROVAL_TITLE.web, name);
    case 'mcp': {
      const parts = mcpParts(tool) ?? { server: tool, tool };
      return format(APPROVAL_TITLE.mcp, parts.server, parts.tool);
    }
    case 'agent':
      return format(APPROVAL_TITLE.agent, name);
    case 'generic':
      return format(APPROVAL_TITLE.generic, name);
  }
}

export type BodyLine =
  | { kind: 'text'; text: string }
  | { kind: 'comment' | 'add' | 'rem'; text: string }
  | { kind: 'kv'; key: string; value: string };

export type ApprovalBody = {
  kind: 'command' | 'diff' | 'mcp' | 'input';
  server?: string;
  lines: BodyLine[];
};

function linesOf(value: string | null): string[] {
  return value ? value.split('\n') : [];
}

/** The code / diff / MCP key-value block under the title. */
export function approvalBody(tool: string, input: ToolInput, fallback: string): ApprovalBody {
  const action = approvalAction(tool);
  const fields = input && typeof input === 'object' ? input : null;
  const value = (key: string): string | null => {
    const found = fields?.[key];
    if (found === undefined || found === null) return null;
    return typeof found === 'string' ? found : JSON.stringify(found);
  };

  if (action === 'command') {
    const command = value('command');
    return { kind: 'command', lines: [{ kind: 'text', text: `$ ${command ?? fallback}` }] };
  }

  if (action === 'change') {
    const lines: BodyLine[] = [];
    const path = textField(input, ['file_path', 'notebook_path', 'path']);
    if (path) lines.push({ kind: 'comment', text: path });
    for (const old of linesOf(value('old_string'))) lines.push({ kind: 'rem', text: `- ${old}` });
    const added = value('new_string') ?? value('content');
    for (const line of linesOf(added)) lines.push({ kind: 'add', text: `+ ${line}` });
    if (!lines.length && fallback) lines.push({ kind: 'text', text: fallback });
    return { kind: 'diff', lines };
  }

  if (action === 'mcp') {
    const lines: BodyLine[] = Object.keys(fields ?? {})
      .sort()
      .map((key) => ({ kind: 'kv' as const, key, value: value(key) ?? '' }));
    return { kind: 'mcp', server: mcpParts(tool)?.server, lines };
  }

  const detail = textField(input, ['file_path', 'path', 'pattern', 'query', 'url']) ?? fallback;
  return { kind: 'input', lines: detail ? [{ kind: 'text', text: detail }] : [] };
}
