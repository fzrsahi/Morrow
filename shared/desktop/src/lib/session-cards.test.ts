// #183 — pure view rules for session cards and the working collapsed bar.
// These are the seams the components consume: status words/colours, tool
// colours, the active session, and the short labels beside them.

import { describe, expect, test } from 'bun:test';
import {
  STATUS_META,
  activeSession,
  collapsedWidth,
  liveReply,
  mascotStatus,
  middleTruncate,
  timeLabel,
  toolColor,
} from './session-cards';
import type { Session } from './types';

const session = (over: Partial<Session> = {}): Session => ({
  session_id: 's',
  source: 'claude',
  last_event: 'PreToolUse',
  cwd_name: 'web-app',
  updated_at: 0,
  status: 'working',
  current_tool: '',
  tool_description: '',
  last_user_prompt: '',
  last_assistant_message: '',
  live_reply: '',
  model: '',
  cwd: '',
  term_app: '',
  tty_path: '',
  cli_pid: null,
  ...over,
});

describe('status words and colours (#list-status)', () => {
  test('every canvas status has its spelled-out word, colour and tier', () => {
    expect(STATUS_META.needs_you).toEqual({ label: 'NEEDS YOU', color: '#FFB833', tier: 0, fill: true });
    expect(STATUS_META.working).toEqual({ label: 'WORKING', color: '#00F5D4', tier: 1, fill: false });
    expect(STATUS_META.thinking).toEqual({ label: 'THINKING', color: '#00F5D4', tier: 1, fill: false });
    expect(STATUS_META.done).toEqual({ label: 'DONE', color: '#D9D9D9', tier: 2, fill: false });
    expect(STATUS_META.idle).toEqual({ label: 'IDLE', color: '#99ADC7', tier: 2, fill: false });
    expect(STATUS_META.stopped).toEqual({ label: 'STOPPED', color: '#F25959', tier: 2, fill: false });
    expect(STATUS_META.error).toEqual({ label: 'ERROR', color: '#F25959', tier: 2, fill: false });
  });

  test('only NEEDS YOU gets a filled badge', () => {
    expect(STATUS_META.needs_you.fill).toBe(true);
    expect(STATUS_META.working.fill).toBe(false);
  });

  test('each status maps to a mascot frame the Mascot component has', () => {
    expect(mascotStatus('needs_you')).toBe('waitingApproval');
    expect(mascotStatus('working')).toBe('working');
    expect(mascotStatus('thinking')).toBe('working');
    expect(mascotStatus('done')).toBe('idle');
    expect(mascotStatus('error')).toBe('idle');
  });
});

describe('tool label colour from the canvas table', () => {
  test('the five tool families carry their canvas colours', () => {
    expect(toolColor('Bash')).toBe('#66FF80');
    expect(toolColor('Running command')).toBe('#66FF80');
    expect(toolColor('Edit')).toBe('#80B3FF');
    expect(toolColor('Write')).toBe('#80B3FF');
    expect(toolColor('Read')).toBe('#E6CC66');
    expect(toolColor('Grep')).toBe('#CC99FF');
    expect(toolColor('MCP')).toBe('#CC99FF');
    expect(toolColor('Agent')).toBe('#FF9966');
  });

  test('an unknown tool is white at 70 percent', () => {
    expect(toolColor('Some exotic tool')).toBe('rgba(255,255,255,0.7)');
  });
});

describe('the active session of the collapsed bar (#collapsed-working)', () => {
  test('picks the first working or thinking session in the ordered list', () => {
    const sessions = [
      session({ session_id: 'needs', status: 'needs_you' }),
      session({ session_id: 'working', status: 'working', current_tool: 'Bash' }),
      session({ session_id: 'thinking', status: 'thinking' }),
    ];
    expect(activeSession(sessions)?.session_id).toBe('working');
  });

  test('no active session means the bar stays idle', () => {
    expect(
      activeSession([
        session({ status: 'needs_you' }),
        session({ status: 'done' }),
      ]),
    ).toBeNull();
    expect(activeSession([])).toBeNull();
  });
});

describe('collapsed bar width (#collapsed-working)', () => {
  test('grows from the 240 pt idle bar to the thinking and tool frames', () => {
    expect(collapsedWidth(null)).toBe(240);
    expect(collapsedWidth(session({ status: 'thinking' }))).toBe(338);
    expect(collapsedWidth(session({ status: 'working', current_tool: 'Bash' }))).toBe(369);
  });

  test('never exceeds the canvas 460 pt ceiling', () => {
    expect(collapsedWidth(session({ current_tool: 'A very long tool name' }))).toBeLessThanOrEqual(
      460,
    );
  });
});

describe('live transcript reply row (#192)', () => {
  test('the newest transcript text wins over the finished hook reply', () => {
    expect(
      liveReply(
        session({
          last_assistant_message: 'Shipped; all tests pass.',
          live_reply: 'Adding the date filter…',
        }),
      ),
    ).toBe('Adding the date filter…');
  });

  test('an empty tail falls back to the reply the hook reported', () => {
    expect(
      liveReply(session({ last_assistant_message: 'Shipped', live_reply: '' })),
    ).toBe('Shipped');
    expect(liveReply(session({ live_reply: '   ' }))).toBe('');
  });

  test('the transcript never changes the status or tool data beside the reply row', () => {
    const s = session({ live_reply: 'Still typing…', current_tool: 'Bash', status: 'working' });
    expect(liveReply(s)).toBe('Still typing…');
    expect(s.status).toBe('working');
    expect(s.current_tool).toBe('Bash');
    expect(s.last_assistant_message).toBe('');
  });
});

describe('short labels', () => {
  test('time is NOW, then minutes, hours and days', () => {
    const now = 10 * 60 * 60 * 1000;
    expect(timeLabel(now, now)).toBe('NOW');
    expect(timeLabel(now - 7 * 60 * 1000, now)).toBe('7M');
    expect(timeLabel(now - 2 * 60 * 60 * 1000, now)).toBe('2H');
    expect(timeLabel(now - 3 * 24 * 60 * 60 * 1000, now)).toBe('3D');
  });

  test('a long workspace dock label is truncated in the middle', () => {
    expect(middleTruncate('customer-onboarding-service-v2-migration', 26)).toBe(
      'customer-onb..v2-migration',
    );
    expect(middleTruncate('web-app', 26)).toBe('web-app');
  });
});
