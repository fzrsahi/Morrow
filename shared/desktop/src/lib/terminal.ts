// #239 AC 5 / L-009 — the terminal-jump button's decisions.
//
// The shell has had `terminal_activate` since #103 and nothing ever called it:
// the platform code focuses a window, copies a path, or explains why it cannot,
// and no surface offered the click. This module is the part that can be tested
// without a terminal — what to send, whether the row earns a button, and what the
// user is told afterwards.
//
// Mirrors `morrow_platform::terminal::{Target, Outcome}` (shared/crates), which is
// where the focusing and the clipboard fallback actually happen.
import { isSupported } from './capabilities';
import type { TerminalOutcome } from './terminal-jump';
import type { CapabilityRow, Session } from './types';

/** The arguments `terminal_activate` takes. Every field is optional: a session
 *  that knows only its working directory still gets the copy fallback. */
export type JumpArgs = {
  app: string | null;
  tty: string | null;
  cwd: string | null;
  pid: number | null;
};

/** The capability row a jump needs; `TerminalActivator` in Rust. */
export const JUMP_CAPABILITY = 'Jump to terminal';

function text(value: unknown): string | null {
  return typeof value === 'string' && value.trim().length > 0 ? value : null;
}

/** The jump request for a session, or `null` when it knows nothing to jump by.
 *
 *  `null` is the interesting answer: a row with no app, no tty, no cwd and no pid
 *  has nothing to focus and nothing to copy, so it must not get a button that is
 *  bound to fail. Windows takes the pid path (#187) and ignores the app id; the
 *  other three are what X11 and Wayland use.
 */
export function jumpArgs(session: Session): JumpArgs | null {
  const args: JumpArgs = {
    app: text(session.term_app),
    tty: text(session.tty_path),
    cwd: text(session.cwd),
    pid: typeof session.cli_pid === 'number' && session.cli_pid > 0 ? session.cli_pid : null,
  };
  if (args.app === null && args.tty === null && args.cwd === null && args.pid === null) return null;
  return args;
}

/** Whether this row earns a terminal button, given the adapter's answer.
 *
 *  Split from `canJump` because the two questions are answered in different
 *  places: the capability table is read once at startup, and the row is drawn many
 *  times. A button bound to fail is worse than no button — this is the same rule
 *  #235 settled for every other adapter row.
 */
export function canJumpWhen(capable: boolean, session: Session): boolean {
  return capable && jumpArgs(session) !== null;
}

/** Whether this row earns a terminal button, from the capability rows. */
export function canJump(session: Session, rows: CapabilityRow[]): boolean {
  return canJumpWhen(isSupported(rows, JUMP_CAPABILITY), session);
}

/** The button's tooltip: it names the app when the session reported one, and says
 *  what will actually happen when it did not — a jump that silently degrades into
 *  a copy should say so before the click, not after. */
export function jumpLabel(session: Session): string {
  const args = jumpArgs(session);
  if (args === null) return 'This session has no terminal to jump to';
  if (args.app !== null) return `Jump to ${args.app}`;
  if (args.tty !== null) return `Jump to ${args.tty}`;
  return 'Copy the working directory';
}

/** What an island row says after a click, or `null` when nothing needs saying.
 *
 *  The permission card already draws a fuller note (`terminal-jump.ts`); this is
 *  the one-line version for a session row, which is a different surface but the
 *  same outcome. A focused window needs no message: it is on screen. The other
 *  two do — one because the user wanted a window and got a clipboard, the other
 *  because neither happened and silence would read as success.
 */
export function outcomeNotice(outcome: TerminalOutcome): { text: string; failed: boolean } | null {
  if (outcome === 'Focused') return null;
  if (typeof outcome === 'object' && 'Copied' in outcome) {
    return { text: `Copied ${outcome.Copied}`, failed: false };
  }
  if (typeof outcome === 'object' && 'NotPossible' in outcome) {
    return { text: outcome.NotPossible, failed: true };
  }
  return null;
}
