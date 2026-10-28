//! Transfer experiment: does the pattern layer carry what the apprentice
//! learned in earlier worlds to a world it has never seen?
//!
//! Each apprentice lives 2,000 steps in each of 3 bush worlds, then 4,000
//! steps in a fresh one, with patterns on or off. Separately, it lives 8,000
//! steps in one bush world, to check the layer costs nothing at home.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use apprentice::adaptation::{Change, Setup};
use apprentice::apprentice::{Apprentice, Drives, Mind};
use apprentice::memory::{Memory, Recall};
use apprentice::metrics::Metrics;
use apprentice::model::{Rates, WorldModel, kind};
use apprentice::sim::{Event, Sim, SimError};
use apprentice::world::{Cell, Pos};

const USAGE: &str = "\
usage: transfer [--seeds N] [--from SEED] [--out DIR]

Seeds FROM..FROM+N-1 (default 1..10). With patterns on and off, prints how
far it anticipated a regrown bush in a fresh world, how long its surprise took
to settle there, and its sleeps, and writes DIR/transfer.csv (default DIR:
runs/transfer).";

type Error = Box<dyn std::error::Error>;

const EARLIER: u64 = 3;
const EARLIER_STEPS: usize = 2_000;
const FRESH_STEPS: usize = 4_000;
const HOME_STEPS: usize = 8_000;
/// Trailing window for surprise, and the share above the old level that
/// counts as settled.
const TRAIL: usize = 100;
const SETTLED: f64 = 1.5;
const MINDS: [&str; 2] = ["without patterns", "with patterns"];

type Learner = Apprentice<Memory>;

fn learner(patterns: bool, seed: u64) -> Learner {
    let recall = Recall {
        patterns,
        ..Recall::default()
    };
    Apprentice::with_model(
        Drives::default(),
        Memory::new(Rates::default(), recall),
        seed,
    )
}

fn bush_world(seed: u64) -> Result<Sim, SimError> {
    Sim::new(Setup::for_change(Change::Food).config, seed)
}

/// What one fresh world showed.
struct Fresh {
    /// Chance of food it gave the first regrown bush it had eaten from, just
    /// before seeing it.
    anticipation: Option<f64>,
    /// Steps until its surprise settled; `None` if it never did.
    settle: Option<usize>,
    sleeps: u64,
}

/// One step; returns the surprise and the outcome's event.
fn step(sim: &mut Sim, mind: &mut Learner) -> (f64, Event) {
    let outcome = sim.step(mind.act(&sim.observe()));
    mind.feel(&outcome);
    (mind.surprise(), outcome.event)
}

/// Whether `p` is in the 3×3 view from `at`.
fn in_view(at: Pos, p: Pos) -> bool {
    (p.x - at.x).abs() <= 1 && (p.y - at.y).abs() <= 1
}

/// Lives the earlier worlds; returns its mean surprise at the end of the last.
fn earlier(mind: &mut Learner, seed: u64) -> Result<f64, SimError> {
    let mut tail = Vec::new();
    for i in 0..EARLIER {
        let mut sim = bush_world(seed * 10 + i)?;
        tail = (0..EARLIER_STEPS).map(|_| step(&mut sim, mind).0).collect();
    }
    let last = &tail[tail.len() - 500..];
    Ok(last.iter().sum::<f64>() / last.len() as f64)
}

