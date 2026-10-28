//! Running behaviour metrics: the numbers every later phase is judged by.

use serde::{Deserialize, Serialize};

use crate::sim::{Event, Outcome};

/// Accumulates outcomes into a summary.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    steps: u64,
    meals: u64,
    sleeps: u64,
    bumps: u64,
    hurts: u64,
    energy_sum: u64,
    since_meal: u64,
    hunt_sum: u64,
}

/// A snapshot of the metrics, ready to print.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub steps: u64,
    pub meals: u64,
    pub sleeps: u64,
    pub bumps: u64,
    pub hurts: u64,
    pub mean_energy: f64,
    /// Mean steps from waking or the last meal to the next meal.
    pub mean_time_to_food: Option<f64>,
}

impl Metrics {
    /// Folds in one step's outcome.
    pub fn record(&mut self, outcome: &Outcome) {
        self.steps += 1;
        self.since_meal += 1;
        self.energy_sum += u64::from(outcome.energy);
        match outcome.event {
            Event::Ate => {
                self.meals += 1;
                self.hunt_sum += self.since_meal;
                self.since_meal = 0;
            }
            Event::Bumped => self.bumps += 1,
            Event::Hurt => self.hurts += 1,
            Event::Moved | Event::AteNothing | Event::Rested => {}
        }
        if outcome.slept {
            self.sleeps += 1;
            self.since_meal = 0;
        }
    }

    pub fn summary(&self) -> Summary {
        let mean = |sum: u64, n: u64| (n > 0).then(|| sum as f64 / n as f64);
        Summary {
            steps: self.steps,
            meals: self.meals,
            sleeps: self.sleeps,
            bumps: self.bumps,
            hurts: self.hurts,
            mean_energy: mean(self.energy_sum, self.steps).unwrap_or(0.0),
            mean_time_to_food: mean(self.hunt_sum, self.meals),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(event: Event, energy: u32, slept: bool) -> Outcome {
        Outcome {
            event,
            energy,
            slept,
        }
    }

    #[test]
    fn empty_summary_has_no_hunt_time() {
        let s = Metrics::default().summary();
        assert_eq!(
            (s.steps, s.mean_energy, s.mean_time_to_food),
            (0, 0.0, None)
        );
    }

    #[test]
    fn time_to_food_resets_on_meal_and_sleep() {
        let mut m = Metrics::default();
        for o in [
            out(Event::Moved, 10, false),
            out(Event::Ate, 30, false), // 2 steps
            out(Event::Moved, 2, false),
            out(Event::Rested, 50, true), // slept: clock resets, no meal
            out(Event::Moved, 49, false),
            out(Event::Moved, 48, false),
            out(Event::Moved, 47, false),
            out(Event::Ate, 70, false), // 4 steps
        ] {
            m.record(&o);
        }
        let s = m.summary();
        assert_eq!((s.meals, s.sleeps), (2, 1));
        assert_eq!(s.mean_time_to_food, Some(3.0));
    }
}
