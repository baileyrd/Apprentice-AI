//! The apprentice chooses its own actions (phase 2 onward).
//!
//! Everything it acts on is learned from its own experience:
//! - **where things are**: the phase 1 predictive-coding world model;
//! - **what things are worth** (valence): the energy change it felt after
//!   entering or eating each kind of square, by a delta rule;
//! - **how fast its world changes** (volatility): the error it finds on
//!   revisiting a square, per step it was away, by a delta rule.
//!
//! Each step it scores every action by its homeostatic drive (hunger weighs
//! food) plus a curiosity bonus (squares it is unsure of, or has not looked at
//! for a while, in a world it has learned is volatile), planned over the
//! believed map. No gradients, no ML library: delta rules and a shortest-path
//! search.
//!
//! A mentor may say a word about a square ([`Apprentice::hear`]). Words start
//! with no meaning. When a square it was told about comes into view, the
//! apprentice moves that word's meaning (a belief over kinds of square)
//! toward what it sees, by a delta rule. Told words never enter the world
//! model: while planning, a told square only gains the food its word means.
//! So a mentor that names what the apprentice is looking at teaches it words,
//! and a mentor whose words keep proving wrong is ignored.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use crate::agent::Agent;
use crate::body::{Action, Observation, Word};
use crate::model::{Belief, KINDS, PredictiveCoding, Rates, WorldModel, error, kind, one_hot};
use crate::rng::Rng;
use crate::sim::Outcome;
use crate::world::{Cell, Dir, Pos};

/// A body's controller that also learns a world model.
pub trait Mind {
    /// Senses, learns from what it sensed, then chooses.
    fn act(&mut self, obs: &Observation) -> Action;
    /// Hears what its last action did.
    fn feel(&mut self, outcome: &Outcome);
    /// Surprise at the last observation.
    fn surprise(&self) -> f64;
    /// Most likely kind at `at` (relative to the wake square); `None` if unseen.
    fn believes(&self, at: Pos) -> Option<Cell>;
}

/// A fixed policy with a world model watching: the comparison for phase 2.
pub struct Watched<A> {
    agent: A,
    model: PredictiveCoding,
    surprise: f64,
}

impl<A: Agent> Watched<A> {
    pub fn new(agent: A, rates: Rates) -> Self {
        Self {
            agent,
            model: PredictiveCoding::new(rates),
            surprise: 0.0,
        }
    }
}

impl<A: Agent> Mind for Watched<A> {
    fn act(&mut self, obs: &Observation) -> Action {
        self.surprise = self.model.observe(obs);
        self.agent.act(obs)
    }

    fn feel(&mut self, outcome: &Outcome) {
        self.agent.feel(outcome);
    }

    fn surprise(&self) -> f64 {
        self.surprise
    }

    fn believes(&self, at: Pos) -> Option<Cell> {
        self.model.believes(at)
    }
}

/// How the apprentice weighs what it wants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drives {
    /// Energy it tries to stay at; hunger grows below it.
    pub setpoint: f64,
    /// Weight of curiosity against food. Zero gives a drive-only apprentice.
    pub curiosity: f64,
    /// Value kept per step of distance to a goal.
    pub discount: f64,
    /// Furthest a plan reaches, in steps.
    pub horizon: u32,
    /// Delta-rule rate for valence.
    pub learning: f64,
    /// Delta-rule rate for volatility: slow, since changes are rare.
    pub vigilance: f64,
    /// Energy units per unit of utility.
    pub scale: f64,
    /// Energy it expects from eating a kind of square it has never tried.
    pub optimism: f64,
}

impl Default for Drives {
    fn default() -> Self {
        Self {
            setpoint: 80.0,
            curiosity: 0.5,
            discount: 0.97,
            horizon: 20,
            learning: 0.2,
            vigilance: 0.01,
            scale: 25.0,
            optimism: 25.0,
        }
    }
}

/// Learned energy change per kind of square.
#[derive(Debug, Clone, PartialEq)]
struct Valence {
    enter: Belief,
    eat: Belief,
    rest: f64,
}

impl Valence {
    /// Neutral about moving and resting; optimistic about eating, so it
    /// tries to eat each kind of square until it learns which ones feed it.
    fn new(optimism: f64) -> Self {
        Self {
            enter: [0.0; KINDS],
            eat: [optimism; KINDS],
            rest: 0.0,
        }
    }
}

