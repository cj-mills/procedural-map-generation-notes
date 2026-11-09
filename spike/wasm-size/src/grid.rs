//! The grid, a deterministic seeded random number generator, and rectangles.

pub const WALL: u8 = 0;
pub const FLOOR: u8 = 1;
pub const WATER: u8 = 2;
pub const GRASS: u8 = 3;
pub const MOUNTAIN: u8 = 4;

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
    pub fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h
    }
    #[inline]
    pub fn idx(&self, x: i32, y: i32) -> usize {
        (y * self.w + x) as usize
    }
    /// Outside the grid reads as wall.
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> u8 {
        if self.inside(x, y) { self.t[self.idx(x, y)] } else { WALL }
    }
    #[inline]
    pub fn set(&mut self, x: i32, y: i32, v: u8) {
        if self.inside(x, y) {
            let i = self.idx(x, y);
            self.t[i] = v;
        }
    }
    pub fn walkable(v: u8) -> bool {
        v == FLOOR || v == GRASS
    }
    pub fn floor_at(&self, x: i32, y: i32) -> bool {
        Self::walkable(self.get(x, y))
    }
    pub fn count(&self, v: u8) -> usize {
        self.t.iter().filter(|&&c| c == v).count()
    }
    pub fn fill_rect(&mut self, r: &Rect, v: u8) {
        for y in r.y1..r.y2 {
            for x in r.x1..r.x2 {
                self.set(x, y, v);
            }
        }
    }
    pub fn h_tunnel(&mut self, x1: i32, x2: i32, y: i32) {
        for x in x1.min(x2)..=x1.max(x2) {
            self.set(x, y, FLOOR);
        }
    }
    pub fn v_tunnel(&mut self, y1: i32, y2: i32, x: i32) {
        for y in y1.min(y2)..=y1.max(y2) {
            self.set(x, y, FLOOR);
        }
    }
    /// An L-shaped corridor between two points, bend chosen by the coin.
    pub fn corridor(&mut self, a: (i32, i32), b: (i32, i32), coin: bool) {
        if coin {
            self.h_tunnel(a.0, b.0, a.1);
            self.v_tunnel(a.1, b.1, b.0);
        } else {
            self.v_tunnel(a.1, b.1, a.0);
            self.h_tunnel(a.0, b.0, b.1);
        }
    }
}

#[derive(Clone, Copy)]
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
    pub fn overlaps(&self, o: &Rect, margin: i32) -> bool {
        self.x1 - margin < o.x2 && self.x2 + margin > o.x1 && self.y1 - margin < o.y2 && self.y2 + margin > o.y1
    }
    pub fn center(&self) -> (i32, i32) {
        ((self.x1 + self.x2) / 2, (self.y1 + self.y2) / 2)
    }
    pub fn width(&self) -> i32 {
        self.x2 - self.x1
    }
    pub fn height(&self) -> i32 {
        self.y2 - self.y1
    }
}

/// PCG32 (O'Neill), seeded through SplitMix64: the same sequence on every platform.
#[derive(Clone)]
pub struct Rng {
    state: u64,
    inc: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        let mut s = seed;
        let mut split = || {
            s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        };
        let mut r = Rng { state: split(), inc: split() | 1 };
        r.next_u32();
        r
    }
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }
    /// Uniform in `lo..hi` (hi exclusive); `lo` when the range is empty.
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        let span = (hi - lo) as u64;
        lo + ((self.next_u32() as u64 * span) >> 32) as i32
    }
    /// True with probability `pct` percent.
    pub fn chance(&mut self, pct: u32) -> bool {
        (self.range(0, 100) as u32) < pct
    }
    pub fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
    }
    /// One of the four orthogonal steps.
    pub fn step(&mut self) -> (i32, i32) {
        DIRS[self.range(0, 4) as usize]
    }
}

pub const DIRS: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
