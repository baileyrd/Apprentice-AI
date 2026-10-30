//! A recurrent network that learns online by e-prop (Bellec et al., 2020).
//!
//! Leaky tanh units with no self connections:
//! `h(t) = α·h(t−1) + (1−α)·tanh(Win·x(t) + Wrec·h(t−1) + b)`, and a linear
//! readout `y = Wout·h(t) + bout`. Every step, each input and recurrent
//! weight updates an eligibility trace from its own presynaptic activity
//! and its unit's slope, `ε(t) = α·ε(t−1) + (1−α)·tanh′(a)·z`; the output
//! error is broadcast to each unit through `Woutᵀ` as a learning signal, and
//! each weight moves by the signal times its trace. No backprop through
//! time and no replay: local in space and time.

use crate::rng::Rng;

/// The network, its state and its traces.
#[derive(Debug, Clone, PartialEq)]
pub struct Eprop {
    ins: usize,
    units: usize,
    outs: usize,
    /// Row per unit: input weights, then recurrent weights, then bias.
    w: Vec<Vec<f64>>,
    /// Row per output: weights, then bias.
    w_out: Vec<Vec<f64>>,
    /// Each unit's leak.
    alpha: Vec<f64>,
    /// Activity.
    h: Vec<f64>,
    /// One trace per weight in `w`, same layout.
    traces: Vec<Vec<f64>>,
    rate: f64,
}

impl Eprop {
    /// `units` units with leaks spread evenly over `leaks`, weights drawn
    /// from `seed`.
    pub fn new(
        ins: usize,
        units: usize,
        outs: usize,
        leaks: (f64, f64),
        rate: f64,
        seed: u64,
    ) -> Self {
        let mut rng = Rng::new(seed);
        let width = ins + units + 1;
        let scale = 1.0 / ((ins + units) as f64).sqrt();
        let w = (0..units)
            .map(|j| {
                (0..width)
                    .map(|i| {
                        let self_link = i == ins + j;
                        let bias = i == width - 1;
                        if self_link || bias {
                            0.0
                        } else {
                            scale * (2.0 * rng.unit() - 1.0)
                        }
                    })
                    .collect()
            })
            .collect();
        let out_scale = 1.0 / (units as f64).sqrt();
        let w_out = (0..outs)
            .map(|_| {
                let mut row: Vec<f64> = (0..units)
                    .map(|_| out_scale * (2.0 * rng.unit() - 1.0))
                    .collect();
                row.push(0.0);
                row
            })
            .collect();
        let span = (units.max(2) - 1) as f64;
        let alpha = (0..units)
            .map(|j| leaks.0 + (leaks.1 - leaks.0) * j as f64 / span)
            .collect();
        Self {
            ins,
            units,
            outs,
            w,
            w_out,
            alpha,
            h: vec![0.0; units],
            traces: vec![vec![0.0; width]; units],
            rate,
        }
    }

    /// Units.
    pub fn units(&self) -> usize {
        self.units
    }

    /// Activity after `x` from state `h`, and the readout of it. Changes
    /// nothing.
    pub fn peek(&self, h: &[f64], x: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let next: Vec<f64> = (0..self.units)
            .map(|j| {
                let a = self.drive(j, h, x);
                self.alpha[j] * h[j] + (1.0 - self.alpha[j]) * a.tanh()
            })
            .collect();
        let y = self.read(&next);
        (next, y)
    }

    /// Its current activity.
    pub fn state(&self) -> &[f64] {
        &self.h
    }

