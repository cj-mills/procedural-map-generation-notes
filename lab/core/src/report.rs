//! The traits data file: per-seed traits for each recipe, their summaries and failing seeds,
//! written as JSON. Written by `map-lab measure` for our core and by the faithfulness harness for
//! the talk's own code, in one format, so `map-lab compare` reads both the same way.

use crate::traits::{self, Traits, FAILURES};

pub const FORMAT: &str = "map-lab-traits/1";

/// The per-seed columns, in order, with what each one is.
pub const COLUMNS: &[(&str, &str)] = &[
    ("floor_share", "Walkable tiles over all tiles. Walkable: floor, grass, treasure and traps; not wall, water or mountain."),
    ("regions", "Groups of walkable tiles joined in four directions."),
    ("dead_ends", "Walkable tiles with exactly one walkable neighbor of four."),
    ("corridor_ratio", "The share of walkable tiles in no 2 x 2 block of walkable tiles: passages one tile wide."),
    ("path", "Steps on the shortest four-way walk from start to exit; null when there is none."),
    ("probed", "1 when path used the standard probe (the open tile nearest the middle of the left edge to the one nearest the middle of the right) because the map has no start and exit of its own; 0 otherwise."),
    ("water", "Water tiles over all tiles."),
    ("grass", "Grass tiles over all tiles."),
    ("mountain", "Mountain tiles over all tiles."),
    ("field_mean", "The noise field's mean, for maps that have one; null otherwise."),
    ("field_sd", "The noise field's standard deviation, for maps that have one; null otherwise."),
    ("rooms", "Rooms placed, for maps built from rooms; null otherwise."),
];

/// A lock stage's per-seed columns (`level::lock_values`), in order. A map that is not a level
/// has null in every one.
pub const LOCK_COLUMNS: &[(&str, &str)] = &[
    ("chokepoints", "Tiles of the route, two steps or more from the start, that closed would cut the exit off from the start: the places the talk's flood fill would pass for a locked door."),
    ("door", "1 when the lock stage placed a door and its key, 0 when it found no place."),
    ("door_at", "Steps from the start to the door over the route's steps: 0.5 is the route's middle. Null without a door."),
    ("start_side", "Of the tiles the start can walk to, the share it still can with the door closed. Null without a door."),
    ("key_off_route", "Steps from the key to the nearest tile of the route, by walking. Null without a door."),
    ("finish", "Steps on the shortest walk that finishes the level: to the key with the door closed, then to the exit. Null without a door, or when the key is behind its door."),
    ("finish_ratio", "finish over the route's steps. Null where finish is."),
];

/// One seed's values, in `COLUMNS` order; `None` is null.
pub struct Row {
    pub seed: u64,
    pub values: Vec<Option<f64>>,
    pub failures: u32,
}

pub fn row(seed: u64, t: &Traits, failures: u32) -> Row {
    let values = t.values().iter().map(|&v| (!v.is_nan()).then_some(v)).collect();
    Row { seed, values, failures }
}

pub fn failure_names(f: u32) -> Vec<&'static str> {
    (0..FAILURES.len()).filter(|i| f & (1 << i) != 0).map(|i| FAILURES[i].0).collect()
}

/// A JSON number: the shortest decimal that reads back as the same `f64`; whole numbers bare.
pub fn num(x: f64) -> String {
    assert!(x.is_finite());
    format!("{x}")
}

