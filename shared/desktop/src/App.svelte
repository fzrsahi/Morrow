<script lang="ts">
  // Render and input only (docs/plan-windows-linux.md §5.3): the shell owns
  // the sessions, the permission requests and the window; this file draws
  // what it is sent, tells the shell when the OS window may be resized, and
  // passes on the user's Approve / Deny.
  import { onMount, tick } from 'svelte';
  import { Channel, invoke } from '@tauri-apps/api/core';
  import { ambientView, pollPlan, type AmbientState, type AmbientSupport } from './lib/ambient';
  import CollapsedBar from './lib/components/CollapsedBar.svelte';
  import Header from './lib/components/Header.svelte';
  import MusicDeck from './lib/components/MusicDeck.svelte';
  import SessionList from './lib/components/SessionList.svelte';
  import PermissionCard from './lib/components/PermissionCard.svelte';
  import SettingsPanel from './lib/components/SettingsPanel.svelte';
  import {
    COLLAPSED_SIZE,
    CLOSE_TRANSITION,
    COLLAPSE_DELAY_MS,
    OPEN_TRANSITION,
    OPEN_WIDTH,
    CollapseGate,
    IslandFit,
    QuitGate,
    islandShape,
    resolveVariant,
    type PanelTab,
    type SessionFilter,
    type Size,
  } from './lib/panel';
  import {
    collapsedWidth,
    controls,
    deckState,
    localPosition,
    refreshPlan,
    seekOffsetUs,
    type LyricLine,
    type Track,
  } from './lib/music';
  import { MOTION, cssVariables } from './lib/tokens';
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

  // B (floating pill) is the default: it clears the GNOME top bar. Variant A
  // covers the clock and is used only when a maintainer asks for ?variant=A.
  const variant = resolveVariant(window.location.search);
  const vars = cssVariables();
  const css = Object.entries(vars)
    .map(([key, value]) => `${key}:${value}`)
    .join(';');
  const style = [
    css,
    `--open-dur:${OPEN_TRANSITION.durationMs}ms`,
    `--open-ease:${OPEN_TRANSITION.css}`,
    `--close-dur:${CLOSE_TRANSITION.durationMs}ms`,
    `--close-ease:${CLOSE_TRANSITION.css}`,
    `--panel-w:${OPEN_WIDTH}px`,
  ].join(';');

  let sessions = $state<Session[]>([]);
  let cards = $state<Card[]>([]);
  let expanded = $state(false);
  let closing = $state(false);
  let prehover = $state(false);
  let openHeight = $state<number | null>(null);
  let tab = $state<PanelTab>('agents');
  let filter = $state<SessionFilter>('all');
  let muted = $state(false);
  let quitArmed = $state(false);
  let settingsOpen = $state(false);
  let hooks = $state<HooksSnapshot | null>(null);
  let capabilities = $state<CapabilityRow[]>([]);
  let autostart = $state<AutostartStatus | null>(null);
  let hooksBusy = $state(false);
  let hooksError = $state<string | null>(null);
  let island: HTMLElement;
  let panelContent: HTMLElement;

  const fit = new IslandFit();
  const collapseGate = new CollapseGate();
  const quitGate = new QuitGate();
  let hoverTimer: ReturnType<typeof setTimeout>;
  let collapseTimer: ReturnType<typeof setTimeout>;
  let openTimer: ReturnType<typeof setTimeout>;
  let closeTimer: ReturnType<typeof setTimeout>;
  let quitTimer: ReturnType<typeof setTimeout>;

  // #197: the three scene facts, and which adapters can read them here.
  let ambient = $state<AmbientState>({ fullscreen: false, locked: false, quiet: false });
  let support = $state<AmbientSupport>({ fullscreenDetection: false, screenLock: false });
  let view = $derived(ambientView(ambient, support));
  let pollTimer: ReturnType<typeof setTimeout> | undefined;
  const POLL_MS = 1000;

  // #193: the shell reads MPRIS/SMTC; the deck ticks from an anchor on this
  // webview's clock. No player probe is scheduled while the Music tab is shut.
  let track = $state<Track | null>(null);
  let lyrics = $state<LyricLine[] | null>(null);
  let positionUs = $state(0);
  let anchorMs = 0;
  let generation = 0;
  let lastCollapsedWidth: number | null = null;
  let tickTimer: ReturnType<typeof setInterval> | undefined;
  let refreshTimer: ReturnType<typeof setInterval> | undefined;
  let deck = $derived(deckState(track));
  let enabled = $derived(controls(track));
  let barWidth = $derived(collapsedWidth(track));

  function answer(id: number, decision: Decision) {
    invoke('answer_permission', { id, decision });
  }

  // The E2E-1 seam: after every fit the shell prints the buttons' rectangles.
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

  // One IPC call carries the size *and* the hit-test shape. Windows used to
  // get a single `h / 2` radius, which swallowed clicks in the square top
  // corners of the open panel (#182).
  function reportFit(size: Size, isExpanded: boolean) {
    const shape = islandShape(
      variant,
      isExpanded,
      Math.round(size.width),
      Math.round(size.height),
    );
    invoke('set_island_size', {
      width: size.width,
      height: size.height,
      radii: {
        topLeft: shape.topLeft,
        topRight: shape.topRight,
        bottomLeft: shape.bottomLeft,
        bottomRight: shape.bottomRight,
      },
      shoulders: shape.shoulders,
    })
      .then(reportCards)
      .catch((error) => (hooksError = String(error)));
  }

  // The music wing widens the collapsed window from 240 to 292 pt. Report that
  // only when it actually changes, so the #182 single-fit path stays intact.
  function reportCollapsedFit() {
    if (lastCollapsedWidth === barWidth) return;
    lastCollapsedWidth = barWidth;
    reportFit({ width: barWidth, height: COLLAPSED_SIZE.height }, false);
  }

  function measure(): Size {
    // The content is laid out at the fixed 680 pt width even while the island
    // is collapsed, so the final size is known before the animation starts.
    return { width: panelContent.offsetWidth, height: panelContent.offsetHeight + 8 };
  }

  function settleOpen() {
    if (fit.phase !== 'opening') return;
    fit.opened();
    const size = measure();
    // A tab switched during the opening animation reaches us here, not in the
    // content effect: the DOM box must catch up with the size we report.
    openHeight = size.height;
    for (const fitSize of fit.layout(size)) reportFit(fitSize, true);
  }

  function openPanel() {
    clearTimeout(hoverTimer);
    clearTimeout(openTimer);
    clearTimeout(collapseTimer);
    if (fit.phase !== 'collapsed') return;
    const final = measure();
    openHeight = final.height;
    // Plan §6: the OS window is at its final size before the animation.
    for (const size of fit.open(final)) reportFit(size, true);
    expanded = true;
    openTimer = setTimeout(settleOpen, OPEN_TRANSITION.durationMs + 40);
  }

  function closePanel() {
    clearTimeout(collapseTimer);
    clearTimeout(openTimer);
    clearTimeout(closeTimer);
    if (fit.phase !== 'open') return;
    // Keep the OS window at the open size while the panel animates shut.
    fit.close();
    expanded = false;
    closing = true;
    closeTimer = setTimeout(() => {
      closing = false;
      openHeight = null;
      const collapsed = { width: barWidth, height: COLLAPSED_SIZE.height };
      for (const size of fit.closed(collapsed)) {
        lastCollapsedWidth = size.width;
        reportFit(size, false);
      }
    }, CLOSE_TRANSITION.durationMs + 40);
  }

  function onPointerEnter() {
    clearTimeout(collapseTimer);
    collapseGate.enter();
    if (fit.phase === 'collapsed') {
      prehover = true;
      clearTimeout(hoverTimer);
      hoverTimer = setTimeout(openPanel, MOTION.dwell * 1000);
    }
  }

  function onPointerLeave() {
    clearTimeout(hoverTimer);
    prehover = false;
    collapseGate.leave(performance.now());
    collapseTimer = setTimeout(() => {
      if (collapseGate.shouldCollapse(performance.now(), cards.length > 0)) closePanel();
    }, COLLAPSE_DELAY_MS);
  }

  function onQuit() {
    clearTimeout(quitTimer);
    if (quitGate.click(performance.now()) === 'quit') {
      invoke('quit');
      return;
    }
    quitArmed = true;
    quitTimer = setTimeout(() => {
      quitGate.reset();
      quitArmed = false;
    }, 3000);
  }

  async function onMute() {
    try {
      await invoke('music_volume_mute');
      muted = !muted;
    } catch (error) {
      hooksError = String(error);
    }
  }

  // #193: one read asks the player, then the deck interpolates locally. The
  // generation guards against a slow lyrics answer landing on a newer track.
  async function readTrack() {
    const mine = ++generation;
    try {
      const next = await invoke<Track | null>('music_track');
      if (mine !== generation) return;
      const changed = (next?.title ?? '') !== (track?.title ?? '') ||
        (next?.artist ?? '') !== (track?.artist ?? '') ||
        (next?.player ?? '') !== (track?.player ?? '');
      track = next;
      anchorMs = performance.now();
      positionUs = next?.position_us ?? 0;
      // A refresh of the same track keeps the old lines until the answer
      // lands; only a new track clears them.
      if (changed) lyrics = null;
      if (next) {
        const lines = await invoke<LyricLine[] | null>('music_lyrics', { track: next });
        if (mine === generation) lyrics = lines;
      }
    } catch (error) {
      hooksError = String(error);
    }
  }

  async function command(name: string, args?: Record<string, unknown>) {
    try {
      await invoke(name, args);
      await readTrack();
    } catch (error) {
      hooksError = String(error);
    }
  }

  function onSeek(fraction: number) {
    if (!track || !enabled.seek || track.length_us === null) return;
    return command('music_seek', {
      offset_us: seekOffsetUs(track, positionUs, fraction),
    });
  }

  // Only the open Music tab schedules work: a position tick and a slow player
  // re-read. Both timers die on tab switch, collapse, lock or unmount.
  $effect(() => {
    const plan = refreshPlan(tab, expanded && !view.suspended);
    if (!plan.active) return;
    void readTrack();
    tickTimer = setInterval(() => {
      positionUs = localPosition(
        track?.position_us ?? 0,
        track?.status ?? 'Stopped',
        track?.length_us ?? null,
        anchorMs,
        performance.now(),
      );
    }, 250);
    refreshTimer = setInterval(() => void readTrack(), plan.intervalMs);
    return () => {
      if (tickTimer !== undefined) clearInterval(tickTimer);
      if (refreshTimer !== undefined) clearInterval(refreshTimer);
      tickTimer = undefined;
      refreshTimer = undefined;
    };
  });

  // Report the 240 ↔ 292 pt transition without adding a fit to the open path.
  $effect(() => {
    void barWidth;
    if (expanded || closing) return;
    reportCollapsedFit();
  });

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

  async function toggleTarget(row: TargetStatus) {
    await run(() =>
      invoke(row.installed ? 'target_uninstall' : 'target_install', { source: row.source }),
    );
  }

  async function toggleOpencode(row: OpencodeStatus) {
    await run(() =>
      invoke(row.plugin_installed ? 'opencode_uninstall' : 'opencode_install'),
    );
  }

  async function toggleKimi(row: KimiStatus) {
    await run(() => invoke(row.installed ? 'kimi_uninstall' : 'kimi_install'));
  }

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

  onMount(() => {
    const channel = new Channel<Snapshot>();
    channel.onmessage = (snapshot) => {
      sessions = snapshot.sessions;
      cards = snapshot.permissions;
    };
    invoke('subscribe', { channel });
    loadHooks();
    loadCapabilities();
    // One seed read so the collapsed wing can show a track; the tab effect
    // owns every read after this, so a closed tab never polls.
    void readTrack();
    reportCollapsedFit();

    return () => {
      clearTimeout(hoverTimer);
      clearTimeout(collapseTimer);
      clearTimeout(openTimer);
      clearTimeout(closeTimer);
      clearTimeout(quitTimer);
      if (pollTimer !== undefined) clearTimeout(pollTimer);
      if (tickTimer !== undefined) clearInterval(tickTimer);
      if (refreshTimer !== undefined) clearInterval(refreshTimer);
      pollTimer = undefined;
      tickTimer = undefined;
      refreshTimer = undefined;
    };
  });

  // Content changes while the panel is open need one fit; the controller
  // ignores the frames produced by the width/height animation itself.
  $effect(() => {
    const content =
      sessions.length + cards.length + Number(settingsOpen) + tab.length +
      (track ? 1 : 0) + (lyrics?.length ?? 0);
    if (!expanded) return;
    void content;
    void tick().then(() => {
      if (fit.phase !== 'open') return;
      const size = measure();
      // The deck (and the Settings panel) can be taller than the tab the
      // panel opened on: keep the DOM box in step with the reported OS size.
      openHeight = size.height;
      for (const fitSize of fit.layout(size)) reportFit(fitSize, true);
    });
  });
