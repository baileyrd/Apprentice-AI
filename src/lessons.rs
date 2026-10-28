//! Lessons: the mentor arranges the world so the next challenge is just
//! within reach.
//!
//! A course is a list of worlds, easiest first. The apprentice lives in each
//! one until it has gone `mastery` steps without running out of energy, or
//! until `cap` steps, then moves to the next. The mentor never touches the
//! apprentice's mind; it only picks which world it lives in.

use crate::apprentice::Mind;
use crate::metrics::{Metrics, Summary};
use crate::sim::{Config, Sim, SimError};

/// A sequence of worlds and when to move on.
#[derive(Debug, Clone, PartialEq)]
pub struct Course {
    pub lessons: Vec<Config>,
    /// Steps without sleep that count as mastering a lesson.
    pub mastery: u64,
    /// Most steps spent on one lesson.
    pub cap: u64,
}

/// How one lesson went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lesson {
    pub steps: u64,
    pub mastered: bool,
}

/// Lives `steps` steps in a world built from `config` and `seed`.
pub fn live(
    mind: &mut impl Mind,
    config: &Config,
    seed: u64,
    steps: u64,
) -> Result<Summary, SimError> {
    let mut sim = Sim::new(config.clone(), seed)?;
    let mut metrics = Metrics::default();
    for _ in 0..steps {
        let outcome = sim.step(mind.act(&sim.observe()));
        mind.feel(&outcome);
        metrics.record(&outcome);
    }
    Ok(metrics.summary())
}

/// Takes the course; lesson `i` is a world built from `seed + i`.
pub fn study(mind: &mut impl Mind, course: &Course, seed: u64) -> Result<Vec<Lesson>, SimError> {
    let mut lessons = Vec::with_capacity(course.lessons.len());
    for (i, config) in course.lessons.iter().enumerate() {
        let mut sim = Sim::new(config.clone(), seed + i as u64)?;
        let (mut steps, mut awake) = (0, 0);
        while steps < course.cap && awake < course.mastery {
            let outcome = sim.step(mind.act(&sim.observe()));
            mind.feel(&outcome);
            steps += 1;
            awake = if outcome.slept { 0 } else { awake + 1 };
        }
        lessons.push(Lesson {
            steps,
            mastered: awake >= course.mastery,
        });
    }
    Ok(lessons)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::ReflexAgent;
    use crate::apprentice::Watched;
    use crate::model::Rates;
    use crate::world::Layout;

    fn reflex() -> Watched<ReflexAgent> {
        Watched::new(ReflexAgent::new(0), Rates::default())
    }

    fn world(food_count: usize) -> Config {
        Config {
            layout: Layout {
                width: 6,
                height: 6,
                food_count,
                ..Layout::default()
            },
            ..Config::default()
        }
    }

    #[test]
    fn a_world_without_food_is_never_mastered() {
        let course = Course {
            lessons: vec![world(0)],
            mastery: 200,
            cap: 300,
        };
        let done = study(&mut reflex(), &course, 1).expect("room");
        assert_eq!(
            done,
            [Lesson {
                steps: 300,
                mastered: false
            }]
        );
    }

    #[test]
    fn a_mastered_lesson_ends_early_and_the_next_begins() {
        let course = Course {
            lessons: vec![world(0), world(0)],
            mastery: 10,
            cap: 300,
        };
        // Starting energy outlasts the first 10 steps, so each lesson is
        // mastered as soon as it can be.
        let done = study(&mut reflex(), &course, 1).expect("room");
        assert!(done.iter().all(|l| l.mastered && l.steps == 10), "{done:?}");
    }

    #[test]
    fn live_counts_what_happened() {
        let s = live(&mut reflex(), &world(3), 1, 500).expect("room");
        assert!(s.meals > 0 || s.sleeps > 0);
    }
}
