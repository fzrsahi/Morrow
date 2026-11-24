// #183 — pure view rules shared by the session cards and the working
// collapsed bar. Words, colours and tiers come from the canvas sections
// #list-status and #collapsed-working; the shell owns the session data.

import type { MascotStatus } from './mascots';
import { MODERN_PALETTE } from './tokens';
import type { CardStatus, Session } from './types';

export type { CardStatus } from './types';

export type StatusMeta = {
  label: string;
  color: string;
  tier: number;
  /** Only NEEDS YOU gets a filled badge (PixelRetroSessionCardView). */
  fill: boolean;
};

export const STATUS_META: Record<CardStatus, StatusMeta> = {
  needs_you: { label: 'NEEDS YOU', color: '#FFB833', tier: 0, fill: true },
  working: { label: 'WORKING', color: '#00F5D4', tier: 1, fill: false },
  thinking: { label: 'THINKING', color: '#00F5D4', tier: 1, fill: false },
  done: { label: 'DONE', color: '#D9D9D9', tier: 2, fill: false },
  idle: { label: 'IDLE', color: '#99ADC7', tier: 2, fill: false },
  stopped: { label: 'STOPPED', color: '#F25959', tier: 2, fill: false },
  error: { label: 'ERROR', color: '#F25959', tier: 2, fill: false },
};

/**
 * #202 — the Modern Clean chip in a session card (components.js ST.m / .mc).
 * Pixel keeps the shouted badge above; the modern card spells the same state
 * in title case and gives idle/stopped/error their softer canvas colours.
 */
export function modernStatus(status: CardStatus): { label: string; color: string } {
  switch (status) {
    case 'needs_you':
      return { label: 'Needs you', color: MODERN_PALETTE.needsYou };
    case 'working':
      return { label: 'Working', color: MODERN_PALETTE.working };
    case 'thinking':
      return { label: 'Thinking', color: MODERN_PALETTE.thinking };
    case 'done':
      return { label: 'Done', color: MODERN_PALETTE.done };
    case 'idle':
      return { label: 'Idle', color: MODERN_PALETTE.idle };
    case 'stopped':
      return { label: 'Stopped', color: MODERN_PALETTE.stopped };
    case 'error':
      return { label: 'Error', color: MODERN_PALETTE.error };
  }
}

export function mascotStatus(status: CardStatus): MascotStatus {
  switch (status) {
    case 'needs_you':
      return 'waitingApproval';
    case 'working':
    case 'thinking':
      return 'working';
    default:
      return 'idle';
  }
}

const TOOL_COLORS: Array<[RegExp, string]> = [
  [/^(bash|execute_command|run_command|running command)$/i, '#66FF80'],
  [/^(edit|write|apply_patch|apply_diff|editing)$/i, '#80B3FF'],
  [/^(read|reading|view_file)$/i, '#E6CC66'],
  [/^(grep|glob|mcp|searching|calling mcp)$/i, '#CC99FF'],
  [/^(agent|delegating|task)$/i, '#FF9966'],
];

/** Colour of the collapsed bar's tool label (NotchPanelView.swift:2790). */
export function toolColor(tool: string): string {
  const name = tool.trim();
  for (const [pattern, color] of TOOL_COLORS) {
    if (pattern.test(name)) return color;
  }
  return 'rgba(255,255,255,0.7)';
}

/** The compact tool word, kept in the payload's own case. */
export function toolLabel(tool: string): string {
  return tool.trim().toUpperCase();
}

/** NOW, then whole minutes/hours/days — the canvas card's time column. */
export function timeLabel(updatedAtMs: number, nowMs: number): string {
  const elapsed = Math.max(0, nowMs - updatedAtMs);
  if (elapsed < 60_000) return 'NOW';
  const minutes = Math.floor(elapsed / 60_000);
  if (minutes < 60) return `${minutes}M`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}H`;
  return `${Math.floor(hours / 24)}D`;
}

/**
 * The one line the card's reply row paints (#192). A streaming transcript
 * line wins over the reply the hook reported; everything else on the card —
 * status, tool, prompt, tier — still comes from the hook fields alone.
 */
export function liveReply(session: Session): string {
  const live = session.live_reply?.trim() ?? '';
  return live || session.last_assistant_message;
}

/** Middle-truncated label for the workspace dock (44 pt max in the canvas). */
export function middleTruncate(text: string, maxChars: number): string {
  if (text.length <= maxChars) return text;
  const keep = Math.max(0, maxChars - 2);
  const head = Math.ceil(keep / 2);
  const tail = keep - head;
  return `${text.slice(0, head)}..${tail ? text.slice(-tail) : ''}`;
}

/**
 * Tight-fit width of the collapsed bar (canvas #collapsed-quiet / -working):
 * 240 pt idle, 338 pt thinking, 369 pt with a tool, never over 460 pt. The
 * label changes are batched by state — the shell resizes only when it changes.
 */
export function collapsedWidth(active: Session | null): number {
  if (!active) return 240;
  return Math.min(460, active.current_tool ? 369 : 338);
}

/**
 * The session the collapsed bar is about: the first working/thinking one in
 * the shell's already-ordered list. Attention states (#186+) own the bar when
 * somebody needs you, so they take precedence there.
 */
export function activeSession(sessions: Session[]): Session | null {
  return (
    sessions.find((s) => s.status === 'working' || s.status === 'thinking') ?? null
  );
}

// #184 — the context elements of the canvas #list-comfortable card. The shell
// resolves the branch and the counts; these are only their drawn words.

/** Swift appends ⧉ to a linked worktree's branch (NotchPanelView.swift:4625). */
export function branchLabel(branch: string, isWorktree: boolean): string {
  if (!branch) return '';
  return isWorktree ? `${branch} ⧉` : branch;
}

/** Twelve segments: filled below the canvas' rounded share, one current. */
export function taskGauge(done: number, total: number): Array<'on' | 'cur' | 'off'> {
  if (total <= 0) return Array(12).fill('off');
  const filled = Math.round((done / total) * 12);
  return Array.from({ length: 12 }, (_, i) =>
    i < filled ? 'on' : i === filled ? 'cur' : 'off',
  );
}

export function taskLabel(done: number, total: number): string {
  return `[${done}/${total} TASKS]`;
}

/** No badge at all when the session has no subagents. */
export function subLabel(count: number): string {
  return count > 0 ? `+${count} SUB` : '';
}
