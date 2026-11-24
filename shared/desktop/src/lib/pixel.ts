// Render the Swift-extracted 5x7 font and 9x9 icon bitmaps as unit rects.
// The SVG uses shape-rendering="crispEdges" (set by the components) so no
// anti-aliasing blurs a pixel. Sizes are snapped to whole device pixels.

import FONT from './pixel-font.json';
import ICONS from './pixel-icons.json';

export { FONT, ICONS };

// The JSON maps are typed by their literal keys, so a lookup by the character
// or icon name a caller passes has to go through a plain index signature.
const FONT_BY_CHAR: Record<string, string | undefined> = FONT;
const ICON_BY_NAME: Record<string, { w: number; h: number; d: string } | undefined> = ICONS;

export type Rect = { x: number; y: number };
export type GlyphGrid = { rects: Rect[]; width: number; height: number };

const GLYPH_WIDTH = 5;
const GLYPH_HEIGHT = 7;
const ADVANCE = 6;
const UNKNOWN_ADVANCE = 3;

function rectsFrom(bits: string, width: number, height: number): Rect[] {
  const rects: Rect[] = [];
  for (let row = 0; row < height; row++) {
    for (let col = 0; col < width; col++) {
      if (bits[row * width + col] === '1') rects.push({ x: col, y: row });
    }
  }
  return rects;
}

/** x advances by 6 per glyph, 3 for a glyph that is not in the font. */
export function pixelTextRects(text: string, max?: number): GlyphGrid {
  let chars = [...String(text).toUpperCase()];
  if (max && chars.length > max) {
    chars = chars.slice(0, max > 2 ? max - 2 : max).concat(['.', '.']);
  }
  let x = 0;
  const rects: Rect[] = [];
  for (const char of chars) {
    const bits = FONT_BY_CHAR[char];
    if (!bits) {
      x += UNKNOWN_ADVANCE;
      continue;
    }
    for (const rect of rectsFrom(bits, GLYPH_WIDTH, GLYPH_HEIGHT)) {
      rects.push({ x: x + rect.x, y: rect.y });
    }
    x += ADVANCE;
  }
  return { rects, width: Math.max(x - 1, 1), height: GLYPH_HEIGHT };
}

export function pixelIconRects(name: string): GlyphGrid {
  const icon = ICON_BY_NAME[name] ?? ICONS.agents;
  return {
    rects: rectsFrom(icon.d, icon.w, icon.h),
    width: icon.w,
    height: icon.h,
  };
}

/** Snap a logical CSS size so that size * devicePixelRatio is a whole number. */
export function snapPixelSize(logical: number, devicePixelRatio: number): number {
  const dpr = devicePixelRatio > 0 ? devicePixelRatio : 1;
  return Math.round(logical * dpr) / dpr;
}
