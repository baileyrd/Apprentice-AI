//! Phase 1 experiment: does surprise fall as the apprentice lives in one world?
//!
//! Fixed map, fixed seed. The reflex baseline drives the body; the world model
//! only watches and learns, so any drop in surprise is the model's alone.

use crate::agent::{Agent, ReflexAgent};
use crate::model::WorldModel;
use crate::sim::{Config, Sim, SimError};
use crate::world::{Cell, Grid};

/// Episodes averaged at each end of the fallback comparison.
pub const WINDOW: usize = 10;
/// Required fall in mean surprise, as a fraction of the opening average.
pub const REQUIRED_DROP: f64 = 0.5;

/// What to run.
#[derive(Debug, Clone)]
pub struct Setup {
    pub config: Config,
    pub seed: u64,
    pub episodes: usize,
    pub steps_per_episode: u64,
    /// Episodes (1-based) after which to snapshot the believed map.
    pub snapshot_at: Vec<usize>,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            config: Config::default(),
            seed: 1,
            episodes: 200,
            steps_per_episode: 100,
            snapshot_at: vec![1, 10, 50, 200],
        }
    }
}

/// The believed map beside the real one, after some episode.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub episode: usize,
    pub real: Grid,
    /// Row-major over `real`; `None` where the square was never seen.
    pub believed: Vec<Option<Cell>>,
}

impl Snapshot {
    /// Seen squares whose most likely kind matches reality, over seen squares.
    pub fn accuracy(&self) -> Option<f64> {
        let real = self.real.cells();
        let seen: Vec<(Cell, Cell)> = self
            .believed
            .iter()
            .zip(real)
            .filter_map(|(b, &r)| b.map(|b| (b, r)))
            .collect();
        let right = seen.iter().filter(|(b, r)| b == r).count();
        (!seen.is_empty()).then(|| right as f64 / seen.len() as f64)
    }

    /// Fraction of real squares ever seen.
    pub fn coverage(&self) -> f64 {
        let seen = self.believed.iter().filter(|b| b.is_some()).count();
        seen as f64 / self.believed.len() as f64
    }
}

/// Everything the experiment measured.
#[derive(Debug, Clone, PartialEq)]
pub struct Results {
    /// Mean surprise per step, one entry per episode.
    pub surprise: Vec<f64>,
    pub snapshots: Vec<Snapshot>,
}

/// The pre-registered fallback test.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Verdict {
    /// Mean surprise over episodes 1..=10.
    pub opening: f64,
    /// Mean surprise over the ten episodes ending at the deadline.
    pub closing: f64,
    /// `1 - closing / opening`.
    pub drop: f64,
    /// True when `drop < REQUIRED_DROP`: run the backprop baseline.
    pub fallback: bool,
}

/// Judges `surprise` at episode `deadline` (1-based). `None` if too short.
pub fn verdict(surprise: &[f64], deadline: usize) -> Option<Verdict> {
    if deadline < WINDOW || surprise.len() < deadline {
        return None;
    }
    let mean = |xs: &[f64]| xs.iter().sum::<f64>() / xs.len() as f64;
    let opening = mean(&surprise[..WINDOW]);
    let closing = mean(&surprise[deadline - WINDOW..deadline]);
    let drop = if opening > 0.0 {
        1.0 - closing / opening
    } else {
        0.0
    };
    Some(Verdict {
        opening,
        closing,
        drop,
        fallback: drop < REQUIRED_DROP,
    })
}

/// Lives `setup.episodes` episodes in one world while `model` learns.
pub fn run(setup: &Setup, mut model: impl WorldModel) -> Result<Results, SimError> {
    let mut sim = Sim::new(setup.config.clone(), setup.seed)?;
    let mut agent = ReflexAgent::new(setup.seed);
    model.observe(&sim.observe());

    let mut surprise = Vec::with_capacity(setup.episodes);
    let mut snapshots = Vec::new();
    for episode in 1..=setup.episodes {
        let mut total = 0.0;
        for _ in 0..setup.steps_per_episode {
            let outcome = sim.step(agent.act(&sim.observe()));
            agent.feel(&outcome);
            total += model.observe(&sim.observe());
        }
        surprise.push(total / setup.steps_per_episode as f64);
        if setup.snapshot_at.contains(&episode) {
            snapshots.push(snapshot(episode, &sim, &model));
        }
    }
    Ok(Results {
        surprise,
        snapshots,
    })
}

fn snapshot(episode: usize, sim: &Sim, model: &impl WorldModel) -> Snapshot {
    let real = sim.grid().clone();
    let start = sim.start();
    let believed = real
        .positions()
        .map(|p| model.believes(p.shift(-start.x, -start.y)))
        .collect();
    Snapshot {
        episode,
        real,
        believed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Backprop, PredictiveCoding, Rates};

    fn pc() -> PredictiveCoding {
        PredictiveCoding::new(Rates::default())
    }

    #[test]
    fn verdict_needs_enough_episodes() {
        assert_eq!(verdict(&[1.0; 5], 200), None);
        assert_eq!(verdict(&[1.0; 200], 5), None);
    }

    #[test]
    fn halving_passes_and_less_triggers_fallback() {
        let mut s = vec![1.0; 200];
        s[190..].fill(0.5);
        let v = verdict(&s, 200).expect("long enough");
        assert!((v.drop - 0.5).abs() < 1e-12);
        assert!(!v.fallback);

        s[190..].fill(0.6);
        assert!(verdict(&s, 200).expect("long enough").fallback);
    }

    #[test]
    fn short_run_learns_and_snapshots() {
        let setup = Setup {
            episodes: 20,
            snapshot_at: vec![1, 20],
            ..Setup::default()
        };
        let r = run(&setup, pc()).expect("default layout has room");
        assert_eq!(r.surprise.len(), 20);
        assert_eq!(r.snapshots.len(), 2);
        let late = r.snapshots[1].accuracy().expect("saw something");
        assert!(late > 0.8, "believed map mostly wrong: {late}");
        assert!(r.snapshots[1].coverage() >= r.snapshots[0].coverage());
    }

    #[test]
    fn backprop_baseline_runs_the_same_world() {
        let setup = Setup {
            episodes: 20,
            snapshot_at: vec![20],
            ..Setup::default()
        };
        let r = run(&setup, Backprop::new(Rates::default())).expect("room");
        let late = r.snapshots[0].accuracy().expect("saw something");
        assert!(late > 0.8, "backprop believed map mostly wrong: {late}");
    }

    #[test]
    fn run_is_deterministic() {
        let setup = Setup {
            episodes: 5,
            ..Setup::default()
        };
        assert_eq!(run(&setup, pc()), run(&setup, pc()));
    }
}
