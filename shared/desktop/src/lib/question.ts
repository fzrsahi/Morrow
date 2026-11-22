// #188 — pure view rules for the single-choice Question card (#question).
// The reply bytes are built in Rust; these are only the panel-side seams.

import type { Card, QuestionCard } from './types';

export type QuestionCardView = Card & { question: QuestionCard };

/** #question: number keys 1–9 pick an option while the panel has focus. */
export function pickIndex(key: string): number | null {
  return /^[1-9]$/.test(key) ? Number(key) - 1 : null;
}

/** An approval card and a question card share the E2E-1 `[data-card]` seam. */
export function isQuestion(card: Card): card is QuestionCardView {
  return card.question != null;
}
