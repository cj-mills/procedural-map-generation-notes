//! Flood fill and Dijkstra maps: culling unreachable areas, placing start and exit, the hot path
//! and hot rooms, and the measured traits.

use crate::Ctx;
use crate::grid::*;
use crate::recipe::{ExitMode, Side};
use std::collections::VecDeque;

pub const UNREACHED: u16 = u16::MAX;

// Frame labels, continuing gen.rs's.
pub const L_CULLED: u16 = 11;
pub const L_START: u16 = 12;
pub const L_EXIT: u16 = 13;
pub const L_HOTPATH: u16 = 14;
pub const L_HOTROOMS: u16 = 15;

/// A Dijkstra map with unit costs: steps from the nearest source over walkable tiles.
pub fn dijkstra(g: &Grid, sources: &[(i32, i32)]) -> Vec<u16> {
    let mut d = vec![UNREACHED; g.t.len()];
    let mut q = VecDeque::new();
    for &(x, y) in sources {
        if g.floor_at(x, y) {
            d[g.idx(x, y)] = 0;
            q.push_back((x, y));
        }
    }
    while let Some((x, y)) = q.pop_front() {
        let here = d[g.idx(x, y)];
        for (dx, dy) in DIRS {
            let (nx, ny) = (x + dx, y + dy);
            if g.floor_at(nx, ny) {
                let i = g.idx(nx, ny);
                if d[i] == UNREACHED {
                    d[i] = here + 1;
                    q.push_back((nx, ny));
                }
            }
        }
    }
    d
}

/// The walkable tile nearest a target point, by straight-line distance.
fn nearest_floor(g: &Grid, target: (i32, i32)) -> Option<(i32, i32)> {
    let mut best: Option<((i32, i32), i32)> = None;
    for y in 0..g.h {
        for x in 0..g.w {
            if g.floor_at(x, y) {
                let (dx, dy) = (x - target.0, y - target.1);
                let d = dx * dx + dy * dy;
                if best.is_none_or(|b| d < b.1) {
                    best = Some(((x, y), d));
                }
            }
        }
    }
    best.map(|b| b.0)
}

pub fn start(c: &mut Ctx, side: Side) {
    let target = match side {
        Side::Left => (0, c.g.h / 2),
        Side::Center => (c.g.w / 2, c.g.h / 2),
    };
    c.start = nearest_floor(&c.g, target);
    c.snap(L_START);
}

/// Wall off every floor tile the start can't reach.
pub fn cull(c: &mut Ctx) {
    if c.start.is_none() {
        c.start = nearest_floor(&c.g, (c.g.w / 2, c.g.h / 2));
    }
    let Some(s) = c.start else { return };
    let d = dijkstra(&c.g, &[s]);
    for (i, t) in c.g.t.iter_mut().enumerate() {
        if Grid::walkable(*t) && d[i] == UNREACHED {
            *t = WALL;
        }
    }
    c.snap(L_CULLED);
}

pub fn exit(c: &mut Ctx, mode: ExitMode) {
    if c.start.is_none() {
        start(c, Side::Left);
    }
    let Some(s) = c.start else { return };
    c.dist = dijkstra(&c.g, &[s]);
    c.exit = match mode {
        ExitMode::Right => nearest_floor(&c.g, (c.g.w - 1, c.g.h / 2)),
        ExitMode::Far => {
            // The reachable tile with the most steps from the start.
            let mut best = (0u16, None);
            for (i, &d) in c.dist.iter().enumerate() {
                if d != UNREACHED && d >= best.0 {
                    best = (d, Some((i as i32 % c.g.w, i as i32 / c.g.w)));
                }
            }
            best.1
        }
    };
    c.snap(L_EXIT);
}

/// The shortest route from start to exit, walked downhill on the exit's Dijkstra map, and the
/// tiles within a few steps of it.
pub fn hot_path(c: &mut Ctx) {
    let (Some(s), Some(e)) = (c.start, c.exit) else { return };
    let from_exit = dijkstra(&c.g, &[e]);
    c.hot = vec![0; c.g.t.len()];
    let mut path = vec![s];
    let mut p = s;
    while from_exit[c.g.idx(p.0, p.1)] > 0 && from_exit[c.g.idx(p.0, p.1)] != UNREACHED {
        let here = from_exit[c.g.idx(p.0, p.1)];
        let Some(next) = DIRS
            .iter()
            .map(|d| (p.0 + d.0, p.1 + d.1))
            .find(|&(x, y)| c.g.inside(x, y) && from_exit[c.g.idx(x, y)] < here)
        else {
            break;
        };
        p = next;
        path.push(p);
    }
    let near = dijkstra(&c.g, &path);
    for (i, &d) in near.iter().enumerate() {
        c.hot[i] = if d == 0 { 2 } else if d <= 3 { 1 } else { 0 };
    }
    c.path = path;
    c.snap(L_HOTPATH);
}

/// Rooms the hot path passes through.
pub fn hot_rooms(c: &mut Ctx) {
    if c.path.is_empty() {
        hot_path(c);
    }
    c.hot_rooms = c
        .rooms
        .iter()
        .map(|r| c.path.iter().any(|&(x, y)| x >= r.x1 && x < r.x2 && y >= r.y1 && y < r.y2) as u8)
        .collect();
    c.snap(L_HOTROOMS);
}

/// Floor share, connected regions, dead ends, corridor share, start-to-exit steps.
pub fn traits(c: &Ctx) -> Vec<f32> {
    let g = &c.g;
    let mut floor = 0u32;
    let mut dead = 0u32;
    let mut corridor = 0u32;
    for y in 0..g.h {
        for x in 0..g.w {
            if !g.floor_at(x, y) {
                continue;
            }
            floor += 1;
            let open: [bool; 4] = DIRS.map(|d| g.floor_at(x + d.0, y + d.1));
            let n = open.iter().filter(|&&o| o).count();
            if n == 1 {
                dead += 1;
            }
            if n == 2 && ((open[0] && open[2]) || (open[1] && open[3])) {
                corridor += 1;
            }
        }
    }
    let mut seen = vec![false; g.t.len()];
    let mut regions = 0u32;
    for i in 0..g.t.len() {
        if Grid::walkable(g.t[i]) && !seen[i] {
            regions += 1;
            let p = (i as i32 % g.w, i as i32 / g.w);
            for (j, &d) in dijkstra(g, &[p]).iter().enumerate() {
                if d != UNREACHED {
                    seen[j] = true;
                }
            }
        }
    }
    let path = match (c.start, c.exit) {
        (Some(_), Some(e)) if !c.dist.is_empty() => c.dist[g.idx(e.0, e.1)] as f32,
        _ => -1.0,
    };
    let f = floor.max(1) as f32;
    vec![floor as f32 / g.t.len() as f32, regions as f32, dead as f32, corridor as f32 / f, path]
}
