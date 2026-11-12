// Checks the WebAssembly build (lab/target/web/, from measure-size.mjs) against the native
// build, over every recipe in lab/recipes/, lab/recipes/own/ and lab/recipes/lock/ and a range
// of seeds:
//
//   - each map, tile for tile: the same tiles hash and the same number of frames as
//     `map-lab hashes` prints, and frames that replay to the final map;
//   - each level: the same caches, the same locked door and key, and the same walk (`Lab.step`
//     against `map-lab walk`) along the shortest walk that finishes it and, with a door, along
//     the route the door refuses: the route, what the walker has seen, the caches found, the key;
//   - each seed's measured traits and failures from `Lab.measure`, taken in two slices, against
//     `map-lab traits`;
//   - with --data, `Lab.summary` over seeds 0..200 against the committed lab/data/traits.json,
//     for the talk's demos: the page's "Measure over 200 seeds" gives the data file's numbers;
//     and the lock stage's two failures against lab/data/lock.json;
//   - the stage table against `map-lab stages`;
//   - a bad recipe throws with where it is wrong.
//
//   node lab/check-wasm.mjs [seeds] [--data]     default 20 seeds per recipe, 0..20
import { execFileSync } from 'node:child_process';
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const here = import.meta.dirname;
const args = process.argv.slice(2);
const withData = args.includes('--data');
const seeds = Number(args.find((a) => !a.startsWith('--')) ?? 20);
const dir = join(here, 'target', 'web');
const glue = await import(join(dir, 'map_lab_wasm.js'));
glue.initSync({ module: readFileSync(join(dir, 'map_lab_wasm_bg.wasm')) });

const list = (sub) => readdirSync(join(here, sub)).filter((f) => f.endsWith('.json')).sort().map((f) => join(here, sub, f));
const demos = list('recipes');
const recipes = [...demos, ...list(join('recipes', 'own')), ...list(join('recipes', 'lock'))];
const idOf = (file) => file.split('/').pop().replace(/\.json$/, '');
const cli = join(here, 'target', 'fast', 'map-lab');
execFileSync('cargo', ['build', '--profile', 'fast', '-p', 'map-lab'], { cwd: here, stdio: ['ignore', 'ignore', 'inherit'] });
const run = (...a) => execFileSync(cli, a, { encoding: 'utf8', maxBuffer: 1 << 28 });

let failed = 0;
const fail = (msg) => {
  failed++;
  console.log(`FAIL ${msg}`);
};

const native = new Map();
for (const line of run('hashes', ...recipes, '--seeds', `0..${seeds}`).trim().split('\n')) {
  const [id, seed, hash, frames, caches, lock] = line.split(' ');
  native.set(`${id} ${seed}`, `${hash} ${frames} ${caches} ${lock}`);
}
const failureKeys = [];
for (let bit = 0; glue.failure_key(bit); bit++) failureKeys.push(glue.failure_key(bit));
const same = (a, b) => a === b || (a == null && Number.isNaN(b));

let maps = 0;
let walks = 0;
for (const file of recipes) {
  const recipe = JSON.parse(readFileSync(file, 'utf8'));
  const id = idOf(file);
  for (let seed = 0; seed < seeds; seed++) {
    const lab = new glue.Lab(JSON.stringify({ ...recipe, seed }));
    const got = `${lab.tiles_hash()} ${lab.frame_count()} ${lab.caches().join(',') || '-'} ${lab.door() < 0 ? '-' : `${lab.door()},${lab.key()}`}`;
    // Replaying every frame's changes from an all-wall start must give the final tiles.
    const tiles = new Uint8Array(lab.width() * lab.height()).fill(255);
    for (let i = 0; i < lab.frame_count(); i++) for (const c of lab.frame_changes(i)) tiles[c >>> 8] = c & 0xff;
    const final = lab.tiles();
    const replayed = final.every((t, i) => tiles[i] === t);
    const want = native.get(`${id} ${seed}`);
    maps++;
    if (got !== want || !replayed) fail(`${id} seed ${seed}: wasm ${got}, native ${want}${replayed ? '' : ', frames do not replay to the final map'}`);

    // The walk: along the shortest walk that finishes the level, which with a locked door goes
    // by way of the key: once with its first move made twice, so its second may be refused, and
    // once as it is, which must finish. One more move comes after the exit, which always is
    // refused. A level with a door is also walked straight along its route, where the door
    // refuses a walker who has no key.
    const path = lab.path();
    const finish = lab.finish();
    if (!lab.walk()) {
      if (finish.length) fail(`${id} seed ${seed}: a level with no walk`);
    } else {
      const w = lab.width();
      for (const [way, twice] of lab.door() < 0 ? [[finish, true], [finish, false]] : [[finish, true], [finish, false], [path, false]]) {
        lab.walk();
        const letters = Array.from(way.slice(1), (to, n) => ({ [-1]: 'L', 1: 'R', [-w]: 'U', [w]: 'D' })[to - way[n]]);
        const moves = [...letters.slice(0, twice ? 1 : 0), ...letters, 'L'];
        let taken = 0;
        for (const m of moves) taken += lab.step('LRUD'.indexOf(m));
        const mine = { steps: lab.route().length - 1, done: lab.walk_done(), found: lab.found(), key: lab.has_key(), shortest: finish.length - 1, seen: lab.seen().join('') };
        const theirs = JSON.parse(run('walk', file, '--seed', String(seed), '--moves', moves.join(''), '--json'));
        const want = { steps: theirs.steps, done: theirs.done, found: theirs.found.reduce((a, f, n) => a | (f << n), 0), key: theirs.hasKey, shortest: theirs.shortest, seen: theirs.seen.join('') };
        const route = Array.from(lab.route(), (i) => `${i % w},${Math.floor(i / w)}`).join(' ');
        if (JSON.stringify(mine) !== JSON.stringify(want) || route !== theirs.route.map((p) => p.join(',')).join(' ') || taken !== theirs.steps) fail(`${id} seed ${seed}: the walk differs from the native one`);
        if (way === finish && !twice && !mine.done) fail(`${id} seed ${seed}: the shortest finishing walk does not finish`);
        walks++;
      }
    }
    lab.free();
  }

  // The batch: two slices give the same rows as the command line, seed by seed.
  const lab = new glue.Lab(JSON.stringify(recipe));
  const first = Math.floor(seeds / 3);
  if (lab.measure(first) !== first || lab.measure(seeds - first) !== seeds) fail(`${id}: measure did not count its seeds`);
  const rows = lab.measured();
  const lines = run('traits', file, '--seeds', `0..${seeds}`).trim().split('\n').map((l) => JSON.parse(l));
  lines.forEach((want, n) => {
    const row = rows.slice(n * 13, n * 13 + 13);
    const values = Object.entries(want).filter(([k]) => k !== 'seed' && k !== 'failures').map(([, v]) => v);
    const failures = failureKeys.filter((_, bit) => row[12] & (1 << bit));
    if (!values.every((v, i) => same(v, row[i])) || failures.join() !== want.failures.join()) fail(`${id} seed ${n}: measured ${[...row]}, native ${JSON.stringify(want)}`);
  });
  lab.free();
}

