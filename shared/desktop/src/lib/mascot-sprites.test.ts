import { describe, expect, test } from 'bun:test';
import manifest from './mascot-sprites.json';
import { spriteFor } from './mascot-sprites';
import { resolveMascot } from './mascots';
import type { MascotStatus } from './mascots';

const STATUSES: MascotStatus[] = ['idle', 'working', 'waitingApproval', 'waitingQuestion'];

describe('exported sprite sheets', () => {
  test('every mascot has a sheet for every macOS status, and every file exists', async () => {
    for (const [mascot, entry] of Object.entries(manifest.mascots)) {
      for (const status of STATUSES) {
        const sheet = entry[status];
        expect(sheet, `${mascot}/${status}`).toBeDefined();
        expect(sheet.frames, `${mascot}/${status}`).toBeGreaterThan(0);
        expect(sheet.file, `${mascot}/${status}`).toBe(`${mascot}-${status}.png`);
        const exists = await Bun.file(new URL(`./assets/mascots/${sheet.file}`, import.meta.url)).exists();
        expect(exists, sheet.file).toBe(true);
      }
    }
  });

  test('an active sheet is 20 fps and an idle sheet is 8 fps', () => {
    expect(manifest.fps).toEqual({ idle: 8, active: 20 });
    expect(manifest.frameSize).toBe(64);
    expect(spriteFor('claude', 'working').fps).toBe(20);
    expect(spriteFor('claude', 'idle').fps).toBe(8);
    expect(spriteFor('claude', 'waitingApproval').fps).toBe(20);
  });

  test('spriteFor routes source aliases to the right sheet', () => {
    const working = spriteFor('cursor-cli', 'working');
    expect(working.mascot).toBe('cursor');
    expect(working.sheet.file).toBe('cursor-working.png');
    expect(working.sheet.frames).toBe(24); // 1.2 s × 20 fps, from the macOS GIF spec
    expect(spriteFor('factory', 'working').mascot).toBe('factory');
    expect(spriteFor('unlisted-cli', 'idle').mascot).toBe('claude');
  });

  test('every resolvable source has a sprite in the manifest', () => {
    for (const source of ['claude', 'cursor', 'cursor-cli', 'factory', 'droid', 'qoderwork',
      'google-antigravity', 'omp', 'dsh', 'minimax', 'unknown']) {
      expect(manifest.mascots[resolveMascot(source)], source).toBeDefined();
    }
  });
});
