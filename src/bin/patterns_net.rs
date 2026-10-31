//! Patterns-network experiment: does holding the long-term patterns in a
//! network instead of a table help the apprentice learn sooner, without
//! losing anything? Each seed is lived twice, table and network, in the
//! shared life of `apprentice::life`. Rules are in docs/patterns-net/README.md.

use std::fs;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::process::ExitCode;

use apprentice::judge::Interval;
use apprentice::life::{Life, Measure, lives};
use apprentice::memory::Recall;
use apprentice::patterns::PatternNet;

const USAGE: &str = "\
usage: patterns_net [--seeds N] [--from SEED] [--judge] [--hidden H] [--rate R] [--out DIR]

Seeds FROM..FROM+N-1 (default 101..110, the tuning seeds), or with --judge
the judging seeds 1-100 and 111-210. Lives each seed with the patterns in
a table and in a network of H hidden units (default 64) learning at rate R
(default 0.2), prints each rule's verdict and writes DIR/patterns_net.csv
(default DIR: runs/patterns-net).";

type Error = Box<dyn std::error::Error>;

/// Parsed flags.
struct Args {
    seeds: Vec<u64>,
    net: PatternNet,
    out: PathBuf,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let (mut count, mut from, mut judge) = (10, 101, false);
    let mut net = PatternNet {
        hidden: NonZeroUsize::new(64).ok_or("64 is not zero")?,
        rate: 0.2,
        seed: 0,
        rare: None,
    };
    let mut out = PathBuf::from("runs/patterns-net");
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
            "--hidden" => net.hidden = value.parse().map_err(|_| bad())?,
            "--rate" => net.rate = value.parse().map_err(|_| bad())?,
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
    Ok(Args { seeds, net, out })
}

/// Per-seed measures, in CSV column order.
const MEASURES: [Measure; 10] = [
    ("raised_sleeps", |l| f64::from(l.raised.sleeps)),
    ("bush_sleeps", |l| f64::from(l.bush.sleeps)),
    ("bush_meals", |l| f64::from(l.bush.meals)),
    ("barren_sleeps", |l| f64::from(l.barren.sleeps)),
    ("barren_hopes", |l| f64::from(l.barren.hopes)),
    ("barren_meals", |l| f64::from(l.barren.meals)),
    ("barren_futile", |l| f64::from(l.barren.futile)),
    ("home_wrong_100", |l| f64::from(l.home.0)),
    ("home_wrong_300", |l| f64::from(l.home.1)),
    ("bush_written", |l| l.bush.traffic.written as f64),
];

fn csv(rows: &[(u64, Life, Life)]) -> String {
    let names: Vec<&str> = MEASURES.iter().map(|m| m.0).collect();
    let mut out = format!("seed,patterns,{}\n", names.join(","));
    for (seed, table, net) in rows {
        for (name, l) in [("table", table), ("net", net)] {
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

fn report(rows: &[(u64, Life, Life)]) -> Result<(), Error> {
    let pair = |f: fn(&Life) -> f64| -> Result<Interval, Error> {
        let base: Vec<f64> = rows.iter().map(|r| f(&r.1)).collect();
        let test: Vec<f64> = rows.iter().map(|r| f(&r.2)).collect();
        Ok(Interval::change(&base, &test)?)
    };
    let mean = |f: fn(&Life) -> f64, net: bool| -> f64 {
        rows.iter()
            .map(|r| f(if net { &r.2 } else { &r.1 }))
            .sum::<f64>()
            / rows.len() as f64
    };
    let raised = pair(MEASURES[0].1)?;
    println!(
        "1. upbringing sleeps {}: {}",
        pct(&raised),
        raised.at_most(-0.1)
    );
    let bush = pair(MEASURES[1].1)?;
    println!("2. bush sleeps {}: {}", pct(&bush), bush.at_most(0.1));
    println!("3. no regressions (each at most +10%):");
    for (name, f) in [MEASURES[3], MEASURES[4], MEASURES[7]] {
        let i = pair(f)?;
        println!("   {name} {}: {}", pct(&i), i.at_most(0.1));
    }
    println!("Reported only (change, mean table -> net):");
    for (name, f) in MEASURES {
        println!(
            "   {name} {}  {:.1} -> {:.1}",
            pct(&pair(f)?),
            mean(f, false),
            mean(f, true)
        );
    }
    Ok(())
}

fn execute(args: &Args) -> Result<(), Error> {
    let table = Recall {
        pattern_net: None,
        ..Recall::default()
    };
    let net = Recall {
        pattern_net: Some(args.net),
        ..table
    };
    let rows = lives(&args.seeds, table, net)?;
    report(&rows)?;
    fs::create_dir_all(&args.out)?;
    fs::write(args.out.join("patterns_net.csv"), csv(&rows))?;
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
        eprintln!("patterns_net: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
