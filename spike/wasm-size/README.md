# Size spike: a Rust map-generator core as WebAssembly

Does a Rust core of every algorithm in Herbert Wolverson's talk fit the site's limit for an
interactive module, 50 KB gzipped? This crate answers that and nothing else: it is not the core.
Each code family the core needs is here once, unpolished, behind a Cargo feature, so that
turning one off measures its share:

| feature    | what it covers                                                        | talk's demos           |
|------------|-----------------------------------------------------------------------|------------------------|
| (always)   | the grid, a seeded PCG32, the recipe parser, the page's exports       |                        |
| `rooms`    | random rooms, sorted and joined by corridors                          | c01, c08, c14, c21     |
| `bsp`      | rooms placed in recursively quartered space                           | c02, c09               |
| `cellular` | cellular automata                                                     | c03, c09, c15–c20      |
| `drunkard` | drunkard's walk                                                       | c04                    |
| `dla`      | diffusion-limited aggregation: walk, attractor, symmetric, erosion    | c05–c08                |
| `voronoi`  | Voronoi cells with walls on their borders                             | c08a                   |
| `noise`    | 2D simplex noise with fBm: height map, overworld, zoom, double noise  | c10–c13                |
| `prefab`   | stamping a vault at the centre or where it fits                       | c09, c14, c15          |
| `combine`  | layers and joins (split, union, intersect)                            | c09                    |
| `analysis` | Dijkstra maps, culling, start and exit, hot path, hot rooms, traits   | c16–c21                |
| `serde`    | the recipe from JSON through serde_json instead of the text parser    |                        |

A recipe is a seed, a size and a pipeline of stages (`src/recipe.rs`). `demos.txt` has one
recipe per demo of the talk. The code was written from what each demo does, not from its code.

## Run

The repo's `rust-toolchain.toml` and `mise.toml` pin the toolchain, wasm-bindgen (to the crate's
version in `Cargo.lock`) and wasm-opt.

```sh
cargo test --release     # every demo builds natively and deterministically; writes target/native-hashes.txt
node measure.mjs         # builds each feature set, runs wasm-bindgen --target web and wasm-opt -Oz,
                         # prints the sizes gzipped as the site counts them, writes sizes.json
node check-wasm.mjs      # runs every demo through the WebAssembly build's exports and checks
                         # its tiles against the native build's
```

`sizes.json` is the last measurement, committed.
