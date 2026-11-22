// #191 — a finished turn is offered once, and a card the pointer has never
// entered is never collapsed by a pointer leave (NotchPanelView.swift:1485).
// The shell decides whether the turn calls for a card at all; these are the
// panel's own presentation rules, kept pure for tests.

import type { Completion } from './types';

export type { Completion };

/** AppState.completionAutoCollapseDelay (macOS), in milliseconds. */
export const COMPLETION_AUTO_COLLAPSE_MS = 5000;

/** One card per finished turn, even if the feed pushes the same reply again. */
export function completionKey(completion: Completion): string {
  return `${completion.session_id}:${completion.updated_at}`;
}

/** AttentionPolicy settles a card as `.card`; `.glance` and `.status_only` stay compact. */
export function shouldAutoOpen(completion: Completion | null, seenKey: string | null): boolean {
  return completion !== null && completion.presentation === 'card' && completionKey(completion) !== seenKey;
}

/** Tracks whether the pointer has visited the card that is currently shown. */
export class CompletionGate {
  private key: string | null = null;
  private entered = false;
  private leftAt: number | null = null;

  begin(key: string): void {
    this.key = key;
    this.entered = false;
    this.leftAt = null;
  }

  enter(): void {
    this.entered = true;
    this.leftAt = null;
  }

  leave(now: number): void {
    this.leftAt = now;
  }

  end(): void {
    this.key = null;
    this.entered = false;
    this.leftAt = null;
  }

  get active(): boolean {
    return this.key !== null;
  }

  /** True while the shown card is waiting for its first pointer visit. */
  blocksCollapse(key: string): boolean {
    return this.key === key && !this.entered;
  }
}
