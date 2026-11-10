// Builds the core's WebAssembly binding for the page and measures it as the site will count an
// interactive: the module, its glue and its .wasm together, each file gzipped with Node's
// gzipSync at its default level, summed. Until the site's own check counts WebAssembly, this is
// the deliverable's check: it fails over 50 KB.
//
//   node lab/measure-size.mjs      builds lab/target/web/, prints the sizes, writes lab/sizes.json
//
// wasm-bindgen copies each export's doc comment into the glue. The page doesn't need them
// there, and they were about a third of the glue gzipped, so this build takes them out. They
// stay in lab/wasm/src/lib.rs and in map_lab_wasm.d.ts, which is written beside the glue for
// whoever writes the page module and is not loaded by the page.
//
// Needs the repo's pinned tools (rust-toolchain.toml, mise.toml): cargo with the wasm32 target,
// wasm-bindgen (the crate's exact version) and wasm-opt.
import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { gzipSync } from 'node:zlib';

const here = import.meta.dirname;
const LIMIT = 50 * 1024;
const run = (cmd, args) => execFileSync(cmd, args, { cwd: here, stdio: ['ignore', 'pipe', 'inherit'] }).toString();
const version = (cmd) => run(cmd, ['--version']).trim().split('\n')[0];

run('cargo', ['build', '--release', '--target', 'wasm32-unknown-unknown', '-p', 'map-lab-wasm']);
const out = join(here, 'target', 'web');
rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
run('wasm-bindgen', ['--target', 'web', '--out-dir', out, join(here, 'target', 'wasm32-unknown-unknown', 'release', 'map_lab_wasm.wasm')]);
const gluePath = join(out, 'map_lab_wasm.js');
const documented = readFileSync(gluePath, 'utf8');
const glue = documented.replace(/^[ \t]*\/\*[\s\S]*?\*\/\n/gm, '');
// Only comments on lines of their own may go: if the glue ever holds one anywhere else, stop.
if (glue.includes('/*') || glue.includes('*/')) throw new Error('the glue has a comment this build does not know how to remove');
writeFileSync(gluePath, glue);
const wasmPath = join(out, 'map_lab_wasm_bg.wasm');
run('wasm-opt', ['-Oz', '--strip-debug', '--strip-producers', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals', '--enable-reference-types', '--enable-multivalue', wasmPath, '-o', wasmPath]);

const files = { wasm: wasmPath, glue: gluePath };
const sizes = {};
let total = 0;
for (const [k, p] of Object.entries(files)) {
  const b = readFileSync(p);
  sizes[k] = { bytes: b.byteLength, gzipped: gzipSync(b).byteLength };
  total += sizes[k].gzipped;
}
console.log(`wasm ${sizes.wasm.bytes} B (${sizes.wasm.gzipped} gz) + glue ${sizes.glue.bytes} B (${sizes.glue.gzipped} gz) = ${total} B gzipped of ${LIMIT}; headroom ${LIMIT - total} B`);
console.log('(the page module that imports them is not written yet; its gzipped size adds to this total)');
writeFileSync(
  join(here, 'sizes.json'),
  JSON.stringify(
    {
      measuredWith: { rustc: version('rustc'), wasmBindgen: version('wasm-bindgen'), wasmOpt: version('wasm-opt'), node: process.version, gzip: 'node:zlib gzipSync, default level' },
      limitBytes: LIMIT,
      counted: 'the .wasm and its wasm-bindgen glue with its doc comments removed, each gzipped, summed; the page module will add to it',
      glueDocComments: { bytes: documented.length - glue.length, gzipped: gzipSync(documented).byteLength - gzipSync(glue).byteLength },
      ...sizes,
      totalGzipped: total,
      headroom: LIMIT - total,
    },
    null,
    2,
  ) + '\n',
);
if (total > LIMIT) {
  console.error(`over the limit by ${total - LIMIT} B`);
  process.exit(1);
}
