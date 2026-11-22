// #193 — the seams of the music deck. All logic is pure: the shell reads
// MPRIS/SMTC (music.rs), the deck only decides what to draw and when to ask
// again. Clock values here are plain numbers so the tick is testable without
// a desktop, a timer, or Tauri.
import { describe, expect, test } from 'bun:test';
import {
  COLLAPSED_MUSIC_WIDTH,
  COLLAPSED_NO_MUSIC_WIDTH,
  REFRESH_MS,
  collapsedWidth,
  controls,
  currentLyricIndex,
  deckState,
  formatTime,
  localPosition,
  progress,
  refreshPlan,
  seekOffsetUs,
  type LyricLine,
  type Track,
} from './music';

const playing: Track = {
  player: 'Spotify',
  status: 'Playing',
  title: 'Midnight City',
  artist: 'M83',
  album: 'Hurry Up, We’re Dreaming',
  length_us: 243_000_000,
  position_us: 107_000_000,
  can_seek: true,
  can_go_next: true,
  can_go_previous: true,
};

const lyrics: LyricLine[] = [
  { at_ms: 0, text: 'Waiting in a car' },
  { at_ms: 1000, text: 'Waiting for a ride in the dark' },
  { at_ms: 107000, text: 'The night city grows' },
  { at_ms: 120000, text: 'Look and see her eyes, they glow' },
];

describe('deck state from the shell’s track', () => {
  test('no track means nothing playing', () => {
    expect(deckState(null)).toBe('none');
  });

  test('a stopped track has nothing to show either', () => {
    expect(deckState({ ...playing, status: 'Stopped' })).toBe('none');
  });

  test('a paused track still shows its title', () => {
    expect(deckState({ ...playing, status: 'Paused' })).toBe('paused');
  });

  test('a playing track shows the synced deck', () => {
    expect(deckState(playing)).toBe('playing');
  });
});

describe('position ticks locally from the anchor', () => {
  test('a playing track advances by the elapsed time', () => {
    // 107 s at t=1000, two seconds later: 109 s, no player call in between.
    expect(localPosition(107_000_000, 'Playing', 243_000_000, 1000, 3000)).toBe(109_000_000);
  });

  test('a paused track holds its position', () => {
    expect(localPosition(107_000_000, 'Paused', 243_000_000, 1000, 3000)).toBe(107_000_000);
  });

  test('a playing track stops at the track length', () => {
    expect(localPosition(107_000_000, 'Playing', 108_000_000, 1000, 9000)).toBe(108_000_000);
  });

  test('a clock read from an earlier frame never rewinds the bar', () => {
    expect(localPosition(107_000_000, 'Playing', 243_000_000, 3000, 1000)).toBe(107_000_000);
  });

  test('an unknown length does not clamp the tick', () => {
    expect(localPosition(107_000_000, 'Playing', null, 1000, 3000)).toBe(109_000_000);
  });
});

describe('time, progress and lyric readout', () => {
  test('positions print as m:ss', () => {
    expect(formatTime(0)).toBe('0:00');
    expect(formatTime(107_000_000)).toBe('1:47');
    expect(formatTime(243_000_000)).toBe('4:03');
  });

  test('progress is a fraction of the known length', () => {
    expect(progress(107_000_000, 243_000_000)).toBeCloseTo(0.4403, 3);
  });

  test('progress is zero when the player reports no length', () => {
    expect(progress(107_000_000, null)).toBe(0);
  });

  test('the current lyric is the last line already reached', () => {
    expect(currentLyricIndex(lyrics, 106_000_000)).toBe(1);
    expect(currentLyricIndex(lyrics, 107_000_000)).toBe(2);
    expect(currentLyricIndex(lyrics, 130_000_000)).toBe(3);
  });

  test('before the first line no lyric is current', () => {
    expect(currentLyricIndex(lyrics, -1)).toBe(-1);
  });

  test('no lyric feed selects no line', () => {
    expect(currentLyricIndex([], 107_000_000)).toBe(-1);
  });
});

describe('unsupported controls are disabled', () => {
  test('a player can report no seek, next or previous', () => {
    expect(controls({ ...playing, can_seek: false, can_go_next: false, can_go_previous: false })).toEqual({
      previous: false,
      playPause: true,
      next: false,
      seek: false,
    });
  });

  test('a player that supports everything enables every control', () => {
    expect(controls(playing)).toEqual({ previous: true, playPause: true, next: true, seek: true });
  });

  test('nothing playing disables every control', () => {
    expect(controls(null)).toEqual({ previous: false, playPause: false, next: false, seek: false });
  });
});

describe('seeking by a scrub fraction', () => {
  test('a fraction of the track becomes a microsecond offset', () => {
    // Half of a 240 s track, currently at 100 s: +20 s.
    expect(seekOffsetUs({ ...playing, length_us: 240_000_000 }, 100_000_000, 0.5)).toBe(20_000_000);
  });

  test('seeking backwards gives a negative offset', () => {
    expect(seekOffsetUs({ ...playing, length_us: 240_000_000 }, 100_000_000, 0.25)).toBe(-40_000_000);
  });
});

describe('the collapsed music wing', () => {
  test('no track leaves the bar at its ordinary width', () => {
    expect(COLLAPSED_NO_MUSIC_WIDTH).toBe(240);
    expect(collapsedWidth(null)).toBe(COLLAPSED_NO_MUSIC_WIDTH);
  });

  test('a track widens the tight-fit window for the wing', () => {
    expect(COLLAPSED_MUSIC_WIDTH).toBe(292);
    expect(collapsedWidth(playing)).toBe(COLLAPSED_MUSIC_WIDTH);
  });
});

describe('the tab owns the refresh loop', () => {
  test('an open music tab refreshes every few seconds', () => {
    expect(refreshPlan('music', true)).toEqual({ active: true, intervalMs: REFRESH_MS });
  });

  test('a closed music tab does not poll', () => {
    expect(refreshPlan('music', false)).toEqual({ active: false, intervalMs: REFRESH_MS });
  });

  test('the agents tab does not poll music', () => {
    expect(refreshPlan('agents', true)).toEqual({ active: false, intervalMs: REFRESH_MS });
  });
});
