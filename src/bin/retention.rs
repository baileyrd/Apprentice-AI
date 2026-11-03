//! Does learning to walk erase turning? Runs `apprentice::retention` on
//! each seed and judges the marks in `docs/retention/README.md`.

use std::process::ExitCode;

use apprentice::judge::{Interval, Verdict};
use apprentice::retention::{Kept, Retention, retain_seeds};

const USAGE: &str = "\
usage: retention [--from SEED] [--seeds N]

Seeds FROM..FROM+N-1 (default 101..102, smoke only; judge on --from 1001
--seeds 200). Prints per-seed results, means, and the two pinned marks.";

const RUN: Retention = Retention {
    turning: 20_000,
    walking: 40_000,
    test: 2_000,
    rate: 0.05,
    stiffness: None,
};
/// Largest drop that still counts as held (mark D).
const DROP: f64 = 0.05;
/// Smallest walking / control weight-distance ratio that counts as
/// touched (mark T).
const TOUCH: f64 = 2.0;

fn parse(args: &[String]) -> Result<Vec<u64>, String> {
    let (mut from, mut n) = (101_u64, 2_u64);
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or(format!("{flag} needs a value"))?;
        let v: u64 = value
            .parse()
            .map_err(|_| format!("bad value for {flag}: {value}"))?;
        match flag.as_str() {
            "--from" => from = v,
            "--seeds" => n = v,
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    Ok((from..from + n).collect())
}

fn run(seeds: &[u64]) -> Result<Vec<Kept>, String> {
    retain_seeds(&RUN, seeds).map_err(|e| e.to_string())
}

fn mean(xs: &[f64]) -> f64 {
    xs.iter().sum::<f64>() / xs.len().max(1) as f64
}

fn execute(seeds: &[u64]) -> Result<(), String> {
    println!("{RUN:?}, seeds {:?}..={:?}", seeds.first(), seeds.last());
    let kept = run(seeds)?;
    println!("seed   before  walked  turned  walk1   |A-0|   |B-A|   |C-A|");
    for (seed, k) in seeds.iter().zip(&kept) {
        println!(
            "{seed:<6} {:>6.3} {:>7.3} {:>7.3} {:>6.3} {:>7.3} {:>7.3} {:>7.3}",
            k.before,
            k.after_walking,
            k.after_turning,
            k.walking_one,
            k.moved_learning,
            k.moved_walking,
            k.moved_turning
        );
    }
    let col = |f: fn(&Kept) -> f64| kept.iter().map(f).collect::<Vec<f64>>();
    println!(
        "means  {:>6.3} {:>7.3} {:>7.3} {:>6.3} {:>7.3} {:>7.3} {:>7.3}",
        mean(&col(|k| k.before)),
        mean(&col(|k| k.after_walking)),
        mean(&col(|k| k.after_turning)),
        mean(&col(|k| k.walking_one)),
        mean(&col(|k| k.moved_learning)),
        mean(&col(|k| k.moved_walking)),
        mean(&col(|k| k.moved_turning)),
    );

    let err = |e: apprentice::judge::JudgeError| e.to_string();
    let drop = Interval::mean(&col(|k| k.before - k.after_walking)).map_err(err)?;
    let control = Interval::mean(&col(|k| k.before - k.after_turning)).map_err(err)?;
    let log_ratio =
        Interval::mean(&col(|k| (k.moved_walking / k.moved_turning).ln())).map_err(err)?;
    let log_share =
        Interval::mean(&col(|k| (k.moved_walking / k.moved_learning).ln())).map_err(err)?;
    let d = drop.at_most(DROP);
    let t = log_ratio.at_least(TOUCH.ln());
    let show = |i: &Interval| format!("{:+.3} [{:+.3}, {:+.3}]", i.mid, i.lo, i.hi);
    let times = |i: &Interval| {
        format!(
            "x{:.2} [x{:.2}, x{:.2}]",
            i.mid.exp(),
            i.lo.exp(),
            i.hi.exp()
        )
    };
    println!(
        "D  drop after walking      {}  held (<= {DROP}): {d}",
        show(&drop)
    );
    println!(
        "T  |B-A| / |C-A|          {}  touched (>= x{TOUCH}): {t}",
        times(&log_ratio)
    );
    println!("   control drop (reported) {}", show(&control));
    println!("   |B-A| / |A-0| (reported) {}", times(&log_share));
    println!(
        "   misses after walking guessing a view the centre never shows (reported) {:.2}",
        mean(&col(|k| k.misplaced))
    );
    let reading = match (d, t) {
        (Verdict::Met, Verdict::Met) => "turning held up: drop consolidation from #707",
        (Verdict::Missed, _) => "turning decayed: consolidation stays on #707",
        (Verdict::Met, _) => "no decay, but walking may not have moved it enough to tell",
        (Verdict::Unclear, _) => "unclear: no reading",
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
    match parse(&args).and_then(|seeds| execute(&seeds)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}