    /// Takes in `x`, then learns that the readout should have been
    /// `target`: traces, learning signal and weight changes, all local.
    pub fn advance(&mut self, x: &[f64], target: &[f64]) {
        let pre: Vec<f64> = x.iter().chain(&self.h).copied().chain([1.0]).collect();
        let drives: Vec<f64> = (0..self.units).map(|j| self.drive(j, &self.h, x)).collect();
        let mut next = vec![0.0; self.units];
        for (j, (a, traces)) in drives.iter().zip(&mut self.traces).enumerate() {
            let slope = 1.0 - a.tanh().powi(2);
            let (keep, take) = (self.alpha[j], 1.0 - self.alpha[j]);
            next[j] = keep * self.h[j] + take * a.tanh();
            for (e, z) in traces.iter_mut().zip(&pre) {
                *e = keep * *e + take * slope * z;
            }
        }
        let error: Vec<f64> = self
            .read(&next)
            .iter()
            .zip(target)
            .map(|(y, t)| y - t)
            .collect();
        for j in 0..self.units {
            let signal: f64 = (0..self.outs).map(|k| self.w_out[k][j] * error[k]).sum();
            for (i, (w, e)) in self.w[j].iter_mut().zip(&self.traces[j]).enumerate() {
                if i != self.ins + j {
                    *w -= self.rate * signal * e;
                }
            }
        }
        let with_bias: Vec<f64> = next.iter().copied().chain([1.0]).collect();
        for (row, e) in self.w_out.iter_mut().zip(&error) {
            for (w, a) in row.iter_mut().zip(&with_bias) {
                *w -= self.rate * e * a;
            }
        }
        self.h = next;
    }

    fn drive(&self, j: usize, h: &[f64], x: &[f64]) -> f64 {
        let row = &self.w[j];
        let input: f64 = row[..self.ins].iter().zip(x).map(|(w, v)| w * v).sum();
        let recur: f64 = row[self.ins..self.ins + self.units]
            .iter()
            .zip(h)
            .map(|(w, v)| w * v)
            .sum();
        input + recur + row[self.ins + self.units]
    }

    fn read(&self, h: &[f64]) -> Vec<f64> {
        self.w_out
            .iter()
            .map(|row| row[self.units] + row.iter().zip(h).map(|(w, v)| w * v).sum::<f64>())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaks_spread_and_no_self_links() {
        let e = Eprop::new(2, 5, 1, (0.5, 0.9), 0.01, 0);
        assert_eq!(e.alpha.first(), Some(&0.5));
        assert!((e.alpha[4] - 0.9).abs() < 1e-12);
        assert!((0..5).all(|j| e.w[j][2 + j] == 0.0));
    }

    #[test]
    fn peek_matches_advance_and_changes_nothing() {
        let mut e = Eprop::new(2, 4, 1, (0.5, 0.9), 0.0, 1);
        let before = e.clone();
        let (h, _) = e.peek(&e.h.clone(), &[1.0, -1.0]);
        assert_eq!(e, before);
        e.advance(&[1.0, -1.0], &[0.0]);
        assert_eq!(e.state(), &h[..]);
    }

    #[test]
    fn learns_a_fixed_mapping() {
        let mut e = Eprop::new(1, 8, 1, (0.0, 0.0), 0.05, 2);
        for t in 0..4_000 {
            let x = if t % 2 == 0 { 1.0 } else { -1.0 };
            e.advance(&[x], &[0.5 * x]);
        }
        let (_, y) = e.peek(&e.h.clone(), &[1.0]);
        assert!((y[0] - 0.5).abs() < 0.1, "{y:?}");
    }

    #[test]
    fn learns_to_echo_the_last_input() {
        // Needs one step of state, carried by the recurrent units.
        let mut rng = Rng::new(3);
        let mut e = Eprop::new(1, 16, 1, (0.0, 0.5), 0.02, 4);
        let mut last = 0.0;
        let mut errors = vec![];
        for _ in 0..20_000 {
            let x = if rng.chance(0.5) { 1.0 } else { -1.0 };
            let (_, y) = e.peek(&e.h.clone(), &[x]);
            errors.push((y[0] - last).abs());
            e.advance(&[x], &[last]);
            last = x;
        }
        let tail = &errors[errors.len() - 1_000..];
        let mean = tail.iter().sum::<f64>() / tail.len() as f64;
        assert!(mean < 0.5, "mean error {mean}");
    }
}
