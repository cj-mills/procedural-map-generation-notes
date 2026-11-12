//! The level a finished map makes for the walk: its start, its exit and the caches. The caches
//! are placed here, by one rule, so a recipe gives the same level in the page, on the command
//! line and in an engine.
//!
//! The talk suggests hiding bonus things in the rooms off the hot path (at 22:51 in the
//! recording) and gives no code for it. This rule is our own.

use crate::analyze::{chokepoints, distances, shortest_path};
use crate::grid::*;
use crate::recipe::{Kind, Recipe, Stage};
use crate::{build, Build, NO_FLOOR};

/// At most this many caches a level.
pub const CACHES: usize = 5;
/// Caches are more than this many steps apart, by walking.
pub const CACHE_SPACING: i32 = 12;

/// Where the caches go, all of them on floor or grass off the hot path: `hot_radius` steps or
/// more from the path by walking (the last hot-path stage's radius, or the talk's 10).
/// - The first is the tile farthest from the path.
/// - Each one after it is the tile farthest, by walking, from the caches already placed, so
///   they spread over the map; it must be more than `CACHE_SPACING` steps from them.
/// - A tie goes to the first tile in row order, as the talk's nearest-tile searches do.
/// - There are up to `CACHES` of them.
/// - A level with a locked door keeps its caches more than `CACHE_SPACING` steps from the key
///   too, as if the key were a cache placed before them.
///
/// None without a path from the start to the exit, and fewer where the map has little off its
/// hot path.
pub fn caches(b: &Build) -> Vec<usize> {
    let mut out = Vec::new();
    if b.path.is_empty() {
        return out;
    }
    let g = b.grid();
    let hot = distances(&g, &b.path);
    // Walking distance from the nearest thing to keep away from: the path, then the caches.
    let mut away = hot.clone();
    let mut least = b.hot_radius;
    let from_key = b.key.map(|k| distances(&g, &[k]));
    while out.len() < CACHES {
        let mut best: Option<usize> = None;
        for i in 0..g.len() {
            let open = matches!(g.t[i], FLOOR | GRASS) && hot[i] >= b.hot_radius && from_key.as_ref().is_none_or(|d| d[i] > CACHE_SPACING);
            if open && away[i] >= least && best.is_none_or(|j| away[i] > away[j]) {
                best = Some(i);
            }
        }
        let Some(i) = best else { break };
        out.push(i);
        away = distances(&g, &out);
        least = CACHE_SPACING + 1;
    }
    out
}

/// The shortest walk that finishes the level: the route, or with a locked door, the shortest
/// way to the key with the door closed and on from the key to the exit. Empty when the level
/// can't be finished: no route, or the key behind its door.
pub fn finish(b: &Build) -> Vec<usize> {
    let (Some(door), Some(key), Some(start), Some(exit)) = (b.door, b.key, b.start, b.exit) else { return b.path.clone() };
    let mut g = b.grid();
    let on = shortest_path(&g, key, exit);
    g.t[door] = WALL;
    let mut walk = shortest_path(&g, start, key);
    if walk.is_empty() || on.is_empty() {
        return Vec::new();
    }
    walk.extend(&on[1..]);
    walk
}

/// How many values a level with a lock stage has (`lock_values`), and `LOCK_COLUMNS`.
pub const LOCK_VALUES: usize = 7;

/// What a lock stage did to a level, as numbers in `LOCK_COLUMNS` order, NaN where the level
/// has no such value. `None` when the map is not a level: no start, no exit or no route.
pub fn lock_values(b: &Build) -> Option<[f64; LOCK_VALUES]> {
    let (start, exit) = (b.start?, b.exit?);
    let g = b.grid();
    // The route on the finished map, which the lock stage read (`analyze::refresh`).
    let route = &b.path;
    if route.is_empty() {
        return None;
    }
    let d = distances(&g, &[start]);
    let steps = (route.len() - 1) as f64;
    let nan = f64::NAN;
    let mut v = [nan; LOCK_VALUES];
    v[0] = chokepoints(&g, start, exit).iter().filter(|&&i| d[i] >= 2).count() as f64;
    v[1] = b.door.is_some() as u8 as f64;
    let (Some(door), Some(key)) = (b.door, b.key) else { return Some(v) };
    v[2] = d[door] as f64 / steps;
    let mut closed = g.clone();
    closed.t[door] = WALL;
    let side = distances(&closed, &[start]);
    let reach = |d: &[i32]| d.iter().filter(|&&x| x >= 0).count() as f64;
    v[3] = reach(&side) / reach(&d);
    v[4] = distances(&g, route)[key] as f64;
    let walk = finish(b);
    if !walk.is_empty() {
        v[5] = (walk.len() - 1) as f64;
        v[6] = v[5] / steps;
    }
    Some(v)
}

/// Why a finished map isn't a level, or `None` when it is one: it has a start and an exit,
/// and a route of a step or more between them.
pub fn not_a_level(b: &Build) -> Option<&'static str> {
    Some(match (b.start, b.exit) {
        (Some(s), Some(e)) if s == e => "the start and the exit are the same tile",
        (Some(_), Some(_)) if b.path.is_empty() => "the start can't reach the exit",
        (Some(_), Some(_)) => return None,
        // A stage that found no floor is the reason, if it is why the start or exit is missing.
        _ if b.stage_failures & NO_FLOOR != 0 => "the map has no floor to stand on",
        (None, _) => "the map has no start",
        (_, None) => "the map has no exit",
    })
}

/// What "Make it walkable" adds to a recipe whose map isn't a level, in the talk's order
/// (`c18-ending-right`): a cull that removes what the center can't reach, then the start, then
/// the exit. Only the stages after the map's last generator count.
/// - The cull goes just before the start, or where the start goes, unless a cull that removes
///   is there already. After it the map's floor is one region, so the start reaches the exit.
/// - A missing start goes at the end of the pipeline, and a missing exit after everything else.
///   A lock stage needs an exit before it, so nothing is ever added after one.
///
/// Each addition is `(index, stage)`, the index in the pipeline once the additions before it
/// are made, each stage with its defaults (`Stage::of`).
pub fn repair(r: &Recipe) -> Vec<(usize, Kind)> {
    let p = &r.pipeline;
    let tail = p.iter().rposition(|s| s.kind.is_generator()).map_or(0, |g| g + 1);
    let start = p[tail..].iter().position(|s| s.kind == Kind::Start).map(|i| tail + i);
    let at = start.unwrap_or(p.len());
    let mut add = Vec::new();
    if !(at > tail && p[at - 1].kind == Kind::Cull && p[at - 1].changes_map()) {
        add.push((at, Kind::Cull));
    }
    if start.is_none() {
        add.push((at + add.len(), Kind::Start));
    }
    if !p[tail..].iter().any(|s| s.kind == Kind::Exit) {
        add.push((p.len() + add.len(), Kind::Exit));
    }
    add
}

/// The button's answer for a recipe whose map isn't a level: what `repair` adds, when the map
/// it makes is a level; otherwise why that map isn't one (`not_a_level`). The recipe is built
/// with the additions and left as it was.
pub fn make_walkable(r: &mut Recipe) -> Result<Vec<(usize, Kind)>, &'static str> {
    let add = repair(r);
    for &(i, kind) in &add {
        r.pipeline.insert(i, Stage::of(kind));
    }
    let why = not_a_level(&build(r, false));
    for &(i, _) in add.iter().rev() {
        r.pipeline.remove(i);
    }
    match why {
        Some(why) => Err(why),
        None if add.is_empty() => Err("the map is a level already"),
        None => Ok(add),
    }
}
