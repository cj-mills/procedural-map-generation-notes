//! A recipe is a pipeline plus a seed: everything needed to build one map exactly.
//!
//! ```json
//! { "format": "map-lab-recipe/1", "seed": 7, "width": 80, "height": 50,
//!   "pipeline": [ { "stage": "cellular", "wall_percent": 55, "iterations": 10 },
//!                 { "stage": "cull" }, { "stage": "start", "at": "left" } ] }
//! ```
//!
//! `STAGES` is the one owner of what a recipe may say: the parser reads it, the stages read
//! their parameters by its order, and the CLI writes the published JSON Schema from it.

use crate::json::{self, Json};

pub const FORMAT: &str = "map-lab-recipe/1";
pub const MAX_SEED: u64 = (1 << 53) - 1;
pub const MIN_SIDE: i64 = 24;
pub const MAX_SIDE: i64 = 256;

pub enum Param {
    /// A whole number in `min..=max`.
    Int { min: i64, max: i64, default: i64 },
    /// A decimal in `min..=max`, at most six digits after the point.
    Num { min: f32, max: f32, default: f32 },
    /// One of the named options; the default is the first.
    Pick(&'static [&'static str]),
    /// Exactly `n` pipelines, each a list of stages, run in order on fresh grids.
    Inputs(usize),
}

pub struct ParamSpec {
    pub name: &'static str,
    pub kind: Param,
}

pub struct StageSpec {
    pub name: &'static str,
    pub params: &'static [ParamSpec],
    /// "talk" first: the talk's version. Labelled variants, each with its own source and a
    /// measured difference, go after it.
    pub variants: &'static [&'static str],
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(u8)]
pub enum Kind {
    Rooms,
    Bsp,
    Cellular,
    Drunkard,
    Dla,
    DlaErode,
    Voronoi,
    Noise,
    Combine,
    Prefab,
    Cull,
    Start,
    Exit,
    HotPath,
    HotRooms,
}

const fn int(name: &'static str, min: i64, max: i64, default: i64) -> ParamSpec {
    ParamSpec { name, kind: Param::Int { min, max, default } }
}
const fn num(name: &'static str, min: f32, max: f32, default: f32) -> ParamSpec {
    ParamSpec { name, kind: Param::Num { min, max, default } }
}
const fn pick(name: &'static str, options: &'static [&'static str]) -> ParamSpec {
    ParamSpec { name, kind: Param::Pick(options) }
}
const TALK: &[&str] = &["talk"];

/// In `Kind` order. Each default is the talk's own value.
pub const STAGES: &[StageSpec] = &[
    StageSpec {
        name: "rooms",
        params: &[int("attempts", 1, 500, 20), int("min_size", 1, 30, 2), int("max_size", 1, 30, 9)],
        variants: TALK,
    },
    StageSpec { name: "bsp", params: &[int("attempts", 1, 5000, 240)], variants: TALK },
    StageSpec {
        name: "cellular",
        params: &[int("wall_percent", 0, 100, 55), int("iterations", 0, 100, 10)],
        variants: TALK,
    },
    StageSpec {
        name: "drunkard",
        params: &[int("lifetime", 1, 100_000, 200), int("floor_divisor", 1, 20, 3)],
        variants: TALK,
    },
    StageSpec {
        name: "dla",
        params: &[pick("mode", &["walk", "attractor", "symmetric"]), int("floor_divisor", 1, 20, 3)],
        variants: TALK,
    },
    StageSpec { name: "dla-erode", params: &[int("iterations", 0, 100_000, 500)], variants: TALK },
    StageSpec {
        name: "voronoi",
        params: &[int("seeds", 1, 200, 16), pick("metric", &["pythagoras", "manhattan", "chebyshev"])],
        variants: TALK,
    },
    StageSpec {
        name: "noise",
        params: &[
            pick("mode", &["overworld", "height", "zoom", "double"]),
            int("octaves", 1, 16, 10),
            num("gain", 0.0, 4.0, 0.1),
            num("lacunarity", 0.0, 16.0, 5.0),
            num("frequency", 0.0, 64.0, 2.0),
        ],
        variants: TALK,
    },
    StageSpec {
        name: "combine",
        params: &[pick("join", &["halves"]), ParamSpec { name: "inputs", kind: Param::Inputs(2) }],
        variants: TALK,
    },
    StageSpec {
        name: "prefab",
        params: &[pick("shape", &["vault", "spine"]), pick("place", &["room", "open-area", "center"])],
        variants: TALK,
    },
    StageSpec { name: "cull", params: &[pick("from", &["center"]), pick("mode", &["remove", "mark"])], variants: TALK },
    StageSpec { name: "start", params: &[pick("at", &["left", "center"])], variants: TALK },
    StageSpec { name: "exit", params: &[pick("at", &["right", "farthest"])], variants: TALK },
    StageSpec { name: "hot-path", params: &[int("radius", 1, 1000, 10), int("cull", 0, 1000, 0)], variants: TALK },
    StageSpec { name: "hot-rooms", params: &[], variants: TALK },
];

pub enum Val {
    Int(i64),
    Num(f32),
    Pick(u8),
    Inputs(Vec<Vec<Stage>>),
}

pub struct Stage {
    pub kind: Kind,
    pub variant: u8,
    pub vals: Vec<Val>,
}

impl Stage {
    pub fn spec(&self) -> &'static StageSpec {
        &STAGES[self.kind as usize]
    }
    pub fn int(&self, i: usize) -> i32 {
        match self.vals[i] {
            Val::Int(v) => v as i32,
            _ => unreachable!(),
        }
    }
    pub fn num(&self, i: usize) -> f32 {
        match self.vals[i] {
            Val::Num(v) => v,
            _ => unreachable!(),
        }
    }
    pub fn pick(&self, i: usize) -> u8 {
        match self.vals[i] {
            Val::Pick(v) => v,
            _ => unreachable!(),
        }
    }
    pub fn inputs(&self, i: usize) -> &[Vec<Stage>] {
        match &self.vals[i] {
            Val::Inputs(v) => v,
            _ => unreachable!(),
        }
    }
}

