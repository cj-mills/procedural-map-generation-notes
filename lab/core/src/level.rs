//! The level a finished map makes for the walk: its start, its exit and the caches. The caches
//! are placed here, by one rule, so a recipe gives the same level in the page, on the command
//! line and in an engine.
//!
//! The talk suggests hiding bonus things in the rooms off the hot path (at 22:51 in the
//! recording) and gives no code for it. This rule is our own.

use crate::analyze::distances;
use crate::grid::*;
use crate::Build;

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
    while out.len() < CACHES {
        let mut best: Option<usize> = None;
        for i in 0..g.len() {
            let open = matches!(g.t[i], FLOOR | GRASS) && hot[i] >= b.hot_radius;
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
