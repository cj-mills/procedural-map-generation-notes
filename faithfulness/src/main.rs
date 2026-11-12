//! The faithfulness check's harness. Each of the talk's 22 demos is compiled from its own source,
//! unchanged, read in place from `WOLVERSON_REPO` with `include!`. Two names it uses are
//! replaced in its module, so it runs without a window and from a seed we choose:
//!
//! - `run`, which in the talk's backend opens a window and steps through the frames on Enter
//!   (`backend/src/lib.rs:140-155`), here calls the demo's `setup` and `build` and keeps the
//!   frames;
//! - `RandomNumberGenerator::new`, seeded from entropy in the library, here seeded from the run's
//!   seed; and `FastNoise::seeded`, whose literal seeds (2, 0..50, 4, 48) are mixed with the
//!   run's seed so noise demos vary over seeds as the rest do.
//!
//! Everything else is the talk's code and its library at the talk's versions. The last frame of
//! each run is converted to the core's tiles and measured with the core's own trait code, so the
//! two sides are measured the same way.
//!
//!   wolverson-harness measure [--seeds 0..200] [--timeout 10] [--jobs 8] --out FILE
//!   wolverson-harness one <demo> <seed>      one run, as the line `measure` reads
//!   wolverson-harness map <demo> <seed>      one run's last frame, in the core's glyphs
//!   wolverson-harness checks [--seeds 0..200] --out FILE
//!                                            what four demos do that reading them leaves open

use map_lab_core::grid::*;
use map_lab_core::report::{self, string, Row};
use map_lab_core::traits::{self, GUARD_HIT, PREFAB_NO_PLACE, EXIT_UNREACHABLE};
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[allow(dead_code, unused_imports)]
mod backend {
    include!(concat!(env!("WOLVERSON_REPO"), "/backend/src/lib.rs"));
}

pub mod shim {
    use crate::backend::{BError, Map, MapGen};
    use std::cell::{Cell, RefCell};
    use std::ops::{Deref, DerefMut};

    thread_local! {
        static SEED: Cell<u64> = Cell::new(0);
        static MADE: Cell<u64> = Cell::new(0);
        static FRAMES: RefCell<Vec<(Map, String)>> = RefCell::new(Vec::new());
    }

    pub fn set_seed(s: u64) {
        SEED.with(|c| c.set(s));
        MADE.with(|c| c.set(0));
    }

