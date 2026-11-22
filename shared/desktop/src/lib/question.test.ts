// #188 — pure seams for the single-choice Question card. The key rules are
// ported from the macOS AppState golden tests; the reply bytes themselves are
// built in Rust (permissions.rs) so the bridge writes exactly Swift's JSON.
// #190 — multi-select and the multi-question wizard (QuestionWizardState).

import { describe, expect, test } from 'bun:test';
import {
  answersOf,
  canConfirm,
  goBack,
  goNext,
  isQuestion,
  newWizard,
  pickIndex,
  progressChip,
  putWizard,
  togglePick,
  wizardFor,
  type QuestionPrompt,
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
          multi_select: false,
          prompts: [],
        },
      }),
    ).toBe(true);
  });
});

const prompts: QuestionPrompt[] = [
  {
    question: 'Which filters?',
    header: 'Filters',
    multi_select: true,
    options: [
      { label: 'Date range', description: '' },
      { label: 'Order status', description: '' },
      { label: 'Customer', description: '' },
    ],
  },
  {
    question: 'Which state library?',
    header: 'State',
    multi_select: false,
    options: [
      { label: 'Zustand', description: '' },
      { label: 'Redux Toolkit', description: '' },
    ],
  },
  {
    question: 'Default range?',
    header: 'Range',
    multi_select: false,
    options: [
      { label: '7 days', description: '' },
      { label: '30 days', description: '' },
    ],
  },
];

describe('multi-select (#question-multi)', () => {
  test('Confirm stays disabled until at least one option is picked', () => {
    let state = newWizard(1);
    expect(canConfirm(state)).toBe(false);
    state = togglePick(state, 0, true);
    expect(canConfirm(state)).toBe(true);
    state = togglePick(state, 0, true);
    expect(canConfirm(state)).toBe(false);
  });

  test('picks accumulate in multi-select and replace in single-select', () => {
    let multi = togglePick(newWizard(1), 0, true);
    multi = togglePick(multi, 2, true);
    expect(multi.picks[0]).toEqual([0, 2]);
    multi = togglePick(multi, 0, true);
    expect(multi.picks[0]).toEqual([2]);

    let single = togglePick(newWizard(1), 0, false);
    expect(single.picks[0]).toEqual([0]);
    single = togglePick(single, 2, false);
    expect(single.picks[0]).toEqual([2]);
  });

  test('a multi-select answer joins the picked labels with ", " in option order', () => {
    let state = togglePick(newWizard(1), 2, true);
    state = togglePick(state, 0, true);
    expect(answersOf(state, prompts.slice(0, 1))).toEqual(['Date range, Customer']);
  });
});

describe('wizard (#question-multi)', () => {
  test('chip is n/total and hidden for a single question', () => {
    expect(progressChip(0, 1)).toBe('');
    expect(progressChip(0, 3)).toBe('1/3');
    expect(progressChip(1, 3)).toBe('2/3');
    expect(progressChip(2, 3)).toBe('3/3');
  });

  test('Back only moves from the second question onward, never past the ends', () => {
    const state = newWizard(3);
    expect(goBack(state).step).toBe(0);
    const second = goNext(state, 3);
    expect(second.step).toBe(1);
    expect(goBack(second).step).toBe(0);
    expect(goNext(goNext(goNext(state, 3), 3), 3).step).toBe(2);
  });

  test('answers are collected in question order, one string per prompt', () => {
    let state = togglePick(newWizard(3), 0, true); // Q1 multi
    state = goNext(state, 3);
    state = togglePick(state, 1, false); // Q2 single
    state = goNext(state, 3);
    // Q3 left unanswered on purpose: a wizard cannot confirm until every step has a pick.
    expect(state.step).toBe(2);
    expect(answersOf(state, prompts)).toEqual(['Date range', 'Redux Toolkit', '']);
  });

  test('answers never leak between two requests in the per-request store', () => {
    let store = { [10]: newWizard(3), [11]: newWizard(3) };
    store = { ...store, [10]: togglePick(store[10], 0, false) };
    expect(store[10].picks[0]).toEqual([0]);
    expect(store[11].picks[0]).toBeNull();
    expect(wizardFor(store, 11, 3).picks[0]).toBeNull();

    // A new request reusing an id starts fresh once the old one is dropped.
    const { [10]: _gone, ...rest } = store;
    expect(wizardFor(rest, 10, 3)).toEqual(newWizard(3));
    // An unknown id never borrows another wizard's picks.
    expect(putWizard(store, 12, newWizard(3))[12]).toEqual(newWizard(3));
  });
});
