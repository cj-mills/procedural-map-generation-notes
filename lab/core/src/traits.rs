//! The measured traits of a finished map, and the failures read from them. One definition, used
//! for our maps and for the talk's own maps in the faithfulness check (`faithfulness/`).

use crate::analyze::{distances, shortest_path};
use crate::grid::*;

pub struct Traits {
    /// Walkable tiles over all tiles.
    pub floor_share: f64,
    /// Groups of walkable tiles joined in four directions.
    pub regions: u32,
    /// Walkable tiles with exactly one walkable neighbor of four.
    pub dead_ends: u32,
    /// The share of walkable tiles in no 2 × 2 block of walkable tiles: passages one tile wide.
    pub corridor_ratio: f64,
    /// Steps on the shortest walk from start to exit; `None` when there is no walk.
    pub path: Option<u32>,
    /// Whether `path` came from the map's own start and exit or from the standard probe: the
    /// open tile nearest the middle of the left edge to the one nearest the middle of the right
    /// (the talk's own rules, `c17-starting-left/src/main.rs:69-86`, `c18-ending-right/src/main.rs:92-109`).
    pub probed: bool,
    /// Tiles of each kind over all tiles, in tile-value order.
    pub tile_shares: [f64; TILE_KINDS],
    /// The noise field's mean and standard deviation, for maps that have one.
    pub field: Option<(f64, f64)>,
    /// Rooms placed, for maps built from rooms.
    pub rooms: Option<u32>,
}

/// How many values a map has (`Traits::values`), and the data file columns.
pub const VALUES: usize = 12;
/// How many numbers `summarize` gives for each value.
pub const SUMMARY: usize = 8;

impl Traits {
    /// The traits as numbers, in the data file's column order (`report::COLUMNS`): floor share,
    /// regions, dead ends, corridor ratio, path, probed (0 or 1), water, grass, mountain, field
    /// mean, field standard deviation, rooms. NaN where a map has no such value: no path, no
    /// field, no rooms.
    pub fn values(&self) -> [f64; VALUES] {
        let nan = f64::NAN;
        [
            self.floor_share,
            self.regions as f64,
            self.dead_ends as f64,
            self.corridor_ratio,
            self.path.map_or(nan, |p| p as f64),
            self.probed as u8 as f64,
            self.tile_shares[WATER as usize],
            self.tile_shares[GRASS as usize],
            self.tile_shares[MOUNTAIN as usize],
            self.field.map_or(nan, |f| f.0),
            self.field.map_or(nan, |f| f.1),
            self.rooms.map_or(nan, |r| r as f64),
        ]
    }
}

/// One value over many seeds: how many seeds have it, then mean, sample standard deviation,
/// minimum, 10th percentile, median, 90th percentile and maximum (percentiles by linear
/// interpolation between order statistics). `None` when no seed has the value.
pub fn summary(values: &[f64]) -> Option<[f64; SUMMARY]> {
    if values.is_empty() {
        return None;
    }
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let sd = if values.len() > 1 { (values.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / (n - 1.0)).sqrt() } else { 0.0 };
    let mut s = values.to_vec();
    // Shell sort: the library's sort costs kilobytes of WebAssembly, and this is a few hundred values.
    let mut gap = s.len() / 2;
    while gap > 0 {
        for i in gap..s.len() {
            let mut j = i;
            while j >= gap && s[j - gap] > s[j] {
                s.swap(j - gap, j);
                j -= gap;
            }
        }
        gap /= 2;
    }
    let q = |p: f64| {
        let h = p * (s.len() - 1) as f64;
        let lo = h as usize;
        let hi = (lo + 1).min(s.len() - 1);
        s[lo] + (h - lo as f64) * (s[hi] - s[lo])
    };
    Some([n, mean, sd, s[0], q(0.1), q(0.5), q(0.9), s[s.len() - 1]])
}

/// How many numbers a seed's row has in `crate::measure`: its values, then its failure bits.
pub const ROW: usize = VALUES + 1;

/// Summaries of measured rows (`crate::measure`), for the page's "Measure over 200 seeds": for
/// each of the `VALUES` values its `SUMMARY` numbers (all NaN but the count, 0, when no seed
/// has it), then for each failure in `FAILURES` order the number of seeds failing it.
pub fn summarize(rows: &[f64]) -> Vec<f64> {
    let seeds = rows.len() / ROW;
    let mut out = Vec::with_capacity(VALUES * SUMMARY + FAILURES.len());
    let mut column = Vec::with_capacity(seeds);
    for c in 0..VALUES {
        column.clear();
        for n in 0..seeds {
            let v = rows[n * ROW + c];
            if !v.is_nan() {
                column.push(v);
            }
        }
        match summary(&column) {
            Some(s) => out.extend(s),
            None => {
                out.push(0.0);
                out.resize(out.len() + SUMMARY - 1, f64::NAN);
            }
        }
    }
    for b in 0..FAILURES.len() {
        out.push((0..seeds).filter(|n| rows[n * ROW + VALUES] as u32 & (1 << b) != 0).count() as f64);
    }
    out
}

