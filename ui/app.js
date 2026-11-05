/* Frost Client launcher UI — plain JS, talks to the Rust backend through Tauri commands. */
'use strict';

const TAURI = window.__TAURI__;
const inv = (cmd, args) => TAURI.core.invoke(cmd, args);
const $ = (s, r = document) => r.querySelector(s);
const esc = s => String(s ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
const fmtNum = n => n >= 1e9 ? (n / 1e9).toFixed(1) + 'B' : n >= 1e6 ? (n / 1e6).toFixed(1) + 'M' : n >= 1e3 ? (n / 1e3).toFixed(1) + 'K' : String(n);

const ICON = {
  flake: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><path d="M12 2v20M3.3 7l17.4 10M20.7 7 3.3 17M9 3.6l3 2.4 3-2.4M9 20.4l3-2.4 3 2.4M3 10.5l3.4.8-.8 3.4M21 10.5l-3.4.8.8 3.4"/></svg>',
  gear: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/></svg>',
  min: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M5 12h14"/></svg>',
  max: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="5" y="5" width="14" height="14" rx="2"/></svg>',
  close: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M6 6l12 12M18 6 6 18"/></svg>',
  folder: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>',
  sliders: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><path d="M4 7h10M18 7h2M4 17h2M10 17h10"/><circle cx="16" cy="7" r="2"/><circle cx="8" cy="17" r="2"/></svg>',
  search: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/></svg>',
};

/* ------------------------------------------------------------------ state */
const VERSION_META = {
  '1.8.9':   { art: 'radial-gradient(120% 90% at 20% 0%,#2aa7c9 0%,transparent 60%),linear-gradient(135deg,#0d3b66,#0b1e44)', note: 'The classic 1.8 PvP experience (Forge).' },
  '1.21.10': { art: 'radial-gradient(120% 90% at 80% 0%,#7a5cff 0%,transparent 60%),linear-gradient(135deg,#1b2a6b,#0b1233)', note: 'Modern combat with the Frost HUD.' },
  '1.21.11': { art: 'radial-gradient(120% 90% at 20% 10%,#4f8bff 0%,transparent 62%),linear-gradient(135deg,#17408f,#0a1a3d)', note: 'Latest 1.21 release with the Frost HUD.' },
  '26.1.2':  { art: 'radial-gradient(120% 90% at 70% 0%,#6fe0ff 0%,transparent 60%),linear-gradient(135deg,#0f5c8a,#0a2447)', note: 'Minecraft 26.1.2 with the Frost HUD.' },
  '26.2':    { art: 'radial-gradient(120% 90% at 30% 0%,#b86cff 0%,transparent 60%),linear-gradient(135deg,#3b2a8f,#0e1240)', note: 'Minecraft 26.2 with the Frost HUD.' },
};
const HUD_LABELS = {
  cps: 'CPS', fps: 'FPS', ping: 'Ping', keystrokes: 'Keystrokes', armor_status: 'Armor Status', potion_status: 'Potion Status',
  direction: 'Direction', reach_display: 'Reach', coordinates: 'Coordinates / Biome',
  bw_resources: 'Bedwars resources', bw_blocks: 'Bedwars blocks', bridge_speed: 'Bridge speed',
};
const QOL = [
  ['fullbright', 'Fullbright', 'Always see everything, even in caves.'],
  ['minimal_viewmodel', 'Minimal viewmodel', 'Smaller, calmer held items.'],
  ['item_physics', 'Item physics', 'Dropped items lie flat on the ground.'],
  ['dynamic_fov', 'Dynamic FOV', 'FOV changes with sprinting and speed effects.'],
  ['zoom_enabled', 'Zoom', 'Hold the zoom key to look far.'],
  ['show_coordinates', 'Coordinates', 'Show your position in the HUD.'],
  ['show_biome', 'Biome', 'Show the current biome.'],
  ['hitboxes', 'Hitboxes', 'Draw entity hitboxes.'],
  ['fps_boost', 'FPS boost', 'Trim expensive effects for more frames.'],
  ['chunk_optimizer', 'Chunk optimizer', 'Cap chunk work per frame.'],
  ['no_entity_shadows', 'No entity shadows', 'Skip the dark blobs under entities.'],
  ['low_biome_blend', 'Low biome blend', 'Cheaper chunk meshing (blend radius 1).'],
  ['cap_sim_distance', 'Cap simulation distance', 'Limit entity / block-tick range to 8 chunks.'],
  ['no_menu_blur', 'No menu blur', 'Remove the blurred menu background.'],
  ['no_clouds', 'No clouds', 'Turn clouds off.'],
  ['fast_graphics', 'Fast graphics (1.8.9)', 'Fast graphics and flat lighting on 1.8.9.'],
];
const MR_CATS = {
  mod: ['adventure', 'cursed', 'decoration', 'economy', 'equipment', 'food', 'game-mechanics', 'library', 'magic', 'management', 'minigame', 'mobs', 'optimization', 'social', 'storage', 'technology', 'transportation', 'utility', 'worldgen'],
  resourcepack: ['combat', 'decoration', 'entities', 'environment', 'equipment', 'fonts', 'gui', 'items', 'models', 'realistic', 'simplistic', 'themed', 'tweaks', 'utility', 'vanilla-like'],
  shader: ['cartoon', 'fantasy', 'high', 'low', 'medium', 'potato', 'realistic', 'semi-realistic', 'vanilla-like'],
};
const KIND_LABEL = { mod: 'Mods', resourcepack: 'Resource packs', shader: 'Shaders' };

const S = {
  page: 'home', st: null, cfg: null, sel: '1.21.11',
  launch: { phase: 'idle', msg: '', frac: 0 }, loggingIn: false,
  settingsTab: 'launcher', hudSel: 'cps', ipc: false,
  mr: { mc: null, kind: 'mod', category: '', query: '', sort: 'relevance', offset: 0, limit: 24, page: null, loading: false, error: '', installed: [], busy: new Set(), tab: 'discover', token: 0 },
};

const loaderOf = v => (S.st?.instances.find(i => i.id === v)?.loader) || (v === '1.8.9' ? 'Forge' : 'Fabric');
const inst = v => S.st?.instances.find(i => i.id === v);
const selInst = () => inst(S.sel);
let saveTimer = null;
function saveCfg() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => inv('save_settings', { settings: S.cfg }).catch(e => toast(e, 'err')), 250);
}

