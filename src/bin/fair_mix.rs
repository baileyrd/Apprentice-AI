//! Fair mix: does keeping what sleep teaches the patterns fair (spread
//! evenly, a budget split in the day's proportions of changes) let the gate
//! go on and a network budget lose nothing? Each seed is lived five ways in
//! the shared life of `apprentice::life`. Rules are in docs/fair-mix/README.md.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use apprentice::judge::Interval;
use apprentice::life::{Life, Measure, lives_under};
use apprentice::memory::{Gate, NetReplay, Recall};

const USAGE: &str = "\
usage: fair_mix [--seeds N] [--from SEED] [--judge] [--out DIR]

Seeds FROM..FROM+N-1 (default 101..110, the tuning seeds), or with --judge
the judging seeds 1-100 and 111-210. Lives each seed five ways: A today's
default; C the gate on; H the gate with fair mix; E a network budget of
144 with 64 rehearsed; K that budget with fair mix. Prints each rule's
verdict and writes DIR/fair_mix.csv (default DIR: runs/fair-mix).";

type Error = Box<dyn std::error::Error>;

/// Parsed flags.
struct Args {
    seeds: Vec<u64>,
    out: PathBuf,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let (mut count, mut from, mut judge) = (10, 101, false);
    let mut out = PathBuf::from("runs/fair-mix");
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
const MEASURES: [Measure; 11] = [
    ("bush_taught", |l| l.bush.traffic.taught as f64),
    ("bush_sleeps", |l| f64::from(l.bush.sleeps)),
    ("barren_sleeps", |l| f64::from(l.barren.sleeps)),
    ("barren_hopes", |l| f64::from(l.barren.hopes)),
    ("home_wrong_100", |l| f64::from(l.home.0)),
    ("home_wrong_300", |l| f64::from(l.home.1)),
    ("raised_sleeps", |l| f64::from(l.raised.sleeps)),
    ("bush_meals", |l| f64::from(l.bush.meals)),
    ("barren_meals", |l| f64::from(l.barren.meals)),
    ("barren_futile", |l| f64::from(l.barren.futile)),
    ("bush_written", |l| l.bush.traffic.written as f64),
];

/// One seed and its life under each of the five settings.
type Row = (u64, Vec<Life>);
const NAMES: [&str; 5] = ["A", "C", "H", "E", "K"];

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
    let (a, c, h, k) = (0, 1, 2, 4);
    for (rule, test) in [(1, h), (3, k)] {
        println!("{rule}. {} vs A, each at most +10%:", NAMES[test]);
        for (name, m) in &MEASURES[1..5] {
            let i = pair(rows, a, test, *m)?;
            println!("   {name} {}: {}", pct(&i), i.at_most(0.1));
        }
    }
    let order = pair(rows, c, h, MEASURES[3].1)?;
    println!(
        "2. barren false hopes, H vs C {}: {}",
        pct(&order),
        order.at_most(-0.5)
    );
    let work = pair(rows, a, k, MEASURES[0].1)?;
    println!(
        "4. bush patterns taught, K vs A {}: {}",
        pct(&work),
        work.at_most(-0.5)
    );
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
    let a = Recall::default();
    let c = Recall {
        gate: Some(Gate::default()),
        ..a
    };
    let e = Recall {
        net_replay: Some(NetReplay {
            budget: 144,
            rehearsal: 64,
        }),
        ..a
    };
    let fair = |r: Recall| Recall {
        fair_mix: true,
        ..r
    };
    let rows = lives_under(&args.seeds, &[a, c, fair(c), e, fair(e)])?;
    report(&rows)?;
    fs::create_dir_all(&args.out)?;
    fs::write(args.out.join("fair_mix.csv"), csv(&rows))?;
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
        eprintln!("fair_mix: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
