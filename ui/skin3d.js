/* Software 3D renderer + painter for the skin editor (no libraries, works offline).
   Draws the Minecraft player model with both skin layers, and maps every screen pixel back to a
   texel so you can paint straight on the model, like NovaSkin. */
'use strict';

const Skin3D = (() => {
  const W = 360, H = 440;
  const view = { yaw: 0.55, pitch: 0.14, zoom: 1, mirror: false, layer: 0, overlay: true };
  let faces = [], builtFor = '';
  let pickCache = null, pickKey = '';

  const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
  const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
  const add3 = (a, b) => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
  const mul = (a, k) => [a[0] * k, a[1] * k, a[2] * k];

  /* ---------- model ---------- */
  function boxes(slim) {
    const aw = slim ? 3 : 4;
    return [
      { x0: -4, x1: 4, y0: 24, y1: 32, z0: -4, z1: 4, tu: 0, tv: 0, w: 8, h: 8, d: 8, k: 0, layer: 0 },
      { x0: -4, x1: 4, y0: 24, y1: 32, z0: -4, z1: 4, tu: 32, tv: 0, w: 8, h: 8, d: 8, k: 0.5, layer: 1 },
      { x0: -4, x1: 4, y0: 12, y1: 24, z0: -2, z1: 2, tu: 16, tv: 16, w: 8, h: 12, d: 4, k: 0, layer: 0 },
      { x0: -4, x1: 4, y0: 12, y1: 24, z0: -2, z1: 2, tu: 16, tv: 32, w: 8, h: 12, d: 4, k: 0.25, layer: 1 },
      { x0: -4 - aw, x1: -4, y0: 12, y1: 24, z0: -2, z1: 2, tu: 40, tv: 16, w: aw, h: 12, d: 4, k: 0, layer: 0 },
      { x0: -4 - aw, x1: -4, y0: 12, y1: 24, z0: -2, z1: 2, tu: 40, tv: 32, w: aw, h: 12, d: 4, k: 0.25, layer: 1 },
      { x0: 4, x1: 4 + aw, y0: 12, y1: 24, z0: -2, z1: 2, tu: 32, tv: 48, w: aw, h: 12, d: 4, k: 0, layer: 0 },
      { x0: 4, x1: 4 + aw, y0: 12, y1: 24, z0: -2, z1: 2, tu: 48, tv: 48, w: aw, h: 12, d: 4, k: 0.25, layer: 1 },
      { x0: -4, x1: 0, y0: 0, y1: 12, z0: -2, z1: 2, tu: 0, tv: 16, w: 4, h: 12, d: 4, k: 0, layer: 0 },
      { x0: -4, x1: 0, y0: 0, y1: 12, z0: -2, z1: 2, tu: 0, tv: 32, w: 4, h: 12, d: 4, k: 0.25, layer: 1 },
      { x0: 0, x1: 4, y0: 0, y1: 12, z0: -2, z1: 2, tu: 16, tv: 48, w: 4, h: 12, d: 4, k: 0, layer: 0 },
      { x0: 0, x1: 4, y0: 0, y1: 12, z0: -2, z1: 2, tu: 0, tv: 48, w: 4, h: 12, d: 4, k: 0.25, layer: 1 },
    ];
  }

  function build(slim) {
    const key = slim ? 'slim' : 'classic';
    if (builtFor === key) return;
    builtFor = key;
    faces = [];
    for (const b of boxes(slim)) {
      const x0 = b.x0 - b.k, x1 = b.x1 + b.k, y0 = b.y0 - b.k, y1 = b.y1 + b.k, z0 = b.z0 - b.k, z1 = b.z1 + b.k;
      const Wd = x1 - x0, Hd = y1 - y0, Dd = z1 - z0;
      const F = (name, O, A, B, n, tx, ty, tw, th, shade) => faces.push({ name, O, A, B, n, tx, ty, tw, th, shade, layer: b.layer });
      F('front', [x0, y1, z1], [Wd, 0, 0], [0, -Hd, 0], [0, 0, 1], b.tu + b.d, b.tv + b.d, b.w, b.h, 0.92);
      F('back', [x1, y1, z0], [-Wd, 0, 0], [0, -Hd, 0], [0, 0, -1], b.tu + 2 * b.d + b.w, b.tv + b.d, b.w, b.h, 0.74);
      F('right', [x0, y1, z0], [0, 0, Dd], [0, -Hd, 0], [-1, 0, 0], b.tu, b.tv + b.d, b.d, b.h, 0.82);
      F('left', [x1, y1, z1], [0, 0, -Dd], [0, -Hd, 0], [1, 0, 0], b.tu + b.d + b.w, b.tv + b.d, b.d, b.h, 0.82);
      F('top', [x0, y1, z0], [Wd, 0, 0], [0, 0, Dd], [0, 1, 0], b.tu + b.d, b.tv, b.w, b.d, 1.0);
      F('bottom', [x0, y0, z1], [Wd, 0, 0], [0, 0, -Dd], [0, -1, 0], b.tu + b.d + b.w, b.tv, b.w, b.d, 0.58);
    }
    pickKey = '';
  }

  /* ---------- camera ---------- */
  function camera() {
    const cy = Math.cos(view.yaw), sy = Math.sin(view.yaw), cp = Math.cos(view.pitch), sp = Math.sin(view.pitch);
    const dist = 74, f = 800 * view.zoom;
    const rot = p => {                       // model space -> camera space (before perspective)
      const x = p[0], y = p[1] - 16, z = p[2];
      const x1 = x * cy + z * sy, z1 = -x * sy + z * cy;
      return [x1, y * cp - z1 * sp, y * sp + z1 * cp];
    };
    const rotN = n => { const x1 = n[0] * cy + n[2] * sy, z1 = -n[0] * sy + n[2] * cy; return [x1, n[1] * cp - z1 * sp, n[1] * sp + z1 * cp]; };
    const proj = r => { const zc = dist - r[2]; return [W / 2 + f * r[0] / zc, H * 0.52 - f * r[1] / zc, zc]; };
    return { rot, rotN, proj, dist };
  }

  /* ---------- rasteriser ---------- */
  // mode 'draw': shaded colour into `img`; mode 'pick': face id and (s,t) per pixel for one layer.
  function raster(mode, tex, layerOnly) {
    const cam = camera();
    const depth = new Float32Array(W * H).fill(Infinity);
    const img = mode === 'draw' ? new ImageData(W, H) : null;
    const fid = mode === 'pick' ? new Int16Array(W * H).fill(-1) : null;
    const ST = mode === 'pick' ? new Float32Array(W * H * 2) : null;
    for (let fi = 0; fi < faces.length; fi++) {
      const fc = faces[fi];
      if (mode === 'draw' && fc.layer === 1 && !view.overlay) continue;
      if (mode === 'pick' && fc.layer !== layerOnly) continue;
      const center = add3(fc.O, mul(add3(fc.A, fc.B), 0.5));
      const rc = cam.rot(center), rn = cam.rotN(fc.n);
      if (dot(rn, [-rc[0], -rc[1], cam.dist - rc[2]]) <= 0) continue;      // back face
      const P = [fc.O, add3(fc.O, fc.A), add3(add3(fc.O, fc.A), fc.B), add3(fc.O, fc.B)].map(p => cam.proj(cam.rot(p)));
      const ST4 = [[0, 0], [1, 0], [1, 1], [0, 1]];
      for (const tri of [[0, 1, 2], [0, 2, 3]]) {
        const a = P[tri[0]], b = P[tri[1]], c = P[tri[2]];
        const area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        if (Math.abs(area) < 1e-6) continue;
        const minX = Math.max(0, Math.floor(Math.min(a[0], b[0], c[0]))), maxX = Math.min(W - 1, Math.ceil(Math.max(a[0], b[0], c[0])));
        const minY = Math.max(0, Math.floor(Math.min(a[1], b[1], c[1]))), maxY = Math.min(H - 1, Math.ceil(Math.max(a[1], b[1], c[1])));
        const iw0 = 1 / a[2], iw1 = 1 / b[2], iw2 = 1 / c[2];
        const s0 = ST4[tri[0]], s1 = ST4[tri[1]], s2 = ST4[tri[2]];
        for (let y = minY; y <= maxY; y++) {
          for (let x = minX; x <= maxX; x++) {
            const px = x + 0.5, py = y + 0.5;
            const l0 = ((b[0] - px) * (c[1] - py) - (b[1] - py) * (c[0] - px)) / area;
            const l1 = ((c[0] - px) * (a[1] - py) - (c[1] - py) * (a[0] - px)) / area;
            const l2 = 1 - l0 - l1;
            if (l0 < -1e-4 || l1 < -1e-4 || l2 < -1e-4) continue;
            const iw = l0 * iw0 + l1 * iw1 + l2 * iw2, zc = 1 / iw, idx = y * W + x;
            if (zc >= depth[idx]) continue;
            const s = (l0 * s0[0] * iw0 + l1 * s1[0] * iw1 + l2 * s2[0] * iw2) * zc;
            const t = (l0 * s0[1] * iw0 + l1 * s1[1] * iw1 + l2 * s2[1] * iw2) * zc;
            if (mode === 'pick') { depth[idx] = zc; fid[idx] = fi; ST[idx * 2] = s; ST[idx * 2 + 1] = t; continue; }
            const u = fc.tx + Math.min(fc.tw - 1, Math.max(0, Math.floor(s * fc.tw)));
            const v = fc.ty + Math.min(fc.th - 1, Math.max(0, Math.floor(t * fc.th)));
            const ti = (v * 64 + u) * 4;
            if (tex[ti + 3] < 8) continue;                                   // transparent texel
            depth[idx] = zc;
            const o = idx * 4, sh = fc.shade, al = tex[ti + 3] / 255;
            img.data[o] = tex[ti] * sh; img.data[o + 1] = tex[ti + 1] * sh; img.data[o + 2] = tex[ti + 2] * sh; img.data[o + 3] = 255 * (al > 0.5 ? 1 : al * 2);
          }
        }
      }
    }
    return { img, fid, ST };
  }

  /* ---------- public ---------- */
  let canvas = null;

  function attach(cv) { canvas = cv; build(stu.skin.slim); redraw(); }

  function redraw() {
    if (!canvas) return;
    build(stu.skin.slim);
    const tex = stu.skin.buf.getContext('2d', { willReadFrequently: true }).getImageData(0, 0, 64, 64).data;
    const { img } = raster('draw', tex);
    const g = canvas.getContext('2d');
    g.clearRect(0, 0, W, H);
    // soft floor shadow
    g.fillStyle = 'rgba(0,0,0,.28)';
    g.beginPath(); g.ellipse(W / 2, H * 0.52 + 16 * 800 * view.zoom / 74 * 0.98, 70 * view.zoom, 11 * view.zoom, 0, 0, 7); g.fill();
    g.putImageData(img, 0, 0);
  }

  function pickBuffers() {
    build(stu.skin.slim);
    const key = [view.yaw, view.pitch, view.zoom, view.layer, builtFor].join('|');
    if (key !== pickKey || !pickCache) { pickCache = raster('pick', null, view.layer); pickKey = key; }
    return pickCache;
  }

  function hit(px, py) {
    const { fid, ST } = pickBuffers();
    const x = Math.floor(px), y = Math.floor(py);
    if (x < 0 || y < 0 || x >= W || y >= H) return null;
    const i = y * W + x;
    if (fid[i] < 0) return null;
    return { face: fid[i], s: ST[i * 2], t: ST[i * 2 + 1] };
  }

  const texel = (f, s, t) => [f.tx + Math.min(f.tw - 1, Math.max(0, Math.floor(s * f.tw))), f.ty + Math.min(f.th - 1, Math.max(0, Math.floor(t * f.th)))];

  /** Texels touched when painting at a hit (the point itself, plus its mirror image). */
  function texelsFor(h) {
    const f = faces[h.face], out = [texel(f, h.s, h.t)];
    if (view.mirror) {
      const P = add3(f.O, add3(mul(f.A, h.s), mul(f.B, h.t))), M = [-P[0], P[1], P[2]], nM = [-f.n[0], f.n[1], f.n[2]];
      for (const g of faces) {
        if (g.layer !== f.layer || dot(g.n, nM) < 0.99) continue;
        const d = sub(M, g.O);
        if (Math.abs(dot(d, g.n)) > 0.03) continue;
        const s = dot(d, g.A) / dot(g.A, g.A), t = dot(d, g.B) / dot(g.B, g.B);
        if (s < -0.002 || s > 1.002 || t < -0.002 || t > 1.002) continue;
        const m = texel(g, s, t);
        if (m[0] !== out[0][0] || m[1] !== out[0][1]) out.push(m);
        break;
      }
    }
    return out;
  }

  function toCanvas(e) {
    const r = canvas.getBoundingClientRect();
    return [(e.clientX - r.left) / r.width * W, (e.clientY - r.top) / r.height * H];
  }

  return { view, attach, redraw, hit, texelsFor, toCanvas, W, H, invalidate() { pickKey = ''; } };
})();
