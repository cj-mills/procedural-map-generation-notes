//! `map-lab compare`: the faithfulness check's statistics. Two traits data files, ours and the
//! talk's own code run natively over many seeds; for each demo and trait, the two distributions
//! side by side and a two-sample Kolmogorov–Smirnov test. The random number generators differ,
//! so maps differ seed by seed; a faithful version gives the same distributions.

use crate::{parse_args, write_out};
use map_lab_core::json::{self, Json};
use map_lab_core::report::{num, string, Obj, COLUMNS};
use map_lab_core::traits::FAILURES;

/// Below this p-value the two distributions count as different.
pub const ALPHA: f64 = 0.01;

/// Failures from this bit on are read from a finished map's traits (`traits::FAILURES`).
const FIRST_TRAIT_FAILURE: usize = 4;

struct Demo {
    id: String,
    /// Per column, the non-null values over seeds.
    cols: Vec<Vec<f64>>,
    nulls: Vec<usize>,
    seeds: usize,
    /// Seeds left out of the traits because a loop the talk leaves uncapped didn't finish.
    unfinished: usize,
    /// Per failure, the seeds that had it.
    fails: Vec<usize>,
    /// What the talk's file says of the demo beyond its rows, each value already JSON.
    extra: Vec<(String, String)>,
}

fn read(path: &str) -> Result<Vec<Demo>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let doc = json::parse(&text).map_err(|e| format!("{path}: line {}: {}", e.line, e.msg))?;
    let bad = || format!("{path}: not a map-lab-traits/1 file");
    let Json::Obj(top) = &doc else { return Err(bad()) };
    let get = |o: &'_ [(String, Json<'_>)], k: &str| o.iter().position(|(n, _)| n == k);
    let Some(Json::Arr(demos)) = get(top, "demos").map(|i| &top[i].1) else { return Err(bad()) };
    let mut out = Vec::new();
    for d in demos {
        let Json::Obj(d) = d else { return Err(bad()) };
        let Some(Json::Str(id)) = get(d, "id").map(|i| &d[i].1) else { return Err(bad()) };
        let Some(Json::Arr(rows)) = get(d, "rows").map(|i| &d[i].1) else { return Err(bad()) };
        let mut cols = vec![Vec::new(); COLUMNS.len()];
        let mut nulls = vec![0; COLUMNS.len()];
        let mut fails = vec![0; FAILURES.len()];
        let mut unfinished = 0;
        for r in rows {
            let Json::Arr(cells) = r else { return Err(bad()) };
            let names: Vec<&str> = match cells.last() {
                Some(Json::Arr(fs)) => fs.iter().filter_map(|f| if let Json::Str(s) = f { Some(s.as_str()) } else { None }).collect(),
                _ => Vec::new(),
            };
            // A seed whose run hit a loop the talk leaves uncapped has no finished map on the
            // talk's side; its traits, and the failures read from them, are left out on both sides.
            let finished = !names.contains(&"guard-hit");
            for name in &names {
                if let Some(b) = FAILURES.iter().position(|(n, _)| n == name) {
                    if finished || b < FIRST_TRAIT_FAILURE {
                        fails[b] += 1;
                    }
                }
            }
            if !finished {
                unfinished += 1;
                continue;
            }
            for c in 0..COLUMNS.len() {
                match &cells[c + 1] {
                    Json::Num(s) => cols[c].push(s.parse::<f64>().map_err(|_| bad())?),
                    Json::Null => nulls[c] += 1,
                    _ => return Err(bad()),
                }
            }
        }
        let mut extra = Vec::new();
        for (k, v) in d.iter() {
            match (k.as_str(), v) {
                ("source" | "classification" | "differences", Json::Str(s)) => extra.push((k.clone(), string(s))),
                // Counts the talk's harness made of its own runs, copied as they are.
                ("astarVsShortest", Json::Obj(o)) => {
                    let mut out = Obj::new();
                    for (name, x) in o {
                        let Json::Num(n) = x else { return Err(bad()) };
                        out.raw(name, n);
                    }
                    extra.push((k.clone(), out.end()));
                }
                _ => {}
            }
        }
        out.push(Demo { id: id.clone(), cols, nulls, seeds: rows.len(), unfinished, fails, extra });
    }
    Ok(out)
}