pub struct Recipe {
    pub seed: u64,
    pub width: i32,
    pub height: i32,
    pub pipeline: Vec<Stage>,
}

/// Where a recipe is wrong: `at` is "line 3, column 7" for malformed JSON, or the path to the
/// value, such as `pipeline[2].attempts`.
pub struct RecipeError {
    pub msg: &'static str,
    pub at: String,
}

impl RecipeError {
    pub fn message(&self) -> String {
        let mut s = self.at.clone();
        s.push_str(": ");
        s.push_str(self.msg);
        s
    }
}

pub fn parse(src: &str) -> Result<Recipe, RecipeError> {
    let doc = json::parse(src).map_err(|e| {
        let mut at = String::from("line ");
        push_uint(&mut at, e.line as u64);
        at.push_str(", column ");
        push_uint(&mut at, e.col as u64);
        RecipeError { msg: e.msg, at }
    })?;
    let fields = object(&doc, "recipe")?;
    let mut seed = None;
    let mut width = 80;
    let mut height = 50;
    let mut pipeline = None;
    let mut format_ok = false;
    for (k, v) in fields {
        let at = |m| err(m, k);
        match k.as_str() {
            "format" => match v {
                Json::Str(s) if s == FORMAT => format_ok = true,
                _ => return Err(at("format must be \"map-lab-recipe/1\"")),
            },
            "seed" => seed = Some(whole(v, 0, MAX_SEED as i64, k)? as u64),
            "width" => width = whole(v, MIN_SIDE, MAX_SIDE, k)? as i32,
            "height" => height = whole(v, MIN_SIDE, MAX_SIDE, k)? as i32,
            "pipeline" => pipeline = Some(stages(v, "pipeline")?),
            "title" | "note" => {
                if !matches!(v, Json::Str(_)) {
                    return Err(at("expected a string"));
                }
            }
            _ => return Err(at("unknown key")),
        }
    }
    if !format_ok {
        return Err(err("format is required: \"map-lab-recipe/1\"", "format"));
    }
    let seed = seed.ok_or_else(|| err("seed is required", "seed"))?;
    let pipeline = pipeline.ok_or_else(|| err("pipeline is required", "pipeline"))?;
    check_order(&pipeline, "pipeline")?;
    Ok(Recipe { seed, width, height, pipeline })
}

fn err(msg: &'static str, at: &str) -> RecipeError {
    RecipeError { msg, at: at.into() }
}

fn object<'a, 'b>(v: &'b Json<'a>, at: &str) -> Result<&'b [(String, Json<'a>)], RecipeError> {
    match v {
        Json::Obj(f) => Ok(f),
        _ => Err(err("expected an object", at)),
    }
}

