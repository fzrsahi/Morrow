import { describe, expect, test } from 'bun:test';
import { MASCOT_SIZES, resolveMascot } from './mascots';

// Mirrors MascotView.swift + SessionSnapshot.normalizedSupportedSource: the
// expected values below are taken from the Swift source, not from the table
// under test.
describe('resolveMascot — macOS source aliases', () => {
  test('cursor aliases share the Cursor mascot', () => {
    for (const source of ['cursor', 'cursor-cli', 'cursor-agent', 'cursoragent', 'cursorcli']) {
      expect(resolveMascot(source)).toBe('cursor');
    }
  });

  test('aliases that share a mascot resolve to that mascot', () => {
    const expected: Record<string, string> = {
      factory: 'factory',
      droid: 'factory',
      'google-antigravity': 'gemini',
      agy: 'gemini',
      'traecn': 'trae',
      traecli: 'trae',
      'qoder-cli': 'qoder',
      qoderwork: 'qoder',
      codybuddycn: 'codebuddy',
      omp: 'pi',
      'aiwork-cli': 'aiwork',
      dsh: 'claude',
      zcode: 'claude',
      clawdbot: 'openclaw',
      minimax: 'minimax',
    };
    for (const [source, mascot] of Object.entries(expected)) {
      expect(resolveMascot(source), source).toBe(mascot);
    }
  });

  test('unknown and empty sources fall back to Clawd', () => {
    for (const source of ['unknown', '', '  ', 'some-future-cli', null, undefined]) {
      expect(resolveMascot(source)).toBe('claude');
    }
  });

  test('normalises case and surrounding whitespace like the macOS normalizer', () => {
    expect(resolveMascot('  CURSOR-CLI \n')).toBe('cursor');
    expect(resolveMascot('QoderWork')).toBe('qoder');
  });

  test('the three panel sizes match the canvas contract', () => {
    expect(MASCOT_SIZES).toEqual({ collapsed: 21, card: 28, compact: 15 });
  });
});
