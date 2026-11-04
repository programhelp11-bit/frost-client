/* Skin editor, cape editor and the community cape gallery. Uses globals from app.js (S, inv, toast, esc, renderView). */
'use strict';

const mkCanvas = (w, h) => { const c = document.createElement('canvas'); c.width = w; c.height = h; return c; };
const SWATCHES = ['#ffffff', '#eaf3ff', '#8fd3ff', '#5aafff', '#3d6bff', '#1e4eaa', '#0e204e', '#6a4dff', '#b86cff', '#ff6b82', '#ffc85a', '#4be0a8', '#8a5a2b', '#2b2b33', '#000000'];

const stu = {
  color: '#8fd3ff',
  skin: { buf: mkCanvas(64, 64), name: 'My skin', slim: false, tool: 'pen', size: 1, guide: true, hist: [], fut: [] },
  cape: { buf: mkCanvas(10, 16), name: 'My cape', tool: 'pen', size: 1, hist: [], fut: [] },
  lib: [], libLoaded: false, thumbs: {},
  gal: { page: null, offset: 0, sort: 'new', loading: false, error: '', favs: [], active: 'frost', loaded: false },
};
const EDS = { skin: stu.skin, cape: stu.cape };
const SCALE = { skin: 8, cape: 20 };

/* ---------------------------------------------------------------- drawing core */
function edCtx(ed) { return ed.buf.getContext('2d', { willReadFrequently: true }); }

function snapshot(ed) {
  ed.hist.push(edCtx(ed).getImageData(0, 0, ed.buf.width, ed.buf.height));
  if (ed.hist.length > 40) ed.hist.shift();
  ed.fut = [];
}

function stamp(ed, x, y, erase) {
  const c = edCtx(ed), s = ed.size, o = s >> 1;
  const px = x - o, py = y - o;
  if (erase) c.clearRect(px, py, s, s);
  else { c.fillStyle = stu.color; c.clearRect(px, py, s, s); c.fillRect(px, py, s, s); }
}

function line(ed, x0, y0, x1, y1, erase) {
  const dx = Math.abs(x1 - x0), dy = Math.abs(y1 - y0), sx = x0 < x1 ? 1 : -1, sy = y0 < y1 ? 1 : -1;
  let err = dx - dy;
  for (;;) {
    stamp(ed, x0, y0, erase);
    if (x0 === x1 && y0 === y1) break;
    const e2 = 2 * err;
    if (e2 > -dy) { err -= dy; x0 += sx; }
    if (e2 < dx) { err += dx; y0 += sy; }
  }
}

function hexToRgb(h) { const n = parseInt(h.slice(1), 16); return [(n >> 16) & 255, (n >> 8) & 255, n & 255]; }
const rgbToHex = (r, g, b) => '#' + [r, g, b].map(v => v.toString(16).padStart(2, '0')).join('');

function flood(ed, x, y) {
  const w = ed.buf.width, h = ed.buf.height, c = edCtx(ed), img = c.getImageData(0, 0, w, h), d = img.data;
  const at = (px, py) => (py * w + px) * 4;
  const i0 = at(x, y), t = [d[i0], d[i0 + 1], d[i0 + 2], d[i0 + 3]];
  const [r, g, b] = hexToRgb(stu.color);
  if (t[0] === r && t[1] === g && t[2] === b && t[3] === 255) return;
  const same = i => d[i] === t[0] && d[i + 1] === t[1] && d[i + 2] === t[2] && d[i + 3] === t[3];
  const stack = [[x, y]];
  while (stack.length) {
    const [cx, cy] = stack.pop();
    if (cx < 0 || cy < 0 || cx >= w || cy >= h) continue;
    const i = at(cx, cy);
    if (!same(i)) continue;
    d[i] = r; d[i + 1] = g; d[i + 2] = b; d[i + 3] = 255;
    stack.push([cx + 1, cy], [cx - 1, cy], [cx, cy + 1], [cx, cy - 1]);
  }
  c.putImageData(img, 0, 0);
}

function pickColor(ed, x, y) {
  const p = edCtx(ed).getImageData(x, y, 1, 1).data;
  if (p[3] > 0) { stu.color = rgbToHex(p[0], p[1], p[2]); const el = document.querySelector('input[data-bind="st-color"]'); if (el) el.value = stu.color; }
}

