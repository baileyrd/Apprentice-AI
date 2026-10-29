//! The rules of the world: energy, eating, hazards and sleep.
//!
//! Pure state machine: `Sim::step` takes an action and returns what happened.
//! No I/O, no clocks.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::body::{Action, Observation};
use crate::rng::Rng;
use crate::world::{Cell, Grid, Layout, Pos};

/// Energy economics. All costs are per step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Physiology {
    pub max_energy: u32,
    pub start_energy: u32,
    /// Paid every step, whatever the action.
    pub basal_cost: u32,
    /// Extra cost of trying to move (paid even when bumping a wall).
    pub move_cost: u32,
    /// Extra cost of stepping onto a hazard.
    pub hazard_cost: u32,
    pub food_value: u32,
}

impl Default for Physiology {
    fn default() -> Self {
        Self {
            max_energy: 100,
            start_energy: 50,
            basal_cost: 1,
            move_cost: 1,
            hazard_cost: 10,
            food_value: 25,
        }
    }
}

/// Where food grows back after it is eaten.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Regrowth {
    /// At once, on a random empty square (phases 0-2).
    #[default]
    Anywhere,
    /// On the same square, `delay` steps later, like a bush: food has places
    /// worth remembering.
    InPlace { delay: u64 },
}

/// Full world configuration.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub layout: Layout,
    pub physiology: Physiology,
    #[serde(default)]
    pub regrowth: Regrowth,
}

/// What an action did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    Moved,
    Bumped,
    Hurt,
    Ate,
    /// Tried to eat where there is no food.
    AteNothing,
    Rested,
}

/// Result of one step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    pub event: Event,
    /// Energy after the step (after any respawn).
    pub energy: u32,
    /// Energy hit zero: the agent slept and woke at its start square.
    /// Its knowledge is untouched; only time was lost.
    pub slept: bool,
}

/// Why a world could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimError {
    NoRoomForAgent,
}

impl fmt::Display for SimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SimError::NoRoomForAgent => write!(f, "no empty square to start the agent on"),
        }
    }
}

impl std::error::Error for SimError {}

/// A running world with one apprentice in it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sim {
    config: Config,
    grid: Grid,
    start: Pos,
    pos: Pos,
    energy: u32,
    t: u64,
    rng: Rng,
    /// Eaten food squares waiting to grow back, with the step they are due.
    #[serde(default)]
    regrowing: Vec<(Pos, u64)>,
}

impl Sim {
    /// Generates a world from `seed` and places the agent on an empty square.
    pub fn new(config: Config, seed: u64) -> Result<Self, SimError> {
        let mut rng = Rng::new(seed);
        let grid = Grid::generate(&config.layout, &mut rng);
        Self::with_grid(config, grid, rng)
    }

    /// Uses a given grid; the agent starts on a random empty square.
    pub fn with_grid(config: Config, grid: Grid, mut rng: Rng) -> Result<Self, SimError> {
        let start = rng
            .pick(&grid.positions_of(Cell::Empty))
            .ok_or(SimError::NoRoomForAgent)?;
        Ok(Self::with_start(config, grid, start, rng))
    }

