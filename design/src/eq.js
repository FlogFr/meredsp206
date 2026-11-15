// Shared helpers for mockups: RBJ biquad magnitude and a log-frequency EQ plot.
const FS = 96000;
function biquad(type, f0, gainDb, q) {
  const A = Math.pow(10, gainDb / 40), w0 = 2 * Math.PI * f0 / FS;
  const c = Math.cos(w0), s = Math.sin(w0), al = s / (2 * q);
  let b, a;
  if (type === 'peak') { b = [1 + al * A, -2 * c, 1 - al * A]; a = [1 + al / A, -2 * c, 1 - al / A]; }
  else if (type === 'lowshelf' || type === 'highshelf') {
    const sq = 2 * Math.sqrt(A) * al, sign = type === 'lowshelf' ? 1 : -1;
    b = [A * ((A + 1) - sign * (A - 1) * c + sq), sign * 2 * A * ((A - 1) - sign * (A + 1) * c), A * ((A + 1) - sign * (A - 1) * c - sq)];
    a = [(A + 1) + sign * (A - 1) * c + sq, -sign * 2 * ((A - 1) + sign * (A + 1) * c), (A + 1) + sign * (A - 1) * c - sq];
  }
  return f => {
    const w = 2 * Math.PI * f / FS;
    const re = (k) => k[0] + k[1] * Math.cos(w) + k[2] * Math.cos(2 * w);
    const im = (k) => -(k[1] * Math.sin(w) + k[2] * Math.sin(2 * w));
    const nb = Math.hypot(re(b), im(b)), na = Math.hypot(re(a), im(a));
    return 20 * Math.log10(nb / na);
  };
}
// Linkwitz-Riley 24 dB/oct magnitude in dB (squared 2nd-order Butterworth)
const lr24hp = fc => f => -20 * Math.log10(1 + Math.pow(fc / f, 4));
const lr24lp = fc => f => -20 * Math.log10(1 + Math.pow(f / fc, 4));

function eqPlot(svg, opts) {
  const { w, h, bands, hp, lp, range = 15 } = opts;
  const ns = 'http://www.w3.org/2000/svg';
  const fx = f => (Math.log10(f / 20) / 3) * w;
  const dy = d => h / 2 - d / range * (h / 2 - 8);
  const add = (tag, attrs, text) => { const e = document.createElementNS(ns, tag); for (const k in attrs) e.setAttribute(k, attrs[k]); if (text) e.textContent = text; svg.appendChild(e); return e; };
  svg.setAttribute('width', w); svg.setAttribute('height', h);
  add('rect', { x: 0, y: 0, width: w, height: h, fill: '#00000040', rx: 8 });
  [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000].forEach(f => {
    add('line', { x1: fx(f), x2: fx(f), y1: 0, y2: h, stroke: '#ffffff14' });
    add('text', { x: fx(f) + 4, y: h - 6, fill: '#ffffff55', 'font-size': 10.5 }, f >= 1000 ? (f / 1000) + 'k' : f);
  });
  for (let d = -range + 3; d <= range - 3; d += 6) {
    add('line', { x1: 0, x2: w, y1: dy(d), y2: dy(d), stroke: d === 0 ? '#ffffff30' : '#ffffff10' });
    add('text', { x: 6, y: dy(d) - 3, fill: '#ffffff55', 'font-size': 10.5 }, (d > 0 ? '+' : '') + d);
  }
  const fns = bands.filter(b => !b.bypass).map(b => biquad(b.type, b.f, b.g, b.q));
  if (hp) fns.push(lr24hp(hp)); if (lp) fns.push(lr24lp(lp));
  if (hp) add('rect', { x: 0, y: 0, width: fx(hp), height: h, fill: '#78aeed10' });
  if (lp) add('rect', { x: fx(lp), y: 0, width: w - fx(lp), height: h, fill: '#78aeed10' });
  bands.forEach(b => {
    if (b.bypass) return;
    const fn = biquad(b.type, b.f, b.g, b.q); let p = '';
    for (let i = 0; i <= 300; i++) { const f = 20 * Math.pow(1000, i / 300); p += (i ? 'L' : 'M') + fx(f).toFixed(1) + ',' + dy(fn(f)).toFixed(1); }
    add('path', { d: p, fill: 'none', stroke: '#ffffff30', 'stroke-width': 1 });
  });
  let p = '', area = '';
  for (let i = 0; i <= 600; i++) {
    const f = 20 * Math.pow(1000, i / 600); const d = Math.max(-range, fns.reduce((s, fn) => s + fn(f), 0));
    p += (i ? 'L' : 'M') + fx(f).toFixed(1) + ',' + dy(d).toFixed(1);
  }
  add('path', { d: p + `L${w},${dy(0)}L0,${dy(0)}Z`, fill: '#3584e426', stroke: 'none' });
  add('path', { d: p, fill: 'none', stroke: '#78aeed', 'stroke-width': 2.5 });
  bands.forEach((b, i) => {
    const x = fx(b.f), y = dy(b.g);
    add('circle', { cx: x, cy: y, r: b.sel ? 12 : 10, fill: b.bypass ? '#c88800' : (b.sel ? '#ffffff' : '#3584e4'), stroke: '#0008', 'stroke-width': 1 });
    add('text', { x: x, y: y + 4, fill: b.sel ? '#1d1d20' : '#fff', 'font-size': 11.5, 'font-weight': 700, 'text-anchor': 'middle' }, i + 1);
  });
}
