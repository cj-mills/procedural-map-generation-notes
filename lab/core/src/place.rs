//! Shaping and combining: two maps joined, and prefabs stamped into a map.

use crate::grid::*;
use crate::record::*;
use crate::recipe::Stage;
use crate::{run_sub, Ctx, GUARD_HIT, PREFAB_NO_PLACE};

/// Run each input pipeline on a fresh map, then take the left half from the first and the right
/// half from the second (`c09-combination/src/main.rs:78-114`).
pub fn combine(c: &mut Ctx, s: &Stage) {
    let inputs = s.inputs(1);
    let mut maps = Vec::new();
    for (n, p) in inputs.iter().enumerate() {
        c.rec.frame(&c.g, Note::CombineInput, n as i32, Vec::new());
        let (g, failures) = run_sub(c, p);
        c.failures |= failures;
        maps.push(g);
    }
    let half = c.g.w / 2;
    for i in 0..c.g.len() {
        let (x, _) = c.g.xy(i);
        c.g.t[i] = if x < half { maps[0].t[i] } else { maps[1].t[i] };
    }
    c.rooms.clear();
    c.made_rooms = false;
    c.rec.frame(&c.g, Note::CombineHalves, 0, Vec::new());
}

/// The vault: treasure ringed by traps inside a one-tile margin left as it was, 6 by 5 as in the
/// talk's "definitely not a trap" (`c14-not-a-trap/src/main.rs:149-155`). Generated, not copied.
const VAULT_W: i32 = 6;
const VAULT_H: i32 = 5;
fn vault(x: i32, y: i32) -> Option<u8> {
    if x == 0 || y == 0 || x == VAULT_W - 1 || y == VAULT_H - 1 {
        None
    } else if x == 1 || y == 1 || x == VAULT_W - 2 || y == VAULT_H - 2 {
        Some(TRAP)
    } else {
        Some(TREASURE)
    }
}

/// The spine: our own 11-wide prefab standing in for the talk's (`c09-combination/src/main.rs:116-133`,
/// whose layout we don't copy), stamped down the middle where two maps meet. A hall between two
/// walls with a doorway every twelve rows, pillars down its middle, open margins on both sides.
const SPINE_W: i32 = 11;
fn spine(x: i32, y: i32, h: i32) -> u8 {
    if y == 0 || y == h - 1 {
        WALL
    } else if x == 0 || x == SPINE_W - 1 {
        FLOOR
    } else if x == 1 || x == SPINE_W - 2 {
        if (5..=7).contains(&(y % 12)) { FLOOR } else { WALL }
    } else if (4..=6).contains(&x) && y % 12 == 0 {
        WALL
    } else {
        FLOOR
    }
}

pub fn prefab(c: &mut Ctx, s: &Stage) {
    let (shape, place) = (s.pick(0), s.pick(1));
    let (w, h) = (c.g.w, c.g.h);
    if shape == 1 {
        let x0 = w / 2 - 4;
        for y in 0..h {
            for x in 0..SPINE_W {
                c.g.set(x0 + x, y, spine(x, y, h));
            }
        }
        let marks = marks_of(&c.g, (0..h).flat_map(|y| (0..SPINE_W).map(move |x| (x0 + x, y))), NEW);
        c.rec.frame(&c.g, Note::PrefabPlaced, 0, marks);
        return;
    }
    let vault_cells = |bx: i32, by: i32| {
        (0..VAULT_H).flat_map(move |y| (0..VAULT_W).map(move |x| (bx + x, by + y))).filter(move |&(x, y)| vault(x - bx, y - by).is_some())
    };
    if c.rec.on {
        // The prefab on its own first, as the talk shows it (`c14-not-a-trap/src/main.rs:81-102`).
        let (bx, by) = (w / 2 - VAULT_W / 2, h / 2 - VAULT_H / 2);
        c.rec.frame(&c.g, Note::PrefabShow, 0, marks_of(&c.g, vault_cells(bx, by), POINT));
    }
    let base = match place {
        // In a random room that has space for it, at the room's centre (`c14-not-a-trap/src/main.rs:104-124`).
        // The talk's loop runs forever when no room is big enough; here that is a failure.
        0 => {
            let big = |r: &Rect| r.width() > 5 && r.height() > 5;
            if !c.rooms.iter().any(big) {
                c.failures |= GUARD_HIT;
                None
            } else {
                loop {
                    let r = c.rooms[c.rng.pick(c.rooms.len()).unwrap()];
                    if big(&r) {
                        let (cx, cy) = r.center();
                        break Some((cx - 3, cy - 2));
                    }
                }
            }
        }
        // Anywhere it fits on open floor, by trying random spots (`c15-still-not-a-trap/src/main.rs:41-67`).
        1 => {
            let mut found = None;
            for _ in 0..PLACE_CAP {
                let (bx, by) = (c.rng.range(1, w - 10), c.rng.range(1, h - 10));
                if Rect::sized(bx, by, VAULT_W, VAULT_H).cells().all(|(x, y)| c.g.get(x, y) == FLOOR) {
                    found = Some((bx, by));
                    break;
                }
            }
            if found.is_none() {
                c.failures |= GUARD_HIT;
            }
            found
        }
        _ => Some((w / 2 - VAULT_W / 2, h / 2 - VAULT_H / 2)),
    };
    let Some((bx, by)) = base else {
        c.failures |= PREFAB_NO_PLACE;
        c.rec.frame(&c.g, Note::PrefabNoPlace, 0, Vec::new());
        return;
    };
    for (x, y) in vault_cells(bx, by) {
        c.g.set(x, y, vault(x - bx, y - by).unwrap());
    }
    c.rec.frame(&c.g, Note::PrefabPlaced, 0, marks_of(&c.g, vault_cells(bx, by), NEW));
}

/// Random spots tried for a prefab before the seed counts as failed. The talk's loop has no cap.
const PLACE_CAP: u32 = 100_000;
