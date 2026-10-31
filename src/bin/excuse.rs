//! Excuse: does excusing an eaten square only while the rest of the map
//! fits stop strangers being taken as home, at no cost elsewhere? Arms A
//! (default), X (excuse off), N (narrower), each seed lived in the shared
//! life of `apprentice::life`. Marks are in docs/excuse/README.md.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use apprentice::judge::Interval;
use apprentice::life::{Life, Measure, lives_under};
use apprentice::memory::{Excuse, Recall};

const USAGE: &str = "\
usage: excuse [--seeds FIRST-LAST] [--out DIR]

Lives each seed (default 501-700, the judging seeds) under A (default),
X (excuse off) and N (excuse only while the rest of the map fits), prints
each mark's verdict and the reported comparisons, and writes
DIR/excuse.csv (default DIR: runs/excuse).";

type Error = Box<dyn std::error::Error>;

const ARMS: [(&str, Excuse); 3] = [
    ("A", Excuse::Always),
    ("X", Excuse::Never),
    ("N", Excuse::WhileFit),
];
const A: usize = 0;
const X: usize = 1;
const N: usize = 2;

/// Per-seed measures, in CSV column order. The first three are judged raw,
/// the life measures by typical % change.
const MEASURES: [Measure; 7] = [
    ("strangers_home", |l| f64::from(l.strangers_home)),
    ("homes_stranger", |l| f64::from(l.home.0 - l.strangers_home)),
    ("home_wrong", |l| f64::from(l.home.0)),
    ("bush_sleeps", |l| f64::from(l.bush.sleeps)),
    ("barren_sleeps_per_1k", |l| f64::from(l.barren.sleeps) / 2.0),
    ("raised_bush_sleeps", |l| f64::from(l.raised_sleeps(false))),
    ("raised_barren_sleeps", |l| f64::from(l.raised_sleeps(true))),
];
const HOME: usize = 3;

/// Most a mark-4 measure may rise, as a typical change.
const NO_WORSE: f64 = 0.1;
/// Mark 2: most extra homes taken as strangers per test.
const KEEP_HOME: f64 = 0.10;
/// Mark 3, Nano's hard limit: most extra wrong answers per test.
const HARD_LIMIT: f64 = 0.25;

/// One seed and its life under each arm.
type Row = (u64, Vec<Life>);

struct Args {
    seeds: (u64, u64),
    out: PathBuf,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut parsed = Args {
        seeds: (501, 700),
        out: PathBuf::from("runs/excuse"),
    };
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or(format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--seeds" => parsed.seeds = range(value)?,
            "--out" => parsed.out = PathBuf::from(value),
            _ => return Err(format!("unknown flag: {flag}")),
        }
    }
    Ok(parsed)
}

fn range(value: &str) -> Result<(u64, u64), String> {
    let bad = || format!("bad --seeds: {value}");
    let (a, b) = value.split_once('-').ok_or_else(bad)?;
    let (a, b): (u64, u64) = (a.parse().map_err(|_| bad())?, b.parse().map_err(|_| bad())?);
    if a > b {
        return Err(bad());
    }
    Ok((a, b))
}

fn column(rows: &[Row], arm: usize, f: fn(&Life) -> f64) -> Vec<f64> {
    rows.iter().map(|r| f(&r.1[arm])).collect()
}

fn mean(xs: &[f64]) -> f64 {
    xs.iter().sum::<f64>() / xs.len() as f64
}

/// Paired raw differences `test - base`, seed by seed.
fn diffs(base: &[f64], test: &[f64]) -> Vec<f64> {
    test.iter().zip(base).map(|(t, b)| t - b).collect()
}

fn raw(i: &Interval) -> String {
    format!("{:+.3} [{:+.3}, {:+.3}]", i.mid, i.lo, i.hi)
}

fn pct(i: &Interval) -> String {
    format!(
        "{:+.0}% [{:+.0}%, {:+.0}%]",
        100.0 * i.mid,
        100.0 * i.lo,
        100.0 * i.hi
    )
}

