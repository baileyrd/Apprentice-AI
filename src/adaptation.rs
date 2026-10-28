//! Phase 2 experiment: after the walls move, how long until it adapts?
//!
//! A mind lives `warmup` steps in one world, then half the walls move and it
//! lives `after` more. Adaptation time is the number of steps after the move
//! until both hold at once:
//! - **surprise is back at its floor**: the mean over the last `trail` steps
//!   (all after the move) is at most twice the mean over the `floor_window`
//!   steps before the move (and never asked below 0.001);
//! - **the believed map is right again**: the share of squares whose believed
//!   structure (wall, hazard or open) matches reality is back to where it was
//!   just before the move.

use crate::apprentice::Mind;
use crate::metrics::{Metrics, Summary};
use crate::sim::{Config, Sim, SimError};
use crate::world::{Cell, Pos};

/// What to run.
#[derive(Debug, Clone)]
pub struct Setup {
    pub config: Config,
    pub seed: u64,
    pub warmup: usize,
    pub after: usize,
    /// Share of walls that move.
    pub wall_fraction: f64,
    pub floor_window: usize,
    pub trail: usize,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            config: Config::default(),
            seed: 1,
            warmup: 3_000,
            after: 5_000,
            wall_fraction: 0.5,
            floor_window: 1_000,
            trail: 100,
        }
    }
}

/// Step-by-step record of one life.
#[derive(Debug, Clone, PartialEq)]
pub struct Trace {
    /// Surprise per step.
    pub surprise: Vec<f64>,
    /// Structural map accuracy per step, after that step.
    pub accuracy: Vec<f64>,
    /// Index of the first step after the walls moved.
    pub change: usize,
    pub summary: Summary,
}

/// How long adaptation took, in steps after the move; `None` if never.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Recovery {
    pub floor: f64,
    pub baseline_accuracy: f64,
    pub surprise: Option<usize>,
    pub map: Option<usize>,
    pub both: Option<usize>,
}

/// Wall, hazard or open floor (food lies on open floor and moves anyway).
fn structure(cell: Cell) -> Cell {
    match cell {
        Cell::Wall | Cell::Hazard => cell,
        Cell::Empty | Cell::Food => Cell::Empty,
    }
}

/// Share of squares whose believed structure matches reality.
pub fn accuracy(sim: &Sim, mind: &impl Mind) -> f64 {
    let grid = sim.grid();
    let start = sim.start();
    let right = grid
        .positions()
        .filter(|&p| {
            let believed = mind.believes(Pos {
                x: p.x - start.x,
                y: p.y - start.y,
            });
            believed.map(structure) == Some(structure(grid.get(p)))
        })
        .count();
    right as f64 / grid.cells().len() as f64
}

/// Lives one life with a wall move in the middle.
pub fn run(setup: &Setup, mind: &mut impl Mind) -> Result<Trace, SimError> {
    let mut sim = Sim::new(setup.config.clone(), setup.seed)?;
    let walls = sim.grid().positions_of(Cell::Wall).len();
    let to_move = (walls as f64 * setup.wall_fraction).round() as usize;
    let mut metrics = Metrics::default();
    let total = setup.warmup + setup.after;
    let mut surprise = Vec::with_capacity(total);
    let mut acc = Vec::with_capacity(total);
    for t in 0..total {
        if t == setup.warmup {
            sim.move_walls(to_move);
        }
        let action = mind.act(&sim.observe());
        let outcome = sim.step(action);
        mind.feel(&outcome);
        metrics.record(&outcome);
        surprise.push(mind.surprise());
        acc.push(accuracy(&sim, mind));
    }
    Ok(Trace {
        surprise,
        accuracy: acc,
        change: setup.warmup,
        summary: metrics.summary(),
    })
}

/// Applies the adaptation rule to a trace.
pub fn recovery(trace: &Trace, floor_window: usize, trail: usize) -> Recovery {
    let c = trace.change;
    let mean = |xs: &[f64]| xs.iter().sum::<f64>() / xs.len().max(1) as f64;
    let floor = mean(&trace.surprise[c.saturating_sub(floor_window)..c]);
    let limit = (2.0 * floor).max(1e-3);
    let baseline_accuracy = if c > 0 { trace.accuracy[c - 1] } else { 0.0 };

    let calm = |i: usize| i + 1 >= c + trail && mean(&trace.surprise[i + 1 - trail..=i]) <= limit;
    let mapped = |i: usize| trace.accuracy[i] >= baseline_accuracy;
    let first = |ok: &dyn Fn(usize) -> bool| {
        (c..trace.surprise.len())
            .find(|&i| ok(i))
            .map(|i| i + 1 - c)
    };
    Recovery {
        floor,
        baseline_accuracy,
        surprise: first(&calm),
        map: first(&mapped),
        both: first(&|i| calm(i) && mapped(i)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::ReflexAgent;
    use crate::apprentice::{Apprentice, Drives, Watched};
    use crate::model::Rates;

    fn trace(surprise: Vec<f64>, accuracy: Vec<f64>, change: usize) -> Trace {
        let summary = Metrics::default().summary();
        Trace {
            surprise,
            accuracy,
            change,
            summary,
        }
    }

    #[test]
    fn structure_folds_food_into_open_floor() {
        assert_eq!(structure(Cell::Food), Cell::Empty);
        assert_eq!(structure(Cell::Hazard), Cell::Hazard);
    }

    #[test]
    fn recovery_waits_for_a_full_calm_trail_and_the_map() {
        // 10 calm steps, then a spike of 5, then calm; map dips then returns.
        let mut s = vec![0.0; 10];
        s.extend([1.0; 5]);
        s.extend([0.0; 20]);
        let mut a = vec![0.9; 10];
        a.extend([0.5; 8]);
        a.extend([0.9; 17]);
        let r = recovery(&trace(s, a, 10), 10, 5);
        assert_eq!(r.map, Some(9));
        assert_eq!(r.surprise, Some(10)); // steps 15..19 are the first calm trail
        assert_eq!(r.both, Some(10));
    }

    #[test]
    fn never_recovering_is_none() {
        let s = [vec![0.0; 10], vec![1.0; 10]].concat();
        let a = [vec![0.9; 10], vec![0.1; 10]].concat();
        let r = recovery(&trace(s, a, 10), 10, 5);
        assert_eq!((r.surprise, r.map, r.both), (None, None, None));
    }

    #[test]
    fn short_lives_adapt_and_differ_by_mind() {
        let setup = Setup {
            warmup: 400,
            after: 400,
            floor_window: 200,
            ..Setup::default()
        };
        let mut reflex = Watched::new(ReflexAgent::new(1), Rates::default());
        let mut curious = Apprentice::new(Drives::default(), Rates::default(), 1);
        let a = run(&setup, &mut reflex).expect("room");
        let b = run(&setup, &mut curious).expect("room");
        assert_eq!(a.surprise.len(), 800);
        assert!(
            b.accuracy[399] > 0.5,
            "curious apprentice barely mapped: {}",
            b.accuracy[399]
        );
        assert_ne!(a.accuracy, b.accuracy);
    }
}
