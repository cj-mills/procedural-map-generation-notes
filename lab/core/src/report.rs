//! The traits data file: per-seed traits for each recipe, their summaries and failing seeds,
//! written as JSON. Written by `map-lab measure` for our core and by the faithfulness harness for
//! the talk's own code, in one format, so `map-lab compare` reads both the same way.

use crate::traits::{Traits, FAILURES};

pub const FORMAT: &str = "map-lab-traits/1";

/// The per-seed columns, in order, with what each one is.
pub const COLUMNS: &[(&str, &str)] = &[
    ("floor_share", "Walkable tiles over all tiles. Walkable: floor, grass, treasure and traps; not wall, water or mountain."),
    ("regions", "Groups of walkable tiles joined in four directions."),
    ("dead_ends", "Walkable tiles with exactly one walkable neighbour of four."),
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

/// One seed's values, in `COLUMNS` order; `None` is null.
pub struct Row {
    pub seed: u64,
    pub values: Vec<Option<f64>>,
    pub failures: u32,
}

pub fn row(seed: u64, t: &Traits, failures: u32) -> Row {
    let values = vec![
        Some(t.floor_share),
        Some(t.regions as f64),
        Some(t.dead_ends as f64),
        Some(t.corridor_ratio),
        t.path.map(|p| p as f64),
        Some(if t.probed { 1.0 } else { 0.0 }),
        Some(t.tile_shares[2]),
        Some(t.tile_shares[3]),
        Some(t.tile_shares[4]),
        t.field.map(|f| f.0),
        t.field.map(|f| f.1),
        t.rooms.map(|r| r as f64),
    ];
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

/// Summary statistics of one column over seeds, nulls left out: n, mean, sample standard
/// deviation, minimum, 10th percentile, median, 90th percentile, maximum (percentiles by linear
/// interpolation between order statistics).
pub fn summary(values: &[f64], nulls: usize) -> String {
    let mut o = Obj::new();
    o.raw("n", &values.len().to_string());
    o.raw("nulls", &nulls.to_string());
    if values.is_empty() {
        return o.end();
    }
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let sd = if values.len() > 1 { (values.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / (n - 1.0)).sqrt() } else { 0.0 };
    let mut s = values.to_vec();
    s.sort_by(f64::total_cmp);
    let q = |p: f64| {
        let h = p * (s.len() - 1) as f64;
        let lo = h.floor() as usize;
        let hi = (lo + 1).min(s.len() - 1);
        s[lo] + (h - lo as f64) * (s[hi] - s[lo])
    };
    let r = |x: f64| num((x * 1e6).round() / 1e6);
    o.raw("mean", &r(mean)).raw("sd", &r(sd)).raw("min", &r(s[0])).raw("p10", &r(q(0.1)));
    o.raw("median", &r(q(0.5))).raw("p90", &r(q(0.9))).raw("max", &r(s[s.len() - 1]));
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
