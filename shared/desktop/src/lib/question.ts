// #188 — pure view rules for the single-choice Question card (#question).
// #189 — typed answers (Other / free text) and the focus lifecycle.
// #190 — multi-select and the multi-question wizard (#question-multi).
// The reply bytes are built in Rust; these are only the panel-side seams.

import type { Card, QuestionCard, QuestionPrompt } from './types';

export type QuestionCardView = Card & { question: QuestionCard };
export { type QuestionPrompt };

/** #question: number keys 1–9 pick an option while the panel has focus. */
export function pickIndex(key: string): number | null {
  return /^[1-9]$/.test(key) ? Number(key) - 1 : null;
}

/** An approval card and a question card share the E2E-1 `[data-card]` seam. */
export function isQuestion(card: Card): card is QuestionCardView {
  return card.question != null;
}

/** The row that opens a typed answer, after the prompt's options (#189). */
export const OTHER_LABEL = 'Other';

/** #189: a prompt with no options is free text; one with options gets the
 *  `Other` row, which reveals the field when picked (NotchPanelView.swift).
 *  Takes the *current* prompt's options so a wizard step types correctly too. */
export function questionInput(options: readonly { label: string }[]): 'free' | 'other' {
  return options.length === 0 ? 'free' : 'other';
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

/** One wizard's picks, held per request id so answers never cross requests. */
export type WizardState = {
  /** 0-based prompt being shown. */
  step: number;
  /** Picked option indices per prompt; `null` until that prompt is answered. */
  picks: (number[] | null)[];
  /** #189: a typed (Other / free-text) answer for a step, instead of options. */
  typed: (string | null)[];
};

/** The prompts a wizard walks, falling back to the single #188 question card. */
export function promptsOf(card: QuestionCard): QuestionPrompt[] {
  if (card.prompts.length > 0) return card.prompts;
  return [
    {
      question: card.question,
      header: card.header,
      options: card.options,
      multi_select: card.multi_select,
    },
  ];
}

export function newWizard(total: number): WizardState {
  return {
    step: 0,
    picks: Array.from({ length: total }, () => null),
    typed: Array.from({ length: total }, () => null),
  };
}

/** Toggle an option at the current step. Multi accumulates; single replaces.
 *  Picking an option drops a typed answer for that step — the two are exclusive. */
export function togglePick(state: WizardState, index: number, multi: boolean): WizardState {
  const current = state.picks[state.step] ?? [];
  const next = multi
    ? current.includes(index)
      ? current.filter((i) => i !== index)
      : [...current, index].sort((a, b) => a - b)
    : current.length === 1 && current[0] === index
      ? []
      : [index];
  const picks = [...state.picks];
  picks[state.step] = next;
  const typed = [...state.typed];
  typed[state.step] = null;
  return { ...state, picks, typed };
}

/** #189: record a typed answer for the current step, clearing its option picks. */
export function setTyped(state: WizardState, text: string): WizardState {
  if (!canSubmitTyped(text)) return state;
  const typed = [...state.typed];
  typed[state.step] = text;
  const picks = [...state.picks];
  picks[state.step] = null;
  return { ...state, typed, picks };
}

/** Confirm / Next is enabled once the current step has a pick or typed text. */
export function canConfirm(state: WizardState): boolean {
  return (state.picks[state.step]?.length ?? 0) > 0 || canSubmitTyped(state.typed[state.step] ?? '');
}

/** `n/total` chip; a single-question request has no progress chip. */
export function progressChip(step: number, total: number): string {
  return total > 1 ? `${step + 1}/${total}` : '';
}

export function goBack(state: WizardState): WizardState {
  return { ...state, step: Math.max(0, state.step - 1) };
}

export function goNext(state: WizardState, total: number): WizardState {
  return { ...state, step: Math.min(total - 1, state.step + 1) };
}

/** Answer strings in prompt order. A multi-select joins labels like macOS. */
export function answersOf(state: WizardState, prompts: QuestionPrompt[]): string[] {
  return prompts.map((prompt, i) => {
    // #189: a typed answer wins over option picks for that step.
    const typed = state.typed[i];
    if (typed) return typed;
    const picked = state.picks[i] ?? [];
    return picked
      .map((index) => prompt.options[index]?.label ?? '')
      .filter(Boolean)
      .join(', ');
  });
}

/** Per-request store: a card id only ever sees its own wizard. */
export function wizardFor(
  store: Record<number, WizardState>,
  id: number,
  total: number,
): WizardState {
  return store[id] ?? newWizard(total);
}

export function putWizard(
  store: Record<number, WizardState>,
  id: number,
  wizard: WizardState,
): Record<number, WizardState> {
  return { ...store, [id]: wizard };
}
