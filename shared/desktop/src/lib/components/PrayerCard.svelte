<script lang="ts">
  // #307 — compact: the next prayer and its countdown. Expanded: the day's
  // five times and the Hijri date. No data means no card: the shell decides
  // when a time is real, and `prayerCardModel` guards the shape here.
  import { prayerCardModel } from '../prayer';
  import type { PrayerSnapshot } from '../types';

  let {
    prayer,
    onToggle,
  }: { prayer: PrayerSnapshot | null | undefined; onToggle?: () => void } = $props();
  let model = $derived(prayerCardModel(prayer));
  // #462: Settings' layout choice is the starting state. Depend on the mode
  // alone so a fresh countdown snapshot does not undo a user's own toggle.
  let startExpanded = $derived(model?.cardMode === 'panoramic');
  let expanded = $state(false);
  $effect(() => {
    expanded = startExpanded;
  });

  function toggle() {
    expanded = !expanded;
    // The island tight-fits to its content; the parent re-measures on expand.
    onToggle?.();
  }
</script>

{#if model}
  <div class="prayer" data-card="prayer">
    <button
      class="top"
      onclick={toggle}
      aria-expanded={expanded}
      aria-label="Prayer times: {model.nextName} in {model.countdown}"
    >
      <span class="mark" aria-hidden="true">☾</span>
      <span class="name">{model.nextName}</span>
      <span class="clock">{model.nextTime}</span>
      <span class="count" class:warn={model.warning !== null}>
        {model.countdown === 'now' ? 'now' : `-${model.countdown}`}
      </span>
      <svg class="chev" class:up={expanded} width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" aria-hidden="true">
        <path d="M4.5 6.5L8 10l3.5-3.5" />
      </svg>
    </button>
    {#if expanded}
      <div class="detail">
        {#if model.warning}
          <div class="warning">{model.warning}</div>
        {/if}
        <div class="hijri">{model.hijri}</div>
        {#each model.times as row (row.key)}
          <div class="row"><span>{row.name}</span><span>{row.time}</span></div>
        {/each}
        <div class="meta">
          {model.cityName} · {model.method}{model.correctionMinutes === 0
            ? ''
            : ` · ${model.correctionMinutes > 0 ? '+' : ''}${model.correctionMinutes}m`}
        </div>
      </div>
    {/if}
  </div>
{/if}

<style>
  .prayer {
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.04);
    overflow: hidden;
    margin-bottom: 6px;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: 30px;
    padding: 5px 10px;
    border: 0;
    background: none;
    color: inherit;
    font: 500 var(--f, 11px) system-ui, sans-serif;
    text-align: left;
    cursor: pointer;
  }
  .mark {
    color: var(--amber, #ffd892);
  }
  .name {
    color: rgba(255, 255, 255, 0.92);
  }
  .clock {
    margin-left: auto;
    color: rgba(255, 255, 255, 0.75);
    font-variant-numeric: tabular-nums;
  }
  .count {
    color: var(--cyan, #00f5d4);
    font-variant-numeric: tabular-nums;
  }
  .count.warn {
    color: var(--amber, #ffd892);
  }
  .chev {
    color: rgba(255, 255, 255, 0.5);
    transition: transform 0.15s ease;
  }
  .chev.up {
    transform: rotate(180deg);
  }
  .detail {
    padding: 6px 10px 8px;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
  }
  .warning {
    color: var(--amber, #ffd892);
  }
  .hijri {
    color: rgba(255, 255, 255, 0.75);
    margin: 2px 0 6px;
  }
  .row,
  .meta {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    color: rgba(255, 255, 255, 0.75);
    font-variant-numeric: tabular-nums;
  }
  .meta {
    margin-top: 6px;
    color: rgba(255, 255, 255, 0.45);
  }
  @media (prefers-reduced-motion: reduce) {
    .chev {
      transition: none;
    }
  }
</style>
