//! Gate experiment: does active memory's gate keep the diary small without
//! losing anything that matters?
//!
//! Each seed is lived twice, gate off and gate on: the language test's life
//! with no mentor (four upbringing worlds, then a new bush world and, from a
//! second upbringing, a new barren world), and the home test (home, then
//! home as left, home with a wall moved, three strangers). Rules are in
//! docs/gate/README.md.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use apprentice::judge::Interval;
use apprentice::life::{Life, Measure, lives};
use apprentice::memory::{Gate, Recall, Traffic};

const USAGE: &str = "\
usage: gate [--seeds N] [--from SEED] [--judge] [--threshold X] [--span K] [--out DIR]

Seeds FROM..FROM+N-1 (default 101..110, the tuning seeds), or with --judge
the judging seeds 1-100 and 111-210. Lives each seed with the gate off and
on (threshold X, default 0.05; active memory of K squares, default 18),
prints each rule's verdict and writes DIR/gate.csv (default DIR: runs/gate).";

type Error = Box<dyn std::error::Error>;

/// Share of unexpected sightings written; 1 when there were none.
fn let_through(t: &Traffic) -> f64 {
    if t.unexpected == 0 {
        return 1.0;
    }
    t.unexpected_written as f64 / t.unexpected as f64
}

/// Parsed flags.
struct Args {
    seeds: Vec<u64>,
    gate: Gate,
    /// Diary capacity with the gate on, in steps; `None` keeps the default.
    capacity: Option<usize>,
    out: PathBuf,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let (mut count, mut from, mut judge) = (10, 101, false);
    let mut gate = Gate::default();
    let mut capacity = None;
    let mut out = PathBuf::from("runs/gate");
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
            "--threshold" => gate.threshold = value.parse().map_err(|_| bad())?,
            "--capacity" => capacity = Some(value.parse().map_err(|_| bad())?),
            "--span" => gate.span = value.parse().map_err(|_| bad())?,
            "--out" => out = PathBuf::from(value),
            _ => return Err(format!("unknown flag: {flag}")),
        }
    }
    if count == 0 || gate.span == 0 {
        return Err("--seeds and --span must be positive".into());
    }
    let seeds = if judge {
        (1..=100).chain(111..=210).collect()
    } else {
        (from..from + count).collect()
    };
    Ok(Args {
        seeds,
        gate,
        capacity,
        out,
    })
}

fn csv(rows: &[(u64, Life, Life)]) -> String {
    let mut out = String::from(
        "seed,gate,raised_sightings,raised_written,raised_naps,raised_sleeps,bush_sightings,bush_written,bush_naps,bush_unexpected,bush_unexpected_written,bush_sleeps,barren_written,barren_sleeps,barren_hopes,barren_meals,barren_futile,bush_meals,home_wrong_100,home_wrong_300\n",
    );
    for (seed, off, on) in rows {
        for (name, l) in [("off", off), ("on", on)] {
            let (r, b) = (l.raised.traffic, l.bush.traffic);
            out.push_str(&format!(
                "{seed},{name},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                r.sightings,
                r.written,
                r.naps,
                l.raised.sleeps,
                b.sightings,
                b.written,
                b.naps,
                b.unexpected,
                b.unexpected_written,
                l.bush.sleeps,
                l.barren.traffic.written,
                l.barren.sleeps,
                l.barren.hopes,
                l.barren.meals,
                l.barren.futile,
                l.bush.meals,
                l.home.0,
                l.home.1,
            ));
        }
    }
    out
}

fn report(rows: &[(u64, Life, Life)]) -> Result<(), Error> {
    let pair = |f: fn(&Life) -> f64| -> Result<Interval, Error> {
        let base: Vec<f64> = rows.iter().map(|r| f(&r.1)).collect();
        let test: Vec<f64> = rows.iter().map(|r| f(&r.2)).collect();
        Ok(Interval::change(&base, &test)?)
    };
    let pct = |i: &Interval| {
        format!(
            "{:+.0}% [{:+.0}%, {:+.0}%]",
            100.0 * i.mid,
            100.0 * i.lo,
            100.0 * i.hi
        )
    };
    let written = pair(|l| l.bush.traffic.written as f64)?;
    println!(
        "1. bush-world diary writes {}: {}",
        pct(&written),
        written.at_most(-0.5)
    );
    let naps = pair(|l| l.bush.traffic.naps as f64)?;
    println!("2. bush-world naps {}: {}", pct(&naps), naps.at_most(-0.3));
    let through: Vec<f64> = rows
        .iter()
        .map(|r| let_through(&r.2.bush.traffic))
        .collect();
    let share = Interval::median(&through)?;
    println!(
        "3. unexpected sightings written, median share {:.3} [{:.3}, {:.3}]: {}",
        share.mid,
        share.lo,
        share.hi,
        share.at_least(0.95)
    );
    println!("4. no regressions (each at most +10%):");
    let checks: [Measure; 4] = [
        ("bush sleeps", |l| f64::from(l.bush.sleeps)),
        ("barren sleeps", |l| f64::from(l.barren.sleeps)),
        ("barren false hopes", |l| f64::from(l.barren.hopes)),
        ("home-test wrong answers (100 steps)", |l| {
            f64::from(l.home.0)
        }),
    ];
    for (name, f) in checks {
        let i = pair(f)?;
        println!("   {name} {}: {}", pct(&i), i.at_most(0.1));
    }
    println!("Reported only:");
    let reported: [Measure; 4] = [
        ("upbringing diary writes", |l| {
            l.raised.traffic.written as f64
        }),
        ("upbringing naps", |l| l.raised.traffic.naps as f64),
        ("upbringing sleeps", |l| f64::from(l.raised.sleeps)),
        ("home-test wrong answers (300 steps)", |l| {
            f64::from(l.home.1)
        }),
    ];
    for (name, f) in reported {
        println!("   {name} {}", pct(&pair(f)?));
    }
    let sum = |f: fn(&Life) -> f64, on: bool| -> f64 {
        rows.iter()
            .map(|r| f(if on { &r.2 } else { &r.1 }))
            .sum::<f64>()
            / rows.len() as f64
    };
    let means: [Measure; 8] = [
        ("barren meals", |l| f64::from(l.barren.meals)),
        ("barren futile returns", |l| f64::from(l.barren.futile)),
        ("bush meals", |l| f64::from(l.bush.meals)),
        ("bush writes", |l| l.bush.traffic.written as f64),
        ("bush naps", |l| l.bush.traffic.naps as f64),
        ("bush sleeps", |l| f64::from(l.bush.sleeps)),
        ("barren false hopes", |l| f64::from(l.barren.hopes)),
        ("home wrong (100)", |l| f64::from(l.home.0)),
    ];
    for (name, f) in means {
        println!(
            "   mean {name}: off {:.1}, on {:.1}",
            sum(f, false),
            sum(f, true)
        );
    }
    Ok(())
}

fn execute(args: &Args) -> Result<(), Error> {
    let off = Recall::default();
    let on = Recall {
        gate: Some(args.gate),
        capacity: args.capacity.unwrap_or(off.capacity),
        ..off
    };
    let rows = lives(&args.seeds, off, on)?;
    report(&rows)?;
    fs::create_dir_all(&args.out)?;
    fs::write(args.out.join("gate.csv"), csv(&rows))?;
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
        eprintln!("gate: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
