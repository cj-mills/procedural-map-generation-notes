// Regenerates the map lab's measurement data from the core:
//
//   lab/data/traits.json         every demo recipe over seeds 0..200 (`map-lab measure`)
//   lab/data/faithfulness.json   that against the talk's own code over the same seeds
//                                (`map-lab compare`, reading faithfulness/talk-traits.json)
//   lab/data/lock.json           the lock stage, which is our own, on its two own recipes and
//                                the pipelines in recipes/lock/, over seeds 0..200 (`map-lab lock`)
//
//   node lab/measure.mjs [--check]    --check: regenerate into lab/target/ and fail if any
//                                     file differs from the committed one
//
// faithfulness/talk-traits.json itself needs a checkout of the talk's code; see faithfulness/README.md.
import { execFileSync } from 'node:child_process';
import { mkdirSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const here = import.meta.dirname;
const check = process.argv.includes('--check');
const outDir = check ? join(here, 'target', 'data-check') : join(here, 'data');
mkdirSync(outDir, { recursive: true });
const run = (args) => execFileSync(join(here, 'target', 'fast', 'map-lab'), args, { cwd: here, stdio: ['ignore', 'inherit', 'inherit'] });
execFileSync('cargo', ['build', '--profile', 'fast', '-p', 'map-lab'], { cwd: here, stdio: ['ignore', 'ignore', 'inherit'] });
const recipes = readdirSync(join(here, 'recipes')).filter((f) => f.endsWith('.json')).sort().map((f) => `recipes/${f}`);
run(['measure', ...recipes, '--seeds', '0..200', '--out', join(outDir, 'traits.json')]);
run(['compare', check ? join(outDir, 'traits.json') : 'data/traits.json', '../faithfulness/talk-traits.json', '--out', join(outDir, 'faithfulness.json')]);
const dir = (sub, keep = () => true) => readdirSync(join(here, sub)).filter((f) => f.endsWith('.json') && keep(f)).sort().map((f) => `${sub}/${f}`);
run(['lock', ...dir('recipes/own', (f) => f.startsWith('locked-door')), ...dir('recipes/lock'), '--seeds', '0..200', '--out', join(outDir, 'lock.json')]);
if (check) {
  let bad = 0;
  for (const f of ['traits.json', 'faithfulness.json', 'lock.json']) {
    // The compare file names its inputs by path; only that line may differ between the two places.
    const norm = (s) => s.replace(/"ours": "[^"]*"/, '"ours": ""');
    const same = norm(readFileSync(join(outDir, f), 'utf8')) === norm(readFileSync(join(here, 'data', f), 'utf8'));
    console.log(`${same ? 'ok  ' : 'FAIL'} lab/data/${f} ${same ? 'regenerates byte for byte' : 'differs from a fresh run: run node lab/measure.mjs and commit it'}`);
    if (!same) bad++;
  }
  if (bad) process.exit(1);
}
