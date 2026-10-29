//! Gate experiment: does active memory's gate keep the diary small without
//! losing anything that matters?
//!
//! Each seed is lived twice, gate off and gate on: the language test's life
//! with no mentor (four upbringing worlds, then a new bush world and, from a
//! second upbringing, a new barren world), and the home test (home, then
//! home as left, home with a wall moved, three strangers). Rules are in
//! docs/gate/README.md.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::thread;

use apprentice::adaptation::{Change, Setup};
use apprentice::apprentice::{Apprentice, Drives, Mind};
use apprentice::home::{Place, at_home_with, step, world};
use apprentice::judge::Interval;
use apprentice::memory::{Gate, Memory, Recall, Traffic};
use apprentice::model::{Rates, WorldModel, kind};
use apprentice::sim::{Config, Event, Regrowth, Sim, SimError};
use apprentice::world::{Cell, Pos};

const USAGE: &str = "\
usage: gate [--seeds N] [--from SEED] [--judge] [--threshold X] [--span K] [--out DIR]

Seeds FROM..FROM+N-1 (default 101..110, the tuning seeds), or with --judge
the judging seeds 1-100 and 111-210. Lives each seed with the gate off and
on (threshold X, default 0.05; active memory of K squares, default 18),
prints each rule's verdict and writes DIR/gate.csv (default DIR: runs/gate).";

type Error = Box<dyn std::error::Error>;
type Learner = Apprentice<Memory>;

const UPBRINGING: u64 = 4;
const WORLD_STEPS: usize = 1_000;
const BARREN_STEPS: usize = 2_000;
const BUSH_STEPS: usize = 4_000;
/// Steps of the barren world in which false hopes are counted.
const WINDOW: usize = 1_000;
/// Chance of food above which a square counts as hoped for.
const HOPE: f64 = 0.5;
/// The home test: steps per trial, and the steps at which answers are read.
const TRIAL_STEPS: usize = 300;
const ASKED: usize = 100;
const STRANGERS: u64 = 3;

fn bushes() -> Config {
    Setup::for_change(Change::Food).config
}

fn barren() -> Config {
    Config {
        regrowth: Regrowth::Anywhere,
        ..bushes()
    }
}

/// What a stretch in one world showed.
#[derive(Debug, Default, Clone, Copy)]
struct Stay {
    sleeps: u32,
    /// False hopes in the first `WINDOW` steps: a square it ate from comes
    /// into view empty when it gave it more than even odds of food.
    hopes: u32,
    meals: u32,
    /// Tries to eat, in the first `WINDOW` steps, at a square it ate from
    /// that holds nothing.
    futile: u32,
    /// What memory did with its sightings during the stay.
    traffic: Traffic,
}

fn since(before: Traffic, after: Traffic) -> Traffic {
    Traffic {
        sightings: after.sightings - before.sightings,
        written: after.written - before.written,
        naps: after.naps - before.naps,
        unexpected: after.unexpected - before.unexpected,
        unexpected_written: after.unexpected_written - before.unexpected_written,
    }
}

fn live(mind: &mut Learner, config: Config, seed: u64, steps: usize) -> Result<Stay, SimError> {
    let mut sim = Sim::new(config, seed)?;
    let mut eaten: HashSet<Pos> = HashSet::new();
    let mut stay = Stay::default();
    let before = mind.model().traffic();
    for t in 0..steps {
        let here = sim.pos();
        if t < WINDOW {
            stay.hopes += false_hopes(mind, &sim, &eaten);
        }
        let outcome = sim.step(mind.act(&sim.observe()));
        mind.feel(&outcome);
        stay.sleeps += u32::from(outcome.slept);
        match outcome.event {
            Event::Ate => {
                stay.meals += 1;
                eaten.insert(here);
            }
            Event::AteNothing if t < WINDOW && eaten.contains(&here) => stay.futile += 1,
            _ => {}
        }
    }
    stay.traffic = since(before, mind.model().traffic());
    Ok(stay)
}

/// Squares it ate from, now in view and empty, to which it gave more than
/// even odds of food just before looking.
fn false_hopes(mind: &Learner, sim: &Sim, eaten: &HashSet<Pos>) -> u32 {
    let start = sim.start();
    let mut hopes = 0;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let at = sim.pos().shift(dx, dy);
            if !eaten.contains(&at) || sim.grid().get(at) == Cell::Food {
                continue;
            }
            let belief = mind.model().predict(at.shift(-start.x, -start.y));
            hopes += u32::from(belief[kind(Cell::Food)] > HOPE);
        }
    }
    hopes
}

