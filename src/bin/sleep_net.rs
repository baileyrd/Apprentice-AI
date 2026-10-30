//! Selective network sleep: can sleep teach the patterns' network only a
//! budget of the day's surprises, with a rehearsal of older samples, and
//! lose nothing? Each seed is lived four ways in the shared life of
//! `apprentice::life`. Rules are in docs/sleep-net/README.md.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use apprentice::judge::Interval;
use apprentice::life::{Life, Measure, lives};
use apprentice::memory::{NetReplay, Recall};

const USAGE: &str = "\
usage: sleep_net [--seeds N] [--from SEED] [--judge] [--budget B] [--rehearsal R] [--out DIR]

Seeds FROM..FROM+N-1 (default 101..110, the tuning seeds), or with --judge
the judging seeds 1-100 and 111-210. Lives each seed four ways: A today's
default; E the network taught at most B of each day's entries (default
144) after R kept older samples (default 64); F the same without
rehearsal; G E plus the map's budget of 144. Prints each rule's verdict
and writes DIR/sleep_net.csv (default DIR: runs/sleep-net).";

type Error = Box<dyn std::error::Error>;

/// Parsed flags.
struct Args {
    seeds: Vec<u64>,
    budget: usize,
    rehearsal: usize,
    out: PathBuf,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let (mut count, mut from, mut judge) = (10, 101, false);
    let (mut budget, mut rehearsal) = (144, 64);
    let mut out = PathBuf::from("runs/sleep-net");
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
            "--rehearsal" => rehearsal = value.parse().map_err(|_| bad())?,
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
    Ok(Args {
        seeds,
        budget,
        rehearsal,
        out,
    })
}

/// Per-seed measures, in CSV column order.
const MEASURES: [Measure; 10] = [
    ("bush_taught", |l| l.bush.traffic.taught as f64),
    ("bush_sleeps", |l| f64::from(l.bush.sleeps)),
    ("barren_sleeps", |l| f64::from(l.barren.sleeps)),
    ("barren_hopes", |l| f64::from(l.barren.hopes)),
    ("home_wrong_100", |l| f64::from(l.home.0)),
    ("home_wrong_300", |l| f64::from(l.home.1)),
    ("raised_sleeps", |l| f64::from(l.raised.sleeps)),
    ("bush_meals", |l| f64::from(l.bush.meals)),
    ("barren_meals", |l| f64::from(l.barren.meals)),
    ("bush_replayed", |l| l.bush.traffic.replayed as f64),
];

/// One seed and its life under each of the four settings.
type Row = (u64, [Life; 4]);
const NAMES: [&str; 4] = ["A", "E", "F", "G"];

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
    let (a, e, f, g) = (0, 1, 2, 3);
    let work = pair(rows, a, e, MEASURES[0].1)?;
    println!(
        "1. bush patterns taught, E vs A {}: {}",
        pct(&work),
        work.at_most(-0.5)
    );
    for (rule, test) in [(2, e), (4, g)] {
        println!("{rule}. {} vs A, each at most +10%:", NAMES[test]);
        for (name, m) in &MEASURES[1..5] {
            let i = pair(rows, a, test, *m)?;
            println!("   {name} {}: {}", pct(&i), i.at_most(0.1));
        }
    }
    let kept = pair(rows, f, e, MEASURES[3].1)?;
    println!(
        "3. barren false hopes, E vs F {}: {}",
        pct(&kept),
        kept.at_most(-0.2)
    );
    println!("Means (A, E, F, G):");
    for (name, m) in MEASURES {
        let means: Vec<String> = (0..4)
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
    let a = Recall::default();
    let replay = NetReplay {
        budget: args.budget,
        rehearsal: args.rehearsal,
    };
    let e = Recall {
        net_replay: Some(replay),
        ..a
    };
    let f = Recall {
        net_replay: Some(NetReplay {
            rehearsal: 0,
            ..replay
        }),
        ..a
    };
    let g = Recall {
        replay_budget: Some(144),
        ..e
    };
    let ae = lives(&args.seeds, a, e)?;
    let fg = lives(&args.seeds, f, g)?;
    let rows: Vec<Row> = ae
        .into_iter()
        .zip(fg)
        .map(|((seed, la, le), (_, lf, lg))| (seed, [la, le, lf, lg]))
        .collect();
    report(&rows)?;
    fs::create_dir_all(&args.out)?;
    fs::write(args.out.join("sleep_net.csv"), csv(&rows))?;
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
        eprintln!("sleep_net: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