function toast(msg, kind = '') {
  const el = document.createElement('div');
  el.className = 'toast ' + kind;
  el.textContent = String(msg);
  $('#toasts').appendChild(el);
  setTimeout(() => el.remove(), kind === 'err' ? 7000 : 3500);
}

function modal({ title, body, actions }) {
  const root = $('#modal-root');
  root.innerHTML = `<div class="modal-bg"><div class="modal glass"><h3>${esc(title)}</h3><p>${body}</p><div class="acts"></div></div></div>`;
  const acts = $('.acts', root);
  for (const a of actions) {
    const b = document.createElement('button');
    b.className = 'btn ' + (a.cls || 'ghost');
    b.textContent = a.label;
    b.onclick = () => { root.innerHTML = ''; a.onClick && a.onClick(); };
    acts.appendChild(b);
  }
}

function ago(ts) {
  if (!ts) return 'Never';
  const d = Math.floor((Date.now() / 1000 - ts) / 86400);
  return d <= 0 ? 'Today' : d === 1 ? 'Yesterday' : `${d} days ago`;
}

/* ------------------------------------------------------------------ shell */
function render() {
  if (!S.st) return;
  const acc = S.st.account;
  const I = {
    home: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><path d="M3 11 12 3l9 8v9a1 1 0 0 1-1 1h-5v-6H9v6H4a1 1 0 0 1-1-1z"/></svg>',
    versions: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><path d="m12 3 9 5-9 5-9-5zM3 13l9 5 9-5"/></svg>',
    browse: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><rect x="3" y="3" width="8" height="8" rx="2"/><rect x="13" y="3" width="8" height="8" rx="2"/><rect x="3" y="13" width="8" height="8" rx="2"/><rect x="13" y="13" width="8" height="8" rx="2"/></svg>',
    cosmetics: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><path d="m12 3 2.6 5.6 6 .7-4.5 4.1 1.2 6L12 16.4 6.7 19.4l1.2-6L3.4 9.3l6-.7z"/></svg>',
    skinedit: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><path d="M4 20l4-1L19 8l-3-3L5 16z"/><path d="M14 7l3 3"/></svg>',
    perf: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><path d="M13 3 5 13h6l-1 8 8-10h-6z"/></svg>',
    capeedit: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><path d="M8 4h8l3 4-2 12H7L5 8z"/><path d="M9 4c0 2 6 2 6 0"/></svg>',
  };
  const nav = [['home', 'Home'], ['versions', 'Versions'], ['browse', 'Browse'], ['cosmetics', 'Cape'], ['skinedit', 'Skin editor'], ['capeedit', 'Cape editor'], ['perf', 'Performance']];
  $('#app').innerHTML = `
    <div class="shell">
      <aside class="rail">
        <div class="brand">${ICON.flake}<span>FROST</span></div>
        <nav>${nav.map(([k, l]) => `<button class="${S.page === k ? 'on' : ''}" data-act="nav" data-p="${k}">${I[k]}<span>${l}</span></button>`).join('')}</nav>
        <div class="grow"></div>
        <button class="rail-btn ${S.page === 'settings' ? 'on' : ''}" data-act="nav" data-p="settings">${ICON.gear}<span>Settings</span></button>
        <div class="who" data-act="${acc ? 'nav' : 'login'}" data-p="cosmetics">
          ${acc ? `<img class="avatar" src="https://mc-heads.net/avatar/${esc(acc.uuid)}/64" alt="" onerror="this.style.visibility='hidden'"><div><b>${esc(acc.username)}</b><small>Microsoft</small></div>`
                : `<div class="avatar ph">?</div><div><b>Guest</b><small>Sign in</small></div>`}
        </div>
      </aside>
      <div class="main">
        <header class="top"><div class="drag" data-act="drag"></div><div class="grow"></div>
          <div class="tools"><button class="ico" data-act="win-min">${ICON.min}</button><button class="ico" data-act="win-max">${ICON.max}</button><button class="ico" data-act="win-close">${ICON.close}</button></div>
        </header>
        <main class="view ${['home'].includes(S.page) ? 'flush' : ''}" id="view"></main>
      </div>
    </div>`;
  renderView();
}

