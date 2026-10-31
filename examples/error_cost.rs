//! What each home-test error costs: sleeps and meals over the 1,000 steps
//! after the answer is read (step `ASKED`), for a home taken as a stranger
//! and a stranger taken as home, against right answers in the same kind of
//! place. Reported only, never a mark (docs/excuse).
//!
//! usage: cargo run --release --example error_cost -- FIRST LAST

use std::process::ExitCode;
use std::thread;

use apprentice::apprentice::{Drives, Mind};
use apprentice::home::{Place, at_home_as, world};
use apprentice::life::{ASKED, STRANGERS};
use apprentice::memory::{Excuse, Recall};
use apprentice::sim::{Event, SimError};

/// Steps counted after the answer is read.
const AFTER: usize = 1_000;

const ARMS: [(&str, Excuse); 3] = [
    ("A", Excuse::Always),
    ("X", Excuse::Never),
    ("N", Excuse::WhileFit),
];

/// One trial: arm, whether the place is home, whether the answer was
/// right, and sleeps and meals over the next `AFTER` steps.
#[derive(Debug, Clone, Copy)]
struct Trial {
    arm: usize,
    home: bool,
    right: bool,
    sleeps: u32,
    meals: u32,
}

fn trial(arm: usize, place: Place, seed: u64) -> Result<Trial, SimError> {
    let recall = Recall {
        excuse: ARMS[arm].1,
        ..Recall::default()
    };
    let (mut mind, home, home_map) = at_home_as(recall, Drives::default(), seed)?;
    let mut sim = world(place, &home, seed);
    let mut t = Trial {
        arm,
        home: place.is_home(),
        right: false,
        sleeps: 0,
        meals: 0,
    };
    for s in 1..=ASKED + AFTER {
        let outcome = sim.step(mind.act(&sim.observe()));
        mind.feel(&outcome);
        if s == ASKED {
            t.right = (mind.map_in_use() == home_map) == t.home;
        }
        if s > ASKED {
            t.sleeps += u32::from(outcome.slept);
            t.meals += u32::from(outcome.event == Event::Ate);
        }
    }
    Ok(t)
}

fn seed_trials(seed: u64) -> Result<Vec<Trial>, SimError> {
    let places: Vec<Place> = [Place::Home, Place::Changed]
        .into_iter()
        .chain((0..STRANGERS).map(Place::Stranger))
        .collect();
    let mut out = Vec::new();
    for arm in 0..ARMS.len() {
        for &place in &places {
            out.push(trial(arm, place, seed)?);
        }
    }
    Ok(out)
}

fn run(seeds: &[u64]) -> Result<Vec<Trial>, String> {
    let workers = thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = seeds.len().div_ceil(workers).max(1);
    let parts: Vec<Result<Vec<Trial>, String>> = thread::scope(|scope| {
        let handles: Vec<_> = seeds
            .chunks(chunk)
            .map(|part| {
                scope.spawn(move || {
                    let mut all = Vec::new();
                    for &seed in part {
                        all.extend(seed_trials(seed).map_err(|e| e.to_string())?);
                    }
                    Ok(all)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|_| Err("worker panicked".into())))
            .collect()
    });
    let mut all = Vec::new();
    for part in parts {
        all.extend(part?);
    }
    Ok(all)
}

/// Count, mean sleeps and mean meals of `trials`.
fn summary<'a>(trials: impl Iterator<Item = &'a Trial>) -> (usize, f64, f64) {
    let (mut n, mut sleeps, mut meals) = (0, 0.0, 0.0);
    for t in trials {
        n += 1;
        sleeps += f64::from(t.sleeps);
        meals += f64::from(t.meals);
    }
    let per = |x: f64| if n == 0 { f64::NAN } else { x / n as f64 };
    (n, per(sleeps), per(meals))
}

fn report(trials: &[Trial]) {
    println!("arm,place,answer,trials,sleeps_per_1k,meals_per_1k");
    for (arm, (name, _)) in ARMS.iter().enumerate() {
        for home in [true, false] {
            let place = if home { "home" } else { "stranger" };
            let mut means = [(0, 0.0, 0.0); 2];
            for (i, right) in [true, false].into_iter().enumerate() {
                let s = summary(
                    trials
                        .iter()
                        .filter(|t| t.arm == arm && t.home == home && t.right == right),
                );
                means[i] = s;
                let answer = if right { "right" } else { "wrong" };
                println!("{name},{place},{answer},{},{:.2},{:.2}", s.0, s.1, s.2);
            }
            let (r, w) = (means[0], means[1]);
            println!(
                "{name},{place},wrong-right,,{:+.2},{:+.2}",
                w.1 - r.1,
                w.2 - r.2
            );
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<u64> = std::env::args()
        .skip(1)
        .filter_map(|a| a.parse().ok())
        .collect();
    let [first, last] = args[..] else {
        eprintln!("usage: error_cost FIRST LAST");
        return ExitCode::from(2);
    };
    let seeds: Vec<u64> = (first..=last).collect();
    match run(&seeds) {
        Ok(trials) => {
            report(&trials);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error_cost: {e}");
            ExitCode::FAILURE
        }
    }
}
