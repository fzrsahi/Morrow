<script lang="ts">
  // #199 — the Settings window shell: sidebar + page pane. It talks to the same
  // shell commands the island used to call; no settings logic lives here beyond
  // mapping controls onto keys in the existing settings file.
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import PixelIcon from './lib/components/PixelIcon.svelte';
  import HooksPage from './lib/components/HooksPage.svelte';
  import CapabilitiesPage from './lib/components/CapabilitiesPage.svelte';
  import AppearancePage from './lib/components/AppearancePage.svelte';
  import PrayerPage from './lib/components/PrayerPage.svelte';
  import BehaviorPage from './lib/components/BehaviorPage.svelte';
  import AboutPage from './lib/components/AboutPage.svelte';
  import SoundPage from './lib/components/SoundPage.svelte';
  import {
    DEFAULT_PRAYER_SETTINGS,
    prayerSettingsFromSettings,
    type PrayerCity,
    type PrayerSettings,
  } from './lib/prayer';
  import {
    DEFAULT_APPEARANCE,
    SETTINGS_NAV,
    appearanceFromSettings,
    type AppearanceSettings,
    type SettingsPageId,
  } from './lib/settings';
  import { isSupported } from './lib/capabilities';
  import {
    followUpMinutesFromSettings,
    SETTINGS_KEY_FOLLOW_UP_MINUTES,
  } from './lib/reminders';
  import { cssVariables } from './lib/tokens';
  import type {
    AutostartStatus,
    CapabilityRow,
    HooksSnapshot,
    KimiStatus,
    OpencodeStatus,
    SoundCue,
    TargetStatus,
    YamlStatus,
  } from './lib/types';

  function initialPage(): SettingsPageId {
    const requested = new URLSearchParams(window.location.search).get('page');
    const found = SETTINGS_NAV.flatMap((group) => group.items).find(
      (item) => item.id === requested && item.available,
    );
    return found?.id ?? 'hooks';
  }
  let page = $state<SettingsPageId>(initialPage());
  let hooks = $state<HooksSnapshot | null>(null);
  let capabilities = $state<CapabilityRow[]>([]);
  let autostart = $state<AutostartStatus | null>(null);
  let appearance = $state<AppearanceSettings>({ ...DEFAULT_APPEARANCE });
  let prayer = $state<PrayerSettings>({ ...DEFAULT_PRAYER_SETTINGS });
  let cities = $state<PrayerCity[]>([]);
  let cues = $state<SoundCue[]>([]);
  let followUpMinutes = $state(1);
  let busy = $state(false);
  let error = $state<string | null>(null);

  function applyAppearance() {
    for (const [name, value] of Object.entries(cssVariables(appearance.fontSize))) {
      document.documentElement.style.setProperty(name, value);
    }
  }

  async function load() {
    try {
      const [h, c, a, stored, availableCities] = await Promise.all([
      const [h, c, a, stored, sound] = await Promise.all([
        invoke<HooksSnapshot>('hooks_status'),
        invoke<CapabilityRow[]>('capabilities'),
        invoke<AutostartStatus>('autostart_status'),
        invoke<unknown>('settings_load'),
        invoke<PrayerCity[]>('prayer_cities'),
        invoke<SoundCue[]>('sound_cues'),
      ]);
      hooks = h;
      capabilities = c;
      autostart = a;
      cues = sound;
      appearance = appearanceFromSettings(stored);
      prayer = prayerSettingsFromSettings(stored);
      cities = availableCities;
      followUpMinutes = followUpMinutesFromSettings(
        (stored as Record<string, unknown>)[SETTINGS_KEY_FOLLOW_UP_MINUTES],
      );
      applyAppearance();
      error = null;
    } catch (cause) {
      error = String(cause);
    }
  }

  async function run(action: () => Promise<unknown>, reload = true) {
    busy = true;
    try {
      await action();
      if (reload) await load();
    } catch (cause) {
      error = String(cause);
    } finally {
      busy = false;
    }
  }

  function saveSetting(key: string, value: string | number | boolean) {
    const next = { ...appearance, [key]: value } as AppearanceSettings;
    appearance = next;
    applyAppearance();
    void run(() => invoke('settings_set', { key, value }), false);
  }

  // #241: the same answer the Capability table gives, read the same way, so
  // the page and the table cannot disagree about whether a sound can play.
  const soundSupported = $derived(isSupported(capabilities, 'Sound effects'));

  function saveFollowUpMinutes(key: string, value: number) {
    followUpMinutes = value;
    void run(() => invoke('settings_set', { key, value }), false);
  }

  onMount(() => {
    void load();
  });
</script>

<div class="win">
  <div class="wt">Morrow Settings<span class="cbx"><i>–</i><i>▢</i><i>✕</i></span></div>
  <div class="wb">
    <nav class="side" aria-label="Settings pages">
      {#each SETTINGS_NAV as group}
        <h6>{group.group}</h6>
        {#each group.items as item}
          <button
            class="item"
            class:on={page === item.id}
            disabled={!item.available}
            aria-current={page === item.id ? 'page' : undefined}
            onclick={() => (page = item.id)}
          >
            <PixelIcon name={item.icon} size={1.1} />
            <span>{item.label}</span>
          </button>
        {/each}
      {/each}
    </nav>
    <div class="pane">
      {#if page === 'hooks'}
        <HooksPage
          {hooks}
          {busy}
          {error}
          onRunHooks={(command) => run(() => invoke(command))}
          onToggleTarget={(row) =>
            run(() =>
              invoke(row.installed ? 'target_uninstall' : 'target_install', { source: row.source }),
            )}
          onToggleOpencode={(row) =>
            run(() => invoke(row.plugin_installed ? 'opencode_uninstall' : 'opencode_install'))}
          onToggleKimi={(row) => run(() => invoke(row.installed ? 'kimi_uninstall' : 'kimi_install'))}
          onToggleYaml={(row) =>
            run(() =>
              invoke(row.installed ? 'yaml_uninstall' : 'yaml_install', { source: row.source }),
            )}
        />
      {:else if page === 'capabilities'}
        <CapabilitiesPage
          {capabilities}
          {autostart}
          {busy}
          onToggleAutostart={() =>
            run(() => invoke('autostart_set', { enabled: !autostart?.installed }))}
        />
      {:else if page === 'sound'}
        <SoundPage
          {cues}
          supported={soundSupported}
          {busy}
          onPreview={(cue) => run(() => invoke('sound_preview', { cue }), false)}
        />
      {:else if page === 'appearance'}
        <AppearancePage {appearance} onChange={saveSetting} />
      {:else if page === 'prayer'}
        <PrayerPage
          settings={prayer}
          {cities}
          onChange={(key, value) => run(() => invoke('prayer_settings_set', { key, value }))}
        />
      {:else if page === 'behavior'}
        <BehaviorPage minutes={followUpMinutes} onChange={saveFollowUpMinutes} />
      {:else if page === 'about'}
        <AboutPage />
      {/if}
    </div>
  </div>
</div>
