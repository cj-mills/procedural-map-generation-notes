//! The core's one source of randomness: PCG32 (O'Neill, XSH-RR) seeded through SplitMix64.
//! Integer-only, so it gives the same sequence on every platform and in WebAssembly.

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

    pub fn next_u64(&mut self) -> u64 {
        ((self.next_u32() as u64) << 32) | self.next_u32() as u64
    }

    /// Uniform in `lo..hi`, `hi` exclusive (the talk's `rng.range`); `lo` when the range is empty.
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        let span = (hi as i64 - lo as i64) as u64;
        lo + ((self.next_u32() as u64 * span) >> 32) as i32
    }

    /// One die of `sides` sides, `1..=sides` (the talk's `rng.roll_dice(1, sides)`).
    pub fn die(&mut self, sides: i32) -> i32 {
        self.range(1, sides + 1)
    }

    /// An index into a slice of `len` items, as the talk's `random_slice_entry` picks one: a die
    /// roll, and no draw at all for a single item.
    pub fn pick(&mut self, len: usize) -> Option<usize> {
        match len {
            0 => None,
            1 => Some(0),
            n => Some(self.die(n as i32) as usize - 1),
        }
    }
}
