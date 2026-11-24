// Frame timing for the exported sprite sheets. Mirrors macOS MascotTimeline
// (minInterval 0.05s → 20 fps, idle branches 0.12s → ≈8 fps) and
// MascotAnimationGate (a hidden/asleep/reduced-motion panel holds one frame).

import type { MascotStatus } from './mascots';

export const ACTIVE_FPS = 20;
export const IDLE_FPS = 8;

export type AnimationInput = {
  status: MascotStatus;
  visible: boolean;
  awake: boolean;
  /** Element/panel hidden independently of the window (e.g. collapsed tab). */
  hidden?: boolean;
  reduceMotion?: boolean;
};

export function shouldAnimate(input: Omit<AnimationInput, 'status'>): boolean {
  return input.visible && input.awake && !input.hidden && !input.reduceMotion;
}

export function fpsFor(input: AnimationInput): number {
  if (!shouldAnimate(input)) return 0;
  return input.status === 'idle' ? IDLE_FPS : ACTIVE_FPS;
}

/** Index into a horizontal sprite sheet for an elapsed time in milliseconds. */
export function frameAt(ms: number, fps: number, frames: number): number {
  if (fps <= 0 || frames <= 0) return 0;
  return Math.floor((Math.max(0, ms) / 1000) * fps) % frames;
}
