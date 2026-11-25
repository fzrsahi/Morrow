<script lang="ts">
  // #307 — the Prayer settings page. Every control writes one top-level key
  // through the shell; a `settings-changed` event refreshes the island's card
  // without a restart. #462 adds the macOS controls that need no OS permission:
  // enable, card layout, Hijri day offset, location mode, chime and a test
  // notification. Automatic location stays unavailable until #443.
  import {
    AUTOMATIC_LOCATION_REASON,
    HIJRI_SOURCES,
    PRAYER_CARD_MODES,
    PRAYER_LOCATION_MODES,
    PRAYER_METHODS,
    type PrayerCity,
    type PrayerSettings,
  } from '../prayer';
  import { SETTINGS_KEYS } from '../settings';

  type Props = {
    settings: PrayerSettings;
    cities: PrayerCity[];
    busy: boolean;
    error: string | null;
    onChange: (key: string, value: string | number | boolean) => void;
    onPreviewChime: () => void;
    onTestBanner: () => void;
  };
  let { settings, cities, busy, error, onChange, onPreviewChime, onTestBanner }: Props = $props();

  function pick(key: string) {
    return (event: Event) => onChange(key, (event.currentTarget as HTMLSelectElement).value);
  }

  function toggle(key: string) {
    return (event: Event) => onChange(key, (event.currentTarget as HTMLInputElement).checked);
  }

  function bounded(event: Event, key: string, limit: number) {
    const value = Number((event.currentTarget as HTMLInputElement).value);
    onChange(key, Number.isFinite(value) ? Math.max(-limit, Math.min(limit, Math.trunc(value))) : 0);
  }

  function minutes(event: Event) {
    bounded(event, SETTINGS_KEYS.prayerCorrectionMinutes, 120);
  }

  function hijriOffset(event: Event) {
    bounded(event, SETTINGS_KEYS.prayerHijriOffset, 2);
  }
</script>

<h2>Prayer times</h2>
<div class="grp">
  <div>
    <span class="n">Enable prayer times</span>
    <span class="s">
      <input
        type="checkbox"
        checked={settings.enabled}
        onchange={toggle(SETTINGS_KEYS.prayerEnabled)}
        aria-label="Enable prayer times"
      />
    </span>
  </div>
</div>
{#if !settings.enabled}
  <p class="note">
    The island card is hidden and no prayer reminders are sent while this is off. Your saved
    settings are kept.
  </p>
{:else}
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

  <div class="grp">
    <div>
      <span class="n">Card display mode</span>
      <span class="s">
        <select value={settings.cardMode} onchange={pick(SETTINGS_KEYS.prayerCardMode)} aria-label="Card display mode">
          {#each PRAYER_CARD_MODES as mode (mode.value)}
            <option value={mode.value}>{mode.label}</option>
          {/each}
        </select>
      </span>
    </div>
    <div>
      <span class="n">Hijri day adjustment</span>
      <span class="s">
        <input
          type="number"
          min="-2"
          max="2"
          step="1"
          value={settings.hijriDayOffset}
          oninput={hijriOffset}
          aria-label="Hijri day adjustment"
        />
      </span>
    </div>
    <div>
      <span class="n">Location mode</span>
      <span class="s">
        <select value={settings.locationMode} onchange={pick(SETTINGS_KEYS.prayerLocationMode)} aria-label="Location mode">
          {#each PRAYER_LOCATION_MODES as mode (mode.value)}
            <option value={mode.value} disabled={mode.value === 'automatic'}>{mode.label}</option>
          {/each}
        </select>
      </span>
    </div>
  </div>
  <p class="note">{AUTOMATIC_LOCATION_REASON}</p>
  <p class="note">
    A day adjustment of -2 to +2 follows an official crescent sighting when it differs from the
    calculated Hijri date.
  </p>

  <div class="grp">
    <div>
      <span class="n">Prayer time chime</span>
      <span class="s">
        <button class="b" disabled={busy} onclick={onPreviewChime}>Preview</button>
        <label class="inline">
          <input
            type="checkbox"
            checked={settings.soundEnabled}
            onchange={toggle(SETTINGS_KEYS.prayerSoundEnabled)}
            aria-label="Prayer time chime"
          />
          Play at prayer time
        </label>
      </span>
    </div>
    <div>
      <span class="n">Test notification</span>
      <span class="s">
        <button class="b" disabled={busy} onclick={onTestBanner}>Test prayer notification banner</button>
      </span>
    </div>
  </div>
  <p class="note">
    The test uses the same notification path as a real prayer time. The chime preview uses the
    shared sound table; a machine with no sound player says so below.
  </p>
{/if}

{#if error}
  <div class="grp"><div><span class="s no">{error}</span></div></div>
{/if}

<style>
  select,
  input[type='number'] {
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
  .inline {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-left: 12px;
  }
  .note {
    margin: 0 0 12px;
    color: rgba(255, 255, 255, 0.45);
    font-size: 12px;
    line-height: 1.5;
  }
</style>
