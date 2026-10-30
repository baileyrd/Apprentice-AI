//! Sweeping: do patterns that know how long a square went unseen, and plans
//! that count the food on every square of their path, make barren worlds
//! easier, in every world of a life, at no cost? Each
//! seed is lived two ways in the shared life of `apprentice::life`. Rules
//! are in docs/sweep/README.md.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use apprentice::apprentice::Drives;
use apprentice::judge::Interval;
use apprentice::life::{Life, Measure, Setting, lives_under};
use apprentice::memory::Recall;
use apprentice::patterns::PatternNet;

const USAGE: &str = "\
usage: sweep [--seeds N] [--from SEED] [--judge] [--out DIR]

Seeds FROM..FROM+N-1 (default 101..110, the tuning seeds), or with --judge
the judging seeds 1-100 and 111-210. Lives each seed two ways: A today's
default; S with patterns that see time unseen and sweeping plans. Prints each rule's verdict and writes
DIR/sweep.csv (default DIR: runs/sweep).";

type Error = Box<dyn std::error::Error>;

/// Parsed flags.
struct Args {
    seeds: Vec<u64>,
    out: PathBuf,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let (mut count, mut from, mut judge) = (10, 101, false);
    let mut out = PathBuf::from("runs/sweep");
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
    Ok(Args { seeds, out })
}

/// Per-seed measures, in CSV column order.
const MEASURES: [Measure; 26] = [
    ("barren_sleeps", |l| f64::from(l.barren.sleeps)),
    ("raised_barren_sleeps", |l| f64::from(l.raised_sleeps(true))),
    ("bush_sleeps", |l| f64::from(l.bush.sleeps)),
    ("raised_bush_sleeps", |l| f64::from(l.raised_sleeps(false))),
    ("barren_hopes", |l| f64::from(l.barren.hopes)),
    ("home_wrong_100", |l| f64::from(l.home.0)),
    ("home_wrong_300", |l| f64::from(l.home.1)),
    ("bush_meals", |l| f64::from(l.bush.meals)),
    ("barren_meals", |l| f64::from(l.barren.meals)),
    ("barren_futile", |l| f64::from(l.barren.futile)),
    ("barren_first_w1_sleeps", |l| {
        f64::from(l.barren_raised[0].sleeps)
    }),
    ("barren_first_w2_sleeps", |l| {
        f64::from(l.barren_raised[1].sleeps)
    }),
    ("barren_first_w3_sleeps", |l| {
        f64::from(l.barren_raised[2].sleeps)
    }),
    ("barren_first_w4_sleeps", |l| {
        f64::from(l.barren_raised[3].sleeps)
    }),
    ("barren_first_w1_meals", |l| {
        f64::from(l.barren_raised[0].meals)
    }),
    ("barren_first_w2_meals", |l| {
        f64::from(l.barren_raised[1].meals)
    }),
    ("barren_first_w3_meals", |l| {
        f64::from(l.barren_raised[2].meals)
    }),
    ("barren_first_w4_meals", |l| {
        f64::from(l.barren_raised[3].meals)
    }),
    ("bush_first_w1_sleeps", |l| {
        f64::from(l.bush_raised[0].sleeps)
    }),
    ("bush_first_w2_sleeps", |l| {
        f64::from(l.bush_raised[1].sleeps)
    }),
    ("bush_first_w3_sleeps", |l| {
        f64::from(l.bush_raised[2].sleeps)
    }),
    ("bush_first_w4_sleeps", |l| {
        f64::from(l.bush_raised[3].sleeps)
    }),
    ("bush_first_w1_meals", |l| f64::from(l.bush_raised[0].meals)),
    ("bush_first_w2_meals", |l| f64::from(l.bush_raised[1].meals)),
    ("bush_first_w3_meals", |l| f64::from(l.bush_raised[2].meals)),
    ("bush_first_w4_meals", |l| f64::from(l.bush_raised[3].meals)),
];

/// One seed and its life under each setting.
type Row = (u64, Vec<Life>);
const NAMES: [&str; 2] = ["A", "S"];

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
    let (a, s) = (0, 1);
    let easier = pair(rows, a, s, MEASURES[0].1)?;
    println!(
        "1. barren test sleeps, S vs A {}: {}",
        pct(&easier),
        easier.at_most(-0.25)
    );
    let every = pair(rows, a, s, MEASURES[1].1)?;
    println!(
        "2. barren upbringing sleeps, S vs A {}: {}",
        pct(&every),
        every.at_most(-0.25)
    );
    println!("3. S vs A, each at most +10%:");
    for (name, m) in &MEASURES[2..6] {
        let i = pair(rows, a, s, *m)?;
        println!("   {name} {}: {}", pct(&i), i.at_most(0.1));
    }
    for (setting, rule) in [(s, "4."), (a, "   (A, not judged)")] {
        let first: Vec<f64> = rows
            .iter()
            .map(|r| f64::from(r.1[setting].barren_raised[0].sleeps))
            .collect();
        let last: Vec<f64> = rows
            .iter()
            .map(|r| f64::from(r.1[setting].barren.sleeps) / 2.0)
            .collect();
        let i = Interval::change(&first, &last)?;
        println!(
            "{rule} {} barren test vs first barren world, per 1,000 steps {}: {}",
            NAMES[setting],
            pct(&i),
            i.at_most(0.0)
        );
    }
    println!("Means ({}):", NAMES.join(", "));
    for (name, m) in MEASURES {
        let means: Vec<String> = (0..NAMES.len())
            .map(|s| {
                let total: f64 = rows.iter().map(|r| m(&r.1[s])).sum();
                format!("{:.1}", total / rows.len() as f64)
            })
            .collect();
        println!("   {name}: {}", means.join(", "));
    }
    Ok(())
}

fn execute(args: &Args) -> Result<(), Error> {
    let a = Setting::from(Recall::default());
    let sweep = Drives {
        sweep: true,
        ..Drives::default()
    };
    let unseen = Recall {
        pattern_net: Some(PatternNet {
            unseen: true,
            ..PatternNet::default()
        }),
        ..Recall::default()
    };
    let s = Setting {
        drives: sweep,
        ..Setting::from(unseen)
    };
    let rows = lives_under(&args.seeds, &[a, s])?;
    report(&rows)?;
    fs::create_dir_all(&args.out)?;
    fs::write(args.out.join("sweep.csv"), csv(&rows))?;
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
        eprintln!("sweep: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
