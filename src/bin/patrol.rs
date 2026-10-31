//! The hand-coded patroller in the test worlds of the shared life, beside
//! the energy each world type charges and pays. Reported next to the
//! revisit judging, never as a mark (docs/revisit/README.md).

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use apprentice::judge::Interval;
use apprentice::life::{BARREN_STEPS, BUSH_STEPS, barren, bushes, test_offset};
use apprentice::patroller::Patroller;
use apprentice::sim::{Config, Event, Sim, SimError};

const USAGE: &str = "\
usage: patrol [--seek] [--against CSV] [--out DIR]

Lives the patroller in the barren and bush test worlds of seeds 1-100 and
141-240. Prints energy per step and per meal for each world type, the
patroller's sleeps and meals per 1,000 steps and, with --against (a
revisit.csv from `revisit --judge`), the paired raw difference to each of
its settings. --seek: the patroller also walks to food in view. Writes
DIR/patrol.csv (default DIR: runs/patrol).";

type Error = Box<dyn std::error::Error>;

/// Sleeps and meals per 1,000 steps in one world.
#[derive(Debug, Clone, Copy)]
struct Rates {
    sleeps: f64,
    meals: f64,
}

fn patrol(config: Config, seed: u64, steps: usize, seek: bool) -> Result<Rates, SimError> {
    let mut sim = Sim::new(config, seed)?;
    let mut p = if seek {
        Patroller::seeking()
    } else {
        Patroller::new()
    };
    let (mut sleeps, mut meals) = (0u32, 0u32);
    for _ in 0..steps {
        let outcome = sim.step(p.act(&sim.observe()));
        sleeps += u32::from(outcome.slept);
        meals += u32::from(outcome.event == Event::Ate);
    }
    let per_k = 1_000.0 / steps as f64;
    Ok(Rates {
        sleeps: f64::from(sleeps) * per_k,
        meals: f64::from(meals) * per_k,
    })
}

/// What a world type charges and pays, and the meals per 1,000 steps that
/// pay for walking every step.
fn energy(name: &str, config: &Config) {
    let p = &config.physiology;
    let rest = p.basal_cost;
    let walk = p.basal_cost + p.move_cost;
    let break_even = 1_000.0 * f64::from(walk) / f64::from(p.food_value);
    println!(
        "{name}: a step costs {rest} resting or eating, {walk} walking ({} into a hazard); a meal gives {}; walking every step breaks even at {break_even:.0} meals per 1,000 steps",
        walk + p.hazard_cost,
        p.food_value
    );
}

/// Per seed: barren and bush rates.
type Row = (u64, Rates, Rates);

fn rows(seeds: &[u64], seek: bool) -> Result<Vec<Row>, SimError> {
    let (dry, wet) = (barren(), bushes());
    seeds
        .iter()
        .map(|&s| {
            let b = patrol(dry.clone(), s + test_offset(&dry), BARREN_STEPS, seek)?;
            let w = patrol(wet.clone(), s + test_offset(&wet), BUSH_STEPS, seek)?;
            Ok((s, b, w))
        })
        .collect()
}

fn mean_line(name: &str, xs: &[f64]) -> Result<String, Error> {
    let i = Interval::mean(xs)?;
    Ok(format!("{name} {:.2} [{:.2}, {:.2}]", i.mid, i.lo, i.hi))
}

/// Per-seed values of `column` for `setting` in a revisit.csv, by seed.
fn against(text: &str, setting: &str, column: &str) -> Result<Vec<(u64, f64)>, Error> {
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().ok_or("empty csv")?.split(',').collect();
    let at = head
        .iter()
        .position(|h| *h == column)
        .ok_or(format!("no column {column}"))?;
    let mut out = Vec::new();
    for line in lines {
        let cells: Vec<&str> = line.split(',').collect();
        if cells.get(1) == Some(&setting) {
            out.push((
                cells[0].parse()?,
                cells.get(at).ok_or("short row")?.parse()?,
            ));
        }
    }
    Ok(out)
}

