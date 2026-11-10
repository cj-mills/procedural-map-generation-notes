//! The core's checks: every demo recipe builds the same map every time and matches the
//! committed golden hashes; frames replay to the map; the parser refuses what it should, and
//! where; the talk's conventions the stages rely on hold; the stage table says what the parser
//! reads; the level's caches keep to their rule; and the walk keeps to its rules.

use map_lab_core::analyze::{distances, shortest_path};
use map_lab_core::grid::*;
use map_lab_core::recipe::{self, parse, Param};
use map_lab_core::walk::{Walk, IN_VIEW, REMEMBERED, SIGHT, UNSEEN};
use map_lab_core::{build, level, measure, tiles_hash, traits, Build};
use std::fs;
use std::path::PathBuf;

fn lab() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// The talk's 22 demos.
fn recipes() -> Vec<(String, String)> {
    recipes_in("recipes")
}

/// Our own recipes, which aren't demos of the talk: settings the talk describes without code.
fn own_recipes() -> Vec<(String, String)> {
    recipes_in("recipes/own")
}

fn recipes_in(dir: &str) -> Vec<(String, String)> {
    let mut v: Vec<_> = fs::read_dir(lab().join(dir))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .map(|p| (p.file_stem().unwrap().to_string_lossy().into_owned(), fs::read_to_string(&p).unwrap()))
        .collect();
    v.sort();
    v
}

#[test]
fn twenty_two_demo_recipes() {
    let names: Vec<_> = recipes().into_iter().map(|(n, _)| n).collect();
    assert_eq!(names.len(), 22, "{names:?}");
    assert!(names.contains(&"c08a-voronoi".to_string()));
}

/// `golden-hashes.txt`: recipe, seed, tiles hash, frame count and the level's caches (cell
/// indices, or `-` for none), for seeds 0..5, written by
/// `map-lab hashes recipes/*.json recipes/own/*.json --seeds 0..5`. A change here changes
/// published maps or levels: it needs a reason in the commit.
#[test]
fn maps_and_levels_match_the_golden_hashes() {
    let golden = fs::read_to_string(lab().join("golden-hashes.txt")).unwrap();
    let all: Vec<_> = recipes().into_iter().chain(own_recipes()).collect();
    let mut n = 0;
    for line in golden.lines() {
        let f: Vec<&str> = line.split(' ').collect();
        let text = &all.iter().find(|(name, _)| name == f[0]).expect(line).1;
        let mut r = parse(text).ok().unwrap();
        r.seed = f[1].parse().unwrap();
        let b = build(&r, true);
        let caches: Vec<String> = level::caches(&b).iter().map(|i| i.to_string()).collect();
        let caches = if caches.is_empty() { "-".to_string() } else { caches.join(",") };
        assert_eq!(format!("{} {} {}", tiles_hash(&b.tiles), b.frames.len(), caches), format!("{} {} {}", f[2], f[3], f[4]), "{line}");
        n += 1;
    }
    assert_eq!(n, all.len() * 5);
    assert_eq!(all.len(), 24);
}

#[test]
fn deterministic_and_frames_replay() {
    for (name, text) in recipes().into_iter().chain(own_recipes()) {
        for seed in [0, 7, 1 << 40] {
            let mut r = parse(&text).ok().unwrap();
            r.seed = seed;
            let a = build(&r, true);
            let b = build(&r, false);
            assert_eq!(a.tiles, b.tiles, "{name} seed {seed}: recording changed the map");
            let mut t = vec![u8::MAX; a.tiles.len()];
            for f in &a.frames {
                for &c in &f.changes {
                    t[(c >> 8) as usize] = c as u8;
                }
            }
            assert_eq!(t, a.tiles, "{name} seed {seed}: frames don't replay to the map");
            assert!(!a.frames.is_empty());
        }
    }
}

fn err(src: &str) -> String {
    match parse(src) {
        Ok(_) => panic!("parsed: {src}"),
        Err(e) => e.message(),
    }
}

