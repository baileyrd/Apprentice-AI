//! Network experiment (world physics): does a network with hidden layers,
//! learning by predictive coding, carry what it learned to a world it has
//! never seen?
//!
//! For each seed a body with random actions lives in one world, then in a
//! new one. Four learners watch the same life and predict each next view:
//! the network, today's map, the same network with no hidden layer, and the
//! network trained by backprop. Rules are in docs/network/README.md.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::thread;

use apprentice::agent::{Agent, RandomAgent};
use apprentice::body::Action;
use apprentice::judge::Interval;
use apprentice::model::{PredictiveCoding, Rates, WorldModel};
use apprentice::network::{Learning, Net};
use apprentice::physics::{
    INPUTS, MEAL, OUTPUTS, input, map_predicts, separation, surprise, target, view_of,
};
use apprentice::sim::{Config, Event, Sim};

const USAGE: &str = "\
usage: network [--seeds N] [--from SEED] [--judge] [--hidden H] [--rate R]
               [--rounds K] [--step S] [--precision P] [--out DIR]

Seeds FROM..FROM+N-1 (default 101..110, the tuning seeds), or with --judge
the judging seeds 1-100 and 111-210. Two hidden layers of H units (default
64), learning rate R (default 0.05), K settling rounds of size S (default
20 and 0.2), hidden-layer precision P (default 100). Prints each rule's verdict and writes DIR/network.csv
(default DIR: runs/network).";

type Error = Box<dyn std::error::Error>;

/// Steps in the first world, and in the new one.
const FIRST_STEPS: usize = 3_000;
const NEW_STEPS: usize = 1_000;
/// Steps over which surprise is totalled: the first in the new world, and
/// the last in the first world.
const WINDOW: usize = 200;
/// Seed offset of the new world.
const NEW_WORLD: u64 = 10_000;

/// How the network is shaped and learns.
#[derive(Debug, Clone, Copy)]
struct Shape {
    hidden: usize,
    rate: f64,
    rounds: u32,
    step: f64,
    precision: f64,
}

/// The four learners.
struct Learners {
    map: PredictiveCoding,
    net: Net,
    flat: Net,
    backprop: Net,
}

impl Learners {
    fn new(shape: Shape, seed: u64) -> Result<Self, Error> {
        let pc = Learning::PredictiveCoding {
            rounds: shape.rounds,
            step: shape.step,
            precision: shape.precision,
        };
        let deep = [INPUTS, shape.hidden, shape.hidden, OUTPUTS];
        Ok(Self {
            map: PredictiveCoding::new(Rates::default()),
            net: Net::new(&deep, shape.rate, pc, seed)?,
            flat: Net::new(&[INPUTS, OUTPUTS], shape.rate, pc, seed)?,
            backprop: Net::new(&deep, shape.rate, Learning::Backprop, seed)?,
        })
    }
}

/// What one world showed, per learner: surprise totals over the window,
/// and the network's energy-prediction error and hidden activity on moves.
#[derive(Debug, Default, Clone)]
struct Stay {
    /// Total "surprise" in the window: map, network, no hidden layer,
    /// backprop.
    surprise: [f64; 4],
    /// Mean squared error of the network's energy prediction, in meals,
    /// over every counted step.
    energy_error: f64,
    /// Hidden activities on moves that bumped, and on moves that went
    /// through; one row per move.
    bumps: Vec<Vec<f64>>,
    moves: Vec<Vec<f64>>,
}

