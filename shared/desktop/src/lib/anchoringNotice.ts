// #200 — the anchoring notice. The probe result is the single source of the
// sentence: Tauri serialises `Anchoring` as `"Available"` (X11 or a compositor
// with wlr-layer-shell) or `{ "Unavailable": "<reason>" }`. The mini-peek and
// Settings → Capabilities show that same string, never a paraphrase of it.

export type AnchoringResult = 'Available' | { Unavailable: string };

/** The reason to show, or `null` when there is nothing to say. */
export function anchoringReason(result: AnchoringResult): string | null {
  return typeof result === 'string' ? null : result.Unavailable;
}

export const NOTICE_DURATION_MS = 5000;

/**
 * The peek's lifetime, independent of the DOM so it can be tested: close at
 * 5 s unless the pointer is resting on the peek; if it is, wait until the
 * pointer leaves. A click always dismisses (handled by the component).
 */
export class NoticeLifetime {
  #hovered = false;
  #expired = false;

  pointerEnter(): void {
    this.#hovered = true;
  }

  /** True when the peek should close now. */
  pointerLeave(): boolean {
    this.#hovered = false;
    return this.#expired;
  }

  /** The 5 s timer fired; true when the peek should close now. */
  timeout(): boolean {
    this.#expired = true;
    return !this.#hovered;
  }
}
