//! The step-through: frames recorded as a pipeline runs, so the page can show each step.
//!
//! A frame is a note (what the step did, as a key the page has text for), an argument (a room
//! number, an iteration), the tiles that changed since the previous frame, and marks: cells to
//! draw highlighted on this frame only (the newest addition, a rejected candidate, the path).
//! Noise stages also keep the field each frame shows, one byte a cell.

use crate::grid::Grid;

/// Marks. Below `GROUP` each is one kind of highlight; `GROUP + n` is group `n` (a room in the
/// order it was placed, a Voronoi cell), for coloring.
pub const NEW: u8 = 1;
pub const REJECTED: u8 = 2;
pub const POINT: u8 = 3;
pub const START: u8 = 4;
pub const EXIT: u8 = 5;
pub const PATH: u8 = 6;
pub const HOT: u8 = 7;
pub const UNREACHABLE: u8 = 8;
pub const IMPORTANT: u8 = 9;
pub const QUIET: u8 = 10;
pub const TARGET: u8 = 11;
pub const GROUP: u8 = 32;
pub const MARK_NAMES: [&str; 12] =
    ["", "new", "rejected", "point", "start", "exit", "path", "hot", "unreachable", "important", "quiet", "target"];

/// A cell index and a value in one `u32`: index in the high 24 bits, value in the low 8.
#[inline]
pub fn pack(i: usize, v: u8) -> u32 {
    ((i as u32) << 8) | v as u32
}

pub struct Frame {
    /// The pipeline stage that recorded it, in pre-order (`Build::stage_names`).
    pub stage: u16,
    pub note: Note,
    pub arg: i32,
    pub changes: Vec<u32>,
    pub marks: Vec<u32>,
    pub field: Vec<u8>,
}

pub struct Recorder {
    pub on: bool,
    pub frames: Vec<Frame>,
    pub stage: u16,
    last: Vec<u8>,
}

impl Recorder {
    pub fn new(on: bool) -> Self {
        Recorder { on, frames: Vec::new(), stage: 0, last: Vec::new() }
    }

    pub fn frame(&mut self, g: &Grid, note: Note, arg: i32, marks: Vec<u32>) {
        self.frame_with_field(g, note, arg, marks, Vec::new());
    }

    pub fn frame_with_field(&mut self, g: &Grid, note: Note, arg: i32, marks: Vec<u32>, field: Vec<u8>) {
        if !self.on {
            return;
        }
        if self.last.len() != g.t.len() {
            // The first frame, or a new grid size: every cell counts as changed from wall.
            self.last = vec![u8::MAX; g.t.len()];
        }
        let mut changes = Vec::new();
        for (i, (&now, was)) in g.t.iter().zip(self.last.iter_mut()).enumerate() {
            if now != *was {
                changes.push(pack(i, now));
                *was = now;
            }
        }
        self.frames.push(Frame { stage: self.stage, note, arg, changes, marks, field });
    }
}

/// Marks for every cell of `cells` with the one value `v`.
pub fn marks_of(g: &Grid, cells: impl IntoIterator<Item = (i32, i32)>, v: u8) -> Vec<u32> {
    cells.into_iter().filter(|&(x, y)| g.inside(x, y)).map(|(x, y)| pack(g.idx(x, y), v)).collect()
}

macro_rules! notes {
    ($($name:ident = $key:literal,)*) => {
        /// What a frame shows, as a stable key the page holds the annotation for.
        #[derive(Clone, Copy, PartialEq, Debug)]
        #[repr(u8)]
        pub enum Note { $($name,)* }
        pub const NOTE_KEYS: &[&str] = &[$($key,)*];
    };
}

notes! {
    RoomAdd = "rooms.add",
    RoomDiscard = "rooms.discard",
    RoomSort = "rooms.sort",
    RoomCorridor = "rooms.corridor",
    BspSubdivide = "bsp.subdivide",
    BspAdd = "bsp.add",
    CellularNoise = "cellular.noise",
    CellularIterate = "cellular.iterate",
    DrunkardSolid = "drunkard.solid",
    DrunkardDig = "drunkard.dig",
    DlaSeed = "dla.seed",
    DlaParticle = "dla.particle",
    ErodeStart = "dla-erode.start",
    ErodeParticle = "dla-erode.particle",
    VoronoiSeeds = "voronoi.seeds",
    VoronoiPythagoras = "voronoi.membership.pythagoras",
    VoronoiManhattan = "voronoi.membership.manhattan",
    VoronoiChebyshev = "voronoi.membership.chebyshev",
    VoronoiWalls = "voronoi.walls",
    VoronoiRoads = "voronoi.roads",
    NoiseMap = "noise.map",
    NoiseFirst = "noise.first",
    NoiseSecond = "noise.second",
    NoiseZoomedOut = "noise.zoomed-out",
    NoiseScale = "noise.scale",
    CombineInput = "combine.input",
    CombineHalves = "combine.halves",
    PrefabShow = "prefab.show",
    PrefabPlaced = "prefab.placed",
    PrefabNoPlace = "prefab.no-place",
    CullCenter = "cull.center",
    CullMarked = "cull.marked",
    CullRemoved = "cull.removed",
    StartDesired = "start.desired",
    StartPlaced = "start.placed",
    ExitPlaced = "exit.placed",
    ExitNoPath = "exit.no-path",
    HotPath = "hot-path.marked",
    HotPathCulled = "hot-path.culled",
    HotRooms = "hot-rooms.marked",
}
