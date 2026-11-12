//! Analysis and placement: reachability, culling, start and exit, the path between them, the hot
//! path and the rooms on it. Distances are breadth-first over walkable tiles in four directions,
//! as the talk's Dijkstra maps are over its four exits (`backend/src/lib.rs:47-68`).

use crate::grid::*;
use crate::record::*;
use crate::recipe::Stage;
use crate::{Ctx, DOOR_NO_PLACE, EXIT_UNREACHABLE, KEY_BEHIND_DOOR, NO_FLOOR};

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
        let m = g.neighbors(i, &mut nb);
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
        let n = g.neighbors(at, &mut nb);
        at = *nb[..n].iter().find(|&&j| d[j] == d[at] - 1).unwrap();
        path.push(at);
    }
    path
}

/// Mark or remove the floor that can't be reached from the open tile nearest the center
/// (`c16-unreachable/src/main.rs:36-73`, `c17-starting-left/src/main.rs:34-67`).
pub fn cull(c: &mut Ctx, s: &Stage) {
    let remove = s.pick(1) == 0;
    let (w, h) = (c.g.w, c.g.h);
    let Some(center) = nearest_walkable(&c.g, w / 2, h / 2) else {
        c.failures |= NO_FLOOR;
        return;
    };
    c.rec.frame(&c.g, Note::CullCenter, 0, vec![pack(center, START)]);
    let d = distances(&c.g, &[center]);
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

/// The start: the open tile nearest a desired point, the left edge's middle or the center
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
    c.hot_radius = radius;
}

/// The rooms the path passes through, and the rest (`c21-hot-rooms/src/main.rs:129-156`): a room
/// counts when a path tile is on any of its tiles but those of its last column. The talk tests
/// the room grown by one row with the library's `point_in_rect`, which leaves out a rectangle's
/// far edges, as `Rect::contains` does here: the added row brings back the room's last row, and
/// its last column stays out (`faithfulness/`, `checks`, counts this in the talk's code).
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

/// Every tile that, closed, leaves `to` unreachable from `from`: the tiles the talk's flood
/// fill would pass as a place for a locked door (at 23:59 in the recording), in order from
/// `from`. `from` and `to` themselves are left out. Found in one pass, by a depth-first search
/// from `from` that keeps, for each tile, the earliest tile its subtree reaches back to; a test
/// checks it against closing each tile and flooding.
pub fn chokepoints(g: &Grid, from: usize, to: usize) -> Vec<usize> {
    let n = g.len();
    const NONE: usize = usize::MAX;
    // `seen[i]` is when the search first reached tile `i`, from 1; 0 is not reached.
    let mut seen = vec![0u32; n];
    let mut low = vec![0u32; n];
    let mut parent = vec![NONE; n];
    let mut stack = vec![(from, 0usize)];
    let mut clock = 1;
    seen[from] = 1;
    low[from] = 1;
    let mut nb = [0usize; 4];
    while let Some(&(i, k)) = stack.last() {
        let m = g.neighbors(i, &mut nb);
        if k < m {
            stack.last_mut().unwrap().1 += 1;
            let j = nb[k];
            if !walkable(g.t[j]) {
                continue;
            }
            if seen[j] == 0 {
                clock += 1;
                seen[j] = clock;
                low[j] = clock;
                parent[j] = i;
                stack.push((j, 0));
            } else if j != parent[i] {
                low[i] = low[i].min(seen[j]);
            }
        } else {
            stack.pop();
            if parent[i] != NONE {
                low[parent[i]] = low[parent[i]].min(low[i]);
            }
        }
    }
    let mut out = Vec::new();
    if seen[to] == 0 {
        return out;
    }
    let mut child = to;
    while parent[child] != NONE && parent[child] != from {
        let v = parent[child];
        if low[child] >= seen[v] {
            out.push(v);
        }
        child = v;
    }
    out.reverse();
    out
}

