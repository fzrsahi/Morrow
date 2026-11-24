// #201 — one OS follow-up per waiting request. The scheduler in App.svelte
// only owns the timer and the invoke; every decision is made here, on an
// injected clock, so the macOS policy can be tested without a desktop.
import { describe, expect, test } from 'bun:test';
import {
  FOLLOW_UP_MS,
  FollowUpReminders,
  followUpIntervalMs,
  followUpMinutesFromSettings,
  type ReminderRoom,
} from './reminders';
import type { Card } from './types';

const awake: ReminderRoom = { capable: true, locked: false, quiet: false };

function approval(id: number, summary = 'npm test'): Card {
  return {
    id,
    session_id: 's1',
    source: 'claude',
    tool_name: 'Bash',
    summary,
    tool_input: { command: summary },
    question: null,
  };
}

function question(id: number, text = 'Ship it?'): Card {
  return {
    id,
    session_id: 's2',
    source: 'claude',
    tool_name: 'AskUserQuestion',
    summary: 'a question',
    tool_input: {},
    question: { question: text, header: 'Release', options: [], from_permission: true },
  };
}

describe('FollowUpReminders (#201)', () => {
  test('a waiting approval is announced once, at the macOS follow-up timeout', () => {
    const reminders = new FollowUpReminders();
    reminders.sync([approval(7)], 0);

    expect(reminders.collect(FOLLOW_UP_MS - 1, awake)).toEqual([]);
    expect(reminders.collect(FOLLOW_UP_MS, awake)).toEqual([
      { id: 7, title: 'Morrow · Approval waiting', body: 's1 · npm test' },
    ]);
    expect(reminders.collect(FOLLOW_UP_MS * 2, awake)).toEqual([]);
  });

  test('a question names its question text in the notification', () => {
    const reminders = new FollowUpReminders();
    reminders.sync([question(8)], 0);

    expect(reminders.collect(FOLLOW_UP_MS, awake)).toEqual([
      { id: 8, title: 'Morrow · Question waiting', body: 's2 · Ship it?' },
    ]);
  });

  test('the bell hint appears only after a reminder, and only while it waits', () => {
    const reminders = new FollowUpReminders();
    reminders.sync([approval(7)], 0);
    expect(reminders.hinted()).toEqual([]);

    reminders.collect(FOLLOW_UP_MS, awake);
    expect(reminders.hinted()).toEqual([7]);

    reminders.sync([], FOLLOW_UP_MS);
    expect(reminders.hinted()).toEqual([]);
  });

  test('a locked screen holds the reminder back and catches it up on unlock', () => {
    const reminders = new FollowUpReminders();
    reminders.sync([approval(7)], 0);

    expect(reminders.collect(FOLLOW_UP_MS, { ...awake, locked: true })).toEqual([]);
    expect(reminders.collect(FOLLOW_UP_MS, awake)).toHaveLength(1);
    expect(reminders.collect(FOLLOW_UP_MS, { ...awake, locked: true })).toEqual([]);
  });

  test('quiet hours hold the reminder back too', () => {
    const reminders = new FollowUpReminders();
    reminders.sync([approval(7)], 0);

    expect(reminders.collect(FOLLOW_UP_MS, { ...awake, quiet: true })).toEqual([]);
    expect(reminders.collect(FOLLOW_UP_MS, awake)).toHaveLength(1);
  });

  test('no notification without the Notifications capability', () => {
    const reminders = new FollowUpReminders();
    reminders.sync([approval(7)], 0);

    expect(reminders.collect(FOLLOW_UP_MS, { ...awake, capable: false })).toEqual([]);
    expect(reminders.collect(FOLLOW_UP_MS, awake)).toHaveLength(1);
  });

  test('answering or dropping a request stops its scheduler', () => {
    const reminders = new FollowUpReminders();
    reminders.sync([approval(7)], 0);
    expect(reminders.active).toBe(true);

    reminders.sync([], FOLLOW_UP_MS);
    expect(reminders.active).toBe(false);
    expect(reminders.nextDue(FOLLOW_UP_MS, awake)).toBeNull();
    expect(reminders.collect(FOLLOW_UP_MS * 2, awake)).toEqual([]);
  });

  test('a request arriving later gets its own clock', () => {
    const reminders = new FollowUpReminders();
    reminders.sync([approval(7)], 0);
    reminders.sync([approval(7), approval(9)], 30_000);

    expect(reminders.collect(FOLLOW_UP_MS, awake).map((r) => r.id)).toEqual([7]);
    expect(reminders.collect(90_000, awake).map((r) => r.id)).toEqual([9]);
  });

  test('nextDue says when to wake and stays quiet while held back', () => {
    const reminders = new FollowUpReminders();
    expect(reminders.nextDue(0, awake)).toBeNull();

    reminders.sync([approval(7)], 0);
    expect(reminders.nextDue(0, awake)).toBe(FOLLOW_UP_MS);
    expect(reminders.nextDue(0, { ...awake, locked: true })).toBeNull();
    expect(reminders.nextDue(0, { ...awake, quiet: true })).toBeNull();
    expect(reminders.nextDue(0, { ...awake, capable: false })).toBeNull();

    reminders.collect(FOLLOW_UP_MS, awake);
    expect(reminders.nextDue(FOLLOW_UP_MS, awake)).toBeNull();
  });

  test('the interval is the smallest non-zero macOS follow-up choice', () => {
    expect(FOLLOW_UP_MS).toBe(60_000);
  });
});

describe('follow-up interval choices (#201 PR203 review)', () => {
  test('Swift choices are [0,1,2,3,5] minutes; 0 disables, unknown falls back to 1', () => {
    expect(followUpIntervalMs(0)).toBeNull();
    expect(followUpIntervalMs(1)).toBe(60_000);
    expect(followUpIntervalMs(2)).toBe(120_000);
    expect(followUpIntervalMs(3)).toBe(180_000);
    expect(followUpIntervalMs(5)).toBe(300_000);
    expect(followUpIntervalMs(7)).toBe(60_000);
  });

  test('a stored value is read as one of the offered choices', () => {
    expect(followUpMinutesFromSettings(0)).toBe(0);
    expect(followUpMinutesFromSettings('5')).toBe(5);
    expect(followUpMinutesFromSettings(null)).toBe(1);
    expect(followUpMinutesFromSettings(9)).toBe(1);
  });

  test('off never collects and never schedules a wake-up', () => {
    const reminders = new FollowUpReminders(0);
    reminders.sync([approval(7)], 0);
    expect(reminders.enabled).toBe(false);
    expect(reminders.collect(600_000, awake)).toEqual([]);
    expect(reminders.nextDue(0, awake)).toBeNull();
  });

  test('changing the setting restarts every wait without a wall clock', () => {
    const reminders = new FollowUpReminders(1);
    reminders.sync([approval(7)], 0);
    reminders.configure(2, 0);
    expect(reminders.collect(60_000, awake)).toEqual([]);
    expect(reminders.collect(120_000, awake)).toHaveLength(1);
  });
});
