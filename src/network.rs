//! A layered network that learns its own features (network phase, world physics).
//!
//! Layer 0 is held to the input. Each layer above it is predicted by the one
//! below: `prediction = weights · activity(below) + bias`, where activity is
//! the layer's value itself for the input and `tanh` of it for hidden layers.
//! The last layer is the output.
//!
//! Two ways to learn, picked by [`Learning`]:
//! - **Predictive coding** (the apprentice's own). The output is held to
//!   what came; the hidden layers settle for a few rounds, each unit moving
//!   to reduce its own prediction error and the error of the layer it
//!   predicts. Then each connection moves by the error at its upper end
//!   times the activity at its lower end. Every update is local.
//!
//!   Each hidden layer weighs its own error by a **precision**: how firmly
//!   it holds to what the layer below predicts. With precision 1, settling
//!   lets the hidden units soak up most of the output's error, so the
//!   output's weights learn little and the network learns barely more than
//!   one without hidden layers. A high precision keeps hidden units near
//!   their prediction and passes the output's error down in proportion,
//!   which is what lets every layer learn.
//! - **Backprop** (comparison only). The output error's gradient is passed
//!   down through the weights, as in ordinary deep learning.
//!
//! With no hidden layer both are the same delta rule.

use std::num::NonZeroUsize;

use crate::rng::Rng;

/// How a [`Net`] turns an error into weight changes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Learning {
    /// Settle hidden layers for `rounds` rounds of size `step`, each hidden
    /// layer's error weighted by `precision`, then learn locally.
    PredictiveCoding {
        rounds: u32,
        step: f64,
        precision: f64,
    },
    /// End-to-end gradient of the output error. Comparison only.
    Backprop,
}

/// Why a network cannot be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetError {
    /// Fewer than two layers, or a layer with no units.
    Shape(Vec<usize>),
}

impl std::fmt::Display for NetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shape(sizes) => write!(f, "layer sizes {sizes:?}: need two or more, none empty"),
        }
    }
}

impl std::error::Error for NetError {}

/// Connections from one layer to the next.
#[derive(Debug, Clone, PartialEq)]
struct Link {
    ins: usize,
    /// Row-major, one row per upper unit.
    weights: Vec<f64>,
    bias: Vec<f64>,
}

impl Link {
    fn new(ins: usize, outs: usize, rng: &mut Rng) -> Self {
        let scale = 1.0 / (ins as f64).sqrt();
        Self {
            ins,
            weights: (0..ins * outs)
                .map(|_| scale * (2.0 * rng.unit() - 1.0))
                .collect(),
            bias: vec![0.0; outs],
        }
    }

    /// What the upper layer should be, given the lower layer's activity.
    fn predict(&self, below: &[f64]) -> Vec<f64> {
        self.weights
            .chunks(self.ins)
            .zip(&self.bias)
            .map(|(row, b)| b + dot(row, below))
            .collect()
    }

    /// Error at the upper layer carried down to the lower one: `Wᵀ · error`.
    fn carry(&self, error: &[f64]) -> Vec<f64> {
        let mut down = vec![0.0; self.ins];
        for (row, e) in self.weights.chunks(self.ins).zip(error) {
            for (d, w) in down.iter_mut().zip(row) {
                *d += w * e;
            }
        }
        down
    }

    /// Local rule: each weight moves by its upper unit's error times its
    /// lower unit's activity.
    fn learn(&mut self, below: &[f64], error: &[f64], rate: f64) {
        for ((row, b), e) in self
            .weights
            .chunks_mut(self.ins)
            .zip(&mut self.bias)
            .zip(error)
        {
            *b += rate * e;
            for (w, a) in row.iter_mut().zip(below) {
                *w += rate * e * a;
            }
        }
    }
}

/// A layered network; see the module docs.
#[derive(Debug, Clone, PartialEq)]
pub struct Net {
    links: Vec<Link>,
    rate: f64,
    learning: Learning,
}

impl Net {
    /// A network with these layer sizes, input first, output last; weights
    /// drawn from `seed`.
    pub fn new(
        sizes: &[usize],
        rate: f64,
        learning: Learning,
        seed: u64,
    ) -> Result<Self, NetError> {
        if sizes.len() < 2 || sizes.contains(&0) {
            return Err(NetError::Shape(sizes.to_vec()));
        }
        let mut rng = Rng::new(seed);
        let links = sizes
            .windows(2)
            .map(|w| Link::new(w[0], w[1], &mut rng))
            .collect();
        Ok(Self {
            links,
            rate,
            learning,
        })
    }

