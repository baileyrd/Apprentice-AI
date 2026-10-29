//! The mentor ("big brother"): it knows the map, the apprentice doesn't.
//!
//! It never writes into the apprentice's model; it only says words about
//! squares, in the apprentice's own frame. Its words are arbitrary symbols,
//! shuffled per mentor, so they mean nothing to the apprentice until it links
//! them to what it sees.
//! - **Hints.** When the apprentice is stuck (hungry), the mentor says its
//!   word for food about the nearest square it believes holds food. The
//!   apprentice still has to walk there and see for itself.
//! - **Naming.** Otherwise, if it names things, it says the right word for a
//!   square the apprentice is looking at, so the word gets its meaning from
//!   the apprentice's own experience.
//!
//! - **Foretelling.** Right after the apprentice eats, a foretelling mentor
//!   says one of two more words about the square: one when food will grow
//!   back there, the other when it won't. They too mean nothing until the
//!   apprentice sees what such squares become. A mentor that foretells at
//!   random says either word, whatever will happen.
//!
//! The help fades like stabilisers on a bike: the chance of saying anything
//! falls linearly to zero at `fade` steps.
//!
//! A blind mentor believes in food on squares that never hold any, to test
//! whether the apprentice inherits its mentor's blind spots.

use std::collections::{HashSet, VecDeque};

use crate::body::{Observation, Said, VIEW, Voice, Word};
use crate::model::{KINDS, kind};
use crate::rng::Rng;
use crate::sim::{Event, Outcome, Regrowth, Sim};
use crate::world::{Cell, Dir, Grid, Pos};

/// When and for how long the mentor helps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Teaching {
    /// The apprentice counts as stuck below this energy.
    pub stuck: u32,
    /// Step at which the mentor falls silent.
    pub fade: u64,
    /// Whether it names what the apprentice is looking at.
    pub names: bool,
    /// What it says about a square the apprentice has just eaten from.
    pub foretells: Foretell,
}

/// What a mentor says about a square the apprentice has just eaten from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Foretell {
    /// Nothing.
    #[default]
    Never,
    /// Its word for what will happen there.
    Truly,
    /// Either word, whatever will happen.
    AtRandom,
}

/// Its word for "food grows back here", and for "it won't".
pub const GROWS_BACK: Word = Word(KINDS as u8);
pub const STAYS_GONE: Word = Word(KINDS as u8 + 1);

impl Default for Teaching {
    fn default() -> Self {
        Self {
            stuck: 40,
            fade: 4_000,
            names: false,
            foretells: Foretell::Never,
        }
    }
}

/// Where the mentor believes food is.
#[derive(Debug, Clone, PartialEq)]
enum Sight {
    /// The real food on the grid.
    True,
    /// Squares that never hold food.
    Blind(Vec<Pos>),
}

/// A mentor watching one apprentice in one world.
#[derive(Debug, Clone, PartialEq)]
pub struct Mentor {
    teaching: Teaching,
    sight: Sight,
    /// Its word for each kind of square.
    words: [Word; KINDS],
    /// The voice it speaks in.
    voice: Voice,
    rng: Rng,
}

/// A shuffled vocabulary: one arbitrary word per kind of square.
fn vocabulary(rng: &mut Rng) -> [Word; KINDS] {
    let mut words: [Word; KINDS] = std::array::from_fn(|k| Word(k as u8));
    for i in (1..KINDS).rev() {
        words.swap(i, rng.below(i + 1));
    }
    words
}

impl Mentor {
    /// A mentor that sees the real map.
    pub fn new(teaching: Teaching, seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        Self {
            teaching,
            sight: Sight::True,
            words: vocabulary(&mut rng),
            voice: Voice(0),
            rng,
        }
    }

    /// A mentor that believes `count` random empty squares of `sim` hold food.
    /// In a bush world food never grows on an empty square, so it is always
    /// wrong.
    pub fn blind(teaching: Teaching, sim: &Sim, count: usize, seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let mut empty = sim.grid().positions_of(Cell::Empty);
        empty.retain(|&p| p != sim.start());
        let mut phantoms = Vec::with_capacity(count);
        while phantoms.len() < count && !empty.is_empty() {
            phantoms.push(empty.swap_remove(rng.below(empty.len())));
        }
        Self {
            teaching,
            sight: Sight::Blind(phantoms),
            words: vocabulary(&mut rng),
            voice: Voice(0),
            rng,
        }
    }

    /// The same mentor, speaking in `voice`.
    pub fn with_voice(self, voice: Voice) -> Self {
        Self { voice, ..self }
    }

