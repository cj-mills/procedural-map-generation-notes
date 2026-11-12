// The map lab's module: one module, mounted once as the lab at the head of the page and once in
// every technique card. It runs the deterministic core in WebAssembly (lab/core in this page's
// repository), so a recipe builds the same map here, on the command line and in an engine.
//
//   mount(root, { params, fallback, reserve, report })     the site's contract, awaited
//
// params.mode is "lab" (the pipeline can be edited, measured, walked and shared) or "card" (the
// pipeline is one of the technique's demos).
//
// A card mounts as its fallback's last step stands: the finished map, the step's label and its
// text, at the same sizes, so the swap moves nothing on the page (the site's spec, Interactives).
// The strip, the transport, the controls, the readout and the rest open when the reader asks: the
// button over the map, the map itself, or a recipe link on the page.
//
// The lab at the page's head doesn't wait to be asked. Its page reserves its open height on its
// tag (`reserve`, from rows.js), the site holds the figure to that from the first paint, and the
// lab opens at mount, within it. Open, the lab's height is fixed by its layout: the map follows
// the width and every other row has a set height, whatever recipe it loads and whatever the
// reader does short of opening the recipe's text or the list of stages to add. Without a reserve
// it mounts closed, as a card does.
//
// The module holds no wording of its own about the
// techniques: each step's text is read from the page's step lists by its key. The names of
// stages, traits and failures and the talk's demos are written into DATA by the page's build,
// from the same files the page's tables are made from. The controls are made from the core's
// stage table, and the caches, the 200-seed summary and the walk are the core's.
import init, { Lab, failure_key, note_key, stage_table } from './map_lab_wasm.js';
import { MAP_WIDE, ROWS, SAYS_NARROW, SAYS_WIDE, actionsSlot, endSlot, mapSlotNarrow, stageSlot } from './rows.js';

let ready; // one WebAssembly instance for every mount on the page
let table; // the core's stage table: every stage and its parameters, by stage name
let theLab; // the page's lab, once it has mounted
const cards = []; // each mounted card: its root and its load()
let pending; // a recipe sent to a card that hasn't mounted yet: its figure and the recipe
let ids = 0; // for the controls' ids, unique on the page whichever mount makes them

// What the page calls things, and the recipes it can load by name. The build replaces this line
// (build.mjs, from page/gen.mjs): traits [{ key, label, digits, to, extra }], fails { key: label },
// groups [{ title, stages: [{ name, title }] }], demos [{ id, label, stage, recipe, own }].
const DATA = { traits: [], fails: {}, groups: [], demos: [] };

const PX = 8; // canvas pixels a tile; CSS scales the canvas down to its column
const SEEDS = 200;
export const NEW_TINT = 0.6; // how strongly a step's new tiles are tinted; the build's figures use it too
const TRAITS = ['floor_share', 'regions', 'dead_ends', 'corridor_ratio', 'path', 'probed', 'water', 'grass', 'mountain', 'field_mean', 'field_sd', 'rooms'];
const WALKABLE = [0, 1, 0, 1, 0, 1, 1]; // by tile: wall, floor, water, grass, mountain, treasure, trap
const TERRAIN = { 2: 'water', 3: 'grass', 4: 'mountain' };
const MARKS = { 1: 'new', 2: 'rejected', 3: 'point', 4: 'start', 5: 'exit', 6: 'path', 7: 'hot path', 8: 'unreachable', 9: 'on the route', 10: 'off the route', 11: 'target', 12: 'locked door', 13: 'key', 14: 'start’s side', 32: 'rooms or cells' };

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
    const recipe = m && JSON.parse(new TextDecoder().decode(Uint8Array.from(atob(m[1].replace(/-/g, '+').replace(/_/g, '/')), (c) => c.charCodeAt(0))));
    // Anything with a pipeline is the core's to accept or refuse, with its own message; anything else isn't a recipe.
    return Array.isArray(recipe?.pipeline) ? recipe : null;
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
    marks: { 1: get('var(--color-primary)'), 2: get('var(--color-red)'), 3: get('var(--color-magenta)'), 4: get('var(--color-green)'), 5: get('var(--color-yellow)'), 6: get('var(--color-primary)'), 7: tint('yellow', 55), 8: tint('red', 65), 9: tint('green', 60), 10: tint('muted', 40), 11: get('var(--color-magenta)'), 12: get('var(--color-cyan)'), 13: get('var(--color-magenta)'), 14: tint('green', 45) },
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

