//! The apprentice's body: what it senses and what it can do.

use serde::{Deserialize, Serialize};

use crate::world::{Cell, Dir, Grid, Pos};

/// Side length of the egocentric view.
pub const VIEW: usize = 3;

/// Everything the agent gets to know about the world on one step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    /// The 3×3 square around the agent, row-major, north row first; index 4 is
    /// the agent's own square.
    pub view: [Cell; VIEW * VIEW],
    pub energy: u32,
    /// Path integration: displacement from the square it last woke on.
    /// A sense of its own body, not knowledge of the world.
    pub offset: Pos,
}

impl Observation {
    /// Index of the agent's own square in `view`.
    pub const CENTER: usize = VIEW * VIEW / 2;

    /// `(dx, dy)` of view index `i` relative to the agent.
    pub fn view_delta(i: usize) -> (i32, i32) {
        let (i, side) = (i as i32, VIEW as i32);
        (i % side - 1, i / side - 1)
    }

    /// Senses the grid around `pos`, which lies `offset` from the wake square.
    pub fn sense(grid: &Grid, pos: Pos, energy: u32, offset: Pos) -> Self {
        let mut view = [Cell::Wall; VIEW * VIEW];
        for (i, cell) in view.iter_mut().enumerate() {
            let (dx, dy) = Self::view_delta(i);
            *cell = grid.get(pos.shift(dx, dy));
        }
        Self {
            view,
            energy,
            offset,
        }
    }

    /// The cell next to the agent in `dir`.
    pub fn toward(&self, dir: Dir) -> Cell {
        let (dx, dy) = dir.delta();
        self.view[((dy + 1) * 3 + dx + 1) as usize]
    }

    /// The cell the agent stands on.
    pub fn here(&self) -> Cell {
        self.view[Self::CENTER]
    }
}

/// A sound the mentor makes about a square. It means nothing until the
/// apprentice links it to what it sees there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Word(pub u8);

/// Whose voice a word was said in. The apprentice can tell voices apart but
/// knows nothing else about who is speaking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Voice(pub u8);

/// A word, and the voice it was said in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Said {
    pub voice: Voice,
    pub word: Word,
}

/// What the agent can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Move(Dir),
    Eat,
    Rest,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sense_reads_neighbours_and_walls_at_edge() {
        let mut g = Grid::empty(3, 3);
        g.set(Pos { x: 1, y: 0 }, Cell::Food);
        let origin = Pos { x: 0, y: 0 };
        let obs = Observation::sense(&g, Pos { x: 1, y: 1 }, 10, origin);
        assert_eq!(obs.toward(Dir::North), Cell::Food);
        assert_eq!(obs.here(), Cell::Empty);

        let corner = Observation::sense(&g, Pos { x: 0, y: 0 }, 10, origin);
        assert_eq!(corner.toward(Dir::West), Cell::Wall);
        assert_eq!(corner.toward(Dir::North), Cell::Wall);
        assert_eq!(corner.toward(Dir::East), Cell::Food);
    }
}
