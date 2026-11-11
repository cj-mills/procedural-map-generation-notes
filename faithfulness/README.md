# The faithfulness check

Is the map lab's core faithful to the talk? Each technique should produce the same measured
traits as Herbert Wolverson's own demo code. The random number generators differ, so the maps
differ seed by seed, and the check is statistical. We run both over the same 200 seeds and
compare the distributions of each trait, demo by demo.

## How the talk's code runs here

`src/main.rs` compiles each of the talk's 22 demos from its own source, read in place with
`include!` from a checkout of
[thebracket/roguelike-celebration-2020](https://github.com/thebracket/roguelike-celebration-2020)
at `604e5b5a`. Nothing is copied into this repo. `build.rs` refuses a checkout at another
commit or with local changes. The library is bracket-lib 0.8.1, and `Cargo.lock` started as a
copy of the talk's own lockfile, so the bracket crates, `rand` and `rand_xorshift` are the
versions it ran with.

The backend has no headless mode, but its window code is all in `run`
(`backend/src/lib.rs:140-155`); every demo's `MapGen::build` only returns frames. So each demo's
module replaces two names, and runs the rest unchanged:

- `run` builds the frames and keeps them, instead of opening a window;
- `RandomNumberGenerator::new` is seeded from the run's seed, instead of from entropy, and
  `FastNoise::seeded` mixes the demo's literal seed with the run's seed.

Each run is its own process with a time limit. In two demos, a seed can make the talk's loop run
forever: c14, when no room is big enough for the vault, and c15, when no open patch fits it.
Such a seed is recorded as timed out (`timedOut`, failures `guard-hit` and `prefab-no-place`).

The last frame of each run is read as tiles (`classification` in the output). Then it is
measured by the core's own trait code (`lab/core/src/traits.rs`), so both sides are measured the
same way.

## Run it

```sh
cd faithfulness
WOLVERSON_REPO=/path/to/roguelike-celebration-2020 cargo build --release
target/release/wolverson-harness measure --seeds 0..200 --timeout 10 --jobs 12 --out talk-traits.json
target/release/wolverson-harness checks --out talk-checks.json   # what four demos do, counted (below)
target/release/wolverson-harness map c18-ending-right 3     # one run's map, in the core's glyphs
cd ../lab && node measure.mjs                                # regenerates data/faithfulness.json
```

`talk-traits.json` is committed: the comparison in `lab/data/faithfulness.json` regenerates
from it without the talk's code. Building bracket-lib compiles its default window backend too,
though no window opens.

## What four demos do, counted

`checks` answers four questions that reading the talk's code leaves open, or that depend on its
library, by counting over the same seeds. `talk-checks.json` is committed, the page's table of
where the talk and its code differ states its counts (`page/lib.mjs`, `value()`, names starting
`talk.`), and the bundle carries the file.

- **The library's rectangle, asked directly:** `for_each`, which draws a room, visits both ends
  of a rectangle; `point_in_rect` leaves out its last column and last row.
- **c21:** the rooms its last frame highlights against the tiles its route steps on. The demo
  tests each room grown by one row (`c21-hot-rooms/src/main.rs:132-141`); the counts say that
  comes to a step on any tile of the room except its last column, and how many rooms the route
  crosses that stay gray.
- **c05:** the particles that dig nothing, because they start on open floor.
- **c03:** whether the map's outer ring keeps its first noise, and how much of it is open.
- **c12:** how many tiles of its first frame change band when the library's noise mixes 1
  octave in place of the demo's 3. The comparison keeps the demo's gain: the library scales its
  result by 1 / (1 + gain + … + gain^octaves), one term more than the octaves it mixes.

## What the comparison says

`lab/data/faithfulness.json` reports, for every demo and trait:
- each side's mean and standard deviation;
- the two-sample Kolmogorov–Smirnov statistic and p-value;
- an effect size;
- a verdict: `differs` when p < 0.01.

It also gives both sides' failure rates. Traits are compared over finished maps only. Each
demo's entry carries the known differences (`differences`), such as c09's spine prefab, which
is our own design because the talk's layout isn't copied. It also says how often the talk's A*
returned a path longer than the shortest (`astarVsShortest`).