/// Bush and barren worlds in turn, ending in the kind unlike `next`, then
/// a new world like `next`.
fn upbringing_then(
    recall: Recall,
    seed: u64,
    next: Config,
    steps: usize,
) -> Result<(Stay, Stay), SimError> {
    let memory = Memory::new(Rates::default(), recall);
    let mut mind = Apprentice::with_model(Drives::default(), memory, seed);
    let (last, other) = if next.regrowth == Regrowth::Anywhere {
        (bushes(), barren())
    } else {
        (barren(), bushes())
    };
    let mut raised = Stay::default();
    for i in 0..UPBRINGING {
        let config = if (UPBRINGING - i) % 2 == 1 {
            last.clone()
        } else {
            other.clone()
        };
        let stay = live(&mut mind, config, seed + 100 * i, WORLD_STEPS)?;
        raised.sleeps += stay.sleeps;
        raised.traffic = add(raised.traffic, stay.traffic);
    }
    let offset = if next.regrowth == Regrowth::Anywhere {
        5_000
    } else {
        6_000
    };
    Ok((raised, live(&mut mind, next, seed + offset, steps)?))
}

fn add(a: Traffic, b: Traffic) -> Traffic {
    Traffic {
        sightings: a.sightings + b.sightings,
        written: a.written + b.written,
        naps: a.naps + b.naps,
        unexpected: a.unexpected + b.unexpected,
        unexpected_written: a.unexpected_written + b.unexpected_written,
    }
}

/// Wrong answers to "is this home?" over the five trials, after `ASKED`
/// steps and after `TRIAL_STEPS`.
fn home_test(recall: Recall, seed: u64) -> Result<(u32, u32), SimError> {
    let places = [Place::Home, Place::Changed]
        .into_iter()
        .chain((0..STRANGERS).map(Place::Stranger));
    let mut wrong = (0, 0);
    for place in places {
        let (mut mind, home, home_map) = at_home_with(recall, seed)?;
        let mut sim = world(place, &home, seed);
        for s in 1..=TRIAL_STEPS {
            step(&mut sim, &mut mind);
            let right = (mind.map_in_use() == home_map) == place.is_home();
            if s == ASKED {
                wrong.0 += u32::from(!right);
            }
            if s == TRIAL_STEPS {
                wrong.1 += u32::from(!right);
            }
        }
    }
    Ok(wrong)
}

/// A named per-seed measure.
type Measure = (&'static str, fn(&Life) -> f64);

/// Everything one seed showed under one setting.
#[derive(Debug, Clone, Copy)]
struct Life {
    raised: Stay,
    bush: Stay,
    barren: Stay,
    /// Wrong home-test answers after `ASKED` and `TRIAL_STEPS` steps.
    home: (u32, u32),
}

fn life(recall: Recall, seed: u64) -> Result<Life, SimError> {
    let (raised, bush) = upbringing_then(recall, seed, bushes(), BUSH_STEPS)?;
    let (_, barren) = upbringing_then(recall, seed, barren(), BARREN_STEPS)?;
    Ok(Life {
        raised,
        bush,
        barren,
        home: home_test(recall, seed)?,
    })
}

/// Share of unexpected sightings written; 1 when there were none.
fn let_through(t: &Traffic) -> f64 {
    if t.unexpected == 0 {
        return 1.0;
    }
    t.unexpected_written as f64 / t.unexpected as f64
}

/// Parsed flags.
struct Args {
    seeds: Vec<u64>,
    gate: Gate,
    /// Diary capacity with the gate on, in steps; `None` keeps the default.
    capacity: Option<usize>,
    out: PathBuf,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let (mut count, mut from, mut judge) = (10, 101, false);
    let mut gate = Gate::default();
    let mut capacity = None;
    let mut out = PathBuf::from("runs/gate");
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
            "--threshold" => gate.threshold = value.parse().map_err(|_| bad())?,
            "--capacity" => capacity = Some(value.parse().map_err(|_| bad())?),
            "--span" => gate.span = value.parse().map_err(|_| bad())?,
            "--out" => out = PathBuf::from(value),
            _ => return Err(format!("unknown flag: {flag}")),
        }
    }
    if count == 0 || gate.span == 0 {
        return Err("--seeds and --span must be positive".into());
    }
    let seeds = if judge {
        (1..=100).chain(111..=210).collect()
    } else {
        (from..from + count).collect()
    };
    Ok(Args {
        seeds,
        gate,
        capacity,
        out,
    })
}

