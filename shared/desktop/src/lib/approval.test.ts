// #186 — pure view rules for the approval card. These are the seams the
// component consumes: the macOS copy table, the tool → title/body mapping,
// the queue label, the shortcut hints and the "answered elsewhere" notice.

import { describe, expect, test } from 'bun:test';
import {
  APPROVAL_TITLE,
  GONE_NOTICE,
  GONE_NOTICE_MS,
  SHORTCUTS,
  approvalAction,
  approvalBody,
  approvalTitle,
  mcpParts,
  queueLabel,
  shortcutHint,
} from './approval';

describe('the macOS copy table (#approval)', () => {
  test('is the exact English approval_title_* table from L10n.swift', () => {
    expect(APPROVAL_TITLE).toEqual({
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
    });
  });
});

describe('tool name and input become the title, not the raw name', () => {
  test('commands, including the Codex aliases', () => {
    expect(approvalAction('Bash')).toBe('command');
    expect(approvalAction('exec_command')).toBe('command');
    expect(approvalTitle('Bash', { command: 'npm run test' })).toBe(
      'Bash wants to run a command',
    );
    expect(approvalTitle('write_stdin', {})).toBe('write_stdin wants to run a command');
  });

  test('changes and reads name the file when the input has one', () => {
    expect(approvalTitle('Edit', { file_path: '/a/b/Dashboard.tsx' })).toBe(
      'Edit wants to change Dashboard.tsx',
    );
    expect(approvalTitle('Edit', {})).toBe('Edit wants to change files');
    expect(approvalTitle('Read', { path: '/a/b/notes.md' })).toBe(
      'Read wants to read notes.md',
    );
    expect(approvalTitle('Read', {})).toBe('Read wants to read files');
  });

  test('search, web and agent keep their own verbs', () => {
    expect(approvalTitle('Grep', { pattern: 'todo' })).toBe('Grep wants to search the files');
    expect(approvalTitle('WebFetch', { url: 'https://example.com' })).toBe(
      'WebFetch wants to access the web',
    );
    expect(approvalTitle('Task', {})).toBe('Task wants to start a subagent');
  });

  test('MCP splits mcp__server__tool into server and tool', () => {
    expect(mcpParts('mcp__github__create_pull_request')).toEqual({
      server: 'github',
      tool: 'create_pull_request',
    });
    expect(approvalAction('mcp__github__create_pull_request')).toBe('mcp');
    expect(approvalTitle('mcp__github__create_pull_request', {})).toBe(
      'github wants to call create_pull_request',
    );
  });

  test('an unknown tool uses the generic copy', () => {
    expect(approvalAction('SomeExoticTool')).toBe('generic');
    expect(approvalTitle('SomeExoticTool', {})).toBe('SomeExoticTool wants to run');
  });
});

describe('the body block (#approval)', () => {
  test('a command is shown with a prompt prefix', () => {
    expect(
      approvalBody('Bash', { command: 'npm run test -- --coverage' }, 'ignored'),
    ).toEqual({ kind: 'command', lines: [{ kind: 'text', text: '$ npm run test -- --coverage' }] });
  });

  test('an edit is a path plus removed and added lines', () => {
    expect(
      approvalBody(
        'Edit',
        {
          file_path: '/a/Dashboard.tsx',
          old_string: 'const a = 1;',
          new_string: 'const a = 2;\nconst b = 3;',
        },
        '',
      ),
    ).toEqual({
      kind: 'diff',
      lines: [
        { kind: 'comment', text: '/a/Dashboard.tsx' },
        { kind: 'rem', text: '- const a = 1;' },
        { kind: 'add', text: '+ const a = 2;' },
        { kind: 'add', text: '+ const b = 3;' },
      ],
    });
  });

  test('an MCP call is a sorted key/value block', () => {
    expect(
      approvalBody(
        'mcp__github__create_pull_request',
        { head: 'feat/dashboard', base: 'main', body: 'Adds filters', owner: 'acme' },
        '',
      ),
    ).toEqual({
      kind: 'mcp',
      server: 'github',
      lines: [
        { kind: 'kv', key: 'base', value: 'main' },
        { kind: 'kv', key: 'body', value: 'Adds filters' },
        { kind: 'kv', key: 'head', value: 'feat/dashboard' },
        { kind: 'kv', key: 'owner', value: 'acme' },
      ],
    });
  });

  test('a read with no file falls back to the one-line summary', () => {
    expect(approvalBody('Read', {}, 'README.md')).toEqual({
      kind: 'input',
      lines: [{ kind: 'text', text: 'README.md' }],
    });
  });
});

describe('queue and shortcut hints (#approval-queue)', () => {
  test('the queue label appears only when more than one request waits', () => {
    expect(queueLabel(1, 3)).toBe('1 of 3');
    expect(queueLabel(2, 3)).toBe('2 of 3');
    expect(queueLabel(1, 1)).toBeNull();
    expect(queueLabel(1, 0)).toBeNull();
  });

  test('hints use Ctrl+Alt on Windows and Linux and vanish without the hotkey', () => {
    expect(SHORTCUTS).toEqual({ hide: 'Esc', deny: 'Ctrl+Alt+D', allow: 'Ctrl+Alt+A' });
    expect(shortcutHint(SHORTCUTS.hide, true)).toBe('Esc');
    expect(shortcutHint(SHORTCUTS.deny, true)).toBe('Ctrl+Alt+D');
    expect(shortcutHint(SHORTCUTS.allow, true)).toBe('Ctrl+Alt+A');
    expect(shortcutHint(SHORTCUTS.allow, false)).toBeNull();
    expect(shortcutHint(SHORTCUTS.deny, false)).toBeNull();
  });
});

describe('answered elsewhere (#approval-gone)', () => {
  test('keeps the canvas sentence and its 1.5 s lifetime', () => {
    expect(GONE_NOTICE).toBe('Answered in the terminal. This request is no longer waiting.');
    expect(GONE_NOTICE_MS).toBe(1500);
  });
});