/// A revisit.csv column, the scale to per 1,000 steps, and the patroller's
/// own value.
type Column = (&'static str, f64, fn(&Row) -> f64);

/// Patroller minus each revisit setting, paired by seed.
fn compare(rows: &[Row], csv: &str) -> Result<(), Error> {
    // revisit.csv has barren rates per 1,000 steps, bush counts per world.
    let measures: [Column; 4] = [
        ("barren_sleeps_per_1k", 1.0, |r| r.1.sleeps),
        ("barren_meals_per_1k", 1.0, |r| r.1.meals),
        ("bush_sleeps", 0.25, |r| r.2.sleeps),
        ("bush_meals", 0.25, |r| r.2.meals),
    ];
    let settings: Vec<String> = csv
        .lines()
        .skip(1)
        .filter_map(|l| l.split(',').nth(1).map(String::from))
        .fold(Vec::new(), |mut v, s| {
            if !v.contains(&s) {
                v.push(s);
            }
            v
        });
    for setting in &settings {
        println!("patroller minus {setting}, per 1,000 steps, paired by seed:");
        for (column, scale, mine) in measures {
            let theirs = against(csv, setting, column)?;
            let diffs: Vec<f64> = theirs
                .iter()
                .filter_map(|&(seed, v)| {
                    rows.iter()
                        .find(|r| r.0 == seed)
                        .map(|r| mine(r) - v * scale)
                })
                .collect();
            let i = Interval::mean(&diffs)?;
            let theirs_mean = theirs.iter().map(|t| t.1 * scale).sum::<f64>() / theirs.len() as f64;
            println!(
                "   {column}: {setting} {theirs_mean:.2}, difference {:+.2} [{:+.2}, {:+.2}] over {} seeds",
                i.mid,
                i.lo,
                i.hi,
                diffs.len()
            );
        }
    }
    Ok(())
}

fn execute(against_csv: Option<PathBuf>, out: PathBuf, seek: bool) -> Result<(), Error> {
    energy("barren", &barren());
    energy("bush", &bushes());
    let seeds: Vec<u64> = (1..=100).chain(141..=240).collect();
    let rows = rows(&seeds, seek)?;
    let col = |f: fn(&Row) -> f64| rows.iter().map(f).collect::<Vec<f64>>();
    println!("patroller, per 1,000 steps (mean [90%]):");
    println!(
        "   barren: {}, {}",
        mean_line("sleeps", &col(|r| r.1.sleeps))?,
        mean_line("meals", &col(|r| r.1.meals))?
    );
    println!(
        "   bush: {}, {}",
        mean_line("sleeps", &col(|r| r.2.sleeps))?,
        mean_line("meals", &col(|r| r.2.meals))?
    );
    if let Some(path) = against_csv {
        compare(&rows, &fs::read_to_string(path)?)?;
    }
    let mut csv = String::from(
        "seed,barren_sleeps_per_1k,barren_meals_per_1k,bush_sleeps_per_1k,bush_meals_per_1k\n",
    );
    for (seed, b, w) in &rows {
        csv.push_str(&format!(
            "{seed},{},{},{},{}\n",
            b.sleeps, b.meals, w.sleeps, w.meals
        ));
    }
    fs::create_dir_all(&out)?;
    fs::write(out.join("patrol.csv"), csv)?;
    Ok(())
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let (mut against_csv, mut out) = (None, PathBuf::from("runs/patrol"));
    let seek = raw.iter().any(|a| a == "--seek");
    let mut it = raw.iter().filter(|a| *a != "--seek");
    while let Some(flag) = it.next() {
        let Some(value) = it.next() else {
            eprintln!("{flag} needs a value\n\n{USAGE}");
            return ExitCode::from(2);
        };
        match flag.as_str() {
            "--against" => against_csv = Some(PathBuf::from(value)),
            "--out" => out = PathBuf::from(value),
            _ => {
                eprintln!("unknown flag: {flag}\n\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    if let Err(e) = execute(against_csv, out, seek) {
        eprintln!("patrol: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
