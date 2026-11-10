//! `map-lab`: the map lab's core on the command line, for agents and for the build.
//!
//!   map-lab build <recipe.json> [--seed N] [--json] [--path] [--frames]
//!   map-lab walk <recipe.json> [--seed N] [--moves LRUD...] [--json]
//!   map-lab traits <recipe.json> [--seeds FROM..TO]
//!   map-lab measure <recipe.json>... [--seeds FROM..TO] [--out FILE]
//!   map-lab compare <ours.json> <theirs.json> [--out FILE]
//!   map-lab hashes <recipe.json>... [--seeds FROM..TO]
//!   map-lab schema
//!   map-lab stages
//!
//! A recipe file is JSON (`map-lab schema` prints its schema; `map-lab stages` prints the same
//! stages and parameters as the table the page's controls read). Output is plain text or JSON
//! on standard output; errors go to standard error with a non-zero exit.

mod compare;
mod docs;
mod schema;

use map_lab_core::grid::{GLYPHS, TILE_KINDS, TILE_NAMES};
use map_lab_core::recipe::{self, Recipe};
use map_lab_core::record::{MARK_NAMES, NOTE_KEYS};
use map_lab_core::report::{self, failure_names, list, string, Obj, Row, COLUMNS};
use map_lab_core::walk::{Walk, IN_VIEW, MOVES, UNSEEN};
use map_lab_core::{build, level, tiles_hash, Build};
use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else { usage() };
    let rest = &args[1..];
    let r = match cmd.as_str() {
        "build" => cmd_build(rest),
        "walk" => cmd_walk(rest),
        "traits" => cmd_traits(rest),
        "measure" => cmd_measure(rest),
        "compare" => compare::cmd(rest),
        "hashes" => cmd_hashes(rest),
        "schema" => {
            println!("{}", schema::schema());
            Ok(())
        }
        "stages" => {
            println!("{}", recipe::stage_table());
            Ok(())
        }
        "-h" | "--help" | "help" => usage(),
        _ => Err(format!("unknown command {cmd:?}")),
    };
    if let Err(e) = r {
        eprintln!("map-lab: {e}");
        exit(1);
    }
}

fn usage() -> ! {
    eprintln!(
        "usage:\n  map-lab build <recipe.json> [--seed N] [--json] [--path] [--frames]\n  map-lab walk <recipe.json> [--seed N] [--moves LRUD...] [--json]\n  map-lab traits <recipe.json> [--seeds FROM..TO]\n  map-lab measure <recipe.json>... [--seeds FROM..TO] [--out FILE]\n  map-lab compare <ours.json> <theirs.json> [--out FILE]\n  map-lab hashes <recipe.json>... [--seeds FROM..TO]\n  map-lab schema\n  map-lab stages"
    );
    exit(2)
}

/// Positional arguments, and the value of each `--flag value` / presence of each `--flag`.
struct Args {
    files: Vec<String>,
    flags: Vec<(String, Option<String>)>,
}

fn parse_args(a: &[String], valued: &[&str]) -> Result<Args, String> {
    let mut files = Vec::new();
    let mut flags = Vec::new();
    let mut i = 0;
    while i < a.len() {
        let s = &a[i];
        if let Some(f) = s.strip_prefix("--") {
            if valued.contains(&f) {
                let v = a.get(i + 1).ok_or(format!("--{f} needs a value"))?;
                flags.push((f.to_string(), Some(v.clone())));
                i += 1;
            } else {
                flags.push((f.to_string(), None));
            }
        } else {
            files.push(s.clone());
        }
        i += 1;
    }
    Ok(Args { files, flags })
}

impl Args {
    fn has(&self, f: &str) -> bool {
        self.flags.iter().any(|(k, _)| k == f)
    }
    fn get(&self, f: &str) -> Option<&str> {
        self.flags.iter().find(|(k, _)| k == f).and_then(|(_, v)| v.as_deref())
    }
    fn seeds(&self, default: (u64, u64)) -> Result<(u64, u64), String> {
        let Some(s) = self.get("seeds") else { return Ok(default) };
        let (a, b) = s.split_once("..").ok_or("--seeds is FROM..TO, TO exclusive")?;
        let a: u64 = a.parse().map_err(|_| "bad --seeds")?;
        let b: u64 = b.parse().map_err(|_| "bad --seeds")?;
        if b <= a {
            return Err("--seeds: TO must be above FROM".into());
        }
        Ok((a, b))
    }
}

