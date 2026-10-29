//! Phase 1 world models.
//!
//! [`PredictiveCoding`] is the apprentice's learner: two layers, local updates
//! only. [`Backprop`] is the pre-registered fallback baseline: the same two
//! layers trained by the gradient of one global sensory loss.
//!
//! Predictive coding:
//! - **Layer 1** holds one belief per square it has seen: a distribution over
//!   cell kinds, keyed by path-integrated position. It predicts what the senses
//!   will report there and corrects itself by its own sensory error.
//! - **Layer 2** holds a prior: the kind distribution it expects of any square.
//!   It predicts layer 1 (and stands in for squares never seen) and corrects
//!   itself by its own error against layer 1.
//!
//! Every update is a local delta rule on one layer's own prediction error,
//! applied online at every step. No gradients flow between layers.

use std::collections::HashMap;

use crate::body::{Observation, Said};
use crate::world::{Cell, Pos};

/// Number of cell kinds.
pub const KINDS: usize = 4;

/// A distribution over cell kinds, indexed by [`kind`].
pub type Belief = [f64; KINDS];

/// Index of `cell` in a [`Belief`].
pub fn kind(cell: Cell) -> usize {
    match cell {
        Cell::Empty => 0,
        Cell::Wall => 1,
        Cell::Food => 2,
        Cell::Hazard => 3,
    }
}

const BY_KIND: [Cell; KINDS] = [Cell::Empty, Cell::Wall, Cell::Food, Cell::Hazard];

/// The certain belief that a square holds `cell`.
pub fn one_hot(cell: Cell) -> Belief {
    let mut b = [0.0; KINDS];
    b[kind(cell)] = 1.0;
    b
}

/// Squared prediction error between a belief and what was sensed.
pub fn error(predicted: &Belief, sensed: Cell) -> f64 {
    let target = one_hot(sensed);
    (0..KINDS).map(|k| (target[k] - predicted[k]).powi(2)).sum()
}

/// Learning rates of the two layers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rates {
    /// Layer 1: how far a square's belief moves toward what was sensed.
    pub sensory: f64,
    /// Layer 2: how far the prior moves toward each layer-1 belief.
    pub prior: f64,
}

impl Default for Rates {
    fn default() -> Self {
        Self {
            sensory: 0.5,
            prior: 0.01,
        }
    }
}

/// Something that predicts what the apprentice will sense, and learns from it.
pub trait WorldModel {
    /// Predicts the whole view, then learns from it.
    ///
    /// Returns the surprise: mean squared prediction error per square,
    /// measured before any update.
    fn observe(&mut self, obs: &Observation) -> f64 {
        let squares = obs.view.len() as f64;
        let mut surprise = 0.0;
        for (i, &sensed) in obs.view.iter().enumerate() {
            let (dx, dy) = Observation::view_delta(i);
            surprise += self.learn(obs.offset.shift(dx, dy), sensed);
        }
        surprise / squares
    }

    /// One square: predict, measure the error, learn. Returns the error.
    fn learn(&mut self, at: Pos, sensed: Cell) -> f64;

    /// Predicted kind distribution at `at` (relative to the wake square).
    fn predict(&self, at: Pos) -> Belief;

    /// Most likely kind at `at`, or `None` if the square was never seen.
    fn believes(&self, at: Pos) -> Option<Cell>;

    /// The body slept. Models with a memory consolidate here.
    fn sleep(&mut self) {}

    /// Eating at `at` fed the body. Models with a memory mark the moment.
    fn fed(&mut self, _at: Pos) {}

    /// A mentor said `said` about the square at `at`. Models with patterns
    /// learn what such squares become, and whom to believe.
    fn hear(&mut self, _at: Pos, _said: Said) {}

    /// Which of its own maps it is using; `None` if it keeps only one.
    fn map_in_use(&self) -> Option<usize> {
        None
    }
}

/// Most likely kind under `b`.
pub fn most_likely(b: &Belief) -> Option<Cell> {
    let best = (0..KINDS).max_by(|&i, &j| b[i].total_cmp(&b[j]))?;
    Some(BY_KIND[best])
}

const UNIFORM: Belief = [1.0 / KINDS as f64; KINDS];

