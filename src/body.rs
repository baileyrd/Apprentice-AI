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
}

impl Observation {
    /// Index of the agent's own square in `view`.
    pub const CENTER: usize = VIEW * VIEW / 2;

    /// Senses the grid around `pos`.
    pub fn sense(grid: &Grid, pos: Pos, energy: u32) -> Self {
        let mut view = [Cell::Wall; VIEW * VIEW];
        for (i, cell) in view.iter_mut().enumerate() {
            let (dx, dy) = (i as i32 % 3 - 1, i as i32 / 3 - 1);
            *cell = grid.get(Pos {
                x: pos.x + dx,
                y: pos.y + dy,
            });
        }
        Self { view, energy }
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
        let obs = Observation::sense(&g, Pos { x: 1, y: 1 }, 10);
        assert_eq!(obs.toward(Dir::North), Cell::Food);
        assert_eq!(obs.here(), Cell::Empty);

        let corner = Observation::sense(&g, Pos { x: 0, y: 0 }, 10);
        assert_eq!(corner.toward(Dir::West), Cell::Wall);
        assert_eq!(corner.toward(Dir::North), Cell::Wall);
        assert_eq!(corner.toward(Dir::East), Cell::Food);
    }
}