function renderView() {
  const v = $('#view');
  if (!v) return;
  const pages = { home: pageHome, versions: pageVersions, browse: pageBrowse, cosmetics: pageCosmetics, skinedit: pageSkinEditor, capeedit: pageCapeEditor, perf: pagePerf, settings: pageSettings };
  v.innerHTML = pages[S.page]();
  if (typeof studioMount === 'function') studioMount();
  if (S.page === 'browse') browseLoadIfNeeded();
}

/* ------------------------------------------------------------------ home */
function launchBlock(version) {
  const L = S.launch, busy = ['preparing', 'starting', 'running'].includes(L.phase);
  if (busy) {
    const pct = Math.round(L.frac * 100);
    return `<div class="progress"><div class="bar"><i style="width:${L.phase === 'preparing' ? pct : 100}%"></i></div>
      <div class="msg">${esc(L.msg)}${L.phase === 'preparing' ? ` · ${pct}%` : ''}</div></div>`;
  }
  if (!S.st.account) {
    return `<div class="row"><button class="btn lg" data-act="login" ${S.loggingIn ? 'disabled' : ''}>${S.loggingIn ? '<span class="spin"></span> Waiting for your browser…' : 'Sign in with Microsoft'}</button></div>`;
  }
  return `<div class="row"><button class="btn lg" data-act="launch" data-v="${version}">Launch</button>
    <button class="ico" data-act="goto-settings-mc" title="Game settings">${ICON.sliders}</button>
    ${L.phase === 'error' ? `<span style="color:var(--bad);max-width:520px;font-size:13px">${esc(L.msg)}</span>` : ''}</div>`;
}

function pageHome() {
  const v = S.sel;
  return `<div class="home">
    <section class="hero">
      <div class="eyebrow">❄ FROST CLIENT · PVP EDITION</div>
      <h1>${esc(v)}</h1>
      <div class="sub">${esc(loaderOf(v))} · ${esc(VERSION_META[v]?.note || '')}</div>
      <div id="launch-block">${launchBlock(v)}</div>
    </section>
    <aside class="vlist glass">
      <h4>VERSION</h4>
      ${S.st.instances.map(i => `<button class="vrow ${S.sel === i.id ? 'on' : ''}" data-act="pick" data-v="${i.id}" data-ctx="home">
        <i style="background:${(VERSION_META[i.id] || {}).art}"></i><b>${esc(i.id)}</b><small>${esc(i.loader)}</small>${S.sel === i.id ? '<em>●</em>' : ''}</button>`).join('')}
    </aside>
  </div>`;
}

function versionCard(i, ctx) {
  const m = VERSION_META[i.id] || {};
  return `<button class="vcard ${S.sel === i.id ? 'on' : ''}" style="--art:${m.art}" data-act="pick" data-v="${i.id}" data-ctx="${ctx}">
    <span class="flake">❄</span>
    ${ctx === 'versions' && i.content ? `<span class="badge">${i.content} installed</span>` : ''}
    <b>${esc(i.id)}</b><small>${esc(i.loader)} · Frost</small></button>`;
}

function refreshLaunchBlock() {
  const el = $('#launch-block');
  if (el && S.page === 'home') el.innerHTML = launchBlock(S.sel);
}

/* ------------------------------------------------------------------ versions */
function pageVersions() {
  const i = selInst(), m = VERSION_META[S.sel] || {};
  return `<h1 class="page-title">Versions</h1><div class="page-sub">Pick a Frost version to launch.</div>
  <div class="vers">
    <div class="vgrid">${S.st.instances.map(x => versionCard(x, 'versions')).join('')}</div>
    <aside class="detail glass">
      <div class="art" style="--art:${m.art}"></div>
      <div class="body">
        <h2>${esc(S.sel)}</h2>
        <div class="muted">${esc(i.loader)} · Frost</div>
        <div class="kv">
          <div><span>Minecraft</span><b>${esc(S.sel)}</b></div>
          <div><span>Loader</span><b>${esc(i.loader)}</b></div>
          <div><span>Frost mod</span><b>${i.has_mod ? 'Bundled' : 'Missing'}</b></div>
          <div><span>Extra content</span><b>${i.content}</b></div>
          <div><span>Last played</span><b>${esc(ago(i.last_played))}</b></div>
        </div>
        <p>${esc(m.note || '')} Mods you install from Browse land in this version's own folder.</p>
        <div class="acts">
          ${S.st.account ? `<button class="btn" data-act="launch" data-v="${S.sel}">Launch</button>` : `<button class="btn" data-act="login">Sign in</button>`}
          <button class="ico" data-act="goto-browse" title="Browse mods">${ICON.search}</button>
          <button class="ico" data-act="open-inst" title="Open folder">${ICON.folder}</button>
        </div>
      </div>
    </aside>
  </div>`;
}

