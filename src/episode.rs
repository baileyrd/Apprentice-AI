//! The one step loop: the world as a stream of events to an agent.
//!
//! Each step, in this order (docs/port):
//! 1. `watch.before`: the experimenter looks before anything is said or sensed;
//! 2. each speaker's `say`, heard by the agent;
//! 3. the agent senses and acts;
//! 4. the world steps;
//! 5. the agent feels the outcome;
//! 6. each speaker's `foretell`, heard by the agent;
//! 7. `watch.after`.
//!
//! The runner talks to a [`Port`], which may fail (an agent in another
//! process). In-process [`Agent`]s are ports that cannot fail.

use std::convert::Infallible;
use std::fmt;

use crate::agent::Agent;
use crate::body::{Action, Observation, Said};
use crate::sim::{Outcome, Sim};
use crate::world::Pos;

/// What the runner talks to. `Error` is `Infallible` for in-process agents.
pub trait Port {
    type Error: std::error::Error;
    /// Chooses the next action from what it senses.
    fn act(&mut self, obs: &Observation) -> Result<Action, Self::Error>;
    /// Hears what its last action did.
    fn feel(&mut self, outcome: &Outcome) -> Result<(), Self::Error>;
    /// Hears `said` about the square at `at`, relative to its wake square.
    fn hear(&mut self, at: Pos, said: Said) -> Result<(), Self::Error>;
    /// Its own name for the world it thinks it is in, if it has one.
    fn place(&self) -> Option<usize>;
}

impl<A: Agent> Port for A {
    type Error = Infallible;

    fn act(&mut self, obs: &Observation) -> Result<Action, Infallible> {
        Ok(Agent::act(self, obs))
    }

    fn feel(&mut self, outcome: &Outcome) -> Result<(), Infallible> {
        Agent::feel(self, outcome);
        Ok(())
    }

    fn hear(&mut self, at: Pos, said: Said) -> Result<(), Infallible> {
        Agent::hear(self, at, said);
        Ok(())
    }

    fn place(&self) -> Option<usize> {
        Agent::place(self)
    }
}

/// Something in the world that speaks to the agent: a mentor.
pub trait Speaker {
    /// What it says before the agent senses this step.
    fn say(&mut self, sim: &Sim) -> Option<(Pos, Said)>;
    /// What it says after the step's outcome.
    fn foretell(&mut self, sim: &Sim, outcome: &Outcome) -> Option<(Pos, Said)>;
}

/// One step, as the experimenter sees it. The agent never sees `pos`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    /// Where the agent stood when it acted.
    pub pos: Pos,
    pub obs: Observation,
    pub action: Action,
    pub outcome: Outcome,
}

/// The experimenter's two looks per step. Both do nothing by default.
pub trait Watch<P: ?Sized> {
    /// Before anything is said or sensed this step: the agent as it was
    /// after the last step.
    fn before(&mut self, _agent: &P, _sim: &Sim) {}
    /// After the agent felt the step and heard any foretelling.
    fn after(&mut self, _agent: &P, _sim: &Sim, _step: &Step) {}
}

/// Watches nothing.
impl<P: ?Sized> Watch<P> for () {}

/// Looks only after each step, with `F`.
pub struct After<F>(pub F);

impl<P: ?Sized, F: FnMut(&P, &Sim, &Step)> Watch<P> for After<F> {
    fn after(&mut self, agent: &P, sim: &Sim, step: &Step) {
        (self.0)(agent, sim, step);
    }
}

/// Which call to the agent failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Hear,
    Act,
    Feel,
}

/// The agent failed: the first error ends the life.
#[derive(Debug)]
pub struct RunError<E> {
    /// The world's step count when it failed. A failed `Act` leaves the
    /// world unmoved.
    pub t: u64,
    pub during: Phase,
    pub cause: E,
}

impl<E: fmt::Display> fmt::Display for RunError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} failed at step {}: {}",
            self.during, self.t, self.cause
        )
    }
}

