//! A life for judging memory settings: the language test's life with no
//! mentor (four upbringing worlds, then a new bush world and, from a second
//! upbringing, a new barren world), and the home test (home, home as left,
//! home with a wall moved, three strangers). Shared by the experiments that
//! compare two [`Recall`] settings seed by seed.

use std::collections::HashSet;
use std::thread;

use crate::adaptation::{Change, Setup};
use crate::apprentice::{Apprentice, Drives, Mind};
use crate::home::{Place, at_home_with, step, world};
use crate::memory::{Memory, Recall, Traffic};
use crate::model::{Rates, WorldModel, kind};
use crate::sim::{Config, Event, Regrowth, Sim, SimError};
use crate::world::{Cell, Pos};

/// The apprentice whose life is measured.
pub type Learner = Apprentice<Memory>;

pub const UPBRINGING: u64 = 4;
pub const WORLD_STEPS: usize = 1_000;
pub const BARREN_STEPS: usize = 2_000;
pub const BUSH_STEPS: usize = 4_000;
/// Steps of the barren world in which false hopes are counted.
pub const WINDOW: usize = 1_000;
/// Chance of food above which a square counts as hoped for.
pub const HOPE: f64 = 0.5;
/// The home test: steps per trial, and the steps at which answers are read.
pub const TRIAL_STEPS: usize = 300;
pub const ASKED: usize = 100;
pub const STRANGERS: u64 = 3;

pub fn bushes() -> Config {
    Setup::for_change(Change::Food).config
}

pub fn barren() -> Config {
    Config {
        regrowth: Regrowth::Anywhere,
        ..bushes()
    }
}

/// What a stretch in one world showed.
#[derive(Debug, Default, Clone, Copy)]
pub struct Stay {
    pub sleeps: u32,
    /// False hopes in the first `WINDOW` steps: a square it ate from comes
    /// into view empty when it gave it more than even odds of food.
    pub hopes: u32,
    pub meals: u32,
    /// Tries to eat, in the first `WINDOW` steps, at a square it ate from
    /// that holds nothing.
    pub futile: u32,
    /// What memory did with its sightings during the stay.
    pub traffic: Traffic,
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

pub fn live(mind: &mut Learner, config: Config, seed: u64, steps: usize) -> Result<Stay, SimError> {
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
pub type Measure = (&'static str, fn(&Life) -> f64);

/// Everything one seed showed under one setting.
#[derive(Debug, Clone, Copy)]
pub struct Life {
    pub raised: Stay,
    pub bush: Stay,
    pub barren: Stay,
    /// Wrong home-test answers after `ASKED` and `TRIAL_STEPS` steps.
    pub home: (u32, u32),
}

pub fn life(recall: Recall, seed: u64) -> Result<Life, SimError> {
    let (raised, bush) = upbringing_then(recall, seed, bushes(), BUSH_STEPS)?;
    let (_, barren) = upbringing_then(recall, seed, barren(), BARREN_STEPS)?;
    Ok(Life {
        raised,
        bush,
        barren,
        home: home_test(recall, seed)?,
    })
}

/// Lives every seed under `base` and under `test`, spread over the
/// machine's cores, in seed order.
pub fn lives(
    seeds: &[u64],
    base: Recall,
    test: Recall,
) -> Result<Vec<(u64, Life, Life)>, LivesError> {
    let workers = thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = seeds.len().div_ceil(workers).max(1);
    let results: Vec<Result<Vec<_>, LivesError>> = thread::scope(|scope| {
        let handles: Vec<_> = seeds
            .chunks(chunk)
            .map(|part| {
                scope.spawn(move || {
                    part.iter()
                        .map(|&seed| Ok((seed, life(base, seed)?, life(test, seed)?)))
                        .collect()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or(Err(LivesError::Panicked)))
            .collect()
    });
    let mut all = Vec::new();
    for part in results {
        all.extend(part?);
    }
    Ok(all)
}

/// Why [`lives`] failed.
#[derive(Debug)]
pub enum LivesError {
    Sim(SimError),
    Panicked,
}

impl From<SimError> for LivesError {
    fn from(e: SimError) -> Self {
        Self::Sim(e)
    }
}

impl std::fmt::Display for LivesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sim(e) => write!(f, "{e}"),
            Self::Panicked => write!(f, "a worker thread panicked"),
        }
    }
}

impl std::error::Error for LivesError {}