/* ------------------------------------------------------------------ browse (Modrinth) */
function pageBrowse() {
  const M = S.mr;
  if (!M.mc) M.mc = S.sel;
  const cats = MR_CATS[M.kind];
  return `<div class="browse">
    <aside class="side">
      <div class="seg" style="display:flex;flex-direction:column;width:100%;padding:3px">${Object.keys(KIND_LABEL).map(k =>
        `<button class="${M.kind === k ? 'on' : ''}" data-act="mr-kind" data-k="${k}" style="text-align:left">${KIND_LABEL[k]}</button>`).join('')}</div>
      <h4>CATEGORIES</h4>
      <button class="${!M.category ? 'on' : ''}" data-act="mr-cat" data-c="">All</button>
      ${cats.map(c => `<button class="${M.category === c ? 'on' : ''}" data-act="mr-cat" data-c="${c}">${esc(c.replace(/-/g, ' ').replace(/^./, x => x.toUpperCase()))}</button>`).join('')}
    </aside>
    <section>
      <h1 class="page-title">Browse</h1>
      <div class="page-sub">Content from Modrinth, installed straight into your instance.</div>
      <div class="bar-row">
        <select class="field" data-bind="mr-mc">${S.st.instances.map(i => `<option value="${i.id}" ${M.mc === i.id ? 'selected' : ''}>${i.id} · ${i.loader}</option>`).join('')}</select>
        <div class="seg"><button class="${M.tab === 'discover' ? 'on' : ''}" data-act="mr-tab" data-t="discover">Discover</button>
          <button class="${M.tab === 'installed' ? 'on' : ''}" data-act="mr-tab" data-t="installed">Installed (${M.installed.length})</button></div>
        <div style="flex:1"></div>
        ${M.tab === 'discover' ? `<input class="field" style="width:240px;user-select:text" placeholder="Search for content" value="${esc(M.query)}" data-bind="mr-q">
        <select class="field" data-bind="mr-sort">${[['relevance', 'Relevance'], ['downloads', 'Downloads'], ['follows', 'Follows'], ['newest', 'Newest'], ['updated', 'Updated']].map(([v, l]) => `<option value="${v}" ${M.sort === v ? 'selected' : ''}>${l}</option>`).join('')}</select>` : ''}
      </div>
      <div id="mr-results">${mrResults()}</div>
    </section>
  </div>`;
}

function mrResults() {
  const M = S.mr;
  if (M.tab === 'installed') {
    if (!M.installed.length) return `<div class="empty">Nothing installed for ${esc(M.mc)} yet.</div>`;
    return M.installed.map(i => `<div class="installed-row glass">
      <img src="${esc(i.icon_url || '')}" alt="" onerror="this.style.visibility='hidden'">
      <div class="grow"><b>${esc(i.title)}</b><small>${esc(i.version)} · ${esc(KIND_LABEL[i.kind] || i.kind)}${i.dependency ? ' · dependency' : ''}</small></div>
      <button class="btn danger sm" data-act="mr-remove" data-id="${esc(i.project_id)}">Remove</button></div>`).join('');
  }
  if (M.loading && !M.page) return `<div class="empty"><span class="spin"></span></div>`;
  if (M.error) return `<div class="empty">${esc(M.error)}</div>`;
  const p = M.page;
  if (!p || !p.hits.length) return `<div class="empty">No results for ${esc(M.mc)}.</div>`;
  const have = new Map(M.installed.map(i => [i.project_id, i]));
  const pages = Math.max(1, Math.ceil(Math.min(p.total_hits, 9000) / M.limit)), cur = Math.floor(M.offset / M.limit);
  const nums = [...new Set([0, cur - 2, cur - 1, cur, cur + 1, cur + 2, pages - 1])].filter(n => n >= 0 && n < pages).sort((a, b) => a - b);
  let pager = '', last = -1;
  for (const n of nums) { if (last >= 0 && n - last > 1) pager += '<button disabled>…</button>'; pager += `<button class="${n === cur ? 'on' : ''}" data-act="mr-page" data-n="${n}">${n + 1}</button>`; last = n; }
  return `<div style="color:var(--ink-3);font-size:12.5px;margin-bottom:10px">${p.total_hits.toLocaleString()} results${M.loading ? ' · loading…' : ''}</div>
  <div class="grid">${p.hits.map(h => {
    const owned = have.get(h.project_id), busy = M.busy.has(h.project_id);
    return `<article class="mod glass">
      <div class="thumb"><img src="${esc(h.icon_url || '')}" alt="" onerror="this.style.visibility='hidden'"></div>
      <div class="info"><h3>${esc(h.title)}</h3>
        <div class="meta">by ${esc(h.author)} · ${fmtNum(h.downloads)} downloads</div>
        <div class="desc">${esc(h.description)}</div>
        <div class="foot">${owned ? `<button class="btn ghost sm" disabled>Installed</button>`
          : `<button class="btn sm" data-act="mr-install" data-id="${esc(h.project_id)}" ${busy ? 'disabled' : ''}>${busy ? '<span class="spin"></span> Installing' : '+ Install'}</button>`}</div></div></article>`;
  }).join('')}</div>
  <div class="pager"><button data-act="mr-page" data-n="${cur - 1}" ${cur === 0 ? 'disabled' : ''}>‹</button>${pager}<button data-act="mr-page" data-n="${cur + 1}" ${cur >= pages - 1 ? 'disabled' : ''}>›</button></div>`;
}

