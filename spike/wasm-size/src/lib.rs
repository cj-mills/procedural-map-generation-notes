//! Size spike for the map lab's generator core: every code family the core needs, built to
//! measure what each costs as WebAssembly. Not the core itself.

pub mod generators;
pub mod grid;
pub mod recipe;
#[cfg(feature = "analysis")]
pub mod analysis;

use grid::*;
use recipe::*;

pub struct Frame {
    pub tiles: Vec<u8>,
    pub label: u16,
    pub stage: u16,
}

/// Everything a pipeline run builds up: the map, earlier layers, rooms, placements, frames.
pub struct Ctx {
    pub g: Grid,
    pub layers: Vec<Grid>,
    pub rng: Rng,
    pub rooms: Vec<Rect>,
    pub frames: Vec<Frame>,
    pub stage: u16,
    pub field: Vec<u8>,
    pub prefab: Option<(i32, i32, i32, i32)>,
    pub start: Option<(i32, i32)>,
    pub exit: Option<(i32, i32)>,
    pub dist: Vec<u16>,
    pub hot: Vec<u8>,
    pub path: Vec<(i32, i32)>,
    pub hot_rooms: Vec<u8>,
}

impl Ctx {
    pub fn new(r: &Recipe) -> Self {
        Ctx {
            g: Grid::new(r.width, r.height, WALL),
            layers: Vec::new(),
            rng: Rng::new(r.seed),
            rooms: Vec::new(),
            frames: Vec::new(),
            stage: 0,
            field: Vec::new(),
            prefab: None,
            start: None,
            exit: None,
            dist: Vec::new(),
            hot: Vec::new(),
            path: Vec::new(),
            hot_rooms: Vec::new(),
        }
    }
    /// Record the map as it stands, for the step-through.
    pub fn snap(&mut self, label: u16) {
        self.frames.push(Frame { tiles: self.g.t.clone(), label, stage: self.stage });
    }
}

pub const NOT_BUILT: &str = "stage not in this build";

pub fn run_stage(c: &mut Ctx, s: Stage) -> Result<(), &'static str> {
    #[allow(unreachable_patterns, unused_variables)]
    match s {
        #[cfg(feature = "rooms")]
        Stage::Rooms { count, min, max } => generators::rooms(c, count, min, max),
        #[cfg(feature = "bsp")]
        Stage::Bsp { attempts, min } => generators::bsp(c, attempts, min),
        #[cfg(feature = "cellular")]
        Stage::Cellular { fill, iterations } => generators::cellular(c, fill, iterations),
        #[cfg(feature = "drunkard")]
        Stage::Drunkard { floor, lifetime, center } => generators::drunkard(c, floor, lifetime, center),
        #[cfg(feature = "dla")]
        Stage::Dla { mode, floor } => generators::dla(c, mode, floor),
        #[cfg(feature = "voronoi")]
        Stage::Voronoi { seeds, metric } => generators::voronoi(c, seeds, metric),
        #[cfg(feature = "noise")]
        Stage::Noise { mode, octaves, gain, lacunarity, freq, scale } => generators::noise(c, mode, octaves, gain, lacunarity, freq, scale),
        #[cfg(feature = "prefab")]
        Stage::Prefab { place } => generators::prefab(c, place),
        #[cfg(feature = "combine")]
        Stage::Layer => {
            let fresh = Grid::new(c.g.w, c.g.h, WALL);
            c.layers.push(core::mem::replace(&mut c.g, fresh));
        }
        #[cfg(feature = "combine")]
        Stage::Combine { join } => generators::combine(c, join),
        #[cfg(feature = "analysis")]
        Stage::Cull => analysis::cull(c),
        #[cfg(feature = "analysis")]
        Stage::Start { side } => analysis::start(c, side),
        #[cfg(feature = "analysis")]
        Stage::Exit { mode } => analysis::exit(c, mode),
        #[cfg(feature = "analysis")]
        Stage::Hotpath => analysis::hot_path(c),
        #[cfg(feature = "analysis")]
        Stage::Hotrooms => analysis::hot_rooms(c),
        _ => return Err(NOT_BUILT),
    }
    Ok(())
}

pub fn traits(c: &Ctx) -> Vec<f32> {
    #[cfg(feature = "analysis")]
    return analysis::traits(c);
    #[cfg(not(feature = "analysis"))]
    vec![c.g.t.iter().filter(|&&t| Grid::walkable(t)).count() as f32 / c.g.t.len() as f32]
}

/// Build a whole recipe natively: the CLI's and the tests' entry point.
pub fn build(src: &str) -> Result<Ctx, ParseError> {
    let r = parse(src)?;
    let mut c = Ctx::new(&r);
    for (i, s) in r.stages.iter().enumerate() {
        c.stage = i as u16;
        run_stage(&mut c, *s).map_err(|m| (m, 0))?;
    }
    Ok(c)
}

#[cfg(feature = "wasm")]
mod web {
    use super::*;
    use wasm_bindgen::prelude::*;

    /// One recipe, built stage by stage: what the page drives.
    #[wasm_bindgen]
    pub struct Lab {
        ctx: Ctx,
        stages: Vec<Stage>,
        next: usize,
    }

