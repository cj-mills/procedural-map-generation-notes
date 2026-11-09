// The talk's code is read in place from a checkout of thebracket/roguelike-celebration-2020,
// never copied into this repo: WOLVERSON_REPO names it, and it must be at the commit the page
// cites.
use std::process::Command;

const COMMIT: &str = "604e5b5a";

fn main() {
    println!("cargo:rerun-if-env-changed=WOLVERSON_REPO");
    let repo = std::env::var("WOLVERSON_REPO").expect(
        "set WOLVERSON_REPO to a checkout of https://github.com/thebracket/roguelike-celebration-2020 at 604e5b5a",
    );
    let head = Command::new("git").args(["-C", &repo, "rev-parse", "HEAD"]).output().expect("git");
    let head = String::from_utf8_lossy(&head.stdout).trim().to_string();
    assert!(head.starts_with(COMMIT), "WOLVERSON_REPO is at {head}, not {COMMIT}");
    let dirty = Command::new("git").args(["-C", &repo, "status", "--porcelain", "--untracked-files=no"]).output().expect("git");
    assert!(dirty.stdout.is_empty(), "WOLVERSON_REPO has local changes");
    println!("cargo:rustc-env=WOLVERSON_COMMIT={head}");
}
