//! The agent port plus two baselines that later learners must beat.

use serde::{Deserialize, Serialize};

use crate::body::{Action, Observation};
use crate::rng::Rng;
use crate::sim::Outcome;
use crate::world::{Cell, Dir};

/// Anything that can live in the world.
pub trait Agent {
    /// Chooses the next action from what it senses.
    fn act(&mut self, obs: &Observation) -> Action;

    /// Hears what its last action did. Baselines ignore it; learners won't.
    fn feel(&mut self, _outcome: &Outcome) {}
}

/// Uniformly random actions. The floor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RandomAgent {
    rng: Rng,
}

impl RandomAgent {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: Rng::new(seed),
        }
    }
}

impl Agent for RandomAgent {
    fn act(&mut self, _obs: &Observation) -> Action {
        match self.rng.below(6) {
            0 => Action::Eat,
            1 => Action::Rest,
            n => Action::Move(Dir::ALL[n - 2]),
        }
    }
}

/// Hand-coded reflexes: eat food underfoot, step to adjacent food, otherwise
/// wander while avoiding walls and hazards. Knows the rules, learns nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReflexAgent {
    rng: Rng,
}

impl ReflexAgent {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: Rng::new(seed),
        }
    }
}

impl Agent for ReflexAgent {
    fn act(&mut self, obs: &Observation) -> Action {
        if obs.here() == Cell::Food {
            return Action::Eat;
        }
        let toward = |want: fn(Cell) -> bool| -> Vec<Dir> {
            Dir::ALL
                .into_iter()
                .filter(|&d| want(obs.toward(d)))
                .collect()
        };
        let food = toward(|c| c == Cell::Food);
        let safe = toward(|c| matches!(c, Cell::Empty | Cell::Food));
        let choice = self.rng.pick(&food).or_else(|| self.rng.pick(&safe));
        choice.map_or(Action::Rest, Action::Move)
    }
}

/// Closed set of built-in agents, so a checkpoint can hold whichever one ran.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Baseline {
    Random(RandomAgent),
    Reflex(ReflexAgent),
}

impl Agent for Baseline {
    fn act(&mut self, obs: &Observation) -> Action {
        match self {
            Baseline::Random(a) => a.act(obs),
            Baseline::Reflex(a) => a.act(obs),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs_with(here: Cell, east: Cell) -> Observation {
        let mut view = [Cell::Wall; 9];
        view[Observation::CENTER] = here;
        view[5] = east;
        Observation { view, energy: 50 }
    }

    #[test]
    fn reflex_eats_food_underfoot() {
        let mut a = ReflexAgent::new(0);
        assert_eq!(a.act(&obs_with(Cell::Food, Cell::Empty)), Action::Eat);
    }

    #[test]
    fn reflex_steps_toward_visible_food() {
        let mut a = ReflexAgent::new(0);
        assert_eq!(
            a.act(&obs_with(Cell::Empty, Cell::Food)),
            Action::Move(Dir::East)
        );
    }

    #[test]
    fn reflex_avoids_hazards_and_rests_when_boxed_in() {
        let mut a = ReflexAgent::new(0);
        assert_eq!(a.act(&obs_with(Cell::Empty, Cell::Hazard)), Action::Rest);
    }

    #[test]
    fn random_uses_every_action() {
        let mut a = RandomAgent::new(9);
        let obs = obs_with(Cell::Empty, Cell::Empty);
        let actions: Vec<Action> = (0..200).map(|_| a.act(&obs)).collect();
        assert!(actions.contains(&Action::Eat));
        assert!(actions.contains(&Action::Rest));
        assert!(Dir::ALL.iter().all(|&d| actions.contains(&Action::Move(d))));
    }
}