/// The locked door and its key. The talk describes them (from 23:46 in the recording, and its
/// slide 31) and has no code for them: a door where a flood fill finds no way around, and the
/// key somewhere before it. Which tile each goes on is our own rule.
/// - The door: of the route's tiles that, closed, cut the exit off from the start
///   (`chokepoints`) and are two steps or more from the start, the one nearest the route's
///   middle in steps, the earlier on a tie. Its passage is the unbroken run of such tiles
///   along the route that holds it, and the door goes in the passage's doorway: its first tile
///   with wall on two sides (exactly two neighbors that can be walked on), or its first tile
///   when it has none. A passage's first tile is often a room's own, at a corridor's mouth.
/// - The key, `before-door`: on the start's side of the closed door, the floor or grass tile
///   farthest from the route by walking, the first in row order on a tie.
/// - The key, `anywhere`: a random floor or grass tile the start can reach with the door open.
///   This is the placement the talk warns against: the key can land behind the door.
///
/// The key is never on the start, the exit or the door. A map with no such door, or no tile for
/// the key, gets neither.
///
/// The route is the shortest one on the map as it is now, found again here: a stage between
/// the exit and this one may have changed the map since the exit stage found its path. Nothing
/// that changes the map, the start or the exit may come after this stage (`recipe`'s order
/// rules), so the door and key always belong to the finished map.
pub fn lock(c: &mut Ctx, s: &Stage) {
    let route = match (c.start, c.exit) {
        (Some(from), Some(to)) => shortest_path(&c.g, from, to),
        _ => Vec::new(),
    };
    let placed = place_lock(c, &route, s.pick(0) == 1);
    if placed.is_none() {
        c.failures |= DOOR_NO_PLACE;
        c.rec.frame(&c.g, Note::LockNoDoor, 0, route.iter().map(|&i| pack(i, PATH)).collect());
    }
}

fn place_lock(c: &mut Ctx, route: &[usize], anywhere: bool) -> Option<()> {
    let (from, to) = (*route.first()?, *route.last()?);
    let d = distances(&c.g, &[from]);
    let mut cut = vec![false; c.g.len()];
    let cuts: Vec<usize> = chokepoints(&c.g, from, to).into_iter().filter(|&i| d[i] >= 2).collect();
    let steps = route.len() as i32 - 1;
    let mut door = *cuts.first()?;
    for &i in &cuts {
        cut[i] = true;
        if (2 * d[i] - steps).abs() < (2 * d[door] - steps).abs() {
            door = i;
        }
    }
    // Every way from the start to the exit passes the door, so the route does, `d[door]` steps in.
    let mut first = d[door] as usize;
    while cut[route[first - 1]] {
        first -= 1;
    }
    let mut nb = [0usize; 4];
    let walled = |i: usize, nb: &mut [usize; 4]| {
        let n = c.g.neighbors(i, nb);
        nb[..n].iter().filter(|&&j| walkable(c.g.t[j])).count() == 2
    };
    let mut at = first;
    while cut[route[at]] && !walled(route[at], &mut nb) {
        at += 1;
    }
    let door = route[if cut[route[at]] { at } else { first }];
    // The start's side: what the start reaches with the door closed.
    let was = c.g.t[door];
    c.g.t[door] = WALL;
    let side = distances(&c.g, &[from]);
    c.g.t[door] = was;
    let free = |i: usize| matches!(c.g.t[i], FLOOR | GRASS) && i != from && i != to && i != door;
    let key = if anywhere {
        let open: Vec<usize> = (0..c.g.len()).filter(|&i| d[i] >= 0 && free(i)).collect();
        open[c.rng.pick(open.len())?]
    } else {
        let off = distances(&c.g, route);
        let mut best: Option<usize> = None;
        for i in 0..c.g.len() {
            if side[i] >= 0 && free(i) && best.is_none_or(|j| off[i] > off[j]) {
                best = Some(i);
            }
        }
        best?
    };
    c.door = Some(door);
    c.key = Some(key);
    let behind = side[key] < 0;
    if behind {
        c.failures |= KEY_BEHIND_DOOR;
    }
    if c.rec.on {
        let ends = [pack(from, START), pack(to, EXIT)];
        let mut marks: Vec<u32> = route.iter().map(|&i| pack(i, PATH)).collect();
        marks.extend(cuts.iter().map(|&i| pack(i, POINT)));
        marks.extend(ends);
        c.rec.frame(&c.g, Note::LockChokepoints, cuts.len() as i32, marks);
        // What the closed door cuts off.
        let far = (0..c.g.len()).filter(|&i| d[i] >= 0 && side[i] < 0 && i != door).map(|i| pack(i, UNREACHABLE));
        let mut marks: Vec<u32> = far.clone().collect();
        marks.push(pack(door, DOOR));
        marks.extend(ends);
        c.rec.frame(&c.g, Note::LockDoor, d[door], marks);
        let mut marks: Vec<u32> = if behind { far.collect() } else { (0..c.g.len()).filter(|&i| side[i] >= 0).map(|i| pack(i, SIDE)).collect() };
        marks.extend([pack(door, DOOR), pack(key, KEY)]);
        marks.extend(ends);
        c.rec.frame(&c.g, if behind { Note::LockKeyBehind } else { Note::LockKey }, d[key], marks);
    }
    Some(())
}