impl<E: std::error::Error + 'static> std::error::Error for RunError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

/// Lives `steps` steps of `agent` in `sim`, in the order in the module docs.
pub fn run<P: Port>(
    sim: &mut Sim,
    agent: &mut P,
    speakers: &mut [&mut dyn Speaker],
    steps: usize,
    watch: &mut impl Watch<P>,
) -> Result<(), RunError<P::Error>> {
    for _ in 0..steps {
        step(sim, agent, speakers, watch)?;
    }
    Ok(())
}

/// [`run`] for an in-process agent, which cannot fail.
pub fn run_agent<A: Agent>(
    sim: &mut Sim,
    agent: &mut A,
    speakers: &mut [&mut dyn Speaker],
    steps: usize,
    watch: &mut impl Watch<A>,
) {
    if let Err(e) = run(sim, agent, speakers, steps, watch) {
        match e.cause {}
    }
}

fn step<P: Port>(
    sim: &mut Sim,
    agent: &mut P,
    speakers: &mut [&mut dyn Speaker],
    watch: &mut impl Watch<P>,
) -> Result<(), RunError<P::Error>> {
    let fail = |during: Phase, t: u64| move |cause| RunError { t, during, cause };
    watch.before(agent, sim);
    for speaker in speakers.iter_mut() {
        if let Some((at, said)) = speaker.say(sim) {
            agent.hear(at, said).map_err(fail(Phase::Hear, sim.t()))?;
        }
    }
    let (pos, obs) = (sim.pos(), sim.observe());
    let action = agent.act(&obs).map_err(fail(Phase::Act, sim.t()))?;
    let outcome = sim.step(action);
    agent.feel(&outcome).map_err(fail(Phase::Feel, sim.t()))?;
    for speaker in speakers.iter_mut() {
        if let Some((at, said)) = speaker.foretell(sim, &outcome) {
            agent.hear(at, said).map_err(fail(Phase::Hear, sim.t()))?;
        }
    }
    let step = Step {
        pos,
        obs,
        action,
        outcome,
    };
    watch.after(agent, sim, &step);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::ReflexAgent;
    use crate::body::{Voice, Word};
    use crate::sim::Config;

    fn sim() -> Sim {
        Sim::new(Config::default(), 7).expect("default layout has room")
    }

    /// The runner gives the same stream as the hand-written loop.
    #[test]
    fn same_as_the_hand_loop() {
        let (mut by_hand, mut a) = (sim(), ReflexAgent::new(3));
        let mut want = Vec::new();
        for _ in 0..300 {
            let outcome = by_hand.step(Agent::act(&mut a, &by_hand.observe()));
            Agent::feel(&mut a, &outcome);
            want.push(outcome);
        }
        let (mut run_sim, mut b) = (sim(), ReflexAgent::new(3));
        let mut got = Vec::new();
        let mut watch = After(|_: &ReflexAgent, _: &Sim, s: &Step| got.push(s.outcome));
        run_agent(&mut run_sim, &mut b, &mut [], 300, &mut watch);
        assert_eq!(got, want);
        assert_eq!(run_sim, by_hand);
    }

    /// Records the order of every event it is part of.
    #[derive(Default)]
    struct Log(Vec<&'static str>);

    impl Agent for Log {
        fn act(&mut self, _obs: &Observation) -> Action {
            self.0.push("act");
            Action::Rest
        }
        fn feel(&mut self, _outcome: &Outcome) {
            self.0.push("feel");
        }
        fn hear(&mut self, _at: Pos, said: Said) {
            self.0.push(if said.word == Word(0) {
                "say"
            } else {
                "foretell"
            });
        }
    }

    struct Chatty;

    impl Speaker for Chatty {
        fn say(&mut self, _sim: &Sim) -> Option<(Pos, Said)> {
            Some((Pos { x: 0, y: 0 }, said(0)))
        }
        fn foretell(&mut self, _sim: &Sim, _outcome: &Outcome) -> Option<(Pos, Said)> {
            Some((Pos { x: 0, y: 0 }, said(1)))
        }
    }

    fn said(word: u8) -> Said {
        Said {
            voice: Voice(0),
            word: Word(word),
        }
    }

    struct Looks(Vec<&'static str>);

    impl Watch<Log> for Looks {
        fn before(&mut self, agent: &Log, _sim: &Sim) {
            self.0.push(if agent.0.is_empty() {
                "first"
            } else {
                "before"
            });
        }
        fn after(&mut self, agent: &Log, _sim: &Sim, _step: &Step) {
            assert_eq!(agent.0.last(), Some(&"foretell"));
            self.0.push("after");
        }
    }

    #[test]
    fn events_come_in_the_fixed_order() {
        let (mut world, mut agent) = (sim(), Log::default());
        let mut looks = Looks(Vec::new());
        run_agent(&mut world, &mut agent, &mut [&mut Chatty], 2, &mut looks);
        let step = ["say", "act", "feel", "foretell"];
        assert_eq!(agent.0, [step, step].concat());
        assert_eq!(looks.0, ["first", "after", "before", "after"]);
    }

    #[derive(Debug)]
    struct Broke;

    impl fmt::Display for Broke {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "broke")
        }
    }

    impl std::error::Error for Broke {}

    /// A port that fails at one phase on one step.
    struct Fails {
        at: u64,
        during: Phase,
        steps: u64,
        felt: u64,
    }

    impl Fails {
        fn check(&self, phase: Phase) -> Result<(), Broke> {
            if self.steps == self.at && self.during == phase {
                return Err(Broke);
            }
            Ok(())
        }
    }

    impl Port for Fails {
        type Error = Broke;
        fn act(&mut self, _obs: &Observation) -> Result<Action, Broke> {
            self.check(Phase::Act)?;
            Ok(Action::Rest)
        }
        fn feel(&mut self, _outcome: &Outcome) -> Result<(), Broke> {
            self.check(Phase::Feel)?;
            self.felt += 1;
            self.steps += 1;
            Ok(())
        }
        fn hear(&mut self, _at: Pos, _said: Said) -> Result<(), Broke> {
            self.check(Phase::Hear)
        }
        fn place(&self) -> Option<usize> {
            None
        }
    }

    fn fails(during: Phase) -> Fails {
        Fails {
            at: 3,
            during,
            steps: 0,
            felt: 0,
        }
    }

    #[test]
    fn a_failed_act_leaves_the_world_unmoved() {
        let (mut world, mut agent) = (sim(), fails(Phase::Act));
        let e = run(&mut world, &mut agent, &mut [], 10, &mut ()).expect_err("fails");
        assert_eq!((e.t, e.during), (3, Phase::Act));
        assert_eq!(world.t(), 3);
        assert_eq!(agent.felt, 3);
    }

    #[test]
    fn a_failed_feel_stops_after_the_step() {
        let (mut world, mut agent) = (sim(), fails(Phase::Feel));
        let mut seen = 0;
        let mut watch = After(|_: &Fails, _: &Sim, _: &Step| seen += 1);
        let e = run(&mut world, &mut agent, &mut [], 10, &mut watch).expect_err("fails");
        assert_eq!((e.t, e.during), (4, Phase::Feel));
        assert_eq!(world.t(), 4);
        assert_eq!(seen, 3, "the failed step is not watched");
    }

    #[test]
    fn a_failed_hear_stops_the_life() {
        let (mut world, mut agent) = (sim(), fails(Phase::Hear));
        let e = run(&mut world, &mut agent, &mut [&mut Chatty], 10, &mut ()).expect_err("fails");
        assert_eq!((e.t, e.during), (3, Phase::Hear));
        assert_eq!(
            world.t(),
            3,
            "said before sensing, so the world never moved"
        );
    }
}
