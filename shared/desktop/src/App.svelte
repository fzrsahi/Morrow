<script lang="ts">
  // Render and input only (docs/plan-windows-linux.md §5.3): the shell owns
  // the sessions, the permission requests and the window; this file draws
  // what it is sent, tells the shell when the OS window may be resized, and
  // passes on the user's Approve / Deny.
  import { onMount, tick } from 'svelte';
  import { Channel, invoke } from '@tauri-apps/api/core';
  import { ambientView, pollPlan, type AmbientState, type AmbientSupport } from './lib/ambient';
  import CollapsedBar from './lib/components/CollapsedBar.svelte';
  import CompletionCard from './lib/components/CompletionCard.svelte';
  import Header from './lib/components/Header.svelte';
  import PixelText from './lib/components/PixelText.svelte';
  import SessionList from './lib/components/SessionList.svelte';
  import PermissionCard from './lib/components/PermissionCard.svelte';
  import {
    COMPLETION_AUTO_COLLAPSE_MS,
    CompletionGate,
    completionKey,
    shouldAutoOpen,
  } from './lib/completion';
  import {
    COLLAPSED_SIZE,
    CLOSE_TRANSITION,
    COLLAPSE_DELAY_MS,
    OPEN_TRANSITION,
    OPEN_WIDTH,
    CollapseGate,
    DragGate,
    IslandFit,
    QuitGate,
    islandShape,
    resolveVariant,
    snapOffset,
    type Offset,
    type PanelTab,
    type SessionDensity,
    type SessionFilter,
    type Size,
  } from './lib/panel';
  import {
    SETTINGS_KEYS,
    sessionDensityFromSettings,
    sessionFilterFromSettings,
  } from './lib/settings';
  import { activeSession, collapsedWidth } from './lib/session-cards';
  import { MOTION, PALETTE, cssVariables } from './lib/tokens';
  import type { CapabilityRow, Card, Completion, Decision, Session, Snapshot } from './lib/types';

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
  // #191: the last finished turn. The shell has already decided (via the
  // attention policy) whether it is a card, a glance or nothing at all.
  let completion = $state<Completion | null>(null);
  let seenCompletion = $state<string | null>(null);
  let expanded = $state(false);
  let closing = $state(false);
  let prehover = $state(false);
  let openHeight = $state<number | null>(null);
  let tab = $state<PanelTab>('agents');
  let filter = $state<SessionFilter>('all');
  // #185: #183's cards stay reachable; a fresh install uses compact rows.
  let density = $state<SessionDensity>('compact');
  let muted = $state(false);
  let quitArmed = $state(false);
  // #199: settings no longer open inside the island; hooksError is still the
  // shell's error line for fits and ambient probes.
  let hooksError = $state<string | null>(null);
  let island: HTMLElement;
  let panelContent: HTMLElement;

  const fit = new IslandFit();
  const collapseGate = new CollapseGate();
  const completionGate = new CompletionGate();
  const quitGate = new QuitGate();
  const drag = new DragGate();
  let dragging = $state(false);
  let suppressClick = false;
  let hoverTimer: ReturnType<typeof setTimeout>;
  let collapseTimer: ReturnType<typeof setTimeout>;
  let openTimer: ReturnType<typeof setTimeout>;
  let closeTimer: ReturnType<typeof setTimeout>;
  let quitTimer: ReturnType<typeof setTimeout>;
  let completionTimer: ReturnType<typeof setTimeout> | undefined;

  // #197: the three scene facts, and which adapters can read them here.
  let ambient = $state<AmbientState>({ fullscreen: false, locked: false, quiet: false });
  let support = $state<AmbientSupport>({ fullscreenDetection: false, screenLock: false });
  let view = $derived(ambientView(ambient, support));
  let active = $derived(activeSession(sessions));
  let completionSession = $derived(
    completion ? (sessions.find((s) => s.session_id === completion!.session_id) ?? null) : null,
  );
  let collapsed = $derived({ width: collapsedWidth(active), height: COLLAPSED_SIZE.height });
  let lastCollapsedWidth = COLLAPSED_SIZE.width;
  let pollTimer: ReturnType<typeof setTimeout> | undefined;
  const POLL_MS = 1000;

  function answer(id: number, decision: Decision) {
    invoke('answer_permission', { id, decision });
  }

  // #199 — Settings moved to its own window; the header gear only asks the
  // shell to create or raise it, lazily on first use. #185's empty state uses
  // the same call: SettingsApp opens on the Hooks page by default.
  function openSettings() {
    invoke('open_settings');
  }

  // #185 — the shell owns the session store, so removing one is an input, not
  // local state. Drop it from the view at once; the next snapshot confirms.
  function deleteSession(sessionId: string) {
    sessions = sessions.filter((session) => session.session_id !== sessionId);
    invoke('session_dismiss', { sessionId }).catch((error) => (hooksError = String(error)));
  }

  // The grouping choice lives in ~/.codeisland/settings.json with the other
  // panel settings, so it survives a restart.
  function setFilter(next: SessionFilter) {
    filter = next;
    invoke('settings_set', { key: SETTINGS_KEYS.sessionFilter, value: next }).catch(
      (error) => (hooksError = String(error)),
    );
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

  function measure(): Size {
    // The content is laid out at the fixed 680 pt width even while the island
    // is collapsed, so the final size is known before the animation starts.
    return { width: panelContent.offsetWidth, height: panelContent.offsetHeight + 8 };
  }

  function settleOpen() {
    if (fit.phase !== 'opening') return;
    fit.opened();
    for (const size of fit.layout(measure())) reportFit(size, true);
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
    clearTimeout(completionTimer);
    completionTimer = undefined;
    if (fit.phase !== 'open') return;
    // Keep the OS window at the open size while the panel animates shut.
    fit.close();
    expanded = false;
    closing = true;
    closeTimer = setTimeout(() => {
      closing = false;
      openHeight = null;
      for (const size of fit.closed(collapsed)) reportFit(size, false);
    }, CLOSE_TRANSITION.durationMs + 40);
  }

  function onPointerEnter() {
    // #182's dwell must not fire mid-drag: a pointer leaving the pill on its
    // way to another spot is not the user leaving the island.
    if (dragging) return;
    clearTimeout(collapseTimer);
    collapseGate.enter();
    if (completionGate.active) completionGate.enter();
    if (fit.phase === 'collapsed') {
      prehover = true;
      clearTimeout(hoverTimer);
      hoverTimer = setTimeout(openPanel, MOTION.dwell * 1000);
    }
  }

  function onPointerLeave() {
    if (dragging) return;
    clearTimeout(hoverTimer);
    prehover = false;
    collapseGate.leave(performance.now());
    if (completionGate.active) completionGate.leave(performance.now());
    collapseTimer = setTimeout(() => {
      // Notionally "blocked": a held approval, or a completion card that is
      // still waiting for its first pointer visit (NotchPanelView.swift:1485).
      const waitingForEntry =
        completion !== null && completionGate.blocksCollapse(completionKey(completion));
      if (collapseGate.shouldCollapse(performance.now(), cards.length > 0 || waitingForEntry)) {
        closePanel();
      }
    }, COLLAPSE_DELAY_MS);
  }

  // #196: the collapsed bar is the drag handle. Pointer capture keeps moves
  // arriving after the pointer leaves the small pill.
  function onBarPointerDown(event: PointerEvent) {
    if (event.button !== 0) return;
    drag.down(event.clientX, event.clientY);
    (event.currentTarget as HTMLElement).setPointerCapture?.(event.pointerId);
  }

  function onBarPointerMove(event: PointerEvent) {
    if (!drag.move(event.clientX, event.clientY)) return;
    if (!dragging) {
      // A dwell timer from pointerenter may already be in flight: the drag
      // wins, and must not let #182 open the panel under the pointer.
      clearTimeout(hoverTimer);
      clearTimeout(openTimer);
      clearTimeout(collapseTimer);
      prehover = false;
    }
    dragging = true;
    invoke('move_island', drag.delta(event.clientX, event.clientY)).catch(
      (error) => (hooksError = String(error)),
    );
  }

  function onBarPointerUp(event: PointerEvent) {
    (event.currentTarget as HTMLElement).releasePointerCapture?.(event.pointerId);
    const moved = drag.delta(event.clientX, event.clientY);
    if (!drag.up()) return;
    const offset: Offset = snapOffset(moved);
    // Apply the snapped spot first, then write it: the file and the window
    // agree even if the app is closed immediately afterwards.
    invoke('move_island', offset)
      .then(() => invoke('commit_island_position'))
      .catch((error) => (hooksError = String(error)));
    // A click still follows pointerup; it belongs to the drag, not the toggle.
    suppressClick = true;
    setTimeout(() => (suppressClick = false), 0);
    dragging = false;
  }

  function onBarPointerCancel() {
    drag.up();
    dragging = false;
  }

  function onBarClick() {
    if (suppressClick) {
      suppressClick = false;
      return;
    }
    openPanel();
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

  async function loadCapabilities() {
    try {
      // #199: the table itself lives in the Settings window; the island only
      // needs to know which ambient adapters it may ask.
      const rows = await invoke<CapabilityRow[]>('capabilities');
      const supported = (feature: string) =>
        rows.some((row) => row.feature === feature && 'Supported' in row.capability);
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

  // #191: macOS's completionAutoCollapseDelay. A card nobody touched goes
  // away on its own; one the pointer visited waits for the pointer to leave.
  function scheduleCompletionCollapse(key: string) {
    clearTimeout(completionTimer);
    completionTimer = setTimeout(() => {
      completionTimer = undefined;
      // Still unvisited: collapse now. Visited: wait for the pointer to leave.
      if (completionGate.active && completionGate.blocksCollapse(key)) closePanel();
    }, COMPLETION_AUTO_COLLAPSE_MS);
  }

  function acceptSnapshot(snapshot: Snapshot) {
    sessions = snapshot.sessions;
    cards = snapshot.permissions;
    const next = snapshot.completion;
    completion = next;
    if (!next) {
      seenCompletion = null;
      completionGate.end();
      clearTimeout(completionTimer);
      completionTimer = undefined;
      return;
    }
    const key = completionKey(next);
    if (!shouldAutoOpen(next, seenCompletion)) return;
    seenCompletion = key;
    completionGate.begin(key);
    // Measure the reply's height only after its DOM exists, so the open path
    // still makes exactly one tight-fit call before the animation.
    void tick().then(() => openPanel());
    scheduleCompletionCollapse(key);
  }

  // The window, not the DOM, hides for a fullscreen app: the island keeps its
  // measured size so card rectangles and the tight-fit chain stay intact.
  $effect(() => {
    const hidden = view.hidden;
    invoke('island_visible', { visible: !hidden }).catch(() => {});
  });

  onMount(() => {
    const channel = new Channel<Snapshot>();
    channel.onmessage = acceptSnapshot;
    invoke('subscribe', { channel });
    invoke<unknown>('settings_load')
      .then((stored) => {
        filter = sessionFilterFromSettings(stored);
        density = sessionDensityFromSettings(stored);
      })
      .catch(() => {});
    loadCapabilities();
    lastCollapsedWidth = collapsed.width;
    reportFit(collapsed, false);

    return () => {
      clearTimeout(hoverTimer);
      clearTimeout(collapseTimer);
      clearTimeout(openTimer);
      clearTimeout(closeTimer);
      clearTimeout(quitTimer);
      clearTimeout(completionTimer);
      if (pollTimer !== undefined) clearTimeout(pollTimer);
      pollTimer = undefined;
    };
  });

  // #183: a card's words can change while its count stays the same, so the
  // fit trigger reads the rendered fields too. It still goes through this
  // one path; IslandFit drops a size that has not changed.
  let sessionContent = $derived(
    sessions
      .map(
        (s) =>
          `${s.session_id}:${s.status}:${s.current_tool}:${s.tool_description}:${s.last_user_prompt}:${s.last_assistant_message}:${s.updated_at}`,
      )
      .join('|'),
  );

  // #183: the working bar grows for thinking/tool labels and the workspace
  // dock. One tight-fit call per change (batched by tick), not per frame.
  $effect(() => {
    const width = collapsed.width;
    if (expanded || closing || width === lastCollapsedWidth) return;
    void tick().then(() => {
      if (fit.phase !== 'collapsed') return;
      lastCollapsedWidth = width;
      reportFit({ width, height: COLLAPSED_SIZE.height }, false);
    });
  });

  // Content changes while the panel is open need one fit; the controller
  // ignores the frames produced by the width/height animation itself.
  $effect(() => {
    const content = `${sessionContent}|${cards.length}|${tab}|${filter}|${density}|${completion ? completionKey(completion) : ''}`;
    if (!expanded) return;
    void content;
    void tick().then(() => {
      if (fit.phase !== 'open') return;
      for (const size of fit.layout(measure())) reportFit(size, true);
    });
  });
</script>

<div
  class="island"
  class:open={expanded}
  class:closing
  class:dragging
  class:locked={view.suspended}
  data-variant={variant}
  data-phase={fit.phase}
  {style}
  style:width={expanded || closing ? undefined : `${collapsed.width}px`}
  style:height={openHeight === null ? undefined : `${openHeight}px`}
  bind:this={island}
  onpointerenter={onPointerEnter}
  onpointerleave={onPointerLeave}
  role="presentation"
>
  {#if !expanded && !closing}
    <button
      class="toggle"
      onclick={onBarClick}
      onpointerdown={onBarPointerDown}
      onpointermove={onBarPointerMove}
      onpointerup={onBarPointerUp}
      onpointercancel={onBarPointerCancel}
      aria-label="Open the Morrow panel"
    >
      <CollapsedBar
        sessionCount={sessions.length}
        {prehover}
        quiet={view.quiet}
        {active}
        glance={completion !== null && completion.presentation !== 'status_only'}
        glanceLabel={completionSession?.cwd_name ?? ''}
      />
      <span class="grip" aria-hidden="true"></span>
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
      onFilter={setFilter}
      onMute={onMute}
      onSettings={openSettings}
      onQuit={onQuit}
    />
    {#if tab === 'agents'}
      <div class="padded">
        {#if completion && completionSession && completion.presentation !== 'status_only'}
          <CompletionCard
            {completion}
            session={completionSession}
            sessionCount={sessions.length}
          />
        {:else}
          <SessionList
            {sessions}
            {filter}
            {density}
            onDelete={deleteSession}
            onOpenSettings={openSettings}
          />
        {/if}
      </div>
    {:else}
      <div class="music-placeholder"><PixelText text="MUSIC" size={1.3} color={PALETTE.cyan} /></div>
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
  /* #196: a dragged island is still the same pill, just elsewhere; the
     dashed "home" slot exists only on the canvas, not on the user's screen. */
  .island.dragging {
    cursor: grabbing;
    box-shadow:
      0 18px 40px rgba(0, 0, 0, 0.55),
      inset 0 0 0 1.2px rgba(0, 245, 212, 0.55);
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
    position: relative;
    width: 100%;
    height: 34px;
    margin: 0;
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-align: inherit;
    cursor: grab;
  }
  .toggle .grip {
    position: absolute;
    left: 50%;
    bottom: 3px;
    width: 28px;
    height: 3px;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.28);
    transform: translateX(-50%);
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.15s ease;
  }
  .toggle:hover .grip,
  .island.dragging .grip {
    opacity: 1;
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
  .music-placeholder {
    display: grid;
    place-items: center;
    height: 90px;
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