pub fn load(path: &str) -> Result<(String, Recipe), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let r = recipe::parse(&text).map_err(|e| format!("{path}: {}", e.message()))?;
    Ok((text, r))
}

/// The id of a recipe file: its name without `.json`.
fn id_of(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.strip_suffix(".json").unwrap_or(name).to_string()
}

fn cmd_build(a: &[String]) -> Result<(), String> {
    let a = parse_args(a, &["seed"])?;
    let [file] = a.files.as_slice() else { return Err("build takes one recipe file".into()) };
    let (_, mut r) = load(file)?;
    if let Some(s) = a.get("seed") {
        r.seed = s.parse().map_err(|_| "bad --seed")?;
    }
    let b = build(&r, a.has("frames"));
    let t = b.traits();
    let failures = b.failures();
    let row = report::row(r.seed, &t, failures);
    let rows = render(&b, a.has("path"));
    if a.has("json") {
        let mut o = Obj::new();
        o.raw("seed", &r.seed.to_string());
        o.raw("width", &b.width.to_string());
        o.raw("height", &b.height.to_string());
        o.raw("legend", &legend());
        o.raw("tiles", &list(rows.iter().map(|r| string(r))));
        o.raw("traits", &traits_obj(&row));
        o.raw("failures", &list(failure_names(failures).into_iter().map(string)));
        o.raw("start", &point(&b, b.start));
        o.raw("exit", &point(&b, b.exit));
        o.raw("caches", &list(level::caches(&b).into_iter().map(|i| point(&b, Some(i)))));
        o.raw("stages", &list(b.stage_names.iter().map(|s| string(s))));
        o.raw("tilesHash", &tiles_hash(&b.tiles).to_string());
        if a.has("frames") {
            o.raw("frames", &frames_json(&b));
        }
        println!("{}", o.end());
    } else {
        for row in &rows {
            println!("{row}");
        }
        println!();
        println!("seed {}  {}x{}  stages: {}", r.seed, b.width, b.height, b.stage_names.join(" > "));
        println!(
            "floor share {}  regions {}  dead ends {}  corridor ratio {}  path {}{}",
            fmt(t.floor_share),
            t.regions,
            t.dead_ends,
            fmt(t.corridor_ratio),
            t.path.map_or("none".into(), |p| p.to_string()),
            if t.probed { " (probe: left edge to right edge)" } else { "" }
        );
        let names = failure_names(failures);
        println!("failures: {}", if names.is_empty() { "none".to_string() } else { names.join(", ") });
        if a.has("frames") {
            for f in &b.frames {
                println!(
                    "frame stage {} {} {} {}  changes {} marks {}",
                    f.stage,
                    b.stage_names[f.stage as usize],
                    NOTE_KEYS[f.note as usize],
                    f.arg,
                    f.changes.len(),
                    f.marks.len()
                );
            }
        }
    }
    Ok(())
}

