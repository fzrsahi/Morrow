// #188 — pure seams for the single-choice Question card. The key rules are
// ported from the macOS AppState golden tests; the reply bytes themselves are
// built in Rust (permissions.rs) so the bridge writes exactly Swift's JSON.

import { describe, expect, test } from 'bun:test';
import {
  canSubmitTyped,
  isQuestion,
  pickIndex,
  questionInput,
  QuestionFocusGate,
} from './question';

describe('number keys (#question)', () => {
  test('1–9 pick the matching option and everything else is ignored', () => {
    expect(pickIndex('1')).toBe(0);
    expect(pickIndex('9')).toBe(8);
    for (const key of ['0', 'a', '', '10', 'Enter', '1x']) {
      expect(pickIndex(key)).toBeNull();
    }
  });
});

describe('typed answers (#question, #question-multi)', () => {
  const cardWith = (options: { label: string; description: string }[]) => ({
    id: 1,
    session_id: 's',
    source: 'claude',
    tool_name: 'AskUserQuestion',
    summary: 'x',
    tool_input: null,
    question: { question: 'Pick one', header: '', from_permission: true, options },
  });

  test('a question with no options is free text: the field shows at once', () => {
    expect(questionInput(cardWith([]))).toBe('free');
  });

  test('a question with options appends the Other row that reveals the field', () => {
    expect(questionInput(cardWith([{ label: 'A', description: '' }]))).toBe('other');
  });

  test('only typed text can be submitted, never an empty field', () => {
    expect(canSubmitTyped('')).toBe(false);
    expect(canSubmitTyped('Jotai, we already use it in admin')).toBe(true);
  });
});

describe('typed-field focus (#189)', () => {
  test('clicking a field asks the island for focus once per card', () => {
    const gate = new QuestionFocusGate();
    expect(gate.focus(7)).toEqual({ open: true });
    expect(gate.focus(7)).toBeNull();
  });

  test('closing the owning card gives focus back once', () => {
    const gate = new QuestionFocusGate();
    gate.focus(7);
    expect(gate.release()).toEqual({ open: false });
    expect(gate.release()).toBeNull();
  });

  test('a field re-asks for focus after its card closed', () => {
    const gate = new QuestionFocusGate();
    gate.focus(7);
    gate.release();
    expect(gate.focus(7)).toEqual({ open: true });
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
