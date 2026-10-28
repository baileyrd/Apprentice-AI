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

/// Full world configuration.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub layout: Layout,
    pub physiology: Physiology,
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
        let energy = config.physiology.start_energy;
        Ok(Self {
            config,
            grid,
            start,
            pos: start,
            energy,
            t: 0,
            rng,
        })
    }

    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    pub fn pos(&self) -> Pos {
        self.pos
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
        Observation::sense(&self.grid, self.pos, self.energy)
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
        Outcome {
            event,
            energy: self.energy,
            slept,
        }
    }

    /// Returns `(event, extra cost, gain)`.
    fn eat(&mut self) -> (Event, u32, u32) {
        if self.grid.get(self.pos) != Cell::Food {
            return (Event::AteNothing, 0, 0);
        }
        self.grid.set(self.pos, Cell::Empty);
        self.grid.place_randomly(Cell::Food, &mut self.rng);
        (Event::Ate, 0, self.config.physiology.food_value)
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
    fn full_grid_has_no_room() {
        let mut g = Grid::empty(1, 1);
        g.set(Pos { x: 0, y: 0 }, Cell::Wall);
        assert_eq!(
            Sim::with_grid(Config::default(), g, Rng::new(0)),
            Err(SimError::NoRoomForAgent)
        );
    }
}
