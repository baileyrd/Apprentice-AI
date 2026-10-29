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
//! keyed on the word and the voice that said it. So a word comes to mean what
//! squares that speaker said it about went on to become: "this one grows
//! back", if that is what keeps happening.
//!
//! **Trust.** When a speaker's word and the general pattern disagree about
//! what a square will most likely be, the sighting settles it: trust in that
//! voice moves toward 1 if the word was right, toward 0 if not. Sightings
//! where they agree say nothing about the speaker. A told square's belief is the
//! general pattern moved toward the word's by that trust, so a speaker whose
//! words add nothing ends up carrying little weight.

use std::collections::HashMap;
use std::num::NonZeroUsize;

use crate::body::{Said, Voice};
use crate::model::{Belief, KINDS, kind, most_likely, one_hot};
use crate::network::{Learning, Net};
use crate::world::Cell;

/// How a square last changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// The youngest age in `age`'s group: every age in a group teaches and
/// predicts the same pattern.
pub fn group_start(age: u64) -> u64 {
    1 << age_group(age)
}

/// Where the general patterns are held.
#[derive(Debug, Clone, PartialEq)]
enum Store {
    /// One belief per state and age group, each learned by a delta rule.
    Table(Box<[[Belief; AGES]; STATES]>),
    /// One network for every state and age (see [`PatternNet`]).
    Net(Box<Net>),
}

/// A network for the general patterns: one hidden layer of `hidden` units,
/// learning at `rate` by predictive coding, weights drawn from `seed`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PatternNet {
    pub hidden: NonZeroUsize,
    pub rate: f64,
    pub seed: u64,
}

impl Default for PatternNet {
    /// The size and rate tuned on seeds 101-110 (docs/patterns-net).
    fn default() -> Self {
        Self {
            hidden: NonZeroUsize::new(64).unwrap_or(NonZeroUsize::MIN),
            rate: 0.2,
            seed: 0,
        }
    }
}

/// Units the pattern network sees: the state, the age group, and the age as
/// one smooth number.
const FEATURES: usize = STATES + AGES + 1;

/// How a pattern network learns: settling with firm hidden units, as in the
/// world-physics network.
const NET_LEARNING: Learning = Learning::PredictiveCoding {
    rounds: 20,
    step: 0.2,
    precision: 100.0,
};

/// What the network sees of a square in state `last`, `age` steps old.
fn features(last: Last, age: u64) -> Vec<f64> {
    let mut x = vec![0.0; FEATURES];
    x[last.index()] = 1.0;
    x[STATES + age_group(age)] = 1.0;
    x[FEATURES - 1] = ((age.max(1) as f64).log2() / AGES as f64).min(1.0);
    x
}

/// A network's output read as a belief: negatives dropped, the rest scaled
/// to sum to 1 (uniform if nothing is left).
fn as_belief(output: &[f64]) -> Belief {
    let b: Belief = std::array::from_fn(|k| output.get(k).copied().unwrap_or(0.0).max(0.0));
    let sum: f64 = b.iter().sum();
    if sum <= 0.0 {
        return UNIFORM;
    }
    b.map(|p| p / sum)
}

/// What squares become, by state and age.
#[derive(Debug, Clone, PartialEq)]
pub struct Patterns {
    rate: f64,
    /// Samples a pattern needs before it predicts.
    min: u32,
    general: Store,
    /// Samples seen per state and age group: the evidence behind each
    /// general pattern, whatever holds it.
    counts: [[u32; AGES]; STATES],
    /// The same, for squares a word was said about, by word and voice.
    worded: HashMap<(Said, usize, usize), (Belief, u32)>,
    /// How far each voice's words are believed.
    trust: HashMap<Voice, f64>,
}

/// Rate at which a settled disagreement moves trust in a voice. Faster than
/// patterns learn, since disagreements are rare.
const TRUST_RATE: f64 = 0.2;