    /// Its word for `cell`.
    pub fn word(&self, cell: Cell) -> Word {
        self.words[kind(cell)]
    }

    /// What it says this step: a square, relative to the apprentice's wake
    /// square, and a word about it in its voice; `None` when it stays quiet.
    pub fn say(&mut self, sim: &Sim) -> Option<(Pos, Said)> {
        let Teaching {
            stuck, fade, names, ..
        } = self.teaching;
        if !self.speaks(sim, fade) {
            return None;
        }
        let (at, word) = if sim.energy() < stuck {
            (self.food_near(sim)?, self.word(Cell::Food))
        } else if names {
            let (dx, dy) = Observation::view_delta(self.rng.below(VIEW * VIEW));
            let at = sim.pos().shift(dx, dy);
            (at, self.word(sim.grid().get(at)))
        } else {
            return None;
        };
        let start = sim.start();
        Some((at.shift(-start.x, -start.y), self.said(word)))
    }

    /// What it says after `outcome`: if the apprentice ate, the square it ate
    /// from, relative to its wake square, and a word for what that square
    /// will do; `None` otherwise or when it stays quiet.
    pub fn foretell(&mut self, sim: &Sim, outcome: &Outcome) -> Option<(Pos, Said)> {
        let how = self.teaching.foretells;
        if outcome.event != Event::Ate || how == Foretell::Never {
            return None;
        }
        if !self.speaks(sim, self.teaching.fade) {
            return None;
        }
        let grows = match how {
            Foretell::AtRandom => self.rng.chance(0.5),
            _ => matches!(sim.config().regrowth, Regrowth::InPlace { .. }),
        };
        let word = if grows { GROWS_BACK } else { STAYS_GONE };
        let start = sim.start();
        Some((sim.pos().shift(-start.x, -start.y), self.said(word)))
    }

    /// `word`, in its voice.
    fn said(&self, word: Word) -> Said {
        Said {
            voice: self.voice,
            word,
        }
    }

    /// Whether it says anything this step: less often as `fade` nears.
    fn speaks(&mut self, sim: &Sim, fade: u64) -> bool {
        sim.t() < fade && self.rng.chance(1.0 - sim.t() as f64 / fade as f64)
    }

    /// The nearest square it believes holds food.
    fn food_near(&self, sim: &Sim) -> Option<Pos> {
        let food = match &self.sight {
            Sight::True => sim.grid().positions_of(Cell::Food),
            Sight::Blind(phantoms) => phantoms.clone(),
        };
        nearest(sim.grid(), sim.pos(), &food)
    }
}