if (withData) {
  // "Measure over 200 seeds" in the page against the published data, rounded as the file is.
  const data = JSON.parse(readFileSync(join(here, 'data', 'traits.json'), 'utf8'));
  const columns = Object.keys(data.columns);
  const round = (x) => Math.round(x * 1e6) / 1e6;
  const t0 = performance.now();
  for (const demo of data.demos) {
    const lab = new glue.Lab(JSON.stringify(demo.recipe));
    lab.measure(data.seeds.from);
    const total = data.seeds.to - data.seeds.from;
    // The data file's seeds start at 0, so the rows before `from` are none.
    if (data.seeds.from !== 0) throw new Error('the data file no longer starts at seed 0');
    lab.measure(total);
    const s = lab.summary();
    columns.forEach((k, c) => {
      const want = demo.summary[k];
      const [n, ...rest] = s.slice(c * 8, c * 8 + 8);
      const got = n ? { n, nulls: total - n, mean: round(rest[0]), sd: round(rest[1]), min: round(rest[2]), p10: round(rest[3]), median: round(rest[4]), p90: round(rest[5]), max: round(rest[6]) } : { n, nulls: total };
      if (JSON.stringify(got) !== JSON.stringify(want)) fail(`${demo.id} ${k}: summary ${JSON.stringify(got)}, data file ${JSON.stringify(want)}`);
    });
    failureKeys.forEach((key, bit) => {
      const want = demo.failingSeeds[key]?.length ?? 0;
      if (s[columns.length * 8 + bit] !== want) fail(`${demo.id}: ${s[columns.length * 8 + bit]} seeds fail ${key}, data file ${want}`);
    });
    lab.free();
  }
  console.log(`${data.demos.length} demos measured over seeds ${data.seeds.from}..${data.seeds.to} give the data file's summaries and failing-seed counts (${Math.round(performance.now() - t0)} ms in Node)`);
  // The lock stage's two failures, counted by the page's measure, against its own data file.
  const lock = JSON.parse(readFileSync(join(here, 'data', 'lock.json'), 'utf8'));
  for (const demo of lock.demos) {
    const lab = new glue.Lab(JSON.stringify(demo.recipe));
    lab.measure(lock.seeds.to - lock.seeds.from);
    const s = lab.summary();
    for (const key of Object.keys(lock.failures)) {
      const got = s[columns.length * 8 + failureKeys.indexOf(key)];
      if (got !== demo.failingSeeds[key].length) fail(`${demo.id}: ${got} seeds fail ${key}, lock.json ${demo.failingSeeds[key].length}`);
    }
    lab.free();
  }
  console.log(`${lock.demos.length} lock recipes measured over seeds ${lock.seeds.from}..${lock.seeds.to} give lock.json's failing-seed counts`);
}

const table = run('stages').trim();
if (glue.stage_table() !== table) fail('the stage table differs from `map-lab stages`');
else console.log(`the stage table matches the native one: ${JSON.parse(table).stages.length} stages`);

for (const bad of ['{"format": "map-lab-recipe/1", "seed": 1, "pipeline": [{"stage": "wobble"}]}', '{"seed": 1,']) {
  try {
    new glue.Lab(bad);
    fail(`a bad recipe built: ${bad}`);
  } catch (e) {
    console.log(`bad recipe throws: ${e.message}`);
  }
}
if (failed) {
  console.error(`${failed} checks failed against the native build`);
  process.exit(1);
}
console.log(`${maps} maps (${recipes.length} recipes x ${seeds} seeds) match the native build tile for tile, frame for frame, cache for cache and lock for lock, replay from their frames, and measure the same`);
console.log(`${walks} levels walk the same as the native build: the route, what was seen, the caches found, the key and the locked door`);