fn stages(v: &Json, at: &str) -> Result<Vec<Stage>, RecipeError> {
    let Json::Arr(items) = v else { return Err(err("expected a list of stages", at)) };
    if items.is_empty() {
        return Err(err("a pipeline needs at least one stage", at));
    }
    let mut out = Vec::with_capacity(items.len());
    for (n, item) in items.iter().enumerate() {
        let mut here = String::from(at);
        here.push('[');
        push_uint(&mut here, n as u64);
        here.push(']');
        out.push(stage(item, &here)?);
    }
    Ok(out)
}

fn stage(v: &Json, at: &str) -> Result<Stage, RecipeError> {
    let fields = object(v, at)?;
    let path = |k: &str| {
        let mut s = String::from(at);
        s.push('.');
        s.push_str(k);
        s
    };
    let name = match fields.iter().find(|(k, _)| k == "stage") {
        Some((_, Json::Str(s))) => s.as_str(),
        Some(_) => return Err(RecipeError { msg: "expected a stage name", at: path("stage") }),
        None => return Err(RecipeError { msg: "each stage needs \"stage\"", at: at.into() }),
    };
    let kind = STAGES.iter().position(|s| s.name == name).ok_or(RecipeError { msg: "unknown stage", at: path("stage") })?;
    let spec = &STAGES[kind];
    // `KINDS` lists `Kind` in `STAGES` order.
    let kind = KINDS[kind];
    let mut variant = 0u8;
    let mut vals: Vec<Option<Val>> = spec.params.iter().map(|_| None).collect();
    for (k, v) in fields {
        if k == "stage" {
            continue;
        }
        if k == "variant" {
            variant = choose(v, spec.variants, &path(k))?;
            continue;
        }
        let i = spec.params.iter().position(|p| p.name == k).ok_or(RecipeError { msg: "unknown parameter", at: path(k) })?;
        let here = path(k);
        vals[i] = Some(match spec.params[i].kind {
            Param::Int { min, max, .. } => Val::Int(whole(v, min, max, &here)?),
            Param::Num { min, max, .. } => Val::Num(decimal(v, min, max, &here)?),
            Param::Pick(options) => Val::Pick(choose(v, options, &here)?),
            Param::Inputs(n) => {
                let Json::Arr(list) = v else { return Err(err("expected a list of pipelines", &here)) };
                if list.len() != n {
                    return Err(err("wrong number of inputs", &here));
                }
                let mut ins = Vec::new();
                for (j, p) in list.iter().enumerate() {
                    let mut at = here.clone();
                    at.push('[');
                    push_uint(&mut at, j as u64);
                    at.push(']');
                    ins.push(stages(p, &at)?);
                }
                Val::Inputs(ins)
            }
        });
    }
    let mut out = Vec::with_capacity(vals.len());
    for (v, p) in vals.into_iter().zip(spec.params) {
        out.push(match (v, &p.kind) {
            (Some(v), _) => v,
            (None, Param::Int { default, .. }) => Val::Int(*default),
            (None, Param::Num { default, .. }) => Val::Num(*default),
            (None, Param::Pick(_)) => Val::Pick(0),
            (None, Param::Inputs(_)) => return Err(RecipeError { msg: "required", at: path(p.name) }),
        });
    }
    let s = Stage { kind, variant, vals: out };
    match kind {
        Kind::Rooms if s.int(2) < s.int(1) => Err(RecipeError { msg: "max_size is below min_size", at: path("max_size") }),
        Kind::Prefab if s.pick(0) == 1 && s.pick(1) != 2 => {
            Err(RecipeError { msg: "the spine prefab spans the map's height: place it at \"center\"", at: path("place") })
        }
        _ => Ok(s),
    }
}

impl Kind {
    /// Stages that start a new map: everything placed on the old one is forgotten.
    pub fn is_generator(self) -> bool {
        matches!(
            self,
            Kind::Rooms | Kind::Bsp | Kind::Cellular | Kind::Drunkard | Kind::Dla | Kind::Voronoi | Kind::Noise | Kind::Combine
        )
    }
}