/// Lives `steps` in `sim`; surprise counts in steps `window` (by index).
fn live(
    sim: &mut Sim,
    body: &mut RandomAgent,
    l: &mut Learners,
    steps: usize,
    window: std::ops::Range<usize>,
    read_units: bool,
) -> Stay {
    let mut stay = Stay::default();
    let mut counted = 0;
    for t in 0..steps {
        let obs = sim.observe();
        l.map.observe(&obs);
        let action = body.act(&obs);
        let x = input(&obs, action);
        let guesses = [
            map_predicts(&l.map, &obs, action),
            view_of(&l.net.predict(&x)),
            view_of(&l.flat.predict(&x)),
            view_of(&l.backprop.predict(&x)),
        ];
        let energy_guess = l.net.predict(&x)[OUTPUTS - 1];
        let units = read_units.then(|| l.net.hidden(&x));
        let outcome = sim.step(action);
        if outcome.slept {
            continue;
        }
        let next = sim.observe();
        let change = f64::from(outcome.energy) - f64::from(obs.energy);
        if window.contains(&t) {
            for (s, g) in stay.surprise.iter_mut().zip(&guesses) {
                *s += surprise(g, &next);
            }
        }
        stay.energy_error += (energy_guess - change / MEAL).powi(2);
        counted += 1;
        if let (Some(units), Action::Move(_)) = (units, action) {
            match outcome.event {
                Event::Bumped => stay.bumps.push(units),
                _ => stay.moves.push(units),
            }
        }
        let y = target(&next, change);
        for net in [&mut l.net, &mut l.flat, &mut l.backprop] {
            net.learn(&x, &y);
        }
    }
    stay.energy_error /= f64::from(counted.max(1));
    stay
}

/// Best single hidden unit's separation of bumps from moves.
fn best_unit(stay: &Stay) -> f64 {
    let units = stay.bumps.first().map_or(0, Vec::len);
    (0..units)
        .filter_map(|u| {
            let hits: Vec<f64> = stay.bumps.iter().map(|r| r[u]).collect();
            let misses: Vec<f64> = stay.moves.iter().map(|r| r[u]).collect();
            separation(&hits, &misses)
        })
        .fold(0.0, f64::max)
}

/// What one seed showed.
#[derive(Debug, Clone)]
struct Life {
    first: Stay,
    new: Stay,
    /// Best hidden unit's separation of bumps in the new world.
    bump_unit: f64,
}

fn life(shape: Shape, seed: u64) -> Result<Life, Error> {
    let config = Config::default();
    let mut body = RandomAgent::new(seed);
    let mut l = Learners::new(shape, seed)?;
    let mut sim = Sim::new(config.clone(), seed)?;
    let first = live(
        &mut sim,
        &mut body,
        &mut l,
        FIRST_STEPS,
        FIRST_STEPS - WINDOW..FIRST_STEPS,
        false,
    );
    l.map = l.map.schema();
    let mut sim = Sim::new(config, seed + NEW_WORLD)?;
    let new = live(&mut sim, &mut body, &mut l, NEW_STEPS, 0..WINDOW, true);
    let bump_unit = best_unit(&new);
    Ok(Life {
        first,
        new,
        bump_unit,
    })
}

/// Parsed flags.
struct Args {
    seeds: Vec<u64>,
    shape: Shape,
    out: PathBuf,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let (mut count, mut from, mut judge) = (10u64, 101u64, false);
    let mut shape = Shape {
        hidden: 64,
        rate: 0.05,
        rounds: 20,
        step: 0.2,
        precision: 100.0,
    };
    let mut out = PathBuf::from("runs/network");
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
            "--hidden" => shape.hidden = value.parse().map_err(|_| bad())?,
            "--rate" => shape.rate = value.parse().map_err(|_| bad())?,
            "--rounds" => shape.rounds = value.parse().map_err(|_| bad())?,
            "--precision" => shape.precision = value.parse().map_err(|_| bad())?,
            "--step" => shape.step = value.parse().map_err(|_| bad())?,
            "--out" => out = PathBuf::from(value),
            _ => return Err(format!("unknown flag: {flag}")),
        }
    }
    if count == 0 || shape.hidden == 0 {
        return Err("--seeds and --hidden must be positive".into());
    }
    let seeds = if judge {
        (1..=100).chain(111..=210).collect()
    } else {
        (from..from + count).collect()
    };
    Ok(Args { seeds, shape, out })
}

