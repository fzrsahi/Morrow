<script lang="ts">
  // #307 — the Prayer settings page. Like AppearancePage, every control writes
  // one top-level key through the shell; a `settings-changed` event refreshes
  // the island's card without a restart.
  import {
    HIJRI_SOURCES,
    PRAYER_METHODS,
    type PrayerCity,
    type PrayerSettings,
  } from '../prayer';
  import { SETTINGS_KEYS } from '../settings';

  type Props = {
    settings: PrayerSettings;
    cities: PrayerCity[];
    onChange: (key: string, value: string | number | boolean) => void;
  };
  let { settings, cities, onChange }: Props = $props();

  function pick(key: string) {
    return (event: Event) => onChange(key, (event.currentTarget as HTMLSelectElement).value);
  }

  function minutes(event: Event) {
    const value = Number((event.currentTarget as HTMLInputElement).value);
    onChange(
      SETTINGS_KEYS.prayerCorrectionMinutes,
      Number.isFinite(value) ? Math.max(-120, Math.min(120, Math.trunc(value))) : 0,
    );
  }
</script>

<h2>Prayer times</h2>
<div class="grp">
  <div>
    <span class="n">City</span>
    <span class="s">
      <select value={settings.cityId} onchange={pick(SETTINGS_KEYS.prayerCityId)} aria-label="City">
        {#each cities as city (city.id)}
          <option value={city.id}>{city.name} · {city.province}</option>
        {/each}
      </select>
    </span>
  </div>
  <div>
    <span class="n">Calculation method</span>
    <span class="s">
      <select value={settings.method} onchange={pick(SETTINGS_KEYS.prayerMethod)} aria-label="Calculation method">
        {#each PRAYER_METHODS as method (method.value)}
          <option value={method.value}>{method.label}</option>
        {/each}
      </select>
    </span>
  </div>
  <div>
    <span class="n">Minute correction</span>
    <span class="s">
      <input
        type="number"
        min="-120"
        max="120"
        step="1"
        value={settings.correctionMinutes}
        oninput={minutes}
        aria-label="Minute correction"
      />
    </span>
  </div>
  <div>
    <span class="n">Hijri calendar</span>
    <span class="s">
      <select value={settings.hijriSource} onchange={pick(SETTINGS_KEYS.prayerHijriSource)} aria-label="Hijri calendar">
        {#each HIJRI_SOURCES as source (source.value)}
          <option value={source.value}>{source.label}</option>
        {/each}
      </select>
    </span>
  </div>
</div>

<style>
  select,
  input {
    max-width: 100%;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 6px;
    padding: 4px 6px;
    background: rgba(255, 255, 255, 0.08);
    color: #fff;
    font: inherit;
    font-size: 12px;
  }
  input[type='number'] {
    width: 74px;
  }
</style>
