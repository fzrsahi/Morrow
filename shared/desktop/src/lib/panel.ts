// Panel open/close logic (#182). Pure so the shell's timing rules can be
// tested without a window: the shape sent to Windows, when the OS window may
// be resized, the pointer-leave dwell and the quit confirmation.

import { MOTION } from './tokens';

export type PanelVariant = 'A' | 'B' | 'C';
/** The two Swift `IslandDisplayMode` raw values, and nothing else. */
export type DisplayMode = 'notch' | 'dynamicIsland';
/** #465: the shell now exposes the prayer card as its own tab as well. */
export type PanelTab = 'agents' | 'workspace' | 'prayer' | 'music';
export type SessionFilter = 'all' | 'sta' | 'agt';
/** #185 keeps #183's comfortable cards reachable; compact is the default. */
export type SessionDensity = 'compact' | 'comfortable';
export type Size = { width: number; height: number };

export type IslandShape = Size & {
  topLeft: number;
  topRight: number;
  bottomLeft: number;
  bottomRight: number;
  shoulders: number;
};

export type Transition = { durationMs: number; overshoots: boolean; css: string };

export const COLLAPSED_SIZE: Size = { width: 240, height: 34 };
export const OPEN_WIDTH = 680;
export const OPEN_BOTTOM_RADIUS = 24;
export const COLLAPSED_BOTTOM_RADIUS = 12;
export const COLLAPSE_DELAY_MS = MOTION.dwell * 1000;
export const QUIT_CONFIRM_MS = 3000;
/** #196: the collapsed bar needs 4 px before a pointer move is a drag, so a
 *  click still reaches the button and opens the panel. */
export const DRAG_THRESHOLD_PX = 4;
/** #196: an island returned within this radius of home is top-centre again. */
export const SNAP_HOME_PX = 24;

/**
 * Swift has two island presentations (`IslandDisplayMode.swift`): `.notch` is
 * a docked shell with concave upper ears, `.dynamicIsland` is a detached
 * floating pill. Canvas C is the former, canvas B the latter. Canvas A (edge
 * tab) is the legacy Svelte shell and has no Swift counterpart, so it is not
 * selectable as a display mode.
 */
export function variantFromDisplayMode(mode: DisplayMode): PanelVariant {
  return mode === 'notch' ? 'C' : 'B';
}

/** An explicit `?variant=` is a maintainer debug override; otherwise the saved
 *  Swift display mode decides. Windows/Linux have no hardware notch, so the
 *  default is Dynamic Island (B), which also clears the GNOME top bar. */
export function resolveVariant(
  query: string | null | undefined,
  mode: DisplayMode = 'dynamicIsland',
): PanelVariant {
  const value = new URLSearchParams(query ?? '').get('variant');
  if (value === 'B' || value === 'C') return value;
  return variantFromDisplayMode(mode);
}

/** The Windows hit-test shape travels with the size in one `set_island_size`. */
export function islandShape(
  variant: PanelVariant,
  expanded: boolean,
  width: number,
  height: number,
): IslandShape {
  const limit = Math.max(0, height / 2);
  const clamp = (r: number) => Math.max(0, Math.min(r, limit));
  if (variant === 'B') {
    const radius = clamp(expanded ? OPEN_BOTTOM_RADIUS : height / 2);
    return {
      width,
      height,
      topLeft: radius,
      topRight: radius,
      bottomLeft: radius,
      bottomRight: radius,
      shoulders: 0,
    };
  }
  const bottom = clamp(expanded ? OPEN_BOTTOM_RADIUS : COLLAPSED_BOTTOM_RADIUS);
  return {
    width,
    height,
    topLeft: 0,
    topRight: 0,
    bottomLeft: bottom,
    bottomRight: bottom,
    shoulders: variant === 'C' ? (expanded ? 14 : 10) : 0,
  };
}

/** Mirror of `morrow_platform::panel::contains_shape`, used by the tests. */
export function containsPoint(shape: IslandShape, x: number, y: number): boolean {
  if (x < 0 || y < 0 || x >= shape.width || y >= shape.height) return false;
  if (
    shape.shoulders > 0 &&
    y < shape.shoulders &&
    (x < shape.shoulders || x >= shape.width - shape.shoulders)
  ) {
    return false;
  }
  const corners: Array<[boolean, boolean, number, number, number]> = [
    [x < shape.topLeft, y < shape.topLeft, shape.topLeft, shape.topLeft, shape.topLeft],
    [
      x >= shape.width - shape.topRight,
      y < shape.topRight,
      shape.topRight,
      shape.width - shape.topRight,
      shape.topRight,
    ],
    [
      x < shape.bottomLeft,
      y >= shape.height - shape.bottomLeft,
      shape.bottomLeft,
      shape.bottomLeft,
      shape.height - shape.bottomLeft,
    ],
    [
      x >= shape.width - shape.bottomRight,
      y >= shape.height - shape.bottomRight,
      shape.bottomRight,
      shape.width - shape.bottomRight,
      shape.height - shape.bottomRight,
    ],
  ];
  for (const [inX, inY, radius, cx, cy] of corners) {
    if (!inX || !inY || radius <= 0) continue;
    const dx = x - cx;
    const dy = y - cy;
    if (dx * dx + dy * dy > radius * radius) return false;
  }
  return true;
}

