import { describe, expect, test } from 'bun:test';
import { COLLAPSED, MOTION, PALETTE, typeScale } from './tokens';

describe('Pixel16 design tokens', () => {
  test('palette is copied 1:1 from the canvas reference', () => {
    expect(PALETTE).toMatchObject({
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
      deny: '#EB6161',
      link: '#73B8FF',
      permissionTag: '#FF9933',
      questionBlue: '#66B3FF',
      diffAdd: '#4DD966',
    });
  });

  test('type scale matches canvas.js scale(base)', () => {
    expect(typeScale(11)).toEqual({
      title: 12,
      body: 11,
      secondary: 10,
      caption: 9,
      badge: 9,
      button: 12,
      link: 10.5,
    });
    expect(typeScale(16)).toEqual({
      title: 17,
      body: 16,
      secondary: 15,
      caption: 14,
      badge: 14,
      button: 15,
      link: 15.5,
    });
  });

  test('collapsed bar metrics match #collapsed-quiet', () => {
    expect(COLLAPSED.height).toBe(34);
    expect(COLLAPSED.mascot).toBe(21);
    expect(COLLAPSED.minWidth).toBe(240);
  });

  test('motion tokens match NotchAnimation / NotchHoverInteraction', () => {
    expect(MOTION.open).toEqual({ response: 0.42, damping: 0.82 });
    expect(MOTION.close).toEqual({ response: 0.38, damping: 1 });
    expect(MOTION.micro).toBe(0.12);
    expect(MOTION.prehover).toEqual({ duration: 0.21, widthDelta: 7, scale: 1.004 });
    expect(MOTION.dwell).toBe(0.5);
  });
});
