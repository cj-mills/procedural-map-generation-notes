//! The map lab core for the page, through wasm-bindgen. A `Lab` is one recipe built with every
//! frame kept; the page steps through the frames and reads the result, the level's caches, and
//! the recipe measured over many seeds. It also holds the walk through the level: the page
//! sends each step and draws what comes back. `stage_table` is what the page's controls are
//! made from.
//!
//! Packed values: a frame's changes and marks are `u32`s, cell index in the high 24 bits and
//! tile or mark in the low 8 (`map_lab_core::record`). Cell index = y × width + x. -1 means none.

use map_lab_core::record::{MARK_NAMES, NOTE_KEYS};
use map_lab_core::traits::{self, FAILURES};
use map_lab_core::recipe::{self, Recipe};
use map_lab_core::walk::Walk;
use map_lab_core::{build, level, tiles_hash, Build};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Lab {
    r: Recipe,
    b: Build,
    /// `measure`'s rows so far.
    rows: Vec<f64>,
    walk: Option<Walk>,
}

fn opt(i: Option<usize>) -> i32 {
    i.map_or(-1, |i| i as i32)
}

#[wasm_bindgen]
impl Lab {
    /// Parse and build a recipe (JSON text); throws with where the recipe is wrong.
    #[wasm_bindgen(constructor)]
    pub fn new(recipe_json: &str) -> Result<Lab, JsError> {
        let r = recipe::parse(recipe_json).map_err(|e| JsError::new(&e.message()))?;
        Ok(Lab { b: build(&r, true), r, rows: Vec::new(), walk: None })
    }
    pub fn width(&self) -> i32 {
        self.b.width
    }
    pub fn height(&self) -> i32 {
        self.b.height
    }
    /// The finished map, one tile value a cell.
    pub fn tiles(&self) -> Vec<u8> {
        self.b.tiles.clone()
    }
    pub fn tiles_hash(&self) -> u32 {
        tiles_hash(&self.b.tiles)
    }
    pub fn frame_count(&self) -> usize {
        self.b.frames.len()
    }
    pub fn frame_stage(&self, i: usize) -> u16 {
        self.b.frames[i].stage
    }
    pub fn frame_note(&self, i: usize) -> u8 {
        self.b.frames[i].note as u8
    }
    pub fn frame_arg(&self, i: usize) -> i32 {
        self.b.frames[i].arg
    }
    pub fn frame_changes(&self, i: usize) -> Vec<u32> {
        self.b.frames[i].changes.clone()
    }
    pub fn frame_marks(&self, i: usize) -> Vec<u32> {
        self.b.frames[i].marks.clone()
    }
    /// A noise frame's field, one byte a cell; empty for other frames.
    pub fn frame_field(&self, i: usize) -> Vec<u8> {
        self.b.frames[i].field.clone()
    }
    pub fn stage_count(&self) -> usize {
        self.b.stage_names.len()
    }
    pub fn stage_name(&self, i: usize) -> String {
        self.b.stage_names[i].into()
    }
    pub fn start(&self) -> i32 {
        opt(self.b.start)
    }
    pub fn exit(&self) -> i32 {
        opt(self.b.exit)
    }
    pub fn path(&self) -> Vec<u32> {
        self.b.path.iter().map(|&i| i as u32).collect()
    }
    /// Walking distance from the start (or the cull point), -1 unreachable; empty if neither.
    pub fn distances(&self) -> Vec<i32> {
        self.b.dist.clone()
    }
    /// Walking distance from the path, after a hot-path stage.
    pub fn hot(&self) -> Vec<i32> {
        self.b.hot.clone()
    }
    pub fn field(&self) -> Vec<f32> {
        self.b.field.clone()
    }
    /// Five numbers a room: x1, y1, x2, y2 (cells include both corners) and 1 if the path passes
    /// through it after a hot-rooms stage, else 0.
    pub fn rooms(&self) -> Vec<i32> {
        let rooms = self.b.rooms.as_deref().unwrap_or(&[]);
        rooms
            .iter()
            .enumerate()
            .flat_map(|(n, r)| [r.x1, r.y1, r.x2, r.y2, self.b.important.get(n).copied().unwrap_or(false) as i32])
            .collect()
    }
    /// The level's caches, as cell indices: placed off the hot path by the core's one rule
    /// (`map_lab_core::level`). Empty without a path from the start to the exit.
    pub fn caches(&self) -> Vec<u32> {
        level::caches(&self.b).iter().map(|&i| i as u32).collect()
    }
    /// The measured traits in the data file's column order: floor share, regions, dead ends,
    /// corridor ratio, path (NaN for none), probed (0 or 1), water, grass, mountain, field mean,
    /// field sd (NaN without a field), rooms (NaN without rooms).
    pub fn traits(&self) -> Vec<f64> {
        self.b.traits().values().into()
    }
    /// Failure bits; `failure_key(bit)` names each.
    pub fn failures(&self) -> u32 {
        self.b.failures()
    }
    /// The locked door and its key as cell indices, after a lock stage that placed them; -1 for none.
    pub fn door(&self) -> i32 {
        opt(self.b.door)
    }
    pub fn key(&self) -> i32 {
        opt(self.b.key)
    }
    /// The shortest walk that finishes the level, as cell indices: the path, or with a locked
    /// door, by way of the key. Empty when the level can't be finished.
    pub fn finish(&self) -> Vec<u32> {
        level::finish(&self.b).iter().map(|&i| i as u32).collect()
    }
    /// Whether the walker has the key.
    pub fn has_key(&self) -> bool {
        self.walk.as_ref().is_some_and(|w| w.has_key)
    }
    /// Start the walk, or start it again, from the level's start (`map_lab_core::walk`). False
    /// when there is no level to walk: no start, no exit, or no path between them.
    pub fn walk(&mut self) -> bool {
        self.walk = Walk::new(&self.b);
        self.walk.is_some()
    }
    /// One step left, right, up or down (0 to 3). False, and nothing changes, when the tile
    /// there can't be walked on, the walk is over, or it hasn't been started.
    pub fn step(&mut self, dir: u8) -> bool {
        self.walk.as_mut().is_some_and(|w| w.step(dir))
    }
    /// Every tile the walker has stood on, as cell indices, the start first: the last is where
    /// they are, and its length less one is the steps taken. Empty before `walk`.
    pub fn route(&self) -> Vec<u32> {
        self.walk.as_ref().map_or(Vec::new(), |w| w.route.iter().map(|&i| i as u32).collect())
    }
    /// One byte a tile: 0 unseen, 1 seen earlier and remembered, 2 in view now. The walker
    /// sees nine tiles, in straight lines over tiles that can be walked on, and the walls that
    /// touch what they see. Empty before `walk`.
    pub fn seen(&self) -> Vec<u8> {
        self.walk.as_ref().map_or(Vec::new(), |w| w.seen.clone())
    }
    /// A bit for each cache in `caches` order, set once the walker has stepped on it.
    pub fn found(&self) -> u32 {
        self.walk.as_ref().map_or(0, |w| w.found.iter().enumerate().map(|(n, &f)| (f as u32) << n).sum())
    }
    /// Whether the walker has reached the exit.
    pub fn walk_done(&self) -> bool {
        self.walk.as_ref().is_some_and(|w| w.done())
    }
    /// "Measure over 200 seeds", a slice at a time so the page stays responsive: builds this
    /// recipe on the next `count` seeds, the first call starting at seed 0, and returns how many
    /// seeds are measured so far. The recipe's own seed is not used.
    pub fn measure(&mut self, count: u32) -> usize {
        let from = self.rows.len() / traits::ROW;
        self.rows.extend(map_lab_core::measure(&mut self.r, from as u64, count));
        from + count as usize
    }
    /// The seeds measured so far, 13 numbers a seed: the traits in `traits` order, then the
    /// seed's failure bits. Row `n` is seed `n`.
    pub fn measured(&self) -> Vec<f64> {
        self.rows.clone()
    }
    /// The summary of the seeds measured so far, as the data file summarizes a demo: for each
    /// of the 12 traits 8 numbers (seeds that have it, mean, standard deviation, minimum, 10th
    /// percentile, median, 90th percentile, maximum; NaN after the count when no seed has it),
    /// then for each failure bit the number of seeds failing it.
    pub fn summary(&self) -> Vec<f64> {
        traits::summarize(&self.rows)
    }
}

/// What a recipe may say, as JSON: every stage and its parameters with their kinds, limits and
/// defaults, for the page's controls (`map_lab_core::recipe::stage_table` documents the shape).
#[wasm_bindgen]
pub fn stage_table() -> String {
    recipe::stage_table()
}

/// The page's annotation key for a frame's note.
#[wasm_bindgen]
pub fn note_key(note: u8) -> String {
    NOTE_KEYS.get(note as usize).copied().unwrap_or("").into()
}

#[wasm_bindgen]
pub fn mark_name(mark: u8) -> String {
    MARK_NAMES.get(mark as usize).copied().unwrap_or("group").into()
}

#[wasm_bindgen]
pub fn failure_key(bit: u32) -> String {
    FAILURES.get(bit as usize).map_or("", |f| f.0).into()
}