fn with(stages: &str) -> String {
    format!(r#"{{"format": "map-lab-recipe/1", "seed": 3, "pipeline": [{stages}]}}"#)
}

#[test]
fn parser_refuses_with_where() {
    assert_eq!(err(r#"{"seed": 1, "pipeline": [{"stage": "cellular"}]}"#), "format: format is required: \"map-lab-recipe/1\"");
    assert_eq!(err(&with(r#"{"stage": "wobble"}"#)), "pipeline[0].stage: unknown stage");
    assert_eq!(err(&with(r#"{"stage": "rooms", "atempts": 3}"#)), "pipeline[0].atempts: unknown parameter");
    assert_eq!(err(&with(r#"{"stage": "rooms", "attempts": 0}"#)), "pipeline[0].attempts: out of range");
    assert_eq!(err(&with(r#"{"stage": "rooms", "attempts": 2.5}"#)), "pipeline[0].attempts: expected a whole number");
    assert_eq!(err(&with(r#"{"stage": "cellular", "variant": "better"}"#)), "pipeline[0].variant: not one of the allowed values");
    assert_eq!(err(&with(r#"{"stage": "cellular"}, {"stage": "exit"}"#)), "pipeline[1]: exit needs a start stage before it");
    assert_eq!(
        err(&with(r#"{"stage": "rooms"}, {"stage": "start"}, {"stage": "cellular"}, {"stage": "start"}, {"stage": "exit"}, {"stage": "hot-rooms"}"#)),
        "pipeline[5]: hot-rooms needs a rooms or bsp stage and an exit stage before it"
    );
    assert_eq!(
        err(&with(r#"{"stage": "combine", "inputs": [[{"stage": "cellular"}], [{"stage": "exit"}]]}"#)),
        "pipeline[0].inputs[1][0]: exit needs a start stage before it"
    );
    assert_eq!(err(&with(r#"{"stage": "prefab", "shape": "spine", "place": "room"}"#)).split(':').next(), Some("pipeline[0].place"));
    assert_eq!(err("{\"seed\": 1,\n  \"pipeline\": [}"), "line 2, column 16: expected a value");
    assert_eq!(err(r#"{"seed": 1, "seed": 2}"#), "line 1, column 13: repeated key");
    assert_eq!(err(&with(r#"{"stage": "noise", "gain": 1e-3}"#)), "line 1, column 84: exponents are not used in recipes; write the decimal");
}

#[test]
fn decimals_are_the_talks_literals() {
    let r = parse(&with(r#"{"stage": "noise", "gain": 0.005, "lacunarity": 4.0, "frequency": 0.08}"#)).ok().unwrap();
    let s = &r.pipeline[0];
    assert_eq!((s.num(2), s.num(3), s.num(4)), (0.005f32, 4.0f32, 0.08f32));
    let r = parse(&with(r#"{"stage": "noise", "gain": -0.1}"#));
    assert!(r.is_err());
}

#[test]
fn defaults_are_the_talks_values() {
    let r = parse(&with(r#"{"stage": "cellular"}, {"stage": "rooms"}, {"stage": "hot-path", "radius": 10}"#));
    assert!(r.is_err(), "hot-path without an exit");
    let r = parse(&with(r#"{"stage": "cellular"}, {"stage": "rooms"}, {"stage": "bsp"}, {"stage": "drunkard"}"#)).ok().unwrap();
    let p = &r.pipeline;
    assert_eq!((p[0].int(0), p[0].int(1)), (55, 10));
    assert_eq!((p[1].int(0), p[1].int(1), p[1].int(2)), (20, 2, 9));
    assert_eq!(p[2].int(0), 240);
    assert_eq!((p[3].int(0), p[3].int(1)), (200, 3));
    assert_eq!((r.width, r.height), (80, 50));
}

#[test]
fn rectangles_include_both_corners() {
    let r = Rect::sized(2, 3, 4, 2);
    assert_eq!(r.cells().count(), 5 * 3);
    assert!(r.contains(2, 3) && !r.contains(6, 3) && !r.contains(2, 5));
    assert!(r.intersects(&Rect::sized(6, 5, 1, 1)), "touching corners intersect");
    assert_eq!((r.width(), r.height(), r.center()), (4, 2, (4, 4)));
}

#[test]
fn lines_include_both_ends() {
    let l = map_lab_core::generate::line((3, 9), (40, 25));
    assert_eq!((l[0], *l.last().unwrap()), ((3, 9), (40, 25)));
    for w in l.windows(2) {
        assert!((w[0].0 - w[1].0).abs() <= 1 && (w[0].1 - w[1].1).abs() <= 1);
    }
}

#[test]
fn the_zoom_runs_the_talks_scales() {
    let s = map_lab_core::noise::zoom_scales();
    assert_eq!(s[0], 1.0);
    assert!(*s.last().unwrap() > 0.1 && s.last().unwrap() - 0.01 <= 0.1);
    assert_eq!(s.len(), 91);
}

#[test]
fn nearest_open_tile_takes_the_first_on_a_tie() {
    let mut g = Grid::new(30, 30, WALL);
    g.set(9, 10, FLOOR);
    g.set(11, 10, FLOOR);
    assert_eq!(nearest_walkable(&g, 10, 10), Some(g.idx(9, 10)));
}

#[test]
fn every_stage_named_in_the_table_runs() {
    for s in recipe::STAGES {
        let one = match s.name {
            "exit" => r#"{"stage": "cellular"}, {"stage": "start"}, {"stage": "exit"}"#.to_string(),
            "hot-path" => r#"{"stage": "cellular"}, {"stage": "start"}, {"stage": "exit"}, {"stage": "hot-path"}"#.into(),
            "hot-rooms" => r#"{"stage": "rooms"}, {"stage": "start"}, {"stage": "exit"}, {"stage": "hot-rooms"}"#.into(),
            "prefab" | "dla-erode" => format!(r#"{{"stage": "rooms"}}, {{"stage": "{}"}}"#, s.name),
            "combine" => r#"{"stage": "combine", "inputs": [[{"stage": "rooms"}], [{"stage": "drunkard"}]]}"#.into(),
            n => format!(r#"{{"stage": "{n}"}}"#),
        };
        let r = parse(&with(&one)).map_err(|e| e.message()).unwrap();
        let b = build(&r, true);
        assert_eq!(b.tiles.len(), 80 * 50, "{}", s.name);
    }
}

#[test]
fn voronoi_edges_are_walls_unless_roads() {
    let map = |stage: &str, seed: u64| {
        let mut r = parse(&with(stage)).map_err(|e| e.message()).unwrap();
        r.seed = seed;
        build(&r, false)
    };
    for seed in 0..20 {
        let default = map(r#"{"stage": "voronoi"}"#, seed);
        let walls = map(r#"{"stage": "voronoi", "edges": "walls"}"#, seed);
        let roads = map(r#"{"stage": "voronoi", "edges": "roads"}"#, seed);
        assert_eq!(default.tiles, walls.tiles, "seed {seed}: walls is the default");
        let g = roads.grid();
        for i in 0..g.len() {
            let (x, y) = g.xy(i);
            if x == 0 || y == 0 || x == g.w - 1 || y == g.h - 1 {
                assert_eq!(g.t[i], WALL, "seed {seed}: the map's edge stays wall");
            } else if walls.tiles[i] == WALL {
                assert_eq!(g.t[i], FLOOR, "seed {seed}: every wall of the talk's rule is road");
            }
        }
        // The talk's edges as floor would be dozens of regions; the road rule joins them.
        assert!(roads.traits().regions <= 3, "seed {seed}: {} regions of road", roads.traits().regions);
        assert!(walls.traits().regions >= 10, "seed {seed}: the walls seal the cells");
    }
    assert_eq!(err(&with(r#"{"stage": "voronoi", "edges": "paths"}"#)), "pipeline[0].edges: not one of the allowed values");
}

#[test]
fn the_stage_table_says_what_the_parser_reads() {
    let table = recipe::stage_table();
    assert!(map_lab_core::json::parse(&table).is_ok(), "{table}");
    assert!(table.starts_with(r#"{"format":"map-lab-stages/1","recipe":{"format":"map-lab-recipe/1","maxSeed":9007199254740991,"minSide":24,"maxSide":256,"width":80,"height":50},"stages":[{"name":"rooms","generator":true,"variants":["talk"],"params":[{"name":"attempts","kind":"int","min":1,"max":500,"default":20},"#), "{table}");
    assert!(table.contains(r#"{"name":"gain","kind":"num","min":0,"max":4,"default":0.1}"#), "{table}");
    assert!(table.contains(r#"{"name":"edges","kind":"pick","options":["walls","roads"],"default":"walls"}"#), "{table}");
    assert!(table.contains(r#"{"name":"combine","generator":true,"variants":["talk"],"params":[{"name":"join","kind":"pick","options":["halves"],"default":"halves"},{"name":"inputs","kind":"inputs","count":2}]}"#), "{table}");
    assert!(table.ends_with(r#"{"name":"hot-rooms","generator":false,"variants":["talk"],"params":[]}]}"#), "{table}");
    for s in recipe::STAGES {
        assert_eq!(table.matches(&format!(r#"{{"name":"{}","generator":"#, s.name)).count(), 1, "{}", s.name);
        // Every decimal in the table reads back as the same `f32`.
        for (i, p) in s.params.iter().enumerate() {
            let Param::Num { min, max, default } = p.kind else { continue };
            for v in [min, max, default] {
                let mut text = String::new();
                recipe::push_decimal(&mut text, v);
                let r = parse(&with(&format!(r#"{{"stage": "{}", "{}": {text}}}"#, s.name, p.name))).map_err(|e| e.message()).unwrap();
                assert_eq!(r.pipeline[0].num(i), v, "{}.{} {text}", s.name, p.name);
            }
        }
    }
    let text = |x: f32| {
        let mut s = String::new();
        recipe::push_decimal(&mut s, x);
        s
    };
    assert_eq!([text(0.1), text(0.005), text(5.0), text(0.08), text(2.5), text(0.000001), text(16.0)], ["0.1", "0.005", "5", "0.08", "2.5", "0.000001", "16"]);
}

#[test]
fn caches_keep_to_their_rule() {
    let mut levels = 0;
    for (name, text) in recipes() {
        for seed in 0..10 {
            let mut r = parse(&text).ok().unwrap();
            r.seed = seed;
            let b = build(&r, false);
            let caches = level::caches(&b);
            assert_eq!(caches, level::caches(&build(&r, true)), "{name} seed {seed}: the same level every time");
            if b.path.is_empty() {
                assert!(caches.is_empty(), "{name} seed {seed}: no caches without a path");
                continue;
            }
            levels += 1;
            assert!(caches.len() <= level::CACHES);
            let g = b.grid();
            let hot = distances(&g, &b.path);
            for (n, &c) in caches.iter().enumerate() {
                assert!(matches!(g.t[c], FLOOR | GRASS), "{name} seed {seed}: a cache on tile {}", g.t[c]);
                assert!(hot[c] >= b.hot_radius, "{name} seed {seed}: a cache {} steps from the path", hot[c]);
                let from = distances(&g, &[c]);
                for &other in &caches[..n] {
                    assert!(from[other] > level::CACHE_SPACING, "{name} seed {seed}: caches {} steps apart", from[other]);
                }
            }
            if let Some(&first) = caches.first() {
                assert_eq!(hot[first], *hot.iter().zip(&g.t).filter(|(_, t)| matches!(**t, FLOOR | GRASS)).map(|(h, _)| h).max().unwrap());
            }
        }
    }
    assert!(levels >= 40, "{levels} levels checked");
    // The hot path's radius is the recipe's: nothing is off a hot path as wide as the map.
    let stages = r#"{"stage": "cellular"}, {"stage": "cull"}, {"stage": "start"}, {"stage": "exit"}, {"stage": "hot-path", "radius": 1000}"#;
    let b = build(&parse(&with(stages)).ok().unwrap(), false);
    assert!(!b.path.is_empty() && level::caches(&b).is_empty());
}

#[test]
fn summaries_are_the_data_files() {
    let s = traits::summary(&[4.0, 1.0, 3.0, 2.0]).unwrap();
    let want = [4.0, 2.5, 1.2909944487358056, 1.0, 1.3, 2.5, 3.7, 4.0];
    for (a, b) in s.iter().zip(want) {
        assert!((a - b).abs() < 1e-12, "{s:?}");
    }
    assert_eq!(traits::summary(&[7.0]).unwrap(), [1.0, 7.0, 0.0, 7.0, 7.0, 7.0, 7.0, 7.0]);
    assert!(traits::summary(&[]).is_none());

    let mut r = parse(&with(r#"{"stage": "rooms"}"#)).ok().unwrap();
    let rows = measure(&mut r, 0, 30);
    assert_eq!(rows.len(), 30 * traits::ROW);
    let sums = traits::summarize(&rows);
    assert_eq!(sums.len(), traits::VALUES * traits::SUMMARY + traits::FAILURES.len());
    // Rooms maps have no noise field: no seed has a field mean (value 9).
    assert_eq!(sums[9 * traits::SUMMARY], 0.0);
    assert!(sums[9 * traits::SUMMARY + 1].is_nan());
    // Every seed has a floor share (value 0), and its row is the build's own traits.
    assert_eq!(sums[0], 30.0);
    r.seed = 4;
    let b = build(&r, false);
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&rows[4 * traits::ROW..5 * traits::ROW - 1]), bits(&b.traits().values()));
    assert_eq!(rows[5 * traits::ROW - 1], b.failures() as f64);
    let failing = |bit: usize| rows.chunks(traits::ROW).filter(|row| row[traits::VALUES] as u32 & (1 << bit) != 0).count() as f64;
    for bit in 0..traits::FAILURES.len() {
        assert_eq!(sums[traits::VALUES * traits::SUMMARY + bit], failing(bit));
    }
}

/// The moves that take a walker along `path`, tile by tile.
fn moves_along(g: &Grid, path: &[usize]) -> Vec<u8> {
    path.windows(2)
        .map(|w| {
            let ((ax, ay), (bx, by)) = (g.xy(w[0]), g.xy(w[1]));
            [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().position(|&d| d == (bx - ax, by - ay)).unwrap() as u8
        })
        .collect()
}

#[test]
fn the_walk_keeps_to_its_rules() {
    let mut walked = 0;
    for (name, text) in recipes() {
        for seed in 0..5 {
            let mut r = parse(&text).ok().unwrap();
            r.seed = seed;
            let b = build(&r, false);
            let Some(mut w) = Walk::new(&b) else {
                assert!(b.path.is_empty(), "{name} seed {seed}: a level with no walk");
                continue;
            };
            walked += 1;
            let g = b.grid();
            assert_eq!((w.at(), w.route.len(), w.done()), (b.start.unwrap(), 1, false));
            assert_eq!(w.caches, level::caches(&b));

            // A step into a wall, or off the map, is refused and changes nothing.
            let (x, y) = g.xy(w.at());
            for (dir, (dx, dy)) in [(-1, 0), (1, 0), (0, -1), (0, 1)].into_iter().enumerate() {
                if !walkable(g.get(x + dx, y + dy)) {
                    let before = w.seen.clone();
                    assert!(!w.step(dir as u8), "{name} seed {seed}: walked into a wall");
                    assert_eq!((w.route.len(), &w.seen), (1, &before));
                }
            }

            // To the first cache and back, then along the path: the cache is found, the exit
            // ends the walk, and the steps are the route's.
            let mut steps = 0;
            if let Some(&cache) = w.caches.first() {
                let there = shortest_path(&g, w.at(), cache);
                let back: Vec<usize> = there.iter().rev().copied().collect();
                for m in moves_along(&g, &there).into_iter().chain(moves_along(&g, &back)) {
                    // The exit may be on the way: the walk ends there.
                    steps += w.step(m) as usize;
                }
                assert!(w.found[0] || w.done(), "{name} seed {seed}: stood on a cache without finding it");
            }
            if !w.done() {
                for m in moves_along(&g, &b.path) {
                    assert!(w.step(m), "{name} seed {seed}: a step along the path was refused");
                    steps += 1;
                }
            }
            assert!(w.done() && w.at() == b.exit.unwrap(), "{name} seed {seed}");
            assert_eq!(w.route.len() - 1, steps);
            assert!(!w.step(0) && !w.step(1), "{name} seed {seed}: a step after the exit");

            // In view: within sight of the walker. Seen tiles stay seen.
            let (px, py) = g.xy(w.at());
            for (i, &s) in w.seen.iter().enumerate() {
                let (x, y) = g.xy(i);
                if s == IN_VIEW && walkable(g.t[i]) {
                    assert!((x - px).pow(2) + (y - py).pow(2) <= SIGHT * SIGHT);
                } else if s == IN_VIEW {
                    assert!((x - px).abs() <= SIGHT + 1 && (y - py).abs() <= SIGHT + 1);
                }
            }
            assert!(w.route.iter().all(|&i| w.seen[i] != UNSEEN));
            assert_eq!(w.seen[w.at()], IN_VIEW);
        }
    }
    assert!(walked >= 20, "{walked} levels walked");
}

#[test]
fn the_walker_sees_nine_tiles_and_not_through_walls() {
    // A corridor along row 12 from x = 2 to x = 21, with a side room below its left end that a
    // wall hides from the right.
    let (w, h) = (24, 24);
    let mut g = Grid::new(w, h, WALL);
    for x in 2..=21 {
        g.set(x, 12, FLOOR);
    }
    for (x, y) in [(2, 13), (2, 14), (3, 14), (4, 14)] {
        g.set(x, y, FLOOR);
    }
    let (start, exit) = (g.idx(2, 12), g.idx(21, 12));
    let path = shortest_path(&g, start, exit);
    let b = Build {
        width: w,
        height: h,
        tiles: g.t.clone(),
        frames: Vec::new(),
        stage_names: Vec::new(),
        rooms: None,
        important: Vec::new(),
        start: Some(start),
        exit: Some(exit),
        path,
        dist: Vec::new(),
        hot: Vec::new(),
        hot_radius: 10,
        field: Vec::new(),
        stage_failures: 0,
    };
    let mut walk = Walk::new(&b).unwrap();
    let seen = |walk: &Walk, x: i32, y: i32| walk.seen[g.idx(x, y)];
    // From (2, 12): nine tiles along the corridor and no farther; the walls beside it, which
    // the line of sight alone shows only in patches; the side room straight below; but not the
    // room's far end, around the corner.
    assert_eq!((seen(&walk, 11, 12), seen(&walk, 12, 12)), (IN_VIEW, UNSEEN));
    assert_eq!((seen(&walk, 5, 11), seen(&walk, 5, 13)), (IN_VIEW, IN_VIEW));
    assert_eq!((seen(&walk, 2, 14), seen(&walk, 4, 14)), (IN_VIEW, UNSEEN));
    assert_eq!(seen(&walk, 5, 10), UNSEEN, "a tile behind the corridor's wall");
    // Twelve steps right: the start is out of sight and remembered, the far end is in view.
    for _ in 0..12 {
        assert!(walk.step(1));
    }
    assert_eq!((seen(&walk, 2, 12), seen(&walk, 21, 12), seen(&walk, 2, 14)), (REMEMBERED, IN_VIEW, REMEMBERED));
    assert_eq!(seen(&walk, 4, 14), UNSEEN, "never in view");
    assert!(!walk.step(2) && !walk.step(3), "walls above and below the corridor");
    assert_eq!(walk.route.len(), 13);
}

#[test]
fn no_walk_without_a_level() {
    let b = build(&parse(&with(r#"{"stage": "cellular"}"#)).ok().unwrap(), false);
    assert!(Walk::new(&b).is_none(), "no start and no exit");
    let b = build(&parse(&with(r#"{"stage": "cellular"}, {"stage": "start"}"#)).ok().unwrap(), false);
    assert!(Walk::new(&b).is_none(), "no exit");
}