function refreshMr() { const el = $('#mr-results'); if (el) el.innerHTML = mrResults(); }

async function browseLoadIfNeeded() {
  const M = S.mr;
  M.installed = await inv('mr_installed', { mc: M.mc }).catch(() => []);
  if (M.tab === 'discover' && !M.page) await mrSearch(); else refreshMr();
}

async function mrSearch() {
  const M = S.mr, token = ++M.token;
  M.loading = true; M.error = ''; refreshMr();
  try {
    const page = await inv('mr_search', { query: { query: M.query, mc_version: M.mc, kind: M.kind, category: M.category || null, sort: M.sort, offset: M.offset, limit: M.limit } });
    if (token !== M.token) return;
    M.page = page;
  } catch (e) { if (token === M.token) M.error = 'Could not reach Modrinth: ' + e; }
  if (token === M.token) { M.loading = false; refreshMr(); }
}


/* ------------------------------------------------------------------ performance */
function pagePerf() {
  const c = S.cfg, v = S.sel, P = (S.perf && S.perf[v]) || null, fabric = loaderOf(v) === 'Fabric';
  if (!P && fabric) perfLoad(v);
  const grp = (key, title, desc, mods) => {
    const on = P && P[key], busy = S.perfBusy === key;
    return setRow(title, `${desc}<br><span class="mods">${mods}</span>`,
      busy ? '<span class="spin"></span>' : (!fabric ? '<span class="pill"><i></i>Fabric only</span>'
        : `<span class="pill ${on ? 'ok' : ''}"><i></i>${on ? 'Installed' : 'Off'}</span><button class="btn ${on ? 'ghost' : ''} sm" data-act="perf-group" data-g="${key}" data-on="${on ? 0 : 1}">${on ? 'Remove' : 'Install'}</button>`));
  };
  const vers = S.st.instances.map(i => `<button class="${v === i.id ? 'on' : ''}" data-act="perf-ver" data-v="${i.id}">${esc(i.id)}</button>`).join('');
  return `<h1 class="page-title">Performance</h1>
    <div class="page-sub">More frames without touching your settings by hand. Pick a version, then switch things on.</div>
    <div class="line" style="margin-bottom:6px"><div class="seg">${vers}</div></div>
    ${setGroup('BUILT-IN',
      setRow('Auto FPS boost', 'Before each launch: fast graphics, no clouds or shadows, no vsync, render distance 8. Works on every version.', tog('auto_optimize', c.auto_optimize !== false)),
      setRow('Vulkan renderer', 'Minecraft 26.2 only. Asks the game for its experimental Vulkan backend, and falls back to OpenGL on GPUs without Vulkan.', tog('perf_vulkan', !!c.perf_vulkan)))}
    ${setGroup('FAST MODS' + (fabric ? '' : ' (FABRIC VERSIONS)'),
      grp('renderer', 'Fast renderer', 'Much faster chunk and entity drawing.', 'Sodium · ImmediatelyFast · Entity Culling'),
      grp('logic', 'Fast math &amp; logic', 'Quicker game logic, collision, lighting math and lower memory use.', 'Lithium · FerriteCore · ModernFix'))}
    ${fabric ? '' : '<p class="muted-note">1.8.9 has no Fabric mods; it gets the built-in boost and the in-game Performance tab (Right Shift).</p>'}`;
}

async function perfLoad(v) {
  S.perf = S.perf || {};
  S.perf[v] = { renderer: false, logic: false, loading: true };
  try { S.perf[v] = await inv('perf_status', { mc: v }); } catch (e) { S.perf[v] = { renderer: false, logic: false }; }
  if (S.page === 'perf') renderView();
}

/* ------------------------------------------------------------------ settings */
function setRow(title, desc, ctl) {
  return `<div class="set-row"><div class="t"><b>${title}</b><small>${desc}</small></div><div class="ctl">${ctl}</div></div>`;
}
const setGroup = (title, ...rows) => `<div class="set-sec">${title}</div><div class="set-card glass">${rows.join('')}</div>`;
const tog = (key, on) => `<div class="toggle ${on ? 'on' : ''}" data-act="cfg-toggle" data-k="${key}"></div>`;

