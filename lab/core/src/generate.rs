//! The generators: each starts a new, solid map and carves it. Line references are to the talk's
//! code at thebracket/roguelike-celebration-2020 `604e5b5a`; the code here is our own.

use crate::grid::*;
use crate::record::*;
use crate::recipe::Stage;
use crate::{Ctx, GUARD_HIT, NO_FLOOR};

/// Random rooms, sorted left to right and joined in that order by L-shaped corridors
/// (`c01-rooms/src/main.rs:21-79`).
pub fn rooms(c: &mut Ctx, s: &Stage) {
    let (attempts, lo, hi) = (s.int(0), s.int(1), s.int(2));
    let (w, h) = (c.g.w, c.g.h);
    c.g = Grid::new(w, h, WALL);
    let mut placed: Vec<(Rect, usize)> = Vec::new();
    for _ in 0..attempts {
        let x = c.rng.range(1, w - 10);
        let y = c.rng.range(1, h - 10);
        let rw = c.rng.range(lo, hi + 1);
        let rh = c.rng.range(lo, hi + 1);
        let room = Rect::sized(x, y, rw, rh);
        if placed.iter().any(|(r, _)| r.grown(1).intersects(&room)) {
            c.rec.frame(&c.g, Note::RoomDiscard, placed.len() as i32, marks_of(&c.g, room.cells(), REJECTED));
            continue;
        }
        for (x, y) in room.cells() {
            c.g.set(x, y, FLOOR);
        }
        let n = placed.len();
        placed.push((room, n));
        c.rec.frame(&c.g, Note::RoomAdd, n as i32 + 1, marks_of(&c.g, room.cells(), group(n)));
    }
    sort_and_join(c, placed);
}

fn group(n: usize) -> u8 {
    GROUP + (n % 200) as u8
}

/// Sort the rooms by their left edge and join each to the one before it (`c01-rooms/src/main.rs:56-79`).
fn sort_and_join(c: &mut Ctx, mut placed: Vec<(Rect, usize)>) {
    // A stable sort, as the talk's `sort_by` is: rooms with the same left edge keep their order.
    // Insertion sort, since there are few rooms and the library's stable sort costs about 3 KB
    // of WebAssembly.
    for i in 1..placed.len() {
        let mut j = i;
        while j > 0 && placed[j - 1].0.x1 > placed[j].0.x1 {
            placed.swap(j - 1, j);
            j -= 1;
        }
    }
    if c.rec.on {
        let marks = placed.iter().flat_map(|(r, n)| marks_of(&c.g, r.cells(), group(*n))).collect();
        c.rec.frame(&c.g, Note::RoomSort, placed.len() as i32, marks);
    }
    for i in 1..placed.len() {
        let a = placed[i - 1].0.center();
        let b = placed[i].0.center();
        let horizontal_first = c.rng.range(0, 2) == 1;
        c.g.corridor(a, b, horizontal_first);
        c.rec.frame(&c.g, Note::RoomCorridor, i as i32, Vec::new());
    }
    c.rooms = placed.into_iter().map(|(r, _)| r).collect();
    c.made_rooms = true;
}

/// Rooms placed inside a list of ever-smaller quarters of the map (`c02-bsprooms/src/main.rs:23-174`).
/// Each attempt picks any rectangle on the list, tries a room near its top-left corner, and on
/// success quarters that rectangle again; the list only grows.
pub fn bsp(c: &mut Ctx, s: &Stage) {
    let attempts = s.int(0);
    let (w, h) = (c.g.w, c.g.h);
    c.g = Grid::new(w, h, WALL);
    let first = Rect::sized(2, 2, w - 5, h - 5);
    let mut rects = vec![first];
    quarter(&mut rects, first);
    if c.rec.on {
        let mut marks = Vec::new();
        for (i, r) in rects.iter().enumerate() {
            marks.extend(marks_of(&c.g, r.cells(), group(i)));
            c.rec.frame(&c.g, Note::BspSubdivide, i as i32, marks.clone());
        }
    }
    let mut placed: Vec<(Rect, usize)> = Vec::new();
    for _ in 0..attempts {
        let rect = if rects.len() == 1 { rects[0] } else { rects[c.rng.die(rects.len() as i32) as usize - 1] };
        let room = sub_rect(c, rect);
        if !fits(&c.g, room) {
            continue;
        }
        for (x, y) in room.cells() {
            c.g.set(x, y, FLOOR);
        }
        let n = placed.len();
        placed.push((room, n));
        quarter(&mut rects, rect);
        c.rec.frame(&c.g, Note::BspAdd, n as i32 + 1, marks_of(&c.g, room.cells(), NEW));
    }
    sort_and_join(c, placed);
}

