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
//! what is sensed now. When the active map stops fitting and another one fits
//! clearly better (by `Recall::margin`),
//! it recalls that one; when none has fitted for a while, it starts a new map
//! from the active map's prior (general expectations, no particular squares). Before any
//! switch the diary is consolidated into the map it was written in.
//!
//! **Doubt.** Misfit builds doubt and fit drains it, so a few steps that
//! happen to look familiar do not wipe out the evidence of being elsewhere.
//! While in doubt, what it senses is held in the diary rather than written
//! into the map in use, so a strange place cannot be learned into home's map
//! while it is still deciding. A bush it ate from is judged by when patterns
//! say it grows back: an expected change is no sign of being somewhere else.
//!
//! A full diary forces a nap: the same consolidation, without world time.
//! Every update is a local delta rule; nothing is trained end to end.
//!
//! **Patterns** (the slowest store, [`crate::patterns`]) learn in sleep from
//! every world how squares change: what a square becomes, given how it last
//! changed and how long ago. They refine the diary's beliefs about squares
//! seen today: a bush eaten an hour ago is expected to have grown back, in a
//! world where the apprentice has never seen that bush grow. A word a mentor
//! says about a square joins its state, so patterns can learn what the word
//! foretells ([`WorldModel::hear`]).

use std::collections::{HashMap, HashSet};

use crate::body::{Observation, VIEW, Word};
use crate::model::{Belief, PredictiveCoding, Rates, WorldModel, error, most_likely, one_hot};
use crate::patterns::{Last, Patterns};
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
    /// How much lower an old map's running error must be than the active
    /// map's before it is recalled: one dip in a stretch that suits any map
    /// is not enough to go back.
    pub margin: f64,
    /// Whether it learns and uses patterns.
    pub patterns: bool,
    /// Rate at which a replayed sighting moves a pattern.
    pub pattern_rate: f64,
    /// Samples a pattern needs before it predicts.
    pub pattern_min: u32,
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
            margin: 0.15,
            patterns: true,
            pattern_rate: 0.05,
            pattern_min: 20,
        }
    }
}

/// A square's state for patterns: how it last changed, since when, when it
/// was last sensed, and any word said about it since.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Seen {
    last: Last,
    since: u64,
    sensed: u64,
    word: Option<Word>,
}

impl Seen {
    fn new(last: Last, t: u64) -> Self {
        Self {
            last,
            since: t,
            sensed: t,
            word: None,
        }
    }
}

/// Where a belief about a square comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// A pattern: how squares like it change (a bush grows back).
    Pattern,
    /// The diary: seen since it last slept.
    Diary,
    /// The map in use: consolidated in sleep.
    Map,
    /// Never seen: the map's general expectation.
    Unknown,
}

/// A sighting in the diary, with the square's state, that state's age and
/// its word just before it.
type Sighting = (Pos, Cell, Option<(Last, u64, Option<Word>)>);

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
    /// How each square last changed, when, and when it was last sensed.
    seen: HashMap<Pos, Seen>,
}

impl Context {
    fn new(map: PredictiveCoding) -> Self {
        Self {
            map,
            stiffness: HashMap::new(),
            fit: 0.0,
            samples: 0,
            seen: HashMap::new(),
        }
    }

