<script lang="ts">
  // #193 — canvas section #music, matched 1:1: a 255 pt audio deck on the left
  // (label, player, gradient artwork, scrub, transport), synced lyrics on the
  // right, 114 pt tall under the #182 header. The shell owns every player call;
  // this component only renders the track it is handed and reports a click.
  import PixelIcon from './PixelIcon.svelte';
  import PixelText from './PixelText.svelte';
  import LineIcon from './LineIcon.svelte';
  import { PALETTE, type Theme } from '../tokens';
  import {
    currentLyricIndex,
    formatTime,
    progress,
    type Controls,
    type DeckState,
    type LyricLine,
    type Track,
  } from '../music';

  type Props = {
    theme?: Theme;
    track: Track | null;
    state: DeckState;
    lyrics: LyricLine[] | null;
    positionUs: number;
    enabled: Controls;
    onPlayPause: () => void;
    onNext: () => void;
    onPrevious: () => void;
    onSeek: (fraction: number) => void;
  };
  let {
    theme = 'pixel',
    track,
    state,
    lyrics,
    positionUs,
    enabled,
    onPlayPause,
    onNext,
    onPrevious,
    onSeek,
  }: Props = $props();

  const CELLS = 40;
  let none = $derived(state === 'none');
  let playing = $derived(state === 'playing');
  let player = $derived(track?.player.trim().toUpperCase() || '—');
  let title = $derived(none ? 'Nothing playing' : track?.title || 'Unknown track');
  let artist = $derived(none ? 'No MPRIS / media session' : track?.artist || '');
  let lengthUs = $derived(track?.length_us ?? null);
  let lit = $derived(Math.round(progress(positionUs, lengthUs) * CELLS));
  let fraction = $derived(Math.round(progress(positionUs, lengthUs) * 100));
  let modernSubtitle = $derived(
    none
      ? 'Start a player: Spotify, a browser tab, mpv…'
      : `${track?.artist ?? ''} · ${track?.album ?? ''}`,
  );
  let current = $derived(currentLyricIndex(lyrics ?? [], positionUs));
  let synced = $derived(playing && lyrics !== null && lyrics.length > 0);

  function scrub(event: MouseEvent) {
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const fraction = rect.width > 0 ? (event.clientX - rect.left) / rect.width : 0;
    onSeek(Math.min(1, Math.max(0, fraction)));
  }
</script>

