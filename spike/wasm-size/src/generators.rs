//! The generators, one function per family, each recording frames for the step-through.
//! Written from what each of the talk's demos does, not from its code.

#![allow(dead_code)]

use crate::Ctx;
use crate::grid::*;
#[allow(unused_imports)]
use crate::recipe::*;

// Frame labels: the page maps each to its step-through annotation.
pub const L_ROOM: u16 = 1;
pub const L_CORRIDORS: u16 = 2;
pub const L_NOISE: u16 = 3;
pub const L_ITERATION: u16 = 4;
pub const L_DIGGER: u16 = 5;
pub const L_PARTICLES: u16 = 6;
pub const L_CELLS: u16 = 7;
pub const L_TERRAIN: u16 = 8;
pub const L_PREFAB: u16 = 9;
pub const L_COMBINED: u16 = 10;

fn floor_share(g: &Grid) -> u32 {
    (g.t.iter().filter(|&&c| Grid::walkable(c)).count() * 100 / g.t.len()) as u32
}

/// Sort rooms left to right and join each to the previous with an L-shaped corridor.
fn join_rooms(c: &mut Ctx) {
    c.rooms.sort_by_key(|r| r.x1);
    for i in 1..c.rooms.len() {
        let (a, b) = (c.rooms[i - 1].center(), c.rooms[i].center());
        let coin = c.rng.chance(50);
        c.g.corridor(a, b, coin);
    }
    c.snap(L_CORRIDORS);
}

#[cfg(feature = "rooms")]
pub fn rooms(c: &mut Ctx, count: u32, min: u32, max: u32) {
    let (w, h) = (c.g.w, c.g.h);
    for _ in 0..count {
        let rw = c.rng.range(min as i32, max as i32 + 1);
        let rh = c.rng.range(min as i32, max as i32 + 1);
        let r = Rect::sized(c.rng.range(1, w - rw - 1), c.rng.range(1, h - rh - 1), rw, rh);
        if c.rooms.iter().all(|o| !o.overlaps(&r, 1)) {
            c.g.fill_rect(&r, FLOOR);
            c.rooms.push(r);
            c.snap(L_ROOM);
        }
    }
    join_rooms(c);
}

#[cfg(feature = "bsp")]
pub fn bsp(c: &mut Ctx, attempts: u32, min: u32) {
    let min = min.max(3) as i32;
    let mut rects = Vec::new();
    quarter(&mut rects, Rect::sized(1, 1, c.g.w - 2, c.g.h - 2));
    for _ in 0..attempts {
        if rects.is_empty() {
            break;
        }
        let pick = c.rng.range(0, rects.len() as i32) as usize;
        let space = rects[pick];
        // A room inside the space, somewhere between `min` and the space's size.
        let rw = c.rng.range(min.min(space.width()), space.width().min(min * 2) + 1);
        let rh = c.rng.range(min.min(space.height()), space.height().min(min * 2) + 1);
        let room = Rect::sized(
            space.x1 + c.rng.range(0, space.width() - rw + 1),
            space.y1 + c.rng.range(0, space.height() - rh + 1),
            rw,
            rh,
        );
        if room.width() >= min && room.height() >= min && clear_around(&c.g, &room) {
            c.g.fill_rect(&room, FLOOR);
            c.rooms.push(room);
            c.snap(L_ROOM);
            rects.swap_remove(pick);
            quarter(&mut rects, space);
        }
    }
    join_rooms(c);
}

#[cfg(feature = "bsp")]
fn quarter(out: &mut Vec<Rect>, r: Rect) {
    let (hw, hh) = (r.width() / 2, r.height() / 2);
    if hw < 3 || hh < 3 {
        return;
    }
    out.push(Rect::sized(r.x1, r.y1, hw, hh));
    out.push(Rect::sized(r.x1 + hw, r.y1, hw, hh));
    out.push(Rect::sized(r.x1, r.y1 + hh, hw, hh));
    out.push(Rect::sized(r.x1 + hw, r.y1 + hh, hw, hh));
}

#[cfg(feature = "bsp")]
fn clear_around(g: &Grid, r: &Rect) -> bool {
    for y in r.y1 - 1..=r.y2 {
        for x in r.x1 - 1..=r.x2 {
            if x < 1 || y < 1 || x >= g.w - 1 || y >= g.h - 1 || g.get(x, y) != WALL {
                return false;
            }
        }
    }
    true
}

