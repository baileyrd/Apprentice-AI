//! Experience decline: which learned part makes barren worlds harder? Each
//! seed is raised barren-first under today's default, then one part is
//! wiped at the start of world 5 (a new barren world) and the rest of the
//! world is lived. Every arm is compared with a newborn in the same world.
//! The comparison is in docs/decline/README.md.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::thread;

use apprentice::apprentice::{Apprentice, Drives};
use apprentice::judge::Interval;
use apprentice::life::{BARREN_STEPS, Learner, Setting, Stay, barren, barren_test_watched, raise};
use apprentice::memory::{Memory, Recall};
use apprentice::model::Rates;
use apprentice::model::{WorldModel, kind};
use apprentice::sim::{Event, Outcome, Sim};
use apprentice::world::{Cell, Pos};

const USAGE: &str = "\
usage: decline [--seeds N] [--from SEED] [--out DIR]

Seeds FROM..FROM+N-1 (default 101..140, tuning seeds). Lives world 5 of a
barren-first life seven ways (E experienced, P patterns wiped, M maps
wiped, V own values wiped, PV patterns and values wiped, F all three wiped,
N newborn), prints each arm against the newborn and the calibration of eaten squares by time since last seen,
and writes DIR/decline.csv and DIR/calibration.csv (default DIR: runs/decline).";

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

/// Upper ends of the bins of steps since an eaten square was last in view:
/// 1–7, 8–31, 32–127, 128+.
const BINS: [u64; 4] = [7, 31, 127, u64::MAX];
const BIN_NAMES: [&str; 4] = ["1-7", "8-31", "32-127", "128+"];

/// One bin: sightings, summed predicted chance of food, sightings with food.
#[derive(Debug, Default, Clone, Copy)]
struct Bin {
    seen: u32,
    predicted: f64,
    food: u32,
}

/// Predicted against actual food on eaten squares as they come back into
/// view, by steps since last in view.
#[derive(Debug, Default, Clone)]
struct Calibration {
    bins: [Bin; 4],
    t: u64,
    eaten: HashSet<Pos>,
    last: HashMap<Pos, u64>,
    view: HashSet<Pos>,
}

impl Calibration {
    /// Called after each step, before the new view is sensed.
    fn watch(&mut self, mind: &Learner, sim: &Sim, outcome: &Outcome, here: Pos) {
        if outcome.event == Event::Ate {
            self.eaten.insert(here);
        }
        self.t += 1;
        let start = sim.start();
        let view: HashSet<Pos> = (-1..=1)
            .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
            .map(|(dx, dy)| sim.pos().shift(dx, dy))
            .collect();
        for &at in &view {
            let back = self.eaten.contains(&at) && !self.view.contains(&at);
            if let (true, Some(&last)) = (back, self.last.get(&at)) {
                let belief = mind.model().predict(at.shift(-start.x, -start.y));
                let i = BINS
                    .iter()
                    .position(|&hi| self.t - last <= hi)
                    .unwrap_or(BINS.len() - 1);
                let bin = &mut self.bins[i];
                bin.seen += 1;
                bin.predicted += belief[kind(Cell::Food)];
                bin.food += u32::from(sim.grid().get(at) == Cell::Food);
            }
        }
        for &at in &view {
            self.last.insert(at, self.t);
        }
        self.view = view;
    }
}

/// World 5 of `seed`'s barren-first life, with `arm` wiped first.
fn world_five(seed: u64, arm: Arm) -> Result<(Stay, [Bin; 4]), Error> {
    let setting = Setting::from(Recall::default());
    let mut mind = if arm == Arm::Newborn {
        let memory = Memory::new(Rates::default(), setting.recall);
        Apprentice::with_model(Drives::default(), memory, seed)
    } else {
        raise(setting, seed, &barren())?.0
    };
    if matches!(arm, Arm::Patterns | Arm::PatternsValues | Arm::Full) {
        mind.model_mut().forget_patterns();
    }
    if matches!(arm, Arm::Maps | Arm::Full) {
        mind.model_mut().forget_maps(Rates::default());
    }
    if matches!(arm, Arm::Values | Arm::PatternsValues | Arm::Full) {
        mind.forget_values();
    }
    let mut calibration = Calibration::default();
    let stay = barren_test_watched(&mut mind, seed, |m, s, o, h| calibration.watch(m, s, o, h))?;
    Ok((stay, calibration.bins))
}

/// One seed and world 5 under each arm, in `ARMS` order.
type Row = (u64, Vec<Stay>, Vec<[Bin; 4]>);

fn rows(seeds: &[u64]) -> Result<Vec<Row>, Error> {
    let workers = thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = seeds.len().div_ceil(workers).max(1);
    let one = |seed: u64| -> Result<Row, String> {
        let worlds: Result<Vec<_>, Error> = ARMS.iter().map(|&a| world_five(seed, a)).collect();
        let (stays, bins) = worlds
            .map_err(|e| format!("seed {seed}: {e}"))?
            .into_iter()
            .unzip();
        Ok((seed, stays, bins))
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
    for (seed, stays, _) in rows {
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

/// Per arm and bin: sightings, pooled predicted and actual chance of food,
/// and the per-seed bias (predicted − actual) with its 90% interval.
fn report_calibration(rows: &[Row]) -> Result<(), Error> {
    println!("calibration on eaten squares back in view, by steps since last in view:");
    for (a, arm) in ARMS.iter().enumerate() {
        println!("   {}:", arm.name());
        for (b, name) in BIN_NAMES.iter().enumerate() {
            let bins: Vec<Bin> = rows.iter().map(|r| r.2[a][b]).collect();
            let seen: u32 = bins.iter().map(|x| x.seen).sum();
            if seen == 0 {
                println!("      {name}: no sightings");
                continue;
            }
            let predicted = bins.iter().map(|x| x.predicted).sum::<f64>() / f64::from(seen);
            let food = f64::from(bins.iter().map(|x| x.food).sum::<u32>()) / f64::from(seen);
            let biases: Vec<f64> = bins
                .iter()
                .filter(|x| x.seen > 0)
                .map(|x| (x.predicted - f64::from(x.food)) / f64::from(x.seen))
                .collect();
            let bias = match Interval::mean(&biases) {
                Ok(i) => format!(
                    "{:+.3} [{:+.3}, {:+.3}] over {} seeds",
                    i.mid,
                    i.lo,
                    i.hi,
                    biases.len()
                ),
                Err(_) => format!("too few seeds ({})", biases.len()),
            };
            println!(
                "      {name}: {seen} sightings, predicted {predicted:.3}, actual {food:.3}, bias {bias}"
            );
        }
    }
    Ok(())
}

fn calibration_csv(rows: &[Row]) -> String {
    let mut out = String::from("seed,arm,bin,seen,predicted_sum,food\n");
    for (seed, _, bins) in rows {
        for (arm, per_bin) in ARMS.iter().zip(bins) {
            for (name, b) in BIN_NAMES.iter().zip(per_bin) {
                out.push_str(&format!(
                    "{seed},{},{name},{},{},{}\n",
                    arm.name(),
                    b.seen,
                    b.predicted,
                    b.food
                ));
            }
        }
    }
    out
}

fn execute(args: &Args) -> Result<(), Error> {
    let rows = rows(&args.seeds)?;
    for measure in MEASURES {
        report(&rows, measure)?;
    }
    report_calibration(&rows)?;
    fs::create_dir_all(&args.out)?;
    fs::write(args.out.join("decline.csv"), csv(&rows))?;
    fs::write(args.out.join("calibration.csv"), calibration_csv(&rows))?;
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