    /// SplitMix64's finaliser over the run's seed and a stream number.
    fn mix(stream: u64) -> u64 {
        let mut z = SEED.with(|c| c.get()).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ stream.wrapping_add(0x632B_E59B_D9B4_E019);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Replaces the backend's `run`: build the frames, keep them, open no window.
    pub fn run(mut gen: Box<dyn MapGen>) -> BError {
        gen.setup();
        let frames = gen.build();
        FRAMES.with(|f| *f.borrow_mut() = frames);
        Ok(())
    }

    pub fn take() -> Vec<(Map, String)> {
        FRAMES.with(|f| std::mem::take(&mut *f.borrow_mut()))
    }

    /// The library's generator, seeded from the run's seed instead of from entropy. Every method
    /// is the library's, reached through `Deref`.
    pub struct RandomNumberGenerator(bracket_lib::prelude::RandomNumberGenerator);

    impl RandomNumberGenerator {
        pub fn new() -> Self {
            let n = MADE.with(|c| {
                c.set(c.get() + 1);
                c.get()
            });
            RandomNumberGenerator(bracket_lib::prelude::RandomNumberGenerator::seeded(mix(n)))
        }
    }

    impl Deref for RandomNumberGenerator {
        type Target = bracket_lib::prelude::RandomNumberGenerator;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for RandomNumberGenerator {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    /// `FastNoise::seeded` with the demo's literal seed mixed with the run's seed.
    pub struct FastNoise;

    impl FastNoise {
        pub fn seeded(literal: u64) -> bracket_lib::prelude::FastNoise {
            bracket_lib::prelude::FastNoise::seeded(mix(1 << 32 | literal))
        }
    }
}

macro_rules! demos {
    ($($m:ident = $dir:literal,)*) => {
        $(
            #[allow(dead_code, unused_imports, unused_variables, unused_mut, clippy::all)]
            mod $m {
                use crate::backend;
                use crate::shim::{run, FastNoise, RandomNumberGenerator};
                include!(concat!(env!("WOLVERSON_REPO"), "/", $dir, "/src/main.rs"));
                pub fn frames() -> Vec<(backend::Map, String)> {
                    let _ = main();
                    crate::shim::take()
                }
            }
        )*
        const DEMOS: &[(&str, fn() -> Vec<(backend::Map, String)>)] = &[$(($dir, $m::frames),)*];
    };
}

demos! {
    c01 = "c01-rooms",
    c02 = "c02-bsprooms",
    c03 = "c03-cellular",
    c04 = "c04-drunkard",
    c05 = "c05-dla",
    c06 = "c06-dla-attractor",
    c07 = "c07-dla-symmetry",
    c08 = "c08-dla-erosion",
    c08a = "c08a-voronoi",
    c09 = "c09-combination",
    c10 = "c10-perlin",
    c11 = "c11-overworld",
    c12 = "c12-noisezoom",
    c13 = "c13-double-noise",
    c14 = "c14-not-a-trap",
    c15 = "c15-still-not-a-trap",
    c16 = "c16-unreachable",
    c17 = "c17-starting-left",
    c18 = "c18-ending-right",
    c19 = "c19-ending-dijkstra",
    c20 = "c20-hot-path",
    c21 = "c21-hot-rooms",
}

/// How a demo's last frame reads as tiles.
#[derive(Clone, Copy, PartialEq)]
enum Reading {
    /// `#` open, `.` solid (the backend's own rule, `backend/src/lib.rs:47-68`); `@`, `>` and `*`
    /// are open tiles carrying the start, the exit and the path; `$` treasure, `^` trap; `!`, the
    /// marker the demos put on a desired start or exit point, solid.
    Dungeon,
    /// c08a's last frame, labeled "Voronoi Boundary Walls": `#` marks the boundaries, read as
    /// walls as the label says; `.` is open; the outer ring, which the demo resets, is wall.
    Voronoi,
    /// c10: every tile `#`, the noise in its color.
    Height,
    /// c11 to c13: `~` water, `;` grass, `^` mountain.
    Terrain,
}

fn reading(id: &str) -> Reading {
    match id {
        "c08a-voronoi" => Reading::Voronoi,
        "c10-perlin" => Reading::Height,
        "c11-overworld" | "c12-noisezoom" | "c13-double-noise" => Reading::Terrain,
        _ => Reading::Dungeon,
    }
}

const READINGS: [&str; 4] = [
    "# open and . solid, the backend's own rule; @ > * open tiles carrying the start, exit and path; $ treasure; ^ trap; ! (a desired-point marker) solid",
    "# (the boundaries) wall, as the frame's label 'Voronoi Boundary Walls' says; . open; the outer ring wall",
    "every tile open; the field is each tile's gray level, (noise + 1) / 2 clamped to 0..1",
    "~ water, ; grass, ^ mountain; for c11 and c12 the field is recovered from each tile's color",
];

/// Known differences between our version and the talk's, beside the random number generator.
fn differences(id: &str) -> &'static str {
    match id {
        "c06-dla-attractor" | "c07-dla-symmetry" => "Our line to the center is the standard error-accumulating Bresenham; the library's octant-based one can break ties the other way.",
        "c08a-voronoi" => "The talk's last frame draws the boundaries with its open-tile glyph under a label calling them walls; both sides here read them as walls.",
        "c09-combination" => "The spine prefab is our own design standing in for the talk's 11-by-50 layout, which isn't copied: traits that the prefab touches differ by design.",
        "c10-perlin" | "c11-overworld" | "c12-noisezoom" | "c13-double-noise" => "Our simplex noise is our own implementation of FastNoise's, with its permutation drawn from our generator.",
        "c14-not-a-trap" => "The talk's loop never ends when no room is big enough for the vault: such seeds time out here. Ours records prefab-no-place and builds the map without it.",
        "c15-still-not-a-trap" => "The talk's loop never ends when no open 7-by-6 patch exists: such seeds time out here. Ours gives up after 100,000 tries and records the failure.",
        "c18-ending-right" | "c19-ending-dijkstra" | "c20-hot-path" | "c21-hot-rooms" => "The talk's A* (bracket-pathfinding 0.8.4) accumulates f rather than g and can return a path longer than the shortest; ours is a shortest path. The path trait measures the shortest walk on both sides; the hot path and hot rooms follow each side's own path.",
        _ => "",
    }
}

struct Run {
    grid: Grid,
    start: Option<usize>,
    exit: Option<usize>,
    field: Vec<f32>,
    rooms: Option<u32>,
    /// Steps on the talk's own drawn path, when it drew one.
    drawn_path: Option<u32>,
}

fn convert(id: &str, frames: &[(backend::Map, String)]) -> Run {
    use backend::{to_cp437, HEIGHT, WIDTH};
    let (map, _) = frames.last().expect("a demo with no frames");
    let mut g = Grid::new(WIDTH as i32, HEIGHT as i32, WALL);
    let (mut start, mut exit, mut stars) = (None, None, 0u32);
    let mut field = Vec::new();
    let r = reading(id);
    let glyph = |c: char| to_cp437(c);
    for (i, (t, col)) in map.tiles.iter().enumerate() {
        let (x, y) = g.xy(i);
        let edge = x == 0 || y == 0 || x == g.w - 1 || y == g.h - 1;
        g.t[i] = match r {
            Reading::Dungeon => {
                if *t == glyph('@') {
                    start = Some(i);
                }
                if *t == glyph('>') {
                    exit = Some(i);
                }
                if *t == glyph('*') {
                    stars += 1;
                }
                if *t == glyph('.') || *t == glyph('!') {
                    WALL
                } else if *t == glyph('$') {
                    TREASURE
                } else if *t == glyph('^') {
                    TRAP
                } else {
                    FLOOR
                }
            }
            Reading::Voronoi => {
                if edge || *t == glyph('#') {
                    WALL
                } else {
                    FLOOR
                }
            }
            Reading::Height => {
                field.push(col.r);
                FLOOR
            }
            Reading::Terrain => {
                if *t == glyph('~') {
                    if id != "c13-double-noise" {
                        field.push(col.b - 0.75);
                    }
                    WATER
                } else if *t == glyph(';') {
                    if id != "c13-double-noise" {
                        field.push(col.g - 0.25);
                    }
                    GRASS
                } else {
                    if id != "c13-double-noise" {
                        field.push(col.r);
                    }
                    MOUNTAIN
                }
            }
        };
    }
    let count = |label: &str| frames.iter().filter(|(_, l)| l.starts_with(label)).count() as u32;
    let rooms = match id {
        "c01-rooms" => Some(count("Add room")),
        "c02-bsprooms" => Some(count("Add Room")),
        _ => None,
    };
    // The talk marks its A* path's tiles `*`, then the start and exit over its two ends.
    let drawn_path = (start.is_some() && exit.is_some() && stars > 0).then_some(stars + 1);
    Run { grid: g, start, exit, field, rooms, drawn_path }
}

fn one(id: &str, seed: u64) -> (Row, Option<(u32, u32)>, Run) {
    let (_, f) = DEMOS.iter().find(|(d, _)| *d == id).unwrap_or_else(|| panic!("no demo {id}"));
    shim::set_seed(seed);
    let frames = f();
    let run = convert(id, &frames);
    let start_exit = if id == "c17-starting-left" || id == "c16-unreachable" { (None, None) } else { (run.start, run.exit) };
    let t = traits::measure(&run.grid, start_exit.0, start_exit.1, &run.field, run.rooms);
    let mut fails = traits::failures(&run.grid, &t);
    if run.start.is_some() && run.exit.is_some() && start_exit.0.is_some() && t.path.is_none() {
        fails |= EXIT_UNREACHABLE;
    }
    let astar = run.drawn_path.zip(t.path);
    (report::row(seed, &t, fails), astar, run)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("one") => {
            let (row, astar, _) = one(&args[1], args[2].parse().unwrap());
            let vals: Vec<String> = row.values.iter().map(|v| v.map_or("null".into(), report::num)).collect();
            let a = astar.map_or("-".into(), |(d, s)| format!("{d},{s}"));
            println!("{}\t{}\t{}\t{}", row.seed, vals.join(","), row.failures, a);
        }
        Some("map") => {
            let (_, _, run) = one(&args[1], args[2].parse().unwrap());
            for y in 0..run.grid.h {
                let row: String = (0..run.grid.w)
                    .map(|x| {
                        let i = run.grid.idx(x, y);
                        if Some(i) == run.start {
                            '@'
                        } else if Some(i) == run.exit {
                            '>'
                        } else {
                            GLYPHS[run.grid.t[i] as usize] as char
                        }
                    })
                    .collect();
                println!("{row}");
            }
        }
        Some("measure") => measure(&args[1..]),
        Some("checks") => checks(&args[1..]),
        _ => {
            eprintln!("usage: wolverson-harness measure [--seeds 0..200] [--timeout 10] [--jobs 8] --out FILE | checks [--seeds 0..200] --out FILE | one <demo> <seed> | map <demo> <seed>");
            std::process::exit(2);
        }
    }
}

fn flag<'a>(a: &'a [String], name: &str) -> Option<&'a str> {
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1)).map(String::as_str)
}

