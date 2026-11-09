//! The core's checks: every demo recipe builds the same map every time and matches the
//! committed golden hashes; frames replay to the map; the parser refuses what it should, and
//! where; and the talk's conventions the stages rely on hold.

use map_lab_core::grid::*;
use map_lab_core::recipe::{self, parse};
use map_lab_core::{build, tiles_hash};
use std::fs;
use std::path::PathBuf;

fn lab() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn recipes() -> Vec<(String, String)> {
    let mut v: Vec<_> = fs::read_dir(lab().join("recipes"))
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

/// `golden-hashes.txt`: recipe, seed, tiles hash, frame count, for seeds 0..5, written by
/// `map-lab hashes recipes/*.json --seeds 0..5`. A change here changes published maps: it needs
/// a reason in the commit.
#[test]
fn maps_match_the_golden_hashes() {
    let golden = fs::read_to_string(lab().join("golden-hashes.txt")).unwrap();
    let mut n = 0;
    for line in golden.lines() {
        let f: Vec<&str> = line.split(' ').collect();
        let text = fs::read_to_string(lab().join("recipes").join(format!("{}.json", f[0]))).unwrap();
        let mut r = parse(&text).ok().unwrap();
        r.seed = f[1].parse().unwrap();
        let b = build(&r, true);
        assert_eq!(format!("{} {}", tiles_hash(&b.tiles), b.frames.len()), format!("{} {}", f[2], f[3]), "{line}");
        n += 1;
    }
    assert_eq!(n, 22 * 5);
}

#[test]
fn deterministic_and_frames_replay() {
    for (name, text) in recipes() {
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
