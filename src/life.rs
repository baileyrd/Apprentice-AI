//! A life for judging memory settings: the language test's life with no
//! mentor (four upbringing worlds, then a new bush world and, from a second
//! upbringing, a new barren world), and the home test (home, home as left,
//! home with a wall moved, three strangers). Shared by the experiments that
//! compare two [`Recall`] settings seed by seed.

use std::collections::HashSet;
use std::convert::Infallible;
use std::fmt;
use std::thread;

use crate::adaptation::{Change, Setup};
use crate::apprentice::{Apprentice, Drives};
use crate::episode::{Port, RunError, Step, Watch, run, run_agent};
use crate::home::{self, HOME_STEPS, Place, world};
use crate::memory::{Memory, Recall, Traffic};
use crate::model::{Rates, WorldModel, kind};
use crate::sim::{Config, Event, Outcome, Regrowth, Sim, SimError};
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
    live_watched(mind, config, seed, steps, |_, _, _, _| {})
}

/// [`live`], with `watch` shown the mind, the world, the outcome and where
/// it stood, after each step and before the next view is sensed.
pub fn live_watched(
    mind: &mut Learner,
    config: Config,
    seed: u64,
    steps: usize,
    watch: impl FnMut(&Learner, &Sim, &Outcome, Pos),
) -> Result<Stay, SimError> {
    let mut sim = Sim::new(config, seed)?;
    let before = mind.model().traffic();
    let mut counting = Counting {
        behave: Behave::default(),
        hopes: 0,
        watch,
    };
    run_agent(&mut sim, mind, &mut [], steps, &mut counting);
    let seen = counting.behave.seen;
    Ok(Stay {
        sleeps: seen.sleeps,
        hopes: counting.hopes,
        meals: seen.meals,
        futile: seen.futile,
        traffic: since(before, mind.model().traffic()),
    })
}

/// What any agent's stay showed, read from its steps alone.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Behaviour {
    pub sleeps: u32,
    pub meals: u32,
    /// Tries to eat, in the first `WINDOW` steps, at a square it ate from
    /// that holds nothing.
    pub futile: u32,
}

/// [`live`] for any agent, behind any port: what its steps showed. Probes
/// of its beliefs (false hopes, memory traffic) are the apprentice's alone.
pub fn behave<P: Port>(
    agent: &mut P,
    config: Config,
    seed: u64,
    steps: usize,
) -> Result<Behaviour, LifeError<P::Error>> {
    let mut sim = Sim::new(config, seed)?;
    let mut behave = Behave::default();
    run(&mut sim, agent, &mut [], steps, &mut behave)?;
    Ok(behave.seen)
}

/// Counts behaviour step by step.
#[derive(Debug, Default)]
struct Behave {
    t: usize,
    eaten: HashSet<Pos>,
    seen: Behaviour,
}

impl Behave {
    fn record(&mut self, step: &Step) {
        let (outcome, here) = (step.outcome, step.pos);
        self.seen.sleeps += u32::from(outcome.slept);
        match outcome.event {
            Event::Ate => {
                self.seen.meals += 1;
                self.eaten.insert(here);
            }
            Event::AteNothing if self.t < WINDOW && self.eaten.contains(&here) => {
                self.seen.futile += 1;
            }
            _ => {}
        }
        self.t += 1;
    }
}

impl<P: ?Sized> Watch<P> for Behave {
    fn after(&mut self, _agent: &P, _sim: &Sim, step: &Step) {
        self.record(step);
    }
}

/// The apprentice's stay: false hopes before each step is sensed, then
/// `watch`, then its behaviour.
struct Counting<F> {
    behave: Behave,
    hopes: u32,
    watch: F,
}

impl<F: FnMut(&Learner, &Sim, &Outcome, Pos)> Watch<Learner> for Counting<F> {
    fn before(&mut self, mind: &Learner, sim: &Sim) {
        if self.behave.t < WINDOW {
            self.hopes += false_hopes(mind, sim, &self.behave.eaten);
        }
    }

