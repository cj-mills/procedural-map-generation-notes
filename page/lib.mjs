// What the page's generator and the bundle's build share: the measurement data, the core's
// command line, the names the page gives things, every value the page states from our own runs,
// and the step figures.
import { execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { crc32, deflateSync } from 'node:zlib';

export const root = dirname(dirname(fileURLToPath(import.meta.url)));
const lab = join(root, 'lab');
const json = (p) => JSON.parse(readFileSync(join(root, p), 'utf8'));

export const traits = json('lab/data/traits.json');
export const faith = Object.fromEntries(json('lab/data/faithfulness.json').demos.map((d) => [d.id, d]));
export const faithTally = json('lab/data/faithfulness.json').tally;
/** What the talk's own code does where reading it leaves that open, counted over the same seeds (faithfulness/, `checks`). */
export const checks = json('faithfulness/talk-checks.json');
/** The talk's own code measured over the same seeds, a summary per demo and trait as in traits.json (faithfulness/, `measure`). */
const talkTraits = json('faithfulness/talk-traits.json');
export const DEMOS = Object.fromEntries(traits.demos.map((d) => [d.id, d]));
export const N = traits.seeds.to - traits.seeds.from; // seeds measured a demo

// ---------------------------------------------------------------- the core's command line
let built = false;
/** Runs `map-lab` (lab/cli), building it first with the repo's pinned toolchain. */
export function cli(...args) {
  const bin = join(lab, 'target/fast/map-lab');
  if (!built) execFileSync('cargo', ['build', '--quiet', '--profile', 'fast', '-p', 'map-lab'], { cwd: lab, stdio: ['ignore', 'inherit', 'inherit'] });
  built = true;
  return execFileSync(bin, args, { cwd: lab, encoding: 'utf8', maxBuffer: 1 << 28 });
}
export const stageTable = () => JSON.parse(cli('stages'));
const cache = new Map();
/** A recipe built with every frame: the core's own JSON. */
export function build(recipe, seed = recipe.seed) {
  const key = JSON.stringify([recipe.pipeline, seed, recipe.width, recipe.height]);
  if (!cache.has(key)) {
    const dir = mkdtempSync(join(tmpdir(), 'map-lab-'));
    const file = join(dir, 'recipe.json');
    writeFileSync(file, JSON.stringify(recipe));
    try {
      cache.set(key, JSON.parse(cli('build', file, '--seed', String(seed), '--json', '--frames', '--path')));
    } finally {
      rmSync(dir, { recursive: true });
    }
  }
  return cache.get(key);
}

// ---------------------------------------------------------------- the page's names
export const GROUPS = [
  { title: 'Generators', kind: 'Generator', stages: ['rooms', 'bsp', 'cellular', 'drunkard', 'dla', 'voronoi', 'noise'] },
  { title: 'Shaping and combining', kind: 'Shaping', stages: ['dla-erode', 'combine', 'prefab'] },
  { title: 'Analysis and placement', kind: 'Analysis', stages: ['cull', 'start', 'exit', 'hot-path', 'hot-rooms'] },
];
/** The card each of the talk's demos belongs to: the stage it is a setting of. */
export const SUBJECT = {
  'c01-rooms': 'rooms', 'c02-bsprooms': 'bsp', 'c03-cellular': 'cellular', 'c04-drunkard': 'drunkard',
  'c05-dla': 'dla', 'c06-dla-attractor': 'dla', 'c07-dla-symmetry': 'dla', 'c08-dla-erosion': 'dla-erode',
  'c08a-voronoi': 'voronoi', 'c09-combination': 'combine', 'c10-perlin': 'noise', 'c11-overworld': 'noise',
  'c12-noisezoom': 'noise', 'c13-double-noise': 'noise', 'c14-not-a-trap': 'prefab', 'c15-still-not-a-trap': 'prefab',
  'c16-unreachable': 'cull', 'c17-starting-left': 'start', 'c18-ending-right': 'exit', 'c19-ending-dijkstra': 'exit',
  'c20-hot-path': 'hot-path', 'c21-hot-rooms': 'hot-rooms',
};
/** A short name for each demo: a row's head, a button's label. */
export const SHORT = {
  'c01-rooms': 'Random rooms', 'c02-bsprooms': 'BSP rooms', 'c03-cellular': 'Cellular automata', 'c04-drunkard': 'Drunkard’s walk',
  'c05-dla': 'DLA, walking', 'c06-dla-attractor': 'DLA, attractor', 'c07-dla-symmetry': 'DLA, symmetric',
  'c08-dla-erosion': 'Rooms, eroded', 'c08a-voronoi': 'Voronoi cells', 'c09-combination': 'BSP + caves + spine',
  'c10-perlin': 'Noise: height map', 'c11-overworld': 'Noise: overworld', 'c12-noisezoom': 'Noise: zoom', 'c13-double-noise': 'Noise: double',
  'c14-not-a-trap': 'Vault in a room', 'c15-still-not-a-trap': 'Vault on open floor', 'c16-unreachable': 'Caves, unreachable marked',
  'c17-starting-left': 'Caves, culled', 'c18-ending-right': 'Caves, exit at right', 'c19-ending-dijkstra': 'Caves, exit farthest',
  'c20-hot-path': 'Caves, hot path cull', 'c21-hot-rooms': 'Rooms, hot rooms',
};
export const short = (id) => id.split('-')[0];
/** Recipes of our own beside the talk's demos (lab/recipes/own/): each shown in a card, and none in the comparison. */
export const OWN = {
  roads: { file: 'recipes/own/voronoi-roads.json', label: 'Roads (our rule)' },
  nocull: { file: 'recipes/own/exit-without-cull.json', label: 'Exit without a cull (ours)' },
};
for (const [id, o] of Object.entries(OWN)) {
  o.recipe = json(`lab/${o.file}`);
  SHORT[id] = o.label;
}
const ownRows = new Map();
/** One of our own recipes measured over the same seeds as the demos, by the core's command line. */
function ownTraits(id) {
  if (!ownRows.has(id)) ownRows.set(id, cli('traits', OWN[id].file, '--seeds', `${traits.seeds.from}..${traits.seeds.to}`).trim().split('\n').map((l) => JSON.parse(l)));
  return ownRows.get(id);
}
export const TRAITS = [
  { key: 'floor_share', label: 'Floor share', digits: 3 },
  { key: 'regions', label: 'Regions', digits: 0 },
  { key: 'dead_ends', label: 'Dead ends', digits: 0 },
  { key: 'corridor_ratio', label: 'Corridor ratio', digits: 3 },
  { key: 'path', label: 'Path', digits: 0 },
];
export const EXTRA_TRAITS = [
  { key: 'water', label: 'Water', digits: 3, extra: true },
  { key: 'grass', label: 'Grass', digits: 3, extra: true },
  { key: 'mountain', label: 'Mountain', digits: 3, extra: true },
  { key: 'rooms', label: 'Rooms placed', digits: 0, extra: true },
];
export const FAIL_NAMES = {
  'prefab-no-place': 'No place for the prefab', 'exit-unreachable': 'Exit unreachable', 'no-floor': 'No floor',
  'guard-hit': 'A loop hit our cap', 'disconnected': 'Not one region', 'no-path': 'No path', 'short-path': 'Short path', 'few-rooms': 'Few rooms',
};
/** One scale a trait for every mark on the page: shares run to 1, the corridor ratio to 0.4, counts to the ten above the highest seen. */
export const SCALE = { floor_share: 1, corridor_ratio: 0.4, water: 1, grass: 1, mountain: 1 };
for (const { key } of [...TRAITS, ...EXTRA_TRAITS]) {
  if (key in SCALE) continue;
  const top = Math.max(...traits.demos.map((d) => d.summary[key].max ?? 0));
  SCALE[key] = 10 * (Math.floor(top / 10) + 1);
}
const digitsOf = Object.fromEntries([...TRAITS, ...EXTRA_TRAITS].map((t) => [t.key, t.digits]));
/** A trait's value as the page prints it. */
export function fmt(v, key) {
  if (v === null || v === undefined) return '–';
  if (digitsOf[key]) return v.toFixed(digitsOf[key]);
  return Number.isInteger(v) ? String(v) : String(Math.round(v * 10) / 10);
}
/** The failures a demo's seeds show, as [key, seeds]: our cap's own flag is left out where the prefab's lists the same seeds. */
export function failuresOf(d) {
  const f = d.failingSeeds;
  return Object.entries(f).filter(([k, seeds]) => !(k === 'guard-hit' && JSON.stringify(seeds) === JSON.stringify(f['prefab-no-place'])));
}

// ---------------------------------------------------------------- every value the page states from our own runs
/** The text of a value an {% own-run %} names, from the data files. Throws on a name it doesn't know. */
export function value(name) {
  const [who, what, which] = name.split('.');
  if (OWN[who] && what === 'fails') return String(ownTraits(who).filter((r) => r.failures.includes(which)).length);
  if (OWN[who] && /^is\d+$/.test(which ?? '')) return String(ownTraits(who).filter((r) => r[what] === Number(which.slice(2))).length);
  const d = traits.demos.find((x) => short(x.id) === who);
  if (d && what === 'fails') return String((d.failingSeeds[which] ?? []).length);
  if (d && what === 'talkfails') return String(Math.round(faith[d.id].failureRates[which][1] * faith[d.id].seeds[1]));
  if (d && what === 'unfinished') return String(faith[d.id].unfinished[which === 'talk' ? 1 : 0]);
  // c18.astar.longer: the seeds on which the talk's own code drew an A* route longer than the shortest walk on its map.
  if (d && what === 'astar' && Number.isInteger(faith[d.id].astarVsShortest?.[which])) return String(faith[d.id].astarVsShortest[which]);
  if (d && d.summary[what] && which in d.summary[what]) return fmt(d.summary[what][which], which === 'n' || which === 'nulls' ? 'n' : what);
  if (d && what === 'faith' && faith[d.id].traits[which]) return faith[d.id].traits[which].ours[0].toFixed(3);
  if (d && what === 'talk' && faith[d.id].traits[which]) return faith[d.id].traits[which].talk[0].toFixed(3);
  // talk.c21.crossedGray is a count from the talk's own code; talk.c05.dugNothing/particles is one count as a share of another.
  if (who === 'talk' && checks[what]) {
    const [a, b] = (which ?? '').split('/').map((k) => checks[what][k]);
    if (Number.isInteger(a) && b === undefined) return a.toLocaleString('en-US');
    if (Number.isInteger(a) && Number.isInteger(b)) return `${((100 * a) / b).toFixed((100 * a) / b < 10 ? 1 : 0)}%`;
  }
  // talk.c02.rooms.median is a trait of the talk's own code over the seeds, as c02.rooms.median is of ours.
  if (who === 'talk') {
    const [, , trait, stat] = name.split('.');
    const s = talkTraits.demos.find((x) => short(x.id) === what)?.summary[trait];
    if (s && stat in s) return fmt(s[stat], stat === 'n' || stat === 'nulls' ? 'n' : trait);
  }
  if (who === 'faith' && what === 'compared') return String(faithTally.traits);
  if (who === 'faith' && what === 'consistent') return String(faithTally.traits - faithTally.differ);
  if (who === 'seeds') return String(what === 'last' ? traits.seeds.to - 1 : N);
  throw new Error(`no value named ${name}`);
}

// ---------------------------------------------------------------- colors: the site's tokens, mixed as the module mixes them
export const tokens = json('page/tokens.json');
const hex = (h) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));
const toLinear = (c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
const toGamma = (c) => (c <= 0.0031308 ? c * 12.92 : 1.055 * c ** (1 / 2.4) - 0.055);
function oklab([r, g, b]) {
  [r, g, b] = [r, g, b].map((v) => toLinear(v / 255));
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return [0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s, 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s, 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s];
}
function fromOklab([L, a, b]) {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s, -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s, -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s]
    .map((v) => Math.round(255 * Math.min(1, Math.max(0, toGamma(v)))));
}
/** CSS's color-mix(in oklab, a pct%, b). */
const mix = (a, pct, b) => fromOklab(oklab(a).map((v, i) => (v * pct) / 100 + (oklab(b)[i] * (100 - pct)) / 100));
/** How strongly a step's new tiles are tinted: the module's NEW_TINT, which the build checks this against. */
export const NEW_TINT = 0.6;
export function palette(theme) {
  const c = Object.fromEntries(Object.entries(tokens.color).map(([k, v]) => [k, hex(v[theme])]));
  const light = hex(tokens.figure.light[theme]), dark = hex(tokens.figure.dark[theme]);
  const tint = (name, pct) => mix(c[name], pct, light);
  return {
    tiles: [dark, light, c.blue, tint('green', 65), mix(c.muted, 70, dark), c.yellow, c.red],
    marks: { 1: c.primary, 2: c.red, 3: c.magenta, 4: c.green, 5: c.yellow, 6: c.primary, 7: tint('yellow', 55), 8: tint('red', 65), 9: tint('green', 60), 10: tint('muted', 40), 11: c.magenta },
    groups: ['blue', 'green', 'yellow', 'magenta', 'cyan', 'red'].map((n) => tint(n, 75)),
    field: Array.from({ length: 33 }, (_, i) => mix(light, Math.round((i / 32) * 100), dark)),
  };
}

