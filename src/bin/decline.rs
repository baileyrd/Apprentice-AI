//! Experience decline: which learned part makes barren worlds harder? Each
//! seed is raised barren-first under today's default, then one part is
//! wiped at the start of world 5 (a new barren world) and the rest of the
//! world is lived. Every arm is compared with a newborn in the same world.
//! The comparison is in docs/decline/README.md.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::thread;

use apprentice::apprentice::{Apprentice, Drives};
use apprentice::judge::Interval;
use apprentice::life::{BARREN_STEPS, Setting, Stay, barren, barren_test, raise};
use apprentice::memory::{Memory, Recall};
use apprentice::model::Rates;

const USAGE: &str = "\
usage: decline [--seeds N] [--from SEED] [--out DIR]

Seeds FROM..FROM+N-1 (default 101..140, tuning seeds). Lives world 5 of a
barren-first life seven ways (E experienced, P patterns wiped, M maps
wiped, V own values wiped, PV patterns and values wiped, F all three wiped,
N newborn), prints each arm against the newborn and writes DIR/decline.csv (default DIR: runs/decline).";

type Error = Box<dyn std::error::Error>;

/// What is wiped at the start of world 5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arm {
    Experienced,
    Patterns,
    Maps,
    Values,
    PatternsValues,
    Full,
    Newborn,
}

const ARMS: [Arm; 7] = [
    Arm::Experienced,
    Arm::Patterns,
    Arm::Maps,
    Arm::Values,
    Arm::PatternsValues,
    Arm::Full,
    Arm::Newborn,
];

impl Arm {
    fn name(self) -> &'static str {
        match self {
            Arm::Experienced => "E",
            Arm::Patterns => "P",
            Arm::Maps => "M",
            Arm::Values => "V",
            Arm::PatternsValues => "PV",
            Arm::Full => "F",
            Arm::Newborn => "N",
        }
    }
}

/// Parsed flags.
struct Args {
    seeds: Vec<u64>,
    out: PathBuf,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let (mut count, mut from) = (40, 101);
    let mut out = PathBuf::from("runs/decline");
    let mut it = args.iter();
    while let Some(flag) = it.next() {
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
    Ok(Args {
        seeds: (from..from + count).collect(),
        out,
    })
}

/// World 5 of `seed`'s barren-first life, with `arm` wiped first.
fn world_five(seed: u64, arm: Arm) -> Result<Stay, Error> {
    let setting = Setting::from(Recall::default());
    if arm == Arm::Newborn {
        let memory = Memory::new(Rates::default(), setting.recall);
        let mut mind = Apprentice::with_model(Drives::default(), memory, seed);
        return Ok(barren_test(&mut mind, seed)?);
    }
    let (mut mind, _) = raise(setting, seed, &barren())?;
    if matches!(arm, Arm::Patterns | Arm::PatternsValues | Arm::Full) {
        mind.model_mut().forget_patterns();
    }
    if matches!(arm, Arm::Maps | Arm::Full) {
        mind.model_mut().forget_maps(Rates::default());
    }
    if matches!(arm, Arm::Values | Arm::PatternsValues | Arm::Full) {
        mind.forget_values();
    }
    Ok(barren_test(&mut mind, seed)?)
}

/// One seed and world 5 under each arm, in `ARMS` order.
type Row = (u64, Vec<Stay>);

fn rows(seeds: &[u64]) -> Result<Vec<Row>, Error> {
    let workers = thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = seeds.len().div_ceil(workers).max(1);
    let one = |seed: u64| -> Result<Row, String> {
        let stays: Result<Vec<Stay>, Error> = ARMS.iter().map(|&a| world_five(seed, a)).collect();
        stays
            .map(|s| (seed, s))
            .map_err(|e| format!("seed {seed}: {e}"))
    };
    let parts: Vec<Result<Vec<Row>, String>> = thread::scope(|scope| {
        let handles: Vec<_> = seeds
            .chunks(chunk)
            .map(|part| scope.spawn(move || part.iter().map(|&s| one(s)).collect()))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or(Err("a worker panicked".into())))
            .collect()
    });
    let mut all = Vec::new();
    for part in parts {
        all.extend(part?);
    }
    Ok(all)
}

/// A per-1,000-steps measure of a world-5 stay.
type Measure = (&'static str, fn(&Stay) -> f64);

const PER_K: f64 = 1_000.0 / BARREN_STEPS as f64;
const MEASURES: [Measure; 3] = [
    ("sleeps_per_1k", |s| f64::from(s.sleeps) * PER_K),
    ("meals_per_1k", |s| f64::from(s.meals) * PER_K),
    ("hopes", |s| f64::from(s.hopes)),
];

fn column(rows: &[Row], arm: usize, f: fn(&Stay) -> f64) -> Vec<f64> {
    rows.iter().map(|r| f(&r.1[arm])).collect()
}

fn mean(xs: &[f64]) -> f64 {
    xs.iter().sum::<f64>() / xs.len() as f64
}

fn csv(rows: &[Row]) -> String {
    let names: Vec<&str> = MEASURES.iter().map(|m| m.0).collect();
    let mut out = format!("seed,arm,{}\n", names.join(","));
    for (seed, stays) in rows {
        for (arm, stay) in ARMS.iter().zip(stays) {
            let values: Vec<String> = MEASURES.iter().map(|m| m.1(stay).to_string()).collect();
            out.push_str(&format!("{seed},{},{}\n", arm.name(), values.join(",")));
        }
    }
    out
}

/// Each arm against the newborn on `measure`: means, paired change, raw
/// difference, and (for sleeps) the share of E's gap it closes.
fn report(rows: &[Row], measure: Measure) -> Result<(), Error> {
    let (name, f) = measure;
    let newborn_at = ARMS.len() - 1;
    let newborn = column(rows, newborn_at, f);
    let gap = mean(&column(rows, 0, f)) - mean(&newborn);
    let own = Interval::mean(&newborn)?;
    println!(
        "{name} (newborn {:.2} [{:.2}, {:.2}]):",
        own.mid, own.lo, own.hi
    );
    for (i, arm) in ARMS.iter().enumerate().take(newborn_at) {
        let xs = column(rows, i, f);
        let change = Interval::change(&newborn, &xs)?;
        let diffs: Vec<f64> = xs.iter().zip(&newborn).map(|(x, n)| x - n).collect();
        let raw = Interval::mean(&diffs)?;
        let closed = (mean(&column(rows, 0, f)) - mean(&xs)) / gap;
        let own = Interval::mean(&xs)?;
        println!(
            "   {} {:.2} [{:.2}, {:.2}]: {:+.0}% [{:+.0}%, {:+.0}%], raw {:+.2} [{:+.2}, {:+.2}], gap closed {:.0}%",
            arm.name(),
            own.mid,
            own.lo,
            own.hi,
            100.0 * change.mid,
            100.0 * change.lo,
            100.0 * change.hi,
            raw.mid,
            raw.lo,
            raw.hi,
            100.0 * closed
        );
    }
    Ok(())
}

fn execute(args: &Args) -> Result<(), Error> {
    let rows = rows(&args.seeds)?;
    for measure in MEASURES {
        report(&rows, measure)?;
    }
    fs::create_dir_all(&args.out)?;
    fs::write(args.out.join("decline.csv"), csv(&rows))?;
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
            eprintln!("{e}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match execute(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("decline: {e}");
            ExitCode::FAILURE
        }
    }
}
