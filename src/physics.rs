//! World physics for the network phase: what a step looks like to a network,
//! and today's map's guess at the next view, for comparison.
//!
//! A step is the 3×3 view and the action taken; what follows is the next
//! view and the energy change. Views are one unit per square per kind of
//! square, in the order of [`crate::model::kind`].

use crate::body::{Action, Observation, VIEW};
use crate::model::{Belief, KINDS, PredictiveCoding, WorldModel, error, kind, one_hot};
use crate::world::{Cell, Dir};

/// Squares in a view.
pub const SQUARES: usize = VIEW * VIEW;
/// Kinds of action: four moves, eat, rest.
pub const ACTIONS: usize = 6;
/// Units the network sees: the view, then the action.
pub const INPUTS: usize = SQUARES * KINDS + ACTIONS;
/// Units it predicts: the next view, then the energy change.
pub const OUTPUTS: usize = SQUARES * KINDS + 1;

/// Energy change of one meal; the energy output is in these units.
pub const MEAL: f64 = 25.0;

fn action_index(action: Action) -> usize {
    match action {
        Action::Move(Dir::North) => 0,
        Action::Move(Dir::East) => 1,
        Action::Move(Dir::South) => 2,
        Action::Move(Dir::West) => 3,
        Action::Eat => 4,
        Action::Rest => 5,
    }
}

fn view_units(view: &[Cell; SQUARES]) -> impl Iterator<Item = f64> + '_ {
    view.iter().flat_map(|&c| one_hot(c))
}

/// What the network sees: the view and the action.
pub fn input(obs: &Observation, action: Action) -> Vec<f64> {
    let mut x: Vec<f64> = view_units(&obs.view).collect();
    let mut act = [0.0; ACTIONS];
    act[action_index(action)] = 1.0;
    x.extend(act);
    x
}

/// What it should have predicted: the next view and the energy change.
pub fn target(next: &Observation, energy_change: f64) -> Vec<f64> {
    let mut y: Vec<f64> = view_units(&next.view).collect();
    y.push(energy_change / MEAL);
    y
}

/// A predicted next view, one belief per square.
pub type View = [Belief; SQUARES];

/// The view part of a network's output.
pub fn view_of(output: &[f64]) -> View {
    std::array::from_fn(|i| std::array::from_fn(|k| output[i * KINDS + k]))
}

/// "Surprise": squared prediction error per square, as in every phase.
pub fn surprise(predicted: &View, next: &Observation) -> f64 {
    predicted
        .iter()
        .zip(&next.view)
        .map(|(b, &c)| error(b, c))
        .sum::<f64>()
        / SQUARES as f64
}

/// Today's map's guess at the next view: the squares around where it
/// expects to stand next, one step over for a move unless it believes that
/// square is a wall.
pub fn map_predicts(map: &PredictiveCoding, obs: &Observation, action: Action) -> View {
    let mut at = obs.offset;
    if let Action::Move(dir) = action {
        let to = at.step(dir);
        if map.predict(to)[kind(Cell::Wall)] <= 0.5 {
            at = to;
        }
    }
    std::array::from_fn(|i| {
        let (dx, dy) = Observation::view_delta(i);
        map.predict(at.shift(dx, dy))
    })
}

/// Area under the curve: the chance a random `hit` scores above a random
/// `miss` (ties count half), folded so that 0.5 is no better than a coin
/// and 1.0 separates them perfectly, whichever way round. `None` without
/// both.
pub fn separation(hits: &[f64], misses: &[f64]) -> Option<f64> {
    if hits.is_empty() || misses.is_empty() {
        return None;
    }
    let mut all: Vec<(f64, bool)> = hits
        .iter()
        .map(|&s| (s, true))
        .chain(misses.iter().map(|&s| (s, false)))
        .collect();
    all.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Sum of the hits' ranks, ties sharing their average rank.
    let mut rank_sum = 0.0;
    let mut i = 0;
    while i < all.len() {
        let j = (i + 1..all.len())
            .find(|&j| all[j].0 != all[i].0)
            .unwrap_or(all.len());
        let rank = (i + j + 1) as f64 / 2.0;
        rank_sum += rank * all[i..j].iter().filter(|e| e.1).count() as f64;
        i = j;
    }
    let (h, m) = (hits.len() as f64, misses.len() as f64);
    let auc = (rank_sum - h * (h + 1.0) / 2.0) / (h * m);
    Some(auc.max(1.0 - auc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Rates;
    use crate::world::Pos;

    fn obs(view: [Cell; SQUARES], x: i32) -> Observation {
        Observation {
            view,
            energy: 50,
            offset: Pos { x, y: 0 },
        }
    }

    #[test]
    fn input_and_target_have_their_sizes_and_one_hot_squares() {
        let o = obs([Cell::Empty; SQUARES], 0);
        let x = input(&o, Action::Eat);
        assert_eq!(x.len(), INPUTS);
        assert_eq!(x.iter().sum::<f64>(), (SQUARES + 1) as f64);
        assert_eq!(x[SQUARES * KINDS + 4], 1.0);
        let y = target(&o, 25.0);
        assert_eq!(y.len(), OUTPUTS);
        assert_eq!(y[OUTPUTS - 1], 1.0);
    }

    #[test]
    fn a_perfect_guess_is_no_surprise() {
        let mut view = [Cell::Empty; SQUARES];
        view[2] = Cell::Wall;
        let o = obs(view, 0);
        let out = target(&o, 0.0);
        assert_eq!(surprise(&view_of(&out), &o), 0.0);
    }

    #[test]
    fn map_expects_the_shifted_view_and_a_bump_at_a_known_wall() {
        let mut map = PredictiveCoding::new(Rates::default());
        let mut view = [Cell::Empty; SQUARES];
        view[5] = Cell::Food; // east
        view[3] = Cell::Wall; // west
        let here = obs(view, 0);
        for _ in 0..5 {
            map.observe(&here);
        }
        let east = map_predicts(&map, &here, Action::Move(Dir::East));
        assert_eq!(crate::model::most_likely(&east[4]), Some(Cell::Food));
        let west = map_predicts(&map, &here, Action::Move(Dir::West));
        assert_eq!(crate::model::most_likely(&west[3]), Some(Cell::Wall));
    }

    #[test]
    fn separation_reads_both_directions_and_ties() {
        assert_eq!(separation(&[3.0, 4.0], &[1.0, 2.0]), Some(1.0));
        assert_eq!(separation(&[1.0, 2.0], &[3.0, 4.0]), Some(1.0));
        assert_eq!(separation(&[1.0, 1.0], &[1.0, 1.0]), Some(0.5));
        assert_eq!(separation(&[], &[1.0]), None);
    }
}