/// Stages that read what an earlier stage placed must come after it in the same pipeline.
fn check_order(stages: &[Stage], at: &str) -> Result<(), RecipeError> {
    let (mut rooms, mut start, mut exit) = (false, false, false);
    for (n, s) in stages.iter().enumerate() {
        let mut here = String::from(at);
        here.push('[');
        push_uint(&mut here, n as u64);
        here.push(']');
        if s.kind.is_generator() {
            (rooms, start, exit) = (matches!(s.kind, Kind::Rooms | Kind::Bsp), false, false);
        }
        let missing = match s.kind {
            Kind::Combine => {
                for (j, p) in s.inputs(1).iter().enumerate() {
                    let mut sub = here.clone();
                    sub.push_str(".inputs[");
                    push_uint(&mut sub, j as u64);
                    sub.push(']');
                    check_order(p, &sub)?;
                }
                None
            }
            Kind::Prefab if s.pick(1) == 0 && !rooms => Some("prefab place=room needs a rooms or bsp stage before it"),
            Kind::Exit if !start => Some("exit needs a start stage before it"),
            Kind::HotPath if !exit => Some("hot-path needs an exit stage before it"),
            Kind::HotRooms if !(exit && rooms) => Some("hot-rooms needs a rooms or bsp stage and an exit stage before it"),
            Kind::DlaErode if n == 0 => Some("dla-erode erodes a map: it needs a stage before it"),
            Kind::Start => {
                start = true;
                None
            }
            Kind::Exit => {
                exit = true;
                None
            }
            _ => None,
        };
        if let Some(msg) = missing {
            return Err(RecipeError { msg, at: here });
        }
    }
    Ok(())
}

const KINDS: [Kind; 15] = [
    Kind::Rooms,
    Kind::Bsp,
    Kind::Cellular,
    Kind::Drunkard,
    Kind::Dla,
    Kind::DlaErode,
    Kind::Voronoi,
    Kind::Noise,
    Kind::Combine,
    Kind::Prefab,
    Kind::Cull,
    Kind::Start,
    Kind::Exit,
    Kind::HotPath,
    Kind::HotRooms,
];

fn choose(v: &Json, options: &[&str], at: &str) -> Result<u8, RecipeError> {
    match v {
        Json::Str(s) => options.iter().position(|o| o == s).map(|i| i as u8).ok_or_else(|| err("not one of the allowed values", at)),
        _ => Err(err("expected a string", at)),
    }
}

fn whole(v: &Json, min: i64, max: i64, at: &str) -> Result<i64, RecipeError> {
    let Json::Num(s) = v else { return Err(err("expected a whole number", at)) };
    let (neg, digits) = match s.strip_prefix('-') {
        Some(d) => (true, d),
        None => (false, *s),
    };
    if digits.contains('.') || digits.len() > 16 {
        return Err(err("expected a whole number", at));
    }
    let mut n: i64 = 0;
    for b in digits.bytes() {
        n = n * 10 + (b - b'0') as i64;
    }
    let n = if neg { -n } else { n };
    if n < min || n > max {
        return Err(err("out of range", at));
    }
    Ok(n)
}

/// A decimal as mantissa / 10^k: one correctly rounded division, so the same `f32` everywhere,
/// and the same one Rust gives the talk's literals (`0.005` here is the talk's `0.005`).
fn decimal(v: &Json, min: f32, max: f32, at: &str) -> Result<f32, RecipeError> {
    let Json::Num(s) = v else { return Err(err("expected a number", at)) };
    let (neg, body) = match s.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, *s),
    };
    let (ip, fp) = body.split_once('.').unwrap_or((body, ""));
    if fp.len() > 6 || ip.len() + fp.len() > 9 {
        return Err(err("at most six decimal places and nine digits", at));
    }
    let mut mant: u32 = 0;
    for b in ip.bytes().chain(fp.bytes()) {
        mant = mant * 10 + (b - b'0') as u32;
    }
    if mant > 1 << 24 {
        return Err(err("too many significant digits", at));
    }
    const POW: [f32; 7] = [1.0, 10.0, 100.0, 1e3, 1e4, 1e5, 1e6];
    let x = mant as f32 / POW[fp.len()];
    let x = if neg { -x } else { x };
    if !(x >= min && x <= max) {
        return Err(err("out of range", at));
    }
    Ok(x)
}

/// Decimal digits without `core::fmt`, which the WebAssembly build leaves out.
pub fn push_uint(s: &mut String, n: u64) {
    if n >= 10 {
        push_uint(s, n / 10);
    }
    s.push((b'0' + (n % 10) as u8) as char);
}