/** CSS has no spring primitive: express the spring's response and damping as
 *  an eased duration. Damping 1 lands without overshoot, < 1 overshoots. */
export function springTransition(response: number, damping: number, speed = 1): Transition {
  const rate = speed > 0 ? speed : 1;
  const overshoots = damping < 1;
  const y1 = overshoots ? 1 + Math.max(0, 1 - damping) * 1.2 : 1;
  return {
    durationMs: Math.round((response * 1000) / rate),
    overshoots,
    css: `cubic-bezier(0.3, ${y1.toFixed(3)}, 0.5, 1)`,
  };
}

export const OPEN_TRANSITION = springTransition(MOTION.open.response, MOTION.open.damping);
export const CLOSE_TRANSITION = springTransition(MOTION.close.response, MOTION.close.damping);

export type FitPhase = 'collapsed' | 'opening' | 'open' | 'closing';

/** Plan §6: resize to the final size *before* the open animation, then only
 *  shrink *after* the close animation. Layout changes inside either animation
 *  are ignored, so a transition costs at most two OS resizes. */
export class IslandFit {
  phase: FitPhase = 'collapsed';
  private measured: Size | null = null;

  open(final: Size): Size[] {
    if (this.phase !== 'collapsed') return [];
    this.phase = 'opening';
    this.measured = final;
    return [final];
  }

  opened(): void {
    if (this.phase === 'opening') this.phase = 'open';
  }

  close(): Size[] {
    if (this.phase !== 'open') return [];
    this.phase = 'closing';
    return [];
  }

  closed(collapsed: Size): Size[] {
    if (this.phase !== 'closing') return [];
    this.phase = 'collapsed';
    this.measured = collapsed;
    return [collapsed];
  }

  layout(size: Size): Size[] {
    if (this.phase !== 'open') return [];
    if (this.measured?.width === size.width && this.measured?.height === size.height) return [];
    this.measured = size;
    return [size];
  }
}

/** The panel stays open while an approval or question card is waiting. */
export class CollapseGate {
  private leftAt: number | null = null;

  enter(): void {
    this.leftAt = null;
  }

  leave(now: number): void {
    this.leftAt = now;
  }

  shouldCollapse(now: number, blocked: boolean, dwell = COLLAPSE_DELAY_MS): boolean {
    return !blocked && this.leftAt !== null && now - this.leftAt >= dwell;
  }
}

export type Point = { x: number; y: number };
export type Offset = Point;

/** Decides whether a pointer gesture on the collapsed bar is a click or a drag.
 *  The DOM forwards pointer events; the threshold stays here so it can be
 *  tested without a window. Distances are in CSS (logical) pixels. */
export class DragGate {
  private start: Point | null = null;
  private active = false;

  get dragging(): boolean {
    return this.active;
  }

  down(x: number, y: number): void {
    this.start = { x, y };
    this.active = false;
  }

  /** Returns true once the gesture has crossed the threshold. */
  move(x: number, y: number): boolean {
    if (this.start === null) return false;
    if (!this.active && Math.hypot(x - this.start.x, y - this.start.y) >= DRAG_THRESHOLD_PX) {
      this.active = true;
    }
    return this.active;
  }

  /** The pointer movement since pointerdown, in logical pixels. */
  delta(x: number, y: number): Offset {
    if (this.start === null) return { x: 0, y: 0 };
    return { x: x - this.start.x, y: y - this.start.y };
  }

  /** Returns true when the gesture was a drag, so the click must be swallowed. */
  up(): boolean {
    const wasDrag = this.active;
    this.start = null;
    this.active = false;
    return wasDrag;
  }
}

/** A small offset is the user failing to land exactly on home, not a new spot. */
export function snapOffset(offset: Offset, radius = SNAP_HOME_PX): Offset {
  return Math.hypot(offset.x, offset.y) <= radius ? { x: 0, y: 0 } : offset;
}

/** Power button: the first click becomes QUIT?, the second confirms within 3 s. */
export class QuitGate {
  private armedAt: number | null = null;

  get armed(): boolean {
    return this.armedAt !== null;
  }

  reset(): void {
    this.armedAt = null;
  }

  click(now: number): 'armed' | 'quit' {
    if (this.armedAt !== null && now - this.armedAt <= QUIT_CONFIRM_MS) return 'quit';
    this.armedAt = now;
    return 'armed';
  }
}
