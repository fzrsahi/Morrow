<script lang="ts">
  // #180 idle, empty and prehover (#collapsed-quiet); #183 adds the working
  // bar (#collapsed-working): thinking dots or a tool label coloured by the
  // canvas table, plus the workspace dock of the active session.
  import Mascot from './Mascot.svelte';
  import PixelText from './PixelText.svelte';
  import PixelIcon from './PixelIcon.svelte';
  import { COLLAPSED, PALETTE } from '../tokens';
  import { mascotStatus, middleTruncate, toolColor, toolLabel } from '../session-cards';
  import type { Session } from '../types';

  type Props = {
    sessionCount: number;
    prehover?: boolean;
    quiet?: boolean;
    /** First working/thinking session from the shell's ordered list. */
    active?: Session | null;
    /** #191: last turn finished — canvas #collapsed-attention's cyan check. */
    glance?: boolean;
    glanceLabel?: string;
  };
  let {
    sessionCount,
    prehover = false,
    quiet = false,
    active = null,
    glance = false,
    glanceLabel = '',
  }: Props = $props();

  let source = $derived(active?.source ?? 'claude');
  let status = $derived(active ? mascotStatus(active.status) : 'idle');
  let toolName = $derived(active?.current_tool ?? '');
  let state = $derived(
    active ? (toolName ? 'tool' : 'thinking') : sessionCount > 0 ? 'idle' : 'empty',
  );
  let dockLabel = $derived(
    active || glance
      ? middleTruncate(glanceLabel || active?.cwd_name || active?.session_id || '', 26)
      : '',
  );
</script>

<div class="col" class:pre={prehover} data-state={state} aria-label="Morrow island, {state}">
  <span class="wing">
    <Mascot {source} {status} size={COLLAPSED.mascot} />
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
    {#if state === 'thinking' || state === 'tool'}
      <span class="dock">
        <PixelIcon name="workspace" size={0.9} color={PALETTE.cyan} />
        <span class="t">{dockLabel}</span>
      </span>
    {:else if glance}
      <span class="dock">
        <PixelIcon name="workspace" size={0.9} color={PALETTE.cyan} />
        <span class="t">{dockLabel}</span>
      </span>
      <svg
        class="check"
        width="13"
        height="13"
        viewBox="0 0 16 16"
        fill={PALETTE.cyan}
        aria-label="completed"
        role="img"
      >
        <circle cx="8" cy="8" r="7" />
        <path
          d="M4.8 8.3l2.2 2.2 4.2-4.6"
          stroke="#000"
          stroke-width="1.8"
          fill="none"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
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
  /* #180 idle/empty states keep their 21 pt mascot; #183 reuses it working. */
  .moon,
  .check {
    flex: none;
  }
  .check {
    filter: drop-shadow(0 0 2px var(--cyan));
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