/* UV outlines of the main skin faces (base layer; overlay is +32 x on the head, +16 y on the body) */
const SKIN_FACES = [
  [8, 0, 8, 8, 'head top'], [16, 0, 8, 8, ''], [0, 8, 8, 8, ''], [8, 8, 8, 8, 'face'], [16, 8, 8, 8, ''], [24, 8, 8, 8, 'back'],
  [20, 20, 8, 12, 'chest'], [32, 20, 8, 12, 'back'], [44, 20, 4, 12, 'R arm'], [36, 52, 4, 12, 'L arm'], [4, 20, 4, 12, 'R leg'], [20, 52, 4, 12, 'L leg'],
  [40, 8, 8, 8, 'hat'], [20, 36, 8, 12, 'jacket'],
];

function drawEditor(kind) {
  const ed = EDS[kind], cv = document.getElementById('ed-' + kind);
  if (!cv) return;
  const sc = SCALE[kind], w = ed.buf.width, h = ed.buf.height;
  const g = cv.getContext('2d');
  g.imageSmoothingEnabled = false;
  g.clearRect(0, 0, cv.width, cv.height);
  // transparency checkerboard
  const cs = sc / 2;
  for (let y = 0; y < h * 2; y++) for (let x = 0; x < w * 2; x++) {
    g.fillStyle = (x + y) % 2 ? '#1a2a52' : '#13204a';
    g.fillRect(x * cs, y * cs, cs, cs);
  }
  g.drawImage(ed.buf, 0, 0, w * sc, h * sc);
  g.strokeStyle = 'rgba(255,255,255,.07)';
  g.lineWidth = 1;
  g.beginPath();
  for (let x = 0; x <= w; x++) { g.moveTo(x * sc + .5, 0); g.lineTo(x * sc + .5, h * sc); }
  for (let y = 0; y <= h; y++) { g.moveTo(0, y * sc + .5); g.lineTo(w * sc, y * sc + .5); }
  g.stroke();
  if (kind === 'skin' && ed.guide) {
    g.strokeStyle = 'rgba(143,211,255,.55)';
    g.fillStyle = 'rgba(143,211,255,.8)';
    g.font = '10px sans-serif';
    for (const [x, y, fw, fh, label] of SKIN_FACES) {
      g.strokeRect(x * sc + .5, y * sc + .5, fw * sc, fh * sc);
      if (label) g.fillText(label, x * sc + 3, y * sc + 11);
    }
  }
  drawPreviews(kind);
}

/* ---------------------------------------------------------------- character preview */
function skinFigure(cv, tex, slim, back) {
  const g = cv.getContext('2d');
  g.imageSmoothingEnabled = false;
  g.clearRect(0, 0, cv.width, cv.height);
  const s = Math.floor(cv.height / 32), ox = Math.floor((cv.width - 16 * s) / 2);
  const aw = slim ? 3 : 4;
  const put = (sx, sy, w, h, dx, dy) => g.drawImage(tex, sx, sy, w, h, ox + dx * s, dy * s, w * s, h * s);
  const both = (base, over, w, h, dx, dy) => { put(base[0], base[1], w, h, dx, dy); put(over[0], over[1], w, h, dx, dy); };
  if (!back) {
    both([8, 8], [40, 8], 8, 8, 4, 0);
    both([20, 20], [20, 36], 8, 12, 4, 8);
    both([44, 20], [44, 36], aw, 12, 4 - aw, 8);       // right arm (viewer's left)
    both([36, 52], [52, 52], aw, 12, 12, 8);           // left arm
    both([4, 20], [4, 36], 4, 12, 4, 20);
    both([20, 52], [4, 52], 4, 12, 8, 20);
  } else {
    both([24, 8], [56, 8], 8, 8, 4, 0);
    both([32, 20], [32, 36], 8, 12, 4, 8);
    both([36 + aw + 4, 52], [52 + aw + 4, 52], aw, 12, 4 - aw, 8);   // left arm seen from behind (viewer's left)
    both([44 + aw + 4, 20], [44 + aw + 4, 36], aw, 12, 12, 8);       // right arm
    both([28, 52], [12, 52], 4, 12, 4, 20);
    both([12, 20], [12, 36], 4, 12, 8, 20);
  }
}