/// One child process per run, so a run that never ends (c14, c15) can be stopped.
fn measure(a: &[String]) {
    let (from, to) = flag(a, "--seeds").map_or((0, 200), |s| {
        let (x, y) = s.split_once("..").expect("--seeds FROM..TO");
        (x.parse().unwrap(), y.parse().unwrap())
    });
    let timeout = Duration::from_secs(flag(a, "--timeout").map_or(10, |s| s.parse().unwrap()));
    let jobs: usize = flag(a, "--jobs").map_or(8, |s| s.parse().unwrap());
    let out = flag(a, "--out").expect("--out FILE");
    let exe = std::env::current_exe().unwrap();
    let work: Vec<(usize, u64)> = (0..DEMOS.len()).flat_map(|d| (from..to).map(move |s| (d, s))).collect();
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<(Row, Option<(u32, u32)>, bool)>>> = Mutex::new((0..work.len()).map(|_| None).collect());
    let started = Instant::now();
    std::thread::scope(|sc| {
        for _ in 0..jobs {
            sc.spawn(|| loop {
                let k = next.fetch_add(1, Ordering::Relaxed);
                let Some(&(d, seed)) = work.get(k) else { break };
                let id = DEMOS[d].0;
                let mut child = Command::new(&exe).args(["one", id, &seed.to_string()]).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
                let t0 = Instant::now();
                let finished = loop {
                    if let Some(st) = child.try_wait().unwrap() {
                        break st.success();
                    }
                    if t0.elapsed() > timeout {
                        let _ = child.kill();
                        let _ = child.wait();
                        break false;
                    }
                    std::thread::sleep(Duration::from_millis(2));
                };
                let r = if finished {
                    let mut s = String::new();
                    child.stdout.take().unwrap().read_to_string(&mut s).unwrap();
                    let f: Vec<&str> = s.trim().split('\t').collect();
                    let values = f[1].split(',').map(|v| if v == "null" { None } else { Some(v.parse().unwrap()) }).collect();
                    let astar = f[3].split_once(',').map(|(x, y)| (x.parse().unwrap(), y.parse().unwrap()));
                    (Row { seed, values, failures: f[2].parse().unwrap() }, astar, false)
                } else {
                    // Didn't finish within the timeout: in c14 and c15, the prefab loop with no place to stop.
                    let mut f = GUARD_HIT;
                    if id == "c14-not-a-trap" || id == "c15-still-not-a-trap" {
                        f |= PREFAB_NO_PLACE;
                    }
                    (Row { seed, values: vec![None; report::COLUMNS.len()], failures: f }, None, true)
                };
                results.lock().unwrap()[k] = Some(r);
            });
        }
    });
    let results = results.into_inner().unwrap();
    let mut demos = Vec::new();
    for (d, (id, _)) in DEMOS.iter().enumerate() {
        let mine: Vec<&(Row, Option<(u32, u32)>, bool)> =
            work.iter().zip(&results).filter(|((dd, _), _)| *dd == d).map(|(_, r)| r.as_ref().unwrap()).collect();
        let timed_out: Vec<String> = mine.iter().filter(|r| r.2).map(|r| r.0.seed.to_string()).collect();
        let astar: Vec<(u32, u32)> = mine.iter().filter_map(|r| r.1).collect();
        let mut extra = vec![
            ("source", string(&format!("thebracket/roguelike-celebration-2020 at {}, {id}/src/main.rs, run without a window by faithfulness/src/main.rs", env!("WOLVERSON_COMMIT")))),
            ("classification", string(READINGS[reading(id) as usize])),
        ];
        if !differences(id).is_empty() {
            extra.push(("differences", string(differences(id))));
        }
        if !timed_out.is_empty() {
            extra.push(("timedOut", report::list(timed_out)));
        }
        if !astar.is_empty() {
            let longer = astar.iter().filter(|(d, s)| d > s).count();
            let extra_steps: u32 = astar.iter().map(|(d, s)| d - s).sum();
            // The talk's drawn A* path against the shortest walk on the same last frame: the seeds where
            // it is longer, of the seeds that drew one, and the extra steps averaged over all of those.
            // Numbers, because the page states the first (page/lib.mjs, value(), `c18.astar.longer`).
            extra.push((
                "astarVsShortest",
                format!(
                    "{{\"longer\": {longer}, \"seeds\": {}, \"meanExtraSteps\": {}}}",
                    astar.len(),
                    report::num((extra_steps as f64 / astar.len() as f64 * 1000.0).round() / 1000.0)
                ),
            ));
        }
        let rows: Vec<Row> = mine.into_iter().map(|r| Row { seed: r.0.seed, values: r.0.values.clone(), failures: r.0.failures }).collect();
        demos.push(report::demo(id, None, &extra.iter().map(|(k, v)| (*k, v.clone())).collect::<Vec<_>>(), &rows));
    }
    let doc = report::traits_doc(
        &format!(
            "faithfulness/ wolverson-harness: the talk's code at {} with bracket-lib 0.8.1, each demo run once per seed in its own process with a {}-second limit",
            env!("WOLVERSON_COMMIT"),
            timeout.as_secs()
        ),
        (from, to),
        &demos,
    );
    std::fs::write(out, format!("{doc}\n")).unwrap();
    eprintln!("{} runs in {:.1}s", work.len(), started.elapsed().as_secs_f64());
}

