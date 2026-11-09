// Loads the full build from measure.mjs in Node and runs every demo recipe through the page's
// exports: each must step through, read back, and give the same final tiles as the native build
// (target/native-hashes.txt, written by `cargo test --release`).
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const here = import.meta.dirname;
const dir = join(here, 'target', 'size', 'full');
const glue = await import(join(dir, 'mapgen_size_spike.js'));
glue.initSync({ module: readFileSync(join(dir, 'mapgen_size_spike_bg.wasm')) });

const demos = readFileSync(join(here, 'demos.txt'), 'utf8').split('\n---\n').map((d) => d.trim());
const native = readFileSync(join(here, 'target', 'native-hashes.txt'), 'utf8').trim().split('\n');
const fnv = (t) => t.reduce((h, b) => Math.imul(h ^ b, 0x01000193) >>> 0, 0x811c9dc5);

let failed = 0;
for (const [i, src] of demos.entries()) {
  const lab = new glue.Lab(src);
  let steps = 0;
  while (lab.step()) steps++;
  const tiles = lab.tiles();
  const h = fnv(tiles);
  const [floor, regions, dead, corridor, path] = lab.traits();
  const ok = String(h) === native[i];
  if (!ok) failed++;
  console.log(
    `${ok ? 'ok  ' : 'FAIL'} ${src.split('\n')[0].padEnd(30)} stages ${steps} frames ${String(lab.frames()).padStart(4)}` +
      `  floor ${floor.toFixed(2)} regions ${regions} dead ${dead} corridor ${corridor?.toFixed(2)} path ${path}`,
  );
  lab.free();
}
try {
  new glue.Lab('seed 1\nwobble');
  failed++;
} catch (e) {
  console.log(`parse error surfaces: ${e.message}`);
}
if (failed) {
  console.error(`${failed} demo(s) differ from the native build`);
  process.exit(1);
}
console.log(`all ${demos.length} demos match the native build tile for tile`);
