// #308 — pure view rules for the quota card. These are the seams the component
// consumes: a missing reading has no rows, the level drives the bar, and a
// percentage only exists once the shell says `ok`.

import { describe, expect, test } from 'bun:test';
import { QUOTA_STATUS_LABEL, quotaBar, quotaCardView } from './quota';
import type { QuotaSnapshot } from './types';

function snapshot(status: QuotaSnapshot['status'], limits: QuotaSnapshot['limits'] = null): QuotaSnapshot {
  return { status, reason: 'the shell could not reach Anthropic', limits };
}

describe('states without numbers', () => {
  test('unavailable and not_connected show the status and no percentage', () => {
    for (const status of ['unavailable', 'not_connected'] as const) {
      const view = quotaCardView(snapshot(status));
      expect(view.title).toBe(QUOTA_STATUS_LABEL[status]);
      expect(view.reason).toBe('the shell could not reach Anthropic');
      expect(view.rows).toEqual([]);
      expect(JSON.stringify(view)).not.toContain('%');
      expect(JSON.stringify(view)).not.toContain('percent');
    }
  });

  test('unauthorized, rate_limited and a missing DTO are equally quiet', () => {
    for (const status of ['unauthorized', 'rate_limited'] as const) {
      expect(quotaCardView(snapshot(status)).rows).toEqual([]);
    }
    const empty = quotaCardView(null);
    expect(empty.title).toBe(QUOTA_STATUS_LABEL.unavailable);
    expect(empty.reason).toBe('');
    expect(empty.rows).toEqual([]);
  });

  test('an ok status with no limits still has no rows', () => {
    expect(quotaCardView(snapshot('ok')).rows).toEqual([]);
  });
});

describe('a real reading', () => {
  const ok: QuotaSnapshot = {
    status: 'ok',
    reason: '',
    limits: [
      {
        kind: 'session',
        percent: 72,
        level: 'normal',
        pace_tone: 'ahead',
        resets_at: 1_800_010_800,
        label: '5h',
      },
      {
        kind: 'weekly_all',
        percent: 104.5,
        level: 'warning',
        pace_tone: null,
        resets_at: null,
        label: 'Week',
      },
      {
        kind: 'weekly_scoped',
        percent: 100,
        level: 'critical',
        pace_tone: 'neutral',
        resets_at: null,
        label: 'Fable',
      },
    ],
  };

  test('the bar class follows the level', () => {
    expect(quotaBar('normal').barClass).toBe('bar-normal');
    expect(quotaBar('warning').barClass).toBe('bar-warning');
    expect(quotaBar('critical').barClass).toBe('bar-critical');
    const rows = quotaCardView(ok).rows;
    expect(rows.map((row) => row.barClass)).toEqual(['bar-normal', 'bar-warning', 'bar-critical']);
  });

  test('percent is shown when ok and the fill cannot overflow', () => {
    const rows = quotaCardView(ok).rows;
    expect(rows[0].percent).toBe(72);
    expect(rows[1].percent).toBe(104.5);
    expect(rows.map((row) => row.width)).toEqual([72, 100, 100]);
    expect(rows.map((row) => row.label)).toEqual(['5h', 'Week', 'Fable']);
  });

  test('pace is shown only when the shell reported one', () => {
    expect(quotaCardView(ok).rows.map((row) => row.paceLabel)).toEqual([
      'ahead of pace',
      null,
      'on pace',
    ]);
  });
});
