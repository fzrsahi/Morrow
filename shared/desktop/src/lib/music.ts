// #193 — pure decisions for the music deck and the collapsed music wing.
//
// The shell reads MPRIS (Linux) or SMTC (Windows) in `music.rs`; this module
// never asks a player anything. It maps the shell's `Track` DTO to a deck
// state, ticks the position from an anchor on the webview's own clock (the
// shell's `monotonic()` clock is not visible here), and says when a refresh is
// allowed. Keeping it pure makes the "no polling while closed" rule testable.

import type { PanelTab } from './panel';

/** Mirrors `morrow_core::music::Status` (serde serialises the variant name). */
export type MusicStatus = 'Playing' | 'Paused' | 'Stopped';

/** Mirrors `morrow_core::music::Track`. No artwork field: see the PR note. */
export type Track = {
  player: string;
  status: MusicStatus;
  title: string;
  artist: string;
  album: string;
  length_us: number | null;
  position_us: number;
  can_seek: boolean;
  can_go_next: boolean;
  can_go_previous: boolean;
};

/** Mirrors `morrow_core::music::LyricLine`. */
export type LyricLine = { at_ms: number; text: string };

/** The three frames of canvas section #music. */
export type DeckState = 'playing' | 'paused' | 'none';

export type Controls = {
  previous: boolean;
  playPause: boolean;
  next: boolean;
  seek: boolean;
};

export type RefreshPlan = { active: boolean; intervalMs: number };

/** Ask the player again only on a control or every few seconds (#music notes). */
export const REFRESH_MS = 3000;

/** The tight-fit window grows from 240 to 292 pt when the right wing has a
 *  track (#collapsed-ambient frame). The canvas fixes the place; Swift's
 *  CyberMusicWingView fixes what goes in it. */
export const COLLAPSED_NO_MUSIC_WIDTH = 240;
export const COLLAPSED_MUSIC_WIDTH = 292;

export function collapsedWidth(track: Track | null): number {
  return track === null ? COLLAPSED_NO_MUSIC_WIDTH : COLLAPSED_MUSIC_WIDTH;
}

export function deckState(track: Track | null): DeckState {
  if (track === null || track.status === 'Stopped') return 'none';
  return track.status === 'Playing' ? 'playing' : 'paused';
}

/** Interpolate the position on the JS clock, the same rule as Rust `Position::at`.
 *  `anchorMs` is `performance.now()` when `baseUs` was read; no IPC per frame. */
export function localPosition(
  baseUs: number,
  status: MusicStatus,
  lengthUs: number | null,
  anchorMs: number,
  nowMs: number,
): number {
  const elapsedMs = Math.max(0, nowMs - anchorMs);
  const position = Math.max(0, baseUs) + (status === 'Playing' ? elapsedMs * 1000 : 0);
  return lengthUs !== null && lengthUs > 0 ? Math.min(position, lengthUs) : position;
}

export function formatTime(positionUs: number): string {
  const seconds = Math.max(0, Math.floor(positionUs / 1_000_000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
}

export function progress(positionUs: number, lengthUs: number | null): number {
  if (lengthUs === null || lengthUs <= 0) return 0;
  return Math.min(1, Math.max(0, positionUs / lengthUs));
}

/** Index of the last line whose timestamp has passed, or -1 before the first. */
export function currentLyricIndex(lines: LyricLine[], positionUs: number): number {
  let current = -1;
  for (let i = 0; i < lines.length; i++) {
    if (lines[i].at_ms * 1000 <= positionUs) current = i;
    else break;
  }
  return current;
}

/** The lyric service is asked by artist, title and duration; album keeps two
 *  editions with the same name apart. Playback fields are not part of it. */
export function lyricKey(track: Track): string {
  return [track.artist, track.title, track.album, track.length_us ?? ''].join('\u0000');
}

/** `undefined` means "never asked"; `null` is a real answer: no lyrics exist.
 *  Both the lines and the absence are kept so re-opening the tab never reasks
 *  the service, and returning to an earlier track gets its lines straight back. */
export class LyricCache {
  private readonly answers = new Map<string, LyricLine[] | null>();

  get(track: Track): LyricLine[] | null | undefined {
    return this.answers.get(lyricKey(track));
  }

  put(track: Track, lines: LyricLine[] | null): void {
    this.answers.set(lyricKey(track), lines);
  }
}

/** Pixel offset for the line list (positive early, negative later): the active
 *  line's centre sits at the centre of the 84 pt lyric box (68 pt of content
 *  under its 8 pt padding). The box's fixed mask fades whatever runs past its
 *  top or bottom edge. */
export function lyricScrollY(
  index: number,
  lineCount: number,
  lineHeight: number,
  viewportHeight: number,
): number {
  if (index < 0 || lineCount <= 0) return 0;
  return viewportHeight / 2 - (index + 0.5) * lineHeight;
}

/** A player that cannot seek/skip reports Can*=false; show those buttons off. */
export function controls(track: Track | null): Controls {
  if (track === null || track.status === 'Stopped') {
    return { previous: false, playPause: false, next: false, seek: false };
  }
  return {
    previous: track.can_go_previous,
    playPause: true,
    next: track.can_go_next,
    seek: track.can_seek,
  };
}

/** Offset in microseconds for `music_seek` when the user clicks the scrubber. */
export function seekOffsetUs(track: Track, positionUs: number, fraction: number): number {
  const length = track.length_us ?? 0;
  return Math.round(fraction * length) - positionUs;
}

/** The refresh loop exists only while the Music tab is actually on screen. */
export function refreshPlan(tab: PanelTab, open: boolean): RefreshPlan {
  return { active: tab === 'music' && open, intervalMs: REFRESH_MS };
}