/// Append the four quarters of `r` (`c02-bsprooms/src/main.rs:86-112`).
fn quarter(rects: &mut Vec<Rect>, r: Rect) {
    let hw = (r.width() / 2).max(1);
    let hh = (r.height() / 2).max(1);
    rects.push(Rect::sized(r.x1, r.y1, hw, hh));
    rects.push(Rect::sized(r.x1, r.y1 + hh, hw, hh));
    rects.push(Rect::sized(r.x1 + hw, r.y1, hw, hh));
    rects.push(Rect::sized(r.x1 + hw, r.y1 + hh, hw, hh));
}

/// A room of at least 4 by 4 near the rectangle's top-left corner (`c02-bsprooms/src/main.rs:122-136`).
fn sub_rect(c: &mut Ctx, r: Rect) -> Rect {
    let rw = (c.rng.die(r.width().min(10)) - 1).max(3) + 1;
    let rh = (c.rng.die(r.height().min(10)) - 1).max(3) + 1;
    let x = r.x1 + c.rng.die(6) - 1;
    let y = r.y1 + c.rng.die(6) - 1;
    Rect { x1: x, y1: y, x2: x + rw, y2: y + rh }
}

/// The room grown by two stays off the map's outer ring and touches no floor (`c02-bsprooms/src/main.rs:138-174`).
fn fits(g: &Grid, room: Rect) -> bool {
    room.grown(2).cells().all(|(x, y)| x >= 1 && y >= 1 && x <= g.w - 2 && y <= g.h - 2 && g.get(x, y) == WALL)
}

/// Random noise, then repeated smoothing: a cell becomes wall with five or more wall neighbours,
/// or none, and floor otherwise. The outer ring keeps its noise (`c03-cellular/src/main.rs:17-70`).
pub fn cellular(c: &mut Ctx, s: &Stage) {
    let (wall_pct, iterations) = (s.int(0), s.int(1));
    for t in c.g.t.iter_mut() {
        *t = if c.rng.range(0, 100) < wall_pct { WALL } else { FLOOR };
    }
    c.rec.frame(&c.g, Note::CellularNoise, wall_pct, Vec::new());
    for i in 0..iterations {
        smooth(&mut c.g);
        c.rec.frame(&c.g, Note::CellularIterate, i + 1, Vec::new());
    }
}

fn smooth(g: &mut Grid) {
    let before = g.clone();
    for y in 1..g.h - 1 {
        for x in 1..g.w - 1 {
            let mut walls = 0;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if (dx, dy) != (0, 0) && before.get(x + dx, y + dy) == WALL {
                        walls += 1;
                    }
                }
            }
            g.set(x, y, if walls == 0 || walls >= 5 { WALL } else { FLOOR });
        }
    }
}

/// Diggers that stagger at random, the first from the centre and each later one from a random
/// floor tile, until a share of the map is floor (`c04-drunkard/src/main.rs:14-76`).
pub fn drunkard(c: &mut Ctx, s: &Stage) {
    let (lifetime, divisor) = (s.int(0), s.int(1));
    let (w, h) = (c.g.w, c.g.h);
    c.g = Grid::new(w, h, WALL);
    let target = (w * h / divisor) as usize;
    c.rec.frame(&c.g, Note::DrunkardSolid, 0, Vec::new());
    let mut from = (w / 2, h / 2);
    let mut n = 1;
    loop {
        let dug = stagger(c, from, lifetime);
        c.rec.frame(&c.g, Note::DrunkardDig, n, dug);
        n += 1;
        if c.g.count(FLOOR) >= target {
            break;
        }
        let open = c.g.indices_of(FLOOR);
        let i = open[c.rng.pick(open.len()).unwrap()];
        from = c.g.xy(i);
    }
}

