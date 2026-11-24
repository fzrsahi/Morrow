import { describe, expect, test } from 'bun:test';
import FONT from './pixel-font.json';
import ICONS from './pixel-icons.json';
import {
  pixelIconRects,
  pixelTextRects,
  snapPixelSize,
} from './pixel';

describe('Swift-extracted pixel bitmaps', () => {
  test('font and icon tables are the verbatim Swift extraction', () => {
    expect(FONT.A).toHaveLength(35); // 5x7
    expect(FONT['0']).toHaveLength(35);
    expect(ICONS.agents).toMatchObject({ w: 9, h: 9 });
    expect(ICONS.agents.d).toHaveLength(81);
  });

  test('text glyphs advance 6 units; unknown glyphs advance 3', () => {
    expect(pixelTextRects('AB').width).toBe(11);
    expect(pixelTextRects('AΩ').width).toBe(8); // A(6) + unknown(3) - 1; Ω is not in the Swift font
    expect(pixelTextRects('AB').rects.length).toBe(
      pixelTextRects('A').rects.length + pixelTextRects('B').rects.length,
    );
  });

  test('max truncation keeps the last two positions for dots', () => {
    const full = pixelTextRects('THINKING');
    const clipped = pixelTextRects('THINKING', 4);
    expect(clipped.width).toBeLessThan(full.width);
    expect(clipped.rects.length).toBeGreaterThan(0);
  });

  test('icons render as a 9x9 grid', () => {
    const icon = pixelIconRects('checkmark');
    expect(icon.width).toBe(9);
    expect(icon.height).toBe(9);
    expect(icon.rects.length).toBeGreaterThan(0);
  });

  test('pixel sizes snap to whole device pixels at 100%, 125% and 150%', () => {
    for (const dpr of [1, 1.25, 1.5]) {
      for (const size of [1.05, 1.15, 1.35]) {
        const snapped = snapPixelSize(size, dpr);
        expect(snapped * dpr).toBeCloseTo(Math.round(snapped * dpr), 9);
        expect(Math.abs(snapped - size)).toBeLessThanOrEqual(0.5 / dpr + 1e-9);
      }
    }
  });
});
