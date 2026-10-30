//! A life for judging memory settings: the language test's life with no
//! mentor (four upbringing worlds, then a new bush world and, from a second
//! upbringing, a new barren world), and the home test (home, home as left,
//! home with a wall moved, three strangers). Shared by the experiments that
//! compare two [`Recall`] settings seed by seed.

use std::collections::HashSet;
use std::thread;

use crate::adaptation::{Change, Setup};
use crate::apprentice::{Apprentice, Drives, Mind};
use crate::home::{Place, at_home_as, step, world};
use crate::memory::{Memory, Recall, Traffic};
use crate::model::{Rates, WorldModel, kind};
use crate::sim::{Config, Event, Regrowth, Sim, SimError};
use crate::world::{Cell, Pos};

/// The apprentice whose life is measured.
pub type Learner = Apprentice<Memory>;

pub const UPBRINGING: u64 = 4;
/// Upbringing worlds, as a count.
pub const RAISED: usize = UPBRINGING as usize;
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
        replayed: after.replayed - before.replayed,
        taught: after.taught - before.taught,
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

/// An apprentice raised in bush and barren worlds in turn, ending in the
/// kind unlike `next`: the mind, and each upbringing world.
pub fn raise(
    setting: Setting,
    seed: u64,
    next: &Config,
) -> Result<(Learner, [Stay; RAISED]), SimError> {
    let memory = Memory::new(Rates::default(), setting.recall);
    let mut mind = Apprentice::with_model(setting.drives, memory, seed);
    let (last, other) = if next.regrowth == Regrowth::Anywhere {
        (bushes(), barren())
    } else {
        (barren(), bushes())
    };
    let mut raised = [Stay::default(); RAISED];
    for (i, slot) in (0..UPBRINGING).zip(&mut raised) {
        let config = if (UPBRINGING - i) % 2 == 1 {
            last.clone()
        } else {
            other.clone()
        };
        *slot = live(&mut mind, config, seed + 100 * i, WORLD_STEPS)?;
    }
    Ok((mind, raised))
}

/// Seed offset of the new world like `next` after an upbringing.
fn offset(next: &Config) -> u64 {
    if next.regrowth == Regrowth::Anywhere {
        5_000
    } else {
        6_000
    }
}

/// The new barren world after a bush-last upbringing on `seed`.
pub fn barren_test(mind: &mut Learner, seed: u64) -> Result<Stay, SimError> {
    let next = barren();
    let seed = seed + offset(&next);
    live(mind, next, seed, BARREN_STEPS)
}

/// [`raise`], then a new world like `next`: each upbringing world, and the
/// new world.
fn upbringing_then(
    setting: Setting,
    seed: u64,
    next: Config,
    steps: usize,
) -> Result<([Stay; RAISED], Stay), SimError> {
    let (mut mind, raised) = raise(setting, seed, &next)?;
    let seed = seed + offset(&next);
    Ok((raised, live(&mut mind, next, seed, steps)?))
}

fn add(a: Traffic, b: Traffic) -> Traffic {
    Traffic {
        sightings: a.sightings + b.sightings,
        written: a.written + b.written,
        naps: a.naps + b.naps,
        unexpected: a.unexpected + b.unexpected,
        unexpected_written: a.unexpected_written + b.unexpected_written,
        replayed: a.replayed + b.replayed,
        taught: a.taught + b.taught,
    }
}

