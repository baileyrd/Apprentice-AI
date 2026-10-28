//! Energy experiment: why the apprentice runs out, and whether the fix holds.
//!
//! Each mind lives 8,000 steps in the bush world (food grows back in place),
//! seeds 1..=N. A reference agent that sees the whole real grid and walks to
//! the nearest food shows what the world allows.

use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use apprentice::adaptation::{Change, Setup};
use apprentice::apprentice::{Apprentice, Drives, Mind};
use apprentice::body::Action;
use apprentice::memory::{Memory, Recall};
use apprentice::metrics::{Metrics, Summary};
use apprentice::model::Rates;
use apprentice::sim::{Sim, SimError};
use apprentice::world::{Cell, Dir, Pos};

const USAGE: &str = "\
usage: energy [--seeds N] [--out DIR]

Lives seeds 1..=N (default 10) for 8,000 steps in the bush world with five
minds: a reference that sees the real grid, the phase 4 apprentice, that
apprentice made patient, that apprentice replaying its meals in sleep, and
both changes together. Prints sleeps, meals, mean energy and the share
of steps spent hungry, and writes DIR/energy.csv (default DIR: runs/energy).";

type Error = Box<dyn std::error::Error>;

const STEPS: usize = 8_000;
/// Energy below which a step counts as hungry.
const HUNGRY: u32 = 50;
const MINDS: [&str; 5] = [
    "sees the map",
    "phase 4",
    "patient",
    "meal replay",
    "patient + meal replay",
];

/// First step toward the nearest food on the real grid, around walls and
/// hazards; eats when on food, rests when no food can be reached.
fn nearest_food(sim: &Sim) -> Action {
    let grid = sim.grid();
    if grid.get(sim.pos()) == Cell::Food {
        return Action::Eat;
    }
    let open = |p: Pos| matches!(grid.get(p), Cell::Empty | Cell::Food);
    let mut first: HashMap<Pos, Dir> = HashMap::new();
    let mut queue = VecDeque::new();
    for dir in Dir::ALL {
        let p = sim.pos().step(dir);
        if open(p) {
            first.insert(p, dir);
            queue.push_back(p);
        }
    }
    while let Some(p) = queue.pop_front() {
        let dir = first[&p];
        if grid.get(p) == Cell::Food {
            return Action::Move(dir);
        }
        for d in Dir::ALL {
            let n = p.step(d);
            if open(n) && !first.contains_key(&n) {
                first.insert(n, dir);
                queue.push_back(n);
            }
        }
    }
    Action::Rest
}

/// One life's numbers.
struct Life {
    summary: Summary,
    hungry: f64,
}

/// Lives one life, choosing each action with `choose`.
fn live(
    seed: u64,
    mut choose: impl FnMut(&Sim) -> Action,
    mut feel: impl FnMut(&apprentice::sim::Outcome),
) -> Result<Life, SimError> {
    let config = Setup::for_change(Change::Food).config;
    let mut sim = Sim::new(config, seed)?;
    let mut metrics = Metrics::default();
    let mut hungry = 0;
    for _ in 0..STEPS {
        if sim.energy() < HUNGRY {
            hungry += 1;
        }
        let action = choose(&sim);
        let outcome = sim.step(action);
        feel(&outcome);
        metrics.record(&outcome);
    }
    Ok(Life {
        summary: metrics.summary(),
        hungry: hungry as f64 / STEPS as f64,
    })
}

fn mind_life(seed: u64, mut mind: impl Mind) -> Result<Life, SimError> {
    let cell = std::cell::RefCell::new(&mut mind);
    live(
        seed,
        |sim| cell.borrow_mut().act(&sim.observe()),
        |outcome| cell.borrow_mut().feel(outcome),
    )
}

fn run(name: &str, seed: u64) -> Result<Life, SimError> {
    let rates = Rates::default();
    let impatient = Drives {
        discount: 0.9,
        ..Drives::default()
    };
    let no_replay = Recall {
        meal_replays: 0,
        ..Recall::default()
    };
    match name {
        "sees the map" => live(seed, nearest_food, |_| {}),
        "phase 4" => mind_life(
            seed,
            Apprentice::with_model(impatient, Memory::new(rates, no_replay), seed),
        ),
        "meal replay" => mind_life(
            seed,
            Apprentice::with_model(impatient, Memory::new(rates, Recall::default()), seed),
        ),
        "patient" => mind_life(
            seed,
            Apprentice::with_model(Drives::default(), Memory::new(rates, no_replay), seed),
        ),
        _ => mind_life(
            seed,
            Apprentice::with_model(
                Drives::default(),
                Memory::new(rates, Recall::default()),
                seed,
            ),
        ),
    }
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

fn parse(args: &[String]) -> Result<(u64, PathBuf), String> {
    let (mut seeds, mut out) = (10, PathBuf::from("runs/energy"));
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or(format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--seeds" => seeds = value.parse().map_err(|_| format!("bad seeds: {value}"))?,
            "--out" => out = PathBuf::from(value),
            _ => return Err(format!("unknown flag: {flag}")),
        }
    }
    if seeds == 0 {
        return Err("--seeds must be positive".into());
    }
    Ok((seeds, out))
}

fn execute(seeds: u64, out: &PathBuf) -> Result<(), Error> {
    let mut csv = String::from("mind,seed,sleeps,meals,mean_energy,hungry_share\n");
    for name in MINDS {
        let mut lives = Vec::new();
        for seed in 1..=seeds {
            let life = run(name, seed)?;
            let s = &life.summary;
            csv.push_str(&format!(
                "{name},{seed},{},{},{:.2},{:.3}\n",
                s.sleeps, s.meals, s.mean_energy, life.hungry
            ));
            lives.push(life);
        }
        let med = |f: fn(&Life) -> f64| median(lives.iter().map(f).collect());
        println!(
            "{name:>22}: median sleeps {:.0}, meals {:.0}, mean energy {:.1}, hungry {:.0}% of steps",
            med(|l| l.summary.sleeps as f64),
            med(|l| l.summary.meals as f64),
            med(|l| l.summary.mean_energy),
            100.0 * med(|l| l.hungry),
        );
    }
    fs::create_dir_all(out)?;
    fs::write(out.join("energy.csv"), csv)?;
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let (seeds, out) = match parse(&args) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("{e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = execute(seeds, &out) {
        eprintln!("energy: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