/// What it tried last, to credit the energy change it then feels.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Tried {
    Enter(usize),
    Eat(usize),
    Rest,
}

/// The learner: acts on its own learned model, valence and volatility.
/// Phase 2 and 3 use [`PredictiveCoding`]; phase 4 a [`crate::memory::Memory`].
pub struct Apprentice<M = PredictiveCoding> {
    model: M,
    drives: Drives,
    rng: Rng,
    valence: Valence,
    volatility: f64,
    last_seen: HashMap<Pos, u64>,
    t: u64,
    /// What it tried last, its energy before, and where it stood.
    pending: Option<(Tried, f64, Pos)>,
    surprise: f64,
    /// Squares a mentor said something about, not yet checked.
    told: HashMap<Pos, Word>,
    /// What each word has turned out to mean.
    lexicon: HashMap<Word, Belief>,
}

/// Meaning of a word on its first check, before what was seen is learned.
const UNIFORM: Belief = [1.0 / KINDS as f64; KINDS];

impl Apprentice {
    /// An apprentice with the phase 1 world model and no memory.
    pub fn new(drives: Drives, rates: Rates, seed: u64) -> Self {
        Self::with_model(drives, PredictiveCoding::new(rates), seed)
    }
}

impl<M: WorldModel> Apprentice<M> {
    /// An apprentice that learns its world with `model`.
    pub fn with_model(drives: Drives, model: M, seed: u64) -> Self {
        Self {
            model,
            drives,
            rng: Rng::new(seed),
            valence: Valence::new(drives.optimism),
            volatility: 0.0,
            last_seen: HashMap::new(),
            t: 0,
            pending: None,
            surprise: 0.0,
            told: HashMap::new(),
            lexicon: HashMap::new(),
        }
    }

    /// A mentor says `word` about the square at `at` (relative to the wake
    /// square).
    pub fn hear(&mut self, at: Pos, word: Word) {
        self.told.insert(at, word);
    }

    /// What `word` has turned out to mean; `None` if never checked.
    pub fn meaning(&self, word: Word) -> Option<Belief> {
        self.lexicon.get(&word).copied()
    }

    /// Checks told squares now in view, learning what their words mean.
    fn check_told(&mut self, obs: &Observation) {
        let rate = self.drives.learning;
        for (i, &sensed) in obs.view.iter().enumerate() {
            let (dx, dy) = Observation::view_delta(i);
            let Some(word) = self.told.remove(&obs.offset.shift(dx, dy)) else {
                continue;
            };
            let meaning = self.lexicon.entry(word).or_insert(UNIFORM);
            let target = one_hot(sensed);
            for k in 0..KINDS {
                meaning[k] += rate * (target[k] - meaning[k]);
            }
        }
    }

    /// Its world model.
    pub fn model(&self) -> &M {
        &self.model
    }

    /// Learned error per step away from a square.
    pub fn volatility(&self) -> f64 {
        self.volatility
    }

    /// Learns how stale beliefs go wrong, from the squares now in view.
    fn learn_volatility(&mut self, obs: &Observation) {
        for (i, &sensed) in obs.view.iter().enumerate() {
            let (dx, dy) = Observation::view_delta(i);
            let at = obs.offset.shift(dx, dy);
            // Only squares coming back into view: one that never left it
            // says nothing about how fast an unwatched world changes.
            let away = self.last_seen.get(&at).map_or(0, |&last| self.t - last);
            if away > 1 {
                let rate = error(&self.model.predict(at), sensed) / away as f64;
                self.volatility += self.drives.vigilance * (rate - self.volatility);
            }
            self.last_seen.insert(at, self.t);
        }
    }

    fn hunger(&self, energy: u32) -> f64 {
        ((self.drives.setpoint - f64::from(energy)) / self.drives.setpoint).clamp(0.0, 1.0)
    }

    /// How much there is to learn by looking at `at`.
    fn novelty(&self, at: Pos) -> f64 {
        let b = self.model.predict(at);
        let unsure = 1.0 - b.iter().map(|p| p * p).sum::<f64>();
        let stale = match self.last_seen.get(&at) {
            Some(&last) => (self.volatility * (self.t - last) as f64).min(1.0),
            None => 1.0,
        };
        unsure + stale
    }

