import { describe, expect, test } from 'bun:test';
import { cueLabel } from './sound';

describe('the Sound page labels (#241)', () => {
  test('the cues the policy speaks in read as sentences', () => {
    expect(cueLabel('SessionStart')).toBe('Session start');
    expect(cueLabel('PermissionRequest')).toBe('Approval needed');
    expect(cueLabel('StopFailure')).toBe('Turn failed');
    expect(cueLabel('AppLaunch')).toBe('App launch');
  });

  test('a cue the map has not met yet is its own name, not a blank row', () => {
    expect(cueLabel('SomeFutureCue')).toBe('SomeFutureCue');
    expect(cueLabel('')).toBe('');
  });
});
