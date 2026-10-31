//! A hand-coded patroller: a baseline with no learning. It eats when it
//! stands on food; otherwise it walks toward the square it has gone longest
//! without seeing, among those it can reach through squares it has seen
//! open. It remembers what each square looked like when last seen and
//! nothing else.

use std::collections::{HashMap, VecDeque};

use crate::body::{Action, Observation};
use crate::world::{Cell, Dir, Pos};

/// The patroller, in its own frame (relative to the wake square).
#[derive(Debug, Clone, Default)]
pub struct Patroller {
    /// Each square seen: how it looked and when.
    seen: HashMap<Pos, (Cell, u64)>,
    t: u64,
}

impl Patroller {
    pub fn new() -> Self {
        Self::default()
    }

    /// Looks, then chooses.
    pub fn act(&mut self, obs: &Observation) -> Action {
        self.t += 1;
        for (i, &cell) in obs.view.iter().enumerate() {
            let (dx, dy) = Observation::view_delta(i);
            self.seen.insert(obs.offset.shift(dx, dy), (cell, self.t));
        }
        if obs.view[Observation::CENTER] == Cell::Food {
            return Action::Eat;
        }
        match self.first_step(obs.offset) {
            Some(dir) => Action::Move(dir),
            None => Action::Rest,
        }
    }

    fn open(&self, at: Pos) -> bool {
        matches!(self.seen.get(&at), Some((Cell::Empty | Cell::Food, _)))
    }

    /// Steps since `at` was last seen; never seen is longest of all.
    fn staleness(&self, at: Pos) -> u64 {
        self.seen
            .get(&at)
            .map_or(u64::MAX, |&(_, last)| self.t - last)
    }

    /// First move of a shortest path to the stalest reachable square: open
    /// squares it can walk to, and never-seen squares next to them. Ties go
    /// to the nearest, then to the first found. `None` if nothing is
    /// staler than where it stands.
    fn first_step(&self, from: Pos) -> Option<Dir> {
        let mut first: HashMap<Pos, Option<Dir>> = HashMap::from([(from, None)]);
        let mut queue = VecDeque::from([from]);
        let mut best: Option<(u64, Dir)> = None;
        while let Some(at) = queue.pop_front() {
            for dir in Dir::ALL {
                let next = at.step(dir);
                if first.contains_key(&next) {
                    continue;
                }
                let step = first[&at].unwrap_or(dir);
                let unseen = !self.seen.contains_key(&next);
                if !unseen && !self.open(next) {
                    continue;
                }
                first.insert(next, Some(step));
                let stale = self.staleness(next);
                if best.is_none_or(|(s, _)| stale > s) {
                    best = Some((stale, step));
                }
                if !unseen {
                    queue.push_back(next);
                }
            }
        }
        best.filter(|&(s, _)| s > 0).map(|(_, dir)| dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::VIEW;

    fn obs(view: [Cell; VIEW * VIEW], offset: Pos) -> Observation {
        Observation {
            view,
            energy: 50,
            offset,
        }
    }

    const ORIGIN: Pos = Pos { x: 0, y: 0 };

    #[test]
    fn it_eats_where_it_stands_on_food() {
        let mut view = [Cell::Empty; VIEW * VIEW];
        view[Observation::CENTER] = Cell::Food;
        assert_eq!(Patroller::new().act(&obs(view, ORIGIN)), Action::Eat);
    }

    #[test]
    fn it_heads_for_the_square_it_saw_longest_ago() {
        let mut p = Patroller::new();
        let walls = [Cell::Wall; VIEW * VIEW];
        // A corridor running east: walls everywhere except the row y = 0.
        let mut corridor = walls;
        corridor[3..6].copy_from_slice(&[Cell::Empty; 3]);
        for x in [0, 1, 2] {
            p.act(&obs(corridor, Pos { x, y: 0 }));
        }
        // Its east end is a wall.
        let mut end = corridor;
        end[5] = Cell::Wall;
        p.act(&obs(end, Pos { x: 3, y: 0 }));
        // Back at x = 1 the west end (x = -1) is stalest: seen at t = 1.
        let action = p.act(&obs(corridor, Pos { x: 1, y: 0 }));
        assert_eq!(action, Action::Move(Dir::West));
    }

    #[test]
    fn it_avoids_hazards_and_walls() {
        let mut view = [Cell::Hazard; VIEW * VIEW];
        view[Observation::CENTER] = Cell::Empty;
        view[1] = Cell::Wall; // north
        view[7] = Cell::Empty; // south
        let mut p = Patroller::new();
        assert_eq!(p.act(&obs(view, ORIGIN)), Action::Move(Dir::South));
    }
}
