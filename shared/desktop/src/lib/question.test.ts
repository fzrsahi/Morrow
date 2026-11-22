// #188 — pure seams for the single-choice Question card. The key rules are
// ported from the macOS AppState golden tests; the reply bytes themselves are
// built in Rust (permissions.rs) so the bridge writes exactly Swift's JSON.

import { describe, expect, test } from 'bun:test';
import { isQuestion, pickIndex } from './question';

describe('number keys (#question)', () => {
  test('1–9 pick the matching option and everything else is ignored', () => {
    expect(pickIndex('1')).toBe(0);
    expect(pickIndex('9')).toBe(8);
    for (const key of ['0', 'a', '', '10', 'Enter', '1x']) {
      expect(pickIndex(key)).toBeNull();
    }
  });
});

// The answers-payload golden (question text as key, `_2` dedup, header
// ignored) is ported from Swift in Rust's `permissions.rs` tests, where the
// hook reply bytes are actually built.

describe('card shape', () => {
  test('only cards carrying a question are question cards', () => {
    const base = {
      id: 1,
      session_id: 's',
      source: 'claude',
      tool_name: 'Bash',
      summary: 'x',
      tool_input: null,
    };
    expect(isQuestion({ ...base, question: null })).toBe(false);
    expect(
      isQuestion({
        ...base,
        question: {
          question: 'Pick one',
          header: '',
          from_permission: true,
          options: [{ label: 'A', description: '' }],
        },
      }),
    ).toBe(true);
  });
});