function capeFigure(cv, face) {
  const g = cv.getContext('2d');
  g.imageSmoothingEnabled = false;
  g.clearRect(0, 0, cv.width, cv.height);
  const s = Math.floor(cv.height / 32), ox = Math.floor((cv.width - 16 * s) / 2);
  const R = (c, x, y, w, h) => { g.fillStyle = c; g.fillRect(ox + x * s, y * s, w * s, h * s); };
  R('#8a8fa6', 4, 0, 8, 8); R('#6f7592', 4, 8, 8, 12); R('#8a8fa6', 0, 8, 4, 12); R('#8a8fa6', 12, 8, 4, 12); R('#4d5370', 4, 20, 4, 12); R('#4d5370', 8, 20, 4, 12);
  g.drawImage(face, 0, 0, 10, 16, ox + 3 * s, 8 * s, 10 * s, 16 * s);
}

function drawPreviews(kind) {
  if (kind === 'skin') {
    const a = document.getElementById('pv-front'), b = document.getElementById('pv-back');
    if (a) skinFigure(a, stu.skin.buf, stu.skin.slim, false);
    if (b) skinFigure(b, stu.skin.buf, stu.skin.slim, true);
  } else {
    const a = document.getElementById('pv-cape');
    if (a) capeFigure(a, stu.cape.buf);
  }
}

/* ---------------------------------------------------------------- cape atlas */
function capeAtlas() {
  const out = mkCanvas(64, 32), g = out.getContext('2d');
  g.imageSmoothingEnabled = false;
  const face = stu.cape.buf;
  g.drawImage(face, 1, 1);                                  // outside (visible from behind)
  g.drawImage(face, 0, 0, 10, 16, 12, 1, 10, 16);           // inside, darkened below
  g.fillStyle = 'rgba(5,10,23,.5)'; g.fillRect(12, 1, 10, 16);
  g.drawImage(face, 0, 0, 1, 16, 0, 1, 1, 16);              // left edge
  g.drawImage(face, 9, 0, 1, 16, 11, 1, 1, 16);             // right edge
  g.drawImage(face, 0, 0, 10, 1, 1, 0, 10, 1);              // top
  g.drawImage(face, 0, 15, 10, 1, 11, 0, 10, 1);            // bottom
  return out;
}
const capeB64 = () => capeAtlas().toDataURL('image/png').split(',')[1];
const skinB64 = () => stu.skin.buf.toDataURL('image/png').split(',')[1];

/* ---------------------------------------------------------------- image helpers */
function loadImg(src) {
  return new Promise((res, rej) => { const i = new Image(); i.onload = () => res(i); i.onerror = () => rej(new Error('Could not read that image')); i.src = src; });
}
const b64Img = b64 => loadImg('data:image/png;base64,' + b64);

async function importInto(kind, src) {
  const img = await loadImg(src), ed = EDS[kind];
  snapshot(ed);
  const c = edCtx(ed);
  c.clearRect(0, 0, ed.buf.width, ed.buf.height);
  if (kind === 'skin') {
    if (img.width % 64 !== 0 || ![img.width, img.width / 2].includes(img.height)) throw new Error('Skins are 64×64 (or 64×32) PNGs');
    c.drawImage(img, 0, 0, img.width, img.height, 0, 0, 64, img.height * 64 / img.width);
  } else if ((img.width === 64 && img.height === 32) || (img.width === 22 && img.height === 17)) {
    c.drawImage(img, 1, 1, 10, 16, 0, 0, 10, 16);
  } else {
    c.imageSmoothingEnabled = false;
    c.drawImage(img, 0, 0, img.width, img.height, 0, 0, 10, 16);
  }
  drawEditor(kind);
}

function pickFile(kind) {
  const inp = document.createElement('input');
  inp.type = 'file'; inp.accept = 'image/png';
  inp.onchange = () => {
    const f = inp.files[0]; if (!f) return;
    const r = new FileReader();
    r.onload = () => importInto(kind, r.result).catch(e => toast(e.message || e, 'err'));
    r.readAsDataURL(f);
  };
  inp.click();
}

