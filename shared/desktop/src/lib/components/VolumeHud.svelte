<script lang="ts">
  // #195 — canvas section #volume, matched 1:1: recessed speaker slot, a
  // 12-segment meter and a mono-bold readout. The shell reads `music_volume`
  // and owns the click; this component only draws the reading it is handed.
  import PixelIcon from './PixelIcon.svelte';
  import { PALETTE } from '../tokens';
  import { readout, segmentColors, type Volume } from '../volume';

  type Props = {
    volume: Volume;
    onToggle: () => void;
    // The HUD replaces the collapsed bar, so it stays a drag handle.
    onpointerdown?: (event: PointerEvent) => void;
    onpointermove?: (event: PointerEvent) => void;
    onpointerup?: (event: PointerEvent) => void;
    onpointercancel?: (event: PointerEvent) => void;
  };
  let { volume, onToggle, onpointerdown, onpointermove, onpointerup, onpointercancel }: Props =
    $props();

  let colors = $derived(segmentColors(volume.percent, volume.muted));
  let label = $derived(readout(volume));
</script>

<button
  type="button"
  class="hud"
  data-volume-hud
  aria-label={volume.muted ? 'Unmute' : 'Mute'}
  title="Click to toggle mute"
  onclick={onToggle}
  {onpointerdown}
  {onpointermove}
  {onpointerup}
  {onpointercancel}
>
  <span class="spk slot" class:muted={volume.muted}>
    <PixelIcon
      name={volume.muted ? 'volumeMute' : 'volume'}
      size={1.2}
      color={volume.muted ? PALETTE.red : PALETTE.cyan}
    />
  </span>
  <span class="led slot" role="img" aria-label="volume meter">
    {#each colors as color, i (i)}
      <i class={color}></i>
    {/each}
  </span>
  <span class="rd" class:mute={volume.muted}>{label}</span>
</button>

<style>
  .hud {
    box-sizing: border-box;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 5px 10px;
    border: 0;
    border-radius: 6px;
    background: var(--surface);
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.5);
    color: var(--tw);
    font: inherit;
    cursor: default;
  }
  .slot {
    background: var(--slot);
    border-radius: 3.5px;
    box-shadow:
      inset 1px 1px 0 rgba(0, 0, 0, 0.7),
      inset -1px -1px 0 rgba(255, 255, 255, 0.08);
  }
  .spk {
    width: 20px;
    height: 20px;
    display: grid;
    place-items: center;
    flex: none;
  }
  .led {
    display: flex;
    gap: 2px;
    padding: 3px 4.5px;
    border-radius: 3px;
  }
  .led i {
    width: 5px;
    height: 9.5px;
    background: rgba(255, 255, 255, 0.12);
  }
  .led i.cyan {
    background: var(--cyan);
  }
  .led i.amber {
    background: var(--amber);
  }
  .led i.red {
    background: var(--red);
  }
  .rd {
    width: 32px;
    text-align: right;
    font: 700 9px ui-monospace, monospace;
    color: var(--tw);
  }
  .rd.mute {
    color: var(--red);
  }
  :focus-visible {
    outline: 2px solid var(--cyan);
    outline-offset: -2px;
  }
</style>