    /// Expected energy gained by eating at a square believed to be `b`.
    fn food(&self, b: &Belief) -> f64 {
        (0..KINDS)
            .map(|k| b[k] * self.valence.eat[k].max(0.0))
            .sum()
    }

    /// Expected energy lost by stepping onto a square believed to be `b`,
    /// beyond what resting would have cost anyway.
    fn toll(&self, b: &Belief) -> f64 {
        let rest = self.valence.rest;
        (0..KINDS)
            .map(|k| b[k] * (rest - self.valence.enter[k]).max(0.0))
            .sum()
    }

    fn utility(&self, at: Pos, hunger: f64) -> f64 {
        let b = self.model.predict(at);
        let mut food = self.food(&b);
        if let Some(meaning) = self.told.get(&at).and_then(|&w| self.meaning(w)) {
            food += self.food(&meaning);
        }
        hunger * food / self.drives.scale + self.drives.curiosity * self.novelty(at)
    }

    /// Best discounted utility reachable through each first step.
    fn plan(&self, obs: &Observation, hunger: f64) -> [Option<f64>; 4] {
        let d = &self.drives;
        let mut best = [None; 4];
        let mut done: HashSet<Pos> = HashSet::new();
        let mut heap = BinaryHeap::new();
        for (i, dir) in Dir::ALL.into_iter().enumerate() {
            if obs.toward(dir) == Cell::Wall {
                continue;
            }
            let at = obs.offset.step(dir);
            let cost = self.toll(&self.model.predict(at)) / d.scale;
            heap.push(Node {
                cost,
                steps: 1,
                at,
                first: i,
            });
        }
        while let Some(n) = heap.pop() {
            if !done.insert(n.at) {
                continue;
            }
            let value = d.discount.powi(n.steps as i32) * self.utility(n.at, hunger) - n.cost;
            let slot = &mut best[n.first];
            *slot = Some(slot.map_or(value, |v: f64| v.max(value)));
            if n.steps >= d.horizon {
                continue;
            }
            for dir in Dir::ALL {
                let at = n.at.step(dir);
                let b = self.model.predict(at);
                if done.contains(&at) || b[kind(Cell::Wall)] > 0.5 {
                    continue;
                }
                let cost = n.cost + self.toll(&b) / d.scale;
                heap.push(Node {
                    cost,
                    steps: n.steps + 1,
                    at,
                    first: n.first,
                });
            }
        }
        best
    }

    fn choose(&mut self, obs: &Observation) -> (Action, Tried) {
        let d = self.drives;
        let hunger = self.hunger(obs.energy);
        let here = kind(obs.here());
        let eat = self.valence.eat[here];
        let eat_value = if eat > 0.0 { hunger * eat } else { eat } / d.scale;
        let mut options = vec![
            (Action::Rest, Tried::Rest, self.valence.rest / d.scale),
            (Action::Eat, Tried::Eat(here), eat_value),
        ];
        let plans = self.plan(obs, hunger);
        for (i, dir) in Dir::ALL.into_iter().enumerate() {
            let target = kind(obs.toward(dir));
            let value = match plans[i] {
                Some(v) => v,
                None => self.valence.enter[target] / d.scale,
            };
            options.push((Action::Move(dir), Tried::Enter(target), value));
        }
        let mut best = (Action::Rest, Tried::Rest, f64::NEG_INFINITY);
        for (action, tried, value) in options {
            let jittered = value + 1e-9 * self.rng.unit();
            if jittered > best.2 {
                best = (action, tried, jittered);
            }
        }
        (best.0, best.1)
    }
}

impl<M: WorldModel> Mind for Apprentice<M> {
    fn act(&mut self, obs: &Observation) -> Action {
        self.t += 1;
        self.learn_volatility(obs);
        self.surprise = self.model.observe(obs);
        self.check_told(obs);
        let (action, tried) = self.choose(obs);
        self.pending = Some((tried, f64::from(obs.energy), obs.offset));
        action
    }

