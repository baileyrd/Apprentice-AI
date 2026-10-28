//! The mentor ("big brother"): it knows the map, the apprentice doesn't.
//!
//! It never writes into the apprentice's model. When the apprentice is stuck
//! (hungry), it may point at a square it believes holds food, in the
//! apprentice's own frame: "look there". The apprentice still has to walk
//! there and see for itself, and it learns how far to trust the pointing.
//! The help fades like stabilisers on a bike: the chance of a hint falls
//! linearly to zero at `fade` steps.
//!
//! A blind mentor believes in food on squares that never hold any, to test
//! whether the apprentice inherits its mentor's blind spots.

use std::collections::{HashSet, VecDeque};

use crate::rng::Rng;
use crate::sim::Sim;
use crate::world::{Cell, Dir, Grid, Pos};

/// When and for how long the mentor helps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Teaching {
    /// The apprentice counts as stuck below this energy.
    pub stuck: u32,
    /// Step at which the mentor falls silent.
    pub fade: u64,
}

impl Default for Teaching {
    fn default() -> Self {
        Self {
            stuck: 40,
            fade: 4_000,
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
    rng: Rng,
}

impl Mentor {
    /// A mentor that sees the real map.
    pub fn new(teaching: Teaching, seed: u64) -> Self {
        Self {
            teaching,
            sight: Sight::True,
            rng: Rng::new(seed),
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
            rng,
        }
    }

    /// The square it points at this step, relative to the apprentice's wake
    /// square; `None` when it stays quiet.
    pub fn hint(&mut self, sim: &Sim) -> Option<Pos> {
        let Teaching { stuck, fade } = self.teaching;
        if sim.energy() >= stuck || sim.t() >= fade {
            return None;
        }
        if !self.rng.chance(1.0 - sim.t() as f64 / fade as f64) {
            return None;
        }
        let food = match &self.sight {
            Sight::True => sim.grid().positions_of(Cell::Food),
            Sight::Blind(phantoms) => phantoms.clone(),
        };
        let target = nearest(sim.grid(), sim.pos(), &food)?;
        let start = sim.start();
        Some(target.shift(-start.x, -start.y))
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
        assert_eq!(m.hint(&sim()), None);
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
        let at = m.hint(&s).expect("speaks early on");
        let start = s.start();
        let real = at.shift(start.x, start.y);
        assert_eq!(s.grid().get(real), Cell::Food);
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
        assert!((0..100).all(|_| m.hint(&s).is_none()));
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
        let at = m.hint(&s).expect("speaks early on");
        let start = s.start();
        assert_eq!(s.grid().get(at.shift(start.x, start.y)), Cell::Empty);
    }
}
