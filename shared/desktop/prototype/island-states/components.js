// PROTOTYPE — island components re-drawn from the SwiftUI sources. Each
// function returns an HTML string; the theme flag `ui.modern` picks the
// Modern Clean branch where the Swift code has one.
(function () {
  const PF = window.PIXEL_FONT, PI = window.PIXEL_ICONS;
  const MASCOT_DIR = '../../../../docs/images/mascots/';
  const MASCOTS = new Set(['aiwork', 'antigravity', 'claude', 'cline', 'codebuddy', 'codex', 'copilot', 'cursor', 'factory', 'gemini', 'grok', 'hermes', 'kimi', 'kiro', 'mimo', 'openclaw', 'opencode', 'pi', 'qoder', 'qwen', 'stepfun', 'trae', 'workbuddy']);
  const ui = (window.UI = { modern: false });
  const C = {
    cyan: '#00F5D4', amber: '#FFB833', red: '#F25959', white: '#F2F7FF', muted: '#99ADC7', dim: '#617085',
    bash: '#66FF80', edit: '#80B3FF', read: '#E6CC66', grep: '#CC99FF', agent: '#FF9966',
  };
  // SessionCardStatus (SessionListOrdering.swift:32-106); pixel colours from PixelRetroSessionCardView.swift:55-68
  const ST = {
    needs: { l: 'NEEDS YOU', m: 'Needs you', c: C.amber, fill: true },
    working: { l: 'WORKING', m: 'Working', c: C.cyan },
    thinking: { l: 'THINKING', m: 'Thinking', c: C.cyan },
    done: { l: 'DONE', m: 'Done', c: '#D9D9D9' },
    idle: { l: 'IDLE', m: 'Idle', c: C.muted, mc: 'rgba(255,255,255,.55)' },
    stopped: { l: 'STOPPED', m: 'Stopped', c: C.red, mc: '#FF8C66' },
    error: { l: 'ERROR', m: 'Error', c: C.red, mc: '#FF6170' },
  };
  const esc = (s) => String(s).replace(/[&<>"]/g, (ch) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[ch]));

  // PixelText: 5x7 glyph, advance 6 units, unknown glyph advances 3 (PixelText.swift:652-718)
  function px(text, size = 1.05, color = 'currentColor', max) {
    let ch = [...String(text).toUpperCase()];
    if (max && ch.length > max) ch = ch.slice(0, max > 2 ? max - 2 : max).concat(['.', '.']);
    let x = 0, d = '';
    for (const k of ch) {
      const g = PF[k];
      if (!g) { x += 3; continue; }
      for (let r = 0; r < 7; r++) for (let c = 0; c < 5; c++) if (g[r * 5 + c] === '1') d += `M${x + c} ${r}h1v1h-1z`;
      x += 6;
    }
    const w = Math.max(x - 1, 1);
    return `<svg class="px" viewBox="0 0 ${w} 7" width="${(w * size).toFixed(2)}" height="${(7 * size).toFixed(2)}" shape-rendering="crispEdges" role="img" aria-label="${esc(text)}"><path d="${d}" fill="${color}"/></svg>`;
  }
  // PixelIcon: 9x9 bitmaps (PixelIcon.swift)
  function pi(name, size = 1.35, color = 'currentColor') {
    const g = PI[name];
    let d = '';
    for (let r = 0; r < g.h; r++) for (let c = 0; c < g.w; c++) if (g.d[r * g.w + c] === '1') d += `M${c} ${r}h1v1h-1z`;
    return `<svg class="pi" viewBox="0 0 ${g.w} ${g.h}" width="${(g.w * size).toFixed(2)}" height="${(g.h * size).toFixed(2)}" shape-rendering="crispEdges" aria-hidden="true"><path d="${d}" fill="${color}"/></svg>`;
  }
  // Line icons standing in for SF Symbols (one stroke weight, 16-unit grid)
  const SV = {
    branch: '<circle cx="5" cy="4" r="1.6"/><circle cx="5" cy="12" r="1.6"/><circle cx="11" cy="5.5" r="1.6"/><path d="M5 5.6v4.8M11 7.1c0 2.4-6 1.6-6 3.3"/>',
    jump: '<rect x="2.5" y="3.5" width="10" height="10" rx="2"/><path d="M7 9l6-6M9 3h4v4"/>',
    folder: '<path d="M2 4.5h4l1.5 1.5H14v6.5H2z"/>',
    bell: '<path d="M4 11V7.5a4 4 0 0 1 8 0V11l1 1.2H3z"/><path d="M6.8 13.6a1.3 1.3 0 0 0 2.4 0"/>',
    moon: '<path d="M12.5 10.5A5 5 0 0 1 6 3.8a5 5 0 1 0 6.5 6.7z"/>',
    chev: '<path d="M4.5 6.5L8 10l3.5-3.5"/>',
    back: '<path d="M9.5 4L5.5 8l4 4"/>',
    copy: '<rect x="5.5" y="5.5" width="8" height="8" rx="1.5"/><path d="M3 10.5V3.5a1 1 0 0 1 1-1h6.5"/>',
    check: '<path d="M3.5 8.5l3 3 6-7"/>',
    term: '<rect x="2" y="3" width="12" height="10" rx="2"/><path d="M5 7l2 1.5L5 10M8.5 10.5H11"/>',
    info: '<circle cx="8" cy="8" r="6"/><path d="M8 7.2v4M8 5h.01"/>',
    warn: '<path d="M8 2.5l6 10.5H2z"/><path d="M8 6.5v3M8 11.3h.01"/>',
    grip: '<path d="M5 5h.01M8 5h.01M11 5h.01M5 9h.01M8 9h.01M11 9h.01"/>',
    cpu: '<rect x="4" y="4" width="8" height="8" rx="1.5"/><path d="M6.5 2v2M9.5 2v2M6.5 12v2M9.5 12v2M2 6.5h2M2 9.5h2M12 6.5h2M12 9.5h2"/>',
    note: '<path d="M6 12V3.5l7-1.5v8.5"/><circle cx="4.5" cy="12" r="1.6"/><circle cx="11.5" cy="10.5" r="1.6"/>',
    gear: '<circle cx="8" cy="8" r="2.2"/><path d="M8 1.8v2M8 12.2v2M1.8 8h2M12.2 8h2M3.6 3.6l1.4 1.4M11 11l1.4 1.4M3.6 12.4L5 11M11 5l1.4-1.4"/>',
    spk: '<path d="M2.5 6h2.5l3.5-3v10l-3.5-3H2.5z"/><path d="M11 5.5a3.5 3.5 0 0 1 0 5"/>',
    power: '<path d="M8 2v5.5"/><path d="M4.8 4.3a5 5 0 1 0 6.4 0"/>',
    link: '<path d="M6.5 9.5l3-3M5 7.5l-1.3 1.3a2.5 2.5 0 0 0 3.5 3.5L8.5 11M11 8.5l1.3-1.3a2.5 2.5 0 0 0-3.5-3.5L7.5 5"/>',
    paint: '<path d="M10.5 2.5l3 3-6 6H4.5v-3z"/><path d="M2.5 13.5h4"/>',
    face: '<circle cx="8" cy="8" r="6"/><path d="M6 6.8h.01M10 6.8h.01M5.8 10a3 3 0 0 0 4.4 0"/>',
    key: '<rect x="2" y="4.5" width="12" height="7" rx="1.5"/><path d="M4.5 7h.01M7 7h.01M9.5 7h.01M5.5 9.3h5"/>',
    slider: '<path d="M3 5h10M3 11h10"/><circle cx="6" cy="5" r="1.5"/><circle cx="10" cy="11" r="1.5"/>',
    grid: '<rect x="2.5" y="2.5" width="4.5" height="4.5" rx="1"/><rect x="9" y="2.5" width="4.5" height="4.5" rx="1"/><rect x="2.5" y="9" width="4.5" height="4.5" rx="1"/><rect x="9" y="9" width="4.5" height="4.5" rx="1"/>',
    about: '<circle cx="8" cy="8" r="6"/><path d="M8 7.2v4M8 5h.01"/>',
    sound: '<path d="M2.5 6h2.5l3.5-3v10l-3.5-3H2.5z"/><path d="M11 5.5a3.5 3.5 0 0 1 0 5M12.8 3.8a6 6 0 0 1 0 8.4"/>',
  };
  const ic = (n, color, size = 12) => `<svg class="ico" viewBox="0 0 16 16" width="${size}" height="${size}" style="${color ? `color:${color}` : ''}" aria-hidden="true">${SV[n]}</svg>`;
  const qbubble = (pulse) => `<svg width="15" height="14" viewBox="0 0 15 14" class="${pulse ? 'pulse' : ''}" aria-label="question waiting"><path d="M2 1h11a1.5 1.5 0 0 1 1.5 1.5v6A1.5 1.5 0 0 1 13 10H6l-3.5 3V10H2A1.5 1.5 0 0 1 .5 8.5v-6A1.5 1.5 0 0 1 2 1z" fill="#FFB347"/><path d="M5.8 4.2a1.7 1.7 0 1 1 2.3 1.6c-.5.2-.6.5-.6 1M7.5 8.2h.01" stroke="#000" stroke-width="1.3" fill="none" stroke-linecap="round"/></svg>`;
  const checkCircle = () => `<svg width="13" height="13" viewBox="0 0 16 16" aria-label="completed" style="filter:drop-shadow(0 0 2px #00F5D4)"><circle cx="8" cy="8" r="7" fill="#00F5D4"/><path d="M4.8 8.3l2.2 2.2 4.2-4.6" stroke="#000" stroke-width="1.8" fill="none" stroke-linecap="round" stroke-linejoin="round"/></svg>`;

  function mascot(src, size, opts = {}) {
    const file = MASCOTS.has(src) ? src : 'claude';
    return `<span class="masc" style="--ms:${size}px"><img src="${MASCOT_DIR}${file}.gif" alt="${esc(src)} mascot" width="${size}" height="${size}">${opts.sleep ? `<i class="zz">${px('z', 1, C.muted)}</i>` : ''}</span>`;
  }
  const badge = (t, c, fill) => `<span class="pb${fill ? ' fill' : ''}" style="--c:${c}">${px(t, 1.05, fill ? '#000' : c)}</span>`;
  const toolColor = (t) => ({ BASH: C.bash, EDIT: C.edit, WRITE: C.edit, READ: C.read, GREP: C.grep, GLOB: C.grep, MCP: C.grep, AGENT: C.agent }[t] || 'rgba(255,255,255,.7)');

  // ---------- collapsed bar (CompactLeftWing / CompactRightWing) ----------
  function dock(label, active = true) {
    return `<span class="dock">${pi('workspace', .9, active ? C.cyan : C.dim)}<span class="t">${esc(label)}</span></span>`;
  }
  function collapsed(state, o = {}) {
    const src = o.src || 'claude';
    const label = (s) => (ui.modern ? `<span style="font:600 10px var(--sans);color:${s.c}">${esc(s.t)}</span>` : px(s.t, 1.3, s.c));
    let left = mascot(src, 21, { sleep: ['idle', 'empty', 'quiet'].includes(state) }), right = '', cls = '';
    switch (state) {
      case 'empty': right = label({ t: 'IDLE', c: C.muted }); break;
      case 'idle': right = pi('workspace', 1.5, C.dim); break;
      case 'prehover': cls = ' pre'; right = pi('workspace', 1.5, C.dim); break;
      case 'thinking':
        left += `<span class="thinkdots"><i></i><i></i><i></i></span>${label({ t: 'THINKING', c: C.cyan })}`;
        right = dock('web-app') + `<span class="dock"><span class="dotg"></span><span class="t">2/4</span></span>`; break;
      case 'tool':
        left += `${label({ t: 'BASH', c: C.bash })}<span class="tooldetail">npm run test -- --coverage</span>`;
        right = dock('web-app'); break;
      case 'approval': left = mascot('mimo', 21); right = dock('ml-pipeline') + `<span class="pulse" style="color:${C.amber}">${ic('bell', '#FFB347', 11)}</span>`; break;
      case 'question': left = mascot('claude', 21); right = dock('web-app') + qbubble(true); break;
      case 'question-dismissed': left = mascot('claude', 21); right = dock('web-app') + qbubble(false); break;
      case 'glance': right = dock('web-app') + checkCircle(); break;
      case 'quiet': right = pi('workspace', 1.5, C.dim) + ic('moon', 'rgba(255,255,255,.5)', 10); break;
      case 'music':
        left += `<span style="color:${C.cyan}">${ic('note', C.cyan, 10)}</span><span class="tooldetail" style="max-width:150px">Midnight City · M83</span>`;
        right = `<span class="spec">${'<i></i>'.repeat(5).replace(/<i><\/i>/g, (m, i) => `<i style="animation-delay:-${(Math.random() * .9).toFixed(2)}s"></i>`)}</span>`; break;
      case 'hidden': return `<div class="isl col hidden"><span class="cap" style="margin:0 auto">hidden</span><div class="winbox"><i>no window shown</i></div></div>`;
    }
    return `<div class="isl col${cls}"><span class="wing">${left}</span><span class="gap"></span><span class="wing">${right}</span><div class="winbox"><i>tight-fit window</i></div></div>`;
  }

  // ---------- expanded header (HeaderNavigationMenuTabs, SessionGroupingTabs) ----------
  function header(o = {}) {
    const tab = o.tab || 'agents', filt = o.filter || 'all';
    // Agents + Music only: Work, Prayer, Mail and Game tabs have no user story in #82.
    const tabs = [['agents', 'AGENTS', 'cpu'], ['music', 'MUSIC', 'note']];
    const t = tabs.map(([k, l, svg]) => {
      const on = k === tab;
      if (ui.modern) return `<span class="tab${on ? ' on' : ''}">${ic(svg, on ? '#fff' : 'rgba(255,255,255,.55)', 11)}${on ? `<span class="tl">${l[0] + l.slice(1).toLowerCase()}</span>` : ''}</span>`;
      return `<span class="tab${on ? ' on' : ''}">${pi(k, 1.35, on ? C.cyan : 'rgba(255,255,255,.55)')}${on ? px(l, 1.15, C.cyan) : ''}</span>`;
    }).join('');
    const f = [['all', 'ALL', 'All'], ['sta', 'STA', 'Status'], ['agt', 'AGT', 'Agent']].map(([k, l, m]) =>
      ui.modern ? `<b class="${k === filt ? 'on' : ''}">${m}</b>` : `<b class="${k === filt ? 'on' : ''}">${px(l, 1.05, k === filt ? C.cyan : 'rgba(255,255,255,.55)')}</b>`).join('');
    const btn = (glyph, svg, color) => ui.modern ? `<span class="hbtn">${ic(svg, color, 11)}</span>` : `<span class="hbtn" style="--hc:${color}">${pi(glyph, 1.35, color)}</span>`;
    const power = o.quit ? `<span class="quit">${px('QUIT?', 1.15, '#000')}</span>` : btn('power', 'power', C.red);
    return `<div class="hdr">${t}<span class="hsp"></span>${tab === 'agents' ? `<span class="filt">${f}</span>` : ''}${btn(o.muted ? 'volumeMute' : 'volume', 'spk', '#fff')}${btn('settings', 'gear', '#fff')}${power}</div>`;
  }

  // ---------- session cards ----------
  function tasks(s) {
    const [d, t, name] = s.tasks;
    const seg = Array.from({ length: 12 }, (_, i) => { const f = Math.round((d / t) * 12); return `<i class="${i < f ? 'on' : i === f ? 'cur' : ''}"></i>`; }).join('');
    return `<div class="ptasks">${px(`[${d}/${t} TASKS]`, .95, d === t ? C.cyan : C.white)}<span class="gauge">${seg}</span>${px('▶ ' + name, .95, C.muted, 40)}</div>`;
  }
  function inlineApproval(tool) {
    return `<div class="papp"><div class="r">${px('[!]', 1.2, C.amber)}${px('APPROVAL REQUIRED:', 1.1, C.amber)}${px(tool, 1.1, C.white)}</div><div class="r"><button class="p16 acc">${pi('checkmark', 1, '#000')}${px('APPROVE', 1.05, '#000')}</button><button class="p16">${px('✕ DENY', 1.05, C.red)}</button></div></div>`;
  }
  function pixelCard(s, o = {}) {
    const st = ST[s.st];
    return `<article class="pcard${o.hover ? ' hov' : ''}" style="--sc:${st.c}">
      <div class="pslot">${mascot(s.src, 28, { sleep: s.st === 'idle' })}<span class="leds"><i></i><i></i><i></i></span></div>
      <div class="pbody">
        <div class="prow">${badge(st.l, st.c, st.fill)}${badge(s.src, C.muted)}${pi('workspace', .85, C.white)}${px(s.cwd, 1.05, C.white, 26)}${s.br ? px('//', 1.05, C.dim) + ic('branch', st.c, 10) + px(s.br, .95, st.c, 22) : ''}${s.sub ? badge(`+${s.sub} SUB`, C.cyan) : ''}<span class="sp"></span>${o.hover ? `<span class="del">${px('✕', .9, C.red)}</span>` : ''}${px(s.t, 1.05, C.dim)}</div>
        ${s.tasks ? tasks(s) : ''}
        ${s.tool ? `<div class="ptool">${badge('▸ ' + s.tool, st.c)}<span>${esc(s.detail)}</span></div>` : ''}
        ${s.approval ? inlineApproval(s.approval) : ''}
        ${s.prompt ? `<div class="pline"><b style="color:rgba(0,245,212,.8)">›</b><span>${esc(s.prompt)}</span></div>` : ''}
        ${s.reply ? `<div class="pline r"><b style="color:${st.c}">●</b><span>${esc(s.reply)}</span></div>` : ''}
        ${s.recap ? `<div class="precap"><b>↻</b>${esc(s.recap)}</div>` : ''}
      </div></article>`;
  }
  function modernCard(s) {
    const st = ST[s.st], sc = st.mc || st.c;
    return `<article class="mcard" style="--sc:${sc}"><i class="rail"></i>${mascot(s.src, 32, { sleep: s.st === 'idle' })}
      <div class="mbody">
        <div class="mrow"><span class="chip">${st.m}</span><b class="mcwd">${esc(s.cwd)}</b>${s.br ? `<span class="mbr">${ic('branch', null, 10)}${esc(s.br)}</span>` : ''}${s.sub ? `<span class="mtag" style="--c:#A68CF2">+${s.sub} Sub</span>` : ''}<span class="hsp"></span><span class="msrc">${esc(s.src)}</span><span class="mt">${s.t.toLowerCase()}</span></div>
        ${s.tool ? `<div class="mtool"><span class="tc" style="--tc:${toolColor(s.tool)}">${s.tool[0] + s.tool.slice(1).toLowerCase()}</span><span>${esc(s.detail)}</span></div>` : ''}
        ${s.approval ? `<div class="acts"><span class="tagp">PERMISSION</span><span class="tt" style="font-size:var(--f)">${s.approval[0] + s.approval.slice(1).toLowerCase()} wants to run a command</span><span class="sp"></span><button class="btn d">Deny</button><button class="btn p">Allow once</button></div>` : ''}
        ${s.prompt ? `<div class="mline"><b>›</b><span>${esc(s.prompt)}</span></div>` : ''}
        ${s.reply ? `<div class="mline" style="color:rgba(255,255,255,.65)"><span>${esc(s.reply)}</span></div>` : ''}
      </div></article>`;
  }
  const card = (s, o) => (ui.modern ? modernCard(s) : pixelCard(s, o));
  function compactRow(s) {
    const st = ST[s.st];
    const detail = s.tool ? `${px('[' + s.tool + ']', .95, toolColor(s.tool))}` : s.st === 'thinking' ? '<span class="thinkdots"><i></i><i></i><i></i></span>' : s.prompt ? `<span class="d">› ${esc(s.prompt)}</span>` : '';
    return `<div class="prow-c">${mascot(s.src, 15, { sleep: s.st === 'idle' })}<span class="nm">${px(s.cwd, 1.05, C.white, 24)}${s.br ? px('//', 1.05, C.dim) + px(s.br, .95, st.c, 18) : ''}</span><span class="d" style="display:flex;gap:6px;align-items:center">${detail}</span>${badge(st.l, st.c, st.fill)}${px(s.t, 1.05, C.dim)}</div>`;
  }

  // ---------- cards (ApprovalBar / QuestionBar) ----------
  function ctxRow(o = {}) {
    return `<div class="ctx${o.hover ? ' hov' : ''}" title="Click to focus this session's terminal">${mascot(o.src || 'claude', 15)}<span>${esc(o.cwd || 'web-app')}</span><span class="br">${ic('branch', null, 10)}${esc(o.br || 'feat/dashboard')}</span>${o.noJump ? '' : `<span class="jump">${ic('jump', null, 11)}</span>`}${o.queue ? `<span class="q">${o.queue}</span>` : ''}</div>`;
  }
  const kbd = (k, on) => (on ? ` <kbd>${k}</kbd>` : '');
  function approval(kind = 'bash', o = {}) {
    const K = o.keys;
    const body = {
      bash: { tool: 'Bash', title: 'Bash wants to run a command', code: `<span>$ npm run test -- --coverage</span>\n<span class="c">Run the test suite with a coverage report</span>` },
      edit: { tool: 'Edit', title: 'Edit wants to change Dashboard.tsx', code: `<span class="c">~/code/web-app/src/components/Dashboard.tsx</span>\n<span class="rem">- const [orders, setOrders] = useState&lt;Order[]&gt;([]);</span>\n<span class="add">+ const [orders, setOrders] = useState&lt;Order[]&gt;([]);</span>\n<span class="add">+ const [filters, setFilters] = useState&lt;OrderFilters&gt;(defaultFilters);</span>` },
      mcp: { tool: 'create_pull_request', title: 'github wants to call create_pull_request', srv: 'MCP · github', code: `<span class="k">base</span> main\n<span class="k">body</span> Adds a date-range and status filter bar above the orders table.\n<span class="k">head</span> feat/dashboard\n<span class="k">owner</span> acme` },
    }[kind];
    return `<div class="card${o.dim ? ' dim' : ''}${o.shake ? ' shake' : ''}">
      ${ctxRow({ queue: o.queue, hover: o.ctxHover, noJump: o.noJump })}
      ${o.notice || ''}
      <div class="ttl"><span class="tagp">PERMISSION</span><span class="tt">${body.title}</span>${body.srv ? `<span class="tsrv">${body.srv}</span>` : ''}</div>
      <div class="code">${body.code}</div>
      <div class="acts"><a class="always">Always allow ${body.tool} this session</a>${kbd('Ctrl+Alt+L', K)}<span class="sp"></span>
        <button class="btn q">Hide${kbd('Esc', K)}</button><button class="btn d">Deny${kbd('Ctrl+Alt+D', K)}</button><button class="btn p">Allow once${kbd('Ctrl+Alt+A', K)}</button></div>
    </div>`;
  }
  function question(kind = 'single', o = {}) {
    const opts = {
      single: [['Zustand (Recommended)', 'Tiny hook-based store with almost no boilerplate'], ['Redux Toolkit', 'Structured slices, DevTools and RTK Query built in'], ['React Context', 'Built into React; fine for state that rarely changes']],
      multi: [['Date range', 'Presets plus a custom range'], ['Order status', ''], ['Customer', 'Type-ahead search'], ['Amount', '']],
    };
    const head = (tag, q, prog) => `<div class="qh"><span class="qmark">?</span>${tag ? `<span class="tagq">${tag}</span>` : ''}<span class="qt">${q}</span>${prog ? `<span class="prog">${prog}</span>` : ''}</div>`;
    let mid = '', acts = '';
    const sel = o.sel ?? -1, hov = o.hover ?? -1;
    if (kind === 'single' || kind === 'other' || kind === 'wizard') {
      const list = opts.single.map(([l, d], i) => `<div class="opt${i === hov ? ' hov' : ''}${i === sel ? ' sel' : ''}"><span class="mk">▸</span><span class="ix">${i + 1}.</span><div><div class="ol">${l}</div>${d ? `<div class="od">${d}</div>` : ''}</div></div>`).join('');
      const other = `<div class="opt${kind === 'other' ? ' sel' : ''}"><span class="mk">▸</span><span class="ix">…</span><div><div class="ol">Other</div></div></div>`;
      mid = head('State', 'Which state library should the dashboard use?', kind === 'wizard' ? '2/3' : '') + `<div class="opts">${list}${other}</div>`;
      if (kind === 'other') mid += `<div class="ans focus"><b>&gt;</b><span class="val">Jotai, we already use it in admin</span><i class="caret"></i></div>`;
      acts = `${kind === 'wizard' ? `<button class="btn q">${ic('back', null, 11)} Back</button>` : ''}<span class="sp"></span><button class="btn q">Hide</button><button class="btn s">Skip</button>${kind === 'other' ? '<button class="btn p">Submit</button>' : kind === 'wizard' ? '<button class="btn p">Next</button>' : ''}`;
    } else if (kind === 'multi') {
      const picked = o.picked || [];
      const list = opts.multi.concat([['Other', '']]).map(([l, d], i) => `<div class="opt${picked.includes(i) ? ' sel' : ''}"><span class="mk">▸</span><span class="cb">${picked.includes(i) ? ic('check', '#000', 9) : ''}</span><div><div class="ol">${l}</div>${d ? `<div class="od">${d}</div>` : ''}</div></div>`).join('');
      mid = head('Filters', 'Which filters should the first version ship with?') + `<div class="opts">${list}</div>`;
      acts = `<span class="sp"></span><button class="btn q">Hide</button><button class="btn s">Skip</button><button class="btn p"${picked.length ? '' : ' disabled'}>Confirm</button>`;
    } else if (kind === 'free') {
      mid = head('', 'What should the default date range be?') + `<div class="ans${o.focus ? ' focus' : ''}"><b>&gt;</b>${o.focus ? '<span class="val">Last 30 days</span><i class="caret"></i>' : '<span class="ph">Type your answer...</span>'}</div>`;
      acts = `<span class="sp"></span><button class="btn q">Hide</button><button class="btn s">Skip</button><button class="btn p"${o.focus ? '' : ' disabled'}>Submit</button>`;
    }
    return `<div class="card">${ctxRow({})}${o.notice || ''}${mid}<div class="acts">${acts}</div></div>`;
  }
  function completion(kind) {
    const s = { src: 'claude', st: 'done', cwd: 'web-app', br: 'feat/dashboard', t: 'NOW', prompt: 'Add filters to the dashboard page' };
    const reply = kind === 'short'
      ? `<div class="md"><p style="margin:0">Shipped dashboard filters; 42 tests pass. Next: your call on the export page.</p></div>`
      : `<div class="md"><h4>## Dashboard filters</h4><p style="margin:0">Added a <strong>date-range</strong> and <strong>status</strong> filter bar above the orders table. State lives in <code>useOrderFilters</code>.</p>
         <ul><li>Presets: today, 7 days, 30 days, custom</li><li>Status chips are multi-select</li><li>URL keeps the filters, so a reload keeps them</li></ul>
         <div class="cb"><span class="lang">ts</span><span class="cp">${ic('copy', '#fff', 11)}</span>const filters = useOrderFilters();
const rows = useOrders({ ...filters, page });</div>
         <table><tr><th>Suite</th><th>Tests</th><th>Result</th></tr><tr><td>orders</td><td>28</td><td>pass</td></tr><tr><td>filters</td><td>14</td><td>pass</td></tr></table>
         <blockquote>Export page still uses the old query; not touched.</blockquote></div>`;
    const head = ui.modern ? modernCard({ ...s, reply: '' }) : pixelCard({ ...s });
    return `<div class="list">${head}</div><div class="card" style="padding-top:4px">${reply}</div><div class="more">2 sessions ${ic('chev', null, 10)}</div>`;
  }

  // ---------- music (RetroCyberMusicHUDContentView / Modern) ----------
  function scrub(p) { return `<div class="scrub">${Array.from({ length: 40 }, (_, i) => `<i class="${i < p ? 'on' : i === p ? 'ph' : ''}"></i>`).join('')}</div>`; }
  function music(kind = 'playing') {
    const playing = kind === 'playing', none = kind === 'none';
    const lyr = kind === 'nolyrics' ? `<div class="lyr" style="justify-content:center;-webkit-mask:none;mask:none">${px('[ NO LYRIC FEED ]', 1.1, 'rgba(255,255,255,.4)')}</div>`
      : none ? `<div class="lyr" style="justify-content:center;-webkit-mask:none;mask:none">${px('[ NOTHING PLAYING ]', 1.1, 'rgba(255,255,255,.4)')}</div>`
      : `<div class="lyr"><span>Waiting in a car</span><span>Waiting for a ride in the dark</span><span class="cur">▶ The night city grows</span><span>Look and see her eyes, they glow</span><span>Waiting in a car</span></div>`;
    if (ui.modern) {
      return `<div class="mdeck"><div class="art${none ? ' none' : ''}">${none ? ic('note', null, 26) : ''}</div><div style="display:flex;flex-direction:column;gap:6px;min-width:0">
        <div style="font:500 11.5px var(--sans)">${none ? 'Nothing playing' : 'Midnight City'}</div><div style="font:10px var(--sans);color:rgba(255,255,255,.6)">${none ? 'Start a player: Spotify, a browser tab, mpv…' : 'M83 · Hurry Up, We\'re Dreaming'}</div>
        ${none ? '' : `<div style="height:4px;border-radius:2px;background:rgba(255,255,255,.18)"><div style="width:42%;height:100%;border-radius:2px;background:#fff"></div></div><div style="display:flex;justify-content:space-between;font:500 7.5px var(--mono);color:rgba(255,255,255,.6)"><span>1:47</span><span>4:03</span></div>`}
        <div class="transport" style="gap:14px">${['◀◀', playing ? '❚❚' : '▶', '▶▶'].map((g) => `<span style="font:600 11px var(--sans);opacity:${none ? .3 : 1}">${g}</span>`).join('')}</div></div></div>`;
    }
    return `<div class="deck"><div class="l">
        <div class="top">${px('AUDIO DECK', 1.1, C.cyan)}<span class="player slotb">${none ? '—' : 'SPOTIFY'}</span></div>
        <div class="trk"><div class="art${none ? ' none' : ''}">${none ? ic('note', null, 14) : ''}</div><div style="min-width:0"><div class="ti">${none ? 'Nothing playing' : 'Midnight City'}</div><div class="ar">${none ? 'No MPRIS / media session' : 'M83'}</div></div></div>
        ${scrub(none ? -1 : 17)}
        <div class="times">${none ? '' : `<span style="color:rgba(0,245,212,.85)">1:47</span><span style="color:rgba(255,255,255,.25)">//</span><span style="color:rgba(255,255,255,.5)">4:03</span><span class="hsp"></span>`}
          <span class="transport"><button class="tb">${px('◀◀', 1, none ? C.dim : C.white)}</button><button class="tb pp">${playing ? px('❚❚', 1, C.cyan) : px('▶', 1, none ? C.dim : C.cyan)}</button><button class="tb">${px('▶▶', 1, none ? C.dim : C.white)}</button></span></div>
      </div><div class="r"><div class="top">${px('LYRICS', 1.1, C.cyan)}${kind === 'playing' ? badge('SYNCED', C.cyan) : ''}</div>${lyr}</div></div>`;
  }
  function volumeHud(pct, muted) {
    const lit = muted ? 0 : Math.round(pct / 100 * 12);
    const col = (i) => (i < 8 ? C.cyan : i < 10 ? C.amber : C.red);
    return `<span class="vol"><span class="spk slotb">${ic('spk', muted ? '#FF4761' : C.cyan, 11)}</span><span class="led slotb">${Array.from({ length: 12 }, (_, i) => `<i style="${i < lit ? `background:${col(i)}` : ''}"></i>`).join('')}</span><span class="rd" style="color:${muted ? C.red : C.white}">${muted ? 'MUTE' : pct + '%'}</span></span>`;
  }

  Object.assign(window, { px, pi, ic, mascot, badge, collapsed, header, card, pixelCard, modernCard, compactRow, approval, question, completion, music, volumeHud, esc, PCOL: C, ST, qbubble, checkCircle });
})();
