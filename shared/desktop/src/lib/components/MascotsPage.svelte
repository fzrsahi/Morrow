<script lang="ts">
  // #465 — the Mascots page: preview status, animation speed, default source
  // and the 33-entry gallery, mirroring `SettingsView.swift:3149-3242`.
  import Mascot from './Mascot.svelte';
  import {
    MASCOT_GALLERY,
    MASCOT_KEYS,
    mascotSpeedLabel,
    type MascotGalleryEntry,
  } from '../mascot-gallery';
  import type { MascotStatus } from '../mascots';

  type Props = {
    mascots: { speed: number; source: string };
    t: (key: string) => string;
    onChange: (key: string, value: string | number) => void;
  };
  let { mascots, t, onChange }: Props = $props();

  // Swift's `@State previewStatus`: a gallery-only choice, not a stored key.
  let previewStatus = $state<MascotStatus>('working');
  const statuses: ReadonlyArray<{ value: MascotStatus; key: string }> = [
    { value: 'working', key: 'processing' },
    { value: 'idle', key: 'idle' },
    { value: 'waitingApproval', key: 'waiting_approval' },
  ];
  const selectedSource = $derived(
    MASCOT_GALLERY.some((entry) => entry.source === mascots.source)
      ? mascots.source
      : MASCOT_GALLERY[0].source,
  );
</script>

<h2>{t('mascots')}</h2>
<div class="grp">
  <div>
    <span class="n">{t('preview_status')}</span>
    <span class="s">
      <span class="seg2">
        {#each statuses as status (status.value)}
          <button
            class:on={previewStatus === status.value}
            aria-pressed={previewStatus === status.value}
            onclick={() => (previewStatus = status.value)}
          >{t(status.key)}</button>
        {/each}
      </span>
    </span>
  </div>
  <div>
    <span class="n">{t('mascot_speed')}</span>
    <span class="s">
      <input
        type="range"
        min="0"
        max="300"
        step="25"
        value={mascots.speed}
        aria-label={t('mascot_speed')}
        oninput={(event) => onChange(MASCOT_KEYS.speed, Number(event.currentTarget.value))}
      />
      {mascotSpeedLabel(mascots.speed, t('speed_off'))}
    </span>
  </div>
  <div>
    <span class="n">{t('default_mascot')}</span>
    <span class="s">{t('default_mascot_desc')}</span>
    <select
      value={selectedSource}
      aria-label={t('default_mascot')}
      onchange={(event) => onChange(MASCOT_KEYS.defaultSource, event.currentTarget.value)}
    >
      {#each MASCOT_GALLERY as entry (entry.source)}
        <option value={entry.source}>{entry.desc}</option>
      {/each}
    </select>
  </div>
</div>
<h3>{t('mascots')}</h3>
<div class="gallery">
  {#each MASCOT_GALLERY as entry: MascotGalleryEntry (entry.source)}
    <div class="card" style:--accent={entry.color}>
      <div class="face"><Mascot source={entry.source} status={previewStatus} size={40} hidden /></div>
      <b>{entry.name}</b>
      <span>{entry.desc}</span>
      <small>{entry.source}</small>
    </div>
  {/each}
</div>

<style>
  h3 {
    margin: 14px 0 8px;
    font-size: 13px;
    font-weight: 600;
  }
  .gallery {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(112px, 1fr));
    gap: 8px;
  }
  .card {
    display: grid;
    justify-items: center;
    gap: 2px;
    padding: 8px 4px;
    border: 1px solid color-mix(in srgb, var(--accent) 30%, transparent);
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.03);
    text-align: center;
    font-size: 11px;
  }
  .card .face {
    width: 48px;
    height: 48px;
    display: grid;
    place-items: center;
    border-radius: 10px;
    background: #000;
  }
  .card span,
  .card small {
    color: rgba(255, 255, 255, 0.55);
    font-size: 10px;
  }
  select {
    max-width: 180px;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 6px;
    padding: 4px 6px;
    background: rgba(255, 255, 255, 0.08);
    color: #fff;
    font: inherit;
    font-size: 12px;
  }
  input[type='range'] {
    width: 140px;
    vertical-align: middle;
    margin-right: 6px;
  }
</style>