{#if theme === 'modern'}
  <div class="mdeck" data-deck={state}>
    <div class="art" class:none={none}>
      {#if none}<LineIcon name="note" color="currentColor" size={26} />{/if}
    </div>
    <div class="mbody">
      <div class="mtitle">{title}</div>
      <div class="msub">{modernSubtitle}</div>
      {#if !none}
        <button
          type="button"
          class="mprogress"
          aria-label="Seek within track"
          disabled={!enabled.seek}
          onclick={scrub}
        ><span class="fill" style="width:{fraction}%"></span></button>
        <div class="mtimes">
          <span>{formatTime(positionUs)}</span><span>{formatTime(lengthUs ?? 0)}</span>
        </div>
      {/if}
      <div class="mtransport">
        <button class="mtb" aria-label="Previous track" disabled={!enabled.previous} onclick={onPrevious}>◀◀</button>
        <button class="mtb" aria-label={playing ? 'Pause' : 'Play'} disabled={!enabled.playPause} onclick={onPlayPause}>{playing ? '❚❚' : '▶'}</button>
        <button class="mtb" aria-label="Next track" disabled={!enabled.next} onclick={onNext}>▶▶</button>
      </div>
    </div>
  </div>
{:else}
<div class="deck" data-deck={state}>
  <div class="l">
    <div class="top">
      <PixelText text="AUDIO DECK" size={1.1} color={PALETTE.cyan} />
      <span class="player slotb">{player}</span>
    </div>
    <div class="trk">
      <!-- Track has no artwork URL and the CSP blocks remote images: the
           gradient is the contract, not a placeholder for a later field. -->
      <div class="art" class:none={none}>
        {#if none}<PixelIcon name="music" size={1.6} color={PALETTE.textDim} />{/if}
      </div>
      <div class="meta">
        <div class="ti">{title}</div>
        <div class="ar">{artist}</div>
      </div>
    </div>
    <button
      type="button"
      class="scrub"
      aria-label="Seek within track"
      disabled={!enabled.seek}
      onclick={scrub}
    >
      {#each Array.from({ length: CELLS }) as _, i}
        <i class:on={i < lit} class:ph={i === lit && !none}></i>
      {/each}
    </button>
    <div class="times">
      {#if !none}
        <span class="now">{formatTime(positionUs)}</span>
        <span class="sep">//</span>
        <span class="len">{formatTime(lengthUs ?? 0)}</span>
      {/if}
      <span class="hsp"></span>
      <span class="transport">
        <button class="tb" aria-label="Previous track" disabled={!enabled.previous} onclick={onPrevious}>
          <PixelText text="&lt;&lt;" size={1} color={none ? PALETTE.textDim : PALETTE.textWhite} />
        </button>
        <button
          class="tb pp"
          aria-label={playing ? 'Pause' : 'Play'}
          disabled={!enabled.playPause}
          onclick={onPlayPause}
        >
          {#if playing}
            <span class="pause" aria-hidden="true"><i></i><i></i></span>
          {:else}
            <PixelText text="▶" size={1} color={none ? PALETTE.textDim : PALETTE.cyan} />
          {/if}
        </button>
        <button class="tb" aria-label="Next track" disabled={!enabled.next} onclick={onNext}>
          <PixelText text="&gt;&gt;" size={1} color={none ? PALETTE.textDim : PALETTE.textWhite} />
        </button>
      </span>
    </div>
  </div>
  <div class="r">
    <div class="top">
      <PixelText text="LYRICS" size={1.1} color={PALETTE.cyan} />
      {#if synced}
        <span class="pb synced"><PixelText text="SYNCED" size={1.05} color={PALETTE.cyan} /></span>
      {/if}
    </div>
    {#if none}
      <div class="lyr msg"><PixelText text="[ NOTHING PLAYING ]" size={1.1} color="rgba(255,255,255,.4)" /></div>
    {:else if lyrics === null || lyrics.length === 0}
      <div class="lyr msg"><PixelText text="[ NO LYRIC FEED ]" size={1.1} color="rgba(255,255,255,.4)" /></div>
    {:else}
      <div class="lyr">
        {#each lyrics as line, i (i)}
          <span class:cur={i === current}>{i === current ? '▶ ' : ''}{line.text}</span>
        {/each}
      </div>
    {/if}
  </div>
</div>
{/if}

<style>
  .deck {
    box-sizing: border-box;
    display: grid;
    grid-template-columns: 255px 1fr;
    gap: 16px;
    height: 114px;
    padding: 4px 18px 16px;
  }
  .l,
  .r {
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
  }
  .r {
    gap: 4px;
  }
  .top {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .slotb {
    background: var(--slot);
    border-radius: 3px;
    box-shadow: inset 1px 1px 0 rgba(0, 0, 0, 0.7), inset -1px -1px 0 rgba(255, 255, 255, 0.08);
  }
  .player {
    font: 700 7.5px ui-monospace, monospace;
    padding: 1.5px 5px;
    color: var(--tm);
  }
  .trk {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .art {
    width: 44px;
    height: 44px;
    border-radius: 3.5px;
    flex: none;
    background: linear-gradient(135deg, #ff4d8d, #6a4cff 60%, #00c2ff);
    display: grid;
    place-items: center;
  }
  .art.none {
    background: var(--slot);
    color: var(--td);
  }
  .meta {
    min-width: 0;
  }
  .ti {
    font: 600 11px system-ui, sans-serif;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--tw);
  }
  .ar {
    font: 10px system-ui, sans-serif;
    color: rgba(255, 255, 255, 0.6);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .scrub {
    display: flex;
    gap: 2px;
    height: 5px;
    width: 258px;
    margin: 0;
    padding: 0;
    border: 0;
    background: none;
    cursor: default;
  }
  .scrub i {
    width: 4.5px;
    background: rgba(255, 255, 255, 0.12);
  }
  .scrub i.on {
    background: var(--cyan);
  }
  .scrub i.ph {
    background: #fff;
  }
  .times {
    font: 700 7.5px ui-monospace, monospace;
    display: flex;
    gap: 4px;
    align-items: center;
  }
  .now {
    color: var(--cyan);
  }
  .sep {
    color: rgba(255, 255, 255, 0.25);
  }
  .len {
    color: rgba(255, 255, 255, 0.5);
  }
  .hsp {
    flex: 1;
  }
  .transport {
    display: flex;
    gap: 4px;
    align-items: center;
  }
  .tb {
    height: 18px;
    width: 25px;
    border-radius: 2.5px;
    display: grid;
    place-items: center;
    background: var(--raised);
    border: 0;
    padding: 0;
    cursor: default;
  }
  .tb.pp {
    width: 29px;
    background: color-mix(in srgb, var(--cyan) 22%, var(--raised));
  }
  .tb:disabled {
    opacity: 0.45;
  }
  .pause {
    display: inline-flex;
    gap: 2px;
  }
  .pause i {
    display: block;
    width: 2px;
    height: 8px;
    background: var(--cyan);
  }
  .pb {
    display: inline-flex;
    align-items: center;
    padding: 2.5px 5px;
    border-radius: 2.5px;
    background: color-mix(in srgb, var(--cyan) 12%, transparent);
  }
  .lyr {
    height: 84px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px 0;
    font: 500 10px system-ui, sans-serif;
    color: rgba(255, 255, 255, 0.42);
    -webkit-mask: linear-gradient(transparent, #000 25%, #000 75%, transparent);
    mask: linear-gradient(transparent, #000 25%, #000 75%, transparent);
  }
  .lyr span {
    white-space: nowrap;
  }
  .lyr .cur {
    color: var(--cyan);
    text-shadow: 0 0 5px rgba(0, 245, 212, 0.65);
    font-weight: 600;
  }
  .lyr.msg {
    justify-content: center;
    -webkit-mask: none;
    mask: none;
  }

  /* #202 — canvas.css:305-306 and components.js music()'s Modern branch. */
  .mdeck {
    display: grid;
    grid-template-columns: 86px 1fr;
    gap: 14px;
    padding: 6px 18px 16px;
  }
  .mdeck .art {
    width: 86px;
    height: 86px;
    border-radius: 14px;
  }
  .mdeck .art.none {
    color: var(--td);
  }
  .mbody {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .mtitle {
    font: 500 11.5px var(--ui, system-ui, sans-serif);
    color: var(--tw);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .msub {
    font: 10px var(--ui, system-ui, sans-serif);
    color: rgba(255, 255, 255, 0.6);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .mprogress {
    height: 4px;
    width: 100%;
    margin: 0;
    padding: 0;
    border: 0;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.18);
    overflow: hidden;
    cursor: default;
  }
  .mprogress .fill {
    display: block;
    height: 100%;
    border-radius: 2px;
    background: #fff;
  }
  .mtimes {
    display: flex;
    justify-content: space-between;
    font: 500 7.5px var(--ui, ui-monospace, monospace);
    color: rgba(255, 255, 255, 0.6);
  }
  .mtransport {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .mtb {
    padding: 0;
    border: 0;
    background: none;
    color: var(--tw);
    font: 600 11px var(--ui, system-ui, sans-serif);
    opacity: 0.9;
    cursor: default;
  }
  .mtb:disabled {
    opacity: 0.3;
  }
  :focus-visible {
    outline: 2px solid var(--cyan);
    outline-offset: -2px;
  }
</style>
