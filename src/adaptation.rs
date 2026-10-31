//! Adaptation experiments (phases 2 and 3): after the world changes, how long
//! until the mind adapts?
//!
//! A mind lives `warmup` steps in one world, then the world changes (half the
//! walls move, or all the food moves) and it lives `after` more. Adaptation
//! time is the number of steps after the change until both hold at once:
//! - **surprise is back at its floor**: the mean over the last `trail` steps
//!   (all after the change) is at most twice the mean over the `floor_window`
//!   steps before it (and never asked below 0.001);
//! - **the believed map is right again**:
//!   - walls: the share of squares whose believed structure (wall, hazard or
//!     open) matches reality is back to where it was just before the move;
//!   - food: every square it believed held food at the move has been checked,
//!     i.e. it no longer believes food is there (unless food regrew there).
//!
//! The food measure was first "no phantom food believed and at least one real
//! food square known". That held only 0-25% of steps even before the move,
//! because food regrows unseen after every meal, so it could not show
//! adaptation; it was replaced before any result was reported.

use crate::apprentice::Mind;
use crate::metrics::{Metrics, Summary};
use crate::sim::{Config, Event, Regrowth, Sim, SimError};
use crate::world::{Cell, Pos};

/// How the world changes after the warm-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// `wall_fraction` of the walls move to random empty squares.
    Walls,
    /// Every food square moves to a new random empty square.
    Food,
}

/// What to run.
#[derive(Debug, Clone)]
pub struct Setup {
    pub config: Config,
    pub seed: u64,
    pub change: Change,
    pub warmup: usize,
    pub after: usize,
    /// Share of walls that move.
    pub wall_fraction: f64,
    pub floor_window: usize,
    pub trail: usize,
}

/// Steps a food bush takes to grow back when food has places.
pub const BUSH_DELAY: u64 = 30;

