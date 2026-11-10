// The map lab's module: one module, mounted once as the lab at the head of the page and once in
// every technique card. It runs the deterministic core in WebAssembly (lab/core in this page's
// repository), so a recipe builds the same map here, on the command line and in an engine.
//
//   mount(root, { params, fallback, report })     the site's contract, awaited
//
// params.mode is "lab" (the pipeline can be edited, measured, walked and shared) or "card" (the
// pipeline is one of the technique's demos). The module holds no wording of its own about the
// techniques: each step's text is read from the page's step lists by its key. The names of
// stages, traits and failures and the talk's demos are written into DATA by the page's build,
// from the same files the page's tables are made from. The controls are made from the core's
// stage table, and the caches, the 200-seed summary and the walk are the core's.
import init, { Lab, failure_key, note_key, stage_table } from './map_lab_wasm.js';

let ready; // one WebAssembly instance for every mount on the page
let table; // the core's stage table: every stage and its parameters, by stage name
let theLab; // the page's lab, once it has mounted
const cards = []; // each mounted card: its root and its load()
let pending; // a recipe sent to a card that hasn't mounted yet: its figure and the recipe

// What the page calls things, and the recipes it can load by name. The build replaces this line
// (build.mjs, from page/gen.mjs): traits [{ key, label, digits, to, extra }], fails { key: label },
// groups [{ title, stages: [{ name, title }] }], demos [{ id, label, stage, recipe, own }].
const DATA = { traits: [], fails: {}, groups: [], demos: [] };

const PX = 8; // canvas pixels a tile; CSS scales the canvas down to its column
const SEEDS = 200;
export const NEW_TINT = 0.6; // how strongly a step's new tiles are tinted; the build's figures use it too
const TRAITS = ['floor_share', 'regions', 'dead_ends', 'corridor_ratio', 'path', 'probed', 'water', 'grass', 'mountain', 'field_mean', 'field_sd', 'rooms'];
const WALKABLE = [0, 1, 0, 1, 0, 1, 1]; // by tile: wall, floor, water, grass, mountain, treasure, trap
const MARKS = { 1: 'new', 2: 'rejected', 3: 'point', 4: 'start', 5: 'exit', 6: 'path', 7: 'hot path', 8: 'unreachable', 9: 'on the route', 10: 'off the route', 11: 'target', 32: 'rooms or cells' };

function h(tag, props = {}, ...kids) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (k === 'class') el.className = v;
    else if (k.startsWith('on')) el.addEventListener(k.slice(2), v);
    else if (v === true) el.setAttribute(k, '');
    else if (v !== false && v != null) el.setAttribute(k, v);
  }
  for (const kid of kids.flat(Infinity)) if (kid != null && kid !== false) el.append(kid);
  return el;
}
const copy = (o) => JSON.parse(JSON.stringify(o));
const clean = (r) => ({ format: 'map-lab-recipe/1', seed: r.seed, width: r.width ?? table.recipe.width, height: r.height ?? table.recipe.height, pipeline: r.pipeline });
const text = (r) => JSON.stringify(clean(r));
// A recipe in the URL's fragment: #recipe= and its JSON, base64url-encoded.
const enc = (r) => btoa(String.fromCharCode(...new TextEncoder().encode(text(r)))).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
function fromHash(hash) {
  const m = /^#recipe=(.+)$/.exec(hash);
  try {
    return m && JSON.parse(new TextDecoder().decode(Uint8Array.from(atob(m[1].replace(/-/g, '+').replace(/_/g, '/')), (c) => c.charCodeAt(0))));
  } catch {
    return null;
  }
}
const count = (n) => n.toLocaleString('en-US');

/** Every stage of a pipeline in the core's order: a combine, then each of its inputs' stages. */
function flat(pipeline, base = [], out = []) {
  pipeline.forEach((s, i) => {
    const path = [...base, i];
    out.push({ s, path });
    if (s.inputs) s.inputs.forEach((p, j) => flat(p, [...path, j], out));
  });
  return out;
}

/** The drawing's colors, from the site's tokens: floor is the figure pair's light and wall its dark, in both themes. */
function palette(el) {
  const probe = h('span');
  el.append(probe);
  const get = (v) => ((probe.style.color = v), getComputedStyle(probe).color);
  const tint = (c, pct) => get(`color-mix(in oklab, var(--color-${c}) ${pct}%, var(--figure-light))`);
  const ramp = (n, f) => Array.from({ length: n + 1 }, (_, i) => get(f(i / n)));
  const dist = ramp(16, (t) => `color-mix(in oklab, var(--color-blue) ${Math.round(t * 75)}%, var(--figure-light))`);
  const field = ramp(32, (t) => `color-mix(in oklab, var(--figure-light) ${Math.round(t * 100)}%, var(--figure-dark))`);
  const c = {
    tiles: [get('var(--figure-dark)'), get('var(--figure-light)'), get('var(--color-blue)'), tint('green', 65), get('color-mix(in oklab, var(--color-muted) 70%, var(--figure-dark))'), get('var(--color-yellow)'), get('var(--color-red)')],
    marks: { 1: get('var(--color-primary)'), 2: get('var(--color-red)'), 3: get('var(--color-magenta)'), 4: get('var(--color-green)'), 5: get('var(--color-yellow)'), 6: get('var(--color-primary)'), 7: tint('yellow', 55), 8: tint('red', 65), 9: tint('green', 60), 10: tint('muted', 40), 11: get('var(--color-magenta)') },
    groups: ['blue', 'green', 'yellow', 'magenta', 'cyan', 'red'].map((n) => tint(n, 75)),
    fog: get('color-mix(in oklab, var(--figure-dark) 60%, var(--color-surface))'),
    focus: get('var(--color-focus)'),
    primary: get('var(--color-primary)'),
    green: get('var(--color-green)'),
    yellow: get('var(--color-yellow)'),
    dist: (t) => dist[Math.round(Math.max(0, Math.min(1, t)) * 16)],
    field: (v) => field[Math.round((v / 255) * 32)],
  };
  probe.remove();
  return c;
}