    /// A network with one hidden layer. Every size is non-zero, so unlike
    /// [`Net::new`] it cannot fail.
    pub fn one_hidden(sizes: [NonZeroUsize; 3], rate: f64, learning: Learning, seed: u64) -> Self {
        let [ins, hidden, outs] = sizes.map(NonZeroUsize::get);
        let mut rng = Rng::new(seed);
        let links = vec![
            Link::new(ins, hidden, &mut rng),
            Link::new(hidden, outs, &mut rng),
        ];
        Self {
            links,
            rate,
            learning,
        }
    }

    /// Every layer's value on a forward sweep, input first, output last.
    pub fn sweep(&self, input: &[f64]) -> Vec<Vec<f64>> {
        let mut layers = vec![input.to_vec()];
        for (l, link) in self.links.iter().enumerate() {
            let next = link.predict(&activity(l, &layers[l]));
            layers.push(next);
        }
        layers
    }

    /// The output it predicts for `input`.
    pub fn predict(&self, input: &[f64]) -> Vec<f64> {
        self.sweep(input).pop().unwrap_or_default()
    }

    /// Hidden units' activities on a forward sweep, all hidden layers in
    /// order. Empty with no hidden layer.
    pub fn hidden(&self, input: &[f64]) -> Vec<f64> {
        let layers = self.sweep(input);
        let last = layers.len() - 1;
        layers[1..last]
            .iter()
            .enumerate()
            .flat_map(|(i, x)| activity(i + 1, x))
            .collect()
    }

    /// Learns that `input` led to `target`.
    pub fn learn(&mut self, input: &[f64], target: &[f64]) {
        let mut x = self.sweep(input);
        let Learning::PredictiveCoding {
            rounds,
            step,
            precision,
        } = self.learning
        else {
            return self.backprop(&x, target);
        };
        let top = x.len() - 1;
        x[top] = target.to_vec();
        for _ in 0..rounds {
            self.settle(&mut x, step, precision);
        }
        let errors = self.weighted_errors(&x, precision);
        for (l, link) in self.links.iter_mut().enumerate() {
            link.learn(&activity(l, &x[l]), &errors[l], self.rate);
        }
    }

    /// Each layer's error times its precision: `precision` for hidden
    /// layers, 1 for the output.
    fn weighted_errors(&self, x: &[Vec<f64>], precision: f64) -> Vec<Vec<f64>> {
        let top = self.links.len() - 1;
        let mut errors = self.errors(x);
        for e in errors[..top].iter_mut().flatten() {
            *e *= precision;
        }
        errors
    }

    /// Each layer's own prediction error, `errors[l]` for layer `l + 1`.
    fn errors(&self, x: &[Vec<f64>]) -> Vec<Vec<f64>> {
        self.links
            .iter()
            .enumerate()
            .map(|(l, link)| {
                let predicted = link.predict(&activity(l, &x[l]));
                x[l + 1].iter().zip(predicted).map(|(v, p)| v - p).collect()
            })
            .collect()
    }

    /// One round of settling: every hidden unit moves down the total
    /// precision-weighted squared error, using only its own error and the
    /// errors it predicts. The step shrinks with precision, to stay stable.
    fn settle(&self, x: &mut [Vec<f64>], step: f64, precision: f64) {
        let errors = self.weighted_errors(x, precision);
        let step = step / precision;
        for l in 1..x.len() - 1 {
            let up = self.links[l].carry(&errors[l]);
            for (i, v) in x[l].iter_mut().enumerate() {
                *v += step * (-errors[l - 1][i] + slope(*v) * up[i]);
            }
        }
    }

    /// Gradient of the output error, passed down from the top, given the
    /// forward sweep `fed`.
    fn backprop(&mut self, fed: &[Vec<f64>], target: &[f64]) {
        let top = fed.len() - 1;
        let mut delta: Vec<f64> = target.iter().zip(&fed[top]).map(|(t, p)| t - p).collect();
        for l in (0..self.links.len()).rev() {
            let below = activity(l, &fed[l]);
            let down = self.links[l].carry(&delta);
            self.links[l].learn(&below, &delta, self.rate);
            delta = down
                .iter()
                .zip(&fed[l])
                .map(|(d, v)| d * slope(*v))
                .collect();
        }
    }
}

/// A layer's activity: the input as it is, hidden layers through `tanh`.
fn activity(layer: usize, x: &[f64]) -> Vec<f64> {
    if layer == 0 {
        return x.to_vec();
    }
    x.iter().map(|v| v.tanh()).collect()
}