</script>

<div
  class="island"
  class:open={expanded}
  class:closing
  class:locked={view.suspended}
  data-variant={variant}
  data-phase={fit.phase}
  {style}
  style:width={expanded ? undefined : `${barWidth}px`}
  style:height={openHeight === null ? undefined : `${openHeight}px`}
  bind:this={island}
  onpointerenter={onPointerEnter}
  onpointerleave={onPointerLeave}
  role="presentation"
>
  {#if !expanded && !closing}
    <button class="toggle" onclick={openPanel} aria-label="Open the Morrow panel">
      <CollapsedBar sessionCount={sessions.length} {prehover} quiet={view.quiet} {track} />
    </button>
  {/if}
  <div class="panel-content" class:visible={expanded || closing} bind:this={panelContent}>
    <Header
      {tab}
      {filter}
      {muted}
      {quitArmed}
      {variant}
      onTab={(next) => (tab = next)}
      onFilter={(next) => (filter = next)}
      onMute={onMute}
      onSettings={() => (settingsOpen = !settingsOpen)}
      onQuit={onQuit}
    />
    {#if tab === 'agents'}
      <div class="padded"><SessionList {sessions} /></div>
      {#if settingsOpen}
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
    {:else}
      <MusicDeck
        {track}
        state={deck}
        {lyrics}
        {positionUs}
        {enabled}
        onPlayPause={() => command('music_play_pause')}
        onNext={() => command('music_next')}
        onPrevious={() => command('music_previous')}
        onSeek={onSeek}
      />
    {/if}
    {#if cards.length > 0}
      <div class="padded cards">
        {#each cards as card (card.id)}
          <PermissionCard {card} onAnswer={answer} />
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .island {
    --fit-dur: var(--close-dur);
    --fit-ease: var(--close-ease);
    position: absolute;
    top: 0;
    left: 0;
    box-sizing: border-box;
    width: 240px;
    height: 34px;
    overflow: hidden;
    background: var(--chassis);
    color: var(--tw);
    font: 11px/1.2 system-ui, sans-serif;
    text-align: left;
    transform-origin: top center;
    border-radius: 0 0 12px 12px;
    transition:
      width var(--fit-dur) var(--fit-ease),
      height var(--fit-dur) var(--fit-ease),
      border-radius var(--fit-dur) var(--fit-ease);
  }
  .island.open {
    --fit-dur: var(--open-dur);
    --fit-ease: var(--open-ease);
    width: var(--panel-w);
    border-radius: 0 0 24px 24px;
  }
  .island[data-variant='B'] {
    border-radius: 999px;
    box-shadow:
      inset 0 0 0 1.2px rgba(255, 255, 255, 0.24),
      0 4px 10px rgba(0, 0, 0, 0.45);
  }
  .island[data-variant='B'].open {
    border-radius: 24px;
  }
  .island[data-variant='C']::before,
  .island[data-variant='C']::after {
    content: '';
    position: absolute;
    top: 0;
    width: 10px;
    height: 10px;
    background: var(--chassis);
  }
  .island[data-variant='C']::before {
    left: -10px;
    border-radius: 0 0 0 10px;
  }
  .island[data-variant='C']::after {
    right: -10px;
    border-radius: 0 0 10px 0;
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
    height: 34px;
    margin: 0;
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-align: inherit;
    cursor: default;
  }
  .panel-content {
    position: absolute;
    top: 0;
    left: 0;
    box-sizing: border-box;
    width: var(--panel-w);
    padding-bottom: 8px;
    visibility: hidden;
    opacity: 0;
    filter: blur(5px);
    transform: scale(0.96);
    transform-origin: top center;
    transition:
      opacity 0.25s ease-out,
      filter 0.25s ease-out,
      transform 0.25s ease-out;
  }
  .panel-content.visible {
    visibility: visible;
    opacity: 1;
    filter: none;
    transform: none;
    transition-delay: 0.08s;
  }
  .padded {
    padding: 0 14px;
  }
  .cards {
    border-top: 1px solid rgba(255, 255, 255, 0.08);
    margin-top: 4px;
  }
  :focus-visible {
    outline: 2px solid var(--cyan);
    outline-offset: -2px;
  }
  @media (prefers-reduced-motion: reduce) {
    .island,
    .panel-content {
      transition: none;
    }
  }
</style>