export async function mount(root, { params, fallback, report }) {
  ready ??= init().then(() => (table = Object.assign(JSON.parse(stage_table()), { by: {} })) && table.stages.forEach((s) => (table.by[s.name] = s)));
  await ready;
  const isLab = params.mode === 'lab';
  // The lab starts from any of the talk's demos; a card has its technique's, by id.
  const demos = isLab ? DATA.demos.filter((d) => !d.own) : params.demos.map((id) => DATA.demos.find((d) => d.id === id));
  const names = { traits: DATA.traits, fails: DATA.fails, titles: {} };
  for (const g of DATA.groups) for (const s of g.stages) names.titles[s.name] = s.title;
  const st = { recipe: null, lab: null, frame: 0, sel: 0, demo: 0, timer: 0, walking: false, summary: null };
  let C = palette(root);

  // ---------------------------------------------------------------- skeleton
  const btn = (label, fn, cls = 'q', title) => h('button', { type: 'button', class: cls, onclick: fn, title, 'aria-label': title }, label);
  const canvas = h('canvas', { role: 'img', 'aria-label': 'The map' });
  const ctx = canvas.getContext('2d');
  const legend = h('ul', { class: 'mlab-legend' });
  const scrub = h('input', { type: 'range', min: '0', value: '0', 'aria-label': 'Step', oninput: () => go(+scrub.value) });
  const pipe = h('ol', { class: 'mlab-pipe', 'aria-label': 'The pipeline' });
  const errorBox = h('p', { class: 'mlab-error', role: 'alert', hidden: true });
  const stepLabel = h('span', { class: 'mlab-n' });
  const stepText = h('span');
  const stepBox = h('p', { class: 'mlab-step', 'aria-live': 'polite' }, stepLabel, stepText);
  const playBtn = btn('Play', () => (st.timer ? stop() : play()), '');
  const transport = h('div', { class: 'mlab-transport', role: 'group', 'aria-label': 'Step through the build' },
    btn('⏮', () => go(0), 'q', 'First step'), btn('◀', () => go(st.frame - 1), 'q', 'Step back'), btn('▶', () => go(st.frame + 1), 'q', 'Step forward'), btn('⏭', () => go(Infinity), 'q', 'The finished map'), playBtn);
  const stepArea = h('div', { class: 'mlab-steps' }, stepBox, transport, scrub);
  const stagePanel = h('div', { class: 'mlab-stage' });
  const walkBox = h('div', { class: 'mlab-walk', hidden: true });
  const readout = h('dl', { class: 'mlab-readout' });
  const seedIn = h('input', { type: 'number', min: '0', max: String(table.recipe.maxSeed), step: '1', 'aria-label': 'Seed', onchange: () => ((st.recipe.seed = Math.max(0, Math.floor(+seedIn.value || 0))), rebuild()) });
  // A recipe is a seed plus a pipeline, so the seed is the strip's first link.
  const seedChip = h('li', { class: 'mlab-seed' }, h('span', { class: 'mlab-name' }, 'seed'), seedIn, btn('New', () => ((st.recipe.seed = Math.floor(Math.random() * 100000)), rebuild()), 'q', 'A new random seed'));
  const walkBtn = btn('Walk this map', () => startWalk(), '');
  const fixBtn = btn('Add cull → start → exit', addMissing, '');
  const walkWhy = h('span', { class: 'mlab-note' });
  const measureBtn = btn(`Measure over ${SEEDS} seeds`, measure, 'q');
  const presets = isLab && h('select', { 'aria-label': 'Start from one of the talk’s demos', onchange: () => load(demos[+presets.value].recipe) },
    h('option', { value: '', disabled: true, hidden: true }, 'your own pipeline'),
    DATA.groups.map((g) => h('optgroup', { label: g.title }, demos.map((d, i) => g.stages.some((s) => s.name === d.stage) && h('option', { value: i }, `${d.id.split('-')[0]} ${d.label}`)))));
  const demoBtns = !isLab && demos.length > 1 && h('div', { class: 'mlab-demos', role: 'group', 'aria-label': 'The talk’s demos of this technique' },
    demos.map((d, i) => h('button', { type: 'button', class: 'q', 'aria-pressed': 'false', onclick: () => ((st.demo = i), load(d.recipe)) }, d.label)));
  const shareNote = h('span', { class: 'mlab-note', role: 'status' });
  const openLink = !isLab && h('a', { href: '#recipe=' }, 'Open in the lab');
  const recipeText = h('textarea', { spellcheck: 'false', rows: '8', 'aria-label': 'The recipe, as JSON' });
  const cliLine = h('code', {}, 'map-lab build recipe.json');
  const fit = h('div', { class: 'mlab-fit', 'data-fit': true }, demoBtns, pipe, errorBox, h('div', { class: 'mlab-view' }, canvas, legend), stepArea, stagePanel, walkBox);
  const limitProbe = h('div', { class: 'mlab-limit' });
  root.append(h('div', { class: 'mlab' }, limitProbe, fit, readout,
    h('div', { class: 'mlab-foot' },
      isLab && [walkBtn, fixBtn, walkWhy, measureBtn],
      h('span', { class: 'mlab-spacer' }),
      isLab ? [h('label', {}, 'Start from ', presets), btn('Copy link', share), shareNote] : openLink),
    isLab && h('details', { class: 'mlab-recipe' }, h('summary', {}, 'Recipe'), recipeText,
      h('div', { class: 'mlab-foot' }, btn('Apply', () => { try { load(JSON.parse(recipeText.value)); } catch (e) { showError(`recipe: ${e.message}`); } }, ''), cliLine,
        h('span', { class: 'mlab-note' }, 'Save it as recipe.json: the same recipe builds the same map on the command line.')))));

  // ---------------------------------------------------------------- recipe and build
  function showError(msg) {
    errorBox.textContent = msg;
    errorBox.hidden = !msg;
  }
  function load(recipe) {
    st.recipe = copy(clean(recipe));
    const stages = flat(st.recipe.pipeline);
    st.sel = isLab ? stages.length - 1 : Math.max(0, stages.findIndex((f) => f.s.stage === params.stage));
    rebuild();
  }
  function rebuild() {
    stop();
    leaveWalk();
    seedIn.value = st.recipe.seed;
    recipeText.value = JSON.stringify(clean(st.recipe), null, 1);
    renderPipe();
    renderStage();
    let lab;
    try {
      lab = new Lab(text(st.recipe));
    } catch (e) {
      // The parser says where the recipe is wrong; the last good map stays.
      showError(e.message);
      return fitToFrame();
    }
    showError('');
    st.lab?.free();
    st.lab = lab;
    st.summary = null;
    measureBtn.hidden = false;
    measureBtn.disabled = false;
    measureBtn.textContent = `Measure over ${SEEDS} seeds`;
    // Which of the talk's demos this pipeline is, if any: a card's marks and a demo's own step texts go by it.
    const pipeline = JSON.stringify(st.recipe.pipeline), same = demos.findIndex((d) => JSON.stringify(d.recipe.pipeline) === pipeline);
    if (!isLab && same >= 0) st.demo = same;
    st.demoId = isLab ? demos[same]?.id : demos[st.demo]?.id;
    st.w = lab.width();
    st.h = lab.height();
    st.n = lab.frame_count();
    st.stageOf = Uint16Array.from({ length: st.n }, (_, i) => lab.frame_stage(i));
    st.final = lab.tiles();
    st.path = lab.path();
    st.cur = -1;
    canvas.width = st.w * PX;
    canvas.height = st.h * PX;
    scrub.max = st.n - 1;
    go(st.n - 1);
    renderReadout();
    if (isLab) {
      renderWalkButton();
      const mine = enc(st.recipe);
      presets.value = same < 0 ? '' : same;
      cliLine.textContent = 'map-lab build recipe.json';
      // The address bar holds the recipe whenever it isn't the page's opening one, so a reload restores it.
      const hash = mine === enc(params.recipe) ? '' : `#recipe=${mine}`;
      if (hash || location.hash.startsWith('#recipe=')) history.replaceState(null, '', hash || location.pathname + location.search);
    } else {
      openLink.href = `#recipe=${enc(st.recipe)}`;
      demoBtns && [...demoBtns.children].forEach((b, i) => b.setAttribute('aria-pressed', String(i === st.demo)));
    }
    fitToFrame();
  }
  /** The drawing and its controls stay within the block size the site allows, by drawing smaller. */
  function fitToFrame() {
    canvas.style.maxBlockSize = '';
    const over = fit.getBoundingClientRect().height - limitProbe.getBoundingClientRect().height;
    if (over > 0) canvas.style.maxBlockSize = `${Math.max(96, canvas.getBoundingClientRect().height - over)}px`;
  }
  let width = 0;
  new ResizeObserver(([e]) => {
    const w = Math.round(e.contentRect.width);
    if (w !== width && ((width = w), st.lab)) fitToFrame();
  }).observe(root);

  function gridAt(i) {
    if (st.cur < 0 || i < st.cur) (st.grid = new Uint8Array(st.w * st.h)), (st.cur = -1);
    // Each change packs its cell and the tile it becomes: cell << 8 | tile.
    while (st.cur < i) for (const c of st.lab.frame_changes(++st.cur)) st.grid[c >>> 8] = c & 0xff;
    return st.grid;
  }
  /** A step's text, written once in the page: in this figure's own step list, or else in the technique's card. */
  function written(key) {
    const find = (k) => ((k = `.step[data-key="${k}"] p:not(.step-label)`), fallback.querySelector(k) ?? document.querySelector(k));
    // A demo's own text for a kind of step, where the page has one, or else the kind's.
    return (st.demoId && find(`${key}#${st.demoId}`)) ?? find(key);
  }
  function go(i) {
    if (!st.lab) return;
    st.frame = Math.max(0, Math.min(st.n - 1, i));
    scrub.value = st.frame;
    const stage = st.stageOf[st.frame];
    stepLabel.textContent = `Step ${count(st.frame + 1)} of ${count(st.n)} · ${st.lab.stage_name(stage)} `;
    const p = written(note_key(st.lab.frame_note(st.frame)));
    stepText.replaceChildren(...(p ? p.cloneNode(true).childNodes : []));
    for (const v of stepText.querySelectorAll('.step-value')) v.textContent = count(st.lab.frame_arg(st.frame));
    // Each chip's bar shows how far the build is through that stage.
    const done = new Map(), all = new Map();
    st.stageOf.forEach((k, n) => (all.set(k, (all.get(k) ?? 0) + 1), n <= st.frame && done.set(k, (done.get(k) ?? 0) + 1)));
    for (const chip of pipe.querySelectorAll('.mlab-chip')) {
      const k = +chip.dataset.i;
      chip.querySelector('i').style.setProperty('--p', all.has(k) ? (done.get(k) ?? 0) / all.get(k) : k < stage || st.frame === st.n - 1 ? 1 : 0);
    }
    canvas.setAttribute('aria-label', `The map at step ${st.frame + 1} of ${st.n}`);
    draw();
    if (fit.getBoundingClientRect().height > limitProbe.getBoundingClientRect().height + 0.5) fitToFrame();
  }
  function play() {
    if (st.frame >= st.n - 1) go(0);
    const stride = Math.max(1, Math.ceil(st.n / 150));
    playBtn.textContent = 'Pause';
    stepBox.setAttribute('aria-live', 'off');
    st.timer = setInterval(() => (st.frame >= st.n - 1 ? stop() : go(st.frame + stride)), 70);
  }
  function stop() {
    clearInterval(st.timer);
    st.timer = 0;
    playBtn.textContent = 'Play';
    stepBox.setAttribute('aria-live', 'polite');
  }

  // ---------------------------------------------------------------- drawing
  function cell(i, color, inset = 0) {
    ctx.fillStyle = color;
    ctx.fillRect((i % st.w) * PX + inset, ((i / st.w) | 0) * PX + inset, PX - 2 * inset, PX - 2 * inset);
  }
  function draw() {
    if (!st.lab || st.walking) return;
    const g = gridAt(st.frame), field = st.lab.frame_field(st.frame), used = new Set();
    for (let i = 0; i < g.length; i++) cell(i, field.length ? C.field(field[i]) : C.tiles[g[i]]);
    // Each mark packs its cell and its kind: cell << 8 | mark. Marks from 32 up tell rooms or cells apart.
    for (const m of st.lab.frame_marks(st.frame)) {
      const k = m & 0xff;
      // What a step has just made is tinted, not covered, so the tiles it made still show.
      ctx.globalAlpha = k === 1 ? NEW_TINT : 1;
      cell(m >>> 8, k >= 32 ? C.groups[(k - 32) % C.groups.length] : C.marks[k]);
      used.add(k >= 32 ? 32 : k);
    }
    ctx.globalAlpha = 1;
    if (st.frame === st.n - 1 && st.lab.start() >= 0) {
      for (const i of st.path) cell(i, C.marks[6], 2);
      cell(st.lab.start(), C.marks[4]);
      if (st.lab.exit() >= 0) cell(st.lab.exit(), C.marks[5]);
      [4, ...(st.lab.exit() >= 0 ? [5] : []), ...(st.path.length ? [6] : [])].forEach((m) => used.add(m));
    }
    legend.replaceChildren(...[...used].sort((a, b) => a - b).map((m) => h('li', {}, h('i', { style: `background:${m === 32 ? C.groups[0] : C.marks[m]}` }), MARKS[m])));
  }
  matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => ((C = palette(root)), st.walking ? drawWalk() : draw()));

  // ---------------------------------------------------------------- the pipeline strip
  const shown = (stage) => table.by[stage].params.filter((p) => p.kind !== 'inputs' && !(p.kind === 'pick' && p.options.length < 2));
  function renderPipe() {
    let n = 0;
    const chip = (s) => {
      const i = n++;
      return h('button', { type: 'button', class: 'mlab-chip', 'data-i': i, 'aria-pressed': String(i === st.sel), onclick: () => select(i) },
        h('span', { class: 'mlab-name' }, s.stage), h('span', { class: 'mlab-vals' }, shown(s.stage).map((p) => s[p.name] ?? p.default).join(' · ') || ' '), h('i'));
    };
    const item = (s) => {
      if (!s.inputs) return h('li', {}, chip(s));
      const head = chip(s);
      return h('li', { class: 'mlab-combine' }, head, s.inputs.map((p, j) => h('ol', { 'aria-label': j ? 'The right half' : 'The left half' }, h('li', { class: 'mlab-half' }, j ? 'right' : 'left'), p.map((x) => item(x)))));
    };
    pipe.replaceChildren(seedChip, ...st.recipe.pipeline.map(item));
    showSelected();
    if (!isLab) return;
    // A new stage goes in after the selected one, in the same list: the pipeline, or a combine's half.
    const add = (name) => {
      const stage = name === 'combine' ? { stage: 'combine', inputs: [[{ stage: 'bsp' }], [{ stage: 'cellular' }]] } : { stage: name };
      const cur = flat(st.recipe.pipeline)[st.sel];
      const [arr, i] = cur ? listOf(cur.path) : [st.recipe.pipeline, -1];
      arr.splice(i + 1, 0, stage);
      st.sel = flat(st.recipe.pipeline).findIndex((f) => f.s === stage);
      rebuild();
    };
    pipe.append(h('li', { class: 'mlab-add' }, h('details', {}, h('summary', {}, '+ Add a stage'),
      h('div', {}, DATA.groups.map((g) => [h('strong', {}, g.title), g.stages.map((s) => h('button', { type: 'button', class: 'q', onclick: () => add(s.name) }, h('code', {}, s.name), ` ${s.title}`))])))));
  }
  /** On a narrow screen the strip scrolls sideways: keep the selected stage in it. */
  function showSelected() {
    const chip = pipe.querySelector('.mlab-chip[aria-pressed="true"]');
    if (!chip) return;
    const a = chip.getBoundingClientRect(), b = pipe.getBoundingClientRect();
    if (a.left < b.left || a.right > b.right) pipe.scrollLeft += a.right > b.right ? a.right - b.right + 8 : a.left - b.left - 8;
  }
  function select(i) {
    st.sel = i;
    for (const c of pipe.querySelectorAll('.mlab-chip')) c.setAttribute('aria-pressed', String(+c.dataset.i === i));
    showSelected();
    renderStage();
    // The strip is also the step-through's chapters: a chip jumps to its stage's last step.
    const last = st.lab ? st.stageOf.lastIndexOf(i) : -1;
    if (last >= 0) stop(), go(last);
    fitToFrame();
  }
  /** The list a stage is in, and its place there. A path alternates stage index and input index. */
  function listOf(path) {
    let arr = st.recipe.pipeline;
    for (let k = 0; k + 2 < path.length; k += 2) arr = arr[path[k]].inputs[path[k + 1]];
    return [arr, path.at(-1)];
  }
  let debounce = 0, ids = 0;
  function renderStage() {
    const stages = flat(st.recipe.pipeline);
    st.sel = Math.max(0, Math.min(st.sel, stages.length - 1));
    const cur = stages[st.sel];
    if (!cur) return stagePanel.replaceChildren();
    const set = (p, v) => { cur.s[p.name] = v; clearTimeout(debounce); debounce = setTimeout(rebuild, 90); };
    const rows = shown(cur.s.stage).map((p) => {
      const v = cur.s[p.name] ?? p.default, id = `mlab-c${++ids}`;
      let input;
      if (p.kind === 'pick') input = h('select', { id, onchange: (e) => set(p, e.target.value) }, p.options.map((o) => h('option', { value: o, selected: o === v }, o)));
      else {
        const step = p.kind === 'int' ? 1 : (p.max - p.min) / 800;
        const num = h('input', { id, type: 'number', min: p.min, max: p.max, step: p.kind === 'int' ? 1 : 'any', value: v, oninput: (e) => { if (e.target.value === '') return; set(p, +e.target.value); if (range) range.value = e.target.value; } });
        const range = p.max <= 200 && h('input', { type: 'range', min: p.min, max: p.max, step, value: v, 'aria-label': `${p.name}, as a slider`, oninput: (e) => { num.value = e.target.value; set(p, +e.target.value); } });
        input = [range, num];
      }
      return h('div', { class: 'mlab-ctl' }, h('label', { for: id }, h('code', {}, p.name)), input);
    });
    const [arr, i] = listOf(cur.path);
    const move = (d) => { [arr[i], arr[i + d]] = [arr[i + d], arr[i]]; st.sel = flat(st.recipe.pipeline).findIndex((f) => f.s === cur.s); rebuild(); };
    const title = names.titles[cur.s.stage];
    stagePanel.replaceChildren(...[
      h('p', { class: 'mlab-head' }, `Stage ${st.sel + 1}: `, h('a', { href: `#${cur.s.stage}` }, title ?? h('code', {}, cur.s.stage))),
      ...rows,
      h('p', { class: 'mlab-note' }, rows.length ? h('a', { href: `#${cur.s.stage}-controls` }, 'What each control does') : 'This stage has no controls.'),
      isLab && h('div', { class: 'mlab-acts' },
        h('button', { type: 'button', class: 'q', disabled: i === 0, onclick: () => move(-1) }, '← Earlier'),
        h('button', { type: 'button', class: 'q', disabled: i === arr.length - 1, onclick: () => move(1) }, 'Later →'),
        h('button', { type: 'button', class: 'q', disabled: arr.length === 1, onclick: () => { arr.splice(i, 1); st.sel = Math.max(0, st.sel - 1); rebuild(); } }, 'Remove')),
    ].filter(Boolean));
  }

  // ---------------------------------------------------------------- this map's traits, and measuring the pipeline
  const digits = (k) => names.traits.find((t) => t.key === k)?.digits ?? 0;
  const fmt = (v, k) => (v == null || Number.isNaN(v) ? 'none' : v.toFixed(digits(k)));
  /** A map's failures as keys. The cap's own flag is left out where the prefab's says the same thing. */
  function failKeys(bits) {
    const keys = [];
    for (let b = 0; bits >> b; b++) if ((bits >> b) & 1) keys.push(failure_key(b));
    return keys.includes('prefab-no-place') ? keys.filter((k) => k !== 'guard-hit') : keys;
  }
  const failName = (k) => names.fails[k] ?? k;
  function renderReadout() {
    const t = st.lab.traits(), value = (k) => t[TRAITS.indexOf(k)];
    const s = st.summary;
    // The summary holds 8 numbers a trait: seeds that have it, mean, sd, lowest, 10th percentile, median, 90th, highest.
    const sum = (k) => s && s[TRAITS.indexOf(k) * 8] > 0 && { p10: s[TRAITS.indexOf(k) * 8 + 4], median: s[TRAITS.indexOf(k) * 8 + 5], p90: s[TRAITS.indexOf(k) * 8 + 6] };
    const rows = names.traits.filter((n) => !n.extra || (!Number.isNaN(value(n.key)) && (n.key === 'rooms' || t[6] + t[7] + t[8] > 0))).map((n) => {
      const v = value(n.key), q = sum(n.key), at = (x) => Math.min(1, Math.max(0, x / n.to)).toFixed(4);
      return h('div', {}, h('dt', {}, h('a', { href: '#our-definitions' }, n.label)),
        h('dd', {}, fmt(v, n.key), n.key === 'path' && t[5] === 1 && !Number.isNaN(v) && h('small', { title: 'The map has no start and exit of its own, so this is between the standard probe’s two points.' }, ' probe'),
          q && h('span', { class: 'mlab-mark', role: 'img', 'aria-label': `Over seeds 0 to ${SEEDS - 1}: median ${fmt(q.median, n.key)}; 8 of 10 seeds fall between ${fmt(q.p10, n.key)} and ${fmt(q.p90, n.key)}.`, style: `--lo:${at(q.p10)};--mid:${at(q.median)};--hi:${at(q.p90)};--now:${Number.isNaN(v) ? 0 : at(v)}` }, h('i'), !Number.isNaN(v) && h('b')),
          q && h('small', {}, `median ${fmt(q.median, n.key)}`)));
    });
    const fails = failKeys(st.lab.failures());
    rows.push(h('div', { class: 'mlab-fails' }, h('dt', {}, 'Fails'), h('dd', {}, fails.length ? fails.map((f, i) => [i > 0 && ', ', h('a', { href: '#what-counts-as-failing' }, failName(f))]) : 'nothing')));
    if (s) {
      const rowsOf = st.lab.measured(), counts = {};
      for (let i = 12; i < rowsOf.length; i += 13) for (const f of failKeys(rowsOf[i])) counts[f] = (counts[f] ?? 0) + 1;
      const list = Object.entries(counts).map(([f, n]) => `${failName(f)} on ${n}`);
      rows.push(h('div', { class: 'mlab-fails' }, h('dt', {}, `Over seeds 0 to ${SEEDS - 1}`), h('dd', {}, list.length ? list.join(', ') : 'no seed fails')));
    }
    readout.replaceChildren(...rows);
    if (!isLab) report(Object.fromEntries(TRAITS.map((k, i) => [k, Number.isNaN(t[i]) ? null : t[i]])), demos[st.demo]?.id);
  }
  /** Builds the pipeline on seeds 0 to 199 in slices sized by time, so the page stays responsive. */
  async function measure() {
    const lab = st.lab;
    measureBtn.disabled = true;
    for (let n = 0, k = 1; n < SEEDS; ) {
      const t0 = performance.now();
      n = lab.measure(Math.min(k, SEEDS - n));
      k = Math.max(1, Math.min(50, Math.round((k * 24) / Math.max(1, performance.now() - t0))));
      measureBtn.textContent = `Measuring: ${n} of ${SEEDS}`;
      await new Promise((r) => setTimeout(r));
      if (st.lab !== lab) return;
    }
    st.summary = lab.summary();
    measureBtn.hidden = true;
    renderReadout();
  }

  // ---------------------------------------------------------------- the walk: the core's, drawn here
  function renderWalkButton() {
    const ok = st.lab.start() >= 0 && st.lab.exit() >= 0 && st.path.length > 1;
    walkBtn.hidden = !ok;
    fixBtn.hidden = ok;
    walkWhy.textContent = ok ? `Reach the exit. The shortest walk is ${count(st.path.length - 1)} steps.` : 'A walk needs a start and an exit the start can reach.';
  }
  function addMissing() {
    const have = new Set(flat(st.recipe.pipeline).map((f) => f.s.stage));
    if (!have.has('cull')) st.recipe.pipeline.push({ stage: 'cull', mode: 'remove' });
    if (!have.has('start')) st.recipe.pipeline.push({ stage: 'start' });
    if (!have.has('exit')) st.recipe.pipeline.push({ stage: 'exit' });
    st.sel = flat(st.recipe.pipeline).length - 1;
    rebuild();
  }
  function startWalk() {
    stop();
    if (!st.lab.walk()) return;
    st.walking = true;
    st.caches = st.lab.caches();
    fit.classList.add('is-walking');
    walkBox.hidden = false;
    renderWalk();
    fitToFrame();
    walkBox.querySelector('button')?.focus();
  }
  function leaveWalk() {
    if (!st.walking) return;
    st.walking = false;
    fit.classList.remove('is-walking');
    walkBox.hidden = true;
    walkBox.replaceChildren();
    canvas.setAttribute('aria-label', 'The map');
  }
  const DIRS = { ArrowLeft: 0, a: 0, ArrowRight: 1, d: 1, ArrowUp: 2, w: 2, ArrowDown: 3, s: 3 };
  function act(dir) {
    if (!st.walking || st.lab.walk_done()) return;
    st.lab.step(dir);
    st.lab.walk_done() ? renderResult(true) : renderWalk();
  }
  fit.addEventListener('keydown', (e) => {
    if (st.walking && e.key in DIRS && !e.metaKey && !e.ctrlKey && !e.altKey && !/^(INPUT|SELECT|TEXTAREA)$/.test(e.target.tagName)) (e.preventDefault(), act(DIRS[e.key]));
  });
  const found = () => st.caches.filter((_, n) => (st.lab.found() >> n) & 1).length;
  let walkStatus;
  function renderWalk() {
    drawWalk();
    const steps = st.lab.route().length - 1;
    if (walkBox.firstChild && walkStatus?.isConnected) return (walkStatus.textContent = `${count(steps)} steps · caches ${found()} of ${st.caches.length}`);
    walkStatus = h('strong', {}, `${count(steps)} steps · caches ${found()} of ${st.caches.length}`);
    walkBox.replaceChildren(
      h('p', { class: 'mlab-step', 'aria-live': 'polite' }, h('span', { class: 'mlab-n' }, 'The walk '), walkStatus, '. Reach the exit.'),
      h('div', { class: 'mlab-pad', role: 'group', 'aria-label': 'Move' }, [['Up', 2, '↑'], ['Left', 0, '←'], ['Down', 3, '↓'], ['Right', 1, '→']].map(([name, dir, arrow]) => h('button', { type: 'button', class: `q mlab-${name.toLowerCase()}`, 'aria-label': name, onclick: () => act(dir) }, arrow))),
      h('p', { class: 'mlab-note' }, 'The arrow keys or the pad move you. You see nine tiles around you in straight lines, and remember what you have seen.'),
      h('div', { class: 'mlab-acts' }, btn('Show me the map', () => renderResult(false)), btn('Leave', () => (leaveWalk(), draw(), fitToFrame()))));
  }
  function drawWalk() {
    // The camera follows the walker and stops at the map's edges: a window of 32 by 20 tiles, large enough to read on a phone.
    const route = st.lab.route(), seen = st.lab.seen(), at = route.at(-1), got = st.lab.found();
    const cols = 32, rows = 20, T = canvas.width / cols, edge = (v, most) => Math.max(0, Math.min(most, v));
    const x0 = edge((at % st.w) - (cols >> 1), st.w - cols), y0 = edge(((at / st.w) | 0) - (rows >> 1), st.h - rows);
    ctx.fillStyle = C.fog;
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    for (let r = 0; r < rows; r++) for (let c = 0; c < cols; c++) {
      const x = x0 + c, y = y0 + r, i = y * st.w + x;
      if (x < 0 || y < 0 || x >= st.w || y >= st.h || !seen[i]) continue;
      ctx.globalAlpha = seen[i] === 2 ? 1 : 0.5;
      ctx.fillStyle = C.tiles[st.final[i]];
      ctx.fillRect(c * T, r * T, T, T);
      if (i === st.lab.exit()) (ctx.fillStyle = C.yellow), ctx.fillRect(c * T + 3, r * T + 3, T - 6, T - 6);
      const cache = st.caches.indexOf(i);
      if (cache >= 0 && !((got >> cache) & 1)) (ctx.fillStyle = C.focus), ctx.fillRect(c * T + T * 0.3, r * T + T * 0.3, T * 0.4, T * 0.4);
    }
    ctx.globalAlpha = 1;
    ctx.fillStyle = C.primary;
    ctx.beginPath();
    ctx.arc(((at % st.w) - x0 + 0.5) * T, (((at / st.w) | 0) - y0 + 0.5) * T, T * 0.36, 0, 7);
    ctx.fill();
    canvas.setAttribute('aria-label', `The walk, from above: what you see and remember around you, after ${route.length - 1} steps`);
  }
  /** Afterwards: the reader's route against the shortest, over the distance map. */
  function renderResult(reached) {
    const route = st.lab.route(), dist = st.lab.distances(), got = st.lab.found();
    let far = 1;
    for (const d of dist) if (d > far) far = d;
    for (let i = 0; i < st.final.length; i++) cell(i, WALKABLE[st.final[i]] && dist[i] >= 0 ? C.dist(dist[i] / far) : C.tiles[st.final[i]]);
    for (const i of st.path) cell(i, C.primary, 2);
    for (const i of route) cell(i, C.focus, 3);
    st.caches.forEach((c, n) => cell(c, (got >> n) & 1 ? C.green : C.yellow, 1));
    cell(st.lab.exit(), C.yellow);
    cell(route[0], C.green);
    const mine = route.length - 1, best = st.path.length - 1;
    canvas.setAttribute('aria-label', 'The whole map: your route and the shortest route, over walking distance from the start');
    walkStatus = null;
    walkBox.replaceChildren(
      h('p', { class: 'mlab-step', 'aria-live': 'polite' },
        h('strong', {}, reached ? `You reached the exit in ${count(mine)} steps. ` : `You stopped after ${count(mine)} steps. `),
        `The shortest walk is ${count(best)}${reached ? `, so yours was ${(mine / best).toFixed(2)} times as long` : ''}. You found ${found()} of ${st.caches.length} caches. `,
        'Your route is orange and the shortest is blue. The shading is walking distance from the start.'),
      h('div', { class: 'mlab-acts' }, btn('Walk it again', () => (leaveWalk(), startWalk()), ''), btn('Back to the build', () => (leaveWalk(), draw(), fitToFrame()))));
    fitToFrame();
  }

  // ---------------------------------------------------------------- sharing, and recipes sent from the page
  function share() {
    const url = `${location.origin}${location.pathname}${location.search}#recipe=${enc(st.recipe)}`;
    navigator.clipboard?.writeText(url).then(() => (shareNote.textContent = 'Link copied'), () => (shareNote.textContent = 'The link is in the address bar'));
  }
  const me = { root, load, recipe: () => st.recipe, demo: (i) => (st.demo = i), demos };
  if (isLab) theLab = me;
  else cards.push(me);
  const shared = isLab ? fromHash(location.hash) : pending?.figure.contains(root) && pending.recipe;
  if (shared) sent(me, shared);
  // A shared recipe the core refuses leaves the lab on its opening recipe, with the parser's message.
  if (!st.lab) {
    const refused = errorBox.textContent;
    load(isLab ? params.recipe : demos[0].recipe);
    if (!st.lab) throw new Error(errorBox.textContent);
    showError(refused);
  }
}

