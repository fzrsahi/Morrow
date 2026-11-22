// Panel open/close behaviour (#182). These tests pin the seams the shell uses:
// the shape sent to Windows, the number of window resizes per transition, the
// pointer-leave dwell and the two-click quit confirmation.

import { describe, expect, test } from 'bun:test';
import {
  COLLAPSE_DELAY_MS,
  DRAG_THRESHOLD_PX,
  QUIT_CONFIRM_MS,
  SNAP_HOME_PX,
  CollapseGate,
  DragGate,
  IslandFit,
  QuitGate,
  containsPoint,
  islandShape,
  resolveVariant,
  snapOffset,
  springTransition,
} from './panel';
import { MOTION } from './tokens';

const OPEN = { width: 680, height: 400 };
const COLLAPSED = { width: 240, height: 34 };

describe('attachment variant (#motion)', () => {
  test('defaults to the floating pill so the GNOME clock stays visible', () => {
    expect(resolveVariant(null)).toBe('B');
    expect(resolveVariant('desk=gnome')).toBe('B');
  });

  test('variant A is only used when explicitly chosen', () => {
    expect(resolveVariant('variant=A')).toBe('A');
    expect(resolveVariant('variant=B')).toBe('B');
    expect(resolveVariant('variant=C')).toBe('C');
  });
});

describe('hit-test shape sent with the size', () => {
  test('the open panel has a square top and 24 pt bottom corners', () => {
    expect(islandShape('A', true, OPEN.width, OPEN.height)).toEqual({
      ...OPEN,
      topLeft: 0,
      topRight: 0,
      bottomLeft: 24,
      bottomRight: 24,
      shoulders: 0,
    });
  });

  test('the collapsed bar keeps the 12 pt bottom of variant A', () => {
    const shape = islandShape('A', false, COLLAPSED.width, COLLAPSED.height);
    expect(shape.bottomLeft).toBe(12);
    expect(shape.bottomRight).toBe(12);
    expect(shape.topLeft).toBe(0);
  });

  test('clicks in all four corner regions of the open panel land on it', () => {
    const shape = islandShape('A', true, OPEN.width, OPEN.height);
    expect(containsPoint(shape, 0, 0)).toBe(true);
    expect(containsPoint(shape, OPEN.width - 1, 0)).toBe(true);
    expect(containsPoint(shape, 0, OPEN.height - 25)).toBe(true);
    expect(containsPoint(shape, OPEN.width - 1, OPEN.height - 25)).toBe(true);
    expect(containsPoint(shape, 340, 200)).toBe(true);
  });

  test('only the 24 pt bottom arcs and pixels outside the island fall through', () => {
    const shape = islandShape('A', true, OPEN.width, OPEN.height);
    expect(containsPoint(shape, 0, OPEN.height - 1)).toBe(false);
    expect(containsPoint(shape, OPEN.width - 1, OPEN.height - 1)).toBe(false);
    expect(containsPoint(shape, -1, 100)).toBe(false);
    expect(containsPoint(shape, OPEN.width, 100)).toBe(false);
  });

  test('variant B is a pill and variant C carries its shoulder cut-outs', () => {
    expect(islandShape('B', false, 240, 34).topLeft).toBe(17);
    expect(islandShape('B', true, OPEN.width, OPEN.height).bottomRight).toBe(24);
    const c = islandShape('C', true, OPEN.width, OPEN.height);
    expect(c.shoulders).toBe(14);
    expect(containsPoint(c, 2, 2)).toBe(false);
    expect(containsPoint(c, 20, 2)).toBe(true);
  });
});

describe('window resizes during open and close (plan §6)', () => {
  test('open reports the final size once, before the animation starts', () => {
    const fit = new IslandFit();
    expect(fit.open(OPEN)).toEqual([OPEN]);
    expect(fit.phase).toBe('opening');
  });

  test('layout changes while opening or closing never resize the window', () => {
    const fit = new IslandFit();
    fit.open(OPEN);
    expect(fit.layout({ ...OPEN, height: 420 })).toEqual([]);
    fit.opened();
    fit.close();
    expect(fit.layout({ width: 680, height: 90 })).toEqual([]);
  });

  test('one open and one close produce exactly two window resizes', () => {
    const fit = new IslandFit();
    const resizes = [...fit.open(OPEN)];
    fit.opened();
    resizes.push(...fit.close(), ...fit.closed(COLLAPSED));
    expect(resizes).toEqual([OPEN, COLLAPSED]);
  });

  test('a content change while the panel is open resizes once', () => {
    const fit = new IslandFit();
    fit.open(OPEN);
    fit.opened();
    const taller = { ...OPEN, height: 480 };
    expect(fit.layout(taller)).toEqual([taller]);
    expect(fit.layout(taller)).toEqual([]);
  });
});