/// Walk a recipe's level by a list of moves and print what the walker knows: the map as far as
/// they have seen it, then the walk's numbers. A move the level refuses is counted, not taken.
fn cmd_walk(a: &[String]) -> Result<(), String> {
    let a = parse_args(a, &["seed", "moves"])?;
    let [file] = a.files.as_slice() else { return Err("walk takes one recipe file".into()) };
    let (_, mut r) = load(file)?;
    if let Some(s) = a.get("seed") {
        r.seed = s.parse().map_err(|_| "bad --seed")?;
    }
    let b = build(&r, false);
    let mut w = Walk::new(&b).ok_or("this map is not a level: a walk needs a start, an exit and a path between them")?;
    let mut refused = 0;
    for m in a.get("moves").unwrap_or("").bytes() {
        let dir = MOVES.iter().position(|&l| l == m.to_ascii_uppercase()).ok_or("--moves is letters from L, R, U and D")?;
        if !w.step(dir as u8) {
            refused += 1;
        }
    }
    // What the walker knows: unseen tiles blank, the exit and unfound caches only once seen.
    let mut chars: Vec<u8> = b.tiles.iter().zip(&w.seen).map(|(&t, &s)| if s == UNSEEN { b' ' } else { GLYPHS[t as usize] }).collect();
    for (&c, &found) in w.caches.iter().zip(&w.found) {
        if !found && w.seen[c] != UNSEEN {
            chars[c] = b'&';
        }
    }
    if let Some(e) = b.exit.filter(|&e| w.seen[e] != UNSEEN) {
        chars[e] = b'>';
    }
    chars[w.at()] = b'@';
    let rows: Vec<String> = chars.chunks(b.width as usize).map(|r| String::from_utf8(r.to_vec()).unwrap()).collect();
    let steps = w.route.len() - 1;
    let shortest = b.path.len() - 1;
    let found = w.found.iter().filter(|&&f| f).count();
    if a.has("json") {
        let mut o = Obj::new();
        o.raw("seed", &r.seed.to_string());
        o.raw("steps", &steps.to_string());
        o.raw("refused", &refused.to_string());
        o.raw("shortest", &shortest.to_string());
        o.raw("done", if w.done() { "true" } else { "false" });
        o.raw("at", &point(&b, Some(w.at())));
        o.raw("route", &list(w.route.iter().map(|&i| point(&b, Some(i)))));
        o.raw("caches", &list(w.caches.iter().map(|&i| point(&b, Some(i)))));
        o.raw("found", &list(w.found.iter().map(|&f| if f { "true".to_string() } else { "false".to_string() })));
        o.raw("inView", &w.seen.iter().filter(|&&s| s == IN_VIEW).count().to_string());
        o.raw("seenAbout", &string("one digit a tile, row by row: 0 unseen, 1 seen earlier and remembered, 2 in view now"));
        o.raw("seen", &list(w.seen.chunks(b.width as usize).map(|row| string(&row.iter().map(|s| (b'0' + s) as char).collect::<String>()))));
        o.raw("map", &list(rows.iter().map(|r| string(r))));
        println!("{}", o.end());
    } else {
        for row in &rows {
            println!("{row}");
        }
        println!();
        println!(
            "seed {}  steps {}  refused moves {}  the shortest walk {}  caches found {} of {}  {}",
            r.seed,
            steps,
            refused,
            shortest,
            found,
            w.caches.len(),
            if w.done() { "at the exit" } else { "not at the exit" }
        );
    }
    Ok(())
}

/// The map as text rows; `@` the start, `>` the exit and `&` each cache, `*` the path with
/// `--path`.
fn render(b: &Build, with_path: bool) -> Vec<String> {
    let mut chars: Vec<u8> = b.tiles.iter().map(|&t| GLYPHS[t as usize]).collect();
    if with_path {
        for &i in &b.path {
            chars[i] = b'*';
        }
    }
    for i in level::caches(b) {
        chars[i] = b'&';
    }
    if let Some(s) = b.start {
        chars[s] = b'@';
    }
    if let Some(e) = b.exit {
        chars[e] = b'>';
    }
    chars.chunks(b.width as usize).map(|r| String::from_utf8(r.to_vec()).unwrap()).collect()
}

fn legend() -> String {
    let mut o = Obj::new();
    for k in 0..TILE_KINDS {
        o.raw(&(GLYPHS[k] as char).to_string(), &string(TILE_NAMES[k]));
    }
    o.raw("@", &string("start"));
    o.raw(">", &string("exit"));
    o.raw("&", &string("cache"));
    o.raw("*", &string("path (with --path)"));
    o.end()
}

fn point(b: &Build, i: Option<usize>) -> String {
    match i {
        Some(i) => format!("[{}, {}]", i as i32 % b.width, i as i32 / b.width),
        None => "null".into(),
    }
}

fn fmt(x: f64) -> String {
    format!("{x:.4}")
}

fn traits_obj(r: &Row) -> String {
    let mut o = Obj::new();
    for ((k, _), v) in COLUMNS.iter().zip(&r.values) {
        o.raw(k, &v.map_or("null".into(), report::num));
    }
    o.end()
}

