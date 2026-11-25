import { describe, expect, test } from 'bun:test';
import { NOTIFICATION_PREVIEWS } from './notification-previews';

describe('notification previews', () => {
  test('covers the Swift grid with unique ids', () => {
    expect(NOTIFICATION_PREVIEWS.length).toBeGreaterThanOrEqual(14);
    expect(new Set(NOTIFICATION_PREVIEWS.map((row) => row.id)).size).toBe(NOTIFICATION_PREVIEWS.length);
  });

  test('the first and last buttons are the Swift anchors', () => {
    expect(NOTIFICATION_PREVIEWS[0].title).toBe('Test MagSafe Charging');
    expect(NOTIFICATION_PREVIEWS[NOTIFICATION_PREVIEWS.length - 1].title).toBe('Test Quick Prompt');
  });
});
