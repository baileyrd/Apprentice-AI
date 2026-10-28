//! Phase 4: an episodic diary, replay in sleep, consolidation and decay.
//!
//! Two stores, as in the design sketch:
//! - **Diary (fast, hippocampus-like).** Every square sensed today, in order,
//!   plus a fast belief per square that tracks the latest sighting. It answers
//!   predictions first.
//! - **Maps (slow, cortex-like).** One predictive-coding map per context
//!   (per world it has lived in). They change only in sleep, when the diary is
//!   replayed into the active map at a low rate. So a map holds how a square
//!   usually is, not how it was last seen: a bush it keeps eating from stays
//!   food in the map even though it was last seen empty.
//!
//! Meals are replayed more than other moments (`meal_replays` extra times
//! each), as rewarding moments are in animal sleep. Without that, a bush is
//! remembered as mostly empty, because it is mostly seen just after a meal.
//!
//! In sleep, squares the diary rehearsed and found as expected grow
//! **stiffer**; squares it did not rehearse **decay** toward the prior, slower
//! the stiffer they are. Decayed squares grow uncertain again, which is what
//! sends a curious apprentice back to look at them.
//!
//! **Contexts.** Each map's fit is a running mean of its prediction error on
//! what is sensed now. When the active map stops fitting and another one fits,
//! it recalls that one; when none has fitted for a while, it starts a new map
//! from the active map's prior (general expectations, no particular squares). Before any
//! switch the diary is consolidated into the map it was written in.
//!
//! A full diary forces a nap: the same consolidation, without world time.
//! Every update is a local delta rule; nothing is trained end to end.

use std::collections::{HashMap, HashSet};

use crate::body::Observation;
use crate::model::{Belief, PredictiveCoding, Rates, WorldModel, error, most_likely, one_hot};
use crate::world::{Cell, Pos};

/// How the memory consolidates, forgets and recognises contexts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Recall {
    /// Rate at which a replayed sighting moves the map's belief.
    pub consolidation: f64,
    /// Share of the way to the prior an unrehearsed, never-confirmed square
    /// fades per step of absence.
    pub decay: f64,
    /// Steps the diary holds before it forces a nap.
    pub capacity: usize,
    /// Extra replays of each meal in sleep: rewarding moments are rehearsed
    /// more, so a place that fed it is remembered as food.
    pub meal_replays: usize,
    /// Rate of each map's running prediction error.
    pub fit_rate: f64,
    /// A map whose running error is above this does not fit.
    pub misfit: f64,
    /// Sightings a map needs since the last switch before its fit counts, and
    /// sightings without any fitting map before it starts a new one.
    pub settle: usize,
}

impl Default for Recall {
    fn default() -> Self {
        Self {
            consolidation: 0.2,
            decay: 2e-4,
            capacity: 64,
            meal_replays: 5,
            fit_rate: 0.02,
            misfit: 0.3,
            settle: 300,
        }
    }
}

/// One slow map and what memory keeps about it.
#[derive(Debug, Clone)]
struct Context {
    map: PredictiveCoding,
    /// Replays that confirmed each square.
    stiffness: HashMap<Pos, u32>,
    /// Running prediction error on what is sensed now, and how many sightings
    /// it rests on.
    fit: f64,
    samples: usize,
}

impl Context {
    fn new(map: PredictiveCoding) -> Self {
        Self {
            map,
            stiffness: HashMap::new(),
            fit: 0.0,
            samples: 0,
        }
    }

    fn judge(&mut self, at: Pos, sensed: Cell, rate: f64) {
        if !self.map.knows(at) {
            return;
        }
        let e = error(&self.map.predict(at), sensed);
        self.fit = if self.samples == 0 {
            e
        } else {
            self.fit + rate * (e - self.fit)
        };
        self.samples += 1;
    }
}

/// The phase 4 world model: diary over context maps.
#[derive(Debug, Clone)]
pub struct Memory {
    recall: Recall,
    sensory: f64,
    fast: HashMap<Pos, Belief>,
    diary: Vec<(Pos, Cell)>,
    /// Where it ate today.
    meals: Vec<Pos>,
    contexts: Vec<Context>,
    active: usize,
    t: u64,
    last_sleep: u64,
    /// Sightings since the active map last fitted.
    lost: usize,
}

impl Memory {
    /// Fast beliefs learn at `rates.sensory`; the first map's prior at
    /// `rates.prior`.
    pub fn new(rates: Rates, recall: Recall) -> Self {
        let slow = Rates {
            sensory: recall.consolidation,
            ..rates
        };
        Self {
            recall,
            sensory: rates.sensory,
            fast: HashMap::new(),
            diary: Vec::new(),
            meals: Vec::new(),
            contexts: vec![Context::new(PredictiveCoding::new(slow))],
            active: 0,
            t: 0,
            last_sleep: 0,
            lost: 0,
        }
    }

