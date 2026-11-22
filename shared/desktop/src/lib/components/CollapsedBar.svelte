<script lang="ts">
  // #180 idle, empty and prehover (#collapsed-quiet); #183 adds the working
  // bar (#collapsed-working): thinking dots or a tool label coloured by the
  // canvas table, plus the workspace dock of the active session.
  import Mascot from './Mascot.svelte';
  import PixelText from './PixelText.svelte';
  import PixelIcon from './PixelIcon.svelte';
  import { COLLAPSED, PALETTE } from '../tokens';
  import { mascotStatus, middleTruncate, toolColor, toolLabel } from '../session-cards';
  import type { Track } from '../music';
  import type { Session } from '../types';

  type Props = {
    sessionCount: number;
    prehover?: boolean;
    quiet?: boolean;
    /** First working/thinking session from the shell's ordered list. */
    active?: Session | null;
    /** #193: currently playing track; null leaves the #183 bar untouched. */
    track?: Track | null;
  };
  let { sessionCount, prehover = false, quiet = false, active = null, track = null }: Props = $props();

  // #193: canvas #collapsed-ambient fixes the wing's place — note + title on
  // the left, spectrum on the right. CyberMusicWingView fixes the behaviour:
  // the spectrum rests when paused and animates only while playing.
  let music = $derived(track !== null);
  let song = $derived(track ? `${track.title} · ${track.artist}`.trim() : '');
  let source = $derived(active?.source ?? 'claude');
  let status = $derived(active ? mascotStatus(active.status) : 'idle');
  let toolName = $derived(active?.current_tool ?? '');
  let state = $derived(
    active ? (toolName ? 'tool' : 'thinking') : sessionCount > 0 ? 'idle' : 'empty',
  );
  let dockLabel = $derived(
    active ? middleTruncate(active.cwd_name || active.session_id, 26) : '',
  );
</script>

<div
  class="col"
  class:pre={prehover}
  class:tight={music && active !== null}
  data-state={state}
  data-music={music}
  aria-label="Morrow island, {state}"
>
  <span class="wing">
    <Mascot {source} {status} size={COLLAPSED.mascot} />
    {#if music}
      <span class="note"><PixelIcon name="music" size={1.1} color={PALETTE.cyan} /></span>
      <span class="song" class:compact={active !== null}>{song}</span>
    {/if}
    {#if state === 'thinking'}
      <span class="thinkdots"><i></i><i></i><i></i></span>
      <PixelText text="THINKING" size={1.3} color={PALETTE.cyan} />
    {:else if state === 'tool'}
      <PixelText text={toolLabel(toolName)} size={1.3} color={toolColor(toolName)} />
      <span class="tooldetail">{active?.tool_description ?? ''}</span>
    {/if}
  </span>
  <span class="gap"></span>
  <span class="wing">
    {#if music}
      <span class="spec" class:still={track?.status !== 'Playing'} aria-label="music visualizer">
        {#each Array.from({ length: 5 }) as _, i}
          <i style="animation-delay:-{(i * 0.18).toFixed(2)}s"></i>
        {/each}
      </span>
    {:else if state === 'thinking' || state === 'tool'}
      <span class="dock">
        <PixelIcon name="workspace" size={0.9} color={PALETTE.cyan} />
        <span class="t">{dockLabel}</span>
      </span>
    {:else if state === 'empty' && !quiet}
      <PixelText text="IDLE" size={1.3} color={PALETTE.textMuted} />
    {:else}
      <PixelIcon name="workspace" size={1.5} color={PALETTE.textDim} />
    {/if}
    {#if quiet}
      <!-- #197: canvas #collapsed-ambient, quiet frame. The moon is a line
           icon (no bitmap in pixel-icons.json), coloured with the #180 token. -->
      <svg
        class="moon"
        viewBox="0 0 16 16"
        width="10"
        height="10"
        fill={PALETTE.textMuted}
        aria-label="quiet hours"
        role="img"
      >
        <path d="M12.5 10.5A5 5 0 0 1 6 3.8a5 5 0 1 0 6.5 6.7z" />
      </svg>
    {/if}
  </span>
</div>

<style>
  .col {
    box-sizing: border-box;
    height: 34px;
    width: 100%;
    min-width: 240px;
    display: flex;
    align-items: center;
    padding: 0 10px;
    gap: 6px;
    transform-origin: top center;
    transition:
      transform 0.21s ease-out,
      padding 0.21s ease-out;
  }
  /* +7 pt total width, scale 1.004 anchored at the top (NotchHoverInteraction). */
  .col.pre {
    padding: 0 13.5px;
    transform: scale(1.004);
  }
  .wing {
    display: flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
  }
  .gap {
    flex: 1 0 96px;
  }
  /* The music wing shares the bar with a #183 working label: give the
     spectrum the room it needs instead of the idle 96 pt spacer. */
  .col.tight .gap {
    flex-basis: 8px;
  }
  /* #180 idle/empty states keep their 21 pt mascot; #183 reuses it working. */
  .moon {
    flex: none;
  }
  /* #193: the right wing of canvas #collapsed-ambient. The note stands in for
     Swift's album thumbnail; the 5-bar spectrum rests while paused. */
  .note {
    display: inline-flex;
  }
  .song {
    font: 9px ui-monospace, monospace;
    color: rgba(255, 255, 255, 0.7);
    max-width: 150px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .song.compact {
    max-width: 100px;
  }
  .spec {
    display: inline-flex;
    gap: 1.5px;
    align-items: flex-end;
    height: 10px;
  }
  .spec i {
    width: 2px;
    background: var(--cyan);
    animation: eq 0.9s ease-in-out infinite alternate;
  }
  .spec.still i {
    animation: none;
    height: 2px;
  }
  @keyframes eq {
    from {
      height: 2px;
    }
    to {
      height: 10px;
    }
  }
  .thinkdots {
    display: inline-flex;
    gap: 3px;
  }
  .thinkdots i {
    width: 4px;
    height: 4px;
    background: var(--cyan);
    opacity: 0.25;
    animation: td 1.05s steps(1) infinite;
  }
  .thinkdots i:nth-child(2) {
    animation-delay: 0.35s;
  }
  .thinkdots i:nth-child(3) {
    animation-delay: 0.7s;
  }
  @keyframes td {
    0%,
    33% {
      opacity: 1;
    }
    34%,
    100% {
      opacity: 0.25;
    }
  }
  .tooldetail {
    font: 9px ui-monospace, monospace;
    color: rgba(255, 255, 255, 0.7);
    max-width: 120px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dock {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2.5px 6px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.06);
    box-shadow: inset 0 0 0 0.5px rgba(255, 255, 255, 0.1);
  }
  .dock .t {
    font: 700 8px ui-monospace, monospace;
    color: rgba(255, 255, 255, 0.95);
    max-width: 64px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  @media (prefers-reduced-motion: reduce) {
    .col {
      transition: none;
    }
    .thinkdots i {
      animation: none;
      opacity: 1;
    }
  }
</style>
