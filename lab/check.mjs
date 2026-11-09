// The repo's checks for the map lab core, in order:
//   1. cargo test: the core (golden hashes, determinism, frames replay, the parser's refusals,
//      the talk's conventions) and the CLI (the schema is current and covers every stage);
//   2. the WebAssembly build, measured: module glue and .wasm within 50 KB gzipped;
//   3. the WebAssembly build against the native build, tile for tile, over 20 seeds a recipe;
//   4. the measurement data regenerates byte for byte.
//
//   node lab/check.mjs          (npm run check)
import { execFileSync } from 'node:child_process';
const here = import.meta.dirname;
const step = (name, cmd, args) => {
  console.log(`\n== ${name}`);
  execFileSync(cmd, args, { cwd: here, stdio: 'inherit' });
};
step('cargo test', 'cargo', ['test', '--release', '--workspace']);
step('WebAssembly size', 'node', ['measure-size.mjs']);
step('WebAssembly matches native', 'node', ['check-wasm.mjs', '20']);
step('measurement data regenerates', 'node', ['measure.mjs', '--check']);
console.log('\nall checks passed');
