//! The grid: cells, positions and directions. Pure data, no rules.

use serde::{Deserialize, Serialize};

use crate::rng::Rng;

/// What occupies a square.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Cell {
    Empty,
    Wall,
    Food,
    Hazard,
}

/// A square on the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

/// One of four compass moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dir {
    North,
    East,
    South,
    West,
}

impl Dir {
    pub const ALL: [Dir; 4] = [Dir::North, Dir::East, Dir::South, Dir::West];

    /// Unit offset `(dx, dy)`; north is `-y`.
    pub fn delta(self) -> (i32, i32) {
        match self {
            Dir::North => (0, -1),
            Dir::East => (1, 0),
            Dir::South => (0, 1),
            Dir::West => (-1, 0),
        }
    }
}

impl Pos {
    /// The neighbouring square in `dir`.
    pub fn step(self, dir: Dir) -> Pos {
        let (dx, dy) = dir.delta();
        self.shift(dx, dy)
    }

    /// This square moved by `(dx, dy)`.
    pub fn shift(self, dx: i32, dy: i32) -> Pos {
        Pos {
            x: self.x + dx,
            y: self.y + dy,
        }
    }
}

/// Layout parameters for a generated grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub width: i32,
    pub height: i32,
    pub wall_density: f64,
    pub hazard_density: f64,
    pub food_count: usize,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            width: 12,
            height: 12,
            wall_density: 0.12,
            hazard_density: 0.05,
            food_count: 5,
        }
    }
}

/// A rectangular grid. Anything outside its bounds reads as `Wall`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Grid {
    width: i32,
    height: i32,
    cells: Vec<Cell>,
}

impl Grid {
    /// An all-empty grid. Panics on non-positive dimensions (a programming error).
    pub fn empty(width: i32, height: i32) -> Self {
        assert!(width > 0 && height > 0, "grid dimensions must be positive");
        Self {
            width,
            height,
            cells: vec![Cell::Empty; (width * height) as usize],
        }
    }

    /// Random walls and hazards, then `food_count` food on empty squares.
    pub fn generate(layout: &Layout, rng: &mut Rng) -> Self {
        let mut grid = Self::empty(layout.width, layout.height);
        for i in 0..grid.cells.len() {
            grid.cells[i] = if rng.chance(layout.wall_density) {
                Cell::Wall
            } else if rng.chance(layout.hazard_density) {
                Cell::Hazard
            } else {
                Cell::Empty
            };
        }
        for _ in 0..layout.food_count {
            grid.place_randomly(Cell::Food, rng);
        }
        grid
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn in_bounds(&self, p: Pos) -> bool {
        (0..self.width).contains(&p.x) && (0..self.height).contains(&p.y)
    }

    /// The cell at `p`; out of bounds is `Wall`.
    pub fn get(&self, p: Pos) -> Cell {
        if !self.in_bounds(p) {
            return Cell::Wall;
        }
        self.cells[self.index(p)]
    }

    /// Sets an in-bounds cell; out-of-bounds writes are ignored.
    pub fn set(&mut self, p: Pos, cell: Cell) {
        if !self.in_bounds(p) {
            return;
        }
        let i = self.index(p);
        self.cells[i] = cell;
    }

    /// Every cell, row-major.
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// Every position, row-major.
    pub fn positions(&self) -> impl Iterator<Item = Pos> + '_ {
        (0..self.cells.len()).map(|i| self.pos_of(i))
    }

    /// All positions holding `cell`, row-major.
    pub fn positions_of(&self, cell: Cell) -> Vec<Pos> {
        (0..self.cells.len())
            .filter(|&i| self.cells[i] == cell)
            .map(|i| self.pos_of(i))
            .collect()
    }

    /// Puts `cell` on a random empty square. Returns where, or `None` if full.
    pub fn place_randomly(&mut self, cell: Cell, rng: &mut Rng) -> Option<Pos> {
        let p = rng.pick(&self.positions_of(Cell::Empty))?;
        self.set(p, cell);
        Some(p)
    }

    fn index(&self, p: Pos) -> usize {
        (p.y * self.width + p.x) as usize
    }

    fn pos_of(&self, i: usize) -> Pos {
        let i = i as i32;
        Pos {
            x: i % self.width,
            y: i / self.width,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn out_of_bounds_is_wall() {
        let g = Grid::empty(3, 3);
        assert_eq!(g.get(Pos { x: -1, y: 0 }), Cell::Wall);
        assert_eq!(g.get(Pos { x: 3, y: 1 }), Cell::Wall);
        assert_eq!(g.get(Pos { x: 1, y: 1 }), Cell::Empty);
    }

    #[test]
    fn generate_is_deterministic_and_places_food() {
        let layout = Layout::default();
        let a = Grid::generate(&layout, &mut Rng::new(42));
        let b = Grid::generate(&layout, &mut Rng::new(42));
        assert_eq!(a, b);
        assert_eq!(a.positions_of(Cell::Food).len(), layout.food_count);
    }

    #[test]
    fn place_on_full_grid_is_none() {
        let mut g = Grid::empty(1, 1);
        g.set(Pos { x: 0, y: 0 }, Cell::Wall);
        assert_eq!(g.place_randomly(Cell::Food, &mut Rng::new(0)), None);
    }

    #[test]
    fn north_is_up() {
        assert_eq!(Pos { x: 2, y: 2 }.step(Dir::North), Pos { x: 2, y: 1 });
    }
}
