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
//!
//! **Words.** A mentor's word said about a square joins that square's state
//! until it next looks different, and the sighting also teaches a pattern
//! keyed on the word. So a word comes to mean what squares it was said about
//! went on to become: "this one grows back", if that is what keeps happening.
//! A word whose pattern is learned answers before the general one.

use std::collections::HashMap;

use crate::body::Word;
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
    /// The same, for squares a word was said about.
    worded: HashMap<(Word, usize, usize), (Belief, u32)>,
}

/// Moves `slot` toward `sensed` at `rate` and counts the sample.
fn nudge(slot: &mut (Belief, u32), rate: f64, sensed: Cell) {
    let (belief, count) = slot;
    let target = one_hot(sensed);
    for k in 0..KINDS {
        belief[k] += rate * (target[k] - belief[k]);
    }
    *count = count.saturating_add(1);
}

impl Patterns {
    /// Learns at `rate`; predicts once a pattern has `min` samples.
    pub fn new(rate: f64, min: u32) -> Self {
        Self {
            rate,
            min,
            table: [[(UNIFORM, 0); AGES]; STATES],
            worded: HashMap::new(),
        }
    }

    /// A square in state `last`, `age` steps old, and told `word` if any, was
    /// seen as `sensed`.
    pub fn learn(&mut self, last: Last, age: u64, word: Option<Word>, sensed: Cell) {
        let (state, group) = (last.index(), age_group(age));
        nudge(&mut self.table[state][group], self.rate, sensed);
        if let Some(word) = word {
            let slot = self
                .worded
                .entry((word, state, group))
                .or_insert((UNIFORM, 0));
            nudge(slot, self.rate, sensed);
        }
    }

    /// What a square in state `last`, `age` steps old, and told `word` if
    /// any, is now: the word's pattern if learned, else the general one;
    /// `None` until a pattern has enough samples.
    pub fn predict(&self, last: Last, age: u64, word: Option<Word>) -> Option<Belief> {
        if let Some(told) = word.and_then(|w| self.foretold(last, age, w)) {
            return Some(told);
        }
        let (belief, count) = self.table[last.index()][age_group(age)];
        (count >= self.min).then_some(belief)
    }

    /// What `word`'s pattern alone says of a square in state `last`, `age`
    /// steps old; `None` until it has enough samples.
    pub fn foretold(&self, last: Last, age: u64, word: Word) -> Option<Belief> {
        let (belief, count) = *self.worded.get(&(word, last.index(), age_group(age)))?;
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
        p.learn(Last::Ate, 40, None, Cell::Food);
        p.learn(Last::Ate, 40, None, Cell::Food);
        assert_eq!(p.predict(Last::Ate, 40, None), None);
        p.learn(Last::Ate, 50, None, Cell::Food); // same age group
        assert!(p.predict(Last::Ate, 40, None).is_some());
    }

    #[test]
    fn it_learns_that_eaten_squares_grow_back_later_not_sooner() {
        let mut p = Patterns::new(0.1, 5);
        for _ in 0..50 {
            for age in 1..30 {
                p.learn(Last::Ate, age, None, Cell::Empty);
            }
            for age in 32..64 {
                p.learn(Last::Ate, age, None, Cell::Food);
            }
        }
        let food = |age| p.predict(Last::Ate, age, None).expect("learned")[kind(Cell::Food)];
        assert!(food(5) < 0.1, "{}", food(5));
        assert!(food(40) > 0.9, "{}", food(40));
        assert_eq!(p.predict(Last::Saw(Cell::Empty), 40, None), None);
    }

    #[test]
    fn a_word_means_what_its_squares_became() {
        let (back, gone) = (Word(4), Word(5));
        let mut p = Patterns::new(0.1, 5);
        for _ in 0..50 {
            p.learn(Last::Ate, 40, Some(back), Cell::Food);
            p.learn(Last::Ate, 40, Some(gone), Cell::Empty);
        }
        let food = |w| p.predict(Last::Ate, 40, w).expect("learned")[kind(Cell::Food)];
        assert!(food(Some(back)) > 0.9, "{}", food(Some(back)));
        assert!(food(Some(gone)) < 0.1, "{}", food(Some(gone)));
        let general = food(None);
        assert!((0.3..0.7).contains(&general), "{general}");
    }

    #[test]
    fn an_unlearned_word_falls_back_to_the_general_pattern() {
        let mut p = Patterns::new(0.1, 5);
        for _ in 0..10 {
            p.learn(Last::Ate, 40, None, Cell::Food);
        }
        p.learn(Last::Ate, 40, Some(Word(9)), Cell::Empty);
        assert_eq!(
            p.predict(Last::Ate, 40, Some(Word(9))),
            p.predict(Last::Ate, 40, None)
        );
    }
}
