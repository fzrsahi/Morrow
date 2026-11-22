<script lang="ts">
  // #180 — idle, empty and prehover states of the collapsed island, matched
  // 1:1 to canvas section #collapsed-quiet. #181 uses the exported Clawd
  // sleeping sheet at 21 pt; #182 adds open/close motion.
  import Mascot from './Mascot.svelte';
  import PixelText from './PixelText.svelte';
  import PixelIcon from './PixelIcon.svelte';
  import { COLLAPSED, PALETTE } from '../tokens';

  type Props = { sessionCount: number; prehover?: boolean };
  let { sessionCount, prehover = false }: Props = $props();

  let state = $derived(sessionCount > 0 ? 'idle' : 'empty');
</script>

<div class="col" class:pre={prehover} data-state={state} aria-label="Morrow island, {state}">
  <span class="wing">
    <Mascot source="claude" status="idle" size={COLLAPSED.mascot} />
  </span>
  <span class="gap"></span>
  <span class="wing">
    {#if state === 'empty'}
      <PixelText text="IDLE" size={1.3} color={PALETTE.textMuted} />
    {:else}
      <PixelIcon name="workspace" size={1.5} color={PALETTE.textDim} />
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
  @media (prefers-reduced-motion: reduce) {
    .col {
      transition: none;
    }
  }
</style>
