// Design tokens for the Windows/Linux island. Values come from the SwiftUI
// sources via the prototype canvas (§ #tokens); keep them in sync with
// shared/desktop/prototype/island-states/canvas.css.

export const PALETTE = {
  chassis: '#0D121A',
  surface: '#141C26',
  surfaceRaised: '#1F2938',
  slot: '#080A0F',
  cyan: '#00F5D4',
  amber: '#FFB833',
  red: '#F25959',
  textWhite: '#F2F7FF',
  textMuted: '#99ADC7',
  textDim: '#617085',
  allow: '#29803D',
  allowHover: '#2B8742',
  deny: '#EB6161',
  link: '#73B8FF',
  permissionTag: '#FF9933',
  questionBlue: '#66B3FF',
  diffAdd: '#4DD966',
  bash: '#66FF80',
  edit: '#80B3FF',
  read: '#E6CC66',
  grep: '#CC99FF',
  agent: '#FF9966',
} as const;

/** #202 — the two panel themes. Pixel stays the default. */
export type Theme = 'pixel' | 'modern';

/** NotchPanelView.swift:4432-4440 — the Modern Clean card's status colours. */
export const MODERN_PALETTE = {
  needsYou: '#FFB833',
  working: '#00F5D4',
  thinking: '#00F5D4',
  done: '#D9D9D9',
  idle: 'rgba(255,255,255,.55)',
  stopped: '#FF8C66',
  error: '#FF6170',
  subagent: '#A68CF2',
} as const;

export type TypeScale = {
  title: number;
  body: number;
  secondary: number;
  caption: number;
  badge: number;
  button: number;
  link: number;
};

// NotchPanelView.swift:4388-4416; canvas.js scale(base).
export function typeScale(base = 11): TypeScale {
  return {
    title: base + 1,
    body: base,
    secondary: base - 1,
    caption: Math.max(9, base - 2),
    badge: Math.max(8.5, base - 2),
    button: Math.min(base + 1, 15),
    link: base - 0.5,
  };
}

export const COLLAPSED = {
  height: 34,
  mascot: 21,
  minWidth: 240,
} as const;

export const MOTION = {
  open: { response: 0.42, damping: 0.82 },
  close: { response: 0.38, damping: 1.0 },
  micro: 0.12,
  prehover: { duration: 0.21, widthDelta: 7, scale: 1.004 },
  dwell: 0.5,
} as const;

export function cssVariables(base = 11, theme: Theme = 'pixel'): Record<string, string> {
  const t = typeScale(base);
  const vars: Record<string, string> = {
    '--chassis': PALETTE.chassis,
    '--surface': PALETTE.surface,
    '--raised': PALETTE.surfaceRaised,
    '--slot': PALETTE.slot,
    '--cyan': PALETTE.cyan,
    '--amber': PALETTE.amber,
    '--red': PALETTE.red,
    '--tw': PALETTE.textWhite,
    '--tm': PALETTE.textMuted,
    '--td': PALETTE.textDim,
    '--allow': PALETTE.allow,
    '--deny': PALETTE.deny,
    '--link': PALETTE.link,
    '--tag': PALETTE.permissionTag,
    '--qblue': PALETTE.questionBlue,
    '--green': PALETTE.diffAdd,
    '--f': `${t.body}px`,
    '--f-title': `${t.title}px`,
    '--f-sec': `${t.secondary}px`,
    '--f-cap': `${t.caption}px`,
    '--f-badge': `${t.badge}px`,
    '--f-btn': `${t.button}px`,
    '--f-link': `${t.link}px`,
    '--ui': theme === 'modern' ? 'system-ui, sans-serif' : 'ui-monospace, monospace',
  };
  if (theme === 'modern') {
    // The Modern Clean card swaps the retro status words for title-case chips;
    // these are the only palette values the canvas changes (#180 keeps the rest).
    vars['--subagent'] = MODERN_PALETTE.subagent;
    vars['--s-idle'] = MODERN_PALETTE.idle;
    vars['--s-stopped'] = MODERN_PALETTE.stopped;
    vars['--s-error'] = MODERN_PALETTE.error;
  }
  return vars;
}
