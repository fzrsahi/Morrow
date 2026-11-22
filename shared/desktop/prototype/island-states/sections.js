// PROTOTYPE — every state on the canvas. Each section: anchor id (tickets
// link to it), the SwiftUI source it re-draws, the ticket that builds it,
// reference renders from UIGalleryHarness, and notes for the implementer.
(function () {
  const S = [
    { src: 'mimo', st: 'needs', cwd: 'ml-pipeline', br: 'exp/feature-store', t: '7M', tool: 'BASH', detail: 'python scripts/backfill.py --month 2026-03', prompt: 'Backfill the feature store for March', approval: 'BASH' },
    { src: 'claude', st: 'working', cwd: 'web-app', br: 'feat/dashboard', sub: 2, t: '12M', tasks: [2, 5, 'WIRE FILTERS INTO THE ORDERS QUERY'], tool: 'EDIT', detail: 'src/components/Dashboard.tsx', prompt: 'Add filters to the dashboard page', reply: 'Adding a date-range and status filter bar above the orders table.' },
    { src: 'codex', st: 'working', cwd: 'api-server', br: 'perf/query-planner', t: '26M', tool: 'BASH', detail: 'cargo test -p planner', prompt: 'Speed up the slow /search endpoint', reply: 'Added cost-based planner; running cargo test…' },
    { src: 'gemini', st: 'done', cwd: 'docs-site', br: 'main', t: '1H', prompt: 'Document the new filter API', reply: 'Updated 6 pages under docs/api and fixed 3 broken links.', recap: 'Documented the filter API (6 pages) and fixed broken links. Next: review the new examples.' },
  ];
  const MORE = [
    { src: 'copilot', st: 'needs', cwd: 'mobile-app', br: 'fix/feed-scroll', t: '4M', prompt: 'Fix the scroll jank on the feed' },
    { src: 'cursor', st: 'thinking', cwd: 'infra', br: 'chore/terraform-1.9', t: '2M' },
    { src: 'opencode', st: 'stopped', cwd: 'payments', br: 'feat/refunds', t: '41M', prompt: 'Add partial refunds to the checkout flow' },
    { src: 'qoder', st: 'idle', cwd: 'customer-onboarding-service-v2-migration', br: 'feature/emails', t: '3H', prompt: 'Migrate the onboarding emails to the new template engine' },
    { src: 'kiro', st: 'error', cwd: 'ml-infra', br: 'main', t: '5M', prompt: 'Retry the GPU node pool upgrade' },
  ];
  const ALL = S.concat(MORE);
  const exp = (inner, o = {}) => `<div class="isl exp"${o.w ? ` style="width:${o.w}px"` : ''}>${inner}<div class="winbox"><i>tight-fit window · ${o.w || 680} × measured</i></div></div>`;
  const slot = (inner, cap) => `<div class="cell"><div class="mini">${window.osbar()}<div class="slot">${inner}</div></div>${cap ? `<div class="cap">${cap}</div>` : ''}</div>`;
  const cells = (list) => list.map(([html, cap]) => slot(html, cap)).join('');
  const note = (kind, iconName, text) => `<div class="note ${kind}">${ic(iconName, null, 12)}<span>${text}</span></div>`;

  window.SECTIONS = [
    { group: 'Tokens', sub: 'Pixel16Palette, PixelText, PixelIcon, type scale, motion' },
    { id: 'tokens', title: 'Design tokens', sub: 'copy these 1:1 into the Svelte port', meta: 'Pixel16DesignSystem.swift · PixelText.swift · PixelIcon.swift', t: 'T01', nostage: true,
      body: () => window.tokensHTML(),
      notes: ['<code>pixel-font.json</code> and <code>pixel-icons.json</code> next to this file are extracted byte-for-byte from the Swift bitmaps. Render them as SVG rects with <code>shape-rendering: crispEdges</code>, or on a canvas.',
        'Pixel sizes are fractional in Swift (1.05, 1.15, 1.35) because macOS renders at 2×. Windows at 100 % and 125 % scaling blurs fractional pixels: snap <code>pixelSize × devicePixelRatio</code> to a whole device pixel.',
        'Body text: <code>system-ui</code> (Segoe UI Variable / Cantarell / Noto Sans). Monospace: <code>ui-monospace</code>, then Cascadia Mono, then DejaVu Sans Mono. Do not ship SF fonts.'] },

    { group: '1 · Collapsed bar', sub: 'CompactLeftWing · CompactRightWing · IdleIndicatorBar' },
    { id: 'collapsed-quiet', title: 'Idle, empty, prehover', sub: 'no work in flight', meta: '≥240 × 34 pt · IdleIndicatorBar', t: 'T01',
      ref: ['collapsed-external-empty', 'collapsed-external-idle'],
      body: () => cells([[collapsed('empty'), 'empty · no session ever seen'], [collapsed('idle'), 'idle · sessions exist, none working'], [collapsed('prehover'), 'prehover · +7 pt width, scale 1.004']]),
      notes: ['Height 34 pt on Windows and Linux: Dynamic Island minimum <code>max(notchHeight, 34)</code>, because there is no menu bar to match (NotchPanelView.swift:238-249).',
        'Mascot 21 pt: <code>min(21, max(18, h − 13))</code>. The idle mascot sleeps (zz); the docs GIF is a stand-in until T02 exports real per-status sprites.',
        'Prehover: pointer enters, width +7 and scale 1.004 anchored top, easeOut 0.21 s. Expand fires after a 0.5 s dwell (setting 0.1–1.0 s).'] },
    { id: 'collapsed-working', title: 'Working', sub: 'thinking, tool label, workspace dock', meta: '≤460 pt · CompactLeftWing', t: 'T04',
      ref: ['collapsed-external-working-short', 'collapsed-external-working-long', 'collapsed-14in-working-short'],
      body: () => cells([[collapsed('thinking'), 'thinking · dots + THINKING'], [collapsed('tool'), 'tool · label coloured by tool']]),
      notes: ['Thinking dots: three 4 pt squares stepping every 0.35 s (Pixel16ThinkingDots).',
        'Tool colours: bash <code>#66FF80</code>, edit/write <code>#80B3FF</code>, read <code>#E6CC66</code>, grep/glob/MCP <code>#CC99FF</code>, agent <code>#FF9966</code>, other white 0.7 (NotchPanelView.swift:2790-2800).',
        'Dock pills: fill white 0.06, 0.5 pt stroke white 0.10, hover cyan 0.25. Project label max 44 pt, middle-truncated. Agents dock shows a green dot and <code>active/total</code>.',
        'Bar width grows with content up to <code>min(0.62 × screenW, 460)</code>. Every width change is a window resize (tight-fit), so batch label changes; do not animate width per frame.'] },
    { id: 'collapsed-attention', title: 'Needs you', sub: 'approval, question, completion glance', meta: 'statusBadges · NotchPanelView.swift:2745', t: 'T07 · T09 · T12',
      ref: ['collapsed-external-approval', 'collapsed-external-question', 'collapsed-external-question-dismissed', 'collapsed-external-glance'],
      body: () => cells([[collapsed('approval'), 'approval waiting · pulsing bell'], [collapsed('question'), 'question waiting · pulsing'], [collapsed('question-dismissed'), 'question hidden by the user · static'], [collapsed('glance'), 'completion glance · cyan check']]),
      notes: ['Normally an approval or question opens its card by itself (auto-expand). The collapsed badge shows when the user pressed Hide, or when smart suppress kept the card closed because that session\'s terminal is in front.',
        'Question bubble amber <code>(1.0, 0.7, 0.28)</code> 9 pt, pulsing. Completion check cyan 8 pt with a 2 pt glow.'] },
    { id: 'collapsed-ambient', title: 'Ambient', sub: 'music wing, quiet hours, hidden', meta: 'CyberMusicWingView · SceneMuteMonitor', t: 'T14 · T18',
      body: () => cells([[collapsed('music'), 'music playing · note + title + spectrum'], [collapsed('quiet'), 'quiet hours · moon'], [collapsed('hidden'), 'hidden · fullscreen app or screen locked']]),
      notes: ['The compact music wing was not traced in detail (CyberMusicView.swift:439-695). Read that file before building it; this frame only fixes its place in the bar.',
        '<span class="badge-port">PORT</span> Hidden: on Windows, hide while <code>fullscreen()</code> is true. On Linux <code>fullscreen()</code> always returns false, so the Capability table must say "Fullscreen detection: not supported" and the island stays.',
        'Screen locked: no sound and no probes (<code>screen_locked</code>). The island may stay drawn; nothing should animate.'] },

    { group: '2 · Expanded · Agents', sub: 'header, session list' },
    { id: 'motion', title: 'Open and close', sub: 'click the island · watch the dashed window', meta: 'NotchAnimation.swift · plan §6', t: 'T03',
      body: () => `<div class="cell"><div class="mini" style="min-width:760px;min-height:220px">${window.osbar()}<div class="slot" id="demo-slot"><div class="isl col mo" id="demo-isl" style="width:240px;height:34px"><div class="inner" id="demo-inner"></div><div class="winbox" id="demo-win"><i id="demo-win-l">window 240 × 34</i></div></div></div></div><div class="demo-btns"><button id="demo-toggle">Open / close</button></div></div>`,
      notes: ['Open: spring response 0.42 s, damping 0.82. Close: response 0.38 s, damping 1.0, no overshoot. Speed setting divides both (0.5–2.0).',
        '<b>Window rule</b> (plan §6, row 1): on open, resize the OS window to the <i>final</i> size before the animation starts; on close, shrink it only after the animation ends. The current <code>ResizeObserver → set_island_size</code> resizes on every layout change, which will stutter once the panel animates.',
        'Content fades in with blur 5 → 0 and scale 0.96 anchored top (blurFade). Collapse on pointer leave after 0.5 s, never while an approval or question card is open.',
        '<span class="badge-port">PORT</span> Windows hit-test: <code>fit()</code> sends one radius <code>h / 2</code> for all four corners. For a 680 × 400 panel that clamps to 200 pt, and clicks in the top corners fall through. The frontend must send the real shape (square top, 24 pt bottom) with the size.'] },
    { id: 'header', title: 'Header bar', sub: 'tabs, filter, mute, settings, quit', meta: 'HeaderNavigationMenuTabs · SessionGroupingTabs', t: 'T03',
      ref: ['list-1'],
      body: () => cells([[exp(header({})), 'Agents tab · ALL'], [exp(header({ filter: 'sta', muted: true, quit: true })), 'STA filter · muted · power clicked once → QUIT?']]),
      notes: ['Tabs: Agents and Music only. Work, Prayer, Mail and Game have no user story in #82; leave them out until they are scoped.',
        'Tab: height 20, padding 8 (active) or 6, radius 5, active fill cyan 0.18 (pixel) or white 0.18 (modern). Inactive icons white 0.55.',
        'Icon buttons: 20 pt box, 24 pt hit area, radius 3, tint 0.08, hover 0.22 and scale 1.08. Power: first click turns into a red <code>QUIT?</code>; a second click within 3 s quits.'] },
    { id: 'list-comfortable', title: 'Session list', sub: 'every card element in one list', meta: '680 pt · PixelRetroSessionCardView', t: 'T04 · T05',
      ref: ['list-4', 'list-8-details'],
      body: () => slot(exp(header({}) + `<div class="list">${S.map((s, i) => card(s, { hover: i === 2 })).join('')}</div><div class="quota"><span>Claude · last 5h: 422K in · 96.4K out</span><span>Today: 922K in · 233K out</span><span class="hsp"></span><span class="bar"><i></i></span></div>`)),
      notes: ['Card: padding 8 × 10, radius 6, fill white 0.03 (hover 0.06, shows the delete ✕). Mascot 28 pt in a 34 pt slot with three LED bars.',
        'Header row: status badge (filled only for NEEDS YOU), source badge, folder + cwd, <code>//</code>, branch in the status colour, <code>+N SUB</code>, time (<code>NOW</code>, <code>7M</code>, <code>2H</code>, <code>3D</code>).',
        'T04 needs these from <code>SessionSnapshot</code> in the DTO: status, current tool + description, last user prompt, last assistant message, model, cwd. T05 adds git branch (<code>morrow_core::git::read</code>), agent tasks, subagents and recap.',
        'Order: NEEDS YOU first, then working/thinking, then the rest, newest first inside a tier (SessionListOrdering.swift:109-115). 90 pt per visible card; scroll past the setting.',
        'Quota footer needs Claude credentials (Capability "Claude credentials"); hide the line when it is not supported.'] },
    { id: 'list-status', title: 'Every status', sub: 'NEEDS YOU · WORKING · THINKING · DONE · IDLE · STOPPED · ERROR', meta: 'SessionCardStatus', t: 'T04',
      body: () => slot(exp(`<div class="list" style="padding-top:8px">${ALL.slice(4).concat([S[3]]).map((s) => card(s)).join('')}</div>`)),
      notes: ['Colours: needs you amber, working/thinking cyan, done white 0.85, idle textMuted, stopped/error red (pixel). Modern: stopped <code>#FF8C66</code>, error <code>#FF6170</code>.'] },
    { id: 'list-compact', title: 'Compact density and grouping', sub: '34 pt rows · group by status', meta: 'compact16BitView · SessionGroupingTabs', t: 'T06',
      ref: ['list-8-compact', 'list-8-status', 'list-8-cli'],
      body: () => cells([[exp(header({}) + `<div class="list" style="gap:3px">${ALL.map(compactRow).join('')}</div>`), 'compact · ALL'],
        [exp(header({ filter: 'sta' }) + ['needs', 'working', 'done'].map((k) => `<div class="ghead">${px(ST[k].l, 1.05, ST[k].c)}${px(String(ALL.filter((s) => s.st === k || (k === 'working' && s.st === 'thinking')).length), 1.05, PCOL.dim)}</div><div class="list" style="gap:3px">${ALL.filter((s) => s.st === k || (k === 'working' && s.st === 'thinking')).map(compactRow).join('')}</div>`).join('')), 'compact · grouped by status (STA)']]),
      notes: ['Compact row: height 34, gap 3, mascot 15, padding 7 × 3.5, radius 4. AGT groups by source with the same header.',
        'Group header text was not traced in Swift; this frame uses <code>LABEL · count</code>. Check NotchPanelView.swift around SessionGroupingTabs before shipping.'] },
    { id: 'list-empty', title: 'Empty and overflow', sub: 'no sessions · more than fit', meta: 'SessionListMetrics', t: 'T06', port: true,
      body: () => cells([[exp(header({}) + `<div class="empty">${mascot('claude', 32, { sleep: true })}${px('NO SESSIONS YET', 1.3, PCOL.muted)}<p>Start an agent in a terminal. Agents without our hook do not show up: open Settings → Hooks.</p><button class="btn s">Open Settings</button></div>`), 'empty list'],
        [exp(header({}) + `<div class="list">${S.slice(0, 2).map((s) => card(s)).join('')}</div><div class="more">6 more sessions ${ic('chev', null, 10)}</div>`), 'overflow']]),
      notes: ['<span class="badge-port">PORT</span> macOS has no designed empty list. On Windows and Linux the first run is the common case, so the empty state points to the hooks page.'] },

    { group: '3 · Approval card', sub: 'ApprovalBar · NotchCardButton' },
    { id: 'approval', title: 'Approval by tool', sub: 'command, file change, MCP call', meta: '680 pt · NotchPanelView.swift:4701', t: 'T07',
      ref: ['approval-bash-short', 'approval-edit', 'approval-mcp', 'approval-write', 'approval-codex'],
      body: () => cells([[exp(header({}) + approval('bash')), 'Bash · command'], [exp(header({}) + approval('edit')), 'Edit · diff'], [exp(header({}) + approval('mcp')), 'MCP tool · key/value input']]),
      notes: ['Titles come from <code>approval_title_*</code> in L10n.swift:617-626: command, change_file, change, read_file, read, search, web, mcp, agent, generic. Port that table, do not invent copy.',
        'Code block: fill white 0.06, radius 6, padding 10 × 8, min height 3 lines. Diff: added <code>#4DD966</code>, removed <code>#FF6666</code>.',
        'Buttons, min height 26, radius 6: Hide (quiet, padding 8), Deny (1 pt red 0.7 stroke, padding 11), Allow once (<code>#29803D</code>, hover <code>#2B8742</code>, padding 14). Label size <code>min(base + 1, 15)</code>.',
        '"Always allow X this session" needs a third decision in Rust (<code>Decision</code> is allow/deny only today). Keep the link out of T07 unless that decision lands with it.',
        'Today the shell holds only <code>PermissionRequest</code> and the card shows the raw tool name. T07 must map tool name + input to these titles and blocks.'] },
    { id: 'approval-queue', title: 'Queue, keys, inline', sub: '1 of 3 · shortcut hints · card inside the list', meta: 'NotchCardQueueLabel · approval16BitBlock', t: 'T07',
      ref: ['approval-queued', 'approval-bash-short-keys'],
      body: () => cells([[exp(header({}) + approval('bash', { queue: '1 of 3' })), 'queued · only when more than one waits'], [exp(header({}) + approval('bash', { keys: true })), 'shortcut hints'], [exp(`<div class="list" style="padding-top:8px">${card(S[0])}</div>`), 'inline approval in the session list']]),
      notes: ['Shortcut hints use the platform names: <code>Ctrl+Alt+…</code> on Windows and Linux where macOS shows ⌥⌘. The global hotkey Capability decides whether hints show at all.'] },
    { id: 'approval-gone', title: 'Answered elsewhere', sub: 'the hook went away before the click', meta: 'answer_permission → false', t: 'T07', port: true,
      body: () => slot(exp(header({}) + approval('bash', { dim: true, notice: note('info', 'info', 'Answered in the terminal. This request is no longer waiting.') }))),
      notes: ['<span class="badge-port">PORT</span> <code>answer_permission</code> returns false when the request is gone. Show this for 1.5 s, then drop the card. macOS drains silently; on Windows and Linux the click needs a visible answer.'] },

    { group: '4 · Question card', sub: 'QuestionBar · OptionRow · MultiSelectRow' },
    { id: 'question', title: 'Single choice', sub: 'idle, hover, Other with text', meta: 'NotchPanelView.swift:4992', t: 'T09 · T10',
      ref: ['question-single', 'question-many', 'question-legacy'],
      body: () => cells([[exp(header({}) + question('single', { hover: 0 })), 'single choice · hover on 1'], [exp(header({}) + question('other')), 'Other picked · field focused (T10)']]),
      notes: ['Options: padding 10 × 7, radius 4, gap 4, min area 72 pt. Fill idle white 0.03, hover white 0.08 + blue 0.2 stroke, selected blue 0.08 + blue 0.4 stroke. Index in blue 0.8.',
        'Clicking an option answers at once (single question). Number keys 1–9 pick when the panel has focus.',
        'Rust: the shell must hold <code>AskUserQuestion</code> (a PermissionRequest) and Notification questions, then reply <code>allow</code> with <code>updatedInput.answers</code> keyed by question text (AppState.handleAskUserQuestion). None of this exists yet.',
        '<b>Typing</b> (T10): the window is <code>focusable(false)</code>. Windows: call <code>island_question_focus(true)</code> when the field is clicked and false on close. Linux: GTK accept-focus is false too and the command is a no-op there today, so typing likely fails on X11 as well. Verify and fix in T10.'] },
    { id: 'question-multi', title: 'Multi-select, free text, wizard', sub: 'Confirm disabled until a pick · 2/3 with Back', meta: 'QuestionWizardState', t: 'T10 · T11',
      ref: ['question-multi', 'question-free-text', 'question-multi-keys'],
      body: () => cells([[exp(header({}) + question('multi', { picked: [0, 2] })), 'multi-select · 2 picked'], [exp(header({}) + question('multi')), 'multi-select · nothing picked'], [exp(header({}) + question('free')), 'free text · empty'], [exp(header({}) + question('free', { focus: true })), 'free text · typing'], [exp(header({}) + question('wizard', { sel: 0 })), 'wizard · question 2 of 3']]),
      notes: ['Progress is the <code>n/total</code> chip only; there are no step dots.',
        'Field: padding 10 × 6, fill white 0.05, 1 pt stroke white 0.10, green <code>&gt;</code> prompt. Enter submits. <span class="badge-port">PORT</span> Focus ring blue 0.55 so a keyboard user sees where typing goes.'] },

    { group: '5 · Completion card', sub: 'CompletionReplyView · MarkdownReplyView' },
    { id: 'completion', title: 'Reply after Stop', sub: 'short and markdown', meta: 'MarkdownReplyView.swift', t: 'T12',
      ref: ['completion-short', 'completion-markdown'],
      body: () => cells([[exp(header({}) + completion('short')), 'short reply'], [exp(header({}) + completion('markdown')), 'markdown reply']]),
      notes: ['Markdown arrives as blocks from <code>morrow_core::markdown::parse</code>; the frontend only renders. Body mono at content size, white 0.85; headings +3/+2/+1; inline code <code>#F5BD8A</code> on white 0.08; links <code>#73B8FF</code>.',
        'Code block: padding 7, fill white 0.06, 0.5 pt hairline, 14 visible lines then scroll, copy button 20 × 18. Tables: 12 visible rows.',
        'Auto-open follows AttentionPolicy (same as macOS). The card does not collapse on pointer leave until the pointer has entered it once.'] },

    { group: '6 · Jump to terminal', sub: 'NotchCardContextRow · terminal_activate' },
    { id: 'jump', title: 'Context row and outcomes', sub: 'hover · focused · copied · not possible', meta: 'Outcome::{Focused, Copied, NotPossible}', t: 'T08', port: true,
      body: () => cells([[exp(approval('bash', { ctxHover: true })), 'hover · jump icon brightens'],
        [exp(approval('bash', { notice: note('ok', 'check', 'Copied <code>~/code/web-app</code>. No terminal window was found for this session.') })), 'Copied(path)'],
        [exp(approval('bash', { shake: true, notice: note('warn', 'warn', 'This desktop does not let apps raise other windows (GNOME Wayland).') })), 'NotPossible(reason) · card shakes']]),
      notes: ['Whole context row is the target: padding 6 × 3, hover fill white 0.09, radius 5. Tooltip "Click to focus this session\'s terminal". The card stays queued after a jump.',
        'Needs <code>term_app</code>, <code>tty_path</code>, full <code>cwd</code> and <code>cli_pid</code> in the Session DTO. Hide the jump icon for remote sessions.',
        '<span class="badge-port">PORT</span> Windows: <code>terminal_activate</code> always returns NotPossible today. W-007 needs a real path: walk up from <code>cli_pid</code> to the window owner, then <code>SetForegroundWindow</code>. Adding the button alone does not pass the gate.'] },

    { group: '7 · Music', sub: 'RetroCyberMusicHUDContentView · NotchVolumeHUDView' },
    { id: 'music', title: 'Audio deck', sub: 'playing with synced lyrics · paused · nothing playing', meta: '680 × 114 pt', t: 'T14 · T15',
      body: () => cells([[exp(header({ tab: 'music' }) + music('playing')), 'playing · lyrics synced'], [exp(header({ tab: 'music' }) + music('nolyrics')), 'paused · lrclib has nothing'], [exp(header({ tab: 'music' }) + music('none')), 'nothing playing']]),
      notes: ['Left column 255 pt: AUDIO DECK label, player badge, 44 pt artwork, title marquee, segmented scrubber (4.5 pt cells, gap 2), times, transport 25/29/25 × 18.',
        'Lyrics column: list height 84, current line cyan with a 5 pt glow, the rest white 0.42, scrolled to centre.',
        'No polling while the tab is closed (music.rs). Tick the position locally with <code>music_position</code>; ask the player again only on a control or every few seconds.',
        '<code>Track</code> has no artwork URL, and the CSP <code>default-src \'self\'</code> blocks remote images. Artwork needs both a field and an <code>img-src</code> entry, or stays a gradient.'] },
    { id: 'volume', title: 'Volume HUD', sub: '12-segment meter · mute', meta: 'NotchVolumeHUDView.swift:73-120', t: 'T16',
      body: () => cells([[`<div class="isl col" style="justify-content:center">${volumeHud(62, false)}</div>`, '62 %'], [`<div class="isl col" style="justify-content:center">${volumeHud(100, false)}</div>`, '100 %'], [`<div class="isl col" style="justify-content:center">${volumeHud(40, true)}</div>`, 'muted · click toggles']]),
      notes: ['Segments 0–7 cyan, 8–9 amber, 10–11 red, unlit white 0.12; each 5 × 9.5, gap 2. Readout mono bold 9, <code>MUTE</code> in red.',
        'Show it in place of the collapsed bar for about 1.5 s after a change, or inside the music deck.'] },

    { group: '8 · Window and desktop', sub: 'port-only: no SwiftUI equivalent' },
    { id: 'anchoring', title: 'Anchoring not available', sub: 'GNOME Wayland · unknown Wayland · AppImage on Wayland', meta: 'compositor::anchoring (#109)', t: 'T21', port: true,
      body: () => slot(`<div class="isl exp peek"><span class="ic">${ic('warn', null, 16)}</span><div><div class="t1">The island is a normal window here</div><div class="t2">GNOME does not let apps pin a panel above other windows. Fullscreen apps will cover it.</div></div></div>`, 'mini-peek · once per launch, 5 s'),
      notes: ['Copy comes from <code>Anchoring::Unavailable(reason)</code>; show the reason as given, one sentence. The same sentence sits in Settings → Capabilities under "Panel anchoring".',
        'Mini-peek size follows the macOS banner (440 × 100 pt) and closes after 5 s unless the pointer is over it.'] },
    { id: 'drag', title: 'Drag and position', sub: 'L-005', meta: 'fit() re-centres today', t: 'T17', port: true,
      body: () => cells([[`<div class="isl col" style="position:relative">${collapsed('idle').replace(/^<div class="isl col">|<\/div>$/g, '')}<span class="grip"></span></div>`, 'hover · grip under the bar'],
        [`<div style="position:relative;width:420px;height:70px"><span class="ghost-slot"></span><div class="isl col dragging" style="position:absolute;left:150px;top:22px;border-radius:999px">${collapsed('idle').replace(/^<div class="isl col">|<\/div>$/g, '')}</div></div>`, 'dragging · dashed slot is home'],
        [`<div class="menu"><div class="hl">Show island</div><div>Settings…</div><div>Reset island position</div><hr><div>Quit Morrow <small>Ctrl+Q</small></div></div>`, 'tray menu']]),
      notes: ['Drag starts after 4 px of movement on the collapsed bar, so a click still expands. Snap back to top-centre within 24 px of home.',
        'Persist the offset per monitor in settings; <code>fit()</code> must keep it instead of re-centring. Tray gets "Reset island position".',
        'A dragged island is a free-floating pill (variant B shape) wherever it lands.'] },

    { group: '9 · Settings window', sub: 'port: a separate window, built lazily' },
    { id: 'settings', title: 'Settings', sub: 'Hooks · Capabilities · Appearance', meta: '760 × 470 · SettingsView.swift', t: 'T20 · T21 · T23', port: true,
      ref: ['settings-general', 'settings-appearance', 'settings-hooks', 'settings-behavior', 'settings-about'],
      body: () => ['hooks', 'capabilities', 'appearance'].map((pg) => `<div class="cell"><div class="mini" style="padding:0 40px 40px">${window.osbar()}${window.settingsHTML(pg)}</div><div class="cap">${pg}</div></div>`).join(''),
      notes: ['Today hooks, autostart and the Capability table sit inside the expanded island. Move them to this window, opened from the gear button and the tray, created on first open (plan §10: "window Settings dibuat lazy").',
        'Pages for the port: General, Appearance, Mascots, Sound, Shortcuts · Agents, Music · Behavior, Hooks, Capabilities, About. macOS-only pages (Mail, Prayer HUD, Remote, Buddy) are left out.',
        'This is a normal, focusable, decorated window. Native title bar on Windows and KDE; on GNOME use the header-bar look Tauri gives with <code>decorations(true)</code>.'] },
  ];

  // ---------- tokens ----------
  window.tokensHTML = function () {
    const sw = (n, hex, src) => `<div class="swatch"><i style="background:${hex}"></i><div><b>${n}</b>${hex}<br>${src}</div></div>`;
    const glyphs = Object.keys(window.PIXEL_FONT).join('');
    return `<div class="tok">
      <div class="sw-row">${[['chassis', '#0D121A'], ['surface', '#141C26'], ['surfaceRaised', '#1F2938'], ['slot', '#080A0F'], ['cyan', '#00F5D4'], ['amber', '#FFB833'], ['red', '#F25959'], ['textWhite', '#F2F7FF'], ['textMuted', '#99ADC7'], ['textDim', '#617085']].map(([n, h]) => sw(n, h, 'Pixel16Palette')).join('')}
        ${[['allow', '#29803D'], ['deny', '#EB6161'], ['link', '#73B8FF'], ['permission tag', '#FF9933'], ['question blue', '#66B3FF'], ['diff add', '#4DD966']].map(([n, h]) => sw(n, h, 'NotchPanelView')).join('')}</div>
      <div class="specimen">${px(glyphs.slice(0, 36), 1.6, PCOL.cyan)}${px(glyphs.slice(36), 1.6, PCOL.white)}${px('THINKING  NEEDS YOU  [2/5 TASKS]  ▸ BASH', 1.05, PCOL.amber)}</div>
      <div class="specimen icongrid">${Object.keys(window.PIXEL_ICONS).map((k) => `<figure>${pi(k, 2.4, PCOL.white)}<figcaption>${k}</figcaption></figure>`).join('')}</div>
      <table class="t"><tr><th>Role</th><th>Size (base = content font, default 11, range 10–16)</th><th>Face</th></tr>
        <tr><td>title</td><td>base + 1, semibold</td><td>system-ui</td></tr><tr><td>body</td><td>base</td><td>system-ui / mono for values</td></tr>
        <tr><td>secondary</td><td>base − 1</td><td>mono</td></tr><tr><td>caption</td><td>max(9, base − 2)</td><td>system-ui</td></tr>
        <tr><td>badge</td><td>max(8.5, base − 2), bold</td><td>mono</td></tr><tr><td>button</td><td>min(base + 1, 15)</td><td>system-ui</td></tr><tr><td>link</td><td>base − 0.5</td><td>system-ui</td></tr></table>
      <table class="t"><tr><th>Motion</th><th>Value</th><th>Source</th></tr>
        <tr><td>open</td><td>spring response 0.42 s, damping 0.82</td><td><code>NotchAnimation.open</code></td></tr>
        <tr><td>close</td><td>spring response 0.38 s, damping 1.0</td><td><code>NotchAnimation.close</code></td></tr>
        <tr><td>width / tab change</td><td>spring 0.40 / 0.84 · 0.38 / 0.82</td><td>NotchPanelView.swift:1386-1392</td></tr>
        <tr><td>micro (hover, buttons)</td><td>easeOut 0.12 s</td><td><code>NotchAnimation.micro</code></td></tr>
        <tr><td>prehover</td><td>easeOut 0.21 s, +7 pt, scale 1.004</td><td>NotchHoverInteraction</td></tr>
        <tr><td>content in</td><td>blur 5 → 0, opacity 0 → 1, scale 0.96 top</td><td><code>blurFade</code></td></tr>
        <tr><td>expand / collapse dwell</td><td>0.5 s / 0.5 s</td><td>Settings.swift:535</td></tr></table>
    </div>`;
  };

  // ---------- settings window ----------
  window.settingsHTML = function (page) {
    const side = [['System', [['general', 'General', 'gear'], ['appearance', 'Appearance', 'paint'], ['mascots', 'Mascots', 'face'], ['sound', 'Sound', 'sound'], ['shortcuts', 'Shortcuts', 'key']]],
      ['Features', [['agents', 'Agents', 'cpu'], ['music', 'Music', 'note']]],
      ['Morrow', [['behavior', 'Behavior', 'slider'], ['hooks', 'Hooks', 'link'], ['capabilities', 'Capabilities', 'grid'], ['about', 'About', 'about']]]];
    const sb = side.map(([g, items]) => `<h6>${g}</h6>` + items.map(([k, l, i]) => `<a class="${k === page ? 'on' : ''}">${ic(i, null, 13)}${l}</a>`).join('')).join('');
    const row = (n, s, cls, b) => `<div><span class="n">${n}</span><span class="s ${cls || ''}">${s}</span>${b ? `<button class="b">${b}</button>` : ''}</div>`;
    let pane = '';
    if (page === 'hooks') pane = `<h2>Hooks</h2><div class="grp">${row('Claude Code', 'Installed · script · bridge · 2.1.4', 'yes', 'Repair')}${row('Coverage', '7 of 9 agents found here are hooked')}</div>
      <div class="grp">${row('Codex', 'Installed', 'yes', 'Remove')}${row('Gemini CLI', 'Installed', 'yes', 'Remove')}${row('Cursor', 'Not installed', '', 'Install')}${row('OpenCode', 'Installed', 'yes', 'Remove')}${row('Kimi', 'Installed', 'yes', 'Remove')}${row('Hermes', 'Needs a manual merge: the file has a hooks block we did not write', 'lim')}</div><div class="grp missing">${row('Qoder', 'Not detected')}${row('Droid', 'Not detected')}</div>`;
    if (page === 'capabilities') pane = `<h2>Capabilities</h2><div class="grp">${row('Panel anchoring', 'Limited — GNOME does not let apps pin a panel above other windows', 'lim')}${row('Now Playing', 'Supported', 'yes')}${row('Lyrics', 'Supported', 'yes')}${row('System volume', 'Supported', 'yes')}${row('Jump to terminal', 'Limited — window focus only, not the tab', 'lim')}${row('Fullscreen detection', 'Not supported — no portable signal on Linux', 'no')}${row('Pointer outside the island', 'Not supported — Wayland hides the global pointer', 'no')}${row('Screen lock', 'Supported', 'yes')}${row('Tray icon', 'Supported', 'yes')}${row('Global hotkey', 'Not supported — X11 only', 'no')}${row('Start at login', 'Supported', 'yes')}</div>`;
    if (page === 'appearance') pane = `<h2>Appearance</h2><div class="grp">${row('Theme', '<span class="seg2"><b class="on">Pixel 16-bit</b><b>Modern Clean</b></span>')}${row('Content font size', '<span class="seg2"><b class="on">11</b><b>12</b><b>14</b><b>16</b></span>')}${row('Accent', 'Cyan')}${row('Animation speed', '1.0×')}</div><div class="grp">${row('Island shape', '<span class="seg2"><b class="on">Edge tab</b><b>Floating pill</b></span>')}${row('Position', 'Top centre · primary monitor', '', 'Reset')}${row('Expand on hover', '<span class="sw"></span>')}${row('Collapse when the pointer leaves', '<span class="sw"></span>')}</div>`;
    const title = { hooks: 'Morrow Settings', capabilities: 'Morrow Settings', appearance: 'Morrow Settings' }[page];
    return `<div class="win"><div class="wt">${title}<span class="cbx"><i>–</i><i>▢</i><i>✕</i></span></div><div class="wb"><nav class="side">${sb}</nav><div class="pane">${pane}</div></div></div>`;
  };
})();