/// The two-sample Kolmogorov–Smirnov statistic and its asymptotic p-value.
pub fn ks(a: &[f64], b: &[f64]) -> (f64, f64) {
    let mut a = a.to_vec();
    let mut b = b.to_vec();
    a.sort_by(f64::total_cmp);
    b.sort_by(f64::total_cmp);
    let (n, m) = (a.len() as f64, b.len() as f64);
    let (mut i, mut j, mut d) = (0, 0, 0f64);
    while i < a.len() && j < b.len() {
        let x = a[i].min(b[j]);
        while i < a.len() && a[i] <= x {
            i += 1;
        }
        while j < b.len() && b[j] <= x {
            j += 1;
        }
        d = d.max((i as f64 / n - j as f64 / m).abs());
    }
    let en = (n * m / (n + m)).sqrt();
    let lambda = (en + 0.12 + 0.11 / en) * d;
    // The alternating series for Q_KS(lambda); it doesn't converge as lambda goes to 0, where
    // Q_KS is 1 (Numerical Recipes returns 1 when it fails to converge).
    let mut p = 0.0;
    let mut sign = 1.0;
    for k in 1..=100 {
        let term = sign * 2.0 * (-2.0 * (k as f64).powi(2) * lambda * lambda).exp();
        p += term;
        if term.abs() < 1e-10 {
            return (d, p.clamp(0.0, 1.0));
        }
        sign = -sign;
    }
    (d, 1.0)
}

/// The two-sided p-value of a two-proportion z-test (pooled), 1 when the rates are equal.
pub fn two_proportions(x1: usize, n1: usize, x2: usize, n2: usize) -> f64 {
    let (p1, p2) = (x1 as f64 / n1 as f64, x2 as f64 / n2 as f64);
    let p = (x1 + x2) as f64 / (n1 + n2) as f64;
    let se = (p * (1.0 - p) * (1.0 / n1 as f64 + 1.0 / n2 as f64)).sqrt();
    if se == 0.0 {
        return 1.0;
    }
    erfc((p1 - p2).abs() / se / std::f64::consts::SQRT_2).min(1.0)
}

/// The complementary error function (Numerical Recipes' Chebyshev fit, error below 1.2e-7).
fn erfc(x: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.5 * x.abs());
    let y = t * (-x * x - 1.26551223
        + t * (1.00002368 + t * (0.37409196 + t * (0.09678418 + t * (-0.18628806 + t * (0.27886807 + t * (-1.13520398 + t * (1.48851587 + t * (-0.82215223 + t * 0.17087277)))))))))
        .exp();
    if x >= 0.0 { y } else { 2.0 - y }
}

fn mean_sd(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let m = v.iter().sum::<f64>() / n;
    let sd = if v.len() > 1 { (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1.0)).sqrt() } else { 0.0 };
    (m, sd)
}

fn r6(x: f64) -> String {
    num((x * 1e6).round() / 1e6)
}

