//! The map lab core for the page, through wasm-bindgen. A `Lab` is one recipe built with every
//! frame kept; the page steps through the frames and reads the result.
//!
//! Packed values: a frame's changes and marks are `u32`s, cell index in the high 24 bits and
//! tile or mark in the low 8 (`map_lab_core::record`). Cell index = y × width + x. -1 means none.

use map_lab_core::grid::Grid;
use map_lab_core::record::{MARK_NAMES, NOTE_KEYS};
use map_lab_core::traits::{self, FAILURES};
use map_lab_core::{build, recipe, tiles_hash, Build};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Lab {
    b: Build,
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
        Ok(Lab { b: build(&r, true) })
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
    /// The measured traits in the data file's column order: floor share, regions, dead ends,
    /// corridor ratio, path (NaN for none), probed (0 or 1), water, grass, mountain, field mean,
    /// field sd (NaN without a field), rooms (NaN without rooms).
    pub fn traits(&self) -> Vec<f64> {
        let t = self.b.traits();
        let nan = f64::NAN;
        vec![
            t.floor_share,
            t.regions as f64,
            t.dead_ends as f64,
            t.corridor_ratio,
            t.path.map_or(nan, |p| p as f64),
            t.probed as u8 as f64,
            t.tile_shares[2],
            t.tile_shares[3],
            t.tile_shares[4],
            t.field.map_or(nan, |f| f.0),
            t.field.map_or(nan, |f| f.1),
            t.rooms.map_or(nan, |r| r as f64),
        ]
    }
    /// Failure bits; `failure_key(bit)` names each.
    pub fn failures(&self) -> u32 {
        let g = Grid { w: self.b.width, h: self.b.height, t: self.b.tiles.clone() };
        self.b.stage_failures | traits::failures(&g, &self.b.traits())
    }
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