/// The goal fewest steps from `from`, walking over open floor and food.
fn nearest(grid: &Grid, from: Pos, goals: &[Pos]) -> Option<Pos> {
    let goals: HashSet<Pos> = goals.iter().copied().collect();
    let mut seen = HashSet::from([from]);
    let mut queue = VecDeque::from([from]);
    while let Some(p) = queue.pop_front() {
        if goals.contains(&p) {
            return Some(p);
        }
        for dir in Dir::ALL {
            let n = p.step(dir);
            let open = matches!(grid.get(n), Cell::Empty | Cell::Food);
            if open && seen.insert(n) {
                queue.push_back(n);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::Action;
    use crate::sim::Config;

    const KINDS_ALL: [Cell; KINDS] = [Cell::Empty, Cell::Wall, Cell::Food, Cell::Hazard];

    fn sim() -> Sim {
        Sim::new(Config::default(), 1).expect("room")
    }

    /// Rests until energy falls below `energy`.
    fn starve(sim: &mut Sim, energy: u32) {
        while sim.energy() >= energy {
            sim.step(Action::Rest);
        }
    }

    #[test]
    fn nearest_walks_around_walls() {
        let mut grid = Grid::empty(5, 1);
        grid.set(Pos { x: 1, y: 0 }, Cell::Wall);
        let goals = [Pos { x: 0, y: 0 }, Pos { x: 4, y: 0 }];
        assert_eq!(nearest(&grid, Pos { x: 2, y: 0 }, &goals), Some(goals[1]));
    }

    #[test]
    fn quiet_while_the_apprentice_is_fed() {
        let mut m = Mentor::new(Teaching::default(), 0);
        assert_eq!(m.say(&sim()), None);
    }

    #[test]
    fn points_at_real_food_when_stuck() {
        let mut s = sim();
        starve(&mut s, 40);
        let teaching = Teaching {
            fade: 1_000_000,
            ..Teaching::default()
        };
        let mut m = Mentor::new(teaching, 0);
        let (at, Said { word, .. }) = m.say(&s).expect("speaks early on");
        let start = s.start();
        let real = at.shift(start.x, start.y);
        assert_eq!(s.grid().get(real), Cell::Food);
        assert_eq!(word, m.word(Cell::Food));
    }

    #[test]
    fn silent_once_faded() {
        let mut s = sim();
        starve(&mut s, 40);
        let teaching = Teaching {
            fade: s.t(),
            ..Teaching::default()
        };
        let mut m = Mentor::new(teaching, 0);
        assert!((0..100).all(|_| m.say(&s).is_none()));
    }

    #[test]
    fn a_blind_mentor_points_at_empty_floor() {
        let mut s = sim();
        starve(&mut s, 40);
        let teaching = Teaching {
            fade: 1_000_000,
            ..Teaching::default()
        };
        let mut m = Mentor::blind(teaching, &s, 5, 0);
        let (at, _) = m.say(&s).expect("speaks early on");
        let start = s.start();
        assert_eq!(s.grid().get(at.shift(start.x, start.y)), Cell::Empty);
    }

    fn foretelling(foretells: Foretell) -> Mentor {
        let teaching = Teaching {
            fade: 1_000_000,
            foretells,
            ..Teaching::default()
        };
        Mentor::new(teaching, 0)
    }

    /// A small world with `regrowth` where the apprentice steps east onto
    /// food and eats it; returns the world and the meal.
    fn fed(regrowth: Regrowth) -> (Sim, Outcome) {
        let mut grid = Grid::empty(5, 5);
        grid.set(Pos { x: 2, y: 2 }, Cell::Food);
        let config = Config {
            regrowth,
            ..Config::default()
        };
        let mut s = Sim::with_start(config, grid, Pos { x: 1, y: 2 }, Rng::new(0));
        s.step(Action::Move(Dir::East));
        let meal = s.step(Action::Eat);
        (s, meal)
    }

    #[test]
    fn it_foretells_what_an_eaten_square_will_do() {
        for (regrowth, word) in [
            (Regrowth::InPlace { delay: 30 }, GROWS_BACK),
            (Regrowth::Anywhere, STAYS_GONE),
        ] {
            let (s, outcome) = fed(regrowth);
            let mut m = foretelling(Foretell::Truly);
            let start = s.start();
            let at = s.pos().shift(-start.x, -start.y);
            assert_eq!(m.foretell(&s, &outcome), Some((at, m.said(word))));
        }
    }

    #[test]
    fn it_foretells_only_after_a_meal_and_only_if_asked() {
        let mut s = sim();
        let rested = s.step(Action::Rest);
        assert_eq!(foretelling(Foretell::Truly).foretell(&s, &rested), None);
        let (s, ate) = fed(Regrowth::Anywhere);
        assert_eq!(Mentor::new(Teaching::default(), 0).foretell(&s, &ate), None);
    }

    #[test]
    fn a_random_foreteller_says_both_words_in_one_world() {
        let (s, ate) = fed(Regrowth::Anywhere);
        let mut m = foretelling(Foretell::AtRandom);
        let words: HashSet<Word> = (0..50)
            .filter_map(|_| m.foretell(&s, &ate).map(|(_, s)| s.word))
            .collect();
        assert_eq!(words, HashSet::from([GROWS_BACK, STAYS_GONE]));
    }

    #[test]
    fn words_differ_by_kind_and_by_mentor() {
        let a = Mentor::new(Teaching::default(), 0);
        let words: HashSet<Word> = KINDS_ALL.iter().map(|&c| a.word(c)).collect();
        assert_eq!(words.len(), KINDS);
        let differs = (1..20).any(|seed| {
            let b = Mentor::new(Teaching::default(), seed);
            KINDS_ALL.iter().any(|&c| a.word(c) != b.word(c))
        });
        assert!(differs);
    }

    #[test]
    fn a_naming_mentor_names_what_the_apprentice_sees() {
        let s = sim();
        let teaching = Teaching {
            names: true,
            fade: 1_000_000,
            ..Teaching::default()
        };
        let mut m = Mentor::new(teaching, 0);
        let start = s.start();
        for _ in 0..50 {
            let (at, Said { word, .. }) = m.say(&s).expect("names while fed");
            let real = at.shift(start.x, start.y);
            assert!((real.x - s.pos().x).abs() <= 1 && (real.y - s.pos().y).abs() <= 1);
            assert_eq!(word, m.word(s.grid().get(real)));
        }
    }
}