/* ---------------------------------------------------------------- page markup */
function toolbar(kind) {
  const ed = EDS[kind];
  const tool = (t, label) => `<button class="${ed.tool === t ? 'on' : ''}" data-act="st-tool" data-ed="${kind}" data-t="${t}">${label}</button>`;
  const size = n => `<button class="${ed.size === n ? 'on' : ''}" data-act="st-size" data-ed="${kind}" data-n="${n}">${n}px</button>`;
  return `<div class="tb">
    <div class="seg">${tool('pen', 'Pencil')}${tool('erase', 'Eraser')}${tool('fill', 'Fill')}${tool('pick', 'Picker')}</div>
    <div class="seg">${size(1)}${size(2)}${size(3)}</div>
    <input type="color" value="${stu.color}" data-bind="st-color" title="Colour">
    <div class="seg"><button data-act="st-undo" data-ed="${kind}">Undo</button><button data-act="st-redo" data-ed="${kind}">Redo</button></div>
  </div>
  <div class="sw">${SWATCHES.map(c => `<button style="background:${c}" data-act="st-swatch" data-c="${c}" title="${c}"></button>`).join('')}</div>`;
}

function libItems(kinds) {
  const items = stu.lib.filter(i => kinds.includes(i.kind));
  if (!items.length) return '<div class="muted-note">Nothing saved yet.</div>';
  return items.map(i => `<div class="lib-item">
      <canvas width="${i.kind === 'skin' ? 32 : 20}" height="32" data-thumb="${i.id}" data-kind="${i.kind}"></canvas>
      <div class="nm"><b>${esc(i.name)}</b><small>${i.kind === 'skin' ? (i.slim ? 'Slim' : 'Classic') + ' skin' : i.kind === 'fav' ? 'Cape' : 'Cape'}</small></div>
      <div class="acts">
        ${i.kind === 'skin' ? `<button class="btn ghost sm" data-act="st-lib-apply" data-id="${i.id}">Wear</button>` : `<button class="btn ghost sm" data-act="ct-wear-lib" data-id="${i.id}">Wear</button>`}
        ${i.kind !== 'fav' ? `<button class="btn ghost sm" data-act="st-lib-edit" data-id="${i.id}" data-kind="${i.kind}">Edit</button>` : ''}
        <button class="btn ghost sm danger" data-act="st-lib-del" data-id="${i.id}">✕</button>
      </div></div>`).join('');
}

function needAccount(title) {
  return `<h1 class="page-title">${title}</h1><div class="empty">Sign in with Microsoft to use this.<br><br><button class="btn" data-act="login">Sign in with Microsoft</button></div>`;
}

function pageSkinEditor() {
  if (!S.st.account) return needAccount('Skin editor');
  const ed = stu.skin;
  return `<h1 class="page-title">Skin editor</h1><div class="page-sub">Draw a skin, save it to your library and wear it everywhere — it is changed on your Minecraft account itself.</div>
  <div class="studio">
    <section class="card glass ed">
      ${toolbar('skin')}
      <div class="ed-wrap"><canvas id="ed-skin" width="${64 * SCALE.skin}" height="${64 * SCALE.skin}"></canvas></div>
      <div class="line"><label class="chk"><input type="checkbox" data-bind="st-guide" ${ed.guide ? 'checked' : ''}> Show guide</label><div class="grow"></div>
        <button class="btn ghost sm" data-act="st-import" data-ed="skin">Import PNG</button>
        <button class="btn ghost sm" data-act="st-load-skin">Load my current skin</button>
        <button class="btn ghost sm" data-act="st-clear" data-ed="skin">Clear</button></div>
    </section>
    <aside class="card glass side">
      <h3>Preview</h3>
      <div class="pv"><canvas id="pv-front" width="128" height="256"></canvas><canvas id="pv-back" width="128" height="256"></canvas></div>
      <input class="field" data-bind="st-name-skin" value="${esc(ed.name)}" maxlength="32" placeholder="Skin name">
      <div class="line"><div class="seg"><button class="${!ed.slim ? 'on' : ''}" data-act="st-variant" data-s="0">Classic</button><button class="${ed.slim ? 'on' : ''}" data-act="st-variant" data-s="1">Slim</button></div></div>
      <div class="line"><button class="btn ghost" data-act="st-save-skin">Save to library</button><button class="btn" data-act="st-apply-skin">Wear this skin</button></div>
      <h3 style="margin-top:18px">Skin library <span class="tag">R in game</span></h3>
      <div class="lib" id="lib-skins">${libItems(['skin'])}</div>
    </aside>
  </div>`;
}