    /// Scores the map on one sighting; `expected` overrides the map where
    /// a known change (a bush growing back) says what to expect instead.
    fn judge(&mut self, at: Pos, sensed: Cell, rate: f64, expected: Option<Belief>) {
        if !self.map.knows(at) {
            return;
        }
        let e = error(&expected.unwrap_or_else(|| self.map.predict(at)), sensed);
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
    diary: Vec<Sighting>,
    /// Where it ate today.
    meals: Vec<Pos>,
    contexts: Vec<Context>,
    active: usize,
    t: u64,
    last_sleep: u64,
    /// Doubt: sightings of misfit, less sightings of fit since, and the step
    /// it began.
    lost: usize,
    lost_since: u64,
    /// Diary and meal entries made before the doubt began.
    doubt: (usize, usize),
    patterns: Patterns,
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
            lost_since: 0,
            doubt: (0, 0),
            patterns: Patterns::new(recall.pattern_rate, recall.pattern_min),
        }
    }

    /// What patterns say about a square seen in this world; `None` if they
    /// are off, the square is unseen, or the pattern is not yet learned.
    fn anticipate(&self, at: Pos) -> Option<Belief> {
        if !self.recall.patterns {
            return None;
        }
        let seen = self.contexts[self.active].seen.get(&at)?;
        self.patterns
            .predict(seen.last, self.t - seen.since, seen.word)
    }

    /// Index of the map in use.
    pub fn active(&self) -> usize {
        self.active
    }

    /// How many maps it has made.
    pub fn contexts(&self) -> usize {
        self.contexts.len()
    }

    /// What it believes at `at`, and where that belief comes from.
    pub fn explain(&self, at: Pos) -> (Belief, Source) {
        if let Some(b) = self.anticipate(at) {
            return (b, Source::Pattern);
        }
        if let Some(b) = self.fast.get(&at) {
            return (*b, Source::Diary);
        }
        let map = &self.contexts[self.active].map;
        let source = if map.knows(at) {
            Source::Map
        } else {
            Source::Unknown
        };
        (map.predict(at), source)
    }

    /// What map `i` alone holds about `at`; `None` if it never learned it.
    pub fn recalls(&self, i: usize, at: Pos) -> Option<Belief> {
        let map = &self.contexts.get(i)?.map;
        map.knows(at).then(|| map.predict(at))
    }

    /// Each map's running error on what is sensed now; `None` while it rests
    /// on too few sightings to count.
    pub fn fits(&self) -> Vec<Option<f64>> {
        let settle = self.recall.settle;
        self.contexts
            .iter()
            .map(|c| (c.samples >= settle).then_some(c.fit))
            .collect()
    }

    /// Doubt held now, and the doubt at which it starts a new map.
    pub fn doubt(&self) -> (usize, usize) {
        (self.lost, self.recall.settle)
    }

    /// Sightings held in the diary, not yet written into any map, because
    /// they were made in doubt.
    pub fn held(&self) -> usize {
        self.diary.len() - self.settled().0
    }

    /// What its patterns say a square in state `last`, `age` steps old, is
    /// now; `None` until that pattern is learned.
    pub fn expects(&self, last: Last, age: u64) -> Option<Belief> {
        self.patterns.predict(last, age, None)
    }

    /// What its patterns say a square in state `last`, `age` steps old, that
    /// it was told `word` about, is now; `None` until that word's pattern is
    /// learned.
    pub fn foretold(&self, last: Last, age: u64, word: Word) -> Option<Belief> {
        self.patterns.foretold(last, age, word)
    }

    /// How it consolidates, forgets and recognises contexts.
    pub fn recall(&self) -> Recall {
        self.recall
    }

    /// Replays the diary into the active map, stiffens what it confirmed,
    /// lets the rest decay, then empties the diary.
    fn consolidate(&mut self) {
        let elapsed = (self.t - self.last_sleep) as f64;
        let (d, m) = self.settled();
        let ctx = &mut self.contexts[self.active];
        let mut rehearsed = HashSet::new();
        for &(at, sensed, before) in &self.diary[..d] {
            if ctx.map.believes(at) == Some(sensed) {
                *ctx.stiffness.entry(at).or_insert(0) += 1;
            }
            ctx.map.learn(at, sensed);
            rehearsed.insert(at);
            if let (true, Some((last, age, word))) = (self.recall.patterns, before) {
                self.patterns.learn(last, age, word, sensed);
            }
        }
        for &at in &self.meals[..m] {
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
        self.diary.drain(..d);
        self.meals.drain(..m);
        self.doubt = (0, 0);
        self.fast.clear();
        self.last_sleep = self.t;
    }

    /// Diary and meal entries it is sure belong to the active map: while in
    /// doubt, only those from before the doubt began, unless the doubt alone
    /// has filled the diary.
    fn settled(&self) -> (usize, usize) {
        let all = (self.diary.len(), self.meals.len());
        let held = self.diary.len() - self.doubt.0;
        if self.lost == 0 || held >= self.recall.capacity * VIEW * VIEW {
            return all;
        }
        self.doubt
    }

    /// Consolidates, then makes `to` the active map. What was sensed since the
    /// old map stopped fitting belongs to the new one's world, so its square
    /// states move across.
    fn switch(&mut self, to: usize) {
        self.consolidate();
        let since = self.lost_since;
        let seen = &mut self.contexts[self.active].seen;
        let recent: Vec<(Pos, Seen)> = seen
            .iter()
            .filter(|(_, s)| s.sensed >= since)
            .map(|(&at, &s)| (at, s))
            .collect();
        for (at, _) in &recent {
            seen.remove(at);
        }
        self.contexts[to].seen.extend(recent);
        self.active = to;
        for ctx in &mut self.contexts {
            ctx.samples = 0;
        }
        self.lost = 0;
    }

    /// Whether old map `i` fits what is sensed now, and clearly better than
    /// the active map.
    fn recognises(&self, i: usize) -> bool {
        let r = self.recall;
        let fit = self.contexts[i].fit;
        fit <= r.misfit && fit + r.margin <= self.contexts[self.active].fit
    }

    /// Recalls a map that fits clearly better, or starts a new one if none has fitted
    /// for `settle` sightings.
    fn reconsider(&mut self, sightings: usize) {
        let r = self.recall;
        let counted = |c: &Context| c.samples >= r.settle;
        let current = &self.contexts[self.active];
        if !counted(current) || current.fit <= r.misfit {
            // Doubt drains as the map fits again; it does not vanish.
            self.lost = self.lost.saturating_sub(sightings);
            return;
        }
        if self.lost == 0 {
            self.lost_since = self.t;
            self.doubt = (self.diary.len().saturating_sub(sightings), self.meals.len());
        }
        self.lost += sightings;
        let best = (0..self.contexts.len())
            .filter(|&i| i != self.active && counted(&self.contexts[i]))
            .min_by(|&i, &j| self.contexts[i].fit.total_cmp(&self.contexts[j].fit));
        match best {
            Some(i) if self.recognises(i) => self.switch(i),
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
        let t = self.t;
        let seen = &mut self.contexts[self.active].seen;
        let before = seen.get(&at).copied();
        let next = match before {
            Some(s) if s.last.looks() == sensed => Seen { sensed: t, ..s },
            _ => Seen::new(Last::Saw(sensed), t),
        };
        seen.insert(at, next);
        self.diary
            .push((at, sensed, before.map(|s| (s.last, t - s.since, s.word))));
        let rate = self.recall.fit_rate;
        // A bush it ate from is judged by when it should grow back, not by
        // the map: an expected change is no sign of being somewhere else.
        let (active, patterns, on) = (self.active, &self.patterns, self.recall.patterns);
        for (i, ctx) in self.contexts.iter_mut().enumerate() {
            let state = if i == active {
                before
            } else {
                ctx.seen.get(&at).copied()
            };
            let expected = state
                .filter(|s| on && s.last == Last::Ate)
                .and_then(|s| patterns.predict(s.last, t - s.since, s.word));
            ctx.judge(at, sensed, rate, expected);
        }
        surprise
    }

    fn predict(&self, at: Pos) -> Belief {
        self.explain(at).0
    }

    fn believes(&self, at: Pos) -> Option<Cell> {
        if let Some(b) = self.anticipate(at) {
            return most_likely(&b);
        }
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
        let ate = Seen::new(Last::Ate, self.t);
        self.contexts[self.active].seen.insert(at, ate);
    }

    fn hear(&mut self, at: Pos, word: Word) {
        if let Some(seen) = self.contexts[self.active].seen.get_mut(&at) {
            seen.word = Some(word);
        }
    }

    fn map_in_use(&self) -> Option<usize> {
        Some(self.active)
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
    fn a_word_holds_until_the_square_changes_and_teaches_its_pattern() {
        let word = Word(9);
        let mut m = Memory::new(
            Rates::default(),
            Recall {
                pattern_min: 1,
                ..Recall::default()
            },
        );
        let mut food = [Cell::Empty; VIEW * VIEW];
        food[Observation::CENTER] = Cell::Food;
        m.observe(&at(ORIGIN, food));
        m.fed(ORIGIN);
        m.hear(ORIGIN, word);
        m.observe(&at(ORIGIN, [Cell::Empty; VIEW * VIEW]));
        assert_eq!(m.contexts[0].seen[&ORIGIN].word, Some(word));
        m.observe(&at(ORIGIN, food)); // grown back: a new state, no word
        assert_eq!(m.contexts[0].seen[&ORIGIN].word, None);
        m.sleep();
        let told = m.foretold(Last::Ate, 1, word).expect("learned");
        assert!(told[kind(Cell::Food)] > told[kind(Cell::Wall)]);
        assert_eq!(m.foretold(Last::Ate, 1, Word(8)), None);
    }

    #[test]
    fn a_word_about_an_unseen_square_is_dropped() {
        let mut m = memory();
        m.hear(Pos { x: 9, y: 9 }, Word(9));
        assert!(m.contexts[0].seen.get(&Pos { x: 9, y: 9 }).is_none());
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

    /// Sees food at `bush`, eats it, then watches it from beside: empty for
    /// 32 steps, then food again.
    fn eat_and_watch(m: &mut Memory, bush: Pos) {
        let beside = bush.shift(1, 0);
        let mut v = [Cell::Empty; VIEW * VIEW];
        v[3] = Cell::Food; // west
        m.observe(&at(beside, v));
        m.fed(bush);
        v[3] = Cell::Empty;
        for _ in 0..32 {
            m.observe(&at(beside, v));
        }
        v[3] = Cell::Food;
        m.observe(&at(beside, v));
    }

    /// A memory that never naps on its own.
    fn no_naps(patterns: bool) -> Memory {
        Memory::new(
            Rates::default(),
            Recall {
                capacity: 1_000,
                patterns,
                ..Recall::default()
            },
        )
    }

    #[test]
    fn patterns_anticipate_regrowth_at_a_bush_never_seen_grow() {
        let food = |m: &Memory, p: Pos| m.predict(p)[kind(Cell::Food)];
        let mut results = Vec::new();
        for patterns in [false, true] {
            let mut m = no_naps(patterns);
            for i in 0..40 {
                eat_and_watch(&mut m, Pos { x: 10 * i, y: 0 });
                m.sleep();
            }
            // A new bush: eaten, then out of view for 40 steps.
            let bush = Pos { x: 0, y: 20 };
            let mut v = [Cell::Empty; VIEW * VIEW];
            v[Observation::CENTER] = Cell::Food;
            m.observe(&at(bush, v));
            m.fed(bush);
            v[Observation::CENTER] = Cell::Empty;
            m.observe(&at(bush, v));
            let away = Pos { x: 0, y: 30 };
            for _ in 0..40 {
                m.observe(&at(away, [Cell::Empty; VIEW * VIEW]));
            }
            results.push(food(&m, bush));
        }
        assert!(results[0] < 0.3, "without patterns: {}", results[0]);
        assert!(results[1] > 0.8, "with patterns: {}", results[1]);
    }

    #[test]
    fn patterns_learn_only_in_sleep() {
        let mut m = no_naps(true);
        m.recall.capacity = 100_000;
        for i in 0..40 {
            eat_and_watch(&mut m, Pos { x: 10 * i, y: 0 });
        }
        assert_eq!(m.patterns.predict(Last::Ate, 40, None), None);
        m.sleep();
        assert!(m.patterns.predict(Last::Ate, 40, None).is_some());
    }

    #[test]
    fn a_switch_moves_what_was_seen_since_the_old_map_stopped_fitting() {
        let mut m = memory();
        live(&mut m, 3, 2_000);
        let before = m.contexts[0].seen.len();
        live(&mut m, 4, 2_000);
        let since = m.lost_since;
        let stale = |c: &Context| c.seen.values().filter(|s| s.sensed >= since).count();
        assert_eq!(m.active(), 1);
        assert_eq!(stale(&m.contexts[0]), 0, "world B's squares left in A");
        assert!(!m.contexts[1].seen.is_empty() && before > 0);
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

    #[test]
    fn an_old_map_is_recalled_only_when_it_fits_clearly_better() {
        let mut m = memory();
        let schema = m.contexts[0].map.schema();
        m.contexts.push(Context::new(schema));
        let mut fits = |active: f64, old: f64| {
            m.contexts[0].fit = active;
            m.contexts[1].fit = old;
            m.recognises(1)
        };
        assert!(!fits(0.35, 0.28), "a near tie is not evidence");
        assert!(fits(0.35, 0.15));
        assert!(!fits(0.9, 0.35), "the old map must fit too");
    }

    #[test]
    fn doubt_drains_rather_than_vanishing() {
        let mut m = memory();
        m.contexts[0].samples = m.recall.settle;
        m.contexts[0].fit = 0.9;
        m.reconsider(9);
        m.reconsider(9);
        m.contexts[0].fit = 0.1;
        m.reconsider(9);
        assert_eq!(m.lost, 9, "one good step does not erase two bad ones");
    }

    #[test]
    fn what_is_sensed_in_doubt_stays_out_of_the_map() {
        let mut m = no_naps(false);
        m.lost = 100; // in doubt from before anything below was sensed
        let far = Pos { x: 50, y: 50 };
        m.observe(&at(far, corner()));
        m.sleep();
        assert!(
            !m.contexts[0].map.knows(far),
            "doubtful sighting written in"
        );
        m.lost = 0;
        m.sleep();
        assert!(m.contexts[0].map.knows(far));
    }

    #[test]
    fn an_expected_regrowth_is_no_sign_of_being_elsewhere() {
        let mut rises = Vec::new();
        for patterns in [false, true] {
            let mut m = no_naps(patterns);
            for i in 0..40 {
                eat_and_watch(&mut m, Pos { x: 10 * i, y: 0 });
                m.sleep();
            }
            // A square the map knows as empty, eaten from 40 steps ago.
            let bush = Pos { x: 0, y: 20 };
            for _ in 0..20 {
                m.observe(&at(bush, [Cell::Empty; VIEW * VIEW]));
            }
            m.sleep();
            let ate = Seen::new(Last::Ate, m.t - 40);
            m.contexts[0].seen.insert(bush, ate);
            let before = m.contexts[0].fit;
            let mut v = [Cell::Empty; VIEW * VIEW];
            v[Observation::CENTER] = Cell::Food;
            m.observe(&at(bush, v));
            rises.push(m.contexts[0].fit - before);
        }
        assert!(rises[0] > 0.02, "without patterns: {}", rises[0]);
        assert!(rises[1] < 0.01, "with patterns: {}", rises[1]);
    }

    #[test]
    fn it_can_say_where_a_belief_comes_from() {
        let mut m = no_naps(false);
        let nw = Pos { x: -1, y: -1 };
        assert_eq!(m.explain(nw).1, Source::Unknown);
        m.observe(&at(ORIGIN, corner()));
        assert_eq!(m.explain(nw).1, Source::Diary);
        m.sleep();
        let (belief, source) = m.explain(nw);
        assert_eq!(source, Source::Map);
        assert_eq!(most_likely(&belief), Some(Cell::Wall));
        assert_eq!(
            m.recalls(0, nw).map(|b| most_likely(&b)),
            Some(Some(Cell::Wall))
        );
        assert_eq!(m.recalls(1, nw), None);
    }
}
