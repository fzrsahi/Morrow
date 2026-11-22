<script lang="ts">
  // Render and input only (docs/plan-windows-linux.md §5.3): the shell owns
  // the sessions, the permission requests and the window; this file draws
  // what it is sent, reports the island's size so the shell can fit the
  // window to it, and passes on the user's Approve / Deny.
  import { onMount } from 'svelte';
  import { Channel, invoke } from '@tauri-apps/api/core';
  import { ambientView, pollPlan, type AmbientState, type AmbientSupport } from './lib/ambient';
  import CollapsedBar from './lib/components/CollapsedBar.svelte';
  import SessionList from './lib/components/SessionList.svelte';
  import PermissionCard from './lib/components/PermissionCard.svelte';
  import SettingsPanel from './lib/components/SettingsPanel.svelte';
  import type {
    AutostartStatus,
    CapabilityRow,
    Card,
    Decision,
    HooksSnapshot,
    KimiStatus,
    OpencodeStatus,
    Session,
    Snapshot,
    TargetStatus,
    YamlStatus,
  } from './lib/types';

  let sessions = $state<Session[]>([]);
  let cards = $state<Card[]>([]);
  let expanded = $state(false);
  let hooks = $state<HooksSnapshot | null>(null);
  let capabilities = $state<CapabilityRow[]>([]);
  let autostart = $state<AutostartStatus | null>(null);
  let hooksBusy = $state(false);
  let hooksError = $state<string | null>(null);
  let island: HTMLElement;

  // #197: the three scene facts, and which adapters can read them here.
  let ambient = $state<AmbientState>({ fullscreen: false, locked: false, quiet: false });
  let support = $state<AmbientSupport>({ fullscreenDetection: false, screenLock: false });
  let view = $derived(ambientView(ambient, support));
  let pollTimer: ReturnType<typeof setTimeout> | undefined;
  const POLL_MS = 1000;

  function answer(id: number, decision: Decision) {
    invoke('answer_permission', { id, decision });
  }

  async function loadCapabilities() {
    try {
      capabilities = await invoke<CapabilityRow[]>('capabilities');
      autostart = await invoke<AutostartStatus>('autostart_status');
      const supported = (feature: string) =>
        capabilities.some(
          (row) => row.feature === feature && 'Supported' in row.capability,
        );
      support = {
        fullscreenDetection: supported('Fullscreen detection'),
        screenLock: supported('Screen lock'),
      };
      schedulePoll();
    } catch (error) {
      hooksError = String(error);
    }
  }

  // #197: ask only the adapters this machine can answer. Read the lock first:
  // a screen that just locked must not run a content probe on this tick.
  async function pollOnce() {
    const next: Partial<AmbientState> = {};
    const p = pollPlan(view, support);
    if (!p.active) return;
    if (p.screenLocked) {
      next.locked = (await invoke<boolean | null>('screen_locked')) === true;
    }
    const after = pollPlan(ambientView({ ...ambient, ...next }, support), support);
    if (after.fullscreen) next.fullscreen = await invoke<boolean>('fullscreen');
    if (after.quietHours) next.quiet = await invoke<boolean>('quiet_hours');
    Object.assign(ambient, next);
  }

  // A recursive timeout, not an interval: when the plan has no probes left
  // there is no next tick at all, and unmount clears the one in flight.
  function schedulePoll() {
    if (pollTimer !== undefined) clearTimeout(pollTimer);
    pollTimer = undefined;
    if (!pollPlan(view, support).active) return;
    pollTimer = setTimeout(async () => {
      pollTimer = undefined;
      try {
        await pollOnce();
      } finally {
        schedulePoll();
      }
    }, POLL_MS);
  }

  // The window, not the DOM, hides for a fullscreen app: the island keeps its
  // measured size so card rectangles and the tight-fit chain stay intact.
  $effect(() => {
    const hidden = view.hidden;
    invoke('island_visible', { visible: !hidden }).catch(() => {});
  });

  async function toggleAutostart() {
    await run(() => invoke('autostart_set', { enabled: !autostart?.installed }));
  }

  async function loadHooks() {
    try {
      hooks = await invoke<HooksSnapshot>('hooks_status');
      hooksError = null;
    } catch (error) {
      hooksError = String(error);
    }
  }

  async function runHooks(command: 'hooks_install' | 'hooks_uninstall') {
    await run(() => invoke(command));
  }

  // One of the other CLIs: install when its file has no hook of ours, remove
  // when it has. The shell skips a CLI that is not on this machine.
  async function toggleTarget(row: TargetStatus) {
    await run(() =>
      invoke(row.installed ? 'target_uninstall' : 'target_install', { source: row.source }),
    );
  }

  // OpenCode is a plugin file, not a hooks key, so it gets its own row.
  async function toggleOpencode(row: OpencodeStatus) {
    await run(() =>
      invoke(row.plugin_installed ? 'opencode_uninstall' : 'opencode_install'),
    );
  }

  // Kimi keeps its hooks in a TOML file; our blocks are appended to it.
  async function toggleKimi(row: KimiStatus) {
    await run(() => invoke(row.installed ? 'kimi_uninstall' : 'kimi_install'));
  }

  // The YAML sources: one managed block, and a refusal is shown rather than
  // silently skipped.
  async function toggleYaml(row: YamlStatus) {
    await run(() =>
      invoke(row.installed ? 'yaml_uninstall' : 'yaml_install', { source: row.source }),
    );
  }

  async function run(action: () => Promise<unknown>) {
    hooksBusy = true;
    try {
      await action();
      await loadHooks();
    } catch (error) {
      hooksError = String(error);
    } finally {
      hooksBusy = false;
    }
  }

  // For the E2E-1 gate (the shell prints them only with MORROW_SMOKE=1).
  function reportCards() {
    const rect = (el: Element | null) => {
      const r = el!.getBoundingClientRect();
      return { x: r.x, y: r.y, width: r.width, height: r.height };
    };
    for (const el of island.querySelectorAll<HTMLElement>('[data-card]')) {
      invoke('smoke_card', {
        id: Number(el.dataset.card),
        approve: rect(el.querySelector('.approve')),
        deny: rect(el.querySelector('.deny')),
      });
    }
  }

  onMount(() => {
    const channel = new Channel<Snapshot>();
    channel.onmessage = (snapshot) => {
      sessions = snapshot.sessions;
      cards = snapshot.permissions;
    };
    invoke('subscribe', { channel });
    loadHooks();
    loadCapabilities();

    // Tight-fit window: the window is as big as the island and no bigger.
    // Measured after every layout change (collapse, expand, a row, a card).
    const observer = new ResizeObserver(() => {
      invoke('set_island_size', { width: island.offsetWidth, height: island.offsetHeight }).then(
        reportCards,
      );
    });
    observer.observe(island);
    return () => {
      observer.disconnect();
      if (pollTimer !== undefined) clearTimeout(pollTimer);
      pollTimer = undefined;
    };
  });