/// One digger: step, then dig, until it leaves the map or has taken `lifetime + 1` steps.
fn stagger(c: &mut Ctx, (mut x, mut y): (i32, i32), lifetime: i32) -> Vec<u32> {
    const STEPS: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
    let mut dug = Vec::new();
    let mut steps = 0;
    loop {
        let (dx, dy) = STEPS[c.rng.range(0, 4) as usize];
        x += dx;
        y += dy;
        if !c.g.inside(x, y) {
            break;
        }
        c.g.set(x, y, FLOOR);
        if c.rec.on {
            dug.push(pack(c.g.idx(x, y), NEW));
        }
        steps += 1;
        if steps > lifetime {
            break;
        }
    }
    dug
}

/// The cap on random-walk steps for one particle. The talk's loops have none; a particle that
/// hits it is a failure of the seed, recorded, rather than a page that hangs.
const WALK_CAP: u32 = 10_000_000;

/// Diffusion-limited aggregation from a small cross at the centre: particles from random points
/// wander (walk), fly straight at the centre (attractor), or fly straight and land mirrored about
/// the centre column (symmetric), and stick where they meet floor
/// (`c05-dla/src/main.rs:19-84`, `c06-dla-attractor/src/main.rs:49-69`, `c07-dla-symmetry/src/main.rs:49-79`).
pub fn dla(c: &mut Ctx, s: &Stage) {
    let (mode, divisor) = (s.pick(0), s.int(1));
    let (w, h) = (c.g.w, c.g.h);
    c.g = Grid::new(w, h, WALL);
    let (cx, cy) = (w / 2, h / 2);
    for (dx, dy) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
        c.g.set(cx + dx, cy + dy, FLOOR);
    }
    c.rec.frame(&c.g, Note::DlaSeed, 0, Vec::new());
    let target = (w * h / divisor) as usize;
    let mut n = 0;
    while c.g.count(FLOOR) < target {
        let start = (c.rng.die(w - 3) + 1, c.rng.die(h - 3) + 1);
        let mut digger = start;
        let mut prev = start;
        if mode == 0 {
            let mut steps = 0;
            while c.g.get(digger.0, digger.1) == WALL {
                prev = digger;
                digger = clamped_step(c, digger);
                steps += 1;
                if steps > WALK_CAP {
                    c.failures |= GUARD_HIT;
                    return;
                }
            }
        } else {
            let path = line(start, (cx, cy));
            let mut next = 0;
            while c.g.get(digger.0, digger.1) == WALL && next < path.len() {
                prev = digger;
                digger = path[next];
                next += 1;
            }
        }
        let mut stuck = vec![prev];
        if mode == 2 && prev.0 != cx {
            let d = (cx - prev.0).abs();
            stuck = vec![(cx - d, prev.1), (cx + d, prev.1)];
        }
        for &(x, y) in &stuck {
            c.g.set(x, y, FLOOR);
        }
        n += 1;
        c.rec.frame(&c.g, Note::DlaParticle, n, marks_of(&c.g, stuck, NEW));
    }
}

/// One random step that stays two tiles inside the left and top edges, one inside the right and
/// bottom (`c05-dla/src/main.rs:57-79`).
fn clamped_step(c: &mut Ctx, (mut x, mut y): (i32, i32)) -> (i32, i32) {
    match c.rng.die(4) {
        1 if x > 2 => x -= 1,
        2 if x < c.g.w - 2 => x += 1,
        3 if y > 2 => y -= 1,
        4 if y < c.g.h - 2 => y += 1,
        _ => {}
    }
    (x, y)
}

