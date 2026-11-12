//! The walk through a level: where the walker is, the route so far, what they have seen and
//! which caches they have found. One set of rules for the page, the command line and the
//! checks, so a recipe and a list of moves give the same walk everywhere.
//!
//! The talk has no walk; these rules are our own.
//!
//! A level with a locked door (`analyze::lock`) has two goals: the door can't be walked through,
//! and blocks the view as a wall does, until the walker has stepped on the key.

use crate::generate::line;
use crate::grid::*;
use crate::{level, Build};

/// How far the walker sees, in tiles.
pub const SIGHT: i32 = 9;
/// A tile's state in `Walk::seen`.
pub const UNSEEN: u8 = 0;
pub const REMEMBERED: u8 = 1;
pub const IN_VIEW: u8 = 2;
/// A move as a letter, in the order `step` numbers them: left, right, up, down. A route written
/// with these letters replays a walk (`map-lab walk --moves`).
pub const MOVES: [u8; 4] = *b"LRUD";

pub struct Walk {
    g: Grid,
    exit: usize,
    /// The locked door, while it is closed: it opens once the walker has the key.
    door: Option<usize>,
    key: Option<usize>,
    /// Whether the walker has stepped on the key.
    pub has_key: bool,
    /// The level's caches (`level::caches`).
    pub caches: Vec<usize>,
    /// For each cache, whether the walker has stepped on it.
    pub found: Vec<bool>,
    /// Every tile the walker has stood on, in order, the start first.
    pub route: Vec<usize>,
    /// For each tile, `UNSEEN`, `REMEMBERED` or `IN_VIEW`.
    pub seen: Vec<u8>,
}

impl Walk {
    /// A walk from the level's start. `None` when the map has no start, no exit or no path
    /// between them, or its key is behind its locked door: there is no level to walk.
    pub fn new(b: &Build) -> Option<Walk> {
        let (start, exit) = (b.start?, b.exit?);
        if level::finish(b).is_empty() {
            return None;
        }
        let caches = level::caches(b);
        let mut w = Walk { g: b.grid(), exit, door: b.door, key: b.key, has_key: false, found: vec![false; caches.len()], caches, route: vec![start], seen: vec![UNSEEN; b.tiles.len()] };
        w.look();
        Some(w)
    }
    /// The tile the walker is on.
    pub fn at(&self) -> usize {
        self.route[self.route.len() - 1]
    }
    /// The walk is over once the walker stands on the exit.
    pub fn done(&self) -> bool {
        self.at() == self.exit
    }
    /// One step left, right, up or down (0 to 3, `MOVES`). False, and nothing changes, when
    /// the tile there can't be walked on, is the locked door while the walker has no key, or
    /// the walk is over. Stepping on a cache finds it; stepping on the key takes it.
    pub fn step(&mut self, dir: u8) -> bool {
        let (x, y) = self.g.xy(self.at());
        let (nx, ny) = [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)][dir as usize & 3];
        if self.done() || !self.open(nx, ny) {
            return false;
        }
        let to = self.g.idx(nx, ny);
        if self.key == Some(to) {
            self.has_key = true;
        }
        self.route.push(to);
        if let Some(n) = self.caches.iter().position(|&c| c == to) {
            self.found[n] = true;
        }
        self.look();
        true
    }
    /// Whether a tile can be walked on now: a closed door can't, and blocks the view as a wall does.
    fn open(&self, x: i32, y: i32) -> bool {
        walkable(self.g.get(x, y)) && (self.has_key || self.door != Some(self.g.idx(x, y)))
    }
    /// The field of view. A tile is in view when it is within `SIGHT` of the walker in a
    /// straight line and every tile between them, on the line from the walker to it, can be
    /// walked on. A tile that can't be walked on is also in view when it touches, corners
    /// included, a tile in view that can: without that, a corridor's walls show only in
    /// patches, where the line to them happens to stay in the corridor. A tile that was in view
    /// stays remembered.
    fn look(&mut self) {
        for s in &mut self.seen {
            *s = (*s).min(REMEMBERED);
        }
        let (px, py) = self.g.xy(self.at());
        for y in py - SIGHT..=py + SIGHT {
            for x in px - SIGHT..=px + SIGHT {
                if !self.g.inside(x, y) || (x - px) * (x - px) + (y - py) * (y - py) > SIGHT * SIGHT {
                    continue;
                }
                let between = line((px, py), (x, y));
                if between.iter().skip(1).rev().skip(1).all(|&(bx, by)| self.open(bx, by)) {
                    let i = self.g.idx(x, y);
                    self.seen[i] = IN_VIEW;
                }
            }
        }
        for y in py - SIGHT - 1..=py + SIGHT + 1 {
            for x in px - SIGHT - 1..=px + SIGHT + 1 {
                if !self.g.inside(x, y) || self.open(x, y) {
                    continue;
                }
                let lit = |(nx, ny): (i32, i32)| self.open(nx, ny) && self.seen[self.g.idx(nx, ny)] == IN_VIEW;
                if (-1..=1).any(|dy| (-1..=1).any(|dx| lit((x + dx, y + dy)))) {
                    let i = self.g.idx(x, y);
                    self.seen[i] = IN_VIEW;
                }
            }
        }
    }
}
