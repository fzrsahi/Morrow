<script lang="ts">
  // #180 — idle, empty and prehover states of the collapsed island, matched
  // 1:1 to canvas section #collapsed-quiet. #181 replaces the sleeping GIF
  // stand-in with real per-source sprites and #182 adds open/close motion.
  import PixelText from './PixelText.svelte';
  import PixelIcon from './PixelIcon.svelte';
  import { COLLAPSED, MOTION, PALETTE } from '../tokens';

  type Props = { sessionCount: number; prehover?: boolean; quiet?: boolean };
  let { sessionCount, prehover = false, quiet = false }: Props = $props();

  // The GIF is a stand-in for the Swift mascot; the asset lives in the shared
  // docs tree so the port does not duplicate binary files.
  const mascot = new URL('../../../../../docs/images/mascots/claude.gif', import.meta.url).href;
  let state = $derived(sessionCount > 0 ? 'idle' : 'empty');
</script>

<div class="col" class:pre={prehover} data-state={state} aria-label="Morrow island, {state}">
  <span class="wing">
    <span class="masc">
      <img src={mascot} alt="Morrow mascot" width={COLLAPSED.mascot} height={COLLAPSED.mascot} />
      <span class="zz"><PixelText text="z" size={1} color={PALETTE.textMuted} /></span>
    </span>
  </span>
  <span class="gap"></span>
  <span class="wing">
    {#if state === 'empty' && !quiet}
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
  .masc {
    position: relative;
    display: inline-block;
    width: 21px;
    height: 21px;
    flex: none;
  }
  .masc img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: contain;
    image-rendering: pixelated;
  }
  .moon {
    flex: none;
  }
  .zz {
    position: absolute;
    right: -3px;
    top: -5px;
    animation: zz 2.4s ease-in-out infinite;
  }
  @keyframes zz {
    50% {
      transform: translateY(-2px);
      opacity: 0.5;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .col,
    .zz {
      transition: none;
      animation: none;
    }
  }
</style>
