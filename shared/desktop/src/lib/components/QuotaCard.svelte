<script lang="ts">
  // #308 — the panel's plan-limit readout. The shell owns every number; this
  // card only draws what it was sent. A state without numbers gets a sentence,
  // never a 0% bar (see quotaCardView).
  import { quotaCardView } from '../quota';
  import type { QuotaSnapshot } from '../types';

  let { quota }: { quota: QuotaSnapshot | null } = $props();
  let view = $derived(quotaCardView(quota));
</script>

<div class="quota" data-status={quota?.status ?? 'unavailable'}>
  <div class="title">{view.title}</div>
  {#if view.rows.length === 0}
    {#if view.reason}
      <div class="reason">{view.reason}</div>
    {/if}
  {:else}
    {#each view.rows as row (row.key)}
      <div class="row">
        <div class="head">
          <span class="label">{row.label}</span>
          <span class="pct">{Math.round(row.percent)}%</span>
        </div>
        <div class="bar {row.barClass}">
          <div class="fill" style:width={`${row.width}%`} style:background={row.barColor}></div>
        </div>
        {#if row.paceLabel}
          <div class="pace">{row.paceLabel}</div>
        {/if}
      </div>
    {/each}
  {/if}
</div>

<style>
  .quota {
    padding: 6px 14px 8px;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
    font: var(--f) system-ui, sans-serif;
  }
  .title {
    color: var(--tm);
    font-size: var(--f-title);
    margin-bottom: 4px;
  }
  .reason {
    color: var(--td);
  }
  .row + .row {
    margin-top: 6px;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    color: var(--tw);
  }
  .pct {
    color: var(--tw);
  }
  .bar {
    height: 4px;
    margin-top: 3px;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.12);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    border-radius: 2px;
  }
  .pace {
    margin-top: 2px;
    color: var(--td);
    font-size: var(--f-caption, 9px);
  }
</style>