function pageSettings() {
  const c = S.cfg, tabs = [['launcher', 'Launcher settings'], ['java', 'Java & memory'], ['storage', 'Storage'], ['accounts', 'Accounts'], ['about', 'About']];
  let body = '';
  const T = S.settingsTab;
  if (T === 'launcher') {
    body = setGroup('GENERAL',
      setRow('Falling snow', 'Little snowflakes drifting over the launcher.', tog('snow', c.snow)),
      setRow('While playing', 'What the launcher window does once the game is running.', `<select class="field" data-bind="while">${[['keep', 'Keep visible'], ['hide', 'Hide launcher']].map(([v, l]) => `<option value="${v}" ${c.while_playing === v ? 'selected' : ''}>${l}</option>`).join('')}</select>`));
  } else if (T === 'java') {
    body = setGroup('MEMORY',
      setRow('Maximum RAM', 'Heap size given to Minecraft.', `<input type="range" min="1024" max="16384" step="256" value="${c.ram_mb}" data-bind="ram"><span class="val" id="v-ram">${(c.ram_mb / 1024).toFixed(1)} GB</span>`)) +
      setGroup('JAVA',
      setRow('Java executable', 'Leave empty — Frost downloads the right Java automatically.', `<input class="field" style="width:320px;user-select:text" placeholder="automatic" value="${esc(c.java_path)}" data-bind="java">`),
      setRow('JVM arguments', 'Extra flags, separated by spaces.', `<input class="field" style="width:420px;user-select:text" value="${esc(c.jvm_args)}" data-bind="jvm">`));
  } else if (T === 'storage') {
    body = setGroup('FOLDERS AND FILES',
      setRow('Launcher folder', esc(S.st.data_dir), `<button class="btn ghost sm" data-act="open-data">${ICON.folder} Open</button>`));
  } else if (T === 'accounts') {
    const a = S.st.account;
    body = setGroup('MICROSOFT ACCOUNT',
      a ? setRow(esc(a.username), 'Signed in', `<button class="btn danger sm" data-act="logout">Sign out</button>`)
        : setRow('Not signed in', 'One click opens the official Microsoft login in your browser.', `<button class="btn sm" data-act="login">Sign in</button>`));
  } else {
    body = setGroup('FROST CLIENT',
      setRow('Version', esc(S.st.os), `<span class="val">v${esc(S.st.app_version)}</span>`)) +
      setGroup('IN-GAME MOD',
      ...S.st.instances.map(i => setRow(`${i.id} · ${i.loader}`, 'Frost HUD, trainer and tab snowflake', `<span class="pill ${i.has_mod ? 'ok' : ''}"><i></i>${i.has_mod ? 'Bundled' : 'Not bundled'}</span>`)));
  }
  return `<div class="settings">
    <aside class="side">${tabs.map(([k, l]) => `<button class="${T === k ? 'on' : ''}" data-act="set-tab" data-t="${k}">${l}</button>`).join('')}</aside>
    <section><h1 class="page-title">${esc(tabs.find(t => t[0] === T)[1])}</h1>${body}</section></div>`;
}

/* ------------------------------------------------------------------ actions */
async function doLogin() {
  if (S.loggingIn) return;
  S.loggingIn = true; renderAll();
  try {
    S.st.account = await inv('login');
    toast(`Signed in as ${S.st.account.username}`, 'ok');
  } catch (e) {
    if (String(e) === 'NO_CLIENT_ID' || String(e).startsWith('BAD_CLIENT_ID')) {
      const bad = String(e).startsWith('BAD_CLIENT_ID') ? String(e).slice(14) : '';
      modal({ title: 'Microsoft sign-in needs an application ID',
        body: (bad ? `The configured ID <b>“${esc(bad)}”</b> is not valid — an Azure application ID is a GUID like <code>3f2c1a9e-8b4d-4c1e-9a77-0123456789ab</code>, not a name.<br><br>` : 'This build has no Microsoft application ID yet.<br><br>')
          + '<b>One-time setup (free, 2 minutes):</b><br>1. Open <b>portal.azure.com → App registrations → New registration</b><br>2. Supported account types: <b>Personal Microsoft accounts only</b><br>3. Redirect URI: platform <b>Mobile and desktop applications</b>, value <code>http://localhost</code><br>4. Copy the <b>Application (client) ID</b> and paste it below in Settings → Accounts (or set it as the <code>FROST_MS_CLIENT_ID</code> repo variable before building).<br><br>Your app must also be approved for Minecraft services: <b>aka.ms/AppRegInfo</b>.',
        actions: [{ label: 'Open settings', cls: 'btn', onClick: () => { S.page = 'settings'; S.settingsTab = 'accounts'; renderAll(); } }, { label: 'Close' }] });
    } else if (String(e).includes('MC_APP_NOT_APPROVED')) {
      modal({ title: 'Minecraft has not approved this app yet',
        body: 'Microsoft login worked, but Mojang only allows approved Azure apps to sign in to Minecraft (HTTP 403).<br><br>Submit the form at <b>aka.ms/AppRegInfo</b> with your Application (client) ID. Approval can take a few days — after that, sign-in works without changing anything here.',
        actions: [{ label: 'Close' }] });
    } else toast(e, 'err');
  }
  S.loggingIn = false; renderAll();
}