export async function mount(root, { params, fallback, reserve, report }) {
  ready ??= init().then(() => (table = Object.assign(JSON.parse(stage_table()), { by: {} })) && table.stages.forEach((s) => (table.by[s.name] = s)));
  await ready;
  const isLab = params.mode === 'lab';
  // The lab starts from any of the talk's demos; a card has its technique's, by id.
  const demos = isLab ? DATA.demos.filter((d) => !d.own) : params.demos.map((id) => DATA.demos.find((d) => d.id === id));
  const names = { traits: DATA.traits, fails: DATA.fails, titles: {} };
  for (const g of DATA.groups) for (const s of g.stages) names.titles[s.name] = s.title;
  const st = { recipe: null, lab: null, frame: 0, sel: 0, demo: 0, timer: 0, walking: false, summary: null, open: false };
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
  // A step's text keeps its reference marker, as the site prints it: a copy opens its note as the original does.
  const stepText = h('span');
  const stepBox = h('p', { class: 'mlab-step', 'aria-live': 'polite' }, stepLabel, stepText);
  // One slot says what is on the map: the step's text, or the walk's count while the reader walks, or the
  // parser's message while the recipe is one the core refuses. The slot keeps its height whichever it holds.
  const walkSays = h('p', { class: 'mlab-step', 'aria-live': 'polite', hidden: true });
  const says = h('div', { class: 'mlab-says' }, stepBox, walkSays, errorBox);
  // Closed, the way in is a button over the map: it takes no room, so the closed lab keeps its fallback's size.
  const openBtn = h('button', { type: 'button', class: 'mlab-open', onclick: () => open(true) }, h('span', { class: 'mlab-tri' }), isLab ? 'Open the lab' : 'Step through it');
  const playBtn = btn('Play', () => (st.timer ? stop() : play()), 'mlab-play');
  const transport = h('div', { class: 'mlab-transport', role: 'group', 'aria-label': 'Step through the build' },
    btn('⏮', () => go(0), 'q mlab-end', 'First step'), btn('◀', () => go(st.frame - 1), 'q', 'Step back'), btn('▶', () => go(st.frame + 1), 'q', 'Step forward'), btn('⏭', () => go(Infinity), 'q mlab-end', 'The finished map'), playBtn, scrub);
  const stagePanel = h('div', { class: 'mlab-stage' });
  const walkBox = h('div', { class: 'mlab-walk', hidden: true });
  const readout = h('dl', { class: 'mlab-readout' });
  const failsBox = h('p', { class: 'mlab-fails' });
  const seedIn = h('input', { type: 'number', min: '0', max: String(table.recipe.maxSeed), step: '1', 'aria-label': 'Seed', onchange: () => ((st.recipe.seed = Math.max(0, Math.floor(+seedIn.value || 0))), rebuild()) });
  // A recipe is a seed plus a pipeline, so the seed is the strip's first link.
  const seedChip = h('li', { class: 'mlab-seed' }, h('span', { class: 'mlab-name' }, 'seed'), seedIn, btn('New', () => ((st.recipe.seed = Math.floor(Math.random() * 100000)), rebuild()), 'q', 'A new random seed'));
  const walkBtn = btn('Walk this map', () => startWalk(), '');
  const fixBtn = btn('Make it walkable', addMissing, '', 'A walk needs a start and an exit the start can reach: this adds what is missing');
  const walkWhy = h('span', { class: 'mlab-note' });
  const measureBtn = btn(`Measure over ${SEEDS} seeds`, measure, 'q');
  const presets = isLab && h('select', { 'aria-label': 'Start from one of the talk’s demos', onchange: () => load(demos[+presets.value].recipe) },
    h('option', { value: '', disabled: true, hidden: true }, 'your own pipeline'),
    DATA.groups.map((g) => h('optgroup', { label: g.title }, demos.map((d, i) => g.stages.some((s) => s.name === d.stage) && h('option', { value: i }, `${d.id.split('-')[0]} ${d.label}`)))));
  const demoBtns = !isLab && demos.length > 1 && h('div', { class: 'mlab-demos', role: 'group', 'aria-label': 'The talk’s demos of this technique' },
    demos.map((d, i) => h('button', { type: 'button', class: 'q', 'aria-pressed': 'false', onclick: () => ((st.demo = i), load(d.recipe)) }, d.label)));
  const shareBtn = btn('Copy link', share, 'q mlab-share');
  const shareNote = h('span', { class: 'mlab-sr', role: 'status' });
  const openLink = !isLab && h('a', { href: '#recipe=' }, 'Open in the lab');
  const recipeText = h('textarea', { spellcheck: 'false', rows: '8', 'aria-label': 'The recipe, as JSON' });
  const cliLine = h('code', {}, 'map-lab build recipe.json');
  // Nothing a reader presses may move because of the press. Open, every part is a row of set height (rows.js),
  // so the lab is one height whatever it shows: the map, then the step buttons, the strip and the stage's
  // slot, which must fit the window with the map; then the slot for the step's text, the actions, and a
  // last part for the readout and the recipe's line, which fill it from the top. Side by side on a wide
  // screen: the step buttons and the stage beside the map, the actions beside the step's text.
  const fit = h('div', { class: `mlab-fit${demoBtns ? ' has-demos' : ''}`, 'data-fit': true }, demoBtns, pipe, h('div', { class: 'mlab-view' }, h('div', { class: 'mlab-map' }, canvas), openBtn, legend), transport, stagePanel, walkBox);
  const box = h('div', { class: `mlab is-closed${isLab ? ' is-lab' : ''}` }, fit,
    h('div', { class: 'mlab-mid' }, says,
      h('div', { class: 'mlab-foot' }, isLab ? [walkBtn, fixBtn, walkWhy, measureBtn, h('label', { class: 'mlab-from' }, 'Start from ', presets)] : openLink)),
    h('div', { class: 'mlab-last' }, readout, failsBox, isLab && h('div', { class: 'mlab-recipe' }, h('details', {}, h('summary', {}, 'Recipe'), recipeText,
      h('div', { class: 'mlab-acts' }, btn('Apply', () => { try { load(JSON.parse(recipeText.value)); } catch (e) { showError(`recipe: ${e.message}`); } }, ''), cliLine,
        h('span', { class: 'mlab-note' }, 'Save it as recipe.json: the same recipe builds the same map on the command line.'))), shareBtn, shareNote)));
  root.append(box);
  canvas.addEventListener('click', () => open(false));

  /** Opens everything the closed lab leaves out. For a card it is the reader's act, so the page may move
   *  for it; the lab at the page's head opens at mount, into the room its page reserved. */
  function open(focus) {
    if (st.open) return;
    st.open = true;
    box.classList.remove('is-closed');
    showSelected();
    if (focus) playBtn.focus();
  }

  // ---------------------------------------------------------------- recipe and build
  function showError(msg) {
    errorBox.textContent = msg;
    errorBox.hidden = !msg;
    says.classList.toggle('has-error', !!msg);
  }
  function load(recipe) {
    // A recipe from the address bar, a link or the recipe's text is the core's to refuse before anything of
    // it is shown: the parser's message says where it is wrong, and the lab keeps what it had.
    let built;
    try {
      built = new Lab(text(recipe));
    } catch (e) {
      return showError(e.message);
    }
    st.recipe = copy(clean(recipe));
    const stages = flat(st.recipe.pipeline);
    st.sel = isLab ? stages.length - 1 : Math.max(0, stages.findIndex((f) => f.s.stage === params.stage));
    rebuild(built);
  }
  /** Shows the recipe, built: by load() already, or here after an edit, which the core may refuse. */
  function rebuild(built) {
    stop();
    leaveWalk();
    seedIn.value = st.recipe.seed;
    recipeText.value = JSON.stringify(clean(st.recipe), null, 1);
    renderPipe();
    renderStage();
    let lab = built;
    try {
      lab ??= new Lab(text(st.recipe));
    } catch (e) {
      // The parser says where the recipe is wrong; the last good map stays.
      return showError(e.message);
    }
    showError('');
    st.lab?.free();
    st.lab = lab;
    st.summary = null;
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
    // The shortest walk that finishes the level: the path, or with a locked door, by way of its key.
    st.finish = lab.finish();
    st.door = lab.door();
    st.cur = -1;
    canvas.width = st.w * PX;
    canvas.height = st.h * PX;
    scrub.max = st.n - 1;
    // The lab shows the finished map. A card shows its own stage's last step, which is its fallback's last
    // step too: in the talk's combined map a prefab follows the stage the card is about.
    const mine = isLab ? -1 : flat(st.recipe.pipeline).findIndex((f) => f.s.stage === params.stage);
    go(mine < 0 ? st.n - 1 : Math.max(0, st.stageOf.lastIndexOf(mine)));
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
  }

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
    stepLabel.replaceChildren(`Step ${count(st.frame + 1)} of ${count(st.n)}`, h('span', { class: 'mlab-at' }, ` · ${st.lab.stage_name(stage)} `));
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
    const g = gridAt(st.frame), field = st.lab.frame_field(st.frame), used = new Set(), land = new Set();
    // A noise field is drawn as brightness only where the map hasn't made terrain of it: the height map.
    // Water, grass and mountain are drawn as what they are.
    for (let i = 0; i < g.length; i++) {
      if (g[i] >= 2 && g[i] <= 4) land.add(g[i]);
      cell(i, field.length && g[i] < 2 ? C.field(field[i]) : C.tiles[g[i]]);
    }
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
      // A locked door and its key, where a lock stage placed them.
      if (st.door >= 0) (cell(st.door, C.marks[12]), cell(st.lab.key(), C.marks[13], 2));
      [4, ...(st.lab.exit() >= 0 ? [5] : []), ...(st.path.length ? [6] : []), ...(st.door >= 0 ? [12, 13] : [])].forEach((m) => used.add(m));
    }
    legend.replaceChildren(
      ...[...land].sort().map((t) => h('li', {}, h('i', { style: `background:${C.tiles[t]}` }), TERRAIN[t])),
      ...[...used].sort((a, b) => a - b).map((m) => h('li', {}, h('i', { style: `background:${m === 32 ? C.groups[0] : C.marks[m]}` }), MARKS[m])));
  }
  matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => ((C = palette(root)), st.walking ? drawWalk() : draw()));

  // ---------------------------------------------------------------- the pipeline strip
  // A stage the core doesn't have has no controls here: the parser names it when the recipe is built.
  const shown = (stage) => (table.by[stage]?.params ?? []).filter((p) => p.kind !== 'inputs' && !(p.kind === 'pick' && p.options.length < 2));
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
    // The strip isn't scrolled to a chip the reader has just pressed: it would move under the press.
    renderStage();
    // The strip is also the step-through's chapters: a chip jumps to its stage's last step.
    const last = st.lab ? st.stageOf.lastIndexOf(i) : -1;
    if (last >= 0) stop(), go(last);
  }
  /** The list a stage is in, and its place there. A path alternates stage index and input index. */
  function listOf(path) {
    let arr = st.recipe.pipeline;
    for (let k = 0; k + 2 < path.length; k += 2) arr = arr[path[k]].inputs[path[k + 1]];
    return [arr, path.at(-1)];
  }
  let debounce = 0;
  function renderStage() {
    const stages = flat(st.recipe.pipeline);
    st.sel = Math.max(0, Math.min(st.sel, stages.length - 1));
    const cur = stages[st.sel];
    if (!cur) return stagePanel.replaceChildren();
    const set = (p, v) => { cur.s[p.name] = v; clearTimeout(debounce); debounce = setTimeout(() => rebuild(), 90); };
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
    const act = (label, name, off, fn) => h('button', { type: 'button', class: 'q', disabled: off, title: name, 'aria-label': name, onclick: fn }, label);
    // The stage's name links to what each of its controls does, in its technique's card. Which stage it is,
    // the strip shows; the head says it for a reader who can't see the strip.
    stagePanel.replaceChildren(
      h('div', { class: 'mlab-head' },
        h('p', {}, h('span', { class: 'mlab-sr' }, `Stage ${st.sel + 1}: `), h('a', { href: `#${cur.s.stage}${rows.length ? '-controls' : ''}` }, title ?? h('code', {}, cur.s.stage))),
        isLab && h('div', { class: 'mlab-moves' }, act('←', 'Move this stage earlier', i === 0, () => move(-1)), act('→', 'Move this stage later', i === arr.length - 1, () => move(1)),
          act('✕', 'Remove this stage', arr.length === 1, () => { arr.splice(i, 1); st.sel = Math.max(0, st.sel - 1); rebuild(); }))),
      // The slot holds the most controls any stage has; a stage with more would scroll inside it.
      rows.length ? h('div', { class: `mlab-ctls${rows.length > 3 ? ' mlab-many' : ''}` }, rows) : h('p', { class: 'mlab-note' }, 'This stage has no controls.'));
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
    // The slot has a cell for every trait the core reports: the five every map has, then rooms placed, then
    // the terrain's three shares on a row of their own. A cell is the trait's name, this map's value and a
    // scale with the value marked on it; measuring adds where 200 seeds fall on the same scale, in the same room.
    const shown = names.traits.filter((n) => !n.extra || (!Number.isNaN(value(n.key)) && (n.key === 'rooms' || t[6] + t[7] + t[8] > 0)));
    const order = [...shown.filter((n) => !n.extra), ...shown.filter((n) => n.key === 'rooms'), ...shown.filter((n) => n.extra && n.key !== 'rooms')];
    readout.replaceChildren(...order.map((n) => {
      const v = value(n.key), q = sum(n.key), at = (x) => Math.min(1, Math.max(0, x / n.to)).toFixed(4), has = !Number.isNaN(v);
      return h('div', { class: n.key === 'water' ? 'mlab-terrain' : null }, h('dt', {}, h('a', { href: '#our-definitions' }, n.label)),
        h('dd', {}, fmt(v, n.key), n.key === 'path' && t[5] === 1 && has && h('small', { title: 'The map has no start and exit of its own, so this is between the standard probe’s two points.' }, ' probe'),
          q && h('small', { class: 'mlab-med' }, ` median ${fmt(q.median, n.key)}`),
          h('span', { class: 'mlab-mark', role: q ? 'img' : null, 'aria-hidden': q ? null : 'true', 'aria-label': q && `Over seeds 0 to ${SEEDS - 1}: median ${fmt(q.median, n.key)}; 8 of 10 seeds fall between ${fmt(q.p10, n.key)} and ${fmt(q.p90, n.key)}.`,
            style: `--now:${has ? at(v) : 0}${q ? `;--lo:${at(q.p10)};--mid:${at(q.median)};--hi:${at(q.p90)}` : ''}` }, q && h('em'), q && h('i'), has && h('b'))));
    }));
    const fails = failKeys(st.lab.failures());
    let over = 'not measured';
    if (s) {
      const rowsOf = st.lab.measured(), counts = {};
      for (let i = 12; i < rowsOf.length; i += 13) for (const f of failKeys(rowsOf[i])) counts[f] = (counts[f] ?? 0) + 1;
      over = Object.entries(counts).map(([f, n]) => `${failName(f)} on ${n}`).join(', ') || 'no seed fails';
    }
    failsBox.replaceChildren(h('span', { class: 'mlab-k' }, 'Fails: '), ...(fails.length ? fails.flatMap((f, i) => [i > 0 && ', ', h('a', { href: '#failure-criteria' }, failName(f))]).filter(Boolean) : ['nothing']),
      '.', ...(isLab ? [' ', h('span', { class: 'mlab-k' }, `Over seeds 0 to ${SEEDS - 1}: `), over, '.'] : []));
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
    // The button stays where it was, saying what was done: a button that left would move what follows it.
    measureBtn.textContent = `Measured over ${SEEDS} seeds`;
    renderReadout();
  }

  // ---------------------------------------------------------------- the walk: the core's, drawn here
  function renderWalkButton() {
    const level = st.lab.start() >= 0 && st.lab.exit() >= 0 && st.path.length > 1;
    // A level whose key is behind its own locked door can't be finished, and nothing the button adds would mend it.
    const ok = level && st.finish.length > 1;
    walkBtn.hidden = !ok;
    fixBtn.hidden = level;
    walkWhy.textContent = ok ? `The shortest walk: ${count(st.finish.length - 1)} steps${st.door >= 0 ? ', by way of the key' : ''}` : level ? 'No walk: the key is behind its own door.' : `adds ${['cull', 'start', 'exit'].filter((name) => name !== 'cull' || !flat(st.recipe.pipeline).some((f) => f.s.stage === 'cull')).join(' → ')}`;
  }
  function addMissing() {
    const have = new Set(flat(st.recipe.pipeline).map((f) => f.s.stage));
    if (!have.has('cull')) st.recipe.pipeline.push({ stage: 'cull', mode: 'remove' });
    if (!have.has('start')) st.recipe.pipeline.push({ stage: 'start' });
    if (!have.has('exit')) st.recipe.pipeline.push({ stage: 'exit' });
    st.sel = flat(st.recipe.pipeline).length - 1;
    rebuild();
  }
  // Walking, the map stays where it is and the lab keeps its height: the pad takes the room of the step
  // buttons and the stage's slot, and the walk's count takes the step's text's.
  function startWalk() {
    stop();
    if (!st.lab.walk()) return;
    st.walking = true;
    st.caches = st.lab.caches();
    box.classList.add('is-walking');
    pipe.inert = true;
    walkBox.hidden = walkSays.hidden = false;
    renderWalk();
    walkBox.querySelector('button')?.focus();
  }
  function leaveWalk() {
    if (!st.walking) return;
    st.walking = false;
    box.classList.remove('is-walking');
    pipe.inert = false;
    walkBox.hidden = walkSays.hidden = true;
    walkBox.replaceChildren();
    walkSays.replaceChildren();
    canvas.setAttribute('aria-label', 'The map');
  }
  const DIRS = { ArrowLeft: 0, a: 0, ArrowRight: 1, d: 1, ArrowUp: 2, w: 2, ArrowDown: 3, s: 3 };
  function act(dir) {
    if (!st.walking || st.lab.walk_done()) return;
    st.lab.step(dir);
    st.lab.walk_done() ? renderResult(true) : renderWalk();
  }
  box.addEventListener('keydown', (e) => {
    if (st.walking && e.key in DIRS && !e.metaKey && !e.ctrlKey && !e.altKey && !/^(INPUT|SELECT|TEXTAREA)$/.test(e.target.tagName)) (e.preventDefault(), act(DIRS[e.key]));
  });
  const found = () => st.caches.filter((_, n) => (st.lab.found() >> n) & 1).length;
  let walkStatus;
  function renderWalk() {
    drawWalk();
    const steps = st.lab.route().length - 1;
    const says = `${count(steps)} steps · ${st.door < 0 ? '' : st.lab.has_key() ? 'key found · ' : 'no key yet · '}caches ${found()} of ${st.caches.length}`;
    if (walkStatus?.isConnected) return (walkStatus.textContent = says);
    walkStatus = h('strong', {}, says);
    walkSays.replaceChildren(h('span', { class: 'mlab-n' }, 'The walk'), walkStatus, st.door < 0 ? '. Reach the exit.' : '. Find the key, open the door, reach the exit.');
    walkBox.replaceChildren(
      h('div', { class: 'mlab-pad', role: 'group', 'aria-label': 'Move' }, [['Up', 2, '↑'], ['Left', 0, '←'], ['Down', 3, '↓'], ['Right', 1, '→']].map(([name, dir, arrow]) => h('button', { type: 'button', class: `q mlab-${name.toLowerCase()}`, 'aria-label': name, onclick: () => act(dir) }, arrow))),
      h('p', { class: 'mlab-note' }, 'The arrow keys or the pad move you. You see nine tiles around you in straight lines, and remember what you have seen.'),
      h('div', { class: 'mlab-acts' }, btn('Show me the map', () => renderResult(false)), btn('Leave', () => (leaveWalk(), draw()))));
  }
  function drawWalk() {
    // The camera follows the walker and stops at the map's edges: a window of 32 by 20 tiles, large enough to read on a phone.
    const route = st.lab.route(), seen = st.lab.seen(), at = route.at(-1), got = st.lab.found(), locked = st.door >= 0 && !st.lab.has_key();
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
      // The door while it is locked, and the key until it is taken.
      if (!locked) continue;
      if (i === st.door) (ctx.fillStyle = C.marks[12]), ctx.fillRect(c * T, r * T, T, T);
      if (i === st.lab.key()) (ctx.fillStyle = C.marks[13]), ctx.fillRect(c * T + T * 0.25, r * T + T * 0.25, T * 0.5, T * 0.5);
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
    for (const i of st.finish) cell(i, C.primary, 2);
    for (const i of route) cell(i, C.focus, 3);
    st.caches.forEach((c, n) => cell(c, (got >> n) & 1 ? C.green : C.yellow, 1));
    cell(st.lab.exit(), C.yellow);
    cell(route[0], C.green);
    const lock = st.door >= 0, key = (color, label) => h('li', {}, h('i', { style: `background:${color}` }), label);
    if (lock) (cell(st.door, C.marks[12]), cell(st.lab.key(), C.marks[13], 1));
    const mine = route.length - 1, best = st.finish.length - 1;
    canvas.setAttribute('aria-label', 'The whole map: your route and the shortest route, over walking distance from the start');
    walkStatus = null;
    walkSays.replaceChildren(
      h('strong', {}, reached ? `You reached the exit in ${count(mine)} steps. ` : `You stopped after ${count(mine)} steps. `),
      `The shortest walk${lock ? ', by way of the key,' : ''} is ${count(best)}${reached ? `, so yours was ${(mine / best).toFixed(2)} times as long` : ''}. You found ${lock && !reached ? (st.lab.has_key() ? 'the key and ' : 'no key and ') : ''}${found()} of ${st.caches.length} caches.`);
    // What the map now shows, as a key where the pad was: the colors are the drawing's own, in either theme.
    walkBox.replaceChildren(
      h('ul', { class: 'mlab-key' }, key(C.focus, 'your route'), key(C.primary, 'the shortest route'), key(C.green, st.caches.length ? 'the start, and each cache you found' : 'the start'),
        key(C.yellow, st.caches.length ? 'the exit, and each cache you missed' : 'the exit'), lock && key(C.marks[12], 'the locked door'), lock && key(C.marks[13], 'its key'), key(C.dist(0.8), 'floor: the bluer, the farther from the start on foot')),
      h('div', { class: 'mlab-acts' }, btn('Walk it again', () => (leaveWalk(), startWalk()), ''), btn('Back to the build', () => (leaveWalk(), draw()))));
  }

  // ---------------------------------------------------------------- sharing, and recipes sent from the page
  function share() {
    const url = `${location.origin}${location.pathname}${location.search}#recipe=${enc(st.recipe)}`;
    // The button says what happened, where it is: a note beside it would need room of its own.
    const said = (label, note) => ((shareBtn.textContent = label), (shareNote.textContent = note), setTimeout(() => ((shareBtn.textContent = 'Copy link'), (shareNote.textContent = '')), 2500));
    const inBar = () => (history.replaceState(null, '', url), said('In the address bar', 'The link is in the address bar'));
    navigator.clipboard ? navigator.clipboard.writeText(url).then(() => said('Link copied', 'Link copied'), inBar) : inBar();
  }
  const me = { root, load, open, recipe: () => st.recipe, demo: (i) => (st.demo = i), demos };
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
  // A card sent a recipe before it mounted was asked for by the reader, and a refused recipe has a message
  // to show: both open.
  if ((shared && !isLab) || !errorBox.hidden) open(false);
  // The lab is the page's main part, so it opens without being asked: its page reserved its open height on
  // its tag and the site has held the figure to it since the first paint, so opening moves nothing. Without
  // a reserve it waits behind its button, as a card does.
  if (isLab && reserve) open(false);
}