// ---------------------------------------------------------------- step figures
export const SCALE_PX = 6; // pixels a tile: drawn larger than a step shows, since a figure is never scaled up

/** A PNG of 8-bit RGB pixels, or of RGBA with `channels` 4. */
function png(width, height, pixels, channels = 3) {
  const chunk = (type, data) => {
    const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
    const out = Buffer.alloc(body.length + 8);
    out.writeUInt32BE(data.length, 0);
    body.copy(out, 4);
    out.writeUInt32BE(crc32(body), body.length + 4);
    return out;
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr.set([8, channels === 4 ? 6 : 2, 0, 0, 0], 8);
  const raw = Buffer.alloc((width * channels + 1) * height);
  for (let y = 0; y < height; y++) pixels.copy(raw, y * (width * channels + 1) + 1, y * width * channels, (y + 1) * width * channels);
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk('IHDR', ihdr), chunk('IDAT', deflateSync(raw, { level: 9 })), chunk('IEND', Buffer.alloc(0))]);
}

/** The map at frame `at` of a build, as the module draws it, as a PNG for one theme. */
export function figure(b, at, theme) {
  const { width: w, height: h } = b, frames = b.frames.list, P = palette(theme), S = SCALE_PX;
  const grid = new Uint8Array(w * h);
  for (let i = 0; i <= at; i++) for (const [cell, t] of frames[i].changes) grid[cell] = t;
  const f = frames[at];
  const rgb = Buffer.alloc(w * S * h * S * 3);
  const fill = (cell, color, inset = 0) => {
    const x0 = (cell % w) * S, y0 = Math.floor(cell / w) * S;
    for (let y = y0 + inset; y < y0 + S - inset; y++) for (let x = x0 + inset; x < x0 + S - inset; x++) rgb.set(color, (y * w * S + x) * 3);
  };
  // A noise field is drawn as brightness only where the map hasn't made terrain of it, as the module draws it.
  const base = (cell) => (f.field?.length && grid[cell] < 2 ? P.field[Math.round((f.field[cell] / 255) * 32)] : P.tiles[grid[cell]]);
  for (let cell = 0; cell < w * h; cell++) fill(cell, base(cell));
  for (const [cell, m] of f.marks ?? []) {
    const color = m >= 32 ? P.groups[(m - 32) % P.groups.length] : P.marks[m];
    // What a step has just made is tinted, not covered: source-over in sRGB, as a canvas blends.
    fill(cell, m === 1 ? color.map((v, i) => Math.round(v * NEW_TINT + base(cell)[i] * (1 - NEW_TINT))) : color);
  }
  if (at === frames.length - 1 && b.start) {
    // The finished level: its path, start and exit, as the module draws them on the last step.
    const rows = b.tiles;
    for (let cell = 0; cell < w * h; cell++) if (rows[Math.floor(cell / w)][cell % w] === '*') fill(cell, P.marks[6], Math.round((2 * S) / 8));
    fill(b.start[1] * w + b.start[0], P.marks[4]);
    if (b.exit) fill(b.exit[1] * w + b.exit[0], P.marks[5]);
  }
  return png(w * S, h * S, rgb);
}