impl Setup {
    /// The default setup for `change`. Moving food only means something when
    /// food has places, so the food test grows food back in place.
    pub fn for_change(change: Change) -> Self {
        let mut setup = Self {
            change,
            ..Self::default()
        };
        if change == Change::Food {
            setup.config.regrowth = Regrowth::InPlace { delay: BUSH_DELAY };
        }
        setup
    }
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            config: Config::default(),
            seed: 1,
            change: Change::Walls,
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
    pub change: Change,
    /// Surprise per step.
    pub surprise: Vec<f64>,
    /// The change's map measure per step, after that step: structural
    /// accuracy for walls, share of stale food beliefs cleared for food.
    pub map: Vec<f64>,
    /// Index of the first step after the change.
    pub changed_at: usize,
    /// Steps after the change until its first meal.
    pub first_meal: Option<usize>,
    /// Food squares it believed in when the food moved (0 for walls).
    pub stale: usize,
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

/// What the mind believes is at grid square `p`.
fn belief(sim: &Sim, mind: &impl Mind, p: Pos) -> Option<Cell> {
    let start = sim.start();
    mind.believes(Pos {
        x: p.x - start.x,
        y: p.y - start.y,
    })
}

/// Share of squares whose believed structure matches reality.
pub fn accuracy(sim: &Sim, mind: &impl Mind) -> f64 {
    let grid = sim.grid();
    let right = grid
        .positions()
        .filter(|&p| belief(sim, mind, p).map(structure) == Some(structure(grid.get(p))))
        .count();
    right as f64 / grid.cells().len() as f64
}

/// Food squares the mind believes in: its beliefs that go stale if food moves.
pub fn believed_food(sim: &Sim, mind: &impl Mind) -> Vec<Pos> {
    sim.grid()
        .positions_of(Cell::Food)
        .into_iter()
        .filter(|&p| belief(sim, mind, p) == Some(Cell::Food))
        .collect()
}

/// Share of `stale` squares no longer wrongly believed to hold food; 1 if none.
pub fn cleared(sim: &Sim, mind: &impl Mind, stale: &[Pos]) -> f64 {
    if stale.is_empty() {
        return 1.0;
    }
    let ok = stale
        .iter()
        .filter(|&&p| sim.grid().get(p) == Cell::Food || belief(sim, mind, p) != Some(Cell::Food))
        .count();
    ok as f64 / stale.len() as f64
}

/// Lives one life with a change in the middle.
pub fn run(setup: &Setup, mind: &mut impl Mind) -> Result<Trace, SimError> {
    let mut sim = Sim::new(setup.config.clone(), setup.seed)?;
    let walls = sim.grid().positions_of(Cell::Wall).len();
    let to_move = (walls as f64 * setup.wall_fraction).round() as usize;
    let mut metrics = Metrics::default();
    let total = setup.warmup + setup.after;
    let mut surprise = Vec::with_capacity(total);
    let mut map = Vec::with_capacity(total);
    let mut first_meal = None;
    let mut stale = Vec::new();
    for t in 0..total {
        if t == setup.warmup {
            match setup.change {
                Change::Walls => {
                    sim.move_walls(to_move);
                }
                Change::Food => {
                    stale = believed_food(&sim, mind);
                    sim.move_food();
                }
            }
        }
        let action = mind.act(&sim.observe());
        let outcome = sim.step(action);
        mind.feel(&outcome);
        metrics.record(&outcome);
        if t >= setup.warmup && first_meal.is_none() && outcome.event == Event::Ate {
            first_meal = Some(t + 1 - setup.warmup);
        }
        surprise.push(mind.surprise());
        map.push(match setup.change {
            Change::Walls => accuracy(&sim, mind),
            Change::Food => cleared(&sim, mind, &stale),
        });
    }
    Ok(Trace {
        change: setup.change,
        surprise,
        map,
        changed_at: setup.warmup,
        first_meal,
        stale: stale.len(),
        summary: metrics.summary(),
    })
}

/// Applies the adaptation rule to a trace.
pub fn recovery(trace: &Trace, floor_window: usize, trail: usize) -> Recovery {
    let c = trace.changed_at;
    let mean = |xs: &[f64]| xs.iter().sum::<f64>() / xs.len().max(1) as f64;
    let floor = mean(&trace.surprise[c.saturating_sub(floor_window)..c]);
    let limit = (2.0 * floor).max(1e-3);
    let baseline_accuracy = match trace.change {
        Change::Walls if c > 0 => trace.map[c - 1],
        Change::Walls => 0.0,
        Change::Food => 1.0,
    };

    let calm = |i: usize| i + 1 >= c + trail && mean(&trace.surprise[i + 1 - trail..=i]) <= limit;
    let mapped = |i: usize| trace.map[i] >= baseline_accuracy;
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

    fn trace(surprise: Vec<f64>, map: Vec<f64>, changed_at: usize) -> Trace {
        Trace {
            change: Change::Walls,
            surprise,
            map,
            changed_at,
            first_meal: None,
            stale: 0,
            summary: Metrics::default().summary(),
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
            b.map[399] > 0.5,
            "curious apprentice barely mapped: {}",
            b.map[399]
        );
        assert_ne!(a.map, b.map);
    }

    #[test]
    fn food_recovery_targets_a_fully_right_food_map() {
        let s = vec![0.0; 30];
        let m = [vec![1.0; 10], vec![0.0; 6], vec![1.0; 14]].concat();
        let t = Trace {
            change: Change::Food,
            ..trace(s, m, 10)
        };
        let r = recovery(&t, 10, 5);
        assert_eq!(r.baseline_accuracy, 1.0);
        assert_eq!(r.map, Some(7)); // wrong for 6 steps, right on the 7th
    }

    #[test]
    fn moving_food_finds_the_first_meal_and_checks_stale_beliefs() {
        let setup = Setup {
            warmup: 600,
            after: 1_500,
            floor_window: 300,
            seed: 5, // a world where it holds food beliefs when the food moves
            ..Setup::for_change(Change::Food)
        };
        // Without revisit: a patrolling apprentice sees the food go at once,
        // so this world holds no stale beliefs to measure.
        let drives = Drives {
            revisit: 0.0,
            ..Drives::default()
        };
        let mut curious = Apprentice::new(drives, Rates::default(), 5);
        let t = run(&setup, &mut curious).expect("room");
        assert!(t.first_meal.is_some(), "never ate after the food moved");
        assert!(t.stale > 0, "believed in no food before the move");
        assert!(t.map[setup.warmup] < 1.0, "stale beliefs cleared at once");
        assert_eq!(t.map.last(), Some(&1.0), "stale food beliefs never cleared");
    }
}
