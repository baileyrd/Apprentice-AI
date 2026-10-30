//! Selective-sleep experiment: does a replay budget let sleep do less work
//! without losing anything, and does it let the memory gate go on? Each seed
//! is lived four ways in the shared life of `apprentice::life`: gate off or
//! on, whole diary or a budget. Rules are in docs/sleep/README.md.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use apprentice::judge::Interval;
use apprentice::life::{Life, Measure, lives};
use apprentice::memory::{Gate, Recall};

const USAGE: &str = "\
usage: sleep [--seeds N] [--from SEED] [--judge] [--budget B] [--out DIR]

Seeds FROM..FROM+N-1 (default 101..110, the tuning seeds), or with --judge
the judging seeds 1-100 and 111-210. Lives each seed four ways: gate off
or on, whole diary or at most B entries replayed per sleep besides meals
(default 144). Prints each rule's verdict and writes DIR/sleep.csv
(default DIR: runs/sleep).";

type Error = Box<dyn std::error::Error>;

/// Parsed flags.
struct Args {
    seeds: Vec<u64>,
    budget: usize,
    out: PathBuf,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let (mut count, mut from, mut judge, mut budget) = (10, 101, false, 144);
    let mut out = PathBuf::from("runs/sleep");
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        if flag == "--judge" {
            judge = true;
            continue;
        }
        let value = it.next().ok_or(format!("{flag} needs a value"))?;
        let bad = || format!("bad {flag}: {value}");
        match flag.as_str() {
            "--seeds" => count = value.parse().map_err(|_| bad())?,
            "--from" => from = value.parse().map_err(|_| bad())?,
            "--budget" => budget = value.parse().map_err(|_| bad())?,
            "--out" => out = PathBuf::from(value),
            _ => return Err(format!("unknown flag: {flag}")),
        }
    }
    if count == 0 {
        return Err("--seeds must be positive".into());
    }
    let seeds = if judge {
        (1..=100).chain(111..=210).collect()
    } else {
        (from..from + count).collect()
    };
    Ok(Args { seeds, budget, out })
}

/// Per-seed measures, in CSV column order.
const MEASURES: [Measure; 10] = [
    ("bush_replayed", |l| l.bush.traffic.replayed as f64),
    ("bush_sleeps", |l| f64::from(l.bush.sleeps)),
    ("barren_sleeps", |l| f64::from(l.barren.sleeps)),
    ("barren_hopes", |l| f64::from(l.barren.hopes)),
    ("home_wrong_100", |l| f64::from(l.home.0)),
    ("home_wrong_300", |l| f64::from(l.home.1)),
    ("raised_sleeps", |l| f64::from(l.raised.sleeps)),
    ("bush_meals", |l| f64::from(l.bush.meals)),
    ("barren_meals", |l| f64::from(l.barren.meals)),
    ("bush_naps", |l| l.bush.traffic.naps as f64),
];

/// One seed and its life under each of the four settings.
type Row = (u64, [Life; 4]);
const NAMES: [&str; 4] = ["A", "B", "C", "D"];

fn csv(rows: &[Row]) -> String {
    let names: Vec<&str> = MEASURES.iter().map(|m| m.0).collect();
    let mut out = format!("seed,setting,{}\n", names.join(","));
    for (seed, lives) in rows {
        for (name, l) in NAMES.iter().zip(lives) {
            let values: Vec<String> = MEASURES.iter().map(|m| m.1(l).to_string()).collect();
            out.push_str(&format!("{seed},{name},{}\n", values.join(",")));
        }
    }
    out
}

fn pct(i: &Interval) -> String {
    format!(
        "{:+.0}% [{:+.0}%, {:+.0}%]",
        100.0 * i.mid,
        100.0 * i.lo,
        100.0 * i.hi
    )
}

/// Paired change of `f` from setting `base` to setting `test`.
fn pair(rows: &[Row], base: usize, test: usize, f: fn(&Life) -> f64) -> Result<Interval, Error> {
    let b: Vec<f64> = rows.iter().map(|r| f(&r.1[base])).collect();
    let t: Vec<f64> = rows.iter().map(|r| f(&r.1[test])).collect();
    Ok(Interval::change(&b, &t)?)
}

fn report(rows: &[Row]) -> Result<(), Error> {
    let (a, b, c, d) = (0, 1, 2, 3);
    let work = pair(rows, a, b, MEASURES[0].1)?;
    println!(
        "1. bush replays, B vs A {}: {}",
        pct(&work),
        work.at_most(-0.5)
    );
    for (rule, test) in [(2, b), (3, d)] {
        println!("{rule}. {} vs A, each at most +10%:", NAMES[test]);
        for (name, f) in &MEASURES[1..5] {
            let i = pair(rows, a, test, *f)?;
            println!("   {name} {}: {}", pct(&i), i.at_most(0.1));
        }
    }
    let fix = pair(rows, c, d, MEASURES[1].1)?;
    println!(
        "4. bush sleeps, D vs C {}: {}",
        pct(&fix),
        fix.at_most(-0.1)
    );
    println!("Means (A, B, C, D):");
    for (name, f) in MEASURES {
        let means: Vec<String> = (0..4)
            .map(|s| {
                format!(
                    "{:.1}",
                    rows.iter().map(|r| f(&r.1[s])).sum::<f64>() / rows.len() as f64
                )
            })
            .collect();
        println!("   {name}: {}", means.join(", "));
    }
    Ok(())
}

fn execute(args: &Args) -> Result<(), Error> {
    let a = Recall::default();
    let b = Recall {
        replay_budget: Some(args.budget),
        ..a
    };
    let c = Recall {
        gate: Some(Gate::default()),
        ..a
    };
    let d = Recall {
        replay_budget: Some(args.budget),
        ..c
    };
    let ab = lives(&args.seeds, a, b)?;
    let cd = lives(&args.seeds, c, d)?;
    let rows: Vec<Row> = ab
        .into_iter()
        .zip(cd)
        .map(|((seed, la, lb), (_, lc, ld))| (seed, [la, lb, lc, ld]))
        .collect();
    report(&rows)?;
    fs::create_dir_all(&args.out)?;
    fs::write(args.out.join("sleep.csv"), csv(&rows))?;
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let args = match parse(&args) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("{e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = execute(&args) {
        eprintln!("sleep: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
