//! Retention: does learning to walk erase turning?
//!
//! One facing learner lives in the walking world with the heading ring.
//! It first only turns in the centre (phase A), then either walks
//! (phase B) or keeps turning (control C). A frozen turning test, the same
//! episode every time, scores it after A, B and C, and the weight distance
//! each phase moved is measured. Marks are in `docs/retention/README.md`.

use crate::facing::{Driver, FEATURES, Learner, Memory, SIDES, Stage, World, nearest};
use crate::network::NetError;

/// One run's settings, shared by every seed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Retention {
    /// Turning-only steps before walking.
    pub turning: usize,
    /// Walking steps (phase B), and more turning in the control (C).
    pub walking: usize,
    /// Steps in the frozen turning test.
    pub test: usize,
    pub rate: f64,
}

/// One seed's results. Test scores are shares of turns predicted right.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Kept {
    /// Turning test after phase A.
    pub before: f64,
    /// Share of the after-walking test's misses that guessed a view the
    /// centre never shows (a side box's back wall or the dark): the
    /// learner acting as if it stood somewhere else.
    pub misplaced: f64,
    /// Turning test after walking (B).
    pub after_walking: f64,
    /// Turning test after more turning (C).
    pub after_turning: f64,
    /// Walking one-ahead over the last `test` motor firings of B that
    /// were not stills.
    pub walking_one: f64,
    /// Weight distance learning turning moved it from the start.
    pub moved_learning: f64,
    /// Weight distance walking moved it from the end of A.
    pub moved_walking: f64,
    /// Weight distance more turning moved it from the end of A.
    pub moved_turning: f64,
}

/// Lives `r` on `seed`.
pub fn retain(r: &Retention, seed: u64) -> Result<Kept, NetError> {
    let mut world = World::new(Stage::Walking, seed);
    let mut learner = Learner::new(
        Memory::Context,
        FEATURES + SIDES,
        world.motors(),
        r.rate,
        seed ^ 0x5eed,
    )?;
    let start = learner.params();
    let mut turner = Driver::turning(&world, seed ^ 0xd21e);
    train(&mut learner, &mut world, &mut turner, r.turning);
    let after_a = learner.clone();
    let test_world = world.clone();
    let test = |l: &Learner| turning_test(l, &test_world, seed ^ 0x7e57, r.test).right;

    let mut walker = learner.clone();
    let mut walk_world = world.clone();
    let mut driver = Driver::new(&walk_world, seed ^ 0xa1c);
    let hits = train(&mut walker, &mut walk_world, &mut driver, r.walking);

    let mut more = learner;
    train(&mut more, &mut world, &mut turner, r.walking);

    let a = after_a.params();
    let walked = turning_test(&walker, &test_world, seed ^ 0x7e57, r.test);
    Ok(Kept {
        before: test(&after_a),
        misplaced: walked.misplaced,
        after_walking: walked.right,
        after_turning: test(&more),
        walking_one: rate(&hits[hits.len().saturating_sub(r.test)..]),
        moved_learning: distance(&start, &a),
        moved_walking: distance(&a, &walker.params()),
        moved_turning: distance(&a, &more.params()),
    })
}

/// Learns online for `steps` steps; returns, for each motor firing that
/// was not a still, whether its next view was predicted right.
fn train(learner: &mut Learner, world: &mut World, driver: &mut Driver, steps: usize) -> Vec<bool> {
    let mut hits = Vec::new();
    let mut sense = world.percept(true);
    for _ in 0..steps {
        let (motor, stared) = driver.drive();
        let [guess, _] = learner.imagine(&sense, motor, motor);
        world.fire(motor);
        let next = world.percept(true);
        if stared.is_some() {
            hits.push(nearest(world.looks(), &guess) == world.faced());
        }
        learner.learn(&sense, motor, &next);
        sense = next;
    }
    hits
}

/// A turning test's result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Test {
    /// Share of turns whose next view was predicted right.
    pub right: f64,
    /// Share of misses that guessed a view the test never showed.
    pub misplaced: f64,
}

/// How a frozen copy of `learner` predicts the view after each turn, on a
/// turning-only episode of `steps` steps from `world`. The same `world`
/// and `seed` give the same episode every time.
pub fn turning_test(learner: &Learner, world: &World, seed: u64, steps: usize) -> Test {
    let mut learner = learner.clone();
    learner.rest();
    let mut world = world.clone();
    let mut driver = Driver::turning(&world, seed);
    let mut turns = Vec::new();
    let mut sense = world.percept(true);
    for _ in 0..steps {
        let (motor, stared) = driver.drive();
        let [guess, _] = learner.imagine(&sense, motor, motor);
        world.fire(motor);
        let next = world.percept(true);
        if stared.is_some() {
            turns.push((nearest(world.looks(), &guess), world.faced()));
        }
        learner.observe(&sense, motor);
        sense = next;
    }
    let shown: Vec<usize> = turns.iter().map(|&(_, truth)| truth).collect();
    let misses: Vec<bool> = turns
        .iter()
        .filter(|(guess, truth)| guess != truth)
        .map(|(guess, _)| !shown.contains(guess))
        .collect();
    Test {
        right: rate(&turns.iter().map(|(g, t)| g == t).collect::<Vec<_>>()),
        misplaced: rate(&misses),
    }
}

fn rate(hits: &[bool]) -> f64 {
    if hits.is_empty() {
        return 0.0;
    }
    hits.iter().filter(|&&h| h).count() as f64 / hits.len() as f64
}

/// Euclidean distance between two weight vectors of the same net.
fn distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn learner(world: &World) -> Learner {
        Learner::new(Memory::Context, FEATURES + SIDES, world.motors(), 0.05, 9)
            .expect("valid shape")
    }

    #[test]
    fn turning_driver_never_steps_forward() {
        let mut world = World::new(Stage::Walking, 3);
        let mut driver = Driver::turning(&world, 4);
        for _ in 0..2_000 {
            world.fire(driver.drive().0);
            assert_eq!(world.at(), None);
        }
    }

    #[test]
    fn turning_test_is_repeatable_and_learns_nothing() {
        let world = World::new(Stage::Walking, 5);
        let l = learner(&world);
        let before = l.params();
        let first = turning_test(&l, &world, 6, 500);
        assert_eq!(first, turning_test(&l, &world, 6, 500));
        assert_eq!(l.params(), before);
    }

    #[test]
    fn learning_turning_beats_a_newborn_on_the_test() {
        let mut world = World::new(Stage::Walking, 7);
        let newborn = learner(&world);
        let mut l = newborn.clone();
        let mut driver = Driver::turning(&world, 8);
        let test_world = world.clone();
        train(&mut l, &mut world, &mut driver, 8_000);
        let score = |l: &Learner| turning_test(l, &test_world, 10, 1_000).right;
        assert!(score(&l) > score(&newborn) + 0.3);
    }

    #[test]
    fn distance_is_euclidean() {
        assert!((distance(&[0.0, 0.0], &[3.0, 4.0]) - 5.0).abs() < 1e-12);
    }
}