/// A Bresenham line from `a` to `b`, both included.
pub fn line(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    let dx = (b.0 - a.0).abs();
    let dy = -(b.1 - a.1).abs();
    let sx = if a.0 < b.0 { 1 } else { -1 };
    let sy = if a.1 < b.1 { 1 } else { -1 };
    let mut err = dx + dy;
    let (mut x, mut y) = a;
    let mut out = Vec::with_capacity((dx - dy + 1) as usize);
    loop {
        out.push((x, y));
        if (x, y) == b {
            return out;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// Erosion: particles dropped on random floor tiles wander until they leave the floor, and
/// carve the wall tile they reach (`c08-dla-erosion/src/main.rs:85-130`).
pub fn dla_erode(c: &mut Ctx, s: &Stage) {
    let iterations = s.int(0);
    c.rec.frame(&c.g, Note::ErodeStart, 0, Vec::new());
    for i in 0..iterations {
        let open: Vec<usize> = (0..c.g.len()).filter(|&i| walkable(c.g.t[i])).collect();
        let Some(k) = c.rng.pick(open.len()) else {
            c.failures |= NO_FLOOR;
            return;
        };
        let mut digger = c.g.xy(open[k]);
        let mut steps = 0;
        while walkable(c.g.get(digger.0, digger.1)) {
            digger = clamped_step(c, digger);
            steps += 1;
            if steps > WALK_CAP {
                c.failures |= GUARD_HIT;
                return;
            }
        }
        c.g.set(digger.0, digger.1, FLOOR);
        c.rec.frame(&c.g, Note::ErodeParticle, i + 1, marks_of(&c.g, [digger], NEW));
    }
}

/// Voronoi cells: random seed points, each tile given to its nearest seed under three distance
/// measures, then walls where neighbouring tiles belong to different seeds
/// (`c08a-voronoi/src/main.rs:20-113`).
pub fn voronoi(c: &mut Ctx, s: &Stage) {
    let (n, metric) = (s.int(0) as usize, s.pick(1));
    let (w, h) = (c.g.w, c.g.h);
    c.g = Grid::new(w, h, WALL);
    let seeds: Vec<(i32, i32)> = (0..n).map(|_| (c.rng.range(1, w - 1), c.rng.range(1, h - 1))).collect();
    if c.rec.on {
        let marks = seeds.iter().enumerate().map(|(i, &(x, y))| pack(c.g.idx(x, y), group(i))).collect();
        c.rec.frame(&c.g, Note::VoronoiSeeds, n as i32, marks);
    }
    let mut chosen = Vec::new();
    for (m, note) in [Note::VoronoiPythagoras, Note::VoronoiManhattan, Note::VoronoiChebyshev].into_iter().enumerate() {
        if !c.rec.on && m as u8 != metric {
            continue;
        }
        let owner: Vec<usize> = (0..c.g.len()).map(|i| nearest_seed(c.g.xy(i), &seeds, m as u8)).collect();
        if c.rec.on {
            let marks = owner.iter().enumerate().map(|(i, &o)| pack(i, group(o))).collect();
            c.rec.frame(&c.g, note, 0, marks);
        }
        if m as u8 == metric {
            chosen = owner;
        }
    }
    let wu = w as usize;
    for i in 0..c.g.len() {
        let (x, y) = c.g.xy(i);
        let edge = x == 0 || y == 0 || x == w - 1 || y == h - 1;
        let border = !edge && (chosen[i] != chosen[i + 1] || chosen[i] != chosen[i + wu]);
        c.g.t[i] = if edge || border { WALL } else { FLOOR };
    }
    c.rec.frame(&c.g, Note::VoronoiWalls, n as i32, Vec::new());
}

/// The nearest seed, the first on a tie. Straight-line distance is compared squared, which
/// orders the same as the talk's square root.
fn nearest_seed((x, y): (i32, i32), seeds: &[(i32, i32)], metric: u8) -> usize {
    let mut best = (0, i64::MAX);
    for (i, &(sx, sy)) in seeds.iter().enumerate() {
        let (dx, dy) = (((x - sx) as i64).abs(), ((y - sy) as i64).abs());
        let d = match metric {
            0 => dx * dx + dy * dy,
            1 => dx + dy,
            _ => dx.max(dy),
        };
        if d < best.1 {
            best = (i, d);
        }
    }
    best.0
}