/// The apprentice's learner: two-layer predictive coding.
#[derive(Debug, Clone, PartialEq)]
pub struct PredictiveCoding {
    rates: Rates,
    prior: Belief,
    cells: HashMap<Pos, Belief>,
}

impl PredictiveCoding {
    /// A blank model: uniform prior, nothing seen.
    pub fn new(rates: Rates) -> Self {
        Self {
            rates,
            prior: UNIFORM,
            cells: HashMap::new(),
        }
    }

    /// Layer 2's expectation for any square.
    pub fn prior(&self) -> Belief {
        self.prior
    }

    /// Whether `at` has ever been seen.
    pub fn knows(&self, at: Pos) -> bool {
        self.cells.contains_key(&at)
    }

    /// Every square it holds a belief about.
    pub fn known(&self) -> impl Iterator<Item = Pos> + '_ {
        self.cells.keys().copied()
    }

    /// Moves the belief at `at` a share `amount` of the way back to the prior.
    pub fn fade(&mut self, at: Pos, amount: f64) {
        let prior = self.prior;
        if let Some(b) = self.cells.get_mut(&at) {
            for (k, p) in b.iter_mut().enumerate() {
                *p += amount * (prior[k] - *p);
            }
        }
    }

    /// A blank model with this one's rates and prior: general expectations
    /// kept, particular squares unknown.
    pub fn schema(&self) -> Self {
        Self {
            rates: self.rates,
            prior: self.prior,
            cells: HashMap::new(),
        }
    }
}

impl WorldModel for PredictiveCoding {
    fn predict(&self, at: Pos) -> Belief {
        self.cells.get(&at).copied().unwrap_or(self.prior)
    }

    fn believes(&self, at: Pos) -> Option<Cell> {
        most_likely(self.cells.get(&at)?)
    }

    /// Layer 1 cancels its sensory error; layer 2 then cancels its error
    /// against the updated layer 1.
    fn learn(&mut self, at: Pos, sensed: Cell) -> f64 {
        let predicted = self.predict(at);
        let surprise = error(&predicted, sensed);
        let target = one_hot(sensed);
        let belief: Belief =
            std::array::from_fn(|k| predicted[k] + self.rates.sensory * (target[k] - predicted[k]));
        for (k, p) in self.prior.iter_mut().enumerate() {
            *p += self.rates.prior * (belief[k] - *p);
        }
        self.cells.insert(at, belief);
        surprise
    }
}

/// Fallback baseline: the same two layers, trained end to end by backprop.
///
/// A square's prediction is `prior + residual`. One global loss, the squared
/// sensory error, is differentiated with respect to both layers, so the prior
/// learns from the senses directly rather than from layer 1's state.
#[derive(Debug, Clone, PartialEq)]
pub struct Backprop {
    rates: Rates,
    prior: Belief,
    residuals: HashMap<Pos, Belief>,
}

impl Backprop {
    /// A blank model: uniform prior, nothing seen.
    pub fn new(rates: Rates) -> Self {
        Self {
            rates,
            prior: UNIFORM,
            residuals: HashMap::new(),
        }
    }
}

impl WorldModel for Backprop {
    fn predict(&self, at: Pos) -> Belief {
        let r = self.residuals.get(&at).copied().unwrap_or([0.0; KINDS]);
        std::array::from_fn(|k| self.prior[k] + r[k])
    }

    fn believes(&self, at: Pos) -> Option<Cell> {
        self.residuals
            .contains_key(&at)
            .then(|| most_likely(&self.predict(at)))?
    }

