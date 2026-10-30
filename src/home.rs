//! Home: a small world the apprentice knows well, and the worlds it is
//! tested in.
//!
//! It lives `HOME_STEPS` in a small bush world. A trial then puts it, as it
//! is at that moment, in home as it left it, home with one wall moved, or a
//! stranger built to the same recipe. It is never told which; "is this
//! home?" is answered from its own catalog of maps.

use crate::adaptation::BUSH_DELAY;
use crate::apprentice::{Apprentice, Drives, Mind};
use crate::memory::{Memory, Recall};
use crate::model::Rates;
use crate::rng::Rng;
use crate::sim::{Config, Regrowth, Sim, SimError};
use crate::world::{Grid, Layout};

/// The apprentice with memory and patterns.
pub type Learner = Apprentice<Memory>;

/// Steps it lives at home before a trial.
pub const HOME_STEPS: usize = 3_000;

/// What a trial puts it in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Home,
    /// Home with one wall moved.
    Changed,
    /// The `k`th stranger for a home.
    Stranger(u64),
}

impl Place {
    /// Whether the right answer is "home".
    pub fn is_home(self) -> bool {
        !matches!(self, Place::Stranger(_))
    }

    pub fn name(self) -> &'static str {
        match self {
            Place::Home => "home",
            Place::Changed => "changed home",
            Place::Stranger(_) => "stranger",
        }
    }
}

/// Home's recipe: 6×6, a few walls and hazards, 3 bushes that grow back.
pub fn config() -> Config {
    Config {
        layout: Layout {
            width: 6,
            height: 6,
            wall_density: 0.12,
            hazard_density: 0.05,
            food_count: 3,
        },
        regrowth: Regrowth::InPlace { delay: BUSH_DELAY },
        ..Config::default()
    }
}

/// One step of `mind` in `sim`.
pub fn step(sim: &mut Sim, mind: &mut Learner) {
    let outcome = sim.step(mind.act(&sim.observe()));
    mind.feel(&outcome);
}

/// A fresh apprentice and its home, before it has lived there.
pub fn newborn(seed: u64) -> Result<(Learner, Sim), SimError> {
    newborn_with(Recall::default(), seed)
}

/// The same, with memory set by `recall`.
pub fn newborn_with(recall: Recall, seed: u64) -> Result<(Learner, Sim), SimError> {
    newborn_as(recall, Drives::default(), seed)
}

/// The same, with memory set by `recall` and wants by `drives`.
pub fn newborn_as(recall: Recall, drives: Drives, seed: u64) -> Result<(Learner, Sim), SimError> {
    let memory = Memory::new(Rates::default(), recall);
    let mind = Apprentice::with_model(drives, memory, seed);
    Ok((mind, Sim::new(config(), seed)?))
}

/// A life at home: the mind, the home as it left it, and its home map.
pub fn at_home(seed: u64) -> Result<(Learner, Sim, Option<usize>), SimError> {
    at_home_with(Recall::default(), seed)
}

/// The same, with memory set by `recall`.
pub fn at_home_with(recall: Recall, seed: u64) -> Result<(Learner, Sim, Option<usize>), SimError> {
    at_home_as(recall, Drives::default(), seed)
}

/// The same, with memory set by `recall` and wants by `drives`.
pub fn at_home_as(
    recall: Recall,
    drives: Drives,
    seed: u64,
) -> Result<(Learner, Sim, Option<usize>), SimError> {
    let (mut mind, mut sim) = newborn_as(recall, drives, seed)?;
    for _ in 0..HOME_STEPS {
        step(&mut sim, &mut mind);
    }
    let home_map = mind.map_in_use();
    Ok((mind, sim, home_map))
}

/// The world a trial puts it in; strangers wake it on home's wake square.
pub fn world(place: Place, home: &Sim, seed: u64) -> Sim {
    match place {
        Place::Home => home.clone(),
        Place::Changed => {
            let mut changed = home.clone();
            changed.move_walls(1);
            changed
        }
        Place::Stranger(k) => {
            let mut rng = Rng::new(seed * 100 + k + 1);
            let grid = Grid::generate(&config().layout, &mut rng);
            Sim::with_start(config(), grid, home.start(), rng)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stranger_wakes_it_on_homes_wake_square() {
        let home = Sim::new(config(), 1).expect("room");
        let stranger = world(Place::Stranger(0), &home, 1);
        assert_eq!(stranger.start(), home.start());
        assert_ne!(stranger.grid(), home.grid());
    }

    #[test]
    fn a_changed_home_differs_by_one_wall() {
        let home = Sim::new(config(), 1).expect("room");
        let changed = world(Place::Changed, &home, 1);
        let differ = home
            .grid()
            .positions()
            .filter(|&p| home.grid().get(p) != changed.grid().get(p))
            .count();
        assert_eq!(differ, 2, "one wall gone, one wall new");
    }
}