/** Loads a recipe sent to a mount, marking which of a card's demos it is, if any. */
function sent(mount, recipe) {
  const pipeline = JSON.stringify(recipe.pipeline);
  mount.demo(Math.max(0, mount.demos.findIndex((d) => JSON.stringify(d.recipe.pipeline) === pipeline)));
  mount.load(recipe);
}

// A link to a recipe (#recipe=…) in a card loads in that card's own step-through, so a failing seed
// is watched where it is described. Anywhere else, and from "Open in the lab", it opens the lab.
addEventListener('click', (e) => {
  const a = e.target.closest?.('a[href^="#recipe="]');
  const recipe = a && !e.defaultPrevented && !e.button && !e.metaKey && !e.ctrlKey && !e.shiftKey && !e.altKey && fromHash(a.hash);
  if (!recipe) return;
  const figure = !a.closest('.mlab') && a.closest('.card')?.querySelector('figure[data-interactive]');
  const card = figure && cards.find((c) => figure.contains(c.root));
  if (figure) {
    e.preventDefault();
    // A card mounts as it nears the window; one that hasn't yet takes the recipe when it does.
    card ? sent(card, recipe) : (pending = { figure, recipe });
    figure.scrollIntoView({ block: 'center' });
  } else if (!theLab) {
    // The lab mounts as it nears the window, and reads the recipe from the address bar.
    [...document.querySelectorAll('figure[data-params]')].find((f) => /"mode":"lab"/.test(f.dataset.params))?.scrollIntoView();
  }
});
addEventListener('hashchange', () => {
  const recipe = theLab && fromHash(location.hash);
  if (!recipe) return;
  if (enc(recipe) !== enc(theLab.recipe())) theLab.load(recipe);
  theLab.root.scrollIntoView();
});