    /// Gradient step on `|sensed - (prior + residual)|²` for both layers.
    /// (The factor 2 of the gradient is folded into the rates.)
    fn learn(&mut self, at: Pos, sensed: Cell) -> f64 {
        let predicted = self.predict(at);
        let surprise = error(&predicted, sensed);
        let target = one_hot(sensed);
        let residual = self.residuals.entry(at).or_insert([0.0; KINDS]);
        for k in 0..KINDS {
            let grad = target[k] - predicted[k];
            residual[k] += self.rates.sensory * grad;
            self.prior[k] += self.rates.prior * grad;
        }
        surprise
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::VIEW;

    const ORIGIN: Pos = Pos { x: 0, y: 0 };

    fn view_of(cell: Cell) -> Observation {
        Observation {
            view: [cell; VIEW * VIEW],
            energy: 50,
            offset: ORIGIN,
        }
    }

    fn sums_to_one(b: Belief) -> bool {
        (b.iter().sum::<f64>() - 1.0).abs() < 1e-9
    }

    #[test]
    fn blank_model_is_maximally_unsure() {
        let m = PredictiveCoding::new(Rates::default());
        assert_eq!(m.predict(ORIGIN), [0.25; KINDS]);
        assert_eq!(m.believes(ORIGIN), None);
        // Uniform guess on a one-hot target: 0.75² + 3 × 0.25².
        assert!((error(&m.prior(), Cell::Empty) - 0.75).abs() < 1e-12);
    }

    #[test]
    fn repeated_sight_drives_surprise_to_zero() {
        let mut m = PredictiveCoding::new(Rates::default());
        let obs = view_of(Cell::Empty);
        let first = m.observe(&obs);
        let later = (0..20).map(|_| m.observe(&obs)).last().unwrap_or(first);
        assert!(first > 0.7);
        assert!(later < 1e-6, "surprise stayed at {later}");
        assert_eq!(m.believes(ORIGIN), Some(Cell::Empty));
    }

    #[test]
    fn walls_are_learned_where_they_are() {
        let mut m = PredictiveCoding::new(Rates::default());
        let mut obs = view_of(Cell::Empty);
        obs.view[0] = Cell::Wall; // north-west
        for _ in 0..10 {
            m.observe(&obs);
        }
        assert_eq!(m.believes(Pos { x: -1, y: -1 }), Some(Cell::Wall));
        assert_eq!(m.believes(Pos { x: 1, y: 1 }), Some(Cell::Empty));
    }

    #[test]
    fn unseen_squares_fall_back_to_the_learned_prior() {
        let mut m = PredictiveCoding::new(Rates::default());
        for _ in 0..200 {
            m.observe(&view_of(Cell::Empty));
        }
        let far = Pos { x: 50, y: -50 };
        assert_eq!(m.believes(far), None);
        let p = m.predict(far);
        assert_eq!(p, m.prior());
        assert!(
            p[kind(Cell::Empty)] > 0.9,
            "prior did not learn the base rate: {p:?}"
        );
    }

    #[test]
    fn empty_map_needs_no_more_surprise_once_seen() {
        let mut m = PredictiveCoding::new(Rates::default());
        let mut obs = view_of(Cell::Empty);
        for step in 0..30 {
            obs.offset = Pos { x: step, y: 0 };
            m.observe(&obs);
        }
        // Stepping east into an all-empty world: only the new column is
        // unseen, and the prior already says "empty".
        obs.offset = Pos { x: 30, y: 0 };
        assert!(m.observe(&obs) < 0.1);
    }

    #[test]
    fn a_square_that_changes_is_relearned() {
        let mut m = PredictiveCoding::new(Rates::default());
        let mut obs = view_of(Cell::Empty);
        obs.view[Observation::CENTER] = Cell::Food;
        for _ in 0..10 {
            m.observe(&obs);
        }
        obs.view[Observation::CENTER] = Cell::Empty; // eaten
        let shock = m.observe(&obs);
        for _ in 0..10 {
            m.observe(&obs);
        }
        assert!(
            shock > 1.0 / 9.0,
            "a vanished meal should surprise: {shock}"
        );
        assert_eq!(m.believes(ORIGIN), Some(Cell::Empty));
    }

    #[test]
    fn beliefs_and_prior_stay_distributions() {
        let mut m = PredictiveCoding::new(Rates::default());
        let mut obs = view_of(Cell::Empty);
        for (i, cell) in [Cell::Wall, Cell::Food, Cell::Hazard]
            .into_iter()
            .cycle()
            .take(30)
            .enumerate()
        {
            obs.view[i % 9] = cell;
            m.observe(&obs);
        }
        assert!(sums_to_one(m.prior()));
        assert!((0..9).all(|i| {
            let (dx, dy) = Observation::view_delta(i);
            sums_to_one(m.predict(ORIGIN.shift(dx, dy)))
        }));
    }
}
