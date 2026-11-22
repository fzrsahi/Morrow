// #185 — pure view rules for the session list. The shell sends sessions in
// tier order; these rules keep that order while making it testable, and decide
// how many rows fit before the "N more sessions" footer takes over.

import type { SessionFilter } from './panel';
import { STATUS_META } from './session-cards';
import { PALETTE } from './tokens';
import type { CardStatus, Session } from './types';

/** 34 pt compact rows, 3 pt gaps: four fit the open panel before it scrolls. */
export const VISIBLE_SESSION_LIMIT = 4;
/** Comfortable cards are ~90 pt; the canvas overflow frame shows two. */
export const COMFORTABLE_SESSION_LIMIT = 2;

/** NEEDS YOU first, then working/thinking, then the rest; newest first inside
 *  a tier and by session id on a tie, so the order never jitters. */
export function orderSessions(sessions: Session[]): Session[] {
  return [...sessions].sort(
    (a, b) =>
      STATUS_META[a.status].tier - STATUS_META[b.status].tier ||
      b.updated_at - a.updated_at ||
      a.session_id.localeCompare(b.session_id),
  );
}

/** How many rows are drawn before the overflow footer, and what is left. */
export function splitOverflow<T>(
  items: T[],
  limit = VISIBLE_SESSION_LIMIT,
): { shown: T[]; remaining: number } {
  const cap = Math.max(0, Math.floor(limit));
  return { shown: items.slice(0, cap), remaining: Math.max(0, items.length - cap) };
}

export type SessionGroup = {
  key: string;
  /** Null for ALL: the group has no header, just the tier-ordered rows. */
  label: string | null;
  color: string;
  sessions: Session[];
};

/** The canvas groups thinking under WORKING; the card still says THINKING. */
const STATUS_GROUPS: CardStatus[] = ['needs_you', 'working', 'done', 'idle', 'stopped', 'error'];

function statusGroup(status: CardStatus): CardStatus {
  return status === 'thinking' ? 'working' : status;
}

/** ALL keeps the shell's tier order in one run; STA groups by status (with
 *  thinking folded into working); AGT groups by source. Headers read
 *  `LABEL · count` on the canvas, and empty groups are never drawn. */
export function groupSessions(sessions: Session[], filter: SessionFilter): SessionGroup[] {
  const ordered = orderSessions(sessions);
  if (filter === 'all') {
    return ordered.length
      ? [{ key: 'all', label: null, color: PALETTE.textWhite, sessions: ordered }]
      : [];
  }
  if (filter === 'sta') {
    return STATUS_GROUPS.flatMap((status) => {
      const rows = ordered.filter((session) => statusGroup(session.status) === status);
      if (rows.length === 0) return [];
      return [
        {
          key: status,
          label: STATUS_META[status].label,
          color: STATUS_META[status].color,
          sessions: rows,
        },
      ];
    });
  }
  const sources: SessionGroup[] = [];
  for (const session of ordered) {
    const key = session.source;
    let group = sources.find((item) => item.key === key);
    if (!group) {
      group = { key, label: key.toUpperCase(), color: PALETTE.textWhite, sessions: [] };
      sources.push(group);
    }
    group.sessions.push(session);
  }
  return sources;
}
