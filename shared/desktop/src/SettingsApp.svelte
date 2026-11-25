<script lang="ts">
  // #199 — the Settings window shell: sidebar + page pane. It talks to the same
  // shell commands the island used to call; no settings logic lives here beyond
  // mapping controls onto keys in the existing settings file.
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import PixelIcon from './lib/components/PixelIcon.svelte';
  import GeneralPage from './lib/components/GeneralPage.svelte';
  import HooksPage from './lib/components/HooksPage.svelte';
  import RemotePage from './lib/components/RemotePage.svelte';
  import CapabilitiesPage from './lib/components/CapabilitiesPage.svelte';
  import AgentsPage from './lib/components/AgentsPage.svelte';
  import AppearancePage from './lib/components/AppearancePage.svelte';
  import PrayerPage from './lib/components/PrayerPage.svelte';
  import BehaviorPage from './lib/components/BehaviorPage.svelte';
  import AboutPage from './lib/components/AboutPage.svelte';
  import ShortcutsPage from './lib/components/ShortcutsPage.svelte';
  import UnavailablePage from './lib/components/UnavailablePage.svelte';
  import SoundPage from './lib/components/SoundPage.svelte';
  import WorkspacePage from './lib/components/WorkspacePage.svelte';
  import {
    DEFAULT_PRAYER_SETTINGS,
    prayerSettingsFromSettings,
    type PrayerCity,
    type PrayerSettings,
  } from './lib/prayer';
  import {
    DEFAULT_APPEARANCE,
    DEFAULT_GESTURES,
    SETTINGS_KEYS,
    SETTINGS_NAV,
    appearanceFromSettings,
    gesturesFromSettings,
    pageLabel,
    type AppearanceSettings,
    type GestureSettings,
    type SettingsPageId,
    type ShortcutBinding,
  } from './lib/settings';
  import { isSupported } from './lib/capabilities';
  import {
    AGENT_KEYS,
    DEFAULT_AGENTS,
    agentsFromSettings,
    type AgentsSettings,
  } from './lib/agents-settings';
  import {
    BEHAVIOR_KEYS,
    DEFAULT_BEHAVIOR,
    behaviorFromSettings,
    type BehaviorSettings,
  } from './lib/behavior';
  import { REMOTE_STATUS_EVENT, type RemoteStatuses } from './lib/remote';
  import { resolveLanguage, translator, type Language } from './lib/strings';
  import { applyWorkspaceSetting, workspaceFromSettings, type WorkspaceSettings } from './lib/workspace';
  import {
    followUpMinutesFromSettings,
    SETTINGS_KEY_FOLLOW_UP_MINUTES,
  } from './lib/reminders';
  import { cssVariables } from './lib/tokens';
  import { APP_VERSION } from './lib/version';
  import { displaySummary, type DisplaySnapshot } from './lib/display';
  import { soundControlsFromSettings, type SoundControls } from './lib/sound-controls';
  import { mascotFromSettings } from './lib/mascot-gallery';
  import MascotsPage from './lib/components/MascotsPage.svelte';
  import { buddyFromSettings, type BuddySettings } from './lib/buddy-settings';
  import BuddyPage from './lib/components/BuddyPage.svelte';
  import { floatingLyricsFromSettings, type FloatingLyrics } from './lib/floating-lyrics';
  import FloatingLyricsPage from './lib/components/FloatingLyricsPage.svelte';
  import type {
    AutostartStatus,
    CapabilityRow,
    HooksSnapshot,
    KimiStatus,
    OpencodeStatus,
    RemoteHost,
    SoundCue,
    TargetStatus,
    YamlStatus,
  } from './lib/types';

  function initialPage(): SettingsPageId {
    const requested = new URLSearchParams(window.location.search).get('page');
    // #239 AC 3: unavailable pages are reachable too — they open onto the reason
    // instead of a dead sidebar row.
    const found = SETTINGS_NAV.flatMap((group) => group.items).find(
      (item) => item.id === requested,
    );
    return found?.id ?? 'hooks';
  }
  let page = $state<SettingsPageId>(initialPage());
  let hooks = $state<HooksSnapshot | null>(null);
  let capabilities = $state<CapabilityRow[]>([]);
  let autostart = $state<AutostartStatus | null>(null);
  let appearance = $state<AppearanceSettings>({ ...DEFAULT_APPEARANCE });
  let workspace = $state<WorkspaceSettings>(workspaceFromSettings({}));
  let prayer = $state<PrayerSettings>({ ...DEFAULT_PRAYER_SETTINGS });
  let cities = $state<PrayerCity[]>([]);
  let cues = $state<SoundCue[]>([]);
  let soundControls = $state<SoundControls>(soundControlsFromSettings({}));
  // #462: the editable chords, read from the same settings document the
  // grabber uses. A write reloads them so the badge reflects the saved state.
  let shortcuts = $state<ShortcutBinding[]>([]);
  // #465: the connected screens and their resolved per-display rows. The shell
  // resolves legacy keys and migration before sending this snapshot.
  let displays = $state<DisplaySnapshot>({
    displays: [],
    configs: {},
    islandEnabled: true,
    displayChoice: 'auto',
    primaryEnabledDisplayId: null,
    readerReason: null,
  });
  // #448: hosts and their tunnel status. Status arrives as an event from the
  // shell's pump, so the card never polls ssh or reaches the network itself.
  let hosts = $state<RemoteHost[]>([]);
  let statuses = $state<RemoteStatuses>({});
  // #239 AC 1: shown on About so the file the user may want to back up or paste
  // into a bug report is reachable without knowing the convention.
  let settingsFile = $state('');
  // #239 AC 4: one language for the window. `system` is not a table —
  // `resolveLanguage` narrows it, as Swift's `effectiveLanguage` does. Derived,
  // so switching the stored tag redraws the sidebar without a restart.
  let language = $derived(
    resolveLanguage(typeof navigator === 'undefined' ? 'en' : navigator.language, appearance.language),
  );
  // #239 AC 3: the page being shown, for the unavailable ones that have no
  // component of their own. Read from the same list the sidebar draws, so a
  // reason can never be a second copy.
  let pendingPage = $derived(
    SETTINGS_NAV.flatMap((group) => group.items).find((item) => item.id === page && !item.available),
  );
  let t = $derived(translator(language));
  let hotkeySupported = $derived(isSupported(capabilities, 'Global hotkey'));
  // The same row the Capability table prints, so the reason a key did not
  // register is not retyped here.
  let hotkeyDetail = $derived.by(() => {
    const row = capabilities.find((entry) => entry.feature === 'Global hotkey');
    if (!row) return null;
    const { capability } = row;
    if ('Degraded' in capability) return capability.Degraded;
    if ('Unsupported' in capability) return capability.Unsupported;
    return null;
  });
  let followUpMinutes = $state(1);
  let gestures = $state<GestureSettings>({ ...DEFAULT_GESTURES });
  // #465: all Swift Behaviour keys, in one state object. A write goes through
  // the same settings_set command as the appearance and gesture controls.
  let behavior = $state<BehaviorSettings>({ ...DEFAULT_BEHAVIOR });
  // #465: the Agents page and the panel read the same Swift settings keys.
  let agents = $state<AgentsSettings>({ ...DEFAULT_AGENTS });
  let mascots = $state(mascotFromSettings({}));
  let buddy = $state<BuddySettings>(buddyFromSettings({}));
  let floatingLyrics = $state<FloatingLyrics>(floatingLyricsFromSettings({}));
  let busy = $state(false);
  let error = $state<string | null>(null);

  function applyAppearance() {
    for (const [name, value] of Object.entries(cssVariables(appearance.fontSize))) {
      document.documentElement.style.setProperty(name, value);
    }
  }

  async function load() {
    try {
      const [h, c, a, stored, availableCities, sound, file, remoteHosts, remoteStatuses, shortcutRows, displayRows] =
        await Promise.all([
          invoke<HooksSnapshot>('hooks_status'),
          invoke<CapabilityRow[]>('capabilities'),
          invoke<AutostartStatus>('autostart_status'),
          invoke<unknown>('settings_load'),
          invoke<PrayerCity[]>('prayer_cities'),
          invoke<SoundCue[]>('sound_cues'),
          invoke<string>('settings_path'),
          invoke<RemoteHost[]>('remote_hosts'),
          invoke<RemoteStatuses>('remote_status'),
          invoke<ShortcutBinding[]>('shortcut_list'),
          invoke<DisplaySnapshot>('display_snapshot'),
        ]);
      hooks = h;
      capabilities = c;
      autostart = a;
      cues = sound;
      soundControls = soundControlsFromSettings(stored);
      mascots = mascotFromSettings(stored);
      buddy = buddyFromSettings(stored);
      floatingLyrics = floatingLyricsFromSettings(stored);
      shortcuts = shortcutRows;
      displays = displayRows;
      settingsFile = file;
      appearance = appearanceFromSettings(stored);
      gestures = gesturesFromSettings(stored);
      behavior = behaviorFromSettings(stored);
      agents = agentsFromSettings(stored);
      workspace = workspaceFromSettings(stored);
      prayer = prayerSettingsFromSettings(stored);
      cities = availableCities;
      hosts = remoteHosts;
      statuses = remoteStatuses;
      followUpMinutes = followUpMinutesFromSettings(
        (stored as Record<string, unknown>)[SETTINGS_KEY_FOLLOW_UP_MINUTES],
      );
      applyAppearance();
      error = null;
    } catch (cause) {
      error = String(cause);
    }
  }

  async function run(action: () => Promise<unknown>, reload = true): Promise<boolean> {
    busy = true;
    try {
      await action();
      if (reload) await load();
      return true;
    } catch (cause) {
      error = String(cause);
      return false;
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

  // #462: the stored key is `appLanguage`, the local field is `language`. Set
  // both so the derived translator redraws this window as soon as the choice
  // changes, while the settings file keeps the key Swift reads.
  function saveLanguage(tag: string) {
    appearance = { ...appearance, language: tag };
    void run(() => invoke('settings_set', { key: SETTINGS_KEYS.language, value: tag }), false);
  }

  function saveAgentSetting(field: keyof typeof AGENT_KEYS, value: string | number | boolean) {
    agents = { ...agents, [field]: value } as AgentsSettings;
    void run(() => invoke('settings_set', { key: AGENT_KEYS[field], value }), false);
  }

  // #239 AC 1: the island's offset is per monitor and lives in the shell's own
  // store, so the settings key alone cannot move it back. Both are reset: the key
  // is what the page reads, the store is what the window obeys.
  function resetPosition() {
    saveSetting(SETTINGS_KEYS.position, DEFAULT_APPEARANCE.position);
    void run(() => invoke('reset_island_position'));
  }

  // #241: the same answer the Capability table gives, read the same way, so
  // the page and the table cannot disagree about whether a sound can play.
  const soundSupported = $derived(isSupported(capabilities, 'Sound effects'));

  function saveFollowUpMinutes(key: string, value: number) {
    followUpMinutes = value;
    void run(() => invoke('settings_set', { key, value }), false);
  }

  // Parity #3: the island reads these keys on the next settings-changed event.
  function saveGestureSetting(key: string, value: boolean) {
    gestures = { ...gestures, [key]: value };
    void run(() => invoke('settings_set', { key, value }), false);
  }

  // #465: one writer for every Behaviour control. The state field name is the
  // key name, so the Swift spelling lives in exactly one table (BEHAVIOR_KEYS).
  function saveBehaviorSetting(
    field: keyof typeof BEHAVIOR_KEYS,
    value: string | number | boolean,
  ) {
    behavior = { ...behavior, [field]: value } as BehaviorSettings;
    void run(() => invoke('settings_set', { key: BEHAVIOR_KEYS[field], value }), false);
  }

  function saveWorkspace(key: string, value: string | boolean) {
    workspace = applyWorkspaceSetting(workspace, key, value);
    void run(() => invoke('settings_set', { key, value }), false);
  }

  // #465: the per-display rows go through the shell's command, not a raw key
  // write, so the planner can keep the "at least one island" rule on reload.
  function saveDisplay(id: string, enabled: boolean, displayMode: string, topOffset: number) {
    void run(() => invoke('display_set', { id, enabled, displayMode, topOffset }));
  }

  function activateDisplay(id: string) {
    void run(() => invoke('display_activate', { id }));
  }

  function setIslandEnabled(enabled: boolean) {
    void run(() => invoke('display_island_enabled', { enabled }));
  }

  onMount(() => {
    void load();
    // #448: the shell publishes a full status snapshot on every engine change;
    // replace ours rather than asking again on a timer.
    let unlisten: (() => void) | undefined;
    void listen<RemoteStatuses>(REMOTE_STATUS_EVENT, (event) => {
      statuses = event.payload;
    })
      .then((off) => (unlisten = off))
      .catch(() => {});
    return () => unlisten?.();
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
            class:pending={!item.available}
            aria-current={page === item.id ? 'page' : undefined}
            onclick={() => (page = item.id)}
          >
            <PixelIcon name={item.icon} size={1.1} />
            <!-- #239 AC 4: the label is the string macOS draws, from
                 shared/contract/strings — not a second copy typed here. -->
            <span>{pageLabel(item, language)}</span>
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
          {t}
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
      {:else if page === 'remote'}
        <RemotePage
          {hosts}
          {statuses}
          {busy}
          {t}
          onSaveHost={(host) => run(() => invoke('remote_host_save', { host }))}
          onRemoveHost={(id) => run(() => invoke('remote_host_remove', { id }))}
          onConnect={(id) => run(() => invoke('remote_connect', { id }), false)}
          onDisconnect={(id) => run(() => invoke('remote_disconnect', { id }), false)}
        />
      {:else if page === 'capabilities'}
        <CapabilitiesPage
          {capabilities}
          {autostart}
          {busy}
          {t}
          onToggleAutostart={() =>
            run(() => invoke('autostart_set', { enabled: !autostart?.installed }))}
        />
      {:else if page === 'sound'}
        <SoundPage
          {cues}
          controls={soundControls}
          supported={soundSupported}
          {busy}
          {t}
          onPreview={(cue) => run(() => invoke('sound_preview', { cue }), false)}
          onChange={(key, value) => run(() => invoke('settings_set', { key, value }))}
        />
      {:else if page === 'shortcuts'}
        <ShortcutsPage
          bindings={shortcuts}
          {hotkeySupported}
          {hotkeyDetail}
          {t}
          onSet={(action, chord) => run(() => invoke('shortcut_set', { action, chord }))}
        />
      {:else if page === 'appearance'}
        <AppearancePage
          {appearance}
          {t}
          onChange={saveSetting}
          onResetPosition={resetPosition}
          {displays}
          onDisplaySet={saveDisplay}
          onDisplayActivate={activateDisplay}
          onIslandEnabled={setIslandEnabled}
          onPreview={(id) => run(() => invoke('notification_preview', { id }), false)}
          onPowerPreview={() => run(() => invoke('power_banner_preview'), false)}
        />
      {:else if page === 'floating-lyrics'}
        <FloatingLyricsPage
          settings={floatingLyrics}
          {t}
          onChange={(key, value) => run(() => invoke('settings_set', { key, value }))}
        />
      {:else if page === 'prayer'}
        <PrayerPage
          settings={prayer}
          {cities}
          {t}
          {busy}
          {error}
          onChange={(key, value) => run(() => invoke('prayer_settings_set', { key, value }))}
          onPreviewChime={() => run(() => invoke('prayer_preview_chime'), false)}
          onTestBanner={() => run(() => invoke('prayer_test_banner'), false)}
        />
      {:else if page === 'behavior'}
        <BehaviorPage
          {t}
          minutes={followUpMinutes}
          {gestures}
          {behavior}
          onChange={saveFollowUpMinutes}
          onGestureChange={saveGestureSetting}
          onBehaviorChange={saveBehaviorSetting}
        />
      {:else if page === 'agents'}
        <AgentsPage settings={agents} {t} onChange={saveAgentSetting} />
      {:else if page === 'mascots'}
        <MascotsPage
          {mascots}
          {t}
          onChange={(key, value) => run(() => invoke('settings_set', { key, value }))}
        />
      {:else if page === 'buddy'}
        <BuddyPage
          settings={buddy}
          {t}
          onChange={(key, value) => run(() => invoke('settings_set', { key, value }))}
          onPreview={(source, status, brightness, orientation) =>
            invoke<string[]>('buddy_frame_preview', { source, status, brightness, orientation })}
        />
      {:else if page === 'workspace'}
        <WorkspacePage settings={workspace} onChange={saveWorkspace} />
      {:else if page === 'about'}
        <AboutPage
          {settingsFile}
          version={APP_VERSION}
          {t}
          onExport={(destination) =>
            invoke<string>('diagnostics_export', { destination })}
        />
      {:else if page === 'general'}
        <GeneralPage
          stored={appearance.language}
          allowHorizontalDrag={behavior.allowHorizontalDrag}
          avoidMenuBarIcons={behavior.avoidMenuBarIcons}
          {t}
          onChange={saveLanguage}
          onBehaviorChange={saveBehaviorSetting}
          onDragDisabled={() => run(() => invoke('reset_island_position'))}
          islandEnabled={displays.islandEnabled}
          displaySummaryText={displaySummary(displays, t)}
          onIslandEnabled={setIslandEnabled}
          onConfigureDisplays={() => (page = 'appearance')}
        />
      {:else}
        <!-- The remaining pages with no controls: an argument, not a blank pane. -->
        <UnavailablePage title={pendingPage?.label ?? ''} reason={pendingPage?.reason ?? ''} {t} />
      {/if}
    </div>
  </div>
</div>
