//! Fractal simplex noise as the talk uses it: the library's `SimplexFractal` with fractional
//! Brownian motion (`c10-perlin/src/main.rs:11-34`), thresholded into water, grass and mountain
//! (`c11-overworld/src/main.rs:28-47`), zoomed (`c12-noisezoom/src/main.rs:31-91`), or two noises
//! blended as the zoom deepens (`c13-double-noise/src/main.rs:31-164`).
//!
//! Our own implementation of 2D simplex noise as FastNoise defines it, the library behind the
//! talk's `FastNoise` (bracket-noise 0.8.2, MIT): its twelve gradients, its `70 ×` scale and its
//! fractal bounding, which sums one more octave than it samples. Only `+ − × ÷`, comparisons and
//! integer casts, so the same field natively and in WebAssembly.

use crate::grid::*;
use crate::record::*;
use crate::recipe::Stage;
use crate::rng::Rng;
use crate::Ctx;

const GRAD_X: [f32; 12] = [1., -1., 1., -1., 1., -1., 1., -1., 0., 0., 0., 0.];
const GRAD_Y: [f32; 12] = [1., 1., -1., -1., 0., 0., 0., 0., 1., -1., 1., -1.];
const SQRT3: f32 = 1.732_050_8;
const F2: f32 = 0.5 * (SQRT3 - 1.0);
const G2: f32 = (3.0 - SQRT3) / 6.0;

pub struct Fbm {
    perm: [u8; 512],
    perm12: [u8; 512],
    octaves: i32,
    gain: f32,
    lacunarity: f32,
    frequency: f32,
    bounding: f32,
}

#[inline]
fn floor(f: f32) -> i32 {
    if f >= 0.0 { f as i32 } else { f as i32 - 1 }
}

impl Fbm {
    pub fn new(seed: u64, octaves: i32, gain: f32, lacunarity: f32, frequency: f32) -> Self {
        let mut rng = Rng::new(seed);
        let mut perm = [0u8; 512];
        for (i, p) in perm.iter_mut().take(256).enumerate() {
            *p = i as u8;
        }
        for j in 0..256 {
            let k = rng.range(j, 256) as usize;
            perm.swap(j as usize, k);
        }
        let mut perm12 = [0u8; 512];
        for j in 0..256 {
            perm[j + 256] = perm[j];
            perm12[j] = perm[j] % 12;
            perm12[j + 256] = perm12[j];
        }
        let mut amp = gain;
        let mut sum = 1.0;
        for _ in 0..octaves {
            sum += amp;
            amp *= gain;
        }
        Fbm { perm, perm12, octaves, gain, lacunarity, frequency, bounding: 1.0 / sum }
    }

    pub fn get(&self, x: f32, y: f32) -> f32 {
        let (mut x, mut y) = (x * self.frequency, y * self.frequency);
        let mut sum = self.simplex(self.perm[0], x, y);
        let mut amp = 1.0;
        for i in 1..self.octaves {
            x *= self.lacunarity;
            y *= self.lacunarity;
            amp *= self.gain;
            sum += self.simplex(self.perm[i as usize], x, y) * amp;
        }
        sum * self.bounding
    }

    #[inline]
    fn grad(&self, offset: u8, i: i32, j: i32, x: f32, y: f32) -> f32 {
        let g = self.perm12[(i & 0xff) as usize + self.perm[(j & 0xff) as usize + offset as usize] as usize] as usize;
        x * GRAD_X[g] + y * GRAD_Y[g]
    }

    fn simplex(&self, offset: u8, x: f32, y: f32) -> f32 {
        let t = (x + y) * F2;
        let i = floor(x + t);
        let j = floor(y + t);
        let t = (i + j) as f32 * G2;
        let x0 = x - (i as f32 - t);
        let y0 = y - (j as f32 - t);
        let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };
        let x1 = x0 - i1 as f32 + G2;
        let y1 = y0 - j1 as f32 + G2;
        let x2 = x0 - 1.0 + 2.0 * G2;
        let y2 = y0 - 1.0 + 2.0 * G2;
        let corner = |dx: f32, dy: f32, ci: i32, cj: i32| {
            let t = 0.5 - dx * dx - dy * dy;
            if t < 0.0 {
                0.0
            } else {
                let t = t * t;
                t * t * self.grad(offset, ci, cj, dx, dy)
            }
        };
        70.0 * (corner(x0, y0, i, j) + corner(x1, y1, i + i1, j + j1) + corner(x2, y2, i + 1, j + 1))
    }
}