function pageCapeEditor() {
  if (!S.st.account) return needAccount('Cape editor');
  const ed = stu.cape;
  return `<h1 class="page-title">Cape editor</h1><div class="page-sub">Draw the back of your cape. Save it to wear it right away: you see it in game, and other Frost Client players see it too when a Frost server is set up.</div>
  <div class="studio">
    <section class="card glass ed">
      ${toolbar('cape')}
      <div class="ed-wrap cape"><canvas id="ed-cape" width="${10 * SCALE.cape}" height="${16 * SCALE.cape}"></canvas></div>
      <div class="line"><div class="seg"><button data-act="ct-preset" data-p="frost">Frost</button><button data-act="ct-preset" data-p="aurora">Aurora</button><button data-act="ct-preset" data-p="ember">Ember</button></div><div class="grow"></div>
        <button class="btn ghost sm" data-act="st-import" data-ed="cape">Import PNG</button>
        <button class="btn ghost sm" data-act="st-clear" data-ed="cape">Clear</button></div>
    </section>
    <aside class="card glass side">
      <h3>Preview</h3>
      <div class="pv"><canvas id="pv-cape" width="128" height="256"></canvas></div>
      <input class="field" data-bind="st-name-cape" value="${esc(ed.name)}" maxlength="24" placeholder="Cape name (3-24 characters)">
      <div class="line"><button class="btn ghost" data-act="st-save-cape">Save</button><button class="btn" data-act="ct-wear">Save &amp; wear</button></div>
      
      <h3 style="margin-top:18px">My capes</h3>
      <div class="lib" id="lib-capes">${libItems(['cape'])}</div>
    </aside>
  </div>`;
}

/* ---------------------------------------------------------------- Frost cape */
function pageCosmetics() {
  if (!S.st.account) return needAccount('Frost cape');
  const on = stu.gal.active !== 'none';
  return `<h1 class="page-title">Frost cape</h1>
  <div class="page-sub">Every Frost Client player gets the Frost cape for free. Only players using Frost Client can see it.</div>
  <div class="ccard glass ${on ? 'worn' : ''}" style="max-width:260px">
    <div class="cthumb" style="background-image:url('frost-cape.png')"></div>
    <b>Frost cape</b><small>Free for everyone</small>
    <div class="acts"><button class="btn ${on ? 'ghost' : ''} sm" data-act="cape-toggle">${on ? 'Wearing - take off' : 'Wear'}</button></div>
  </div>`;
}

/* ---------------------------------------------------------------- library */
async function libLoad() {
  try { stu.lib = await inv('lib_list'); } catch (e) { stu.lib = []; }
  stu.libLoaded = true;
  refreshLib();
}

async function refreshLib() {
  for (const [id, kinds] of [['lib-skins', ['skin']], ['lib-capes', ['cape', 'fav']]]) {
    const el = document.getElementById(id);
    if (el) el.innerHTML = libItems(kinds);
  }
  for (const cv of document.querySelectorAll('canvas[data-thumb]')) {
    const id = cv.dataset.thumb, kind = cv.dataset.kind;
    try {
      let b64 = stu.thumbs[id];
      if (!b64) { b64 = await inv('lib_png', { id }); stu.thumbs[id] = b64; }
      const img = await b64Img(b64), g = cv.getContext('2d');
      g.imageSmoothingEnabled = false;
      g.clearRect(0, 0, cv.width, cv.height);
      if (kind === 'skin') { g.drawImage(img, 8, 8, 8, 8, 0, 0, 32, 32); g.drawImage(img, 40, 8, 8, 8, 0, 0, 32, 32); }
      else g.drawImage(img, 1, 1, 10, 16, 0, 0, 20, 32);
    } catch (e) { /* thumbnail is optional */ }
  }
}

/* ---------------------------------------------------------------- mounting + pointer input */
let stroke = null;

