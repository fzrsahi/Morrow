// #239 AC 5 / L-009 — the decisions behind the terminal-jump button.
import { describe, expect, test } from 'bun:test';

import { canJump, canJumpWhen, jumpArgs, jumpLabel, outcomeNotice } from './terminal';
import { isSupported } from './capabilities';
import type { TerminalOutcome } from './terminal-jump';
import type { CapabilityRow, Session } from './types';

function session(over: Partial<Session> = {}): Session {
  return {
    session_id: 's1',
    source: 'claude',
    status: 'running',
    cwd: '/home/dev/project',
    term_app: 'Alacritty',
    tty_path: '/dev/pts/3',
    cli_pid: 4242,
    ...over,
  } as Session;
}

const supported: CapabilityRow[] = [{ feature: 'Jump to terminal', capability: { Supported: null } }];
const unsupported: CapabilityRow[] = [
  { feature: 'Jump to terminal', capability: { Unsupported: 'no wmctrl' } },
];

describe('what a jump sends', () => {
  test('a session that knows everything sends everything', () => {
    expect(jumpArgs(session())).toEqual({
      app: 'Alacritty',
      tty: '/dev/pts/3',
      cwd: '/home/dev/project',
      pid: 4242,
    });
  });

  test('an empty field is null, not an empty string on the wire', () => {
    // The shell turns `Some("")` into a target with an empty app id and asks the
    // compositor to focus nothing.
    const args = jumpArgs(session({ term_app: '', tty_path: '  ', cwd: '' }));
    expect(args).toEqual({ app: null, tty: null, cwd: null, pid: 4242 });
  });

  test('a session with nothing to jump by has no args at all', () => {
    const bare = session({ term_app: '', tty_path: '', cwd: '', cli_pid: 0 });
    expect(jumpArgs(bare)).toBeNull();
  });

  test('the working directory alone is enough: that is the copy fallback', () => {
    const args = jumpArgs(session({ term_app: '', tty_path: '', cli_pid: 0 }));
    expect(args).toEqual({ app: null, tty: null, cwd: '/home/dev/project', pid: null });
  });
});

describe('whether the row earns a button', () => {
  test('a supported adapter and something to jump by', () => {
    expect(canJump(session(), supported)).toBe(true);
  });

  test('an unsupported adapter means no button, not a button that fails', () => {
    expect(canJump(session(), unsupported)).toBe(false);
  });

  test('a session with nothing to jump by gets no button even when supported', () => {
    const bare = session({ term_app: '', tty_path: '', cwd: '', cli_pid: 0 });
    expect(canJump(bare, supported)).toBe(false);
  });

  test('a machine with no capability row at all has no jump button', () => {
    expect(canJump(session(), [])).toBe(false);
  });

  test('the startup boolean and the row list agree', () => {
    // The island reads the table once and keeps a boolean; the component takes the
    // rows. They must not be able to disagree about the same machine.
    for (const rows of [supported, unsupported, []]) {
      expect(canJumpWhen(isSupported(rows, 'Jump to terminal'), session())).toBe(
        canJump(session(), rows),
      );
    }
  });
});

describe('what the button says it will do', () => {
  test('it names the app when the session reported one', () => {
    expect(jumpLabel(session())).toBe('Jump to Alacritty');
  });

  test('it names the tty when there is no app id', () => {
    expect(jumpLabel(session({ term_app: '' }))).toBe('Jump to /dev/pts/3');
  });

  test('it promises a copy when that is all that can happen', () => {
    // A jump that silently degrades into a clipboard should say so *before* the
    // click, not after.
    expect(jumpLabel(session({ term_app: '', tty_path: '' }))).toBe('Copy the working directory');
  });
});

describe('what the user is told afterwards', () => {
  test('a focused window says nothing: it is on screen', () => {
    expect(outcomeNotice('Focused' as TerminalOutcome)).toBeNull();
  });

  test('a copied path says what is on the clipboard', () => {
    expect(outcomeNotice({ Copied: '/home/dev/project' })).toEqual({
      text: 'Copied /home/dev/project',
      failed: false,
    });
  });

  test('neither happening is reported, not swallowed', () => {
    // The shell returns `NotPossible` with the compositor's reasons; silence here
    // would read as success and the user would click again.
    expect(outcomeNotice({ NotPossible: 'no wmctrl; no xdotool' })).toEqual({
      text: 'no wmctrl; no xdotool',
      failed: true,
    });
  });
});