#[cfg(feature = "cellular")]
pub fn cellular(c: &mut Ctx, fill: u32, iterations: u32) {
    let (w, h) = (c.g.w, c.g.h);
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let v = if c.rng.chance(fill) { WALL } else { FLOOR };
            c.g.set(x, y, v);
        }
    }
    c.snap(L_NOISE);
    for _ in 0..iterations {
        let src = c.g.clone();
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let mut n = 0;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if (dx, dy) != (0, 0) && src.get(x + dx, y + dy) == WALL {
                            n += 1;
                        }
                    }
                }
                c.g.set(x, y, if n > 4 || n == 0 { WALL } else { FLOOR });
            }
        }
        c.snap(L_ITERATION);
    }
}

#[cfg(feature = "drunkard")]
pub fn drunkard(c: &mut Ctx, floor: u32, lifetime: u32, center: bool) {
    let (w, h) = (c.g.w, c.g.h);
    let mid = (w / 2, h / 2);
    c.g.set(mid.0, mid.1, FLOOR);
    let mut diggers = 0;
    while floor_share(&c.g) < floor && diggers < 2000 {
        let mut p = if center { mid } else { random_floor(c).unwrap_or(mid) };
        for _ in 0..lifetime {
            c.g.set(p.0, p.1, FLOOR);
            let d = c.rng.step();
            p = ((p.0 + d.0).clamp(1, w - 2), (p.1 + d.1).clamp(1, h - 2));
        }
        diggers += 1;
        c.snap(L_DIGGER);
    }
}

pub fn random_floor(c: &mut Ctx) -> Option<(i32, i32)> {
    let open: Vec<usize> = (0..c.g.t.len()).filter(|&i| Grid::walkable(c.g.t[i])).collect();
    if open.is_empty() {
        return None;
    }
    let i = open[c.rng.range(0, open.len() as i32) as usize] as i32;
    Some((i % c.g.w, i / c.g.w))
}

#[cfg(feature = "dla")]
pub fn dla(c: &mut Ctx, mode: DlaMode, floor: u32) {
    let (w, h) = (c.g.w, c.g.h);
    let mid = (w / 2, h / 2);
    if mode != DlaMode::Erode {
        for (dx, dy) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
            c.g.set(mid.0 + dx, mid.1 + dy, FLOOR);
        }
        c.snap(L_NOISE);
    }
    let mut placed = 0u32;
    let mut guard = 0u32;
    while floor_share(&c.g) < floor && guard < 200_000 {
        guard += 1;
        let landed = match mode {
            DlaMode::Walk | DlaMode::Symmetric => {
                // A particle wanders until it steps onto the structure, and sticks where it was.
                let mut p = (c.rng.range(1, w - 1), c.rng.range(1, h - 1));
                let mut prev = p;
                let mut steps = 0;
                while c.g.get(p.0, p.1) != FLOOR && steps < 4000 {
                    prev = p;
                    let d = c.rng.step();
                    p = ((p.0 + d.0).clamp(1, w - 2), (p.1 + d.1).clamp(1, h - 2));
                    steps += 1;
                }
                (steps < 4000).then_some(prev)
            }
            DlaMode::Attract => {
                // A particle flies straight at the centre and sticks before the first floor.
                let start = (c.rng.range(1, w - 1), c.rng.range(1, h - 1));
                let mut prev = start;
                let mut hit = None;
                line(start, mid, |p| {
                    if hit.is_none() {
                        if c.g.get(p.0, p.1) == FLOOR {
                            hit = Some(prev);
                        }
                        prev = p;
                    }
                });
                hit
            }
            DlaMode::Erode => {
                // From inside the open space, wander until reaching rock, and carve it.
                let Some(mut p) = random_floor(c) else { break };
                let mut steps = 0;
                while c.g.get(p.0, p.1) == FLOOR && steps < 4000 {
                    let d = c.rng.step();
                    p = ((p.0 + d.0).clamp(1, w - 2), (p.1 + d.1).clamp(1, h - 2));
                    steps += 1;
                }
                Some(p)
            }
        };
        if let Some(p) = landed {
            c.g.set(p.0, p.1, FLOOR);
            if mode == DlaMode::Symmetric {
                c.g.set(w - 1 - p.0, p.1, FLOOR);
            }
            placed += 1;
            if placed % 25 == 0 {
                c.snap(L_PARTICLES);
            }
        }
    }
    c.snap(L_PARTICLES);
}