/// The talk's thresholds: below zero is water, below a half grass, the rest mountain.
fn terrain(n: f32) -> u8 {
    if n < 0.0 {
        WATER
    } else if n < 0.5 {
        GRASS
    } else {
        MOUNTAIN
    }
}

/// One byte a cell for the step-through: the talk's colour driver, 0 to 1, scaled to 0 to 255.
fn byte(v: f32) -> u8 {
    let v = if v < 0.0 { 0.0 } else if v > 1.0 { 1.0 } else { v };
    (v * 255.0 + 0.5) as u8
}

/// The zoom's scales, accumulated in `f32` exactly as the talk's loop does: 1.0, 0.99, … while
/// above 0.1 (`c12-noisezoom/src/main.rs:77-91`).
pub fn zoom_scales() -> Vec<f32> {
    let mut out = Vec::new();
    let mut s: f32 = 1.0;
    while s > 0.1 {
        out.push(s);
        s -= 0.01;
    }
    out
}

pub fn noise(c: &mut Ctx, s: &Stage) {
    let (mode, octaves, gain, lacunarity, frequency) = (s.pick(0), s.int(1), s.num(2), s.num(3), s.num(4));
    // Every noise stage of a recipe uses the same noise, from the recipe's seed, as each of the
    // talk's demos fixes one seed for its noise (`c10-perlin/src/main.rs:13`); the second noise of
    // `double` has its own (`c13-double-noise/src/main.rs:31`).
    let first = Fbm::new(c.seed ^ 0x6E6F_6973_6531, octaves, gain, lacunarity, frequency);
    let (w, h) = (c.g.w, c.g.h);
    let cells = (w * h) as usize;
    let mut field = vec![0f32; cells];
    let sample = |field: &mut Vec<f32>, f: &dyn Fn(f32, f32) -> f32| {
        for y in 0..h {
            for x in 0..w {
                field[(y * w + x) as usize] = f(x as f32, y as f32);
            }
        }
    };
    match mode {
        // height: a grey-scale height map, (noise + 1) / 2 (`c10-perlin/src/main.rs:21-26`).
        1 => {
            sample(&mut field, &|x, y| (first.get(x / 50.0, y / 25.0) + 1.0) / 2.0);
            c.g.t.iter_mut().for_each(|t| *t = FLOOR);
            let bytes = if c.rec.on { field.iter().map(|&v| byte(v)).collect() } else { Vec::new() };
            c.rec.frame_with_field(&c.g, Note::NoiseMap, 0, Vec::new(), bytes);
        }
        // overworld: one map from the noise (`c11-overworld/src/main.rs:28-47`).
        0 => {
            sample(&mut field, &|x, y| first.get(x / 100.0, y / 50.0));
            show(c, &field, Note::NoiseMap, 0);
        }
        // zoom and double: the map redrawn as the sampling scale shrinks.
        _ => {
            let second = (mode == 3).then(|| {
                Fbm::new(c.seed ^ 0x6E6F_6973_6532, octaves / 2, gain / 2.0, lacunarity + 1.0, frequency * 4.0)
            });
            if let Some(second) = &second {
                sample(&mut field, &|x, y| first.get(x * 0.5, y * 0.5));
                show(c, &field, Note::NoiseFirst, 0);
                sample(&mut field, &|x, y| second.get(x * 0.5, y * 0.5));
                show(c, &field, Note::NoiseSecond, 0);
            }
            let at = |field: &mut Vec<f32>, s: f32| match &second {
                None => sample(field, &|x, y| first.get(x * (s * 0.5), y * (s * 0.5))),
                Some(second) => sample(field, &|x, y| {
                    let mut n = first.get(x * (s * 0.5), y * (s * 0.5));
                    n *= f32::max(0.5, s);
                    n + f32::min(0.25, 0.75 - s) * second.get(x * s, y * s)
                }),
            };
            let scales = zoom_scales();
            if c.rec.on {
                at(&mut field, 1.0);
                show(c, &field, Note::NoiseZoomedOut, 100);
                for &s in &scales {
                    at(&mut field, s);
                    show(c, &field, Note::NoiseScale, (s * 100.0 + 0.5) as i32);
                }
            } else {
                at(&mut field, *scales.last().unwrap());
                show(c, &field, Note::NoiseScale, 0);
            }
        }
    }
    c.field = field;
}

fn show(c: &mut Ctx, field: &[f32], note: Note, arg: i32) {
    for (t, &n) in c.g.t.iter_mut().zip(field) {
        *t = terrain(n);
    }
    let bytes = if c.rec.on { field.iter().map(|&n| byte((n + 1.0) / 2.0)).collect() } else { Vec::new() };
    c.rec.frame_with_field(&c.g, note, arg, Vec::new(), bytes);
}
