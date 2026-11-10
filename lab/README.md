# The map lab core

One deterministic Rust core for every technique in Herbert Wolverson's talk *Procedural Map
Generation Techniques* (Roguelike Celebration 2020). It covers all 22 demos in the talk's code,
[thebracket/roguelike-celebration-2020](https://github.com/thebracket/roguelike-celebration-2020)
at `604e5b5a`: `c01-rooms` to `c21-hot-rooms`, plus `c08a-voronoi`. A **recipe** is a
pipeline of stages plus a seed. The page (as WebAssembly), the CLI and a later game-engine level
all build the same map from the same recipe, tile for tile.

The code is our own, written from what each demo does. Each stage cites the talk's code by file
and line, and none of it is copied (MIT, `LICENSE`). By default each stage is faithful to the
talk, and its defaults are the talk's own values. `faithfulness/` checks this against the talk's
own code, run over the same seeds.

## Layout

```
core/       map-lab-core: the grid, the random number generator, the recipe parser, every stage,
            the step-through frames, the measured traits, the level's caches and the walk
wasm/       map-lab-wasm: the page's binding (wasm-bindgen)
cli/        map-lab: the command line for agents and for the build
recipes/    one recipe per demo of the talk, with the talk's parameters
recipes/own/        recipes that aren't demos of the talk: what it describes without code
schema/     recipe.schema.json, the published JSON Schema (written by `map-lab schema`)
data/       traits.json and faithfulness.json, the measurement data (written by measure.mjs)
golden-hashes.txt   every recipe's map and caches for seeds 0..5: the regression check
measure-size.mjs    builds the WebAssembly binding and measures it gzipped; writes sizes.json
check-wasm.mjs      checks the WebAssembly build against the native build: maps, caches, walks, traits
measure.mjs         regenerates data/ (or, with --check, checks it regenerates byte for byte)
check.mjs           the repo's checks: all of the above (npm run check)
time-measure.mjs    times "Measure over 200 seeds" in headless Chromium under a 4x CPU slowdown
```

## Recipes

```json
{
  "format": "map-lab-recipe/1",
  "seed": 7,
  "width": 80,
  "height": 50,
  "pipeline": [
    { "stage": "cellular", "wall_percent": 55, "iterations": 10 },
    { "stage": "cull", "mode": "remove" },
    { "stage": "start", "at": "left" },
    { "stage": "exit", "at": "right" },
    { "stage": "hot-path", "radius": 10, "cull": 15 }
  ]
}
```

`schema/recipe.schema.json` describes every stage and parameter. It is written from the core's
`STAGES` table (`core/src/recipe.rs`), the one source of what a recipe may say. A test fails if
the committed schema falls behind that table. `map-lab stages` prints the same table without the
wording, as one line of JSON: each stage's parameters with their kinds, limits and defaults. The
page's controls are made from it, and the binding exports it (`stage_table`). The parser is hand-written: in the size spike,
serde_json took the WebAssembly module from 27,899 to 67,805 bytes gzipped. It refuses unknown
keys, values out of range and stages out of order, and says where:
`pipeline[3]: exit needs a start stage before it`.

Every stage takes `"variant"`. Today the only variant is `"talk"`, the talk's version. Labeled
variants go after it, each with its own source and a measured difference.

| stage | from the talk's demo | what it does |
|---|---|---|
| `rooms` | c01, c08, c14, c21 | random rooms, sorted left to right, joined by L-shaped corridors |
| `bsp` | c02, c09 | rooms in ever-smaller quarters of the map |
| `cellular` | c03, c09, c15–c20 | noise, then smoothing passes |
| `drunkard` | c04 | staggering diggers until a share of the map is floor |
| `dla` | c05, c06, c07 | aggregation from the center: walk, attractor, symmetric |
| `dla-erode` | c08 | aggregation particles eroding an existing map |
| `voronoi` | c08a | cells around random seeds, with walls on their edges, or roads |
| `noise` | c10–c13 | fractal simplex noise: height map, overworld, zoom, double |
| `combine` | c09 | two pipelines, left half of one and right half of the other |
| `prefab` | c09, c14, c15 | a vault in a room or on open floor; a spine down the middle |
| `cull` | c16, c17 | mark or remove what the center can't reach |
| `start`, `exit` | c17–c19 | placed by nearest open tile or by walking distance |
| `hot-path`, `hot-rooms` | c20, c21 | distance from the path; the rooms it passes through |

## The CLI

```sh
cd lab
cargo run --profile fast -p map-lab -- build recipes/c18-ending-right.json --seed 3 --path
cargo run --profile fast -p map-lab -- build recipes/c03-cellular.json --json --frames
cargo run --profile fast -p map-lab -- walk recipes/c20-hot-path.json --seed 7 --moves RRRRUULD
cargo run --profile fast -p map-lab -- traits recipes/c15-still-not-a-trap.json --seeds 0..20
cargo run --profile fast -p map-lab -- measure recipes/*.json --seeds 0..200 --out traits.json
cargo run --profile fast -p map-lab -- schema
cargo run --profile fast -p map-lab -- stages
```

`build` prints the map (`#` wall, `.` floor, `~` water, `"` grass, `^` mountain, `$` treasure,
`!` trap, `@` start, `>` exit, `&` a cache, `*` the path with `--path`), its traits and its
failures. With `--json` the same is JSON; `--frames` adds every step.

## Voronoi edges: walls or roads

The talk's demo finds the edges between Voronoi cells and draws walls there. That is
`edges: walls`, the default: a tile is wall when its right or lower neighbor belongs to another
cell. The talk also describes roads along those edges, in a game of its author's, and gives no
code for them. `edges: roads` is our own rule for that: the edges are floor and the cells solid,
and a tile is road when its right, lower or lower-right neighbor belongs to another cell.

The lower-right neighbor is the difference. Without it, the edges touch only corner to corner
wherever a boundary runs down and to the left. That seals a cell, which is all a wall has to do.
As floor it is 31 to 110 separate regions on each of seeds 0..200. With it, the roads are one
region on 181 of those seeds and two on the other 19 (`recipes/own/voronoi-roads.json`).

## The level: start, exit and caches

A map with a start, an exit and a path between them is a level that can be walked. The core
also places its caches (`core/src/level.rs`), so a recipe gives the same level in the page, on
the command line and in an engine. The talk suggests hiding bonus things off the hot path and
gives no code for it; the rule is our own.

- Caches go on floor or grass off the hot path: at least as many steps from the path, by
  walking, as the hot path's radius. That is the recipe's last `hot-path` stage's radius, or the
  talk's 10.
- The first is the tile farthest from the path. Each one after it is the tile farthest, by
  walking, from the caches already placed, and more than 12 steps from them.
- A tie goes to the first tile in row order. There are up to five.

A map with no path has none, and a map with little off its hot path has fewer. Every one of
seeds 0..200 of c18, c19 and c20 has five; c21 has five on 198 seeds and four on two.

## The walk

The walk through a level is the core's too (`core/src/walk.rs`): the page sends each step and
draws what comes back, and `map-lab walk` replays a route from its moves. So a recipe and a list
of moves give the same walk everywhere, and the walk's rules have the repo's checks. The talk
has no walk; these rules are our own.

- The walker starts on the start and takes one step at a time: left, right, up or down, written
  `L`, `R`, `U`, `D`. A step onto a tile that can't be walked on is refused.
- Stepping on a cache finds it. Standing on the exit ends the walk.
- **The field of view:** a tile is in view when it is within nine tiles of the walker in a
  straight line and every tile on the line between them can be walked on. A tile that can't be
  walked on is also in view when it touches a tile in view that can, corners included. Without
  that second rule a corridor's walls show only in patches.
- A tile that was in view stays remembered. The map the walker knows is what they have seen.

`map-lab walk` prints that map (unseen tiles blank; the exit and the caches only once seen),
then the steps taken against the shortest walk and the caches found. With `--json` it adds the
route and every tile's state.

## Measured traits and failures

These are defined once in `core/src/traits.rs`, and their wording is in `data/traits.json`:

- floor share
- connected regions
- dead ends
- corridor ratio: the share of walkable tiles in no 2 × 2 block of walkable tiles
- start-to-exit path length: from the map's own start and exit, or else from a standard probe
  between the middle of the left edge and the middle of the right
- terrain shares and the noise field's spread, for noise maps
- rooms placed

Failures are the criteria a seed can fail: a prefab with no place, an exit that can't be
reached, the walkable tiles split into regions, no path, start and exit less than a quarter of
the map's width apart, fewer than four rooms, or a loop the talk leaves uncapped that hits our
cap. `data/traits.json` lists every value, with the recipe and seed behind it, and the seeds
where each failure happens.

## The page's binding

`stage_table()` gives the stages and parameters for the page's controls.

`new Lab(recipeJson)` builds a recipe and keeps a frame for every step. A frame holds:
- the stage that recorded it;
- a note key, for the page's annotation;
- an argument (a room or iteration number);
- the tiles changed since the previous frame;
- marks, cells highlighted on that frame only;
- for noise, the field.

The page replays the changes to step through the build. It reads the finished map's tiles,
rooms, start, exit, path, caches, distance maps, field, traits and failures.

The walk is `lab.walk()` to start it, or start it again, and `lab.step(direction)` for each
move. After each one the page reads `lab.seen()` (a byte a tile: unseen, remembered, in view),
`lab.route()`, `lab.found()` and `lab.walk_done()`.

"Measure over 200 seeds" is `lab.measure(count)`: it builds the lab's recipe on the next `count`
seeds, from seed 0, and returns how many are measured so far. The page calls it in slices, so it
stays responsive. `lab.measured()` gives a row of traits and failures a seed, and
`lab.summary()` their summary. The summary is the one the data file holds (`core/src/traits.rs`):
`check-wasm.mjs --data` measures every demo this way and compares it with `data/traits.json`.

How long that takes, in headless Chromium under the 4x CPU slowdown the site's performance check
applies (`node time-measure.mjs`, read on November 9, 2026; times vary by machine and run):

| recipe | first build | 200 seeds in one call | one slice of 10 seeds |
|---|---|---|---|
| c20-hot-path, the page's opening recipe | 4 ms | 828 ms | 42 ms |
| c05-dla, the slowest | 63 ms | 9,103 ms | 470 ms |
| c10-perlin | 9 ms | 1,942 ms | 98 ms |
| c01-rooms, among the fastest | 1 ms | 78 ms | 5 ms |

So the page can't measure in one call. A slice has to be sized by time, not by seeds: one seed
of c05 is about 47 ms.

`wasm/src/lib.rs` documents each export. So does `target/web/map_lab_wasm.d.ts`, which
`measure-size.mjs` writes beside the glue. The glue itself is built without those comments:
wasm-bindgen copies them in, and they were about a third of its gzipped size.

## Determinism

There is one random number generator, PCG32 seeded through SplitMix64, and every stage draws
from it in pipeline order. Every stage of the core uses integer arithmetic, except noise. Noise
uses only `+ − × ÷`, comparisons and integer casts on `f32`, which IEEE 754 defines exactly. So
optimization level and target don't change a map: `check-wasm.mjs` compares the WebAssembly
build with the native one, and `golden-hashes.txt` pins the maps. A change to the golden hashes
changes published maps and needs a reason in its commit.

## Checks

```sh
npm run check     # from the repo root
```

The checks run `cargo test`, measure the WebAssembly module and fail if the `.wasm` plus its
glue exceeds 50 KB gzipped. Until the site's own budget for interactives counts WebAssembly,
this repo holds itself to that limit. They also check the WebAssembly build against the native
one over 20 seeds a recipe (maps, frames, caches, walks, measured traits, the stage table and
the 200-seed summaries), and that `data/` regenerates byte for byte.

The pinned toolchain is in `rust-toolchain.toml` and `mise.toml` at the repo root. Node is
26.13.0.