/// Bresenham's line from a to b, calling `f` on each point after a.
pub fn line(a: (i32, i32), b: (i32, i32), mut f: impl FnMut((i32, i32))) {
    let (dx, dy) = ((b.0 - a.0).abs(), -(b.1 - a.1).abs());
    let (sx, sy) = (if a.0 < b.0 { 1 } else { -1 }, if a.1 < b.1 { 1 } else { -1 });
    let (mut x, mut y, mut err) = (a.0, a.1, dx + dy);
    while (x, y) != b {
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
        f((x, y));
    }
}

#[cfg(feature = "voronoi")]
pub fn voronoi(c: &mut Ctx, seeds: u32, metric: Metric) {
    let (w, h) = (c.g.w, c.g.h);
    let pts: Vec<(i32, i32)> = (0..seeds.max(2)).map(|_| (c.rng.range(1, w - 1), c.rng.range(1, h - 1))).collect();
    let mut owner = vec![0u16; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let mut best = (i32::MAX, 0u16);
            for (i, p) in pts.iter().enumerate() {
                let (dx, dy) = (p.0 - x, p.1 - y);
                let d = match metric {
                    Metric::Euclid => dx * dx + dy * dy,
                    Metric::Manhattan => dx.abs() + dy.abs(),
                };
                if d < best.0 {
                    best = (d, i as u16);
                }
            }
            owner[(y * w + x) as usize] = best.1;
        }
    }
    // A tile is wall where a neighbour belongs to another cell: the cells' borders.
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let o = owner[(y * w + x) as usize];
            let edge = DIRS.iter().any(|d| owner[((y + d.1) * w + x + d.0) as usize] != o);
            c.g.set(x, y, if edge { WALL } else { FLOOR });
        }
    }
    c.snap(L_CELLS);
}

#[cfg(feature = "noise")]
pub fn noise(c: &mut Ctx, mode: NoiseMode, octaves: u32, gain: f32, lacunarity: f32, freq: f32, scale: f32) {
    let (w, h) = (c.g.w, c.g.h);
    let a = Simplex::new(&mut c.rng);
    let b = Simplex::new(&mut c.rng);
    c.field = vec![0; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32 / w as f32 * scale, y as f32 / h as f32 * scale);
            let mut n = a.fbm(fx, fy, octaves, gain, lacunarity, freq);
            if mode == NoiseMode::Double {
                // A second, finer layer adds detail the first one smooths away.
                n = n * 0.75 + 0.25 * b.fbm(fx, fy, (octaves / 2).max(1), gain * 0.5, lacunarity + 1.0, freq * 4.0);
            }
            let t = match mode {
                NoiseMode::Height => if n < 0.0 { WALL } else { FLOOR },
                _ => if n < 0.0 { WATER } else if n < 0.5 { GRASS } else { MOUNTAIN },
            };
            let i = c.g.idx(x, y);
            c.g.t[i] = t;
            c.field[i] = ((n.clamp(-1.0, 1.0) + 1.0) * 127.5) as u8;
        }
    }
    c.snap(L_TERRAIN);
}

/// 2D simplex noise over a permutation drawn from the recipe's generator.
#[cfg(feature = "noise")]
pub struct Simplex {
    perm: [u8; 512],
}

#[cfg(feature = "noise")]
impl Simplex {
    pub fn new(rng: &mut Rng) -> Self {
        let mut p = [0u8; 256];
        for (i, v) in p.iter_mut().enumerate() {
            *v = i as u8;
        }
        for i in (1..256).rev() {
            let j = rng.range(0, i as i32 + 1) as usize;
            p.swap(i, j);
        }
        let mut perm = [0u8; 512];
        for i in 0..512 {
            perm[i] = p[i & 255];
        }
        Simplex { perm }
    }