pub fn string(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

pub fn list<I: IntoIterator<Item = String>>(items: I) -> String {
    let v: Vec<String> = items.into_iter().collect();
    format!("[{}]", v.join(", "))
}

/// A JSON object written key by key, values already JSON: on one line, or one key per line.
pub struct Obj {
    s: String,
    sep: String,
    close: String,
    empty: bool,
}

impl Obj {
    pub fn new() -> Self {
        Obj { s: "{".into(), sep: ", ".into(), close: "}".into(), empty: true }
    }
    /// One key per line, indented by `indent` spaces; the closing brace two spaces less.
    pub fn lines(indent: usize) -> Self {
        let pad = " ".repeat(indent);
        Obj { s: format!("{{\n{pad}"), sep: format!(",\n{pad}"), close: format!("\n{}}}", " ".repeat(indent - 2)), empty: true }
    }
    pub fn raw(&mut self, k: &str, v: &str) -> &mut Self {
        if !self.empty {
            self.s.push_str(&self.sep);
        }
        self.empty = false;
        self.s.push_str(&string(k));
        self.s.push_str(": ");
        self.s.push_str(v);
        self
    }
    pub fn end(&mut self) -> String {
        if self.empty {
            return "{}".into();
        }
        let mut s = std::mem::take(&mut self.s);
        s.push_str(&self.close);
        s
    }
}

impl Default for Obj {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary statistics of one column over seeds, nulls left out, as `traits::summary` defines
/// them and rounded to six places: n, mean, sample standard deviation, minimum, 10th percentile,
/// median, 90th percentile, maximum.
pub fn summary(values: &[f64], nulls: usize) -> String {
    let mut o = Obj::new();
    o.raw("n", &values.len().to_string());
    o.raw("nulls", &nulls.to_string());
    if let Some(s) = traits::summary(values) {
        for (k, x) in ["mean", "sd", "min", "p10", "median", "p90", "max"].iter().zip(&s[1..]) {
            o.raw(k, &num((x * 1e6).round() / 1e6));
        }
    }
    o.end()
}

/// One recipe's entry in the data file.
pub fn demo(id: &str, recipe: Option<(&str, &str)>, extra: &[(&str, String)], rows: &[Row]) -> String {
    let mut o = Obj::lines(6);
    o.raw("id", &string(id));
    if let Some((file, text)) = recipe {
        o.raw("recipeFile", &string(file));
        o.raw("recipe", text);
    }
    for (k, v) in extra {
        o.raw(k, v);
    }
    let mut sums = Obj::lines(8);
    for (c, (name, _)) in COLUMNS.iter().enumerate() {
        let vals: Vec<f64> = rows.iter().filter_map(|r| r.values[c]).collect();
        sums.raw(name, &summary(&vals, rows.len() - vals.len()));
    }
    o.raw("summary", &sums.end());
    let mut fails = Obj::new();
    for (b, (name, _)) in FAILURES.iter().enumerate() {
        let seeds: Vec<String> = rows.iter().filter(|r| r.failures & (1 << b) != 0).map(|r| r.seed.to_string()).collect();
        if !seeds.is_empty() {
            fails.raw(name, &list(seeds));
        }
    }
    o.raw("failingSeeds", &fails.end());
    let rows = rows.iter().map(|r| {
        let mut cells = vec![r.seed.to_string()];
        cells.extend(r.values.iter().map(|v| v.map_or("null".into(), num)));
        cells.push(list(failure_names(r.failures).into_iter().map(string)));
        list(cells)
    });
    o.raw("rows", &format!("[\n        {}\n      ]", rows.collect::<Vec<_>>().join(",\n        ")));
    o.end()
}

/// The whole data file.
pub fn traits_doc(generated_by: &str, seeds: (u64, u64), demos: &[String]) -> String {
    let mut o = Obj::lines(2);
    o.raw("format", &string(FORMAT));
    o.raw("generatedBy", &string(generated_by));
    let mut s = Obj::new();
    s.raw("from", &seeds.0.to_string()).raw("to", &seeds.1.to_string()).raw("note", &string("seeds from..to, to exclusive"));
    o.raw("seeds", &s.end());
    let mut cols = Obj::lines(4);
    for (k, v) in COLUMNS {
        cols.raw(k, &string(v));
    }
    o.raw("columns", &cols.end());
    o.raw("rowLayout", &string("each row: [seed, then the columns in order, then the seed's failures]"));
    let mut f = Obj::lines(4);
    for (k, v) in FAILURES {
        f.raw(k, &string(v));
    }
    o.raw("failures", &f.end());
    o.raw("demos", &format!("[\n    {}\n  ]", demos.join(",\n    ")));
    o.end()
}
