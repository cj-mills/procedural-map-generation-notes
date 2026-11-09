//! Analysis and placement: reachability, culling, start and exit, the path between them, the hot
//! path and the rooms on it. Distances are breadth-first over walkable tiles in four directions,
//! as the talk's Dijkstra maps are over its four exits (`backend/src/lib.rs:47-68`).

use crate::grid::*;
use crate::record::*;
use crate::recipe::Stage;
use crate::{Ctx, EXIT_UNREACHABLE, NO_FLOOR};

/// Steps from the nearest source to every tile; -1 where none reaches. Sources count as reached
/// whatever they are; every other tile must be walkable.
pub fn distances(g: &Grid, sources: &[usize]) -> Vec<i32> {
    let mut d = vec![-1; g.len()];
    let mut queue = Vec::with_capacity(g.len());
    for &s in sources {
        if d[s] < 0 {
            d[s] = 0;
            queue.push(s);
        }
    }
    let mut head = 0;
    let mut nb = [0usize; 4];
    while head < queue.len() {
        let i = queue[head];
        head += 1;
        let m = g.neighbours(i, &mut nb);
        for &j in &nb[..m] {
            if d[j] < 0 && walkable(g.t[j]) {
                d[j] = d[i] + 1;
                queue.push(j);
            }
        }
    }
    d
}

/// A shortest path from `from` to `to`, both included; empty when there is none.
pub fn shortest_path(g: &Grid, from: usize, to: usize) -> Vec<usize> {
    let d = distances(g, &[to]);
    if d[from] < 0 {
        return Vec::new();
    }
    let mut path = vec![from];
    let mut at = from;
    let mut nb = [0usize; 4];
    while at != to {
        let n = g.neighbours(at, &mut nb);
        at = *nb[..n].iter().find(|&&j| d[j] == d[at] - 1).unwrap();
        path.push(at);
    }
    path
}

/// Mark or remove the floor that can't be reached from the open tile nearest the centre
/// (`c16-unreachable/src/main.rs:36-73`, `c17-starting-left/src/main.rs:34-67`).
pub fn cull(c: &mut Ctx, s: &Stage) {
    let remove = s.pick(1) == 0;
    let (w, h) = (c.g.w, c.g.h);
    let Some(centre) = nearest_walkable(&c.g, w / 2, h / 2) else {
        c.failures |= NO_FLOOR;
        return;
    };
    c.rec.frame(&c.g, Note::CullCenter, 0, vec![pack(centre, START)]);
    let d = distances(&c.g, &[centre]);
    let lost: Vec<usize> = (0..c.g.len()).filter(|&i| walkable(c.g.t[i]) && d[i] < 0).collect();
    if remove {
        for &i in &lost {
            c.g.t[i] = WALL;
        }
        c.rec.frame(&c.g, Note::CullRemoved, lost.len() as i32, Vec::new());
    } else {
        c.rec.frame(&c.g, Note::CullMarked, lost.len() as i32, lost.iter().map(|&i| pack(i, UNREACHABLE)).collect());
    }
    c.dist = d;
}

/// The start: the open tile nearest a desired point, the left edge's middle or the centre
/// (`c17-starting-left/src/main.rs:69-94`, `c19-ending-dijkstra/src/main.rs:68-90`).
pub fn start(c: &mut Ctx, s: &Stage) {
    let (w, h) = (c.g.w, c.g.h);
    let want = if s.pick(0) == 0 { (0, h / 2) } else { (w / 2, h / 2) };
    let Some(at) = nearest_walkable(&c.g, want.0, want.1) else {
        c.failures |= NO_FLOOR;
        return;
    };
    c.rec.frame(&c.g, Note::StartDesired, 0, marks_of(&c.g, [want], TARGET));
    c.rec.frame(&c.g, Note::StartPlaced, 0, vec![pack(at, START)]);
    c.start = Some(at);
    c.dist = distances(&c.g, &[at]);
}

/// The exit: the open tile nearest the right edge's middle, or the reachable tile farthest from
/// the start by walking distance, the last on a tie; then the path to it
/// (`c18-ending-right/src/main.rs:92-131`, `c19-ending-dijkstra/src/main.rs:92-122`).
pub fn exit(c: &mut Ctx, s: &Stage) {
    let (w, h) = (c.g.w, c.g.h);
    let Some(from) = c.start else { return };
    let mut marks = Vec::new();
    let at = if s.pick(0) == 0 {
        marks.extend(marks_of(&c.g, [(w - 1, h / 2)], TARGET));
        nearest_walkable(&c.g, w - 1, h / 2)
    } else {
        let d = distances(&c.g, &[from]);
        let mut best: Option<(usize, i32)> = None;
        for (i, &v) in d.iter().enumerate() {
            if v >= 0 && best.is_none_or(|(_, b)| v >= b) {
                best = Some((i, v));
            }
        }
        best.map(|(i, _)| i)
    };
    let Some(at) = at else {
        c.failures |= NO_FLOOR;
        return;
    };
    c.exit = Some(at);
    c.path = shortest_path(&c.g, from, at);
    if c.path.is_empty() {
        c.failures |= EXIT_UNREACHABLE;
    }
    if c.rec.on {
        marks.extend(c.path.iter().map(|&i| pack(i, PATH)));
        marks.push(pack(from, START));
        marks.push(pack(at, EXIT));
        let note = if c.path.is_empty() { Note::ExitNoPath } else { Note::ExitPlaced };
        c.rec.frame(&c.g, note, c.path.len().saturating_sub(1) as i32, marks);
    }
}

/// The hot path: walking distance from the path itself; tiles within `radius` are on the hot
/// path, and with `cull` set, every tile `cull` or more from it, other than the path, becomes
/// wall (`c20-hot-path/src/main.rs:131-154`).
pub fn hot_path(c: &mut Ctx, s: &Stage) {
    let (radius, cull) = (s.int(0), s.int(1));
    let hot = distances(&c.g, &c.path);
    let near = |v: i32, r: i32| v >= 0 && v < r;
    c.rec.frame(&c.g, Note::HotPath, radius, (0..c.g.len()).filter(|&i| near(hot[i], radius)).map(|i| pack(i, HOT)).collect());
    if cull > 0 {
        for i in 0..c.g.len() {
            if !near(hot[i], cull) && hot[i] != 0 {
                c.g.t[i] = WALL;
            }
        }
        c.rec.frame(&c.g, Note::HotPathCulled, cull, Vec::new());
    }
    c.hot = hot;
}

/// The rooms the path passes through, and the rest (`c21-hot-rooms/src/main.rs:129-156`): a room
/// counts when a path tile is inside it or on the row below it, as in the talk.
pub fn hot_rooms(c: &mut Ctx, _s: &Stage) {
    let mut marks = Vec::new();
    c.important = c
        .rooms
        .iter()
        .map(|r| {
            let below = Rect { y2: r.y2 + 1, ..*r };
            let hit = c.path.iter().any(|&i| {
                let (x, y) = c.g.xy(i);
                below.contains(x, y)
            });
            if c.rec.on {
                marks.extend(marks_of(&c.g, r.cells(), if hit { IMPORTANT } else { QUIET }));
            }
            hit
        })
        .collect();
    c.rec.frame(&c.g, Note::HotRooms, c.important.iter().filter(|&&b| b).count() as i32, marks);
}