    pub fn fbm(&self, x: f32, y: f32, octaves: u32, gain: f32, lacunarity: f32, freq: f32) -> f32 {
        let (mut sum, mut amp, mut f, mut norm) = (0.0, 1.0, freq, 0.0);
        for _ in 0..octaves.max(1) {
            sum += amp * self.at(x * f, y * f);
            norm += amp;
            amp *= gain;
            f *= lacunarity;
        }
        sum / norm
    }

    pub fn at(&self, x: f32, y: f32) -> f32 {
        const F2: f32 = 0.366_025_42; // (sqrt(3) - 1) / 2
        const G2: f32 = 0.211_324_87; // (3 - sqrt(3)) / 6
        const GRAD: [(f32, f32); 8] = [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0), (0.7071, 0.7071), (-0.7071, 0.7071), (0.7071, -0.7071), (-0.7071, -0.7071)];
        let s = (x + y) * F2;
        let (i, j) = ((x + s).floor(), (y + s).floor());
        let t = (i + j) * G2;
        let (x0, y0) = (x - (i - t), y - (j - t));
        let (i1, j1) = if x0 > y0 { (1.0, 0.0) } else { (0.0, 1.0) };
        let corners = [(x0, y0, 0.0, 0.0), (x0 - i1 + G2, y0 - j1 + G2, i1, j1), (x0 - 1.0 + 2.0 * G2, y0 - 1.0 + 2.0 * G2, 1.0, 1.0)];
        let (ii, jj) = ((i as i32 & 255) as usize, (j as i32 & 255) as usize);
        let mut n = 0.0;
        for (cx, cy, oi, oj) in corners {
            let t = 0.5 - cx * cx - cy * cy;
            if t > 0.0 {
                let g = GRAD[(self.perm[ii + oi as usize + self.perm[jj + oj as usize] as usize] & 7) as usize];
                let t2 = t * t;
                n += t2 * t2 * (g.0 * cx + g.1 * cy);
            }
        }
        70.0 * n
    }
}

/// A vault in the talk's spirit: '#' wall, '.' floor, ' ' leave as is.
#[cfg(feature = "prefab")]
const VAULT: (&str, i32, i32) = ("\
 ####### \
##.....##\
#..#.#..#\
#.......#\
##.....##\
 ###.### ", 9, 6);

#[cfg(feature = "prefab")]
pub fn prefab(c: &mut Ctx, place: Place) {
    let (src, pw, ph) = VAULT;
    let (w, h) = (c.g.w, c.g.h);
    let at = match place {
        Place::Center => Some(((w - pw) / 2, (h - ph) / 2)),
        Place::Fit => {
            // Somewhere the vault's whole footprint is open floor, tried in random order.
            let mut spots: Vec<(i32, i32)> = (1..h - ph - 1).flat_map(|y| (1..w - pw - 1).map(move |x| (x, y))).collect();
            for i in (1..spots.len()).rev() {
                let j = c.rng.range(0, i as i32 + 1) as usize;
                spots.swap(i, j);
            }
            spots.into_iter().find(|&(x, y)| (0..ph).all(|dy| (0..pw).all(|dx| c.g.floor_at(x + dx, y + dy))))
        }
    };
    if let Some((x, y)) = at {
        for (i, ch) in src.bytes().enumerate() {
            let (dx, dy) = (i as i32 % pw, i as i32 / pw);
            match ch {
                b'#' => c.g.set(x + dx, y + dy, WALL),
                b'.' => c.g.set(x + dx, y + dy, FLOOR),
                _ => {}
            }
        }
        c.prefab = Some((x, y, pw, ph));
    }
    c.snap(L_PREFAB);
}

#[cfg(feature = "combine")]
pub fn combine(c: &mut Ctx, join: Join) {
    let Some(lower) = c.layers.pop() else { return };
    let w = c.g.w;
    for i in 0..c.g.t.len() {
        let (a, b) = (lower.t[i], c.g.t[i]);
        c.g.t[i] = match join {
            Join::Split => if (i as i32 % w) < w / 2 { a } else { b },
            Join::Union => if Grid::walkable(a) || Grid::walkable(b) { FLOOR } else { WALL },
            Join::Intersect => if Grid::walkable(a) && Grid::walkable(b) { FLOOR } else { WALL },
        };
    }
    let g = &c.g;
    c.rooms.retain(|r| {
        let m = r.center();
        g.floor_at(m.0, m.1)
    });
    c.snap(L_COMBINED);
}
