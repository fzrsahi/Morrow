// #201 — the follow-up reminder policy (macOS user story 27). One OS
// notification per approval or question that is still waiting when the
// follow-up interval elapses. No timers, no IPC, no clock of its own: App.svelte
// owns the wake-up and the `notify_send` call; this module only decides what is
// due and when to wake, so the policy is testable on an injected clock.

import type { AmbientState } from './ambient';
import type { Card } from './types';

/**
 * macOS `FollowUpReminderController.intervalChoices` is [0, 1, 2, 3, 5]
 * minutes, where 0 is off. One minute is the shortest enabled choice and the
 * default timeout a waiting request is first re-announced at.
 */
export const FOLLOW_UP_CHOICES_MINUTES = [0, 1, 2, 3, 5] as const;
export const DEFAULT_FOLLOW_UP_MINUTES = 1;
export const SETTINGS_KEY_FOLLOW_UP_MINUTES = 'followUpReminderMinutes';
export const FOLLOW_UP_MS = 60_000;

/** Milliseconds for a stored choice, or `null` when reminders are off. */
export function followUpIntervalMs(minutes: number): number | null {
  if (!FOLLOW_UP_CHOICES_MINUTES.includes(minutes as (typeof FOLLOW_UP_CHOICES_MINUTES)[number])) {
    return DEFAULT_FOLLOW_UP_MINUTES * FOLLOW_UP_MS;
  }
  return minutes === 0 ? null : minutes * FOLLOW_UP_MS;
}

/** The stored setting as one of the offered choices, defaulting to 1 minute. */
export function followUpMinutesFromSettings(value: unknown): number {
  if (value === null || value === undefined || value === '') return DEFAULT_FOLLOW_UP_MINUTES;
  const minutes = typeof value === 'number' ? value : Number(value);
  return FOLLOW_UP_CHOICES_MINUTES.includes(minutes as (typeof FOLLOW_UP_CHOICES_MINUTES)[number])
    ? minutes
    : DEFAULT_FOLLOW_UP_MINUTES;
}

/** What the shell and the ambient poll say about delivering a reminder now.
 *  `locked` and `quiet` are the ambient scene's own fields, not a second
 *  definition of them (#33 review). */
export type ReminderRoom = Pick<AmbientState, 'locked' | 'quiet'> & {
  /** Capability "Notifications" is Supported (never Degraded/Unsupported). */
  capable: boolean;
};

/** One `notify_send` call, paired with the card it reminds about. */
export type Reminder = {
  id: number;
  title: string;
  body: string;
};

type Entry = {
  card: Card;
  /** Timestamp on the same clock the caller passes to `collect`. */
  waitingSince: number;
  /** A due reminder was claimed; a request never gets a second one. */
  sent: boolean;
};

function eligible(room: ReminderRoom): boolean {
  return room.capable && !room.locked && !room.quiet;
}

function content(card: Card): Reminder {
  const question = card.question;
  const detail = (question?.question || card.summary || card.tool_name).trim();
  return {
    id: card.id,
    title: question ? 'Morrow · Question waiting' : 'Morrow · Approval waiting',
    body: detail ? `${card.session_id} · ${detail}` : card.session_id,
  };
}

export class FollowUpReminders {
  private entries = new Map<number, Entry>();
  private intervalMs: number | null = FOLLOW_UP_MS;

  constructor(minutes: number = DEFAULT_FOLLOW_UP_MINUTES) {
    this.intervalMs = followUpIntervalMs(minutes);
  }

  /** #201 PR203 review: honour the stored 0/1/2/3/5-minute choice. Changing
   *  it restarts every wait and clears already-sent bells, like Swift's
   *  `FollowUpReminderController.settingsChanged()`. `now` is injected so the
   *  policy stays testable without a wall clock. */
  configure(minutes: number, now: number): void {
    this.intervalMs = followUpIntervalMs(minutes);
    for (const entry of this.entries.values()) {
      entry.waitingSince = now;
      entry.sent = false;
    }
  }

  /** True when reminders are on (a choice other than 0). */
  get enabled(): boolean {
    return this.intervalMs !== null;
  }

  /** Reconcile with the feed's held requests. A new id starts a new wait;
   *  an id that is gone was answered or dropped, so its scheduler stops and
   *  any bell hint goes with it. */
  sync(cards: Card[], now: number): void {
    const pending = new Set(cards.map((card) => card.id));
    for (const id of this.entries.keys()) {
      if (!pending.has(id)) this.entries.delete(id);
    }
    for (const card of cards) {
      if (!this.entries.has(card.id)) {
        this.entries.set(card.id, { card, waitingSince: now, sent: false });
      } else {
        this.entries.get(card.id)!.card = card;
      }
    }
  }

  /** Claim every request that has waited long enough and may be notified now.
   *  Held-back requests keep their wait and are delivered on the first eligible
   *  pass after the hold ends — the macOS `.deferred` → `.catchUp` path. */
  collect(now: number, room: ReminderRoom): Reminder[] {
    if (!eligible(room) || this.intervalMs === null) return [];
    const due: Reminder[] = [];
    for (const entry of this.entries.values()) {
      if (entry.sent || now - entry.waitingSince < this.intervalMs) continue;
      entry.sent = true;
      due.push(content(entry.card));
    }
    return due;
  }

  /** Absolute time of the next wake-up, or null when there is nothing to
   *  schedule (no wait left, all sent, or the room is held back). */
  nextDue(now: number, room: ReminderRoom): number | null {
    if (!eligible(room) || this.intervalMs === null) return null;
    let earliest: number | null = null;
    for (const entry of this.entries.values()) {
      if (entry.sent) continue;
      const due = entry.waitingSince + this.intervalMs;
      if (earliest === null || due < earliest) earliest = due;
    }
    return earliest === null ? null : Math.max(now, earliest);
  }

  /** Ids whose reminder went out: already-hinted cards keep the bell until
   *  the request is answered or dropped and leaves the feed. */
  hinted(): number[] {
    return [...this.entries.values()]
      .filter((entry) => entry.sent)
      .map((entry) => entry.card.id);
  }

  /** False once every request was answered or dropped: no timer to arm. */
  get active(): boolean {
    return this.entries.size > 0;
  }
}