    /// Uses a given grid and wake square, which is cleared if it held
    /// anything.
    pub fn with_start(config: Config, mut grid: Grid, start: Pos, rng: Rng) -> Self {
        grid.set(start, Cell::Empty);
        let energy = config.physiology.start_energy;
        Self {
            config,
            grid,
            start,
            pos: start,
            energy,
            t: 0,
            rng,
            regrowing: Vec::new(),
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    pub fn pos(&self) -> Pos {
        self.pos
    }

    /// The square the agent wakes on after sleep.
    pub fn start(&self) -> Pos {
        self.start
    }

    pub fn energy(&self) -> u32 {
        self.energy
    }

    /// Steps taken so far.
    pub fn t(&self) -> u64 {
        self.t
    }

    /// What the agent senses right now.
    pub fn observe(&self) -> Observation {
        let offset = self.pos.shift(-self.start.x, -self.start.y);
        Observation::sense(&self.grid, self.pos, self.energy, offset)
    }

    /// Applies one action and advances time.
    pub fn step(&mut self, action: Action) -> Outcome {
        let (event, cost, gain) = match action {
            Action::Rest => (Event::Rested, 0, 0),
            Action::Eat => self.eat(),
            Action::Move(dir) => self.walk(self.pos.step(dir)),
        };
        let phys = &self.config.physiology;
        let total_cost = i64::from(phys.basal_cost) + i64::from(cost);
        let energy = i64::from(self.energy) - total_cost + i64::from(gain);
        self.energy = energy.clamp(0, i64::from(phys.max_energy)) as u32;
        self.t += 1;

        let slept = self.energy == 0;
        if slept {
            self.pos = self.start;
            self.energy = phys.start_energy;
        }
        self.regrow();
        Outcome {
            event,
            energy: self.energy,
            slept,
        }
    }

    /// Moves `count` walls to random empty squares, never onto the agent or
    /// its wake square. Returns how many moved (fewer if walls or room ran out).
    pub fn move_walls(&mut self, count: usize) -> usize {
        let mut moved = 0;
        for _ in 0..count {
            let Some(from) = self.rng.pick(&self.grid.positions_of(Cell::Wall)) else {
                break;
            };
            let room: Vec<Pos> = self
                .grid
                .positions_of(Cell::Empty)
                .into_iter()
                .filter(|&p| p != self.pos && p != self.start)
                .collect();
            let Some(to) = self.rng.pick(&room) else {
                break;
            };
            self.grid.set(from, Cell::Empty);
            self.grid.set(to, Cell::Wall);
            moved += 1;
        }
        moved
    }

    /// Moves every food square (grown or regrowing) to a random empty square
    /// that held none before, never under the agent. Returns how many moved.
    pub fn move_food(&mut self) -> usize {
        let mut old = self.grid.positions_of(Cell::Food);
        for &p in &old {
            self.grid.set(p, Cell::Empty);
        }
        old.extend(self.regrowing.drain(..).map(|(p, _)| p));
        let mut moved = 0;
        for _ in &old {
            let room: Vec<Pos> = self
                .grid
                .positions_of(Cell::Empty)
                .into_iter()
                .filter(|p| *p != self.pos && !old.contains(p))
                .collect();
            let Some(to) = self.rng.pick(&room) else {
                break;
            };
            self.grid.set(to, Cell::Food);
            moved += 1;
        }
        moved
    }

    /// Returns `(event, extra cost, gain)`.
    fn eat(&mut self) -> (Event, u32, u32) {
        if self.grid.get(self.pos) != Cell::Food {
            return (Event::AteNothing, 0, 0);
        }
        self.grid.set(self.pos, Cell::Empty);
        match self.config.regrowth {
            Regrowth::Anywhere => {
                self.grid.place_randomly(Cell::Food, &mut self.rng);
            }
            Regrowth::InPlace { delay } => self.regrowing.push((self.pos, self.t + 1 + delay)),
        }
        (Event::Ate, 0, self.config.physiology.food_value)
    }

    /// Grows back due food on empty squares the agent is not standing on.
    fn regrow(&mut self) {
        let (t, pos, grid) = (self.t, self.pos, &mut self.grid);
        self.regrowing.retain(|&(p, due)| {
            let ready = due <= t && p != pos && grid.get(p) == Cell::Empty;
            if ready {
                grid.set(p, Cell::Food);
            }
            !ready
        });
    }

    /// Returns `(event, extra cost, gain)`.
    fn walk(&mut self, to: Pos) -> (Event, u32, u32) {
        let phys = &self.config.physiology;
        match self.grid.get(to) {
            Cell::Wall => (Event::Bumped, phys.move_cost, 0),
            Cell::Hazard => {
                self.pos = to;
                (Event::Hurt, phys.move_cost + phys.hazard_cost, 0)
            }
            Cell::Empty | Cell::Food => {
                self.pos = to;
                (Event::Moved, phys.move_cost, 0)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Dir;

    /// 3×1 corridor: agent at x=0, then `middle`, then `right`.
    fn corridor(middle: Cell, right: Cell) -> Sim {
        let mut g = Grid::empty(3, 1);
        g.set(Pos { x: 1, y: 0 }, middle);
        g.set(Pos { x: 2, y: 0 }, right);
        let mut sim = Sim::with_grid(Config::default(), g, Rng::new(0)).expect("room");
        sim.start = Pos { x: 0, y: 0 };
        sim.pos = sim.start;
        sim
    }

    #[test]
    fn rest_pays_basal_only() {
        let mut sim = corridor(Cell::Empty, Cell::Empty);
        let out = sim.step(Action::Rest);
        assert_eq!((out.event, out.energy), (Event::Rested, 49));
    }

    #[test]
    fn bumping_a_wall_costs_and_stays_put() {
        let mut sim = corridor(Cell::Wall, Cell::Empty);
        let out = sim.step(Action::Move(Dir::East));
        assert_eq!((out.event, out.energy, sim.pos().x), (Event::Bumped, 48, 0));
    }

    #[test]
    fn hazard_hurts_but_moves() {
        let mut sim = corridor(Cell::Hazard, Cell::Empty);
        let out = sim.step(Action::Move(Dir::East));
        assert_eq!((out.event, out.energy, sim.pos().x), (Event::Hurt, 38, 1));
    }

    #[test]
    fn food_regrows_in_place_after_the_delay_once_the_agent_leaves() {
        let mut sim = corridor(Cell::Food, Cell::Empty);
        sim.config.regrowth = Regrowth::InPlace { delay: 2 };
        sim.step(Action::Move(Dir::East));
        sim.step(Action::Eat);
        assert!(sim.grid().positions_of(Cell::Food).is_empty());
        sim.step(Action::Rest);
        sim.step(Action::Rest); // due, but the agent stands on it
        assert_eq!(sim.grid().get(Pos { x: 1, y: 0 }), Cell::Empty);
        sim.step(Action::Move(Dir::East));
        assert_eq!(sim.grid().get(Pos { x: 1, y: 0 }), Cell::Food);
    }

    #[test]
    fn moving_food_takes_regrowing_bushes_along() {
        let mut sim = Sim::new(Config::default(), 3).expect("room");
        sim.config.regrowth = Regrowth::InPlace { delay: 50 };
        let food = sim.grid().positions_of(Cell::Food);
        sim.pos = food[0];
        sim.step(Action::Eat);
        assert_eq!(sim.move_food(), food.len());
        let after = sim.grid().positions_of(Cell::Food);
        assert_eq!(after.len(), food.len());
        assert!(after.iter().all(|p| !food.contains(p)));
        for _ in 0..60 {
            sim.step(Action::Rest);
        }
        assert_eq!(sim.grid().positions_of(Cell::Food).len(), food.len());
    }

    #[test]
    fn eating_gains_energy_and_food_regrows_elsewhere() {
        let mut sim = corridor(Cell::Food, Cell::Empty);
        sim.step(Action::Move(Dir::East));
        let out = sim.step(Action::Eat);
        assert_eq!((out.event, out.energy), (Event::Ate, 72));
        assert_eq!(sim.grid().positions_of(Cell::Food).len(), 1);
        assert_ne!(sim.grid().get(sim.pos()), Cell::Food);
    }

    #[test]
    fn eating_nothing_is_wasted_time() {
        let mut sim = corridor(Cell::Empty, Cell::Empty);
        let out = sim.step(Action::Eat);
        assert_eq!((out.event, out.energy), (Event::AteNothing, 49));
    }

    #[test]
    fn energy_caps_at_max() {
        let mut sim = corridor(Cell::Empty, Cell::Empty);
        sim.grid.set(sim.pos, Cell::Food);
        sim.energy = 99;
        assert_eq!(sim.step(Action::Eat).energy, 100);
    }

    #[test]
    fn zero_energy_sleeps_and_respawns() {
        let mut sim = corridor(Cell::Empty, Cell::Empty);
        sim.step(Action::Move(Dir::East));
        sim.energy = 1;
        let out = sim.step(Action::Rest);
        assert!(out.slept);
        assert_eq!((sim.pos(), out.energy), (Pos { x: 0, y: 0 }, 50));
    }

    #[test]
    fn moving_walls_keeps_their_count_and_spares_the_agent() {
        let mut sim = Sim::new(Config::default(), 3).expect("room");
        let before = sim.grid().positions_of(Cell::Wall);
        let moved = sim.move_walls(before.len());
        let after = sim.grid().positions_of(Cell::Wall);
        assert_eq!((moved, after.len()), (before.len(), before.len()));
        assert_ne!(before, after);
        assert_ne!(sim.grid().get(sim.pos()), Cell::Wall);
        assert_ne!(sim.grid().get(sim.start()), Cell::Wall);
    }

    #[test]
    fn moving_walls_on_a_wall_free_grid_moves_none() {
        let mut sim = corridor(Cell::Empty, Cell::Empty);
        assert_eq!(sim.move_walls(3), 0);
    }

    #[test]
    fn moving_food_keeps_the_count_and_uses_new_squares() {
        let mut sim = Sim::new(Config::default(), 4).expect("room");
        let before = sim.grid().positions_of(Cell::Food);
        assert_eq!(sim.move_food(), before.len());
        let after = sim.grid().positions_of(Cell::Food);
        assert_eq!(after.len(), before.len());
        assert!(after.iter().all(|p| !before.contains(p) && *p != sim.pos()));
    }

    #[test]
    fn full_grid_has_no_room() {
        let mut g = Grid::empty(1, 1);
        g.set(Pos { x: 0, y: 0 }, Cell::Wall);
        assert_eq!(
            Sim::with_grid(Config::default(), g, Rng::new(0)),
            Err(SimError::NoRoomForAgent)
        );
    }
}
