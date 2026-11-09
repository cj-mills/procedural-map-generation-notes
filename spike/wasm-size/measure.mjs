// Builds the spike with each family turned off in turn and measures the WebAssembly module and
// its wasm-bindgen glue, gzipped the way the site's tools/check-perf.ts counts an interactive
// module: Node's gzipSync at its default level, on the file as written.
//
//   node measure.mjs            prints the table and writes sizes.json
//
// Needs the repo's pinned tools (rust-toolchain.toml, mise.toml): cargo with the wasm32 target,
// wasm-bindgen and wasm-opt.
import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { gzipSync } from 'node:zlib';
import { join } from 'node:path';

const here = import.meta.dirname;
const FAMILIES = ['rooms', 'bsp', 'cellular', 'drunkard', 'dla', 'voronoi', 'noise', 'prefab', 'combine', 'analysis'];
const KB = 1024;
const LIMIT = 50 * KB;

const run = (cmd, args) => execFileSync(cmd, args, { cwd: here, stdio: ['ignore', 'pipe', 'inherit'] }).toString();
const version = (cmd) => run(cmd, ['--version']).trim().split('\n')[0];

function measure(name, features) {
  run('cargo', ['build', '--lib', '--release', '--target', 'wasm32-unknown-unknown', '--no-default-features', '--features', features.join(',')]);
  const out = join(here, 'target', 'size', name);
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  const raw = join(here, 'target', 'wasm32-unknown-unknown', 'release', 'mapgen_size_spike.wasm');
  run('wasm-bindgen', ['--target', 'web', '--out-dir', out, '--no-typescript', raw]);
  const bg = join(out, 'mapgen_size_spike_bg.wasm');
  run('wasm-opt', ['-Oz', '--strip-debug', '--strip-producers', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals', '--enable-reference-types', '--enable-multivalue', bg, '-o', bg]);
  const wasm = readFileSync(bg);
  const glue = readFileSync(join(out, 'mapgen_size_spike.js'));
  return {
    name,
    features,
    wasm: wasm.byteLength,
    wasmGz: gzipSync(wasm).byteLength,
    glue: glue.byteLength,
    glueGz: gzipSync(glue).byteLength,
  };
}

const all = [...FAMILIES, 'wasm'];
const rows = [measure('full', all), measure('base (no families)', ['wasm']), measure('full, serde_json recipe', [...all, 'serde'])];
for (const f of FAMILIES) rows.push(measure(`without ${f}`, all.filter((x) => x !== f)));

const full = rows[0];
const kb = (n) => `${(n / KB).toFixed(1)} KB`;
const pad = (s, n) => String(s).padEnd(n);
const lines = [];
lines.push(`${pad('build', 30)}${pad('wasm', 9)}${pad('wasm gz', 10)}${pad('glue', 9)}${pad('glue gz', 10)}${pad('total gz', 10)}share gz`);
for (const r of rows) {
  const total = r.wasmGz + r.glueGz;
  const share = r.name.startsWith('without ') ? `${full.wasmGz + full.glueGz - total} B` : '';
  lines.push(`${pad(r.name, 30)}${pad(r.wasm, 9)}${pad(r.wasmGz, 10)}${pad(r.glue, 9)}${pad(r.glueGz, 10)}${pad(total, 10)}${share}`);
}
const fullTotal = full.wasmGz + full.glueGz;
lines.push('');
lines.push(`full core, wasm + glue gzipped: ${fullTotal} B (${kb(fullTotal)}) of ${kb(LIMIT)}; headroom ${LIMIT - fullTotal} B (${kb(LIMIT - fullTotal)})`);
console.log(lines.join('\n'));

writeFileSync(
  join(here, 'sizes.json'),
  JSON.stringify(
    {
      measuredWith: { rustc: version('rustc'), wasmBindgen: version('wasm-bindgen'), wasmOpt: version('wasm-opt'), node: process.version, gzip: 'node:zlib gzipSync, default level' },
      limitBytes: LIMIT,
      rows,
    },
    null,
    2,
  ) + '\n',
);