describe('collapse on pointer leave', () => {
  test('collapses 0.5 s after the pointer leaves', () => {
    const gate = new CollapseGate();
    gate.leave(1000);
    expect(gate.shouldCollapse(1000 + COLLAPSE_DELAY_MS - 1, false)).toBe(false);
    expect(gate.shouldCollapse(1000 + COLLAPSE_DELAY_MS, false)).toBe(true);
  });

  test('an approval or question card keeps the panel open', () => {
    const gate = new CollapseGate();
    gate.leave(1000);
    expect(gate.shouldCollapse(9000, true)).toBe(false);
  });

  test('re-entering before the dwell cancels the collapse', () => {
    const gate = new CollapseGate();
    gate.leave(1000);
    gate.enter();
    expect(gate.shouldCollapse(9000, false)).toBe(false);
  });
});

describe('quit confirmation', () => {
  test('the first click only arms QUIT?', () => {
    const gate = new QuitGate();
    expect(gate.click(1000)).toBe('armed');
    expect(gate.armed).toBe(true);
  });

  test('a second click within 3 s quits', () => {
    const gate = new QuitGate();
    gate.click(1000);
    expect(gate.click(1000 + QUIT_CONFIRM_MS)).toBe('quit');
  });

  test('a click after 3 s starts a new confirmation', () => {
    const gate = new QuitGate();
    gate.click(1000);
    expect(gate.click(1000 + QUIT_CONFIRM_MS + 1)).toBe('armed');
    expect(gate.click(2000 + QUIT_CONFIRM_MS + 1)).toBe('quit');
  });
});

describe('drag gesture (#196)', () => {
  test('a pointer that moves less than 4 px never becomes a drag', () => {
    const drag = new DragGate();
    drag.down(100, 100);
    expect(drag.move(103, 100)).toBe(false);
    expect(drag.dragging).toBe(false);
    // A click leaves the collapsed bar alone and still opens the panel.
    expect(drag.up()).toBe(false);
  });

  test('movement reaches the drag threshold at exactly 4 px', () => {
    const drag = new DragGate();
    drag.down(100, 100);
    expect(DRAG_THRESHOLD_PX).toBe(4);
    expect(drag.move(100 + DRAG_THRESHOLD_PX, 100)).toBe(true);
    expect(drag.dragging).toBe(true);
    expect(drag.up()).toBe(true);
  });

  test('a finished drag resets the gate for the next gesture', () => {
    const drag = new DragGate();
    drag.down(100, 100);
    drag.move(140, 100);
    expect(drag.up()).toBe(true);
    expect(drag.dragging).toBe(false);
    drag.down(200, 200);
    expect(drag.move(201, 200)).toBe(false);
    expect(drag.up()).toBe(false);
  });

  test('the gate reports the pointer delta for the live window move', () => {
    const drag = new DragGate();
    drag.down(100, 100);
    drag.move(112, 94);
    expect(drag.delta(112, 94)).toEqual({ x: 12, y: -6 });
    drag.up();
    expect(drag.delta(112, 94)).toEqual({ x: 0, y: 0 });
  });

  test('an offset within 24 px of home snaps back to top-centre', () => {
    expect(SNAP_HOME_PX).toBe(24);
    expect(snapOffset({ x: 20, y: 0 })).toEqual({ x: 0, y: 0 });
    expect(snapOffset({ x: 16, y: 16 })).toEqual({ x: 0, y: 0 });
  });

  test('an offset outside 24 px stays where the user left it', () => {
    expect(snapOffset({ x: 24.1, y: 0 })).toEqual({ x: 24.1, y: 0 });
    expect(snapOffset({ x: 18, y: 18 })).toEqual({ x: 18, y: 18 });
  });
});

describe('spring motion', () => {
  test('uses the open and close springs from the design tokens', () => {
    const open = springTransition(MOTION.open.response, MOTION.open.damping);
    const close = springTransition(MOTION.close.response, MOTION.close.damping);
    expect(open.durationMs).toBe(420);
    expect(close.durationMs).toBe(380);
    expect(open.overshoots).toBe(true);
    expect(close.overshoots).toBe(false);
  });

  test('the speed setting divides both springs', () => {
    expect(springTransition(MOTION.open.response, MOTION.open.damping, 2).durationMs).toBe(210);
  });
});