/// Lives every seed both ways, spread over the machine's cores.
fn lives(
    seeds: &[u64],
    gate: Gate,
    capacity: Option<usize>,
) -> Result<Vec<(u64, Life, Life)>, Error> {
    let off = Recall::default();
    let on = Recall {
        gate: Some(gate),
        capacity: capacity.unwrap_or(off.capacity),
        ..off
    };
    let workers = thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = seeds.len().div_ceil(workers);
    let results: Vec<Result<Vec<_>, SimError>> = thread::scope(|scope| {
        let handles: Vec<_> = seeds
            .chunks(chunk)
            .map(|part| {
                scope.spawn(move || {
                    part.iter()
                        .map(|&seed| Ok((seed, life(off, seed)?, life(on, seed)?)))
                        .collect()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|_| Ok(Vec::new())))
            .collect()
    });
    let mut all = Vec::new();
    for part in results {
        all.extend(part?);
    }
    if all.len() != seeds.len() {
        return Err("a worker thread panicked".into());
    }
    Ok(all)
}

fn csv(rows: &[(u64, Life, Life)]) -> String {
    let mut out = String::from(
        "seed,gate,raised_sightings,raised_written,raised_naps,raised_sleeps,bush_sightings,bush_written,bush_naps,bush_unexpected,bush_unexpected_written,bush_sleeps,barren_written,barren_sleeps,barren_hopes,barren_meals,barren_futile,bush_meals,home_wrong_100,home_wrong_300\n",
    );
    for (seed, off, on) in rows {
        for (name, l) in [("off", off), ("on", on)] {
            let (r, b) = (l.raised.traffic, l.bush.traffic);
            out.push_str(&format!(
                "{seed},{name},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                r.sightings,
                r.written,
                r.naps,
                l.raised.sleeps,
                b.sightings,
                b.written,
                b.naps,
                b.unexpected,
                b.unexpected_written,
                l.bush.sleeps,
                l.barren.traffic.written,
                l.barren.sleeps,
                l.barren.hopes,
                l.barren.meals,
                l.barren.futile,
                l.bush.meals,
                l.home.0,
                l.home.1,
            ));
        }
    }
    out
}

fn report(rows: &[(u64, Life, Life)]) -> Result<(), Error> {
    let pair = |f: fn(&Life) -> f64| -> Result<Interval, Error> {
        let base: Vec<f64> = rows.iter().map(|r| f(&r.1)).collect();
        let test: Vec<f64> = rows.iter().map(|r| f(&r.2)).collect();
        Ok(Interval::change(&base, &test)?)
    };
    let pct = |i: &Interval| {
        format!(
            "{:+.0}% [{:+.0}%, {:+.0}%]",
            100.0 * i.mid,
            100.0 * i.lo,
            100.0 * i.hi
        )
    };
    let written = pair(|l| l.bush.traffic.written as f64)?;
    println!(
        "1. bush-world diary writes {}: {}",
        pct(&written),
        written.at_most(-0.5)
    );
    let naps = pair(|l| l.bush.traffic.naps as f64)?;
    println!("2. bush-world naps {}: {}", pct(&naps), naps.at_most(-0.3));
    let through: Vec<f64> = rows
        .iter()
        .map(|r| let_through(&r.2.bush.traffic))
        .collect();
    let share = Interval::median(&through)?;
    println!(
        "3. unexpected sightings written, median share {:.3} [{:.3}, {:.3}]: {}",
        share.mid,
        share.lo,
        share.hi,
        share.at_least(0.95)
    );
    println!("4. no regressions (each at most +10%):");
    let checks: [Measure; 4] = [
        ("bush sleeps", |l| f64::from(l.bush.sleeps)),
        ("barren sleeps", |l| f64::from(l.barren.sleeps)),
        ("barren false hopes", |l| f64::from(l.barren.hopes)),
        ("home-test wrong answers (100 steps)", |l| {
            f64::from(l.home.0)
        }),
    ];
    for (name, f) in checks {
        let i = pair(f)?;
        println!("   {name} {}: {}", pct(&i), i.at_most(0.1));
    }
    println!("Reported only:");
    let reported: [Measure; 4] = [
        ("upbringing diary writes", |l| {
            l.raised.traffic.written as f64
        }),
        ("upbringing naps", |l| l.raised.traffic.naps as f64),
        ("upbringing sleeps", |l| f64::from(l.raised.sleeps)),
        ("home-test wrong answers (300 steps)", |l| {
            f64::from(l.home.1)
        }),
    ];
    for (name, f) in reported {
        println!("   {name} {}", pct(&pair(f)?));
    }
    let sum = |f: fn(&Life) -> f64, on: bool| -> f64 {
        rows.iter()
            .map(|r| f(if on { &r.2 } else { &r.1 }))
            .sum::<f64>()
            / rows.len() as f64
    };
    let means: [Measure; 8] = [
        ("barren meals", |l| f64::from(l.barren.meals)),
        ("barren futile returns", |l| f64::from(l.barren.futile)),
        ("bush meals", |l| f64::from(l.bush.meals)),
        ("bush writes", |l| l.bush.traffic.written as f64),
        ("bush naps", |l| l.bush.traffic.naps as f64),
        ("bush sleeps", |l| f64::from(l.bush.sleeps)),
        ("barren false hopes", |l| f64::from(l.barren.hopes)),
        ("home wrong (100)", |l| f64::from(l.home.0)),
    ];
    for (name, f) in means {
        println!(
            "   mean {name}: off {:.1}, on {:.1}",
            sum(f, false),
            sum(f, true)
        );
    }
    Ok(())
}

fn execute(args: &Args) -> Result<(), Error> {
    let rows = lives(&args.seeds, args.gate, args.capacity)?;
    report(&rows)?;
    fs::create_dir_all(&args.out)?;
    fs::write(args.out.join("gate.csv"), csv(&rows))?;
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
        eprintln!("gate: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