/** The digits a diagram prints on a tile, 3 by 5, a row a string. */
const DIGITS = ['111101101101111', '010110010010111', '111001111100111', '111001111001111', '101101111001001', '111100111001111', '111100111101111', '111001001001001', '111101111101111', '111101111001111'];
/** How a diagram's `key` may draw a tile that carries a number. */
const NUMBERED = ['floor', 'wall', 'route', 'hot', 'point', 'count'];

/** A rule's diagram: a few tiles drawn large from rows of characters, for one theme.
 *    `#` solid   `.` open   `_` nothing: a gap between two drawings
 *    `o` where a particle starts   `*` a tile it passes   `+` a tile newly opened   `x` a tile rejected
 *    `u` an open tile that can't be reached
 *    `W` `F` the tile a rule has just decided, now wall or floor
 *    `a`-`d` a tile of one cell or rectangle, by tint   `A`-`D` the same, on the cell's edge
 *    `0`-`9` a number on an open tile, or on the tile its `key` names: { 0: 'route', 1: 'hot', '4+': 'wall' }
 *  The marks differ in size and shape as well as color, so they read without color. */
export function diagram(rows, theme, key = {}) {
  const P = palette(theme), T = 24, w = rows[0].length, h = rows.length;
  const line = P.tiles[4]; // between tiles: the muted wall tone
  const px = Buffer.alloc(w * T * h * T * 4);
  const fill = (x, y, color, inset = 0) => {
    for (let py = y * T + inset; py < (y + 1) * T - inset; py++) for (let qx = x * T + inset; qx < (x + 1) * T - inset; qx++) px.set([...color, 255], (py * w * T + qx) * 4);
  };
  const ring = (x, y, base) => (fill(x, y, P.marks[1], 1), fill(x, y, base, 4));
  const light = ([r, g, b]) => 0.2126 * toLinear(r / 255) + 0.7152 * toLinear(g / 255) + 0.0722 * toLinear(b / 255) > 0.3;
  const digit = (x, y, n, on) => {
    const ink = light(on) ? P.tiles[0] : P.tiles[1];
    [...DIGITS[n]].forEach((bit, i) => {
      if (bit !== '1') return;
      const x0 = x * T + 8 + (i % 3) * 3, y0 = y * T + 5 + Math.floor(i / 3) * 3;
      for (let py = y0; py < y0 + 3; py++) for (let qx = x0; qx < x0 + 3; qx++) px.set([...ink, 255], (py * w * T + qx) * 4);
    });
  };
  const styleOf = (n) => key[n] ?? Object.entries(key).find(([k]) => k.endsWith('+') && n >= Number(k.slice(0, -1)))?.[1] ?? 'floor';
  const kinds = { '#': [P.tiles[0]], '.': [P.tiles[1]], 'o': [P.tiles[0], P.marks[3], 4], '*': [P.tiles[0], P.marks[7], 8], '+': [P.tiles[1], P.marks[1], 3], 'x': [P.tiles[0], P.marks[2], 6], 'u': [P.tiles[1], P.marks[2], 5] };
  rows.forEach((row, y) => [...row].forEach((ch, x) => {
    if (ch === '_') return;
    fill(x, y, line);
    if (kinds[ch]) {
      fill(x, y, kinds[ch][0], 1);
      if (kinds[ch][1]) fill(x, y, kinds[ch][1], kinds[ch][2]);
    } else if (ch === 'W' || ch === 'F') ring(x, y, P.tiles[ch === 'W' ? 0 : 1]);
    else if (/[a-d]/i.test(ch)) {
      fill(x, y, P.groups['abcd'.indexOf(ch.toLowerCase())], 1);
      if (ch < 'a') fill(x, y, P.tiles[0], 7);
    } else if (/\d/.test(ch)) {
      const style = styleOf(Number(ch));
      if (!NUMBERED.includes(style)) throw new Error(`a rule's diagram has no numbered tile "${style}"`);
      const base = { floor: P.tiles[1], wall: P.tiles[0], route: P.marks[6], hot: P.marks[7], point: P.marks[3], count: line }[style];
      if (style === 'count') ring(x, y, base);
      else fill(x, y, base, 1);
      digit(x, y, Number(ch), base);
    } else throw new Error(`a rule's diagram has no tile "${ch}"`);
  }));
  return png(w * T, h * T, px, 4);
}

