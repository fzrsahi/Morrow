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

/** The row that opens a typed answer, after the hook's options (#189). */
export const OTHER_LABEL = 'Other';

/** #189: a question with no options is free text; one with options gets the
 *  `Other` row, which reveals the field when picked (NotchPanelView.swift). */
export function questionInput(card: QuestionCardView): 'free' | 'other' {
  return card.question.options.length === 0 ? 'free' : 'other';
}

/** Swift's `canSubmitText` / `canSubmitOther`: only an empty field is blocked. */
export function canSubmitTyped(text: string): boolean {
  return text.length > 0;
}

/** #189: the island is `focusable(false)` until a field is clicked. Only one
 *  field may own the keyboard, and closing its card gives focus back once. */
export class QuestionFocusGate {
  private owner: number | null = null;

  get ownerId(): number | null {
    return this.owner;
  }

  /** The field was clicked: the request to send, or null if it already owns. */
  focus(id: number): { open: boolean } | null {
    if (this.owner === id) return null;
    this.owner = id;
    return { open: true };
  }

  /** The owning field, card or panel went away: release focus once. */
  release(): { open: boolean } | null {
    if (this.owner === null) return null;
    this.owner = null;
    return { open: false };
  }
}