/// A map shorter than this many steps from start to exit, as a fraction of its width, counts as
/// an exit next to the start.
pub const SHORT_PATH_DIVISOR: i32 = 4;
/// Fewer rooms than this counts as a failed room map.
pub const MIN_ROOMS: u32 = 4;

pub fn measure(g: &Grid, start: Option<usize>, exit: Option<usize>, field: &[f32], rooms: Option<u32>) -> Traits {
    let n = g.len();
    let open: Vec<bool> = g.t.iter().map(|&t| walkable(t)).collect();
    let floor = open.iter().filter(|&&o| o).count();
    let mut nb = [0usize; 4];

    let mut regions = 0;
    let mut seen = vec![false; n];
    let mut stack = Vec::new();
    for i in 0..n {
        if !open[i] || seen[i] {
            continue;
        }
        regions += 1;
        seen[i] = true;
        stack.push(i);
        while let Some(j) = stack.pop() {
            let m = g.neighbors(j, &mut nb);
        for &k in &nb[..m] {
                if open[k] && !seen[k] {
                    seen[k] = true;
                    stack.push(k);
                }
            }
        }
    }

    let mut dead_ends = 0;
    let mut in_block = vec![false; n];
    for i in 0..n {
        if !open[i] {
            continue;
        }
        let m = g.neighbors(i, &mut nb);
        if nb[..m].iter().filter(|&&k| open[k]).count() == 1 {
            dead_ends += 1;
        }
        let (x, y) = g.xy(i);
        if x + 1 < g.w && y + 1 < g.h {
            let w = g.w as usize;
            let block = [i, i + 1, i + w, i + w + 1];
            if block.iter().all(|&k| open[k]) {
                block.iter().for_each(|&k| in_block[k] = true);
            }
        }
    }
    let corridor = (0..n).filter(|&i| open[i] && !in_block[i]).count();

    let (from, to, probed) = match (start, exit) {
        (Some(s), Some(e)) => (Some(s), Some(e), false),
        _ => (nearest_walkable(g, 0, g.h / 2), nearest_walkable(g, g.w - 1, g.h / 2), true),
    };
    let path = match (from, to) {
        (Some(a), Some(b)) => {
            let d = distances(g, &[a]);
            (d[b] >= 0).then_some(d[b] as u32)
        }
        _ => None,
    };

    let mut tile_shares = [0.0; TILE_KINDS];
    for &t in &g.t {
        tile_shares[t as usize] += 1.0;
    }
    tile_shares.iter_mut().for_each(|s| *s /= n as f64);

    let field = (!field.is_empty()).then(|| {
        let m = field.len() as f64;
        let mean = field.iter().map(|&v| v as f64).sum::<f64>() / m;
        let var = field.iter().map(|&v| (v as f64 - mean) * (v as f64 - mean)).sum::<f64>() / m;
        (mean, var.sqrt())
    });

    Traits {
        floor_share: floor as f64 / n as f64,
        regions,
        dead_ends,
        corridor_ratio: if floor == 0 { 0.0 } else { corridor as f64 / floor as f64 },
        path,
        probed,
        tile_shares,
        field,
        rooms,
    }
}

/// Failures, as bits. The first four are set by stages as they run, the rest read from traits.
pub const PREFAB_NO_PLACE: u32 = 1 << 0;
pub const EXIT_UNREACHABLE: u32 = 1 << 1;
pub const NO_FLOOR: u32 = 1 << 2;
pub const GUARD_HIT: u32 = 1 << 3;
pub const DISCONNECTED: u32 = 1 << 4;
pub const NO_PATH: u32 = 1 << 5;
pub const SHORT_PATH: u32 = 1 << 6;
pub const FEW_ROOMS: u32 = 1 << 7;
pub const FAILURES: [(&str, &str); 8] = [
    ("prefab-no-place", "a prefab stage found no place for its prefab (the talk's loop would run forever)"),
    ("exit-unreachable", "the exit stage's exit can't be walked to from the start"),
    ("no-floor", "a stage needed a walkable tile and the map had none"),
    ("guard-hit", "a loop the talk leaves uncapped hit our cap: a particle that never lands, a prefab that never fits"),
    ("disconnected", "the walkable tiles form more than one region"),
    ("no-path", "no walk from start to exit (the map's own, or the standard probe's)"),
    ("short-path", "start and exit fewer than a quarter of the map's width apart, in steps"),
    ("few-rooms", "a room generator placed fewer than four rooms"),
];

pub fn failures(g: &Grid, t: &Traits) -> u32 {
    let mut f = 0;
    if t.regions > 1 {
        f |= DISCONNECTED;
    }
    match t.path {
        None => f |= NO_PATH,
        Some(p) if (p as i32) < g.w / SHORT_PATH_DIVISOR => f |= SHORT_PATH,
        _ => {}
    }
    if t.rooms.is_some_and(|r| r < MIN_ROOMS) {
        f |= FEW_ROOMS;
    }
    f
}

/// For the faithfulness check: the length in steps of a given path, and of the shortest one.
pub fn path_steps(g: &Grid, from: usize, to: usize) -> Option<u32> {
    let p = shortest_path(g, from, to);
    (!p.is_empty()).then(|| p.len() as u32 - 1)
}