/// One comparison of `f` from arm `base` to arm `test`: raw, then %.
fn compare(
    rows: &[Row],
    base: usize,
    test: usize,
    f: fn(&Life) -> f64,
) -> Result<(Interval, Interval), Error> {
    let (b, t) = (column(rows, base, f), column(rows, test, f));
    Ok((Interval::mean(&diffs(&b, &t))?, Interval::change(&b, &t)?))
}

fn marks(rows: &[Row]) -> Result<(), Error> {
    let strangers = |arm| column(rows, arm, MEASURES[0].1);
    let (a, x, n) = (strangers(A), strangers(X), strangers(N));

    let door = Interval::mean(&diffs(&a, &x))?;
    println!(
        "0. door reproduces, X - A strangers taken as home: {}: {}",
        raw(&door),
        door.at_most(0.0)
    );

    let gap: Vec<f64> = (0..a.len())
        .map(|i| (n[i] - a[i]) - 0.5 * (x[i] - a[i]))
        .collect();
    let gap = Interval::mean(&gap)?;
    println!(
        "1. N closes half the gap, (N-A) - (X-A)/2: {}: {}",
        raw(&gap),
        gap.at_most(0.0)
    );

    let (keep, keep_pct) = compare(rows, A, N, MEASURES[1].1)?;
    println!(
        "2. N keeps home, N - A homes taken as stranger: {} ({}): {}",
        raw(&keep),
        pct(&keep_pct),
        keep.at_most(KEEP_HOME)
    );

    let (wrong, wrong_pct) = compare(rows, A, N, MEASURES[2].1)?;
    println!(
        "3. hard limit, N - A total wrong: {} ({}): {}",
        raw(&wrong),
        pct(&wrong_pct),
        wrong.at_most(HARD_LIMIT)
    );

    println!("4. no regressions, N vs A, each at most +10%:");
    for (name, f) in &MEASURES[HOME..] {
        let (r, c) = compare(rows, A, N, *f)?;
        println!(
            "   {name} {}, raw {}: {}",
            pct(&c),
            raw(&r),
            c.at_most(NO_WORSE)
        );
    }
    Ok(())
}

fn reported(rows: &[Row]) -> Result<(), Error> {
    for (label, base, test) in [("N vs X", X, N), ("X vs A", A, X), ("N vs A", A, N)] {
        println!("Reported only, {label}:");
        for (name, f) in MEASURES {
            let (r, c) = compare(rows, base, test, f)?;
            println!("   {name} raw {} ({})", raw(&r), pct(&c));
        }
    }
    println!("Means (A, X, N):");
    for (name, f) in MEASURES {
        let m: Vec<String> = (0..ARMS.len())
            .map(|arm| format!("{:.3}", mean(&column(rows, arm, f))))
            .collect();
        println!("   {name}: {}", m.join(", "));
    }
    Ok(())
}

fn csv(rows: &[Row]) -> String {
    let heads: Vec<&str> = MEASURES.iter().map(|m| m.0).collect();
    let mut out = format!("seed,arm,{}\n", heads.join(","));
    for (seed, lives) in rows {
        for ((name, _), l) in ARMS.iter().zip(lives) {
            let values: Vec<String> = MEASURES.iter().map(|m| m.1(l).to_string()).collect();
            out.push_str(&format!("{seed},{name},{}\n", values.join(",")));
        }
    }
    out
}

fn execute(args: &Args) -> Result<(), Error> {
    let seeds: Vec<u64> = (args.seeds.0..=args.seeds.1).collect();
    let settings: Vec<Recall> = ARMS
        .iter()
        .map(|&(_, excuse)| Recall {
            excuse,
            ..Recall::default()
        })
        .collect();
    let rows = lives_under(&seeds, &settings)?;
    println!("seeds {}-{}", args.seeds.0, args.seeds.1);
    marks(&rows)?;
    reported(&rows)?;
    fs::create_dir_all(&args.out)?;
    fs::write(args.out.join("excuse.csv"), csv(&rows))?;
    Ok(())
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let args = match parse(&raw) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = execute(&args) {
        eprintln!("excuse: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