    /// Index of the map in use.
    pub fn active(&self) -> usize {
        self.active
    }

    /// How many maps it has made.
    pub fn contexts(&self) -> usize {
        self.contexts.len()
    }

    /// Replays the diary into the active map, stiffens what it confirmed,
    /// lets the rest decay, then empties the diary.
    fn consolidate(&mut self) {
        let elapsed = (self.t - self.last_sleep) as f64;
        let ctx = &mut self.contexts[self.active];
        let mut rehearsed = HashSet::new();
        for &(at, sensed) in &self.diary {
            if ctx.map.believes(at) == Some(sensed) {
                *ctx.stiffness.entry(at).or_insert(0) += 1;
            }
            ctx.map.learn(at, sensed);
            rehearsed.insert(at);
        }
        for &at in &self.meals {
            for _ in 0..self.recall.meal_replays {
                ctx.map.learn(at, Cell::Food);
            }
        }
        let faded: Vec<Pos> = ctx.map.known().filter(|p| !rehearsed.contains(p)).collect();
        for at in faded {
            let stiff = f64::from(ctx.stiffness.get(&at).copied().unwrap_or(0));
            let amount = (self.recall.decay * elapsed / (1.0 + stiff)).min(1.0);
            ctx.map.fade(at, amount);
        }
        self.diary.clear();
        self.meals.clear();
        self.fast.clear();
        self.last_sleep = self.t;
    }

    /// Consolidates, then makes `to` the active map.
    fn switch(&mut self, to: usize) {
        self.consolidate();
        self.active = to;
        for ctx in &mut self.contexts {
            ctx.samples = 0;
        }
        self.lost = 0;
    }

    /// Recalls a map that fits better, or starts a new one if none has fitted
    /// for `settle` sightings.
    fn reconsider(&mut self, sightings: usize) {
        let r = self.recall;
        let counted = |c: &Context| c.samples >= r.settle;
        let current = &self.contexts[self.active];
        if !counted(current) || current.fit <= r.misfit {
            self.lost = 0;
            return;
        }
        self.lost += sightings;
        let best = (0..self.contexts.len())
            .filter(|&i| i != self.active && counted(&self.contexts[i]))
            .min_by(|&i, &j| self.contexts[i].fit.total_cmp(&self.contexts[j].fit));
        match best {
            Some(i) if self.contexts[i].fit <= r.misfit => self.switch(i),
            _ if self.lost < r.settle => {}
            _ => {
                let schema = self.contexts[self.active].map.schema();
                self.contexts.push(Context::new(schema));
                self.switch(self.contexts.len() - 1);
            }
        }
    }
}

impl WorldModel for Memory {
    fn observe(&mut self, obs: &Observation) -> f64 {
        let squares = obs.view.len() as f64;
        let mut surprise = 0.0;
        for (i, &sensed) in obs.view.iter().enumerate() {
            let (dx, dy) = Observation::view_delta(i);
            surprise += self.learn(obs.offset.shift(dx, dy), sensed);
        }
        self.t += 1;
        self.reconsider(obs.view.len());
        if self.diary.len() >= self.recall.capacity * obs.view.len() {
            self.consolidate();
        }
        surprise / squares
    }

    fn learn(&mut self, at: Pos, sensed: Cell) -> f64 {
        let predicted = self.predict(at);
        let surprise = error(&predicted, sensed);
        let target = one_hot(sensed);
        let fast =
            std::array::from_fn(|k| predicted[k] + self.sensory * (target[k] - predicted[k]));
        self.fast.insert(at, fast);
        self.diary.push((at, sensed));
        let rate = self.recall.fit_rate;
        for ctx in &mut self.contexts {
            ctx.judge(at, sensed, rate);
        }
        surprise
    }

    fn predict(&self, at: Pos) -> Belief {
        match self.fast.get(&at) {
            Some(b) => *b,
            None => self.contexts[self.active].map.predict(at),
        }
    }

    fn believes(&self, at: Pos) -> Option<Cell> {
        match self.fast.get(&at) {
            Some(b) => most_likely(b),
            None => self.contexts[self.active].map.believes(at),
        }
    }

    fn sleep(&mut self) {
        self.consolidate();
    }

