import { describe, expect, test } from 'bun:test';

import { anchoringReason, NoticeLifetime } from './anchoringNotice';
import { describeCapability } from './settings';

// #200 — the probe result is the single source of the sentence. Tauri serialises
// `Anchoring` as `"Available"` or `{ "Unavailable": "<reason>" }`.
describe('anchoring notice reason', () => {
  test('X11 and a working layer surface show nothing', () => {
    expect(anchoringReason('Available')).toBeNull();
  });

  test('an unavailable compositor shows the exact reason, not a paraphrase', () => {
    const reason = 'Anchoring is not available on this desktop (no wlr-layer-shell)';
    expect(anchoringReason({ Unavailable: reason })).toBe(reason);
  });

  test('the peek and the Capabilities row read the same sentence', () => {
    const reason = 'GNOME does not let apps pin a panel above other windows';
    const peek = anchoringReason({ Unavailable: reason });
    const row = describeCapability({
      feature: 'Panel anchoring',
      capability: { Degraded: reason },
    }).reason;
    expect(peek).toBe(reason);
    expect(row).toBe(reason);
    expect(peek).toBe(row);
  });
});

describe('anchoring notice lifetime', () => {
  test('closes at 5 s when the pointer is not over the peek', () => {
    const lifetime = new NoticeLifetime();
    expect(lifetime.timeout()).toBe(true);
  });

  test('stays open past 5 s while the pointer rests on it', () => {
    const lifetime = new NoticeLifetime();
    lifetime.pointerEnter();
    expect(lifetime.timeout()).toBe(false);
  });

  test('closes as soon as the pointer leaves after the deadline', () => {
    const lifetime = new NoticeLifetime();
    lifetime.pointerEnter();
    expect(lifetime.timeout()).toBe(false);
    expect(lifetime.pointerLeave()).toBe(true);
  });

  test('leaving before the deadline does not close it', () => {
    const lifetime = new NoticeLifetime();
    lifetime.pointerEnter();
    expect(lifetime.pointerLeave()).toBe(false);
    expect(lifetime.timeout()).toBe(true);
  });
});
