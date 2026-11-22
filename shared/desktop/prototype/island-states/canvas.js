// PROTOTYPE — renders SECTIONS, wires the controls and the variant switcher.
(function () {
  const VARIANTS = {
    A: { name: 'Edge tab · monitor top', text: '<b>Variant A — Edge tab.</b> What the Svelte shell does today: flush with the top edge of the primary monitor, square top, rounded bottom (12 pt collapsed, 24 pt open). On GNOME it covers the clock in the top bar.' },
    B: { name: 'Floating pill · work area', text: '<b>Variant B — Floating pill.</b> The macOS "Dynamic Island" mode on a screen without a notch: a capsule 6 pt below the top of the work area, 1.2 pt white 0.24 stroke and a soft shadow. Clears the GNOME top bar; costs 6 pt of screen.' },
    C: { name: 'Shoulder tab · work area', text: '<b>Variant C — Shoulder tab.</b> The macOS notch shape (NotchPanelShape, shoulders 10 pt collapsed, 14 pt open) hung from the top of the work area: under the GNOME bar, at the screen edge on Windows and KDE. The window must include the shoulders, and the hit-test must exclude their cut-outs.' },
  };
  const KEYS = Object.keys(VARIANTS);
  const p = new URLSearchParams(location.search);
  const state = { variant: KEYS.includes(p.get('variant')) ? p.get('variant') : 'A', theme: p.get('theme') || 'pixel', desk: p.get('desk') || 'win', f: p.get('f') || '11', ref: p.get('ref') || '0', win: p.get('win') || '1' };

  function scale(f) {
    const r = document.documentElement.style, b = +f;
    r.setProperty('--f', b + 'px'); r.setProperty('--f-title', b + 1 + 'px'); r.setProperty('--f-sec', b - 1 + 'px');
    r.setProperty('--f-cap', Math.max(9, b - 2) + 'px'); r.setProperty('--f-badge', Math.max(8.5, b - 2) + 'px');
    r.setProperty('--f-btn', Math.min(b + 1, 15) + 'px'); r.setProperty('--f-link', b - 0.5 + 'px');
  }
  const osbar = () => `<div class="osbar"><span><span class="pillws"></span><span class="pillws"></span></span><span class="clock">Tue 22 Nov 09:41</span><span class="tray">${ic('spk', '#fff', 13)}${ic('power', '#fff', 13)}</span></div>`;

  window.osbar = osbar;

  function render() {
    window.UI.modern = state.theme === 'modern';
    scale(state.f);
    document.body.classList.toggle('show-ref', state.ref === '1');
    document.body.classList.toggle('show-win', state.win === '1');
    document.querySelectorAll('.seg').forEach((seg) => seg.querySelectorAll('button').forEach((b) => b.setAttribute('aria-pressed', String(state[seg.dataset.k] === b.dataset.v))));
    document.getElementById('variant-callout').innerHTML = `${VARIANTS[state.variant].text}<div class="meta">Theme: ${state.theme === 'modern' ? 'Modern Clean' : 'Pixel 16-bit (default)'} · Desktop: ${{ win: 'Windows 11', gnome: 'GNOME (top bar 32 px)', kde: 'KDE Plasma (panel at the bottom)' }[state.desk]} · Content font ${state.f}</div>`;
    let html = '', toc = '';
    for (const s of window.SECTIONS) {
      if (s.group) { html += `<h2 class="group">${s.group}<small>${s.sub}</small></h2>`; continue; }
      toc += `<a href="#${s.id}">#${s.id}</a>`;
      const refs = (s.ref || []).map((r) => `<figure><img src="ref/${r}.webp" alt="SwiftUI render ${r}" loading="lazy"><figcaption>${r}</figcaption></figure>`).join('');
      const body = s.body();
      const stage = s.nostage ? body : `<div class="stage multi desk-${state.desk} v-${state.variant}${window.UI.modern ? ' modern' : ''}">${body}</div>`;
      html += `<section class="sec" id="${s.id}"><div class="sec-h"><h3>${s.title}</h3><span class="sub">${s.sub}</span>${s.port ? '<span class="badge-port">PORT</span>' : ''}<span class="meta">${s.meta}</span><span class="badge-t">${s.t}</span><a class="anchor" href="#${s.id}">#</a></div>
        ${stage}${refs ? `<div class="refs">${refs}</div>` : ''}<ul class="notes">${s.notes.map((n) => `<li>${n}</li>`).join('')}</ul></section>`;
    }
    document.getElementById('sections').innerHTML = html;
    document.getElementById('toc').innerHTML = toc;
    document.getElementById('vlabel').innerHTML = `<b>${state.variant}</b>${VARIANTS[state.variant].name}`;
    document.getElementById('themebtn').textContent = state.theme === 'modern' ? '◐ Modern Clean' : '◐ Pixel 16-bit';
    wireDemo();
    labelWindows();
    history.replaceState(null, '', '?' + new URLSearchParams(state).toString() + location.hash);
  }

  // Tight-fit label: the dashed box is the OS window; print its size.
  function labelWindows() {
    document.querySelectorAll('.isl').forEach((el) => {
      const i = el.querySelector(':scope > .winbox > i');
      if (i && !i.id) i.textContent = `window ${Math.round(el.offsetWidth)} × ${Math.round(el.offsetHeight)}`;
    });
  }

  // Motion demo: grow the window first on open, shrink it after on close.
  function wireDemo() {
    const isl = document.getElementById('demo-isl'); if (!isl) return;
    const inner = document.getElementById('demo-inner'), win = document.getElementById('demo-win'), lbl = document.getElementById('demo-win-l');
    inner.innerHTML = header({}) + `<div class="list">${pixelCard({ src: 'claude', st: 'working', cwd: 'web-app', br: 'feat/dashboard', t: '12M', tool: 'EDIT', detail: 'src/components/Dashboard.tsx', prompt: 'Add filters to the dashboard page' })}</div>`;
    const COL = [240, 34], OPEN = [680, 150];
    const setWin = ([w, h], tag) => { win.style.inset = 'auto'; win.style.left = `${(isl.offsetWidth - w) / 2}px`; win.style.top = '0'; win.style.width = w + 'px'; win.style.height = h + 'px'; lbl.textContent = `window ${w} × ${h} · ${tag}`; };
    let open = false;
    setWin(COL, 'collapsed');
    const toggle = () => {
      open = !open;
      if (open) {
        setWin(OPEN, 'resized first, then animate');
        isl.classList.remove('closing'); isl.classList.add('open', 'exp');
        isl.style.width = OPEN[0] + 'px'; isl.style.height = OPEN[1] + 'px';
        setTimeout(() => setWin(OPEN, 'open'), 450);
      } else {
        isl.classList.add('closing'); isl.classList.remove('open');
        isl.style.width = COL[0] + 'px'; isl.style.height = COL[1] + 'px';
        setWin(OPEN, 'animating · window still open size');
        setTimeout(() => { isl.classList.remove('exp'); setWin(COL, 'shrunk after the animation'); }, 400);
      }
    };
    isl.onclick = toggle;
    document.getElementById('demo-toggle').onclick = toggle;
  }

  document.querySelectorAll('.seg').forEach((seg) => seg.addEventListener('click', (e) => {
    const b = e.target.closest('button'); if (!b) return;
    state[seg.dataset.k] = b.dataset.v; render();
  }));
  const cycle = (d) => { state.variant = KEYS[(KEYS.indexOf(state.variant) + d + KEYS.length) % KEYS.length]; render(); };
  document.getElementById('prev').onclick = () => cycle(-1);
  document.getElementById('next').onclick = () => cycle(1);
  document.getElementById('themebtn').onclick = () => { state.theme = state.theme === 'modern' ? 'pixel' : 'modern'; render(); };
  addEventListener('keydown', (e) => {
    if (e.target.closest('input, textarea, [contenteditable]')) return;
    if (e.key === 'ArrowLeft') cycle(-1);
    else if (e.key === 'ArrowRight') cycle(1);
    else if (e.key.toLowerCase() === 't') document.getElementById('themebtn').click();
  });
  render();
  if (location.hash) document.querySelector(location.hash)?.scrollIntoView();
})();
