// #191 — the completion card's presentation and pointer gate. The shell
// decides what a finished turn calls for; these are the panel's rules for
// opening once, and for not collapsing a card the user never touched.
import { describe, expect, test } from 'bun:test';
import {
  COMPLETION_AUTO_COLLAPSE_MS,
  CompletionGate,
  completionKey,
  shouldAutoOpen,
  type Completion,
} from './completion';

const card: Completion = {
  session_id: 'a',
  updated_at: 12,
  turn_failed: false,
  presentation: 'card',
  blocks: [],
};

describe('completion presentation', () => {
  test('a card opens once per finished turn', () => {
    const key = completionKey(card);
    expect(shouldAutoOpen(card, null)).toBe(true);
    expect(shouldAutoOpen(card, key)).toBe(false);
    expect(completionKey({ ...card, updated_at: 13 })).not.toBe(key);
    expect(completionKey({ ...card, session_id: 'b' })).not.toBe(key);
  });

  test('glance and status-only never auto-open', () => {
    expect(shouldAutoOpen({ ...card, presentation: 'glance' }, null)).toBe(false);
    expect(shouldAutoOpen({ ...card, presentation: 'status_only' }, null)).toBe(false);
    expect(shouldAutoOpen(null, null)).toBe(false);
  });

  test('the auto-collapse delay matches the macOS card', () => {
    expect(COMPLETION_AUTO_COLLAPSE_MS).toBe(5000);
  });
});

describe('CompletionGate', () => {
  test('a card the pointer never entered does not collapse on leave', () => {
    const gate = new CompletionGate();
    gate.begin(completionKey(card));
    expect(gate.blocksCollapse(completionKey(card))).toBe(true);
    gate.leave(1000);
    expect(gate.blocksCollapse(completionKey(card))).toBe(true);
  });

  test('entering the card once allows a later leave to collapse it', () => {
    const gate = new CompletionGate();
    gate.begin(completionKey(card));
    gate.enter();
    gate.leave(1000);
    expect(gate.blocksCollapse(completionKey(card))).toBe(false);
  });

  test('a new completion is a new card with a fresh entry flag', () => {
    const gate = new CompletionGate();
    gate.begin(completionKey(card));
    gate.enter();
    gate.begin(completionKey({ ...card, updated_at: 13 }));
    expect(gate.blocksCollapse(completionKey(card))).toBe(false);
    expect(gate.blocksCollapse(completionKey({ ...card, updated_at: 13 }))).toBe(true);
    gate.end();
    expect(gate.blocksCollapse(completionKey({ ...card, updated_at: 13 }))).toBe(false);
  });
});
