//! A recipe: a seed, a size and a pipeline of stages. Parsed from a line-based text form by a
//! hand-written parser, or (feature `serde`) from JSON through serde_json, to measure both.
//!
//! Text form, one stage per line, `#` starts a comment:
//!
//! ```text
//! seed 7
//! size 80 50
//! cellular fill=45 iterations=12
//! cull
//! start side=left
//! exit mode=far
//! hotpath
//! ```

#[cfg(feature = "serde")]
use serde::Deserialize;

#[derive(Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Deserialize), serde(rename_all = "lowercase"))]
pub enum DlaMode {
    Walk,
    Attract,
    Symmetric,
    Erode,
}

#[derive(Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Deserialize), serde(rename_all = "lowercase"))]
pub enum Metric {
    Euclid,
    Manhattan,
}

#[derive(Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Deserialize), serde(rename_all = "lowercase"))]
pub enum NoiseMode {
    Height,
    Overworld,
    Double,
}

#[derive(Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Deserialize), serde(rename_all = "lowercase"))]
pub enum Join {
    Split,
    Union,
    Intersect,
}

#[derive(Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Deserialize), serde(rename_all = "lowercase"))]
pub enum Side {
    Left,
    Center,
}

#[derive(Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Deserialize), serde(rename_all = "lowercase"))]
pub enum ExitMode {
    Right,
    Far,
}

#[derive(Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Deserialize), serde(rename_all = "lowercase"))]
pub enum Place {
    Center,
    Fit,
}

#[derive(Clone, Copy)]
#[cfg_attr(feature = "serde", derive(Deserialize), serde(tag = "stage", rename_all = "lowercase"))]
pub enum Stage {
    Rooms { count: u32, min: u32, max: u32 },
    Bsp { attempts: u32, min: u32 },
    Cellular { fill: u32, iterations: u32 },
    Drunkard { floor: u32, lifetime: u32, center: bool },
    Dla { mode: DlaMode, floor: u32 },
    Voronoi { seeds: u32, metric: Metric },
    Noise { mode: NoiseMode, octaves: u32, gain: f32, lacunarity: f32, freq: f32, scale: f32 },
    Prefab { place: Place },
    Layer,
    Combine { join: Join },
    Cull,
    Start { side: Side },
    Exit { mode: ExitMode },
    Hotpath,
    Hotrooms,
}

#[cfg_attr(feature = "serde", derive(Deserialize))]
pub struct Recipe {
    pub seed: u64,
    pub width: i32,
    pub height: i32,
    pub stages: Vec<Stage>,
}