/** Loads a recipe sent to a mount, marking which of a card's demos it is, if any. */
function sent(mount, recipe) {
  const pipeline = JSON.stringify(recipe.pipeline);
  mount.demo(Math.max(0, mount.demos.findIndex((d) => JSON.stringify(d.recipe.pipeline) === pipeline)));
  mount.load(recipe);
}
/** The same, for a recipe link the reader has just followed: the mount opens to show it. */
function followed(mount, recipe) {
  sent(mount, recipe);
  mount.open(false);
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
    card ? followed(card, recipe) : (pending = { figure, recipe });
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
  theLab.open(false);
  theLab.root.scrollIntoView();
});

// The open lab's rows, from rows.js: `rem` is a length in rem, and the names below are the slots.
const rem = (n) => `${+n.toFixed(4)}rem`;
const N = ROWS.narrow, M = ROWS.mid, W = ROWS.wide;
const actionRows = (layout) => `repeat(${layout.actions}, minmax(${rem(ROWS.action)}, auto))`;
const DEMOS_ROW = 2.75; // a card's row of demo buttons
const ctlsSlot = (rows) => rem(rows * (ROWS.control + ROWS.controlGap) - ROWS.controlGap);

const style = document.createElement('style');
style.textContent = `
.mlab { container-type: inline-size; display: grid; grid-template-columns: minmax(0, 1fr); gap: ${rem(ROWS.gap)}; font-size: var(--size-step--1); }
.mlab [hidden] { display: none !important; }
.mlab-sr { position: absolute; inline-size: 1px; block-size: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
.mlab p { margin: 0; }
.mlab ul, .mlab ol, .mlab dl { margin: 0; padding: 0; list-style: none; }
.mlab button { padding: 0 var(--space-2xs); min-block-size: 2.25rem; }
.mlab button.q, .mlab-chip { border-color: var(--color-muted); background: var(--color-paper); color: var(--color-ink); }
.mlab button:disabled { opacity: 0.45; cursor: default; }
.mlab button[aria-pressed="true"] { border-color: var(--color-primary); box-shadow: inset 0 0 0 1px var(--color-primary); }
.mlab select, .mlab textarea { font: inherit; color: inherit; padding: var(--space-3xs); border: 1px solid var(--color-muted); border-radius: var(--radius); background: var(--color-paper); }
.mlab :is(a, button, input, select, textarea, summary):focus-visible { outline: 2px solid var(--color-focus); outline-offset: 1px; }
.mlab code { font-weight: 400; }
.mlab-name { font-family: var(--font-code); }
.mlab-note, .mlab-readout small { color: var(--color-muted); }
.mlab-acts { display: flex; flex-wrap: wrap; gap: 0.25rem 0.5rem; align-items: center; }

/* The drawing and its controls: one column, the map over the step buttons, the strip and the stage's slot.
   Each row has a set height, so the part is one height whatever the stage and the step; the map's slot is
   what the window leaves it (rows.js, mapSlotNarrow). */
.mlab-fit { --mlab-map: ${mapSlotNarrow()}; display: grid; gap: ${rem(ROWS.fitGap)} 1rem; align-items: start; grid-template-columns: minmax(0, 1fr);
  grid-template-areas: "view" "ctl" "pipe" "stage"; grid-template-rows: auto minmax(${rem(N.transport)}, auto) minmax(${rem(ROWS.strip)}, auto) minmax(${rem(stageSlot(N))}, auto); }
.mlab-fit.has-demos { --mlab-map: ${mapSlotNarrow(DEMOS_ROW + ROWS.fitGap)}; grid-template-areas: "demos" "view" "ctl" "pipe" "stage"; grid-template-rows: minmax(${rem(DEMOS_ROW)}, auto) auto minmax(${rem(N.transport)}, auto) minmax(${rem(ROWS.strip)}, auto) minmax(${rem(stageSlot(N))}, auto); }
.mlab-demos { grid-area: demos; display: flex; gap: 0.5rem; overflow-x: auto; scrollbar-width: thin; }
.mlab-demos button { flex: none; }
.mlab-view { grid-area: view; display: grid; grid-template-columns: minmax(0, 1fr); gap: 0.25rem; }
/* A map of another shape than the page's is drawn inside the same slot, smaller. */
.mlab-map { display: grid; grid-template: 100% / 100%; place-items: center; block-size: var(--mlab-map); }
.mlab canvas { display: block; max-inline-size: 100%; max-block-size: 100%; image-rendering: pixelated; border: 1px solid var(--color-rule); background: var(--figure-dark); }
.mlab-legend { display: flex; gap: 0 0.75rem; block-size: ${rem(ROWS.legend - 0.25)}; line-height: ${rem(ROWS.legend - 0.25)}; max-inline-size: 100%; overflow: hidden; white-space: nowrap; color: var(--color-muted); }
.mlab-legend li { flex: none; }
.mlab-legend i { display: inline-block; inline-size: 0.7em; block-size: 0.7em; margin-inline-end: 0.3em; border: 1px solid var(--color-rule); }
/* The scrubber stands in for the first and last buttons where there is one row. */
.mlab-transport { grid-area: ctl; display: grid; grid-template-columns: auto auto auto minmax(0, 1fr); gap: 0.25rem 0.5rem; align-items: center; }
.mlab-transport button { min-inline-size: 2.25rem; }
.mlab-end { display: none; }
.mlab-play { min-inline-size: 4rem; }
.mlab input[type="range"] { inline-size: 100%; min-inline-size: 0; margin: 0; }
/* The strip is one row that scrolls sideways, with room under its chips for a scrollbar where one is drawn. */
.mlab-pipe { grid-area: pipe; display: flex; gap: 0.5rem; align-items: flex-start; overflow-x: auto; scrollbar-width: thin; min-block-size: ${rem(ROWS.strip)}; }
.mlab-pipe > li, .mlab-combine ol > li { flex: none; display: flex; align-items: center; min-block-size: 3.125rem; }
.mlab-seed { gap: 0.25rem; }
.mlab-seed input { inline-size: 7ch; block-size: 2.25rem; }
.mlab-chip { position: relative; display: grid; align-content: center; justify-items: start; min-block-size: 3.125rem; padding-block-end: 3px; text-align: start; line-height: 1.25; }
.mlab-vals { color: var(--color-muted); font-size: var(--size-step--2); min-block-size: 1.25em; }
.mlab-chip i { position: absolute; inset: auto 0 0 0; block-size: 3px; background: linear-gradient(to right, var(--color-primary) calc(var(--p, 0) * 100%), var(--color-rule) 0); }
.mlab-combine { gap: 0.5rem; }
.mlab-combine ol { display: flex; gap: 0.25rem; padding: 0 0.25rem; border: 1px dashed var(--color-muted); border-radius: var(--radius); }
.mlab-half { color: var(--color-muted); font-size: var(--size-step--2); }
.mlab-add { position: relative; }
.mlab-add summary, .mlab-recipe summary { cursor: pointer; padding: 0 var(--space-2xs); line-height: 2.25rem; }
.mlab-add summary { border: 1px dashed var(--color-muted); border-radius: var(--radius); white-space: nowrap; }
.mlab-add div { display: grid; gap: 0.25rem; padding-block-start: 0.25rem; min-inline-size: 15rem; }
.mlab-add div button { text-align: start; }
/* The stage's slot: its title beside its buttons, then room for the most controls a stage has. Five go two to
   a row, by number, so three rows hold any stage. */
.mlab-stage { grid-area: stage; display: grid; gap: ${rem(ROWS.controlGap)}; align-content: start; grid-template-columns: minmax(0, 1fr); }
.mlab-head { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 0.25rem 0.5rem; align-items: center; min-block-size: ${rem(N.head)}; }
.mlab-head p { font-weight: 600; line-height: 1.3; }
.mlab-moves { display: flex; gap: 0.25rem; }
.mlab-moves button { min-inline-size: 2.25rem; }
.mlab-ctls { display: grid; gap: ${rem(ROWS.controlGap)} 0.5rem; max-block-size: ${ctlsSlot(N.controls)}; overflow-y: auto; }
.mlab-ctl { display: grid; grid-template-columns: minmax(6.5rem, auto) minmax(0, 1fr) auto; gap: 0.5rem; align-items: center; min-block-size: ${rem(ROWS.control)}; }
.mlab-ctl > :last-child:nth-child(2) { grid-column: 2 / -1; justify-self: start; }
.mlab-ctl :is(select, input[type="number"]) { block-size: ${rem(ROWS.control)}; padding-block: 0; }
.mlab-many { grid-template-columns: 1fr 1fr; }
.mlab-many .mlab-ctl { grid-template-columns: auto minmax(0, 1fr); }
.mlab-many input[type="range"] { display: none; }
.mlab-many :is(select, input[type="number"]) { inline-size: 100%; min-inline-size: 0; }
.mlab-stage > .mlab-note { line-height: ${rem(ROWS.control)}; }
/* Walking: the pad takes the room of the step buttons, the strip and the stage's slot. */
.mlab-walk { grid-row: ctl-start / stage-end; grid-column: ctl; display: grid; gap: 0.5rem; align-content: start; justify-items: center; }
.is-walking :is(.mlab-transport, .mlab-stage, .mlab-pipe) { display: none; }
.is-walking .mlab-legend { visibility: hidden; }
.mlab-key { justify-self: stretch; display: grid; gap: 0.125rem; line-height: 1.4; }
.mlab-key i { display: inline-block; inline-size: 0.7em; block-size: 0.7em; margin-inline-end: 0.4em; border: 1px solid var(--color-rule); }
.mlab-pad { display: grid; grid-template-areas: ". up ." "left down right"; grid-template-columns: repeat(3, 2.75rem); gap: 0.25rem; justify-content: center; }
.mlab-pad button { min-block-size: 2.75rem; }
.mlab-up { grid-area: up; } .mlab-left { grid-area: left; } .mlab-down { grid-area: down; } .mlab-right { grid-area: right; }

/* What is on the map, in words, and what to do with the map. The slot for the words is sized for the
   longest step's text at each width, so nothing under it moves as the build is stepped through. */
.mlab-mid { display: grid; grid-template-columns: minmax(0, 1fr); gap: ${rem(ROWS.gap)} 1rem; }
.mlab-says { display: grid; grid-template-columns: minmax(0, 1fr); min-block-size: ${SAYS_NARROW}; }
.mlab-step { padding: 0.5rem; border-inline-start: 4px solid var(--color-primary); background: var(--color-paper); line-height: 1.4; }
.mlab-n { display: block; color: var(--color-muted); font-weight: 600; }
.mlab-error { padding: 0.5rem; border-inline-start: 4px solid var(--color-red); background: var(--color-paper); font-family: var(--font-code); line-height: 1.4; overflow-wrap: anywhere; }
.has-error > .mlab-step, .is-walking .mlab-says > :not(:nth-child(2)) { display: none; }
.mlab.is-walking .mlab-says > .mlab-step:nth-child(2) { display: block; }
.mlab-foot { display: flex; align-items: center; min-block-size: 2.25rem; }
.is-lab .mlab-foot { display: grid; grid-template-columns: auto minmax(0, 1fr); grid-template-areas: "walk note" "measure measure" "from from"; grid-template-rows: ${actionRows(N)}; gap: ${rem(ROWS.actionGap)}; align-content: start; min-block-size: ${rem(actionsSlot(N))}; }
.mlab-foot > button { justify-self: start; white-space: nowrap; }
.mlab-foot > button:nth-of-type(-n + 2) { grid-area: walk; }
.mlab-foot > .mlab-note { grid-area: note; font-size: var(--size-step--2); line-height: 1.3; }
.mlab-foot > button:nth-of-type(3) { grid-area: measure; }
.mlab-from { grid-area: from; display: flex; gap: 0.5rem; align-items: center; white-space: nowrap; }
.mlab-from select { flex: 1; min-inline-size: 0; block-size: 2.25rem; padding-block: 0; }

/* The last part: the readout, a cell for each trait this map has, three to a row with the terrain's shares
   on a row of their own; what fails, on this map and over the seeds measured; and the recipe's line. They
   fill the part from the top, and the part has room for every trait the core reports, so a map with more
   traits to show moves the recipe's line down and nothing else. */
.mlab-last { display: grid; grid-template-columns: minmax(0, 1fr); gap: ${rem(ROWS.traitGap)}; align-content: start; min-block-size: ${rem(endSlot(N))}; }
.mlab-readout { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); grid-auto-rows: minmax(${rem(ROWS.trait)}, auto); gap: ${rem(ROWS.traitGap)} 0.75rem; }
.mlab-readout > div { min-inline-size: 0; container-type: inline-size; }
.mlab-terrain { grid-column-start: 1; }
.mlab-readout dt { color: var(--color-muted); font-size: var(--size-step--2); line-height: 1.3; white-space: nowrap; }
.mlab-readout dd { margin: 0; font-variant-numeric: tabular-nums; line-height: 1.4; white-space: nowrap; }
.mlab-med { display: none; }
@container (inline-size >= 7.5rem) { .mlab-med { display: inline; } }
/* A trait's scale, with this map's value marked on it; measured, the bar is where 8 of 10 seeds fall and the dot their median. */
.mlab-mark { position: relative; display: block; max-inline-size: 8rem; block-size: 12px; }
.mlab-mark::before { content: ''; position: absolute; inset-inline: 0; inset-block-start: 5px; block-size: 2px; background: var(--color-rule); }
.mlab-mark em { position: absolute; inset-block-start: 3px; inset-inline-start: calc(var(--lo) * 100%); inline-size: max(2px, calc((var(--hi) - var(--lo)) * 100%)); block-size: 6px; border-radius: 3px; background: color-mix(in oklab, var(--color-primary) 85%, var(--color-surface)); }
.mlab-mark i { position: absolute; inset-block-start: 2px; inset-inline-start: calc(var(--mid) * 100%); inline-size: 8px; block-size: 8px; margin-inline-start: -4px; border-radius: 50%; background: var(--color-ink); box-shadow: 0 0 0 2px var(--color-surface); }
.mlab-mark b { position: absolute; inset-block-start: 0; inset-inline-start: calc(var(--now) * 100%); inline-size: 3px; block-size: 12px; margin-inline-start: -1.5px; border-radius: 1.5px; background: var(--color-focus); box-shadow: 0 0 0 1.5px var(--color-surface); }
.mlab-fails { line-height: 1.4; }
/* A card's step-through is opened by its reader and reserves nothing: its last part is as tall as what it shows. */
.mlab:not(.is-lab) :is(.mlab-last, .mlab-head, .mlab-head p) { min-block-size: 0; }
.mlab-k { color: var(--color-muted); }

/* The recipe's text opens under its summary; the link to this recipe is copied from the same line. */
.mlab-recipe { position: relative; min-block-size: ${rem(ROWS.recipe)}; }
.mlab-share { position: absolute; inset-block-start: 0; inset-inline-end: 0; }
.mlab-recipe textarea { display: block; inline-size: 100%; margin-block: 0.5rem; font-family: var(--font-code); font-size: var(--size-step--2); }

/* One column with room: the actions in two rows, the traits five to a row. */
@container (inline-size >= ${M.at}rem) {
  .is-lab .mlab-foot { grid-template-columns: auto minmax(0, 1fr) auto; grid-template-areas: "walk note measure" "from from from"; grid-template-rows: ${actionRows(M)}; min-block-size: ${rem(actionsSlot(M))}; }
  .mlab-last { min-block-size: ${rem(endSlot(M))}; }
  .mlab-readout { grid-template-columns: repeat(5, minmax(0, 1fr)); }
  .mlab-terrain { grid-column-start: auto; }
}
/* Two columns: the step buttons and the stage's slot beside the map, the actions beside the step's text. */
@container (inline-size >= ${W.at}rem) {
  .mlab-fit { --mlab-map: ${MAP_WIDE}; grid-template-columns: ${ROWS.map * 100}cqi minmax(0, 1fr); grid-template-areas: "pipe pipe" "view ctl" "view stage"; grid-template-rows: minmax(${rem(ROWS.strip)}, auto) minmax(${rem(W.transport)}, auto) minmax(${rem(stageSlot(W))}, 1fr); }
  .mlab-fit.has-demos { --mlab-map: ${MAP_WIDE}; grid-template-areas: "demos demos" "pipe pipe" "view ctl" "view stage"; grid-template-rows: minmax(${rem(DEMOS_ROW)}, auto) minmax(${rem(ROWS.strip)}, auto) minmax(${rem(W.transport)}, auto) minmax(${rem(stageSlot(W))}, 1fr); }
  .mlab-transport { grid-template-columns: repeat(4, auto) minmax(0, 1fr); align-content: start; }
  .mlab-end { display: block; }
  .mlab-play { grid-row: 2; grid-column: 1 / span 2; }
  .mlab-transport input { grid-row: 2; grid-column: 3 / -1; }
  .mlab-head { grid-template-columns: minmax(0, 1fr); min-block-size: ${rem(W.head)}; align-content: start; }
  .mlab-head p { min-block-size: 1.5rem; }
  .mlab-ctls { max-block-size: ${ctlsSlot(W.controls)}; }
  .mlab-many { grid-template-columns: minmax(0, 1fr); }
  .mlab-many .mlab-ctl { grid-template-columns: minmax(6.5rem, auto) minmax(0, 1fr); }
  .mlab-many :is(select, input[type="number"]) { inline-size: 6.5rem; max-inline-size: 100%; justify-self: start; }
  .is-walking .mlab-pipe { display: flex; opacity: 0.45; }
  .mlab-mid { grid-template-columns: ${ROWS.map * 100}cqi minmax(0, 1fr); }
  .mlab-says { min-block-size: ${SAYS_WIDE}; }
  .is-lab .mlab-foot { grid-template-columns: auto minmax(0, 1fr); grid-template-areas: "walk note" "measure measure" "from from"; grid-template-rows: ${actionRows(W)}; min-block-size: ${rem(actionsSlot(W))}; }
  .mlab-last { min-block-size: ${rem(endSlot(W))}; }
}

/* Closed: the fallback's last step, box for box (the site's .step: its picture, its label, its text), so the
   swap moves nothing. The button lies over the map and takes no room of its own. */
.mlab-open { display: none; }
.mlab.is-closed, .is-closed :is(.mlab-fit, .mlab-view, .mlab-map, .mlab-mid, .mlab-says) { display: block; min-block-size: 0; block-size: auto; }
.is-closed .mlab-view { position: relative; inline-size: min(100%, 480px); }
.is-closed :is(.mlab-demos, .mlab-pipe, .mlab-error, .mlab-legend, .mlab-transport, .mlab-stage, .mlab-walk, .mlab-last, .mlab-foot, .mlab-at) { display: none; }
.is-closed canvas { inline-size: 480px; block-size: auto; max-block-size: none; cursor: pointer; }
.is-closed .mlab-step { margin-block-start: var(--space-3xs); padding: 0; border: 0; background: none; }
.is-closed .mlab-n { margin-block-end: var(--space-3xs); }
.mlab.is-closed .mlab-open { display: flex; align-items: center; gap: var(--space-2xs); position: absolute; inset-block-end: var(--space-s); inset-inline-start: 50%; translate: -50% 0; min-block-size: 2.75rem; padding: var(--space-2xs) var(--space-s); font-weight: 600; white-space: nowrap; box-shadow: 0 0 0 2px var(--color-surface), 0 2px 10px rgb(0 0 0 / 0.4); }
.mlab-tri { inline-size: 0; block-size: 0; border-block: 0.4em solid transparent; border-inline-start: 0.65em solid currentColor; }
`;
document.head.append(style);
