//! The map: a grid of tiles, and rectangles with the talk's conventions.

/// Tile values. In the talk's code `#` is open and `.` is solid (`backend/src/lib.rs:47-68`);
/// here every tile is named for what it is.
pub const WALL: u8 = 0;
pub const FLOOR: u8 = 1;
pub const WATER: u8 = 2;
pub const GRASS: u8 = 3;
pub const MOUNTAIN: u8 = 4;
pub const TREASURE: u8 = 5;
pub const TRAP: u8 = 6;
pub const TILE_KINDS: usize = 7;

/// The glyph for each tile value, as the CLI prints it.
pub const GLYPHS: [u8; TILE_KINDS] = [b'#', b'.', b'~', b'"', b'^', b'$', b'!'];
pub const TILE_NAMES: [&str; TILE_KINDS] = ["wall", "floor", "water", "grass", "mountain", "treasure", "trap"];

/// What can be walked on, for every analysis stage and measured trait: floor and the things
/// placed on it, and grass. Water and mountains are not.
#[inline]
pub fn walkable(t: u8) -> bool {
    matches!(t, FLOOR | GRASS | TREASURE | TRAP)
}

#[derive(Clone)]
pub struct Grid {
    pub w: i32,
    pub h: i32,
    pub t: Vec<u8>,
}

impl Grid {
    pub fn new(w: i32, h: i32, fill: u8) -> Self {
        Grid { w, h, t: vec![fill; (w * h) as usize] }
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.t.len()
    }
    #[inline]
    pub fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h
    }
    #[inline]
    pub fn idx(&self, x: i32, y: i32) -> usize {
        (y * self.w + x) as usize
    }
    #[inline]
    pub fn xy(&self, i: usize) -> (i32, i32) {
        (i as i32 % self.w, i as i32 / self.w)
    }
    /// Outside the grid reads as wall.
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> u8 {
        if self.inside(x, y) { self.t[self.idx(x, y)] } else { WALL }
    }
    /// Outside the grid is ignored.
    #[inline]
    pub fn set(&mut self, x: i32, y: i32, v: u8) {
        if self.inside(x, y) {
            let i = self.idx(x, y);
            self.t[i] = v;
        }
    }
    pub fn count(&self, v: u8) -> usize {
        self.t.iter().filter(|&&c| c == v).count()
    }
    pub fn indices_of(&self, v: u8) -> Vec<usize> {
        (0..self.len()).filter(|&i| self.t[i] == v).collect()
    }
    /// The four orthogonal neighbors of `i` inside the grid, in the order left, right, up, down
    /// (the order of the talk's `get_available_exits`, `backend/src/lib.rs:48-68`).
    #[inline]
    pub fn neighbors(&self, i: usize, out: &mut [usize; 4]) -> usize {
        let (x, y) = self.xy(i);
        let mut n = 0;
        if x > 0 {
            out[n] = i - 1;
            n += 1;
        }
        if x < self.w - 1 {
            out[n] = i + 1;
            n += 1;
        }
        if y > 0 {
            out[n] = i - self.w as usize;
            n += 1;
        }
        if y < self.h - 1 {
            out[n] = i + self.w as usize;
            n += 1;
        }
        n
    }
    /// The talk's L-shaped corridor between two room centers; `horizontal_first` is its coin.
    pub fn corridor(&mut self, a: (i32, i32), b: (i32, i32), horizontal_first: bool) {
        if horizontal_first {
            self.h_line(a.0, b.0, a.1);
            self.v_line(a.1, b.1, b.0);
        } else {
            self.v_line(a.1, b.1, a.0);
            self.h_line(a.0, b.0, b.1);
        }
    }
    fn h_line(&mut self, x1: i32, x2: i32, y: i32) {
        for x in x1.min(x2)..=x1.max(x2) {
            self.set(x, y, FLOOR);
        }
    }
    fn v_line(&mut self, y1: i32, y2: i32, x: i32) {
        for y in y1.min(y2)..=y1.max(y2) {
            self.set(x, y, FLOOR);
        }
    }
}

/// A rectangle as the talk's library defines one: `sized(x, y, w, h)` runs from `(x, y)` to
/// `(x + w, y + h)`, and its cells include both corners, so it covers `(w + 1) × (h + 1)` tiles.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Rect {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

impl Rect {
    pub fn sized(x: i32, y: i32, w: i32, h: i32) -> Self {
        Rect { x1: x, y1: y, x2: x + w, y2: y + h }
    }
    /// Touching counts as intersecting.
    pub fn intersects(&self, o: &Rect) -> bool {
        self.x1 <= o.x2 && self.x2 >= o.x1 && self.y1 <= o.y2 && self.y2 >= o.y1
    }
    pub fn grown(&self, by: i32) -> Rect {
        Rect { x1: self.x1 - by, y1: self.y1 - by, x2: self.x2 + by, y2: self.y2 + by }
    }
    pub fn center(&self) -> (i32, i32) {
        ((self.x1 + self.x2) / 2, (self.y1 + self.y2) / 2)
    }
    pub fn width(&self) -> i32 {
        (self.x2 - self.x1).abs()
    }
    pub fn height(&self) -> i32 {
        (self.y2 - self.y1).abs()
    }
    /// The far edges excluded, unlike the cells.
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x1 && x < self.x2 && y >= self.y1 && y < self.y2
    }
    /// Every cell, both corners included, row by row.
    pub fn cells(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        (self.y1..=self.y2).flat_map(move |y| (self.x1..=self.x2).map(move |x| (x, y)))
    }
}

/// The index of the walkable tile nearest `(px, py)` by straight-line distance, the first in
/// row order on a tie (the talk's `min_by` over the tiles, e.g. `c17-starting-left/src/main.rs:71-86`).
pub fn nearest_walkable(g: &Grid, px: i32, py: i32) -> Option<usize> {
    let mut best: Option<(usize, i64)> = None;
    for i in 0..g.len() {
        if !walkable(g.t[i]) {
            continue;
        }
        let (x, y) = g.xy(i);
        let d = ((x - px) as i64).pow(2) + ((y - py) as i64).pow(2);
        if best.is_none_or(|(_, b)| d < b) {
            best = Some((i, d));
        }
    }
    best.map(|(i, _)| i)
}