function renderAll() { render(); }

const ACTIONS = {
  nav: el => { S.page = el.dataset.p; render(); },
  drag: () => {},
  'win-min': () => inv('win_minimize'),
  'win-max': () => inv('win_toggle_max'),
  'win-close': () => inv('win_close'),
  login: () => doLogin(),
  logout: async () => { await inv('logout'); S.st.account = null; render(); },
  pick: el => {
    S.sel = el.dataset.v; S.cfg.selected_version = S.sel; S.mr.mc = S.sel; S.mr.page = null; saveCfg(); renderView();
    if (S.page === 'home') renderView();
  },
  launch: async el => {
    S.launch = { phase: 'preparing', msg: 'Preparing…', frac: 0 }; renderView();
    try { await inv('launch_game', { version: el.dataset.v || S.sel }); }
    catch (e) { S.launch = { phase: 'error', msg: String(e), frac: 0 }; if (String(e) === 'NO_CLIENT_ID' || String(e).startsWith('BAD_CLIENT_ID')) { S.launch = { phase: 'idle', msg: '', frac: 0 }; doLogin(); } renderView(); }
  },
  'goto-settings-mc': () => { S.page = 'settings'; S.settingsTab = 'java'; render(); },
  'goto-browse': () => { S.page = 'browse'; S.mr.mc = S.sel; S.mr.page = null; render(); },
  'perf-ver': el => { S.sel = el.dataset.v; renderView(); },
  'perf-group': async el => {
    const g = el.dataset.g, on = el.dataset.on === '1';
    S.perfBusy = g; renderView();
    try {
      const r = await inv('perf_group', { mc: S.sel, group: g, on });
      const bad = r.filter(x => !x.ok).map(x => x.mod);
      toast(bad.length ? `No build for ${S.sel}: ${bad.join(', ')}` : (on ? 'Installed' : 'Removed'), bad.length === r.length && r.length ? 'err' : 'ok');
    } catch (e) { toast(e, 'err'); }
    S.perfBusy = null; await perfLoad(S.sel);
  },
  'open-inst': () => inv('open_instance_folder', { mc: S.sel }).catch(e => toast(e, 'err')),
  'open-data': () => inv('open_data_folder').catch(e => toast(e, 'err')),
  /* browse */
  'mr-kind': el => { Object.assign(S.mr, { kind: el.dataset.k, category: '', offset: 0, page: null, tab: 'discover' }); render(); },
  'mr-cat': el => { Object.assign(S.mr, { category: el.dataset.c, offset: 0, page: null }); render(); },
  'mr-tab': el => { S.mr.tab = el.dataset.t; render(); },
  'mr-page': el => { S.mr.offset = Math.max(0, +el.dataset.n) * S.mr.limit; mrSearch(); },
  'mr-install': async el => {
    const id = el.dataset.id, M = S.mr;
    M.busy.add(id); refreshMr();
    try {
      const added = await inv('mr_install', { projectId: id, mc: M.mc });
      toast(`Installed ${added.map(a => a.title).join(', ')}`, 'ok');
      M.installed = await inv('mr_installed', { mc: M.mc });
      S.st.instances.find(i => i.id === M.mc).content = M.installed.length;
    } catch (e) { toast(e, 'err'); }
    M.busy.delete(id); refreshMr();
  },
  'mr-remove': async el => {
    try { await inv('mr_remove', { mc: S.mr.mc, projectId: el.dataset.id }); S.mr.installed = await inv('mr_installed', { mc: S.mr.mc }); render(); }
    catch (e) { toast(e, 'err'); }
  },
  /* settings */
  'set-tab': el => { S.settingsTab = el.dataset.t; renderView(); },
  'cfg-toggle': el => { const k = el.dataset.k; S.cfg[k] = !S.cfg[k]; saveCfg(); snowSet(); renderView(); },
};

async function bridge(cmd, args, okMsg) {
  try { (await inv(cmd, args)) ? toast(okMsg, 'ok') : toast('Start the game and open a world first.', 'err'); }
  catch (e) { toast(e, 'err'); }
}

document.addEventListener('click', e => {
  const el = e.target.closest('[data-act]');
  if (!el || el.dataset.act === 'drag') return;
  const fn = ACTIONS[el.dataset.act] || (typeof STUDIO_ACTIONS !== 'undefined' ? STUDIO_ACTIONS[el.dataset.act] : null);
  if (fn) fn(el, e);
});
document.addEventListener('mousedown', e => { if (e.target.closest('[data-act="drag"]') && e.button === 0) inv('win_drag'); });
document.addEventListener('dblclick', e => { if (e.target.closest('[data-act="drag"]')) inv('win_toggle_max'); });

