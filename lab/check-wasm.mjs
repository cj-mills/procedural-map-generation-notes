// Runs every recipe in lab/recipes/ through the WebAssembly build (lab/target/web/, from
// measure-size.mjs) over a range of seeds, and checks each map against the native build, tile
// for tile: the same tiles hash and the same number of frames as `map-lab hashes` prints. Also
// checks that a bad recipe throws with where it is wrong.
//
//   node lab/check-wasm.mjs [seeds]     default 20 seeds per recipe, 0..20
import { execFileSync } from 'node:child_process';
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const here = import.meta.dirname;
const seeds = Number(process.argv[2] ?? 20);
const dir = join(here, 'target', 'web');
const glue = await import(join(dir, 'map_lab_wasm.js'));
glue.initSync({ module: readFileSync(join(dir, 'map_lab_wasm_bg.wasm')) });

const recipes = readdirSync(join(here, 'recipes')).filter((f) => f.endsWith('.json')).sort();
const cli = join(here, 'target', 'fast', 'map-lab');
execFileSync('cargo', ['build', '--profile', 'fast', '-p', 'map-lab'], { cwd: here, stdio: ['ignore', 'ignore', 'inherit'] });
const native = new Map();
const lines = execFileSync(cli, ['hashes', ...recipes.map((f) => join(here, 'recipes', f)), '--seeds', `0..${seeds}`], { encoding: 'utf8', maxBuffer: 1 << 26 });
for (const line of lines.trim().split('\n')) {
  const [id, seed, hash, frames] = line.split(' ');
  native.set(`${id} ${seed}`, `${hash} ${frames}`);
}

let failed = 0;
let checked = 0;
for (const f of recipes) {
  const recipe = JSON.parse(readFileSync(join(here, 'recipes', f), 'utf8'));
  const id = f.replace(/\.json$/, '');
  for (let seed = 0; seed < seeds; seed++) {
    const lab = new glue.Lab(JSON.stringify({ ...recipe, seed }));
    const got = `${lab.tiles_hash()} ${lab.frame_count()}`;
    // Replaying every frame's changes from an all-wall start must give the final tiles.
    const tiles = new Uint8Array(lab.width() * lab.height()).fill(255);
    for (let i = 0; i < lab.frame_count(); i++) for (const c of lab.frame_changes(i)) tiles[c >>> 8] = c & 0xff;
    const final = lab.tiles();
    const replayed = final.every((t, i) => tiles[i] === t);
    const want = native.get(`${id} ${seed}`);
    checked++;
    if (got !== want || !replayed) {
      failed++;
      console.log(`FAIL ${id} seed ${seed}: wasm ${got}, native ${want}${replayed ? '' : ', frames do not replay to the final map'}`);
    }
    lab.free();
  }
}
for (const bad of ['{"format": "map-lab-recipe/1", "seed": 1, "pipeline": [{"stage": "wobble"}]}', '{"seed": 1,']) {
  try {
    new glue.Lab(bad);
    failed++;
    console.log(`FAIL a bad recipe built: ${bad}`);
  } catch (e) {
    console.log(`bad recipe throws: ${e.message}`);
  }
}
if (failed) {
  console.error(`${failed} of ${checked} maps differ from the native build`);
  process.exit(1);
}
console.log(`${checked} maps (${recipes.length} recipes x ${seeds} seeds) match the native build tile for tile, frame for frame, and replay from their frames`);
