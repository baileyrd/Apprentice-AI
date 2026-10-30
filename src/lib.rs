//! Apprentice AI: a small learner that earns its knowledge in a grid world.
//!
//! Phase 0 is the world, the body, two baselines and the measurements; no
//! learning yet. Everything here is pure; I/O lives in the binary.

pub mod adaptation;
pub mod agent;
pub mod apprentice;
pub mod body;
pub mod experiment;
pub mod facing;
pub mod gru;
pub mod home;
pub mod judge;
pub mod lessons;
pub mod life;
pub mod memory;
pub mod mentor;
pub mod metrics;
pub mod model;
pub mod network;
pub mod patterns;
pub mod peer;
pub mod physics;
pub mod relearn;
pub mod report;
pub mod rng;
pub mod sim;
pub mod world;

use serde::{Deserialize, Serialize};

use agent::{Agent, Baseline};
use body::Action;
use metrics::Metrics;
use sim::{Event, Sim};
use world::Pos;

/// One line of the step log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepRecord {
    pub t: u64,
    pub pos: Pos,
    pub action: Action,
    pub event: Event,
    pub energy: u32,
    pub slept: bool,
}

/// Everything needed to resume a run exactly where it stopped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub sim: Sim,
    pub agent: Baseline,
    pub metrics: Metrics,
}

impl Checkpoint {
    /// Runs `steps` more steps, handing each record to `on_step`.
    pub fn run(&mut self, steps: u64, mut on_step: impl FnMut(&StepRecord)) {
        for _ in 0..steps {
            let action = self.agent.act(&self.sim.observe());
            let outcome = self.sim.step(action);
            self.agent.feel(&outcome);
            self.metrics.record(&outcome);
            on_step(&StepRecord {
                t: self.sim.t(),
                pos: self.sim.pos(),
                action,
                event: outcome.event,
                energy: outcome.energy,
                slept: outcome.slept,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent::{RandomAgent, ReflexAgent};
    use sim::Config;

    fn fresh(agent: Baseline) -> Checkpoint {
        let sim = Sim::new(Config::default(), 7).expect("default layout has room");
        Checkpoint {
            sim,
            agent,
            metrics: Metrics::default(),
        }
    }

    #[test]
    fn resume_from_checkpoint_replays_exactly() {
        let mut whole = fresh(Baseline::Random(RandomAgent::new(1)));
        let mut full_log = Vec::new();
        whole.run(400, |r| full_log.push(r.clone()));

        let mut first = fresh(Baseline::Random(RandomAgent::new(1)));
        let mut split_log = Vec::new();
        first.run(200, |r| split_log.push(r.clone()));
        let json = serde_json::to_string(&first).expect("serialise");
        let mut resumed: Checkpoint = serde_json::from_str(&json).expect("deserialise");
        resumed.run(200, |r| split_log.push(r.clone()));

        assert_eq!(full_log, split_log);
        assert_eq!(whole, resumed);
    }

    #[test]
    fn reflex_beats_random() {
        let mut random = fresh(Baseline::Random(RandomAgent::new(3)));
        let mut reflex = fresh(Baseline::Reflex(ReflexAgent::new(3)));
        random.run(5_000, |_| {});
        reflex.run(5_000, |_| {});
        let (r, x) = (random.metrics.summary(), reflex.metrics.summary());
        assert!(
            x.meals > r.meals,
            "reflex {} vs random {} meals",
            x.meals,
            r.meals
        );
        assert_eq!(x.hurts, 0);
    }
}
