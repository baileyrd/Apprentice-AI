//! Patterns: the slowest memory, and the only one that is not about a place.
//!
//! The maps say what is where in one world. Patterns say how squares change,
//! in any world: given what a square has looked like, and for how long, what
//! will it look like now? "A square I ate from is empty for a while, then
//! food again" is a pattern; "food grows at (3, 4)" is not.
//!
//! A square's **state** is how it last changed: it came to look like a kind
//! of square, or the apprentice ate from it (which leaves it looking empty).
//! The state keeps its start time until the square looks different, so its
//! age says how long the square has been like this. Ages are grouped by powers
//! of two, so the key is general rather than tuned to any one world.
//!
//! Patterns learn only in sleep, from the day's sightings replayed from the
//! diary, by a delta rule. Every world feeds the same table.

use crate::model::{Belief, KINDS, kind, one_hot};
use crate::world::Cell;

/// How a square last changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Last {
    /// It came to look like this.
    Saw(Cell),
    /// The apprentice ate from it.
    Ate,
}

impl Last {
    fn index(self) -> usize {
        match self {
            Last::Saw(cell) => kind(cell),
            Last::Ate => KINDS,
        }
    }

    /// What the square looks like while in this state.
    pub fn looks(self) -> Cell {
        match self {
            Last::Saw(cell) => cell,
            Last::Ate => Cell::Empty,
        }
    }
}

const STATES: usize = KINDS + 1;
/// Age groups: 1, 2–3, 4–7, … , 128 and over.
const AGES: usize = 8;
const UNIFORM: Belief = [1.0 / KINDS as f64; KINDS];

/// Age group of `age` steps: its power of two, capped.
fn age_group(age: u64) -> usize {
    (age.max(1).ilog2() as usize).min(AGES - 1)
}

/// What squares become, by state and age.
#[derive(Debug, Clone, PartialEq)]
pub struct Patterns {
    rate: f64,
    /// Samples a pattern needs before it predicts.
    min: u32,
    table: [[(Belief, u32); AGES]; STATES],
}

impl Patterns {
    /// Learns at `rate`; predicts once a pattern has `min` samples.
    pub fn new(rate: f64, min: u32) -> Self {
        Self {
            rate,
            min,
            table: [[(UNIFORM, 0); AGES]; STATES],
        }
    }

    /// A square in state `last`, `age` steps old, was seen as `sensed`.
    pub fn learn(&mut self, last: Last, age: u64, sensed: Cell) {
        let (belief, count) = &mut self.table[last.index()][age_group(age)];
        let target = one_hot(sensed);
        for k in 0..KINDS {
            belief[k] += self.rate * (target[k] - belief[k]);
        }
        *count = count.saturating_add(1);
    }

    /// What a square in state `last`, `age` steps old, is now; `None` until
    /// the pattern has enough samples.
    pub fn predict(&self, last: Last, age: u64) -> Option<Belief> {
        let (belief, count) = self.table[last.index()][age_group(age)];
        (count >= self.min).then_some(belief)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ages_group_by_powers_of_two() {
        let groups: Vec<usize> = [0, 1, 2, 3, 4, 31, 32, 127, 128, 10_000]
            .into_iter()
            .map(age_group)
            .collect();
        assert_eq!(groups, [0, 0, 1, 1, 2, 4, 5, 6, 7, 7]);
    }

    #[test]
    fn a_pattern_is_silent_until_it_has_enough_samples() {
        let mut p = Patterns::new(0.1, 3);
        p.learn(Last::Ate, 40, Cell::Food);
        p.learn(Last::Ate, 40, Cell::Food);
        assert_eq!(p.predict(Last::Ate, 40), None);
        p.learn(Last::Ate, 50, Cell::Food); // same age group
        assert!(p.predict(Last::Ate, 40).is_some());
    }

    #[test]
    fn it_learns_that_eaten_squares_grow_back_later_not_sooner() {
        let mut p = Patterns::new(0.1, 5);
        for _ in 0..50 {
            for age in 1..30 {
                p.learn(Last::Ate, age, Cell::Empty);
            }
            for age in 32..64 {
                p.learn(Last::Ate, age, Cell::Food);
            }
        }
        let food = |age| p.predict(Last::Ate, age).expect("learned")[kind(Cell::Food)];
        assert!(food(5) < 0.1, "{}", food(5));
        assert!(food(40) > 0.9, "{}", food(40));
        assert_eq!(p.predict(Last::Saw(Cell::Empty), 40), None);
    }
}
