// #187 — pure view rules for the jump-to-terminal outcomes on a card.
// The shell owns the outcome; this module only decides what the card shows.
// Seam: the serialized `terminal::Outcome` wire shape from Rust → the notice
// (kind, words, shake) the canvas #jump draws.

import { describe, expect, test } from 'bun:test';
import { jumpView, showJumpIcon, type TerminalOutcome } from './terminal-jump';

describe('#jump outcome on the card', () => {
  test('Focused: an ok notice, no shake — the card stays queued', () => {
    expect(jumpView('Focused')).toEqual({
      kind: 'ok',
      before: "Focused this session's terminal.",
      code: null,
      after: '',
      shake: false,
    });
  });

  test('Copied(path): the path is set apart as code in the ok notice', () => {
    const outcome: TerminalOutcome = { Copied: '~/code/web-app' };
    expect(jumpView(outcome)).toEqual({
      kind: 'ok',
      before: 'Copied ',
      code: '~/code/web-app',
      after: '. No terminal window was found for this session.',
      shake: false,
    });
  });

  test('NotPossible(reason): a warning notice and a shake', () => {
    const outcome: TerminalOutcome = {
      NotPossible: 'This desktop does not let apps raise other windows (GNOME Wayland).',
    };
    expect(jumpView(outcome)).toEqual({
      kind: 'warn',
      before: 'This desktop does not let apps raise other windows (GNOME Wayland).',
      code: null,
      after: '',
      shake: true,
    });
  });
});

describe('#jump icon visibility', () => {
  test('a local session can jump', () => {
    expect(showJumpIcon({ is_remote: false })).toBe(true);
  });

  test('a remote session has no local terminal to raise', () => {
    expect(showJumpIcon({ is_remote: true })).toBe(false);
  });
});