/// Wrong answers to "is this home?" over the five trials, after `ASKED`
/// steps and after `TRIAL_STEPS`.
fn home_test(setting: Setting, seed: u64) -> Result<(u32, u32), SimError> {
    let places = [Place::Home, Place::Changed]
        .into_iter()
        .chain((0..STRANGERS).map(Place::Stranger));
    let mut wrong = (0, 0);
    for place in places {
        let (mut mind, home, home_map) = at_home_as(setting.recall, setting.drives, seed)?;
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

/// What a life is lived with: how it remembers and what it wants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Setting {
    pub recall: Recall,
    pub drives: Drives,
}

impl From<Recall> for Setting {
    fn from(recall: Recall) -> Self {
        Self {
            recall,
            drives: Drives::default(),
        }
    }
}

/// Everything one seed showed under one setting.
#[derive(Debug, Clone, Copy)]
pub struct Life {
    /// The bush-first upbringing's sleeps and traffic, all four worlds.
    pub raised: Stay,
    pub bush: Stay,
    pub barren: Stay,
    /// Wrong home-test answers after `ASKED` and `TRIAL_STEPS` steps.
    pub home: (u32, u32),
    /// Each upbringing world before the bush test (bush first) and before
    /// the barren test (barren first).
    pub bush_raised: [Stay; RAISED],
    pub barren_raised: [Stay; RAISED],
}

impl Life {
    /// Sleeps over the upbringing worlds of both lives that were barren
    /// (`barren`) or bush (`!barren`).
    pub fn raised_sleeps(&self, barren: bool) -> u32 {
        // Bush-first lives have barren worlds at odd places, barren-first
        // lives at even ones.
        let pick = |stays: &[Stay; RAISED], odd: bool| -> u32 {
            stays
                .iter()
                .enumerate()
                .filter(|(i, _)| (i % 2 == 1) == odd)
                .map(|(_, s)| s.sleeps)
                .sum()
        };
        pick(&self.bush_raised, barren) + pick(&self.barren_raised, !barren)
    }
}

pub fn life(setting: impl Into<Setting>, seed: u64) -> Result<Life, SimError> {
    let setting = setting.into();
    let (bush_raised, bush) = upbringing_then(setting, seed, bushes(), BUSH_STEPS)?;
    let (barren_raised, barren) = upbringing_then(setting, seed, barren(), BARREN_STEPS)?;
    let raised = bush_raised.iter().fold(Stay::default(), |mut all, s| {
        all.sleeps += s.sleeps;
        all.traffic = add(all.traffic, s.traffic);
        all
    });
    Ok(Life {
        raised,
        bush,
        barren,
        home: home_test(setting, seed)?,
        bush_raised,
        barren_raised,
    })
}

/// Lives every seed under `base` and under `test`, spread over the
/// machine's cores, in seed order.
pub fn lives(
    seeds: &[u64],
    base: Recall,
    test: Recall,
) -> Result<Vec<(u64, Life, Life)>, LivesError> {
    let all = lives_under(seeds, &[base, test])?;
    Ok(all
        .into_iter()
        .map(|(seed, l)| (seed, l[0], l[1]))
        .collect())
}

/// Lives every seed under each of `settings`, spread over the machine's
/// cores, in seed order; each seed's lives in the order of `settings`.
pub fn lives_under<S: Into<Setting> + Copy + Sync>(
    seeds: &[u64],
    settings: &[S],
) -> Result<Vec<(u64, Vec<Life>)>, LivesError> {
    let workers = thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = seeds.len().div_ceil(workers).max(1);
    let one = |seed: u64| -> Result<(u64, Vec<Life>), LivesError> {
        let each: Result<Vec<Life>, SimError> = settings.iter().map(|&r| life(r, seed)).collect();
        Ok((seed, each?))
    };
    let results: Vec<Result<Vec<_>, LivesError>> = thread::scope(|scope| {
        let handles: Vec<_> = seeds
            .chunks(chunk)
            .map(|part| scope.spawn(move || part.iter().map(|&seed| one(seed)).collect()))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raised_sleeps_sorts_upbringing_worlds_by_kind() {
        let stays = |s: [u32; RAISED]| {
            s.map(|sleeps| Stay {
                sleeps,
                ..Stay::default()
            })
        };
        let life = Life {
            raised: Stay::default(),
            bush: Stay::default(),
            barren: Stay::default(),
            home: (0, 0),
            // Bush first: bush, barren, bush, barren.
            bush_raised: stays([1, 10, 1, 10]),
            // Barren first: barren, bush, barren, bush.
            barren_raised: stays([100, 2, 100, 2]),
        };
        assert_eq!(life.raised_sleeps(true), 220);
        assert_eq!(life.raised_sleeps(false), 6);
    }
}
