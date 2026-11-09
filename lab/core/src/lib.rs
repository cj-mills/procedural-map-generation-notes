//! The map lab's generator core: every technique of Herbert Wolverson's talk *Procedural Map
//! Generation Techniques* (Roguelike Celebration 2020), as stages of a pipeline. A pipeline plus
//! a seed is a recipe (`recipe`); building one gives the same map on every platform, natively
//! and as WebAssembly, tile for tile.
//!
//! The talk's code (thebracket/roguelike-celebration-2020 at `604e5b5a`) is cited by file and
//! line beside each stage; none of it is copied.

pub mod analyse;
pub mod generate;
pub mod grid;
pub mod json;
pub mod noise;
pub mod place;
pub mod recipe;
pub mod record;
pub mod rng;
pub mod traits;
#[cfg(feature = "report")]
pub mod report;

use grid::{Grid, Rect, WALL};
use recipe::{Kind, Recipe, Stage};
use record::{Frame, Recorder};
use rng::Rng;
pub use traits::{EXIT_UNREACHABLE, GUARD_HIT, NO_FLOOR, PREFAB_NO_PLACE};

/// A pipeline's working state as its stages run.
pub struct Ctx<'r> {
    pub seed: u64,
    pub g: Grid,
    pub rng: &'r mut Rng,
    pub rec: &'r mut Recorder,
    names: &'r mut Vec<&'static str>,
    pub rooms: Vec<Rect>,
    /// Whether `rooms` came from the current map's generator.
    pub made_rooms: bool,
    pub start: Option<usize>,
    pub exit: Option<usize>,
    pub path: Vec<usize>,
    /// Walking distance from the cull point or the start, whichever came last; -1 unreachable.
    pub dist: Vec<i32>,
    /// Walking distance from the path, after a hot-path stage.
    pub hot: Vec<i32>,
    /// For each room, whether the path passes through it, after a hot-rooms stage.
    pub important: Vec<bool>,
    pub field: Vec<f32>,
    pub failures: u32,
}

/// A finished map and everything the page, the CLI and the measurements read from it.
pub struct Build {
    pub width: i32,
    pub height: i32,
    pub tiles: Vec<u8>,
    pub frames: Vec<Frame>,
    /// Every stage in the order it ran (pre-order through `combine` inputs); a frame's `stage`
    /// indexes it.
    pub stage_names: Vec<&'static str>,
    /// The rooms of the map's generator, when it was `rooms` or `bsp`; sorted by left edge.
    pub rooms: Option<Vec<Rect>>,
    pub important: Vec<bool>,
    pub start: Option<usize>,
    pub exit: Option<usize>,
    pub path: Vec<usize>,
    pub dist: Vec<i32>,
    pub hot: Vec<i32>,
    pub field: Vec<f32>,
    /// Failures set by the stages as they ran (`traits::FAILURES`, the first four).
    pub stage_failures: u32,
}

impl Build {
    pub fn grid(&self) -> Grid {
        Grid { w: self.width, h: self.height, t: self.tiles.clone() }
    }
    pub fn traits(&self) -> traits::Traits {
        let rooms = self.rooms.as_ref().map(|r| r.len() as u32);
        traits::measure(&self.grid(), self.start, self.exit, &self.field, rooms)
    }
    /// Every failure: the stages' and the traits'.
    pub fn failures(&self) -> u32 {
        let t = self.traits();
        self.stage_failures | traits::failures(&self.grid(), &t)
    }
}

/// Build a recipe's map; with `record`, keep a frame for each step.
pub fn build(r: &Recipe, record: bool) -> Build {
    let mut rng = Rng::new(r.seed);
    let mut rec = Recorder::new(record);
    let mut names = Vec::new();
    let mut c = Ctx::fresh(r.seed, r.width, r.height, &mut rng, &mut rec, &mut names);
    run_stages(&mut c, &r.pipeline);
    let Ctx { g, rooms, made_rooms, important, start, exit, path, dist, hot, field, failures, .. } = c;
    let rooms = made_rooms.then_some(rooms);
    Build {
        width: r.width,
        height: r.height,
        tiles: g.t,
        frames: rec.frames,
        stage_names: names,
        rooms,
        important,
        start,
        exit,
        path,
        dist,
        hot,
        field,
        stage_failures: failures,
    }
}

impl<'r> Ctx<'r> {
    fn fresh(seed: u64, w: i32, h: i32, rng: &'r mut Rng, rec: &'r mut Recorder, names: &'r mut Vec<&'static str>) -> Self {
        Ctx {
            seed,
            g: Grid::new(w, h, WALL),
            rng,
            rec,
            names,
            rooms: Vec::new(),
            made_rooms: false,
            start: None,
            exit: None,
            path: Vec::new(),
            dist: Vec::new(),
            hot: Vec::new(),
            important: Vec::new(),
            field: Vec::new(),
            failures: 0,
        }
    }
}

/// Run a pipeline on a fresh map of the same size, sharing the random numbers and the frames:
/// `combine`'s inputs.
pub fn run_sub(c: &mut Ctx, stages: &[Stage]) -> (Grid, u32) {
    let mut sub = Ctx::fresh(c.seed, c.g.w, c.g.h, c.rng, c.rec, c.names);
    run_stages(&mut sub, stages);
    (sub.g, sub.failures)
}

fn run_stages(c: &mut Ctx, stages: &[Stage]) {
    for s in stages {
        c.rec.stage = c.names.len() as u16;
        c.names.push(s.spec().name);
        if s.kind.is_generator() {
            c.rooms.clear();
            c.made_rooms = false;
            c.start = None;
            c.exit = None;
            c.path.clear();
            c.dist.clear();
            c.hot.clear();
            c.important.clear();
            c.field.clear();
        }
        match s.kind {
            Kind::Rooms => generate::rooms(c, s),
            Kind::Bsp => generate::bsp(c, s),
            Kind::Cellular => generate::cellular(c, s),
            Kind::Drunkard => generate::drunkard(c, s),
            Kind::Dla => generate::dla(c, s),
            Kind::DlaErode => generate::dla_erode(c, s),
            Kind::Voronoi => generate::voronoi(c, s),
            Kind::Noise => noise::noise(c, s),
            Kind::Combine => place::combine(c, s),
            Kind::Prefab => place::prefab(c, s),
            Kind::Cull => analyse::cull(c, s),
            Kind::Start => analyse::start(c, s),
            Kind::Exit => analyse::exit(c, s),
            Kind::HotPath => analyse::hot_path(c, s),
            Kind::HotRooms => analyse::hot_rooms(c, s),
        }
    }
}

/// FNV-1a over the tiles: the check that two builds made the same map.
pub fn tiles_hash(tiles: &[u8]) -> u32 {
    tiles.iter().fold(0x811c_9dc5u32, |h, &b| (h ^ b as u32).wrapping_mul(0x0100_0193))
}
