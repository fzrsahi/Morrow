// #187 — the card's side of a jump to the session's terminal (L-009, W-007).
// The shell decides the outcome; this module only turns its wire shape into
// the notice the canvas #jump draws. Keeping it pure makes the three outcomes
// testable without a desktop or a clipboard.

/** Serialized `morrow_platform::terminal::Outcome` from `terminal_activate`. */
export type TerminalOutcome =
  | 'Focused'
  | { Copied: string }
  | { NotPossible: string };

export type JumpView = {
  /** The #jump note family: green for a jump or a hand-off, amber for a refusal. */
  kind: 'ok' | 'warn' | null;
  /** Words before the highlighted path. */
  before: string;
  /** The copied path, set apart as <code> like the canvas; null otherwise. */
  code: string | null;
  /** Words after the highlighted path. */
  after: string;
  /** NotPossible shakes the card (#jump's third cell). */
  shake: boolean;
};

const NONE: JumpView = { kind: null, before: '', code: null, after: '', shake: false };

/** What a card shows after a jump. The card itself stays in the queue. */
export function jumpView(outcome: TerminalOutcome | null): JumpView {
  if (outcome === null) return NONE;
  if (outcome === 'Focused') {
    return {
      kind: 'ok',
      before: "Focused this session's terminal.",
      code: null,
      after: '',
      shake: false,
    };
  }
  if (typeof outcome === 'object' && 'Copied' in outcome) {
    return {
      kind: 'ok',
      before: 'Copied ',
      code: outcome.Copied,
      after: '. No terminal window was found for this session.',
      shake: false,
    };
  }
  if (typeof outcome === 'object' && 'NotPossible' in outcome) {
    return {
      kind: 'warn',
      before: outcome.NotPossible,
      code: null,
      after: '',
      shake: true,
    };
  }
  return NONE;
}

/** A remote session has no window on this machine to raise (#jump note). */
export function showJumpIcon(session: { is_remote: boolean }): boolean {
  return !session.is_remote;
}