function studioMount() {
  if (!S.st || !S.st.account) return;
  if (S.page === 'skinedit' || S.page === 'capeedit') {
    const kind = S.page === 'skinedit' ? 'skin' : 'cape';
    const cv = document.getElementById('ed-' + kind);
    if (cv && !cv._wired) {
      cv._wired = true;
      cv.addEventListener('contextmenu', e => e.preventDefault());
      cv.addEventListener('pointerdown', e => edDown(kind, cv, e));
      cv.addEventListener('pointermove', e => edMove(kind, cv, e));
      cv.addEventListener('pointerup', () => { stroke = null; });
      cv.addEventListener('pointerleave', () => { stroke = null; });
    }
    drawEditor(kind);
    libLoad();
  }
}

function cell(kind, cv, e) {
  const r = cv.getBoundingClientRect(), ed = EDS[kind];
  const x = Math.floor((e.clientX - r.left) / r.width * ed.buf.width);
  const y = Math.floor((e.clientY - r.top) / r.height * ed.buf.height);
  return [Math.max(0, Math.min(ed.buf.width - 1, x)), Math.max(0, Math.min(ed.buf.height - 1, y))];
}

function edDown(kind, cv, e) {
  e.preventDefault();
  cv.setPointerCapture(e.pointerId);
  const ed = EDS[kind], [x, y] = cell(kind, cv, e), erase = e.button === 2 || ed.tool === 'erase';
  if (ed.tool === 'pick') { pickColor(ed, x, y); ed.tool = 'pen'; renderView(); return; }
  snapshot(ed);
  if (ed.tool === 'fill' && e.button !== 2) { flood(ed, x, y); drawEditor(kind); return; }
  stamp(ed, x, y, erase);
  stroke = { x, y, erase };
  drawEditor(kind);
}

function edMove(kind, cv, e) {
  if (!stroke || !(e.buttons & 3)) return;
  const [x, y] = cell(kind, cv, e);
  if (x === stroke.x && y === stroke.y) return;
  line(EDS[kind], stroke.x, stroke.y, x, y, stroke.erase);
  stroke.x = x; stroke.y = y;
  drawEditor(kind);
}

function studioBind(el, v) {
  switch (el.dataset.bind) {
    case 'st-color': stu.color = v; return true;
    case 'st-guide': stu.skin.guide = el.checked; drawEditor('skin'); return true;
    case 'st-name-skin': stu.skin.name = v; return true;
    case 'st-name-cape': stu.cape.name = v; return true;
  }
  return false;
}

/* ---------------------------------------------------------------- presets */
function applyPreset(name) {
  const ed = stu.cape; snapshot(ed);
  const g = edCtx(ed), W = 10, H = 16;
  const stops = { frost: ['#1e4eaa', '#8fd3ff'], aurora: ['#3d6bff', '#4be0a8'], ember: ['#8a1f3a', '#ffc85a'] }[name];
  const grad = g.createLinearGradient(0, 0, 0, H);
  grad.addColorStop(0, stops[0]); grad.addColorStop(1, stops[1]);
  g.clearRect(0, 0, W, H); g.fillStyle = grad; g.fillRect(0, 0, W, H);
  g.fillStyle = 'rgba(255,255,255,.85)';
  for (const [x, y] of [[4, 5], [5, 5], [3, 6], [4, 6], [5, 6], [6, 6], [4, 7], [5, 7], [4, 4], [5, 8]]) g.fillRect(x, y, 1, 1);
  g.fillStyle = 'rgba(255,255,255,.55)';
  for (let x = 0; x < W; x += 2) g.fillRect(x, H - 1, 1, 1);
  drawEditor('cape');
}

