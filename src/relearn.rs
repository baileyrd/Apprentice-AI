//! Phase 4 experiment: learn world A, then world B, then A again.
//!
//! One mind lives `phase` steps in world A, then `phase` steps in world B (a
//! different layout), then `phase` more in A, which it finds as it left it.
//! It is never told the world changed. The question is whether learning B
//! wiped out A (catastrophic forgetting) or whether A comes back fast.
//!
//! Rule, fixed before running: the time to learn a phase's world is the steps
//! from the phase's start until the believed map's structural accuracy (see
//! [`crate::adaptation::accuracy`]) reaches 95% of what it was at the end of
//! the first A phase. Savings is the second A time over the first.

use crate::adaptation::accuracy;
use crate::apprentice::Mind;
use crate::metrics::{Metrics, Summary};
use crate::sim::{Config, Sim, SimError};

/// Share of the first phase's final accuracy that counts as learned.
pub const TARGET: f64 = 0.95;

/// What to run.
#[derive(Debug, Clone)]
pub struct Setup {
    pub config: Config,
    /// World A's seed; world B's is `seed + 1000`.
    pub seed: u64,
    /// Steps in each of the three phases.
    pub phase: usize,
}

/// Step-by-step record of one life.
#[derive(Debug, Clone, PartialEq)]
pub struct Trace {
    pub surprise: Vec<f64>,
    /// Structural accuracy against the world it is in, after each step.
    pub map: Vec<f64>,
    pub phase: usize,
    pub summary: Summary,
}

/// How long each phase took to learn, in steps from its start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Learning {
    pub target: f64,
    pub first_a: Option<usize>,
    pub b: Option<usize>,
    pub second_a: Option<usize>,
    /// Accuracy after the first step back in A.
    pub on_return: f64,
}

impl Learning {
    /// Second A time over first A time; `None` unless both were learned.
    pub fn savings(&self) -> Option<f64> {
        Some(self.second_a? as f64 / self.first_a?.max(1) as f64)
    }
}

/// Lives A, then B, then A.
pub fn run(setup: &Setup, mind: &mut impl Mind) -> Result<Trace, SimError> {
    let mut a = Sim::new(setup.config.clone(), setup.seed)?;
    let mut b = Sim::new(setup.config.clone(), setup.seed + 1000)?;
    let mut metrics = Metrics::default();
    let total = 3 * setup.phase;
    let mut surprise = Vec::with_capacity(total);
    let mut map = Vec::with_capacity(total);
    for t in 0..total {
        let sim = if (setup.phase..2 * setup.phase).contains(&t) {
            &mut b
        } else {
            &mut a
        };
        let action = mind.act(&sim.observe());
        let outcome = sim.step(action);
        mind.feel(&outcome);
        metrics.record(&outcome);
        surprise.push(mind.surprise());
        map.push(accuracy(sim, mind));
    }
    Ok(Trace {
        surprise,
        map,
        phase: setup.phase,
        summary: metrics.summary(),
    })
}

/// Applies the learning rule to a trace.
pub fn learning(trace: &Trace) -> Learning {
    let p = trace.phase;
    let target = TARGET * trace.map[p - 1];
    let reach = |start: usize| {
        trace.map[start..start + p]
            .iter()
            .position(|&v| v >= target)
            .map(|i| i + 1)
    };
    Learning {
        target,
        first_a: reach(0),
        b: reach(p),
        second_a: reach(2 * p),
        on_return: trace.map[2 * p],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apprentice::{Apprentice, Drives};
    use crate::memory::{Memory, Recall};
    use crate::model::Rates;

    fn trace(map: Vec<f64>, phase: usize) -> Trace {
        Trace {
            surprise: vec![0.0; map.len()],
            map,
            phase,
            summary: Metrics::default().summary(),
        }
    }

    #[test]
    fn learning_times_count_from_each_phase_start() {
        let map = [
            vec![0.0, 0.5, 1.0, 1.0],
            vec![0.0, 0.0, 0.0, 0.96],
            vec![0.95, 1.0, 1.0, 1.0],
        ]
        .concat();
        let l = learning(&trace(map, 4));
        assert_eq!(l.target, 0.95);
        assert_eq!((l.first_a, l.b, l.second_a), (Some(3), Some(4), Some(1)));
        assert_eq!(l.on_return, 0.95);
        assert_eq!(l.savings(), Some(1.0 / 3.0));
    }

    #[test]
    fn a_world_never_relearned_has_no_savings() {
        let map = [vec![1.0; 3], vec![1.0; 3], vec![0.5; 3]].concat();
        let l = learning(&trace(map, 3));
        assert_eq!((l.second_a, l.savings()), (None, None));
    }

    #[test]
    fn memory_brings_world_a_back_faster_than_relearning_it() {
        let setup = Setup {
            config: Config::default(),
            seed: 3,
            phase: 1_500,
        };
        let plain = &mut Apprentice::new(Drives::default(), Rates::default(), 3);
        let memory = Memory::new(Rates::default(), Recall::default());
        let remembering = &mut Apprentice::with_model(Drives::default(), memory, 3);
        let p = learning(&run(&setup, plain).expect("room"));
        let m = learning(&run(&setup, remembering).expect("room"));
        assert_eq!(remembering.model().contexts(), 2);
        let (Some(fast), Some(slow)) = (m.second_a, p.second_a) else {
            panic!("A was not relearned: {m:?} / {p:?}");
        };
        assert!(fast < slow, "memory {fast} vs plain {slow}");
    }
}