fn frames_json(b: &Build) -> String {
    let mut marks = Obj::new();
    for (i, m) in MARK_NAMES.iter().enumerate().skip(1) {
        marks.raw(m, &i.to_string());
    }
    let frames = b.frames.iter().map(|f| {
        let mut o = Obj::new();
        o.raw("stage", &f.stage.to_string());
        o.raw("note", &string(NOTE_KEYS[f.note as usize]));
        o.raw("arg", &f.arg.to_string());
        o.raw("changes", &list(f.changes.iter().map(|c| format!("[{}, {}]", c >> 8, c & 0xff))));
        o.raw("marks", &list(f.marks.iter().map(|c| format!("[{}, {}]", c >> 8, c & 0xff))));
        if !f.field.is_empty() {
            o.raw("field", &list(f.field.iter().map(|v| v.to_string())));
        }
        o.end()
    });
    let mut o = Obj::new();
    o.raw("markValues", &marks.end());
    o.raw("about", &string("Each frame lists [cell index, tile] for tiles changed since the previous frame, and [cell index, mark] for cells highlighted on this frame only; a mark of 32 or more is group mark - 32 (a room, a Voronoi cell). Cell index = y * width + x."));
    o.raw("list", &list(frames));
    o.end()
}

fn run_seeds(r: &mut Recipe, (from, to): (u64, u64)) -> Vec<Row> {
    (from..to)
        .map(|seed| {
            r.seed = seed;
            let b = build(r, false);
            report::row(seed, &b.traits(), b.failures())
        })
        .collect()
}

fn cmd_traits(a: &[String]) -> Result<(), String> {
    let a = parse_args(a, &["seeds"])?;
    let [file] = a.files.as_slice() else { return Err("traits takes one recipe file".into()) };
    let (_, mut r) = load(file)?;
    let seeds = a.seeds((r.seed, r.seed + 1))?;
    for row in run_seeds(&mut r, seeds) {
        let mut o = Obj::new();
        o.raw("seed", &row.seed.to_string());
        for ((k, _), v) in COLUMNS.iter().zip(&row.values) {
            o.raw(k, &v.map_or("null".into(), report::num));
        }
        o.raw("failures", &list(failure_names(row.failures).into_iter().map(string)));
        println!("{}", o.end());
    }
    Ok(())
}

pub const DEFAULT_SEEDS: (u64, u64) = (0, 200);

fn cmd_measure(a: &[String]) -> Result<(), String> {
    let a = parse_args(a, &["seeds", "out"])?;
    if a.files.is_empty() {
        return Err("measure takes recipe files".into());
    }
    let seeds = a.seeds(DEFAULT_SEEDS)?;
    let mut demos = Vec::new();
    for file in &a.files {
        let (text, mut r) = load(file)?;
        let rows = run_seeds(&mut r, seeds);
        demos.push(report::demo(&id_of(file), Some((file, text.trim())), &[], &rows));
    }
    let doc = report::traits_doc(
        "map-lab measure, map-lab-core 0.1.0: each recipe built once per seed, the recipe's own seed replaced",
        seeds,
        &demos,
    );
    write_out(a.get("out"), &doc)
}

pub fn write_out(path: Option<&str>, doc: &str) -> Result<(), String> {
    match path {
        Some(p) => std::fs::write(p, format!("{doc}\n")).map_err(|e| format!("{p}: {e}")),
        None => {
            println!("{doc}");
            Ok(())
        }
    }
}

fn cmd_hashes(a: &[String]) -> Result<(), String> {
    let a = parse_args(a, &["seeds"])?;
    for file in &a.files {
        let (_, mut r) = load(file)?;
        let seeds = a.seeds((r.seed, r.seed + 1))?;
        for seed in seeds.0..seeds.1 {
            r.seed = seed;
            let b = build(&r, true);
            let caches: Vec<String> = level::caches(&b).iter().map(|i| i.to_string()).collect();
            let caches = if caches.is_empty() { "-".into() } else { caches.join(",") };
            println!("{} {} {} {} {}", id_of(file), seed, tiles_hash(&b.tiles), b.frames.len(), caches);
        }
    }
    Ok(())
}
