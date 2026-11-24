<script lang="ts">
  // Render and input only (docs/plan-windows-linux.md §5.3): the shell owns
  // the sessions, the permission requests and the window; this file draws
  // what it is sent, tells the shell when the OS window may be resized, and
  // passes on the user's Approve / Deny.
  import { onMount, tick, untrack } from 'svelte';
  import { Channel, invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { GONE_NOTICE_MS } from './lib/approval';
  import {
    ambientView,
    pollPlan,
    probesActive,
    type AmbientState,
    type AmbientSupport,
  } from './lib/ambient';
  import { isSupported } from './lib/capabilities';
  import CollapsedBar from './lib/components/CollapsedBar.svelte';
  import CompletionCard from './lib/components/CompletionCard.svelte';
  import Header from './lib/components/Header.svelte';
  import MusicDeck from './lib/components/MusicDeck.svelte';
  import SessionList from './lib/components/SessionList.svelte';
  import PermissionCard from './lib/components/PermissionCard.svelte';
  import QuestionCard from './lib/components/QuestionCard.svelte';
  import VolumeHud from './lib/components/VolumeHud.svelte';
  import {
    answersOf,
    canConfirm,
    goBack,
    goNext,
    isQuestion,
    pickIndex,
    progressChip,
    promptsOf,
    putWizard,
    QuestionFocusGate,
    togglePick,
    wizardFor,
    type WizardState,
  } from './lib/question';
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
    type DisplayMode,
    type Offset,
    type PanelTab,
    type SessionDensity,
    type SessionFilter,
    type Size,
  } from './lib/panel';
  import {
    SETTINGS_CHANGED_EVENT,
    SETTINGS_KEYS,
    appearanceFromSettings,
    sessionDensityFromSettings,
    sessionFilterFromSettings,
  } from './lib/settings';
  import { activeSession, collapsedWidth } from './lib/session-cards';
  import type { TerminalOutcome } from './lib/terminal-jump';
  import {
    COLLAPSED_MUSIC_WIDTH,
    LyricCache,
    controls,
    deckState,
    localPosition,
    refreshPlan,
    seekOffsetUs,
    type LyricLine,
    type Track,
  } from './lib/music';
  import { MOTION, PALETTE, cssVariables, type Theme } from './lib/tokens';
  import { VOLUME_HUD_MS, VolumeHudGate, setVolumePlan, volumeSupported, type Volume } from './lib/volume';
  import type { CapabilityRow, Card, Completion, Decision, Session, Snapshot } from './lib/types';

  // The display mode mirrors Swift's IslandDisplayMode: `.dynamicIsland` is
  // canvas B (floating pill, default — no hardware notch on Windows/Linux),
  // `.notch` is canvas C (docked shell with shoulder cut-outs). Both can be
  // chosen in Settings → Appearance. ?variant=B|C stays a maintainer override.
  let displayMode = $state<DisplayMode>('dynamicIsland');
  let variant = $derived(resolveVariant(window.location.search, displayMode));
  // #202: the theme is a view state, not a URL flag. It loads from the same
  // settings document as the filter and changes live when Settings writes it.
  let theme = $state<Theme>('pixel');
  const style = $derived([
    ...Object.entries(cssVariables(11, theme)).map(([key, value]) => `${key}:${value}`),
    `--open-dur:${OPEN_TRANSITION.durationMs}ms`,
    `--open-ease:${OPEN_TRANSITION.css}`,
    `--close-dur:${CLOSE_TRANSITION.durationMs}ms`,
    `--close-ease:${CLOSE_TRANSITION.css}`,
    `--panel-w:${OPEN_WIDTH}px`,
  ].join(';'));

  let sessions = $state<Session[]>([]);
  let cards = $state<Card[]>([]);
  // #186: Hide keeps the request pending but out of the card stack; a click
  // on a request the shell no longer holds shows a notice before it drops.
  let hiddenIds = $state<number[]>([]);
  let gone = $state<{ card: Card }[]>([]);
  // #187: the jump outcome belongs to the session, so it survives the card
  // being answered and re-issued. The card itself stays in the queue.
  let jumpOutcomes = $state<Record<string, TerminalOutcome>>({});
  // #188: the option being answered, held while the shell writes the reply so
  // a click (or a number key) shows the canvas' selected row until it lands.
  let selected = $state<{ id: number; label: string } | null>(null);
  // #190: each request id owns its wizard, so a pick can never leak into a
  // different permission request (or a reused id after the old one is dropped).
  let wizards = $state<Record<number, WizardState>>({});
  let hotkeySupported = $state(false);
  // #189: the typed field that borrowed the island's keyboard focus.
  const questionFocus = new QuestionFocusGate();
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
  // #201: pure timing/decision state; this component only arms the timer and
  // calls the shell.
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
  // #201: no reminder on a machine without notify-send; ids whose follow-up
  // went out keep the collapsed bar's bell until the request leaves the feed.
  let notificationsCapable = $state(false);
  let reminderIds = $state<number[]>([]);

  // #193: the shell reads MPRIS/SMTC; the deck ticks from an anchor on this
  // webview's clock. No player probe is scheduled while the Music tab is shut.
  let track = $state<Track | null>(null);
  let lyrics = $state<LyricLine[] | null>(null);
  // #194: keep both the lines and "the service had none" per track, so a
  // re-read or a reopened tab never asks lrclib for the same track twice.
  const lyricCache = new LyricCache();
  let positionUs = $state(0);
  let anchorMs = 0;
  let generation = 0;
  let tickTimer: ReturnType<typeof setInterval> | undefined;
  let refreshTimer: ReturnType<typeof setInterval> | undefined;
  let deck = $derived(deckState(track));
  let enabled = $derived(controls(track));

  // #195: `music_volume*` answers on the shell; the HUD only draws the reading
  // and toggles mute on a click. A 1 s read is the price of showing the meter
  // when the *system* (not this panel) changes the volume.
  const VOLUME_POLL_MS = 1000;
  let volume = $state<Volume>({ percent: 0, muted: false });
  let volumeCapable = $state(false);
  const volumeGate = new VolumeHudGate();
  let hudUntil = $state(0);
  let hudVisible = $state(false);

  let active = $derived(activeSession(sessions));
  let liveCards = $derived(
    cards.filter((card) => !hiddenIds.includes(card.id) && !gone.some((g) => g.card.id === card.id)),
  );
  let visibleCards = $derived([...liveCards, ...gone.map((g) => g.card)]);
  // #190: drop wizard state for requests no longer held. The effect depends on
  // the id list only, so pruning never re-triggers itself through `wizards`.
  $effect(() => {
    const ids = liveCards.map((card) => card.id).join(',');
    const current = untrack(() => wizards);
    const live = new Set(ids ? ids.split(',').map(Number) : []);
    const stale = Object.keys(current).filter((id) => !live.has(Number(id)));
    if (stale.length === 0) return;
    const next = { ...current };
    for (const id of stale) delete next[Number(id)];
    wizards = next;
  });
  // #201: the collapsed bell (and the amber `?`) marks a request still waiting
  // — hidden by the user (#186/#188) or followed up by an OS notification.
  // Reconciled with the feed, so an answered card drops out of the hint and
  // out of the tight-fit width.
  let hintedIds = $derived([...new Set([...hiddenIds, ...reminderIds])]);
  let hintedApprovals = $derived(
    cards.filter((card) => !isQuestion(card) && hintedIds.includes(card.id)).length,  );
  let hintedQuestions = $derived(
    cards.filter((card) => isQuestion(card) && hintedIds.includes(card.id)).length,
  );
  // #186 bell + #193 music wing both need room in the tight-fit pill.
  // Idle music is the canvas' 292 pt frame; a working bar keeps its wider size.
  let collapsed = $derived({
    width: Math.min(
      460,
      (track === null
        ? collapsedWidth(active)
        : Math.max(COLLAPSED_MUSIC_WIDTH, collapsedWidth(active) + 16)) +
        (hintedIds.length > 0 ? 20 : 0),
    ),
    height: COLLAPSED_SIZE.height,
  });  let completionSession = $derived(
    completion ? (sessions.find((s) => s.session_id === completion!.session_id) ?? null) : null,
  );
  let lastCollapsedWidth = COLLAPSED_SIZE.width;
  let pollTimer: ReturnType<typeof setTimeout> | undefined;
  const POLL_MS = 1000;

  // All three answers share the "answered elsewhere" path: the feed can drop
  // a card while IPC is in flight, so the notice keeps the words it had when
  // clicked. `send` is the one shell call that differs between them.
  async function settle(id: number, send: () => Promise<boolean>) {
    const card = cards.find((c) => c.id === id);
    try {
      if (await send()) {
        hiddenIds = hiddenIds.filter((hidden) => hidden !== id);
      } else {
        // Answered in the terminal (or its hook went) before this click.
        if (!card || gone.some((g) => g.card.id === id)) return;
        hiddenIds = hiddenIds.filter((hidden) => hidden !== id);
        gone = [...gone, { card }];
        setTimeout(() => (gone = gone.filter((g) => g.card.id !== id)), GONE_NOTICE_MS);
      }
    } catch (error) {
      hooksError = String(error);
    } finally {
      if (selected?.id === id) selected = null;
      releaseQuestionFocus(id);
    }
  }

  function answer(id: number, decision: Decision) {
    return settle(id, () => invoke<boolean>('answer_permission', { id, decision }));
  }

  // #188: one click (or number key) answers at once. The shell keys the reply
  // by question text; the label we send is the option the user picked.
  function answerQuestion(id: number, label: string) {
    if (selected?.id === id) return;
    selected = { id, label };
    return settle(id, () => invoke<boolean>('answer_question', { id, answer: label }));
  }

  // #188: macOS' Skip answers the hook without a pick (deny for a
  // PermissionRequest, bare output for a Notification question).
  function skipQuestion(id: number) {
    if (selected?.id === id) return;
    return settle(id, () => invoke<boolean>('skip_question', { id }));
  }

  // #190: the one place that binds a request id to its prompts and wizard.
  // Looking both up by id keeps a pick from ever reaching another request.
  function questionView(id: number) {
    const card = cards.find((c) => c.id === id);
    if (!card?.question) return null;
    const prompts = promptsOf(card.question);
    const wizard = wizardFor(wizards, id, prompts.length);
    const prompt = prompts[wizard.step];
    if (!prompt) return null;
    return { card, prompts, wizard, prompt };
  }

  // A single-choice card keeps #188's in-flight selected row; a multi-select
  // or wizard card shows the picks collected for its own request id.
  function questionPicks(id: number, view: NonNullable<ReturnType<typeof questionView>>) {
    if (!view.prompt.multi_select && view.prompts.length === 1) {
      const inFlight = selected;
      if (inFlight?.id !== id) return [];
      const index = view.prompt.options.findIndex((o) => o.label === inFlight.label);
      return index < 0 ? [] : [index];
    }
    return view.wizard.picks[view.wizard.step] ?? [];
  }

  function questionToggle(id: number, index: number) {
    const view = questionView(id);
    if (!view) return;
    wizards = putWizard(wizards, id, togglePick(view.wizard, index, view.prompt.multi_select));
  }

  function questionBack(id: number) {
    const view = questionView(id);
    if (!view) return;
    wizards = putWizard(wizards, id, goBack(view.wizard));
  }

  // Next on any but the last step; Confirm on the last sends one answer per
  // question. The shell builds #188's `updatedInput.answers` from these strings.
  function questionAdvance(id: number) {
    const view = questionView(id);
    if (!view || !canConfirm(view.wizard)) return;
    if (view.wizard.step < view.prompts.length - 1) {
      wizards = putWizard(wizards, id, goNext(view.wizard, view.prompts.length));
      return;
    }
    const answers = answersOf(view.wizard, view.prompts);
    return settle(id, () => invoke<boolean>('answer_questions', { id, answers }));
  }

  // #189: the panel is focusable(false), so a typed field must ask the shell
  // to make the island accept keys while it is open, and give them back when
  // the field, its card or the panel goes away. The gate keeps this to one
  // call each way even with several cards on screen.
  function focusQuestionField(id: number) {
    const change = questionFocus.focus(id);
    if (change) {
      invoke('island_question_focus', change).catch((error) => (hooksError = String(error)));
    }
  }

  function releaseQuestionFocus(id?: number) {
    if (id !== undefined && questionFocus.ownerId !== id) return;
    const change = questionFocus.release();
    if (change) {
      invoke('island_question_focus', change).catch((error) => (hooksError = String(error)));
    }
  }

  // #question: 1–9 pick the first visible question while the panel is open.
  // #190: for a multi-select or wizard card the key toggles the current row.
  function onKeyDown(event: KeyboardEvent) {
    if (!expanded || closing || event.ctrlKey || event.altKey || event.metaKey) return;
    const target = event.target as HTMLElement | null;
    if (target && /^(INPUT|TEXTAREA)$/.test(target.tagName)) return;
    const index = pickIndex(event.key);
    if (index === null) return;
    const card = liveCards.find(isQuestion);
    if (!card?.question) return;
    const view = questionView(card.id);
    if (!view || !view.prompt.options[index]) return;
    event.preventDefault();
    if (view.prompt.multi_select || view.prompts.length > 1) {
      questionToggle(card.id, index);
    } else {
      void answerQuestion(card.id, view.prompt.options[index].label);
    }
  }

  function hide(id: number) {
    if (!hiddenIds.includes(id)) hiddenIds = [...hiddenIds, id];
    releaseQuestionFocus(id);
  }

  // #187 — a click on the card's context row asks the shell to raise the
  // session's terminal window (or copy its path). The shell owns the outcome;
  // the card only draws it. No card is removed here.
  async function jumpToTerminal(session: Session) {
    try {
      const outcome = await invoke<TerminalOutcome>('terminal_activate', {
        app: session.term_app || null,
        tty: session.tty_path || null,
        cwd: session.cwd || null,
        pid: session.cli_pid,
      });
      jumpOutcomes = { ...jumpOutcomes, [session.session_id]: outcome };
    } catch (error) {
      hooksError = String(error);
    }
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
      // #188: a question card has no Approve / Deny buttons; E2E-1 measures
      // approvals, so leave question cards out of that report.
      const approve = el.querySelector('.approve');
      const deny = el.querySelector('.deny');
      if (!approve || !deny) continue;
      invoke('smoke_card', {
        id: Number(el.dataset.card),
        approve: rect(approve),
        deny: rect(deny),
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
    clearTimeout(completionTimer);
    completionTimer = undefined;
    if (fit.phase !== 'open') return;
    releaseQuestionFocus();
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
      // A held approval, or a completion card still waiting for its first
      // pointer visit, blocks collapse (NotchPanelView.swift:1485).
      const waitingForEntry =
        completion !== null && completionGate.blocksCollapse(completionKey(completion));
      if (collapseGate.shouldCollapse(performance.now(), visibleCards.length > 0 || waitingForEntry)) {
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
    // #188: macOS' collapsed question badge reopens a card the user hid;
    // approvals keep #186's bell-only reminder.
    const reopened = cards.find((card) => isQuestion(card) && hiddenIds.includes(card.id));
    if (reopened) hiddenIds = hiddenIds.filter((id) => id !== reopened.id);
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

  // #195: seed=true records the first reading without popping the HUD on
  // launch. Later readings that differ show it for VOLUME_HUD_MS.
  async function readVolume(seed = false) {
    try {
      const next = await invoke<Volume>('music_volume');
      volume = next;
      muted = next.muted;
      const now = performance.now();
      if (seed) volumeGate.observe(next, now);
      else if (volumeGate.observe(next, now)) hudUntil = now + VOLUME_HUD_MS;
    } catch (error) {
      hooksError = String(error);
    }
  }

  // #239 AC 2: the meter sets a level. The HUD draws the level optimistically —
  // a slow or failing read must not leave the meter where it was after a click —
  // and the shell's answer corrects it a moment later.
  async function onSetVolume(percent: number) {
    if (!volumeCapable) return;
    const plan = setVolumePlan(muted, percent);
    const now = performance.now();
    volumeGate.show(now);
    hudUntil = now + VOLUME_HUD_MS;
    volume = { percent: plan.percent, muted: plan.unmuteFirst ? false : muted };
    muted = volume.muted;
    try {
      if (plan.unmuteFirst) await invoke('music_volume_mute');
      await invoke('music_volume_set', { percent: plan.percent });
      await readVolume();
    } catch (error) {
      hooksError = String(error);
    }
  }

  async function onMute() {
    if (!volumeCapable) return;
    // Show immediately: the follow-up read may be slow, or fail on a device
    // that answered the mute command but cannot report its level.
    const now = performance.now();
    volumeGate.show(now);
    hudUntil = now + VOLUME_HUD_MS;
    try {
      await invoke('music_volume_mute');
      muted = !muted;
      await readVolume();
    } catch (error) {
      hooksError = String(error);
    }
  }

  // One timer per new dwell; a second change while the HUD is up restarts it.
  $effect(() => {
    const until = hudUntil;
    if (until <= performance.now()) {
      hudVisible = false;
      return;
    }
    hudVisible = true;
    const timer = setTimeout(() => (hudVisible = false), until - performance.now());
    return () => clearTimeout(timer);
  });

  // Only a machine with a System volume adapter asks for the level. It keeps
  // reading while the bar is collapsed: an OS-level change must raise the HUD.
  // A locked screen stops the reads too — no sound, no animation, no probe
  // (#197); the effect resumes the meter when the screen unlocks.
  $effect(() => {
    if (!volumeCapable || !probesActive(view)) return;
    void readVolume();
    const timer = setInterval(() => void readVolume(), VOLUME_POLL_MS);
    return () => clearInterval(timer);
  });

  // #193: one read asks the player, then the deck interpolates locally. The
  // generation guards against a slow lyrics answer landing on a newer track;
  // #194 keeps the answer in a per-track cache and only asks on a miss.
  async function readTrack() {
    const mine = ++generation;
    try {
      const next = await invoke<Track | null>('music_track');
      if (mine !== generation) return;
      track = next;
      anchorMs = performance.now();
      positionUs = next?.position_us ?? 0;
      const cached = next ? lyricCache.get(next) : undefined;
      // A cached null is an answer ("the service has none"), not a miss: the
      // placeholder stays up without another call on this track.
      lyrics = cached ?? null;
      if (next && cached === undefined) {
        const lines = await invoke<LyricLine[] | null>('music_lyrics', { track: next });
        if (mine !== generation) return;
        lyricCache.put(next, lines);
        lyrics = lines;
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
    const plan = refreshPlan(tab, expanded && probesActive(view));
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

  async function loadCapabilities() {
    try {
      // #199: the table itself lives in the Settings window; the island only
      // needs to know which ambient adapters it may ask.
      const rows = await invoke<CapabilityRow[]>('capabilities');
      support = {
        fullscreenDetection: isSupported(rows, 'Fullscreen detection'),
        screenLock: isSupported(rows, 'Screen lock'),
      };
      // #201: a machine without notify-send must not claim to remind.
      notificationsCapable = isSupported(rows, 'Notifications');
      // #186: shortcut hints are only honest when a global hotkey can fire.
      hotkeySupported = isSupported(rows, 'Global hotkey');
      // #195: no HUD (and no volume poll) on a machine without the adapter.
      volumeCapable = volumeSupported(rows);
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
    // #238 AC 9: the shell owns the reminder schedule, so it hands over the bells.
    reminderIds = snapshot.reminder_ids ?? [];
    // #189: a field whose card was answered or dropped elsewhere must not
    // leave the island holding keys meant for the app underneath.
    if (questionFocus.ownerId !== null && !cards.some((c) => c.id === questionFocus.ownerId)) {
      releaseQuestionFocus();
    }
    // A hidden request answered outside the panel drops off the feed;
    // forget it so the collapsed bell stops with it (#186).
    const pending = new Set(snapshot.permissions.map((card) => card.id));
    hiddenIds = hiddenIds.filter((id) => pending.has(id));
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
  // measured size so card rectangles and the tight-fit chain stay intact. A
  // locked screen is different: the window stays drawn, only content probes
  // (including transcript tails in the shell) stop until it unlocks (#197).
  $effect(() => {
    const hidden = view.hidden;
    invoke('island_visible', { visible: !hidden }).catch(() => {});
    invoke('set_probes_active', { active: probesActive(view) }).catch(() => {});
  });

  // #201: arm one wake-up for the earliest due follow-up. The pure scheduler
  // owns the timeout and the once-per-request rule; this effect only arms and
  // cancels the timer and calls the shell. It shares #197's ambient gate: a
  onMount(() => {
    const channel = new Channel<Snapshot>();
    channel.onmessage = acceptSnapshot;
    invoke('subscribe', { channel });
    invoke<unknown>('settings_load')
      .then((stored) => {
        filter = sessionFilterFromSettings(stored);
        density = sessionDensityFromSettings(stored);
        const appearance = appearanceFromSettings(stored);
        theme = appearance.theme;
        displayMode = appearance.displayMode;
      })
      .catch(() => {});
    // #202: Settings lives in another webview. The shell emits one event after
    // every settings write; adopting the theme here needs no restart and does
    // not touch the fit/reportCards chain.
    let unlisten: (() => void) | undefined;
    void listen<unknown>(SETTINGS_CHANGED_EVENT, (event) => {
      const appearance = appearanceFromSettings(event.payload);
      theme = appearance.theme;
      displayMode = appearance.displayMode;
    })
      .then((off) => (unlisten = off))
      .catch(() => {});
    loadCapabilities();
    // One seed read so the collapsed wing can show a track; the tab effect
    // owns every read after this, so a closed tab never polls.
    void readTrack();
    // #195: the first volume reading is a baseline, not a change to announce.
    void readVolume(true);
    lastCollapsedWidth = collapsed.width;
    reportFit(collapsed, false);

    return () => {
      unlisten?.();
      releaseQuestionFocus();
      clearTimeout(hoverTimer);
      clearTimeout(collapseTimer);
      clearTimeout(openTimer);
      clearTimeout(closeTimer);
      clearTimeout(quitTimer);
      clearTimeout(completionTimer);
      if (pollTimer !== undefined) clearTimeout(pollTimer);
      if (tickTimer !== undefined) clearInterval(tickTimer);
      if (refreshTimer !== undefined) clearInterval(refreshTimer);
      pollTimer = undefined;
      tickTimer = undefined;
      refreshTimer = undefined;
    };
  });

  // #183/#184: a card's words can change while its count stays the same, so
  // the fit trigger reads every rendered field — including the branch, task
  // progress, subagent count and recap — too. It still goes through this one
  // path; IslandFit drops a size that has not changed.
  let sessionContent = $derived(
    sessions
      .map(
        (s) =>
          `${s.session_id}:${s.status}:${s.current_tool}:${s.tool_description}:${s.last_user_prompt}:${s.last_assistant_message}:${s.live_reply}:${s.updated_at}:${s.git_branch}:${s.git_is_worktree}:${s.tasks_done}:${s.tasks_total}:${s.task_active}:${s.subagent_count}:${s.recap}`,
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

  // #186: a card's title and body can change while its count stays the same.
  let cardContent = $derived(
    visibleCards
      .map((card) => {
        const question = card.question
          ? `${card.question.question}:${card.question.header}:${card.question.options
              .map((option) => `${option.label}/${option.description}`)
              .join(',')}`
          : '';
        return `${card.id}:${card.tool_name}:${JSON.stringify(card.tool_input)}:${question}:${JSON.stringify(jumpOutcomes[card.session_id] ?? null)}`;
      })
      .join('|'),
  );

  // Content changes while the panel is open need one fit; the controller
  // ignores the frames produced by the width/height animation itself.
  $effect(() => {
    const content = `${sessionContent}|${cardContent}|${cards.length}|${gone.length}|${tab}|${filter}|${density}|${track?.title ?? ''}|${lyrics?.length ?? 0}|${completion ? completionKey(completion) : ''}`;
    if (!expanded) return;
    void content;
    void tick().then(() => {
      if (fit.phase !== 'open') return;
      const size = measure();
      // The deck (and any other tall tab) can be taller than the tab the
      // panel opened on: keep the DOM box in step with the reported OS size.
      openHeight = size.height;
      for (const fitSize of fit.layout(size)) reportFit(fitSize, true);
    });
  });
</script>

<svelte:window onkeydown={onKeyDown} />

<div
  class="island"
  class:open={expanded}
  class:closing
  class:dragging
  class:locked={view.suspended}
  data-variant={variant}
  data-phase={fit.phase}
  data-theme={theme}
  {style}
  style:width={expanded ? undefined : `${collapsed.width}px`}
  style:height={openHeight === null ? undefined : `${openHeight}px`}
  bind:this={island}
  onpointerenter={onPointerEnter}
  onpointerleave={onPointerLeave}
  role="presentation"
>
  {#if !expanded && !closing}
    {#if hudVisible && volumeCapable}
      <!-- #195/#239: for ~1.5 s after a change the meter takes the bar's place;
           it reads, and now also writes, through the shell's existing commands. -->
      <VolumeHud
        {volume}
        onToggle={onMute}
        onSet={onSetVolume}
        onpointerdown={onBarPointerDown}
        onpointermove={onBarPointerMove}
        onpointerup={onBarPointerUp}
        onpointercancel={onBarPointerCancel}
      />
    {:else}
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
          {theme}
          sessionCount={sessions.length}
          {prehover}
          quiet={view.quiet}
          {active}
          {hintedApprovals}
          {hintedQuestions}
          {track}
          glance={completion !== null && completion.presentation !== 'status_only'}
          glanceLabel={completionSession?.cwd_name ?? ''}
        />
        <span class="grip" aria-hidden="true"></span>
      </button>
    {/if}
  {/if}
  <div class="panel-content" class:visible={expanded || closing} bind:this={panelContent}>
    <Header
      {theme}
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
            {theme}
            {sessions}
            {filter}
            {density}
            approvals={liveCards}
            onAnswer={answer}
            onDelete={deleteSession}
            onOpenSettings={openSettings}
          />
        {/if}
      </div>
    {:else}
      <MusicDeck
        {theme}
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
    {#if visibleCards.length > 0}
      <div class="padded cards">
        {#each visibleCards as card, i (card.id)}
          {#if isQuestion(card)}
            {@const view = questionView(card.id)}
            {#if view}
              <QuestionCard
                {card}
                session={sessions.find((s) => s.session_id === card.session_id) ?? null}
                position={i + 1}
                total={visibleCards.length}
                {hotkeySupported}
                prompt={view.prompt}
                chip={progressChip(view.wizard.step, view.prompts.length)}
                picks={questionPicks(card.id, view)}
                canBack={view.wizard.step > 0}
                canAdvance={canConfirm(view.wizard)}
                isLast={view.wizard.step === view.prompts.length - 1}
                gone={gone.some((g) => g.card.id === card.id)}
                onPick={answerQuestion}
                onToggle={questionToggle}
                onBack={questionBack}
                onAdvance={questionAdvance}
                onSkip={skipQuestion}
                onHide={hide}
                onFieldFocus={focusQuestionField}
              />
            {/if}
          {:else}
            <PermissionCard
              {card}
              session={sessions.find((s) => s.session_id === card.session_id) ?? null}
              position={i + 1}
              total={visibleCards.length}
              {hotkeySupported}
              gone={gone.some((g) => g.card.id === card.id)}
              outcome={jumpOutcomes[card.session_id] ?? null}
              onAnswer={answer}
              onHide={hide}
              onJump={jumpToTerminal}
            />
          {/if}
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
  /* #195: the 30 pt HUD sits centred in the 34 pt collapsed pill. */
  .island > :global([data-volume-hud]) {
    margin: 2px auto;
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