/* ---------------------------------------------------------------- actions */
const STUDIO_ACTIONS = {
  'st-tool': el => { EDS[el.dataset.ed].tool = el.dataset.t; renderView(); },
  'st-size': el => { EDS[el.dataset.ed].size = +el.dataset.n; renderView(); },
  'st-swatch': el => { stu.color = el.dataset.c; const i = document.querySelector('input[data-bind="st-color"]'); if (i) i.value = stu.color; },
  'st-undo': el => { const ed = EDS[el.dataset.ed], s = ed.hist.pop(); if (!s) return; ed.fut.push(edCtx(ed).getImageData(0, 0, ed.buf.width, ed.buf.height)); edCtx(ed).putImageData(s, 0, 0); drawEditor(el.dataset.ed); },
  'st-redo': el => { const ed = EDS[el.dataset.ed], s = ed.fut.pop(); if (!s) return; ed.hist.push(edCtx(ed).getImageData(0, 0, ed.buf.width, ed.buf.height)); edCtx(ed).putImageData(s, 0, 0); drawEditor(el.dataset.ed); },
  'st-clear': el => { const ed = EDS[el.dataset.ed]; snapshot(ed); edCtx(ed).clearRect(0, 0, ed.buf.width, ed.buf.height); drawEditor(el.dataset.ed); },
  'st-import': el => pickFile(el.dataset.ed),
  'st-variant': el => { stu.skin.slim = el.dataset.s === '1'; renderView(); },
  'st-load-skin': async () => {
    try {
      const r = await inv('skin_current');
      stu.skin.slim = r.slim;
      await importInto('skin', 'data:image/png;base64,' + r.png_b64);
      renderView();
    } catch (e) { toast(e, 'err'); }
  },
  'st-save-skin': async () => {
    try {
      await inv('lib_save', { kind: 'skin', name: stu.skin.name || 'My skin', pngB64: skinB64(), slim: stu.skin.slim });
      toast('Saved to your library — it is in the R wheel in game', 'ok'); await libLoad();
    } catch (e) { toast(e, 'err'); }
  },
  'st-apply-skin': async () => {
    try {
      await inv('upload_skin', { pngB64: skinB64(), slim: stu.skin.slim });
      toast('Skin updated on your account', 'ok');
    } catch (e) { toast(e, 'err'); }
  },
  'st-save-cape': async () => {
    try {
      await inv('lib_save', { kind: 'cape', name: stu.cape.name || 'My cape', pngB64: capeB64(), slim: false });
      toast('Cape saved', 'ok'); await libLoad();
    } catch (e) { toast(e, 'err'); }
  },
  'ct-preset': el => applyPreset(el.dataset.p),
  'ct-wear': async () => {
    try {
      const id = await inv('lib_save', { kind: 'cape', name: stu.cape.name || 'My cape', pngB64: capeB64(), slim: false });
      await inv('cape_select', { id: typeof id === 'string' ? id : (id && id.id) });
      stu.gal.active = typeof id === 'string' ? id : (id && id.id);
      toast('You are wearing your cape now', 'ok'); await libLoad();
    } catch (e) { toast(e, 'err'); }
  },
  'ct-wear-lib': async el => {
    try { await inv('cape_select', { id: el.dataset.id }); stu.gal.active = el.dataset.id; toast('Cape changed', 'ok'); renderView(); }
    catch (e) { toast(e, 'err'); }
  },
  'st-lib-edit': async el => {
    try {
      const b64 = await inv('lib_png', { id: el.dataset.id }), kind = el.dataset.kind === 'skin' ? 'skin' : 'cape';
      const item = stu.lib.find(i => i.id === el.dataset.id);
      if (kind === 'skin' && item) { stu.skin.slim = item.slim; stu.skin.name = item.name; }
      if (kind === 'cape' && item) stu.cape.name = item.name;
      await importInto(kind, 'data:image/png;base64,' + b64);
      renderView();
    } catch (e) { toast(e, 'err'); }
  },
  'st-lib-apply': async el => {
    try {
      const b64 = await inv('lib_png', { id: el.dataset.id }), item = stu.lib.find(i => i.id === el.dataset.id);
      await inv('upload_skin', { pngB64: b64, slim: !!(item && item.slim) });
      toast('Skin updated on your account', 'ok');
    } catch (e) { toast(e, 'err'); }
  },
  'st-lib-del': async el => {
    try { await inv('lib_delete', { id: el.dataset.id }); delete stu.thumbs[el.dataset.id]; await libLoad(); } catch (e) { toast(e, 'err'); }
  },
  'cape-toggle': async () => {
    const next = stu.gal.active === 'none' ? 'frost' : 'none';
    try { await inv('cape_select', { id: next }); stu.gal.active = next; toast(next === 'none' ? 'Cape taken off' : 'Frost cape on', 'ok'); renderView(); }
    catch (e) { toast(e, 'err'); }
  },
};
