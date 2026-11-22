import { describe, expect, test } from 'bun:test';
import { fpsFor, frameAt, shouldAnimate } from './mascot-animation';

// macOS: MascotTimeline.minInterval = 0.05s (20 fps), the idle branches use
// TimelineView(interval: 0.12) (≈8 fps), and MascotAnimationGate halts the
// timeline while the panel is hidden, the display is asleep or Reduce Motion
// is on. Expected values come from those Swift sources.
describe('fpsFor — mascot frame rate', () => {
  const shown = { visible: true, awake: true };

  test('active statuses run at 20 fps', () => {
    expect(fpsFor({ status: 'working', ...shown })).toBe(20);
    expect(fpsFor({ status: 'waitingApproval', ...shown })).toBe(20);
    expect(fpsFor({ status: 'waitingQuestion', ...shown })).toBe(20);
  });

  test('idle runs at 8 fps', () => {
    expect(fpsFor({ status: 'idle', ...shown })).toBe(8);
  });

  test('a hidden, asleep or reduced-motion panel is stopped', () => {
    expect(fpsFor({ status: 'working', visible: false, awake: true })).toBe(0);
    expect(fpsFor({ status: 'working', visible: true, awake: false })).toBe(0);
    expect(fpsFor({ status: 'working', ...shown, reduceMotion: true })).toBe(0);
    expect(fpsFor({ status: 'idle', ...shown, hidden: true })).toBe(0);
  });
});

describe('shouldAnimate — MascotAnimationGate equivalent', () => {
  test('only a visible, awake panel with motion allowed animates', () => {
    expect(shouldAnimate({ visible: true, awake: true })).toBe(true);
    expect(shouldAnimate({ visible: false, awake: true })).toBe(false);
    expect(shouldAnimate({ visible: true, awake: false })).toBe(false);
    expect(shouldAnimate({ visible: true, awake: true, reduceMotion: true })).toBe(false);
    expect(shouldAnimate({ visible: true, awake: true, hidden: true })).toBe(false);
  });
});

describe('frameAt — sprite sheet stepping', () => {
  test('steps once per frame interval and wraps', () => {
    expect(frameAt(0, 20, 24)).toBe(0);
    expect(frameAt(49, 20, 24)).toBe(0);
    expect(frameAt(50, 20, 24)).toBe(1);
    expect(frameAt(1200, 20, 24)).toBe(0);
  });

  test('idle steps at its slower rate', () => {
    expect(frameAt(124, 8, 8)).toBe(0);
    expect(frameAt(125, 8, 8)).toBe(1);
    expect(frameAt(1000, 8, 8)).toBe(0);
  });

  test('a stopped sheet holds frame zero', () => {
    expect(frameAt(4000, 0, 8)).toBe(0);
  });
});