    fn error(msg: &str, line: u32) -> JsError {
        // "line N: message" without core::fmt, which would cost several KB.
        let mut s = String::from(msg);
        if line > 0 {
            let mut digits = Vec::new();
            let mut n = line;
            while n > 0 {
                digits.push(b'0' + (n % 10) as u8);
                n /= 10;
            }
            digits.reverse();
            s.insert_str(0, ": ");
            s.insert_str(0, core::str::from_utf8(&digits).unwrap_or(""));
            s.insert_str(0, "line ");
        }
        JsError::new(&s)
    }

    #[wasm_bindgen]
    impl Lab {
        #[wasm_bindgen(constructor)]
        pub fn new(recipe: &str) -> Result<Lab, JsError> {
            let r = parse(recipe).map_err(|(m, l)| error(m, l))?;
            Ok(Lab { ctx: Ctx::new(&r), stages: r.stages, next: 0 })
        }
        /// Run the next stage; false once the pipeline is done.
        pub fn step(&mut self) -> Result<bool, JsError> {
            let Some(&s) = self.stages.get(self.next) else { return Ok(false) };
            self.ctx.stage = self.next as u16;
            run_stage(&mut self.ctx, s).map_err(|m| error(m, 0))?;
            self.next += 1;
            Ok(true)
        }
        pub fn run(&mut self) -> Result<(), JsError> {
            while self.step()? {}
            Ok(())
        }
        pub fn stages(&self) -> usize {
            self.stages.len()
        }
        pub fn width(&self) -> i32 {
            self.ctx.g.w
        }
        pub fn height(&self) -> i32 {
            self.ctx.g.h
        }
        pub fn tiles(&self) -> Vec<u8> {
            self.ctx.g.t.clone()
        }
        pub fn field(&self) -> Vec<u8> {
            self.ctx.field.clone()
        }
        pub fn frames(&self) -> usize {
            self.ctx.frames.len()
        }
        pub fn frame_tiles(&self, i: usize) -> Vec<u8> {
            self.ctx.frames.get(i).map(|f| f.tiles.clone()).unwrap_or_default()
        }
        /// The frame's annotation id and the stage it belongs to, packed as label << 16 | stage.
        pub fn frame_info(&self, i: usize) -> u32 {
            self.ctx.frames.get(i).map_or(0, |f| (f.label as u32) << 16 | f.stage as u32)
        }
        pub fn distances(&self) -> Vec<u16> {
            self.ctx.dist.clone()
        }
        pub fn hot(&self) -> Vec<u8> {
            self.ctx.hot.clone()
        }
        pub fn hot_rooms(&self) -> Vec<u8> {
            self.ctx.hot_rooms.clone()
        }
        /// [start x, start y, exit x, exit y], -1 where not placed.
        pub fn places(&self) -> Vec<i32> {
            let (s, e) = (self.ctx.start.unwrap_or((-1, -1)), self.ctx.exit.unwrap_or((-1, -1)));
            vec![s.0, s.1, e.0, e.1]
        }
        pub fn traits(&self) -> Vec<f32> {
            traits(&self.ctx)
        }
    }
}

#[cfg(all(test, not(feature = "serde")))]
mod tests {
    use super::*;

    /// One recipe per demo of the talk, c01 to c21 and c08a, shared with check-wasm.mjs.
    const DEMOS: &str = include_str!("../demos.txt");

    pub fn demos() -> Vec<String> {
        DEMOS.split("\n---\n").map(|d| d.trim().to_string()).collect()
    }

    /// FNV-1a over the final tiles: the same value natively and in WebAssembly.
    pub fn fnv(t: &[u8]) -> u32 {
        t.iter().fold(0x811c_9dc5u32, |h, &b| (h ^ b as u32).wrapping_mul(0x0100_0193))
    }

    #[test]
    fn native_hashes() {
        // check-wasm.mjs compares the module's output against these.
        let hashes: Vec<String> = demos().iter().map(|d| fnv(&build(d).unwrap().g.t).to_string()).collect();
        std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/target/native-hashes.txt"), hashes.join("\n") + "\n").unwrap();
    }

    #[test]
    fn every_demo_builds_and_is_deterministic() {
        let demos = demos();
        assert_eq!(demos.len(), 22);
        for (i, src) in demos.iter().enumerate() {
            let a = build(src).unwrap_or_else(|e| panic!("demo {}: {} (line {})", i + 1, e.0, e.1));
            let b = build(src).unwrap();
            assert!(a.g.t == b.g.t, "demo {} not deterministic", i + 1);
            assert!(!a.frames.is_empty(), "demo {} recorded no frames", i + 1);
            let t = traits(&a);
            assert!(t[0] > 0.0, "demo {} has no floor", i + 1);
        }
    }

    #[test]
    fn hot_path_reaches_the_exit() {
        let c = build("seed 3\ncellular\ncull\nstart\nexit mode=far\nhotpath").unwrap();
        assert_eq!(c.path.last().copied(), c.exit);
        assert_eq!(c.path.len() as f32 - 1.0, traits(&c)[4]);
    }

    #[test]
    fn decimals_parse_exactly() {
        let r = parse("noise gain=0.35 freq=-1.5").unwrap();
        match r.stages[0] {
            Stage::Noise { gain, freq, .. } => {
                assert_eq!(gain, 0.35f32);
                assert_eq!(freq, -1.5f32);
            }
            _ => panic!(),
        }
    }
}