const style = document.createElement('style');
style.textContent = `
.mlab { container-type: inline-size; display: grid; gap: var(--space-xs); font-size: var(--size-step--1); }
.mlab [hidden] { display: none !important; }
.mlab-limit { position: fixed; inset-block-start: 0; visibility: hidden; pointer-events: none; block-size: var(--interactive-max-block-size); }
.mlab-fit { display: grid; gap: var(--space-2xs) var(--space-s); grid-template-columns: minmax(0, 1fr); grid-template-areas: "demos" "view" "steps" "pipe" "error" "stage"; align-items: start; }
.mlab-demos { grid-area: demos; }
.mlab-pipe { grid-area: pipe; }
.mlab-error { grid-area: error; }
.mlab-view { grid-area: view; display: grid; justify-items: center; gap: var(--space-3xs); }
.mlab-steps { grid-area: steps; display: grid; gap: var(--space-2xs); }
.mlab-stage { grid-area: stage; }
.mlab-walk { grid-area: steps; display: grid; gap: var(--space-2xs); }
.is-walking > :is(.mlab-pipe, .mlab-demos, .mlab-steps, .mlab-stage, .mlab-error), .is-walking .mlab-legend { display: none; }
@container (inline-size >= 44rem) {
  .mlab-fit { grid-template-columns: minmax(0, 1.95fr) minmax(14rem, 1fr); grid-template-rows: auto auto auto auto 1fr; grid-template-areas: "demos demos" "pipe pipe" "error error" "view steps" "view stage"; }
  .mlab-walk { grid-row: 4 / span 2; }
}
.mlab p { margin: 0; }
.mlab ul, .mlab ol, .mlab dl { margin: 0; padding: 0; list-style: none; }
.mlab canvas { display: block; max-inline-size: 100%; block-size: auto; image-rendering: pixelated; border: 1px solid var(--color-rule); background: var(--figure-dark); }
.mlab-legend { display: flex; flex-wrap: wrap; gap: 0 var(--space-xs); min-block-size: 1.5em; justify-self: start; color: var(--color-muted); }
.mlab-legend i { display: inline-block; inline-size: 0.7em; block-size: 0.7em; margin-inline-end: 0.3em; border: 1px solid var(--color-rule); }
.mlab button { padding: var(--space-3xs) var(--space-2xs); min-block-size: 2.25rem; }
.mlab button.q, .mlab-chip { border-color: var(--color-muted); background: var(--color-paper); color: var(--color-ink); }
.mlab button:disabled { opacity: 0.45; cursor: default; }
.mlab button[aria-pressed="true"] { border-color: var(--color-primary); box-shadow: inset 0 0 0 1px var(--color-primary); }
.mlab select, .mlab textarea { font: inherit; color: inherit; padding: var(--space-3xs); border: 1px solid var(--color-muted); border-radius: var(--radius); background: var(--color-paper); }
.mlab :is(a, button, input, select, textarea, summary):focus-visible { outline: 2px solid var(--color-focus); outline-offset: 1px; }
.mlab-demos { display: flex; gap: var(--space-2xs); overflow-x: auto; padding-block-end: var(--space-3xs); }
.mlab-demos button { flex: none; }
.mlab-transport, .mlab-acts, .mlab-foot { display: flex; flex-wrap: wrap; gap: var(--space-3xs) var(--space-2xs); align-items: center; }
.mlab-pipe { display: flex; gap: var(--space-2xs); align-items: stretch; overflow-x: auto; padding-block-end: var(--space-3xs); }
.mlab-pipe > li, .mlab-combine ol > li { flex: none; display: flex; }
.mlab-seed { gap: var(--space-3xs); align-items: center; }
.mlab-seed input { inline-size: 7ch; }
.mlab-name { font-family: var(--font-code); }
.mlab code { font-weight: 400; }
.mlab-chip { position: relative; display: grid; justify-items: start; padding-block-end: calc(var(--space-3xs) + 3px); text-align: start; }
.mlab-vals { color: var(--color-muted); font-size: var(--size-step--2); min-block-size: 1.2em; }
.mlab-chip i { position: absolute; inset: auto 0 0 0; block-size: 3px; background: linear-gradient(to right, var(--color-primary) calc(var(--p, 0) * 100%), var(--color-rule) 0); }
.mlab-combine { gap: var(--space-2xs); align-items: stretch; }
.mlab-combine ol { display: flex; gap: var(--space-3xs); align-items: stretch; padding: var(--space-3xs); border: 1px dashed var(--color-muted); border-radius: var(--radius); }
.mlab-half { align-items: center; color: var(--color-muted); font-size: var(--size-step--2); }
.mlab-add { position: relative; }
.mlab-add summary, .mlab-recipe summary { cursor: pointer; padding: var(--space-3xs) var(--space-2xs); min-block-size: 2.25rem; }
.mlab-add summary { border: 1px dashed var(--color-muted); border-radius: var(--radius); white-space: nowrap; }
.mlab-add div { display: grid; gap: var(--space-3xs); padding-block-start: var(--space-3xs); min-inline-size: 15rem; }
.mlab-add div button { text-align: start; }
.mlab-error { padding: var(--space-3xs) var(--space-2xs); border-inline-start: 4px solid var(--color-red); background: var(--color-paper); font-family: var(--font-code); }
.mlab-step { padding: var(--space-2xs); border-inline-start: 4px solid var(--color-primary); background: var(--color-paper); min-block-size: 6.6em; line-height: 1.4; }
.mlab-walk .mlab-step { min-block-size: 0; }
.mlab-n { display: block; color: var(--color-muted); font-weight: 600; }
.mlab-transport button { min-inline-size: 2.25rem; }
.mlab input[type="range"] { inline-size: 100%; min-inline-size: 0; margin: 0; }
.mlab-stage { display: grid; gap: var(--space-3xs); }
.mlab-head { font-weight: 600; }
.mlab-ctl { display: grid; grid-template-columns: minmax(6.5rem, auto) minmax(0, 1fr) auto; gap: var(--space-2xs); align-items: center; }
.mlab-ctl > :last-child:nth-child(2) { grid-column: 2 / -1; justify-self: start; }
.mlab-note, .mlab-readout small { color: var(--color-muted); }
.mlab-readout { display: flex; flex-wrap: wrap; gap: var(--space-2xs) var(--space-s); }
.mlab-readout > div { min-inline-size: 6.5rem; }
.mlab-readout dt { color: var(--color-muted); }
.mlab-readout dd { margin: 0; font-variant-numeric: tabular-nums; }
.mlab-fails { flex-basis: 100%; }
.mlab-mark { position: relative; display: block; inline-size: 6.5rem; block-size: 14px; margin-block: 2px; }
.mlab-mark::before { content: ''; position: absolute; inset-inline: 0; inset-block-start: 6px; block-size: 2px; background: var(--color-rule); }
.mlab-mark::after { content: ''; position: absolute; inset-block-start: 4px; inset-inline-start: calc(var(--lo) * 100%); inline-size: max(2px, calc((var(--hi) - var(--lo)) * 100%)); block-size: 6px; border-radius: 3px; background: color-mix(in oklab, var(--color-primary) 85%, var(--color-surface)); }
.mlab-mark i { position: absolute; inset-block-start: 3px; inset-inline-start: calc(var(--mid) * 100%); inline-size: 8px; block-size: 8px; margin-inline-start: -4px; border-radius: 50%; background: var(--color-ink); box-shadow: 0 0 0 2px var(--color-surface); }
.mlab-mark b { position: absolute; inset-block-start: 0; inset-inline-start: calc(var(--now) * 100%); inline-size: 3px; block-size: 14px; margin-inline-start: -1.5px; border-radius: 1.5px; background: var(--color-focus); box-shadow: 0 0 0 1.5px var(--color-surface); }
.mlab-spacer { flex: 1; }
.mlab-recipe textarea { display: block; inline-size: 100%; margin-block: var(--space-2xs); font-family: var(--font-code); font-size: var(--size-step--2); }
.mlab-pad { display: grid; grid-template-areas: ". up ." "left down right"; grid-template-columns: repeat(3, 2.75rem); gap: var(--space-3xs); justify-content: center; }
.mlab-pad button { min-block-size: 2.75rem; }
.mlab-up { grid-area: up; } .mlab-left { grid-area: left; } .mlab-down { grid-area: down; } .mlab-right { grid-area: right; }
`;
document.head.append(style);
