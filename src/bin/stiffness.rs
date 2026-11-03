//! Can a local per-weight brake stop walking from erasing turning? Runs
//! `apprentice::retention` plain (P) and with stiffness (S) on the same
//! seeds and judges the marks in `docs/stiffness/README.md`.

use std::process::ExitCode;

use apprentice::judge::{Interval, JudgeError, Verdict};
use apprentice::retention::{Kept, Retention, retain_seeds};

const USAGE: &str = "\
usage: stiffness [--tune] [--strength L] [--from SEED] [--seeds N]

Seeds FROM..FROM+N-1 (default 101..102, smoke). --tune runs every strength
in {1, 3, 10, 30} against plain on the seeds (tune on --from 101 --seeds
10) and picks one by the pinned rule. Otherwise runs plain and strength L
(default 10) and judges the three marks (judge on --from 1201 --seeds 200).";

const PLAIN: Retention = Retention {
    turning: 20_000,
    walking: 40_000,
    test: 2_000,
    rate: 0.05,
    stiffness: None,
};
const STRENGTHS: [f64; 4] = [1.0, 3.0, 10.0, 30.0];
/// Largest stiff drop that counts as held (mark 1).
const HELD: f64 = 0.05;
/// Largest walking loss against plain that counts as no regression
/// (mark 3, and the tuning filter).
const WALK_LOSS: f64 = 0.02;

struct Args {
    seeds: Vec<u64>,
    tune: bool,
    strength: f64,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let (mut from, mut n) = (101_u64, 2_u64);
    let mut a = Args {
        seeds: Vec::new(),
        tune: false,
        strength: 10.0,
    };
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        if flag == "--tune" {
            a.tune = true;
            continue;
        }
        let value = it.next().ok_or(format!("{flag} needs a value"))?;
        let bad = || format!("bad value for {flag}: {value}");
        match flag.as_str() {
            "--from" => from = value.parse().map_err(|_| bad())?,
            "--seeds" => n = value.parse().map_err(|_| bad())?,
            "--strength" => a.strength = value.parse().map_err(|_| bad())?,
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    a.seeds = (from..from + n).collect();
    Ok(a)
}

fn arm(strength: Option<f64>, seeds: &[u64]) -> Result<Vec<Kept>, String> {
    let r = Retention {
        stiffness: strength,
        ..PLAIN
    };
    retain_seeds(&r, seeds).map_err(|e| e.to_string())
}

fn col(kept: &[Kept], f: impl Fn(&Kept) -> f64) -> Vec<f64> {
    kept.iter().map(f).collect()
}

fn mean(xs: &[f64]) -> f64 {
    xs.iter().sum::<f64>() / xs.len().max(1) as f64
}

fn drop(k: &Kept) -> f64 {
    k.before - k.after_walking
}

fn paired(p: &[Kept], s: &[Kept], f: impl Fn(&Kept, &Kept) -> f64) -> Result<Interval, String> {
    let xs: Vec<f64> = p.iter().zip(s).map(|(p, s)| f(p, s)).collect();
    Interval::mean(&xs).map_err(|e: JudgeError| e.to_string())
}

fn show(i: &Interval) -> String {
    format!("{:+.3} [{:+.3}, {:+.3}]", i.mid, i.lo, i.hi)
}

fn summary(label: &str, k: &[Kept]) {
    println!(
        "{label:<8} drop {:.3}  walking one-ahead {:.3}  |B-A| {:.3}  misplaced {:.2}",
        mean(&col(k, drop)),
        mean(&col(k, |k| k.walking_one)),
        mean(&col(k, |k| k.moved_walking)),
        mean(&col(k, |k| k.misplaced)),
    );
}

/// The pinned tuning rule: smallest mean drop among strengths whose mean
/// walking one-ahead is at most `WALK_LOSS` below plain's; if none, the
/// smallest strength.
fn tune(seeds: &[u64]) -> Result<(), String> {
    let plain = arm(None, seeds)?;
    summary("plain", &plain);
    let walk = mean(&col(&plain, |k| k.walking_one));
    let mut best: Option<(f64, f64)> = None;
    for strength in STRENGTHS {
        let stiff = arm(Some(strength), seeds)?;
        summary(&format!("L={strength}"), &stiff);
        let d = mean(&col(&stiff, drop));
        let fair = walk - mean(&col(&stiff, |k| k.walking_one)) <= WALK_LOSS;
        if fair && best.is_none_or(|(_, b)| d < b) {
            best = Some((strength, d));
        }
    }
    match best {
        Some((strength, _)) => println!("picked strength {strength}"),
        None => println!(
            "no strength keeps walking within {WALK_LOSS}; rule takes {}",
            STRENGTHS[0]
        ),
    }
    Ok(())
}

fn judge(seeds: &[u64], strength: f64) -> Result<(), String> {
    let plain = arm(None, seeds)?;
    let stiff = arm(Some(strength), seeds)?;
    summary("P", &plain);
    summary(&format!("S L={strength}"), &stiff);
    let held = Interval::mean(&col(&stiff, drop)).map_err(|e| e.to_string())?;
    let halved = paired(&plain, &stiff, |p, s| drop(s) - 0.5 * drop(p))?;
    let walk = paired(&plain, &stiff, |p, s| p.walking_one - s.walking_one)?;
    let p_drop = Interval::mean(&col(&plain, drop)).map_err(|e| e.to_string())?;
    let m1 = held.at_most(HELD);
    let m2 = halved.at_most(0.0);
    let m3 = walk.at_most(WALK_LOSS);
    println!(
        "1 held:          S drop {}  (top <= {HELD}): {m1}",
        show(&held)
    );
    println!(
        "2 halved:        S - P/2 {}  (top <= 0): {m2}",
        show(&halved)
    );
    println!(
        "3 no regression: P - S walking {}  (top <= {WALK_LOSS}): {m3}",
        show(&walk)
    );
    println!("  P drop (reported) {}", show(&p_drop));
    let reading = match (m1, m2, m3) {
        (_, _, Verdict::Missed) => {
            "protects turning only by slowing walking: a trade, not proposed"
        }
        (Verdict::Met, _, Verdict::Met) => "stiffness is enough here: propose it for #707",
        (_, Verdict::Met, Verdict::Met) => "helps but not enough: next is adding units",
        _ => "no proposal; see marks",
    };
    println!("reading: {reading}");
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let result = parse(&args).and_then(|a| {
        println!(
            "{PLAIN:?}, seeds {:?}..={:?}",
            a.seeds.first(),
            a.seeds.last()
        );
        if a.tune {
            tune(&a.seeds)
        } else {
            judge(&a.seeds, a.strength)
        }
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}
