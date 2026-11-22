// #188 — pure view rules for the single-choice Question card (#question).
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

/** One wizard's picks, held per request id so answers never cross requests. */
export type WizardState = {
  /** 0-based prompt being shown. */
  step: number;
  /** Picked option indices per prompt; `null` until that prompt is answered. */
  picks: (number[] | null)[];
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
  return { step: 0, picks: Array.from({ length: total }, () => null) };
}

/** Toggle an option at the current step. Multi accumulates; single replaces. */
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
  return { ...state, picks };
}

/** Confirm / Next is enabled only once the current step has a pick. */
export function canConfirm(state: WizardState): boolean {
  return (state.picks[state.step]?.length ?? 0) > 0;
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