fn fresh(mind: &mut Learner, seed: u64, floor: f64) -> Result<Fresh, SimError> {
    let mut sim = bush_world(1_000 + seed)?;
    let mut metrics = Metrics::default();
    let mut eaten: HashSet<Pos> = HashSet::new();
    let (mut anticipation, mut settle) = (None, None);
    let mut surprise = Vec::with_capacity(FRESH_STEPS);
    for t in 0..FRESH_STEPS {
        if anticipation.is_none() {
            let start = sim.start();
            let regrown = eaten
                .iter()
                .find(|&&p| sim.grid().get(p) == Cell::Food && in_view(sim.pos(), p));
            if let Some(p) = regrown {
                let rel = p.shift(-start.x, -start.y);
                anticipation = Some(mind.model().predict(rel)[kind(Cell::Food)]);
            }
        }
        let here = sim.pos();
        let outcome = sim.step(mind.act(&sim.observe()));
        mind.feel(&outcome);
        metrics.record(&outcome);
        if outcome.event == Event::Ate {
            eaten.insert(here);
        }
        surprise.push(mind.surprise());
        if settle.is_none() && t + 1 >= TRAIL {
            let mean = surprise[t + 1 - TRAIL..].iter().sum::<f64>() / TRAIL as f64;
            if mean <= SETTLED * floor {
                settle = Some(t + 1);
            }
        }
    }
    Ok(Fresh {
        anticipation,
        settle,
        sleeps: metrics.summary().sleeps,
    })
}

fn home(patterns: bool, seed: u64) -> Result<u64, SimError> {
    let mut mind = learner(patterns, seed);
    let mut sim = bush_world(seed)?;
    let mut metrics = Metrics::default();
    for _ in 0..HOME_STEPS {
        let outcome = sim.step(mind.act(&sim.observe()));
        mind.feel(&outcome);
        metrics.record(&outcome);
    }
    Ok(metrics.summary().sleeps)
}

fn median(mut xs: Vec<f64>) -> f64 {
    xs.sort_by(f64::total_cmp);
    let n = xs.len();
    match n {
        0 => f64::NAN,
        _ if n % 2 == 1 => xs[n / 2],
        _ => (xs[n / 2 - 1] + xs[n / 2]) / 2.0,
    }
}

/// Parsed flags: seed count, first seed, output directory.
fn parse(args: &[String]) -> Result<(u64, u64, PathBuf), String> {
    let (mut seeds, mut from, mut out) = (10, 1, PathBuf::from("runs/transfer"));
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or(format!("{flag} needs a value"))?;
        let number = || value.parse().map_err(|_| format!("bad {flag}: {value}"));
        match flag.as_str() {
            "--seeds" => seeds = number()?,
            "--from" => from = number()?,
            "--out" => out = PathBuf::from(value),
            _ => return Err(format!("unknown flag: {flag}")),
        }
    }
    if seeds == 0 {
        return Err("--seeds must be positive".into());
    }
    Ok((seeds, from, out))
}

fn execute(seeds: u64, from: u64, out: &PathBuf) -> Result<(), Error> {
    let mut csv = String::from("mind,seed,anticipation,settle,fresh_sleeps,home_sleeps\n");
    let show = |x: Option<f64>| x.map_or(String::new(), |v| format!("{v:.3}"));
    for (m, name) in MINDS.into_iter().enumerate() {
        let patterns = m == 1;
        let (mut ant, mut settle, mut fresh_sleeps, mut home_sleeps) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for seed in from..from + seeds {
            let mut mind = learner(patterns, seed);
            let floor = earlier(&mut mind, seed)?;
            let f = fresh(&mut mind, seed, floor)?;
            let h = home(patterns, seed)?;
            csv.push_str(&format!(
                "{name},{seed},{},{},{},{h}\n",
                show(f.anticipation),
                f.settle.map_or(String::new(), |s| s.to_string()),
                f.sleeps
            ));
            ant.extend(f.anticipation);
            // A life that never settled counts as the whole stay.
            settle.push(f.settle.unwrap_or(FRESH_STEPS) as f64);
            fresh_sleeps.push(f.sleeps as f64);
            home_sleeps.push(h as f64);
        }
        println!(
            "{name:>16}: anticipation {:.2} ({} seeds saw a regrown bush), settle {:.0} steps, fresh-world sleeps {:.1}, home sleeps {:.1}",
            median(ant.clone()),
            ant.len(),
            median(settle),
            median(fresh_sleeps),
            median(home_sleeps),
        );
    }
    fs::create_dir_all(out)?;
    fs::write(out.join("transfer.csv"), csv)?;
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let (seeds, from, out) = match parse(&args) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("{e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = execute(seeds, from, &out) {
        eprintln!("transfer: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