/// What four of the talk's demos do, where reading the code leaves it open or depends on the
/// library: counted from each demo's own frames over the seeds, and from the library asked
/// directly. The page's table of where the talk and its code differ states these.
///
/// - c21 tests a room with the library's `point_in_rect` after adding a row to it
///   (`c21-hot-rooms/src/main.rs:132-141`) and draws it with the library's `for_each`. Which
///   tiles each covers is the library's to say.
/// - c05 starts a particle on any tile, open ones included (`c05-dla/src/main.rs:49-55`).
/// - c03's pass leaves out the map's outer ring (`c03-cellular/src/main.rs:58-59`).
/// - c12 mixes 3 octaves at a gain of 0.005 (`c12-noisezoom/src/main.rs:61-62`).
fn checks(a: &[String]) {
    use backend::{a_star_search, to_cp437, FractalType, NoiseType, Point, Rect, GRAY, HEIGHT, RGB, WIDTH, YELLOW};
    let (from, to) = flag(a, "--seeds").map_or((0, 200), |s| {
        let (x, y) = s.split_once("..").expect("--seeds FROM..TO");
        (x.parse::<u64>().unwrap(), y.parse::<u64>().unwrap())
    });
    let out = flag(a, "--out").expect("--out FILE");
    let xy = |i: usize| ((i % WIDTH) as i32, (i / WIDTH) as i32);

    // The library's rectangle, asked directly: 2 wide and 2 high, at (1, 1).
    let r = Rect::with_size(1, 1, 2, 2);
    let mut drawn = 0;
    r.for_each(|_| drawn += 1);
    let inside = (0..6).flat_map(|y| (0..6).map(move |x| Point::new(x, y))).filter(|p| r.point_in_rect(*p)).count();

    // c21: the rooms its last frame highlights, against the tiles its route steps on.
    let (yellow, gray) = (RGB::named(YELLOW), RGB::named(GRAY));
    let (mut rooms, mut highlighted, mut crossed, mut crossed_gray, mut crossed_gray_seeds) = (0, 0, 0, 0, 0);
    let (mut not_last_column_rule, mut last_row_only, mut last_row_only_seeds) = (0, 0, 0);
    for seed in from..to {
        shim::set_seed(seed);
        let frames = c21::frames();
        let (map, _) = frames.last().unwrap();
        let find = |c: char| map.tiles.iter().position(|t| t.0 == to_cp437(c));
        // The demo's own call, made again on its last frame: the glyphs are as they were when it ran.
        let path = a_star_search(find('@').expect("a start"), find('>').expect("an exit"), map);
        let steps: Vec<(i32, i32)> = path.steps.iter().map(|&i| xy(i)).collect();
        // A room is a block of yellow or gray tiles: rooms never touch, and corridors keep another color.
        let colored = |i: usize| map.tiles[i].1 == yellow || map.tiles[i].1 == gray;
        let mut seen = vec![false; WIDTH * HEIGHT];
        let (mut gray_here, mut row_here) = (0, 0);
        for i0 in 0..WIDTH * HEIGHT {
            if seen[i0] || !colored(i0) {
                continue;
            }
            let (mut x1, mut y1, mut x2, mut y2) = (i32::MAX, i32::MAX, 0, 0);
            let hot = map.tiles[i0].1 == yellow;
            let mut stack = vec![i0];
            seen[i0] = true;
            while let Some(i) = stack.pop() {
                let (x, y) = xy(i);
                x1 = x1.min(x);
                y1 = y1.min(y);
                x2 = x2.max(x);
                y2 = y2.max(y);
                assert!((map.tiles[i].1 == yellow) == hot, "a room in two colors");
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= WIDTH as i32 || ny >= HEIGHT as i32 {
                        continue;
                    }
                    let n = ny as usize * WIDTH + nx as usize;
                    if !seen[n] && colored(n) {
                        seen[n] = true;
                        stack.push(n);
                    }
                }
            }
            let any = |xb: i32, yb: i32| steps.iter().any(|&(x, y)| x >= x1 && x <= xb && y >= y1 && y <= yb);
            let whole = any(x2, y2); // a step on any tile the room is drawn on
            rooms += 1;
            highlighted += hot as u32;
            crossed += whole as u32;
            crossed_gray += (whole && !hot) as u32;
            gray_here += (whole && !hot) as u32;
            // The rule the counts point to: a step on any of its tiles but those of its last column.
            not_last_column_rule += (hot != any(x2 - 1, y2)) as u32;
            // Highlighted by steps on its last row alone: what the added row is for.
            last_row_only += (hot && !any(x2 - 1, y2 - 1)) as u32;
            row_here += (hot && !any(x2 - 1, y2 - 1)) as u32;
        }
        crossed_gray_seeds += (gray_here > 0) as u32;
        last_row_only_seeds += (row_here > 0) as u32;
    }

    // c05: a particle a frame after the first. One that starts on open floor digs nothing.
    let open = |m: &backend::Map| m.tiles.iter().filter(|t| t.0 == to_cp437('#')).count();
    let (mut particles, mut dug_nothing) = (0, 0);
    for seed in from..to {
        shim::set_seed(seed);
        let frames = c05::frames();
        particles += frames.len() - 1;
        dug_nothing += frames.windows(2).filter(|w| open(&w[1].0) == open(&w[0].0)).count();
    }

    // c03: the outer ring on the last frame, against the first frame's noise.
    let ring: Vec<usize> = (0..WIDTH * HEIGHT).filter(|&i| i % WIDTH == 0 || i / WIDTH == 0 || i % WIDTH == WIDTH - 1 || i / WIDTH == HEIGHT - 1).collect();
    let (mut ring_same, mut ring_open) = (0, 0);
    for seed in from..to {
        shim::set_seed(seed);
        let frames = c03::frames();
        let (first, last) = (&frames[0].0, &frames.last().unwrap().0);
        ring_same += ring.iter().all(|&i| first.tiles[i].0 == last.tiles[i].0) as u32;
        ring_open += ring.iter().filter(|&&i| last.tiles[i].0 == to_cp437('#')).count();
    }

    // c12: its first frame, against the library's noise from the same seed with the demo's own
    // settings but for 1 octave in place of 3 (`c12-noisezoom/src/main.rs:60-64`), sampled as the
    // demo samples it (`:33`) and read into its three bands (`:34-48`). The gain stays the
    // demo's: the library scales its result by the gain whatever the octaves.
    let mut band_differs = 0;
    for seed in from..to {
        shim::set_seed(seed);
        let frames = c12::frames();
        shim::set_seed(seed);
        let mut noise = shim::FastNoise::seeded(4);
        noise.set_noise_type(NoiseType::SimplexFractal);
        noise.set_fractal_type(FractalType::FBM);
        noise.set_fractal_octaves(1);
        noise.set_fractal_gain(0.005);
        noise.set_fractal_lacunarity(4.0);
        noise.set_frequency(0.08);
        for (i, t) in frames[0].0.tiles.iter().enumerate() {
            let (x, y) = xy(i);
            let n = noise.get_noise(x as f32 * 0.5, y as f32 * 0.5);
            let band = if n < 0.0 { '~' } else if n < 0.5 { ';' } else { '^' };
            band_differs += (t.0 != to_cp437(band)) as u32;
        }
    }

    let n = to - from;
    let doc = format!(
        r#"{{
  "note": "faithfulness/ wolverson-harness checks: the talk's code at {commit} with bracket-lib 0.8.1, counted from each demo's own frames; see checks() in faithfulness/src/main.rs",
  "seeds": {{ "from": {from}, "to": {to} }},
  "rect": {{
    "what": "the library's Rect::with_size(1, 1, 2, 2): the tiles for_each visits, which is how a room is drawn, and the tiles point_in_rect holds for",
    "drawn": {drawn},
    "inside": {inside}
  }},
  "c21": {{
    "what": "the rooms of c21-hot-rooms' last frame over the seeds: all of them; those highlighted; those with a step of the route on a tile they are drawn on; of those, the ones left gray, and the seeds with one; rooms whose highlight is not what 'a step on any of its tiles but those of its last column' gives; rooms highlighted by steps on their last row alone, and the seeds with one",
    "rooms": {rooms},
    "highlighted": {highlighted},
    "crossed": {crossed},
    "crossedGray": {crossed_gray},
    "crossedGraySeeds": {crossed_gray_seeds},
    "notLastColumnRule": {not_last_column_rule},
    "lastRowOnly": {last_row_only},
    "lastRowOnlySeeds": {last_row_only_seeds}
  }},
  "c05": {{
    "what": "the particles of c05-dla over the seeds, a frame each, and those whose frame has no more open tiles than the one before",
    "particles": {particles},
    "dugNothing": {dug_nothing}
  }},
  "c03": {{
    "what": "the outer ring of c03-cellular's map: the seeds where its last frame has the first frame's ring tile for tile, and the ring tiles open on the last frame, of all the ring's tiles over the seeds",
    "ringSameSeeds": {ring_same},
    "ringOpen": {ring_open},
    "ringTiles": {ring_tiles}
  }},
  "c12": {{
    "what": "the tiles of c12-noisezoom's first frame whose band (water, grass, mountain) is not the one the library's noise gives from the same seed and settings with 1 octave in place of 3, of all its tiles over the seeds",
    "bandDiffers": {band_differs},
    "tiles": {tiles}
  }}
}}
"#,
        commit = env!("WOLVERSON_COMMIT"),
        ring_tiles = ring.len() as u64 * n,
        tiles = (WIDTH * HEIGHT) as u64 * n,
    );
    std::fs::write(out, doc).unwrap();
}