    fn fed(&mut self, at: Pos) {
        self.meals.push(at);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::VIEW;
    use crate::model::kind;

    const ORIGIN: Pos = Pos { x: 0, y: 0 };

    fn memory() -> Memory {
        Memory::new(Rates::default(), Recall::default())
    }

    fn at(offset: Pos, view: [Cell; VIEW * VIEW]) -> Observation {
        Observation {
            view,
            energy: 50,
            offset,
        }
    }

    /// A view with a wall to the north-west, else open.
    fn corner() -> [Cell; VIEW * VIEW] {
        let mut v = [Cell::Empty; VIEW * VIEW];
        v[0] = Cell::Wall;
        v
    }

    #[test]
    fn the_diary_answers_first_and_the_map_keeps_it_after_sleep() {
        let mut m = memory();
        for _ in 0..10 {
            m.observe(&at(ORIGIN, corner()));
        }
        let nw = Pos { x: -1, y: -1 };
        assert_eq!(m.believes(nw), Some(Cell::Wall));
        assert!(!m.contexts[0].map.knows(nw), "map changed while awake");
        m.sleep();
        assert!(m.fast.is_empty() && m.diary.is_empty());
        assert_eq!(m.believes(nw), Some(Cell::Wall));
    }

    #[test]
    fn a_bush_stays_food_in_the_map_though_last_seen_eaten() {
        let mut m = memory();
        let mut v = [Cell::Empty; VIEW * VIEW];
        v[Observation::CENTER] = Cell::Food;
        for _ in 0..20 {
            m.observe(&at(ORIGIN, v));
        }
        v[Observation::CENTER] = Cell::Empty; // eaten
        for _ in 0..2 {
            m.observe(&at(ORIGIN, v));
        }
        assert_eq!(m.believes(ORIGIN), Some(Cell::Empty));
        m.sleep();
        assert_eq!(m.believes(ORIGIN), Some(Cell::Food));
    }

    #[test]
    fn a_meal_is_rehearsed_until_the_place_means_food() {
        let mut plain = Memory::new(
            Rates::default(),
            Recall {
                meal_replays: 0,
                ..Recall::default()
            },
        );
        let mut rewarded = memory();
        for m in [&mut plain, &mut rewarded] {
            let mut v = [Cell::Empty; VIEW * VIEW];
            v[Observation::CENTER] = Cell::Food;
            m.observe(&at(ORIGIN, v));
            m.fed(ORIGIN);
            v[Observation::CENTER] = Cell::Empty; // eaten, then seen empty
            for _ in 0..10 {
                m.observe(&at(ORIGIN, v));
            }
            m.sleep();
        }
        assert_eq!(plain.believes(ORIGIN), Some(Cell::Empty));
        assert_eq!(rewarded.believes(ORIGIN), Some(Cell::Food));
    }

    #[test]
    fn unrehearsed_squares_decay_and_stiff_ones_decay_less() {
        let mut m = memory();
        let east = Pos { x: 5, y: 0 };
        for _ in 0..30 {
            m.observe(&at(ORIGIN, corner())); // rehearsed often: stiff
        }
        m.observe(&at(east, corner())); // seen once
        m.sleep();
        let sure = |m: &Memory, p: Pos| m.predict(p)[kind(Cell::Wall)];
        let (stiff, loose) = (Pos { x: -1, y: -1 }, Pos { x: 4, y: -1 });
        let (before_stiff, before_loose) = (sure(&m, stiff), sure(&m, loose));
        let far = Pos { x: 50, y: 50 };
        for _ in 0..2_000 {
            m.observe(&at(far, [Cell::Empty; VIEW * VIEW]));
        }
        m.sleep();
        let lost_stiff = before_stiff - sure(&m, stiff);
        let lost_loose = before_loose - sure(&m, loose);
        assert!(lost_loose > 0.05, "loose square barely faded: {lost_loose}");
        assert!(
            lost_stiff < lost_loose / 5.0,
            "{lost_stiff} vs {lost_loose}"
        );
    }

    #[test]
    fn a_full_diary_forces_a_nap() {
        let mut m = Memory::new(
            Rates::default(),
            Recall {
                capacity: 5,
                ..Recall::default()
            },
        );
        for _ in 0..5 {
            m.observe(&at(ORIGIN, corner()));
        }
        assert!(m.diary.is_empty());
        assert!(m.contexts[0].map.knows(ORIGIN));
    }

    /// Lives in a striped world: walls on every `period`-th column.
    fn live(m: &mut Memory, period: i32, steps: i32) {
        for s in 0..steps {
            let offset = Pos {
                x: s % 20,
                y: (s / 20) % 20,
            };
            let view = std::array::from_fn(|i| {
                let (dx, _) = Observation::view_delta(i);
                if (offset.x + dx).rem_euclid(period) == 0 {
                    Cell::Wall
                } else {
                    Cell::Empty
                }
            });
            m.observe(&at(offset, view));
        }
    }

    #[test]
    fn a_new_world_gets_a_new_map_and_the_old_one_is_recalled() {
        let mut m = memory();
        live(&mut m, 3, 2_000);
        assert_eq!((m.contexts(), m.active()), (1, 0));
        live(&mut m, 4, 2_000);
        assert_eq!((m.contexts(), m.active()), (2, 1), "no new map for B");
        live(&mut m, 3, 1_000);
        assert_eq!(m.active(), 0, "world A not recalled");
        assert_eq!(m.contexts(), 2);
        assert_eq!(m.believes(Pos { x: 3, y: 5 }), Some(Cell::Wall));
    }
}
