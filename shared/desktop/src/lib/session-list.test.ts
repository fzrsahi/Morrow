import { describe, expect, test } from 'bun:test';

import {
  groupSessions,
  orderSessions,
  splitOverflow,
  VISIBLE_SESSION_LIMIT,
} from './session-list';
import type { CardStatus, Session } from './types';

function session(id: string, status: CardStatus, source: string, updatedAt: number): Session {
  return {
    session_id: id,
    source,
    last_event: 'PreToolUse',
    cwd_name: id,
    updated_at: updatedAt,
    status,
    current_tool: '',
    tool_description: '',
    last_user_prompt: '',
    last_assistant_message: '',
    model: '',
    cwd: `/home/u/${id}`,
    term_app: 'kitty',
    tty_path: '/dev/pts/3',
    cli_pid: 1000,
  };
}

const IDS = (rows: Session[]) => rows.map((row) => row.session_id);

describe('session list ordering', () => {
  test('tier order wins, newest first inside a tier, id breaks ties', () => {
    const rows = [
      session('old-done', 'done', 'claude', 100),
      session('slow', 'working', 'codex', 10),
      session('new-done', 'idle', 'cursor', 200),
      session('needs', 'needs_you', 'mimo', 1),
      session('fast', 'thinking', 'claude', 30),
      session('tie-b', 'working', 'claude', 30),
    ];
    expect(IDS(orderSessions(rows))).toEqual([
      'needs',
      'fast',
      'tie-b',
      'slow',
      'new-done',
      'old-done',
    ]);
    expect(IDS(rows)).toEqual([
      'old-done',
      'slow',
      'new-done',
      'needs',
      'fast',
      'tie-b',
    ]);
  });
});

describe('session list grouping', () => {
  const rows = [
    session('needs', 'needs_you', 'claude', 1),
    session('work', 'working', 'codex', 2),
    session('think', 'thinking', 'codex', 3),
    session('done', 'done', 'claude', 4),
    session('idle', 'idle', 'cursor', 5),
    session('err', 'error', 'cursor', 6),
  ];

  test('ALL is one tier-ordered group with no header', () => {
    const groups = groupSessions(rows, 'all');
    expect(groups).toHaveLength(1);
    expect(groups[0].label).toBeNull();
    expect(IDS(groups[0].sessions)).toEqual(IDS(orderSessions(rows)));
  });

  test('STA groups by status, thinking under WORKING, empty groups omitted', () => {
    const groups = groupSessions(rows, 'sta');
    expect(groups.map((group) => group.label)).toEqual([
      'NEEDS YOU',
      'WORKING',
      'DONE',
      'IDLE',
      'ERROR',
    ]);
    expect(groups.map((group) => group.sessions.length)).toEqual([1, 2, 1, 1, 1]);
    expect(IDS(groups[1].sessions)).toEqual(['think', 'work']);
  });

  test('AGT groups by source in first-appearance order', () => {
    const groups = groupSessions(rows, 'agt');
    expect(groups.map((group) => group.label)).toEqual(['CLAUDE', 'CODEX', 'CURSOR']);
    expect(IDS(groups[1].sessions)).toEqual(['think', 'work']);
  });

  test('an empty list has no groups', () => {
    expect(groupSessions([], 'sta')).toEqual([]);
    expect(groupSessions([], 'agt')).toEqual([]);
  });
});

describe('session list overflow', () => {
  test('a short list has no remainder', () => {
    const rows = [session('a', 'working', 'claude', 1), session('b', 'done', 'claude', 2)];
    expect(splitOverflow(rows, 4)).toEqual({ shown: rows, remaining: 0 });
  });

  test('a long list shows the first rows and counts the rest', () => {
    const rows = Array.from({ length: 7 }, (_, i) => session(String(i), 'working', 'claude', i));
    const result = splitOverflow(rows);
    expect(result.shown).toHaveLength(VISIBLE_SESSION_LIMIT);
    expect(IDS(result.shown)).toEqual(['0', '1', '2', '3']);
    expect(result.remaining).toBe(3);
  });

  test('the limit is configurable for comfortable cards', () => {
    const rows = Array.from({ length: 5 }, (_, i) => session(String(i), 'working', 'claude', i));
    expect(splitOverflow(rows, 2).remaining).toBe(3);
  });
});
