// #465 — the 33-entry Mascots gallery from `SettingsView.swift:3155-3189`.
// Names, sources, descriptions and colours are copied verbatim: the gallery
// must list the same bodies the macOS page lists, in the same order. The
// sprite a row draws still comes from `resolveMascot`, so a source with no
// ported asset falls back to Clawd exactly as the island does.

import { resolveMascot } from './mascots';

export type MascotGalleryEntry = {
  name: string;
  source: string;
  desc: string;
  color: string;
};

export const MASCOT_GALLERY: readonly MascotGalleryEntry[] = [
  { name: 'Clawd', source: 'claude', desc: 'Claude Code', color: '#de8890' },
  { name: 'Dex', source: 'codex', desc: 'Codex (OpenAI)', color: '#ebebed' },
  { name: 'Grok', source: 'grok', desc: 'Grok CLI', color: '#ffffff' },
  { name: 'Gemini', source: 'gemini', desc: 'Gemini CLI', color: '#4796e4' },
  { name: 'CursorBot', source: 'cursor', desc: 'Cursor', color: '#f54f00' },
  { name: 'TraeBot', source: 'trae', desc: 'Trae', color: '#f54f00' },
  { name: 'TraeCNBot', source: 'traecn', desc: 'Trae CN', color: '#f54f00' },
  { name: 'CopilotBot', source: 'copilot', desc: 'GitHub Copilot', color: '#59bff2' },
  { name: 'QoderBot', source: 'qoder', desc: 'Qoder', color: '#2adb5c' },
  { name: 'QoderBot', source: 'qoderwork', desc: 'QoderWork', color: '#2adb5c' },
  { name: 'Droid', source: 'droid', desc: 'Factory', color: '#d56a26' },
  { name: 'Buddy', source: 'codebuddy', desc: 'CodeBuddy', color: '#6c4dff' },
  { name: 'BuddyCN', source: 'codybuddycn', desc: 'CodyBuddyCN', color: '#6c4dff' },
  { name: 'StepFun', source: 'stepfun', desc: 'StepFun', color: '#6c4dff' },
  { name: 'AntiGravity', source: 'antigravity', desc: 'AntiGravity', color: '#6c4dff' },
  { name: 'WorkBuddy', source: 'workbuddy', desc: 'WorkBuddy', color: '#7961de' },
  { name: 'Hermes', source: 'hermes', desc: 'Hermes', color: '#6c4dff' },
  { name: 'Molty', source: 'openclaw', desc: 'OpenClaw', color: '#ed5c3d' },
  { name: 'QwenBot', source: 'qwen', desc: 'Qwen Code', color: '#7c3aed' },
  { name: 'KimiBot', source: 'kimi', desc: 'Kimi Code CLI', color: '#4a8fff' },
  { name: 'Kiro', source: 'kiro', desc: 'Kiro', color: '#9e73ff' },
  { name: 'MiniMax', source: 'minimax', desc: 'MiniMax Code CLI', color: '#ff5c4d' },
  { name: 'Pi', source: 'pi', desc: 'Pi', color: '#8c6ef2' },
  { name: 'Oh My Pi', source: 'omp', desc: 'Oh My Pi', color: '#8c6ef2' },
  { name: 'OpBot', source: 'opencode', desc: 'OpenCode', color: '#8c8c92' },
  { name: 'MiMoBot', source: 'mimo', desc: 'MiMo Code / Xiaomi MiMo', color: '#ff6900' },
  { name: 'ClineBot', source: 'cline', desc: 'Cline', color: '#00b37d' },
  { name: 'Gemini', source: 'google-antigravity', desc: 'Google Antigravity', color: '#4796e4' },
  { name: 'AiWorkBot', source: 'aiwork', desc: 'AiWork', color: '#1fb847' },
  { name: 'AiWorkBot', source: 'aiwork-cli', desc: 'AiWork CLI', color: '#1f8ce6' },
  { name: 'Neko', source: 'neko', desc: 'Pixel Cyber Cat', color: '#ff73b3' },
  { name: 'Mecha', source: 'mecha', desc: 'Cyber Titan Mecha', color: '#00f5d4' },
  { name: 'Ghost', source: 'ghost', desc: '8-Bit Pacman Ghost', color: '#d9d9fa' },
];

export const MASCOT_KEYS = {
  speed: 'mascotSpeed',
  defaultSource: 'defaultSource',
} as const;

export const DEFAULT_MASCOT_SPEED = 100;
export const DEFAULT_MASCOT_SOURCE = 'claude';

/** Swift's slider: 0–300 in 25-point steps, 0 written as `speed_off`. */
export function clampMascotSpeed(value: number): number {
  if (!Number.isFinite(value)) return DEFAULT_MASCOT_SPEED;
  return Math.max(0, Math.min(300, Math.round(value / 25) * 25));
}

export function mascotSpeedLabel(percent: number, offLabel = 'Off'): string {
  const speed = clampMascotSpeed(percent);
  return speed === 0 ? offLabel : `${(speed / 100).toFixed(1)}×`;
}

export function mascotFromSettings(value: unknown): { speed: number; source: string } {
  const stored = value !== null && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
  const rawSpeed = stored[MASCOT_KEYS.speed];
  const rawSource = stored[MASCOT_KEYS.defaultSource];
  return {
    speed: typeof rawSpeed === 'number' ? clampMascotSpeed(rawSpeed) : DEFAULT_MASCOT_SPEED,
    source: typeof rawSource === 'string' && rawSource.trim() ? rawSource : DEFAULT_MASCOT_SOURCE,
  };
}

/** The sprite a gallery row actually draws, so a missing asset falls back. */
export function gallerySprite(entry: MascotGalleryEntry): string {
  return resolveMascot(entry.source);
}