/// Lives every seed, spread over the machine's cores.
fn lives(seeds: &[u64], shape: Shape) -> Result<Vec<(u64, Life)>, Error> {
    let workers = thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = seeds.len().div_ceil(workers);
    let results: Vec<Result<Vec<(u64, Life)>, String>> = thread::scope(|scope| {
        let handles: Vec<_> = seeds
            .chunks(chunk)
            .map(|part| {
                scope.spawn(move || {
                    part.iter()
                        .map(|&seed| {
                            life(shape, seed)
                                .map(|l| (seed, l))
                                .map_err(|e| e.to_string())
                        })
                        .collect()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err("a worker thread panicked".into()))
            })
            .collect()
    });
    let mut all = Vec::new();
    for part in results {
        all.extend(part?);
    }
    Ok(all)
}

const NAMES: [&str; 4] = ["map", "network", "no hidden layer", "backprop"];

fn csv(rows: &[(u64, Life)]) -> String {
    let mut out = String::from(
        "seed,new_map,new_net,new_flat,new_backprop,first_map,first_net,first_flat,first_backprop,energy_error_first,energy_error_new,bump_unit,bumps,moves\n",
    );
    for (seed, l) in rows {
        let s = |x: &[f64; 4]| x.map(|v| format!("{v:.3}")).join(",");
        out.push_str(&format!(
            "{seed},{},{},{:.4},{:.4},{:.3},{},{}\n",
            s(&l.new.surprise),
            s(&l.first.surprise),
            l.first.energy_error,
            l.new.energy_error,
            l.bump_unit,
            l.new.bumps.len(),
            l.new.moves.len(),
        ));
    }
    out
}

fn report(rows: &[(u64, Life)]) -> Result<(), Error> {
    let column = |stay: fn(&Life) -> &Stay, i: usize| -> Vec<f64> {
        rows.iter().map(|(_, l)| stay(l).surprise[i]).collect()
    };
    fn new(l: &Life) -> &Stay {
        &l.new
    }
    fn first(l: &Life) -> &Stay {
        &l.first
    }
    let pct = |i: &Interval| {
        format!(
            "{:+.0}% [{:+.0}%, {:+.0}%]",
            100.0 * i.mid,
            100.0 * i.lo,
            100.0 * i.hi
        )
    };
    let net = column(new, 1);
    let vs = |i: usize| Interval::change(&column(new, i), &net);
    let map = vs(0)?;
    println!(
        "1. network vs map, new world {}: {}",
        pct(&map),
        map.at_most(-0.25)
    );
    let flat = vs(2)?;
    println!(
        "2. network vs no hidden layer, new world {}: {}",
        pct(&flat),
        flat.at_most(-0.25)
    );
    let bp = vs(3)?;
    println!(
        "3. network vs backprop, new world {}: {}",
        pct(&bp),
        bp.at_most(0.1)
    );
    let hits = rows.iter().filter(|(_, l)| l.bump_unit >= 0.9).count();
    let share = Interval::share(hits, rows.len())?;
    println!(
        "4. a bump unit (AUC >= 0.9) in {hits}/{} seeds [{:.2}, {:.2}]: {}",
        rows.len(),
        share.lo,
        share.hi,
        share.at_least(0.8)
    );
    println!("Reported only (mean total surprise over {WINDOW} steps):");
    for (i, name) in NAMES.iter().enumerate() {
        let mean = |xs: Vec<f64>| xs.iter().sum::<f64>() / xs.len() as f64;
        println!(
            "   {name:>16}: new world {:.1}, first world's last steps {:.1}",
            mean(column(new, i)),
            mean(column(first, i))
        );
    }
    let mean =
        |f: fn(&Life) -> f64| rows.iter().map(|(_, l)| f(l)).sum::<f64>() / rows.len() as f64;
    println!(
        "   energy prediction error (meals², network): first world {:.4}, new world {:.4}",
        mean(|l| l.first.energy_error),
        mean(|l| l.new.energy_error)
    );
    println!("   best bump unit, mean AUC: {:.3}", mean(|l| l.bump_unit));
    Ok(())
}

fn execute(args: &Args) -> Result<(), Error> {
    let rows = lives(&args.seeds, args.shape)?;
    report(&rows)?;
    fs::create_dir_all(&args.out)?;
    fs::write(args.out.join("network.csv"), csv(&rows))?;
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
        eprintln!("network: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