    fn feel(&mut self, outcome: &Outcome) {
        if outcome.slept {
            self.model.sleep();
        }
        let Some((tried, before, at)) = self.pending.take() else {
            return;
        };
        if outcome.slept {
            return; // the energy reset says nothing about the action
        }
        let felt = f64::from(outcome.energy) - before;
        if matches!(tried, Tried::Eat(_)) && felt > 0.0 {
            self.model.fed(at);
        }
        let rate = self.drives.learning;
        let slot = match tried {
            Tried::Enter(k) => &mut self.valence.enter[k],
            Tried::Eat(k) => &mut self.valence.eat[k],
            Tried::Rest => &mut self.valence.rest,
        };
        *slot += rate * (felt - *slot);
    }

    fn surprise(&self) -> f64 {
        self.surprise
    }

    fn believes(&self, at: Pos) -> Option<Cell> {
        self.model.believes(at)
    }
}

/// A frontier entry in the planner's cheapest-first search.
#[derive(Debug, Clone, Copy)]
struct Node {
    cost: f64,
    steps: u32,
    at: Pos,
    first: usize,
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Node {}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Node {
    /// Reversed, so `BinaryHeap` pops the cheapest (then fewest steps) first.
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cost
            .total_cmp(&self.cost)
            .then(other.steps.cmp(&self.steps))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::ReflexAgent;
    use crate::body::VIEW;
    use crate::sim::Event;

    const ORIGIN: Pos = Pos { x: 0, y: 0 };

    fn obs(view: [Cell; VIEW * VIEW], energy: u32) -> Observation {
        Observation {
            view,
            energy,
            offset: ORIGIN,
        }
    }

    fn open(energy: u32) -> Observation {
        obs([Cell::Empty; VIEW * VIEW], energy)
    }

    fn apprentice(curiosity: f64) -> Apprentice {
        let drives = Drives {
            curiosity,
            ..Drives::default()
        };
        Apprentice::new(drives, Rates::default(), 0)
    }

    fn felt(energy: u32) -> Outcome {
        Outcome {
            event: Event::Moved,
            energy,
            slept: false,
        }
    }

    /// Teaches food = +24 and the usual -2 for a step.
    fn taught(curiosity: f64) -> Apprentice {
        let mut a = apprentice(curiosity);
        a.valence.eat = [-1.0, -1.0, 24.0, -1.0];
        a.valence.enter = [-2.0, -2.0, -2.0, -12.0];
        a.valence.rest = -1.0;
        a
    }

    #[test]
    fn valence_is_learned_from_felt_energy() {
        let mut a = apprentice(0.0);
        a.pending = Some((Tried::Eat(kind(Cell::Food)), 40.0, ORIGIN));
        a.feel(&felt(64));
        let expected = 25.0 + 0.2 * (24.0 - 25.0);
        assert!((a.valence.eat[kind(Cell::Food)] - expected).abs() < 1e-12);
    }

    #[test]
    fn sleep_teaches_nothing() {
        let mut a = apprentice(0.0);
        a.pending = Some((Tried::Rest, 1.0, ORIGIN));
        a.feel(&Outcome {
            event: Event::Rested,
            energy: 50,
            slept: true,
        });
        assert_eq!(a.valence, Valence::new(Drives::default().optimism));
    }

    #[test]
    fn hungry_apprentice_eats_food_underfoot() {
        let mut a = taught(0.0);
        let mut view = [Cell::Empty; VIEW * VIEW];
        view[Observation::CENTER] = Cell::Food;
        assert_eq!(a.act(&obs(view, 20)), Action::Eat);
    }

    #[test]
    fn hungry_apprentice_walks_to_food_it_sees() {
        let mut a = taught(0.0);
        let mut view = [Cell::Empty; VIEW * VIEW];
        view[5] = Cell::Food; // east
        assert_eq!(a.act(&obs(view, 20)), Action::Move(Dir::East));
    }

    #[test]
    fn sated_incurious_apprentice_rests() {
        let mut a = taught(0.0);
        assert_eq!(a.act(&open(95)), Action::Rest);
    }

    #[test]
    fn it_steps_around_a_hazard_it_has_learned_hurts() {
        let mut a = taught(0.0);
        // Food two squares east, a hazard between; the way round is open.
        let mut view = [Cell::Empty; VIEW * VIEW];
        view[5] = Cell::Hazard;
        for _ in 0..5 {
            a.model.observe(&obs(view, 20));
        }
        let food = Pos { x: 2, y: 0 };
        for _ in 0..5 {
            let mut v = [Cell::Empty; VIEW * VIEW];
            v[Observation::CENTER] = Cell::Food;
            a.model.observe(&Observation {
                view: v,
                energy: 20,
                offset: food,
            });
        }
        let action = a.act(&obs(view, 20));
        assert!(
            matches!(action, Action::Move(Dir::North | Dir::South)),
            "{action:?}"
        );
    }

    #[test]
    fn curious_apprentice_heads_for_the_unknown() {
        let mut a = taught(1.0);
        // Everything up to one square east has been seen as open; beyond
        // that lies the unknown.
        for y in -10..=10 {
            for x in -10..=0 {
                let o = Observation {
                    offset: Pos { x, y },
                    ..open(95)
                };
                a.model.observe(&o);
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        a.last_seen.insert(
                            Pos {
                                x: x + dx,
                                y: y + dy,
                            },
                            0,
                        );
                    }
                }
            }
        }
        assert_eq!(a.act(&open(95)), Action::Move(Dir::East));
    }

    #[test]
    fn a_changed_square_raises_volatility() {
        let mut a = apprentice(0.0);
        let away = Observation {
            offset: Pos { x: 5, y: 0 },
            ..open(95)
        };
        a.act(&open(95));
        for _ in 0..3 {
            a.act(&away);
        }
        a.act(&open(95)); // back, nothing changed
        let calm = a.volatility();
        for _ in 0..3 {
            a.act(&away);
        }
        let mut view = [Cell::Empty; VIEW * VIEW];
        view[0] = Cell::Wall; // back, and a wall has appeared
        a.act(&obs(view, 95));
        assert!(a.volatility() > calm, "{} vs {calm}", a.volatility());
    }

    #[test]
    fn walls_are_never_walked_into_once_learned() {
        let mut a = taught(1.0);
        let mut view = [Cell::Wall; VIEW * VIEW];
        view[Observation::CENTER] = Cell::Empty;
        view[3] = Cell::Empty; // only west is open
        let action = a.act(&obs(view, 50));
        assert!(
            matches!(action, Action::Move(Dir::West) | Action::Rest),
            "{action:?}"
        );
    }

    const FOOD_WORD: Word = Word(7);

    #[test]
    fn a_word_means_what_was_seen_where_it_was_said() {
        let mut a = taught(0.0);
        assert_eq!(a.meaning(FOOD_WORD), None);
        let mut view = [Cell::Empty; VIEW * VIEW];
        view[5] = Cell::Food; // east
        for _ in 0..10 {
            a.hear(Pos { x: 1, y: 0 }, FOOD_WORD);
            a.act(&obs(view, 95));
        }
        let m = a.meaning(FOOD_WORD).expect("checked");
        assert!(m[kind(Cell::Food)] > 0.85, "{m:?}");
        assert!(a.told.is_empty());
    }

    #[test]
    fn a_word_said_out_of_view_waits_to_be_checked() {
        let mut a = taught(0.0);
        a.hear(Pos { x: 5, y: 0 }, FOOD_WORD);
        a.act(&open(20));
        assert_eq!(a.meaning(FOOD_WORD), None);
        assert_eq!(a.told.get(&Pos { x: 5, y: 0 }), Some(&FOOD_WORD));
    }

    #[test]
    fn a_hungry_apprentice_follows_a_word_it_knows() {
        let mut a = taught(0.0);
        a.lexicon.insert(FOOD_WORD, one_hot(Cell::Food));
        a.hear(Pos { x: 0, y: -4 }, FOOD_WORD);
        assert_eq!(a.act(&open(20)), Action::Move(Dir::North));
    }

    #[test]
    fn an_unknown_word_draws_nothing() {
        let mut told = taught(0.0);
        told.hear(Pos { x: 0, y: -4 }, FOOD_WORD);
        let plans = told.plan(&open(20), 1.0);
        assert_eq!(plans, taught(0.0).plan(&open(20), 1.0));
    }

    #[test]
    fn watched_reflex_learns_while_reflex_drives() {
        let mut w = Watched::new(ReflexAgent::new(0), Rates::default());
        let mut view = [Cell::Empty; VIEW * VIEW];
        view[Observation::CENTER] = Cell::Food;
        assert_eq!(w.act(&obs(view, 50)), Action::Eat);
        assert!(w.surprise() > 0.0);
        assert_eq!(w.believes(ORIGIN), Some(Cell::Food));
    }
}