/** Which frames of a run a step list shows: of the frames in `among`, each kind of step's first and
 *  last, thinned to `limit`, then `mid` more from the middles of the longest runs between them. */
export function pickFrames(frames, among, limit, mid = 0) {
  const first = new Map(), last = new Map();
  for (const i of among) {
    if (!first.has(frames[i].note)) first.set(frames[i].note, i);
    last.set(frames[i].note, i);
  }
  // A kind's first step and the run's last always stay.
  const must = new Set([...first.values(), among.at(-1)]);
  const keep = [...new Set([...must, ...last.values()])].sort((a, b) => a - b);
  while (keep.length > limit) {
    const gaps = keep.map((k, n) => (must.has(k) ? Infinity : keep[n + 1] - keep[n - 1]));
    const n = gaps.indexOf(Math.min(...gaps));
    if (gaps[n] === Infinity) break;
    keep.splice(n, 1);
  }
  for (; mid > 0; mid--) {
    const gaps = keep.slice(1).map((k, n) => k - keep[n]);
    const n = gaps.indexOf(Math.max(...gaps));
    if (!(gaps[n] > 1)) break;
    keep.splice(n + 1, 0, keep[n] + Math.floor(gaps[n] / 2));
  }
  return keep;
}

/** A recipe as the page links it: #recipe= and its JSON, base64url-encoded, as the module reads it. */
export function recipeLink(recipe, seed = recipe.seed) {
  // The seed goes last, so links to one pipeline on different seeds share all but their ends, and
  // the map's size is left to the core's default when it is the default.
  const size = recipe.width === 80 && recipe.height === 50 ? {} : { width: recipe.width, height: recipe.height };
  const r = { format: 'map-lab-recipe/1', ...size, pipeline: recipe.pipeline, seed };
  return `#recipe=${Buffer.from(JSON.stringify(r)).toString('base64url')}`;
}
export const cleanRecipe = (r) => ({ format: r.format, seed: r.seed, width: r.width, height: r.height, pipeline: r.pipeline });
export function flatStages(pipeline, out = []) {
  for (const s of pipeline) {
    out.push(s.stage);
    for (const input of s.inputs ?? []) flatStages(input, out);
  }
  return out;
}
export const pipelineWords = (pipeline) => pipeline.map((s) => (s.stage === 'combine' ? `combine(${s.inputs.map(pipelineWords).join(' / ')})` : s.stage)).join(' → ');
export const siteTokens = (site) => (existsSync(join(site, 'src/styles/tokens.json')) ? JSON.parse(readFileSync(join(site, 'src/styles/tokens.json'), 'utf8')) : null);