let qTimer = null;
function onBind(e) {
  const el = e.target.closest('[data-bind]'); if (!el) return;
  const v = el.value, c = S.cfg;
  if (typeof studioBind === 'function' && studioBind(el, v)) return;
  switch (el.dataset.bind) {
    case 'mr-mc': S.mr.mc = v; S.mr.page = null; render(); break;
    case 'mr-q': clearTimeout(qTimer); qTimer = setTimeout(() => { S.mr.query = v; S.mr.offset = 0; mrSearch(); }, 350); break;
    case 'mr-sort': S.mr.sort = v; S.mr.offset = 0; mrSearch(); break;
    case 'zoom-fov': c.qol.zoom_fov = +v; $('#v-zoom').textContent = v + '°'; saveCfg(); break;
    case 'while': c.while_playing = v; saveCfg(); break;
    case 'ram': c.ram_mb = +v; $('#v-ram').textContent = (v / 1024).toFixed(1) + ' GB'; saveCfg(); break;
    case 'java': c.java_path = v; saveCfg(); break;
    case 'jvm': c.jvm_args = v; saveCfg(); break;
    case 'cid': c.ms_client_id = v.trim(); saveCfg(); break;
  }
}
document.addEventListener('input', onBind);
document.addEventListener('change', onBind);

/* ------------------------------------------------------------------ snow */
let snowOn = true;
function snowSet() { snowOn = !!(S.cfg && S.cfg.snow); }
(function snow() {
  const cv = $('#snow'), g = cv.getContext('2d');
  let W = 0, H = 0, flakes = [], last = 0;
  const fit = () => { const d = Math.min(window.devicePixelRatio || 1, 2); W = cv.clientWidth; H = cv.clientHeight; cv.width = W * d; cv.height = H * d; g.setTransform(d, 0, 0, d, 0, 0); };
  const mk = top => ({ x: Math.random() * W, y: top ? -10 : Math.random() * H, r: .8 + Math.random() * 2.4, v: .25 + Math.random() * .7, ph: Math.random() * 6.28, sw: .3 + Math.random() * .8 });
  fit(); flakes = Array.from({ length: 70 }, () => mk(false));
  window.addEventListener('resize', fit);
  function star(x, y, r) {
    g.beginPath();
    for (let i = 0; i < 3; i++) { const a = i * Math.PI / 3, dx = Math.cos(a) * r, dy = Math.sin(a) * r; g.moveTo(x - dx, y - dy); g.lineTo(x + dx, y + dy); }
    g.stroke();
  }
  function frame(t) {
    requestAnimationFrame(frame);
    if (t - last < 33 || document.hidden) return;
    const dt = Math.min((t - last) / 16, 3); last = t;
    g.clearRect(0, 0, W, H);
    if (!snowOn) return;
    g.strokeStyle = 'rgba(205,230,255,.8)'; g.fillStyle = 'rgba(215,236,255,.75)'; g.lineWidth = .8;
    for (const f of flakes) {
      f.y += f.v * dt; f.ph += .012 * dt; f.x += Math.sin(f.ph) * f.sw * .5 * dt;
      if (f.y > H + 10) Object.assign(f, mk(true));
      if (f.r > 1.9) star(f.x, f.y, f.r * 1.5); else { g.beginPath(); g.arc(f.x, f.y, f.r * .7, 0, 6.283); g.fill(); }
    }
  }
  requestAnimationFrame(frame);
})();

/* ------------------------------------------------------------------ boot */
async function boot() {
  S.st = await inv('get_state');
  S.cfg = S.st.settings;
  S.sel = S.st.instances.some(i => i.id === S.cfg.selected_version) ? S.cfg.selected_version : S.st.instances[0].id;
  S.ipc = S.st.ipc_connected;
  snowSet(); render();
  const L = TAURI.event.listen;
  L('launch-status', e => {
    S.launch = { phase: e.payload.phase, msg: e.payload.message, frac: S.launch.frac };
    if (e.payload.phase === 'error') toast(e.payload.message, 'err');
    refreshLaunchBlock(); if (S.page === 'versions') renderView();
    inv('get_state').then(s => { S.st.instances = s.instances; }).catch(() => {});
  });
  L('launch-progress', e => { S.launch.frac = e.payload.fraction; refreshLaunchBlock(); });
  L('settings-changed', async () => { const st = await inv('get_state'); S.cfg = st.settings; });
  L('launch-warning', e => toast(e.payload, 'err'));
  L('ipc-connected', e => { S.ipc = e.payload; });
}
boot().catch(e => { document.body.insertAdjacentHTML('beforeend', `<pre style="position:fixed;left:20px;top:90px;color:#ff9aa8;z-index:99">${esc(e)}</pre>`); });