/// Trust in a voice not yet heard.
const STRANGER: f64 = 0.5;

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
            general: Store::Table(Box::new([[UNIFORM; AGES]; STATES])),
            counts: [[0; AGES]; STATES],
            worded: HashMap::new(),
            trust: HashMap::new(),
        }
    }

    /// The same, with the general patterns held in a network instead of a
    /// table. Word patterns stay a table.
    pub fn with_net(rate: f64, min: u32, net: PatternNet) -> Self {
        let size = |n: usize| NonZeroUsize::new(n).unwrap_or(NonZeroUsize::MIN);
        let sizes = [size(FEATURES), net.hidden, size(KINDS)];
        Self {
            general: Store::Net(Box::new(Net::one_hidden(
                sizes,
                net.rate,
                NET_LEARNING,
                net.seed,
            ))),
            ..Self::new(rate, min)
        }
    }

    /// How far words in `voice` are believed, from 0 to 1.
    pub fn trust(&self, voice: Voice) -> f64 {
        self.trust.get(&voice).copied().unwrap_or(STRANGER)
    }

    /// A square in state `last`, `age` steps old, and told `said` if
    /// anything, was seen as `sensed`.
    pub fn learn(&mut self, last: Last, age: u64, said: Option<Said>, sensed: Cell) {
        if let Some(said) = said {
            self.judge(last, age, said, sensed);
        }
        let (state, group) = (last.index(), age_group(age));
        let count = &mut self.counts[state][group];
        *count = count.saturating_add(1);
        match &mut self.general {
            Store::Table(table) => {
                let target = one_hot(sensed);
                let belief = &mut table[state][group];
                for k in 0..KINDS {
                    belief[k] += self.rate * (target[k] - belief[k]);
                }
            }
            Store::Net(net) => net.learn(&features(last, age), &one_hot(sensed)),
        }
        if let Some(said) = said {
            let slot = self
                .worded
                .entry((said, state, group))
                .or_insert((UNIFORM, 0));
            nudge(slot, self.rate, sensed);
        }
    }

    /// Where the word of `said` and the general pattern disagree about what
    /// is most likely, moves trust in its speaker by whether the word was
    /// right about `sensed`.
    fn judge(&mut self, last: Last, age: u64, said: Said, sensed: Cell) {
        let general = self.general(last, age);
        let (Some(general), Some(told)) = (general, self.foretold(last, age, said)) else {
            return;
        };
        let told = most_likely(&told);
        if told == most_likely(&general) {
            return;
        }
        let hit = if told == Some(sensed) { 1.0 } else { 0.0 };
        let trust = self.trust.entry(said.voice).or_insert(STRANGER);
        *trust += TRUST_RATE * (hit - *trust);
    }

    /// What a square in state `last`, `age` steps old, and told `said` if
    /// anything, is now: the general pattern moved toward the word's pattern
    /// by trust in the speaker; `None` until a pattern has enough samples.
    pub fn predict(&self, last: Last, age: u64, said: Option<Said>) -> Option<Belief> {
        let general = self.general(last, age);
        let Some((said, told)) = said.and_then(|s| Some((s, self.foretold(last, age, s)?))) else {
            return general;
        };
        let base = general.unwrap_or(UNIFORM);
        let trust = self.trust(said.voice);
        Some(std::array::from_fn(|k| {
            base[k] + trust * (told[k] - base[k])
        }))
    }

    /// What the general pattern alone says; `None` until it has enough
    /// samples.
    fn general(&self, last: Last, age: u64) -> Option<Belief> {
        let (state, group) = (last.index(), age_group(age));
        if self.counts[state][group] < self.min {
            return None;
        }
        Some(match &self.general {
            Store::Table(table) => table[state][group],
            Store::Net(net) => as_belief(&net.predict(&features(last, age))),
        })
    }

    /// What the pattern of `said` alone says of a square in state `last`,
    /// `age` steps old; `None` until it has enough samples.
    pub fn foretold(&self, last: Last, age: u64, said: Said) -> Option<Belief> {
        let (belief, count) = *self.worded.get(&(said, last.index(), age_group(age)))?;
        (count >= self.min).then_some(belief)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(voice: u8, word: u8) -> Said {
        Said {
            voice: Voice(voice),
            word: crate::body::Word(word),
        }
    }

    #[test]
    fn an_honest_voice_is_trusted_and_a_random_one_is_not() {
        let (honest, random) = (Voice(1), Voice(2));
        let mut p = Patterns::new(0.05, 5);
        for i in 0..2_000 {
            let grows = i % 2 == 0;
            let sensed = if grows { Cell::Food } else { Cell::Empty };
            let truth = said(1, if grows { 4 } else { 5 });
            let noise = said(2, if i % 3 == 0 { 4 } else { 5 });
            p.learn(Last::Ate, 40, Some(truth), sensed);
            p.learn(Last::Ate, 40, Some(noise), sensed);
        }
        assert!(p.trust(honest) > 0.9, "{}", p.trust(honest));
        assert!(p.trust(random) < 0.6, "{}", p.trust(random));
        assert_eq!(p.trust(Voice(3)), STRANGER);
    }

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

    fn net(hidden: usize, rate: f64) -> PatternNet {
        PatternNet {
            hidden: NonZeroUsize::new(hidden).expect("non-zero"),
            rate,
            seed: 1,
        }
    }

    #[test]
    fn a_network_learns_regrowth_too_and_keeps_the_evidence_gate() {
        let mut p = Patterns::with_net(0.1, 5, net(16, 0.05));
        p.learn(Last::Ate, 40, None, Cell::Food);
        assert_eq!(
            p.predict(Last::Ate, 40, None),
            None,
            "one sample is not enough"
        );
        for _ in 0..200 {
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
        let b = p.predict(Last::Ate, 40, None).expect("learned");
        assert!((b.iter().sum::<f64>() - 1.0).abs() < 1e-9 && b.iter().all(|&x| x >= 0.0));
    }

    #[test]
    fn a_belief_from_a_network_drops_negatives() {
        assert_eq!(as_belief(&[-1.0, 1.0, 1.0, 0.0]), [0.0, 0.5, 0.5, 0.0]);
        assert_eq!(as_belief(&[-1.0; KINDS]), UNIFORM);
    }

    #[test]
    fn a_word_means_what_its_squares_became() {
        let (back, gone) = (said(0, 4), said(0, 5));
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
        p.learn(Last::Ate, 40, Some(said(0, 9)), Cell::Empty);
        assert_eq!(
            p.predict(Last::Ate, 40, Some(said(0, 9))),
            p.predict(Last::Ate, 40, None)
        );
    }
}