pub fn cmd(a: &[String]) -> Result<(), String> {
    let a = parse_args(a, &["out"])?;
    let [ours, theirs] = a.files.as_slice() else { return Err("compare takes two traits files: ours, then the talk's".into()) };
    let (mine, talk) = (read(ours)?, read(theirs)?);
    let mut demos = Vec::new();
    let mut tally = (0, 0);
    for d in &mine {
        let Some(t) = talk.iter().find(|t| t.id == d.id) else { continue };
        let mut o = Obj::lines(6);
        o.raw("id", &string(&d.id));
        o.raw("seeds", &format!("[{}, {}]", d.seeds, t.seeds));
        if d.unfinished + t.unfinished > 0 {
            o.raw("unfinished", &format!("[{}, {}]", d.unfinished, t.unfinished));
        }
        for (k, v) in &t.extra {
            o.raw(k, v);
        }
        let mut traits = Obj::lines(8);
        for (c, (name, _)) in COLUMNS.iter().enumerate() {
            if *name == "probed" {
                continue;
            }
            let (x, y) = (&d.cols[c], &t.cols[c]);
            let mut e = Obj::new();
            if x.is_empty() || y.is_empty() {
                if x.is_empty() && y.is_empty() {
                    continue;
                }
                e.raw("verdict", &string("one side only"));
                e.raw("nulls", &format!("[{}, {}]", d.nulls[c], t.nulls[c]));
                traits.raw(name, &e.end());
                continue;
            }
            let ((mx, sx), (my, sy)) = (mean_sd(x), mean_sd(y));
            let (dks, p) = ks(x, y);
            let pooled = ((sx * sx + sy * sy) / 2.0).sqrt();
            let effect = if pooled > 0.0 { (mx - my) / pooled } else if mx == my { 0.0 } else { f64::INFINITY };
            let verdict = if dks == 0.0 { "identical" } else if p >= ALPHA { "consistent" } else { "differs" };
            tally.0 += 1;
            if verdict == "differs" {
                tally.1 += 1;
            }
            e.raw("ours", &format!("[{}, {}]", r6(mx), r6(sx))).raw("talk", &format!("[{}, {}]", r6(my), r6(sy)));
            e.raw("ksD", &r6(dks)).raw("ksP", &r6(p));
            e.raw("effect", &if effect.is_finite() { r6(effect) } else { "null".into() });
            if d.nulls[c] + t.nulls[c] > 0 {
                e.raw("nulls", &format!("[{}, {}]", d.nulls[c], t.nulls[c]));
            }
            e.raw("verdict", &string(verdict));
            traits.raw(name, &e.end());
        }
        o.raw("traits", &traits.end());
        let mut f = Obj::new();
        for (b, (name, _)) in FAILURES.iter().enumerate() {
            if d.fails[b] + t.fails[b] > 0 {
                let (nd, nt) = if b < FIRST_TRAIT_FAILURE { (d.seeds, t.seeds) } else { (d.seeds - d.unfinished, t.seeds - t.unfinished) };
                let p = two_proportions(d.fails[b], nd, t.fails[b], nt);
                f.raw(name, &format!("[{}, {}, {}]", r6(d.fails[b] as f64 / nd as f64), r6(t.fails[b] as f64 / nt as f64), r6(p)));
            }
        }
        o.raw("failureRates", &f.end());
        demos.push(o.end());
    }
    let mut o = Obj::lines(2);
    o.raw("format", &string("map-lab-faithfulness/1"));
    o.raw("ours", &string(ours)).raw("talk", &string(theirs));
    o.raw(
        "method",
        &string(&format!(
            "For each demo and trait: [mean, standard deviation] for ours and for the talk's code, the two-sample Kolmogorov-Smirnov statistic D and its asymptotic p-value, and the effect size (difference of means over the pooled standard deviation). A trait differs when p < {ALPHA}; KS is conservative for whole-number traits. Traits are over finished maps only: seeds with guard-hit (a loop the talk leaves uncapped that didn't end, counted per side in unfinished) are left out. failureRates: [ours, talk's, p], the share of seeds with each failure (for failures read from a finished map's traits, the share of finished seeds) and a two-proportion z-test's p-value."
        )),
    );
    o.raw("tally", &format!("{{\"traits\": {}, \"differ\": {}}}", tally.0, tally.1));
    o.raw("demos", &format!("[\n    {}\n  ]", demos.join(",\n    ")));
    write_out(a.get("out"), &o.end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ks_on_known_samples() {
        let a: Vec<f64> = (0..100).map(|i| i as f64).collect();
        assert_eq!(ks(&a, &a), (0.0, 1.0));
        let b: Vec<f64> = (200..300).map(|i| i as f64).collect();
        let (d, p) = ks(&a, &b);
        assert_eq!(d, 1.0);
        assert!(p < 1e-20);
        let c: Vec<f64> = (0..100).map(|i| i as f64 + 0.5).collect();
        let (d, p) = ks(&a, &c);
        assert!(d <= 0.011 && p > 0.99, "{d} {p}");
    }

    #[test]
    fn proportions() {
        assert_eq!(two_proportions(10, 100, 10, 100), 1.0);
        // 50/100 against 30/100: z = 2.89, p = 0.0039.
        assert!((two_proportions(50, 100, 30, 100) - 0.0039).abs() < 2e-4);
        assert_eq!(two_proportions(0, 10, 0, 10), 1.0);
    }
}