    fn after(&mut self, mind: &Learner, sim: &Sim, step: &Step) {
        (self.watch)(mind, sim, &step.outcome, step.pos);
        self.behave.record(step);
    }
}

/// A life could not be lived: no world, or the agent failed.
#[derive(Debug)]
pub enum LifeError<E> {
    Sim(SimError),
    Agent(RunError<E>),
}

impl<E> From<SimError> for LifeError<E> {
    fn from(e: SimError) -> Self {
        Self::Sim(e)
    }
}

impl<E> From<RunError<E>> for LifeError<E> {
    fn from(e: RunError<E>) -> Self {
        Self::Agent(e)
    }
}

/// An in-process agent can't fail, so only the world's error is left.
impl From<LifeError<Infallible>> for SimError {
    fn from(e: LifeError<Infallible>) -> Self {
        match e {
            LifeError::Sim(e) => e,
            LifeError::Agent(e) => match e.cause {},
        }
    }
}

impl<E: fmt::Display> fmt::Display for LifeError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sim(e) => write!(f, "{e}"),
            Self::Agent(e) => write!(f, "{e}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for LifeError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sim(e) => Some(e),
            Self::Agent(e) => Some(e),
        }
    }
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

/// Seed offset of the new (test) world like `next` after an upbringing.
pub fn test_offset(next: &Config) -> u64 {
    if next.regrowth == Regrowth::Anywhere {
        5_000
    } else {
        6_000
    }
}

/// The new barren world after a bush-last upbringing on `seed`.
pub fn barren_test(mind: &mut Learner, seed: u64) -> Result<Stay, SimError> {
    barren_test_watched(mind, seed, |_, _, _, _| {})
}

/// [`barren_test`], with `watch` as in [`live_watched`].
pub fn barren_test_watched(
    mind: &mut Learner,
    seed: u64,
    watch: impl FnMut(&Learner, &Sim, &Outcome, Pos),
) -> Result<Stay, SimError> {
    let next = barren();
    let seed = seed + test_offset(&next);
    live_watched(mind, next, seed, BARREN_STEPS, watch)
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
    let seed = seed + test_offset(&next);
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
/// steps and after `TRIAL_STEPS`, and strangers taken as home after `ASKED`.
/// The apprentice always names a place; if it ever named none, that trial
/// would count as wrong.
fn home_test(setting: Setting, seed: u64) -> Result<((u32, u32), u32), SimError> {
    let (mut wrong, mut strangers) = ((0, 0), 0);
    for place in trial_places() {
        let memory = Memory::new(Rates::default(), setting.recall);
        let mut mind = Apprentice::with_model(setting.drives, memory, seed);
        let answers = home_trial(&mut mind, place, seed)?.unwrap_or(Answers {
            asked: false,
            last: false,
        });
        wrong.0 += u32::from(!answers.asked);
        wrong.1 += u32::from(!answers.last);
        strangers += u32::from(!answers.asked && !place.is_home());
    }
    Ok((wrong, strangers))
}

/// The home test's five trials: home, changed home, three strangers.
pub fn trial_places() -> impl Iterator<Item = Place> {
    [Place::Home, Place::Changed]
        .into_iter()
        .chain((0..STRANGERS).map(Place::Stranger))
}

/// Whether a trial's answer to "is this home?" was right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Answers {
    /// After `ASKED` steps.
    pub asked: bool,
    /// After `TRIAL_STEPS`.
    pub last: bool,
}