pub type ParseError = (&'static str, u32);

#[cfg(feature = "serde")]
pub fn parse(src: &str) -> Result<Recipe, ParseError> {
    let r: Recipe = serde_json::from_str(src).map_err(|e| ("invalid recipe JSON", e.line() as u32))?;
    if r.width < 8 || r.height < 8 || r.width > 512 || r.height > 512 {
        return Err(("size out of range", 0));
    }
    Ok(r)
}

#[cfg(not(feature = "serde"))]
pub fn parse(src: &str) -> Result<Recipe, ParseError> {
    let mut r = Recipe { seed: 1, width: 80, height: 50, stages: Vec::new() };
    for (n, raw) in src.lines().enumerate() {
        let line_no = n as u32 + 1;
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let mut words = line.split_ascii_whitespace();
        let head = words.next().unwrap_or("");
        let mut kv = Params { words: words.clone(), line: line_no };
        let e = |m: &'static str| (m, line_no);
        match head {
            "seed" => r.seed = int(words.next().ok_or(e("seed needs a value"))?, line_no)? as u64,
            "size" => {
                r.width = int(words.next().ok_or(e("size needs a width"))?, line_no)? as i32;
                r.height = int(words.next().ok_or(e("size needs a height"))?, line_no)? as i32;
                if r.width < 8 || r.height < 8 || r.width > 512 || r.height > 512 {
                    return Err(e("size out of range"));
                }
            }
            _ => r.stages.push(stage(head, &mut kv)?),
        }
    }
    Ok(r)
}

#[cfg(not(feature = "serde"))]
fn stage(head: &str, p: &mut Params) -> Result<Stage, ParseError> {
    Ok(match head {
        "rooms" => Stage::Rooms { count: p.u("count", 20)?, min: p.u("min", 4)?, max: p.u("max", 10)? },
        "bsp" => Stage::Bsp { attempts: p.u("attempts", 240)?, min: p.u("min", 6)? },
        "cellular" => Stage::Cellular { fill: p.u("fill", 45)?, iterations: p.u("iterations", 15)? },
        "drunkard" => Stage::Drunkard {
            floor: p.u("floor", 33)?,
            lifetime: p.u("lifetime", 400)?,
            center: p.word("from")?.map_or(Ok(true), |w| pick(w, &[("center", true), ("random", false)], p.line))?,
        },
        "dla" => Stage::Dla {
            mode: p.word("mode")?.map_or(Ok(DlaMode::Walk), |w| {
                pick(
                    w,
                    &[("walk", DlaMode::Walk), ("attract", DlaMode::Attract), ("symmetric", DlaMode::Symmetric), ("erode", DlaMode::Erode)],
                    p.line,
                )
            })?,
            floor: p.u("floor", 33)?,
        },
        "voronoi" => Stage::Voronoi {
            seeds: p.u("seeds", 32)?,
            metric: p.word("metric")?.map_or(Ok(Metric::Euclid), |w| {
                pick(w, &[("euclid", Metric::Euclid), ("manhattan", Metric::Manhattan)], p.line)
            })?,
        },
        "noise" => Stage::Noise {
            mode: p.word("mode")?.map_or(Ok(NoiseMode::Overworld), |w| {
                pick(w, &[("height", NoiseMode::Height), ("overworld", NoiseMode::Overworld), ("double", NoiseMode::Double)], p.line)
            })?,
            octaves: p.u("octaves", 5)?,
            gain: p.f("gain", 0.5)?,
            lacunarity: p.f("lacunarity", 2.0)?,
            freq: p.f("freq", 2.0)?,
            scale: p.f("scale", 1.0)?,
        },
        "prefab" => Stage::Prefab {
            place: p.word("place")?.map_or(Ok(Place::Fit), |w| pick(w, &[("center", Place::Center), ("fit", Place::Fit)], p.line))?,
        },
        "layer" => Stage::Layer,
        "combine" => Stage::Combine {
            join: p.word("join")?.map_or(Ok(Join::Split), |w| {
                pick(w, &[("split", Join::Split), ("union", Join::Union), ("intersect", Join::Intersect)], p.line)
            })?,
        },
        "cull" => Stage::Cull,
        "start" => Stage::Start {
            side: p.word("side")?.map_or(Ok(Side::Left), |w| pick(w, &[("left", Side::Left), ("center", Side::Center)], p.line))?,
        },
        "exit" => Stage::Exit {
            mode: p.word("mode")?.map_or(Ok(ExitMode::Far), |w| pick(w, &[("right", ExitMode::Right), ("far", ExitMode::Far)], p.line))?,
        },
        "hotpath" => Stage::Hotpath,
        "hotrooms" => Stage::Hotrooms,
        _ => return Err(("unknown stage", p.line)),
    })
}

#[cfg(not(feature = "serde"))]
struct Params<'a> {
    words: core::str::SplitAsciiWhitespace<'a>,
    line: u32,
}

#[cfg(not(feature = "serde"))]
impl<'a> Params<'a> {
    fn word(&self, key: &str) -> Result<Option<&'a str>, ParseError> {
        for w in self.words.clone() {
            let (k, v) = w.split_once('=').ok_or(("expected key=value", self.line))?;
            if k == key {
                return Ok(Some(v));
            }
        }
        Ok(None)
    }
    fn u(&self, key: &str, default: u32) -> Result<u32, ParseError> {
        self.word(key)?.map_or(Ok(default), |v| int(v, self.line).map(|n| n as u32))
    }
    fn f(&self, key: &str, default: f32) -> Result<f32, ParseError> {
        self.word(key)?.map_or(Ok(default), |v| decimal(v, self.line))
    }
}

#[cfg(not(feature = "serde"))]
fn pick<T: Copy>(w: &str, table: &[(&str, T)], line: u32) -> Result<T, ParseError> {
    table.iter().find(|(k, _)| *k == w).map(|(_, v)| *v).ok_or(("unknown value", line))
}

#[cfg(not(feature = "serde"))]
fn int(s: &str, line: u32) -> Result<u64, ParseError> {
    if s.is_empty() || s.len() > 12 {
        return Err(("expected a whole number", line));
    }
    let mut n: u64 = 0;
    for b in s.bytes() {
        if !b.is_ascii_digit() {
            return Err(("expected a whole number", line));
        }
        n = n * 10 + (b - b'0') as u64;
    }
    Ok(n)
}

/// A decimal as mantissa / 10^k: one correctly rounded division, so the same value everywhere.
#[cfg(not(feature = "serde"))]
fn decimal(s: &str, line: u32) -> Result<f32, ParseError> {
    let (neg, body) = match s.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, s),
    };
    let (ip, fp) = body.split_once('.').unwrap_or((body, ""));
    if fp.len() > 6 || (ip.is_empty() && fp.is_empty()) {
        return Err(("expected a decimal", line));
    }
    let mant = int(&[ip, fp].concat(), line).map_err(|_| ("expected a decimal", line))?;
    if mant > 1 << 24 {
        return Err(("decimal too precise", line));
    }
    const POW: [f32; 7] = [1.0, 10.0, 100.0, 1e3, 1e4, 1e5, 1e6];
    let v = mant as f32 / POW[fp.len()];
    Ok(if neg { -v } else { v })
}
