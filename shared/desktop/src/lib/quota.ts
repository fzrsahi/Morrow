// #308 — pure view rules for the quota card. The shell owns the fetch, the
// numbers and every status decision; this module only turns its DTO into rows
// the card can draw. Keeping it separate makes the "no fake 0%" rule testable
// without a DOM or a network.

import type { QuotaLevel, QuotaPaceTone, QuotaSnapshot, QuotaStatus } from './types';

export const QUOTA_STATUS_LABEL: Record<QuotaStatus, string> = {
  unavailable: 'Plan limits unavailable',
  not_connected: 'Not connected to Claude',
  unauthorized: 'Claude sign-in needed',
  rate_limited: 'Claude is rate limiting requests',
  ok: 'Plan limits',
};

const PACE_LABEL: Record<QuotaPaceTone, string> = {
  ahead: 'ahead of pace',
  behind: 'behind pace',
  neutral: 'on pace',
};

export type QuotaRow = {
  key: string;
  label: string;
  percent: number;
  /** Width of the fill, clamped so a server above 100% cannot overflow. */
  width: number;
  level: QuotaLevel;
  barClass: string;
  barColor: string;
  paceTone: QuotaPaceTone | null;
  paceLabel: string | null;
  resetsAt: number | null;
};

export type QuotaCardView = {
  title: string;
  reason: string;
  rows: QuotaRow[];
};

/** The level is a class *and* an inline colour: no CSS can turn it into 0%. */
export function quotaBar(level: QuotaLevel): { barClass: string; barColor: string } {
  return {
    barClass: `bar-${level}`,
    barColor:
      level === 'normal' ? 'var(--cyan)' : level === 'warning' ? 'var(--amber)' : 'var(--red)',
  };
}

/// A non-ok state (or no DTO at all) has no rows. The card renders only the
/// status sentence and the reason, so no percentage can appear by accident.
export function quotaCardView(quota: QuotaSnapshot | null): QuotaCardView {
  if (!quota || quota.status !== 'ok' || !quota.limits) {
    const status = quota?.status ?? 'unavailable';
    return {
      title: QUOTA_STATUS_LABEL[status],
      reason: quota?.reason.trim() ?? '',
      rows: [],
    };
  }
  return {
    title: QUOTA_STATUS_LABEL.ok,
    reason: '',
    rows: quota.limits.map((limit) => ({
      key: limit.kind,
      label: limit.label,
      percent: limit.percent,
      width: Math.max(0, Math.min(100, limit.percent)),
      level: limit.level,
      ...quotaBar(limit.level),
      paceTone: limit.pace_tone,
      paceLabel: limit.pace_tone ? PACE_LABEL[limit.pace_tone] : null,
      resetsAt: limit.resets_at,
    })),
  };
}