</script>

<div
  class="island"
  class:wide={expanded || cards.length > 0}
  class:locked={view.suspended}
  bind:this={island}
>
  <button class="toggle" onclick={() => (expanded = !expanded)}>
    {#if expanded || cards.length > 0}
      <div class="pill">
        <span class="dot" class:live={sessions.length > 0}></span>
        {#if sessions.length > 0}
          <span class="count">{sessions.length}</span>
          <span class="source">{sessions[0].source}</span>
        {:else}
          <span class="source">Morrow</span>
        {/if}
      </div>
    {:else}
      <CollapsedBar sessionCount={sessions.length} quiet={view.quiet} />
    {/if}
    {#if expanded}
      <div class="padded"><SessionList {sessions} /></div>
    {/if}
  </button>
  {#if expanded}
    <div class="padded">
      <SettingsPanel
        {hooks}
        {capabilities}
        {autostart}
        {hooksBusy}
        {hooksError}
        onRunHooks={runHooks}
        onToggleTarget={toggleTarget}
        onToggleOpencode={toggleOpencode}
        onToggleKimi={toggleKimi}
        onToggleYaml={toggleYaml}
        onToggleAutostart={toggleAutostart}
      />
    </div>
  {/if}
  {#each cards as card (card.id)}
    <div class="padded"><PermissionCard {card} onAnswer={answer} /></div>
  {/each}
</div>

<style>
  /* Fixed widths, so the measured size never depends on the window's. */
  .island {
    position: absolute;
    top: 0;
    left: 0;
    box-sizing: border-box;
    width: 240px;
    margin: 0;
    padding: 0;
    border: 0;
    border-radius: 0 0 17px 17px;
    background: #000;
    color: #f2f2f2;
    font: 13px/1.2 system-ui, sans-serif;
    text-align: left;
    cursor: default;
  }
  .island.wide {
    width: 360px;
    padding-bottom: 10px;
    border-radius: 0 0 20px 20px;
  }
  /* #197: a locked screen is drawn but inert — no sound (none is wired yet),
     no animation, no content probes. The lock watch itself is still polled so
     the island can come back on unlock. Reduced-motion stays honoured in the
     components; this rule is independent of that preference. */
  .island.locked :global(*) {
    animation: none !important;
    transition: none !important;
  }
  .toggle {
    display: block;
    width: 100%;
    margin: 0;
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-align: inherit;
    cursor: default;
  }
  .padded {
    padding: 0 14px;
  }
  .pill {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 14px;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #555;
  }
  .dot.live {
    background: #34c759;
  }
  .count {
    font-weight: 600;
  }
  .source {
    color: #d0d0d0;
  }
</style>
