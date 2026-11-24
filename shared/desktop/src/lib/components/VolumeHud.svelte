<script lang="ts">
  // #195 — canvas section #volume, matched 1:1: recessed speaker slot, a
  // 12-segment meter and a mono-bold readout.
  //
  // #239 AC 2 — the meter is an input now. Before this the HUD only *drew* a
  // reading and the volume could be turned down from anywhere except the one
  // surface that shows it. The two controls are separated by what they mean: the
  // speaker mutes, the meter sets a level (and unmutes, see `setVolumePlan`).
  import PixelIcon from './PixelIcon.svelte';
  import { PALETTE } from '../tokens';
  import { VOLUME_SEGMENTS, percentForKey, percentForSegment, readout, segmentColors, setVolumePlan, type Volume } from '../volume';

  type Props = {
    volume: Volume;
    onToggle: () => void;
    onSet: (percent: number) => void;
    // The HUD replaces the collapsed bar, so it stays a drag handle.
    onpointerdown?: (event: PointerEvent) => void;
    onpointermove?: (event: PointerEvent) => void;
    onpointerup?: (event: PointerEvent) => void;
    onpointercancel?: (event: PointerEvent) => void;
  };
  let { volume, onToggle, onSet, onpointerdown, onpointermove, onpointerup, onpointercancel }: Props =
    $props();

  let colors = $derived(segmentColors(volume.percent, volume.muted));
  let label = $derived(readout(volume));

  function set(percent: number) {
    const plan = setVolumePlan(volume.muted, percent);
    onSet(plan.percent);
  }

  // A click on the strip asks for the level that lights exactly that segment.
  function onMeterClick(event: MouseEvent) {
    const meter = event.currentTarget as HTMLElement;
    const box = meter.getBoundingClientRect();
    if (box.width <= 0) return;
    const width = box.width / VOLUME_SEGMENTS;
    set(percentForSegment(Math.floor((event.clientX - box.left) / width)));
  }

  function onMeterKey(event: KeyboardEvent) {
    const percent = percentForKey(volume.percent, event.key);
    // A key the meter does not own keeps its meaning: swallowing Tab would trap
    // focus inside a HUD that disappears in 1.5 s.
    if (percent === null) return;
    event.preventDefault();
    set(percent);
  }
</script>

<!-- Not a <button> any more: it holds a button (mute) and a slider (level), and
     a button cannot contain either. It stays the island's drag handle. -->
<div
  class="hud"
  data-volume-hud
  role="group"
  aria-label="Volume"
  {onpointerdown}
  {onpointermove}
  {onpointerup}
  {onpointercancel}
>
  <button
    type="button"
    class="spk slot"
    class:muted={volume.muted}
    aria-label={volume.muted ? 'Unmute' : 'Mute'}
    title="Click to toggle mute"
    onclick={onToggle}
  >
    <PixelIcon
      name={volume.muted ? 'volumeMute' : 'volume'}
      size={1.2}
      color={volume.muted ? PALETTE.red : PALETTE.cyan}
    />
  </button>
  <span
    class="led slot"
    role="slider"
    tabindex="0"
    aria-label="Volume level"
    aria-valuemin="0"
    aria-valuemax="100"
    aria-valuenow={volume.percent}
    aria-valuetext={label}
    title="Click or use the arrow keys to set the level"
    onclick={onMeterClick}
    onkeydown={onMeterKey}
  >
    {#each colors as color, i (i)}
      <i class={color}></i>
    {/each}
  </span>
  <span class="rd" class:mute={volume.muted}>{label}</span>
</div>

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
    /* Same slot as before; only the element changed, from a span inside the
       button to the button itself, so the 20 px target is preserved. */
    width: 20px;
    height: 20px;
    padding: 0;
    border: 0;
    cursor: pointer;
    display: grid;
    place-items: center;
    flex: none;
  }
  .led {
    display: flex;
    gap: 2px;
    cursor: pointer;
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
