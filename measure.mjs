// The page's own measurements: every number marked {% own-run values="…" /%} in the page comes
// from here. build.mjs runs measure() on every build and refuses a page whose sentence does not
// state each value it names, so the prose can't drift from the code it describes.
//
//   node measure.mjs        print each value's name, text and raw number
//
// Each value has the text the page writes (`text`) and the unrounded number behind it (`raw`).
// All runs use the interactive's own functions, on its 80 × 50 map.
import { generate, noise, openShare, rockNeighbours, step } from './deliverable/assets/caves.js';

const WIDTH = 80;
const HEIGHT = 50;
const SEEDS = Array.from({ length: 50 }, (_, i) => i + 1);

const percent1 = (x) => `${(x * 100).toFixed(1)}%`;
const percent0 = (x) => `${Math.round(x * 100)}%`;
const count = (x, to = 1) => (Math.round(x / to) * to).toLocaleString('en-US');

const map = (rockPercent, seed, iterations) => generate({ width: WIDTH, height: HEIGHT, rockPercent, seed, iterations });
const changed = (a, b) => a.reduce((n, v, i) => n + (v !== b[i] ? 1 : 0), 0);
const mean = (xs) => xs.reduce((a, b) => a + b, 0) / xs.length;

/** The rule without its second half: rock only where more than four neighbours are rock. */
function stepWithoutZero(grid) {
  const out = grid.slice();
  for (let y = 1; y < HEIGHT - 1; y++) {
    for (let x = 1; x < WIDTH - 1; x++) out[y * WIDTH + x] = rockNeighbours(grid, WIDTH, x, y) > 4 ? 1 : 0;
  }
  return out;
}

/** Sizes of the open regions, tiles joined edge to edge (not diagonally). */
function regions(grid) {
  const seen = new Uint8Array(grid.length);
  const sizes = [];
  for (let start = 0; start < grid.length; start++) {
    if (grid[start] || seen[start]) continue;
    let size = 0;
    const stack = [start];
    seen[start] = 1;
    while (stack.length) {
      const i = stack.pop();
      size++;
      const x = i % WIDTH;
      const y = Math.floor(i / WIDTH);
      for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
        const nx = x + dx;
        const ny = y + dy;
        if (nx < 0 || ny < 0 || nx >= WIDTH || ny >= HEIGHT) continue;
        const j = ny * WIDTH + nx;
        if (!grid[j] && !seen[j]) {
          seen[j] = 1;
          stack.push(j);
        }
      }
    }
    sizes.push(size);
  }
  return sizes;
}

function connectivity(rockPercent) {
  const runs = SEEDS.map((seed) => regions(map(rockPercent, seed, 10)));
  return {
    largest: mean(runs.map((r) => Math.max(...r) / r.reduce((a, b) => a + b, 0))),
    regions: mean(runs.map((r) => r.length)),
    split: runs.filter((r) => r.length > 1).length,
  };
}

export function measure() {
  const values = {};
  const put = (name, raw, text) => (values[name] = { raw, text });

  for (const g of [0, 3, 10]) {
    const share = openShare(map(55, 1, g));
    put(`open.g${g}`, share, percent1(share));
  }

  const churn = mean(SEEDS.map((seed) => {
    const nine = map(55, seed, 9);
    return changed(nine, step(nine, WIDTH, HEIGHT));
  }));
  put('churn.g10', churn, count(churn, 10));

  let grid = noise(WIDTH, HEIGHT, 55, 1);
  let before = grid;
  for (let i = 0; i < 10; i++) [before, grid] = [grid, stepWithoutZero(grid)];
  const withoutZero = changed(before, grid);
  put('churn.nozero.seed1', withoutZero, count(withoutZero));

  for (const p of [55, 60, 65]) {
    const c = connectivity(p);
    put(`largest.${p}`, c.largest, percent0(c.largest));
    put(`regions.${p}`, c.regions, count(c.regions));
    put(`split.${p}`, c.split, String(c.split));
  }
  return values;
}

/**
 * Each {% own-run values="…" /%} in a Markdoc source with the sentence it closes: the text from
 * the previous tag (or the paragraph's start) up to the tag. Returns problems, empty when every
 * named value is known and stated.
 */
export function checkOwnRuns(source, values) {
  const problems = [];
  const body = source.replace(/^---\n[\s\S]*?\n---\n/, '');
  const tag = /\{%\s*own-run\b([^%]*)%\}/g;
  let m;
  while ((m = tag.exec(body))) {
    const line = body.slice(0, m.index).split('\n').length;
    const named = /values="([^"]*)"/.exec(m[1]);
    if (!named || !named[1].trim()) {
      problems.push(`line ${line}: {% own-run %} names no values; every own number is regenerated and checked`);
      continue;
    }
    const before = body.slice(0, m.index);
    const start = Math.max(before.lastIndexOf('\n\n') + 2, before.lastIndexOf('%}') + 2, 0);
    const sentence = before.slice(start);
    for (const name of named[1].split(/\s+/).filter(Boolean)) {
      const v = values[name];
      if (!v) problems.push(`line ${line}: no measurement named ${name} in measure.mjs`);
      else if (!new RegExp(`(?<![\\d.,])${v.text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}(?![\\d]|[.,]\\d)`).test(sentence))
        problems.push(`line ${line}: ${name} is ${v.text} now, which the sentence doesn't state: ${JSON.stringify(sentence.trim())}`);
    }
  }
  return problems;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  for (const [name, v] of Object.entries(measure())) console.log(`${name.padEnd(20)} ${v.text.padEnd(8)} ${v.raw}`);
}