/// Slope of `tanh` at `v`.
fn slope(v: f64) -> f64 {
    1.0 - v.tanh().powi(2)
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PC: Learning = Learning::PredictiveCoding {
        rounds: 20,
        step: 0.2,
        precision: 1.0,
    };

    /// Exclusive or: no single layer can learn it.
    const XOR: [([f64; 2], f64); 4] = [
        ([0.0, 0.0], 0.0),
        ([0.0, 1.0], 1.0),
        ([1.0, 0.0], 1.0),
        ([1.0, 1.0], 0.0),
    ];

    fn xor_error(net: &mut Net, epochs: usize) -> f64 {
        for _ in 0..epochs {
            for (x, y) in XOR {
                net.learn(&x, &[y]);
            }
        }
        XOR.iter()
            .map(|(x, y)| (net.predict(x)[0] - y).powi(2))
            .sum::<f64>()
            / 4.0
    }

    #[test]
    fn rejects_shapes_without_two_layers() {
        assert!(Net::new(&[3], 0.1, PC, 1).is_err());
        assert!(Net::new(&[3, 0, 2], 0.1, PC, 1).is_err());
    }

    #[test]
    fn hidden_layers_learn_what_one_layer_cannot() {
        let mut flat = Net::new(&[2, 1], 0.1, PC, 1).expect("shape");
        let mut deep = Net::new(&[2, 8, 1], 0.1, PC, 1).expect("shape");
        let flat_error = xor_error(&mut flat, 2_000);
        let deep_error = xor_error(&mut deep, 2_000);
        assert!(flat_error > 0.2, "one layer learned xor: {flat_error}");
        assert!(deep_error < 0.02, "hidden layer missed xor: {deep_error}");
    }

    #[test]
    fn backprop_learns_xor_too() {
        let mut net = Net::new(&[2, 8, 1], 0.1, Learning::Backprop, 1).expect("shape");
        let error = xor_error(&mut net, 2_000);
        assert!(error < 0.02, "backprop missed xor: {error}");
    }

    #[test]
    fn without_hidden_layers_both_rules_agree() {
        let mut pc = Net::new(&[2, 1], 0.1, PC, 3).expect("shape");
        let mut bp = Net::new(&[2, 1], 0.1, Learning::Backprop, 3).expect("shape");
        for (x, y) in XOR {
            pc.learn(&x, &[y]);
            bp.learn(&x, &[y]);
        }
        assert_eq!(pc, Net { learning: PC, ..bp });
    }

    #[test]
    fn predictive_coding_follows_the_gradient_on_one_step() {
        // From the same weights, one predictive-coding step should move the
        // output error about as far as backprop does.
        let x = [1.0, 0.0];
        let pc = Net::new(&[2, 4, 1], 0.05, PC, 5).expect("shape");
        let bp = Net {
            learning: Learning::Backprop,
            ..pc.clone()
        };
        let before = (pc.predict(&x)[0] - 1.0).powi(2);
        let after = |mut n: Net| {
            n.learn(&x, &[1.0]);
            (n.predict(&x)[0] - 1.0).powi(2)
        };
        let (pc_after, bp_after) = (after(pc), after(bp));
        assert!(pc_after < before && bp_after < before);
        assert!(
            (pc_after - bp_after).abs() < 0.25 * (before - bp_after),
            "pc {pc_after} vs bp {bp_after} from {before}"
        );
    }

    #[test]
    fn high_precision_learns_like_backprop() {
        // Precision 1 lets hidden units absorb the error; high precision
        // passes it down, so one step lands where backprop's does.
        let x = [1.0, 0.0, 0.5];
        let base = Net::new(&[3, 6, 6, 2], 0.05, PC, 7).expect("shape");
        let after = |learning: Learning| {
            let mut n = Net {
                learning,
                ..base.clone()
            };
            n.learn(&x, &[1.0, -1.0]);
            n.predict(&x)
        };
        let gap = |learning: Learning| {
            let (a, b) = (after(learning), after(Learning::Backprop));
            a.iter().zip(&b).map(|(p, q)| (p - q).abs()).sum::<f64>()
        };
        let firm = Learning::PredictiveCoding {
            rounds: 200,
            step: 0.2,
            precision: 100.0,
        };
        assert!(gap(firm) < 0.2 * gap(PC), "{} vs {}", gap(firm), gap(PC));
    }

    #[test]
    fn hidden_reads_every_hidden_unit() {
        let net = Net::new(&[3, 4, 5, 2], 0.1, PC, 1).expect("shape");
        let h = net.hidden(&[1.0, 0.0, 1.0]);
        assert_eq!(h.len(), 9);
        assert!(h.iter().all(|v| v.abs() < 1.0));
        assert!(
            Net::new(&[3, 2], 0.1, PC, 1)
                .expect("shape")
                .hidden(&[0.0; 3])
                .is_empty()
        );
    }
}