/// One home-test trial for a fresh agent behind any port: `HOME_STEPS` at
/// home, then `TRIAL_STEPS` in `place`. It answers by the place it names
/// ([`Port::place`]), compared with the one it named at the end of home.
/// `None` (n/a) if it named no place at home.
pub fn home_trial<P: Port>(
    agent: &mut P,
    place: Place,
    seed: u64,
) -> Result<Option<Answers>, LifeError<P::Error>> {
    let mut home = Sim::new(home::config(), seed)?;
    run(&mut home, agent, &mut [], HOME_STEPS, &mut ())?;
    let Some(home_place) = agent.place() else {
        return Ok(None);
    };
    let right = |agent: &P| (agent.place() == Some(home_place)) == place.is_home();
    let mut sim = world(place, &home, seed);
    run(&mut sim, agent, &mut [], ASKED, &mut ())?;
    let asked = right(agent);
    run(&mut sim, agent, &mut [], TRIAL_STEPS - ASKED, &mut ())?;
    Ok(Some(Answers {
        asked,
        last: right(agent),
    }))
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
    /// Strangers taken as home after `ASKED` steps (part of `home.0`).
    pub strangers_home: u32,
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
    let (home, strangers_home) = home_test(setting, seed)?;
    let raised = bush_raised.iter().fold(Stay::default(), |mut all, s| {
        all.sleeps += s.sleeps;
        all.traffic = add(all.traffic, s.traffic);
        all
    });
    Ok(Life {
        raised,
        bush,
        barren,
        home,
        strangers_home,
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
    use crate::body::{Action, Observation, Said};
    use crate::episode::Phase;

    #[derive(Debug)]
    struct Broke;

    impl fmt::Display for Broke {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "broke")
        }
    }

    impl std::error::Error for Broke {}

    /// A port and nothing more (not an `Agent`, no probes): rests, names
    /// `place`, and fails to act at step `fail_at`.
    struct Rester {
        place: Option<usize>,
        fail_at: Option<u64>,
        acted: u64,
    }

    fn rester(place: Option<usize>) -> Rester {
        Rester {
            place,
            fail_at: None,
            acted: 0,
        }
    }

    impl Port for Rester {
        type Error = Broke;
        fn act(&mut self, _obs: &Observation) -> Result<Action, Broke> {
            if self.fail_at == Some(self.acted) {
                return Err(Broke);
            }
            self.acted += 1;
            Ok(Action::Rest)
        }
        fn feel(&mut self, _outcome: &Outcome) -> Result<(), Broke> {
            Ok(())
        }
        fn hear(&mut self, _at: Pos, _said: Said) -> Result<(), Broke> {
            Ok(())
        }
        fn place(&self) -> Option<usize> {
            self.place
        }
    }

    #[test]
    fn any_port_has_its_behaviour_counted() {
        // Resting costs 1 a step from 50: it sleeps every 50 steps.
        let seen = behave(&mut rester(None), barren(), 1, 200).expect("lives");
        let want = Behaviour {
            sleeps: 4,
            meals: 0,
            futile: 0,
        };
        assert_eq!(seen, want);
    }

    #[test]
    fn naming_no_place_is_no_answer() {
        let answers = home_trial(&mut rester(None), Place::Home, 1).expect("lives");
        assert_eq!(answers, None);
    }

    #[test]
    fn a_place_named_answers_the_home_test() {
        // It always names the same place, so it says "home" everywhere.
        let right = Answers {
            asked: true,
            last: true,
        };
        let wrong = Answers {
            asked: false,
            last: false,
        };
        for (place, want) in [(Place::Home, right), (Place::Stranger(0), wrong)] {
            let answers = home_trial(&mut rester(Some(3)), place, 1).expect("lives");
            assert_eq!(answers, Some(want), "{}", place.name());
        }
    }

    #[test]
    fn an_agent_failure_ends_the_trial() {
        let mut failing = Rester {
            fail_at: Some(10),
            ..rester(Some(3))
        };
        match home_trial(&mut failing, Place::Home, 1) {
            Err(LifeError::Agent(e)) => assert_eq!((e.t, e.during), (10, Phase::Act)),
            other => panic!("expected the agent's failure, got {other:?}"),
        }
    }

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
            strangers_home: 0,
            // Bush first: bush, barren, bush, barren.
            bush_raised: stays([1, 10, 1, 10]),
            // Barren first: barren, bush, barren, bush.
            barren_raised: stays([100, 2, 100, 2]),
        };
        assert_eq!(life.raised_sleeps(true), 220);
        assert_eq!(life.raised_sleeps(false), 6);
    }
}
