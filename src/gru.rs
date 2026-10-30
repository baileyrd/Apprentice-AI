//! A small GRU trained by backprop through time: a ceiling check only.
//!
//! The apprentice's own learning is local; this is the usual way recurrent
//! networks are trained, used to ask whether a task can be learned by a
//! network that keeps state at all (see `docs/facing/README.md`). Like
//! [`crate::network::Learning::Backprop`], it is a comparison, never the
//! apprentice's learner.
//!
//! Step: `z = σ(Wz·x + Uz·h + bz)`, `r = σ(Wr·x + Ur·h + br)`,
//! `n = tanh(Wn·x + Un·(r⊙h) + bn)`, `h' = (1−z)⊙n + z⊙h`, output
//! `y = Wy·h' + by`.

use crate::rng::Rng;

/// One step's inputs and what should come out.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub input: Vec<f64>,
    pub target: Vec<f64>,
}

/// How training goes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Training {
    pub epochs: usize,
    /// Steps per chunk; gradients are cut at chunk edges.
    pub chunk: usize,
    pub rate: f64,
    /// Largest gradient norm per chunk.
    pub clip: f64,
}

/// Parameter blocks, in a fixed order.
const BLOCKS: usize = 11;
const WZ: usize = 0;
const UZ: usize = 1;
const BZ: usize = 2;
const WR: usize = 3;
const UR: usize = 4;
const BR: usize = 5;
const WN: usize = 6;
const UN: usize = 7;
const BN: usize = 8;
const WY: usize = 9;
const BY: usize = 10;

/// The network: every parameter, block by block.
#[derive(Debug, Clone, PartialEq)]
pub struct Gru {
    ins: usize,
    hidden: usize,
    outs: usize,
    params: Vec<Vec<f64>>,
}

/// What a forward step keeps for the backward pass.
#[derive(Debug, Clone)]
struct Trace {
    x: Vec<f64>,
    h: Vec<f64>,
    z: Vec<f64>,
    r: Vec<f64>,
    n: Vec<f64>,
    rh: Vec<f64>,
    out: Vec<f64>,
}

impl Gru {
    /// A GRU with weights drawn from `seed`.
    pub fn new(ins: usize, hidden: usize, outs: usize, seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let mut draw = |rows: usize, cols: usize| -> Vec<f64> {
            let scale = 1.0 / (cols as f64).sqrt();
            (0..rows * cols)
                .map(|_| scale * (2.0 * rng.unit() - 1.0))
                .collect()
        };
        let params = vec![
            draw(hidden, ins),
            draw(hidden, hidden),
            vec![0.0; hidden],
            draw(hidden, ins),
            draw(hidden, hidden),
            vec![0.0; hidden],
            draw(hidden, ins),
            draw(hidden, hidden),
            vec![0.0; hidden],
            draw(outs, hidden),
            vec![0.0; outs],
        ];
        Self {
            ins,
            hidden,
            outs,
            params,
        }
    }

    /// Hidden units.
    pub fn hidden(&self) -> usize {
        self.hidden
    }

    /// A zero hidden state.
    pub fn rest(&self) -> Vec<f64> {
        vec![0.0; self.hidden]
    }

    /// One step: the new hidden state and the output read from it.
    pub fn step(&self, h: &[f64], x: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let t = self.forward(h, x);
        let h2 = self.next_hidden(&t);
        (h2, t.out)
    }

    fn forward(&self, h: &[f64], x: &[f64]) -> Trace {
        let p = &self.params;
        let gate = |w: usize, u: usize, b: usize, hh: &[f64]| -> Vec<f64> {
            let a = matvec(&p[w], x);
            let c = matvec(&p[u], hh);
            (0..self.hidden).map(|i| a[i] + c[i] + p[b][i]).collect()
        };
        let z: Vec<f64> = gate(WZ, UZ, BZ, h).into_iter().map(sigmoid).collect();
        let r: Vec<f64> = gate(WR, UR, BR, h).into_iter().map(sigmoid).collect();
        let rh: Vec<f64> = r.iter().zip(h).map(|(a, b)| a * b).collect();
        let n: Vec<f64> = gate(WN, UN, BN, &rh).into_iter().map(f64::tanh).collect();
        let mut t = Trace {
            x: x.to_vec(),
            h: h.to_vec(),
            z,
            r,
            n,
            rh,
            out: vec![],
        };
        let h2 = self.next_hidden(&t);
        let y = matvec(&p[WY], &h2);
        t.out = (0..self.outs).map(|i| y[i] + p[BY][i]).collect();
        t
    }

    fn next_hidden(&self, t: &Trace) -> Vec<f64> {
        (0..self.hidden)
            .map(|i| (1.0 - t.z[i]) * t.n[i] + t.z[i] * t.h[i])
            .collect()
    }

    /// Squared error over a chunk from hidden state `h`, its gradient
    /// (block by block), and the hidden state at the chunk's end.
    fn chunk_gradient(&self, h: &[f64], chunk: &[Sample]) -> (f64, Vec<Vec<f64>>, Vec<f64>) {
        let mut traces = Vec::with_capacity(chunk.len());
        let mut state = h.to_vec();
        for s in chunk {
            let t = self.forward(&state, &s.input);
            state = self.next_hidden(&t);
            traces.push(t);
        }
        let mut grad: Vec<Vec<f64>> = self.params.iter().map(|b| vec![0.0; b.len()]).collect();
        let mut loss = 0.0;
        let mut dh_next = vec![0.0; self.hidden];
        for (t, s) in traces.iter().zip(chunk).rev() {
            let h2 = self.next_hidden(t);
            let dy: Vec<f64> = t.out.iter().zip(&s.target).map(|(o, y)| o - y).collect();
            loss += dy.iter().map(|e| e * e).sum::<f64>();
            outer_add(&mut grad[WY], &dy, &h2);
            add(&mut grad[BY], &dy);
            let mut dh2 = transpose_matvec(&self.params[WY], &dy, self.hidden);
            add(&mut dh2, &dh_next);
            dh_next = self.backward_step(t, &dh2, &mut grad);
        }
        (loss, grad, state)
    }

    /// Back through one step: adds parameter gradients, returns the
    /// gradient at the step's incoming hidden state.
    fn backward_step(&self, t: &Trace, dh2: &[f64], grad: &mut [Vec<f64>]) -> Vec<f64> {
        let p = &self.params;
        let hn = self.hidden;
        let mut dh: Vec<f64> = (0..hn).map(|i| dh2[i] * t.z[i]).collect();
        let da_n: Vec<f64> = (0..hn)
            .map(|i| dh2[i] * (1.0 - t.z[i]) * (1.0 - t.n[i] * t.n[i]))
            .collect();
        let da_z: Vec<f64> = (0..hn)
            .map(|i| dh2[i] * (t.h[i] - t.n[i]) * t.z[i] * (1.0 - t.z[i]))
            .collect();
        outer_add(&mut grad[WN], &da_n, &t.x);
        outer_add(&mut grad[UN], &da_n, &t.rh);
        add(&mut grad[BN], &da_n);
        let drh = transpose_matvec(&p[UN], &da_n, hn);
        let da_r: Vec<f64> = (0..hn)
            .map(|i| drh[i] * t.h[i] * t.r[i] * (1.0 - t.r[i]))
            .collect();
        for i in 0..hn {
            dh[i] += drh[i] * t.r[i];
        }
        outer_add(&mut grad[WZ], &da_z, &t.x);
        outer_add(&mut grad[UZ], &da_z, &t.h);
        add(&mut grad[BZ], &da_z);
        outer_add(&mut grad[WR], &da_r, &t.x);
        outer_add(&mut grad[UR], &da_r, &t.h);
        add(&mut grad[BR], &da_r);
        add(&mut dh, &transpose_matvec(&p[UZ], &da_z, hn));
        add(&mut dh, &transpose_matvec(&p[UR], &da_r, hn));
        dh
    }

    /// Trains on `life`, in order, by truncated BPTT and Adam. Returns the
    /// mean squared error per step of the last epoch.
    pub fn train(&mut self, life: &[Sample], how: &Training) -> f64 {
        let mut adam = Adam::new(&self.params);
        let mut last = 0.0;
        for _ in 0..how.epochs {
            let mut h = self.rest();
            let mut total = 0.0;
            for chunk in life.chunks(how.chunk.max(1)) {
                let (loss, mut grad, end) = self.chunk_gradient(&h, chunk);
                clip(&mut grad, how.clip);
                adam.apply(&mut self.params, &grad, how.rate);
                total += loss;
                h = end;
            }
            last = total / life.len().max(1) as f64;
        }
        last
    }
}

/// Adam's running moments, one per parameter.
struct Adam {
    m: Vec<Vec<f64>>,
    v: Vec<Vec<f64>>,
    t: i32,
}

impl Adam {
    fn new(params: &[Vec<f64>]) -> Self {
        let zeros: Vec<Vec<f64>> = params.iter().map(|b| vec![0.0; b.len()]).collect();
        Self {
            m: zeros.clone(),
            v: zeros,
            t: 0,
        }
    }

    fn apply(&mut self, params: &mut [Vec<f64>], grad: &[Vec<f64>], rate: f64) {
        const B1: f64 = 0.9;
        const B2: f64 = 0.999;
        self.t += 1;
        let (c1, c2) = (1.0 - B1.powi(self.t), 1.0 - B2.powi(self.t));
        for b in 0..BLOCKS {
            for i in 0..params[b].len() {
                let g = grad[b][i];
                self.m[b][i] = B1 * self.m[b][i] + (1.0 - B1) * g;
                self.v[b][i] = B2 * self.v[b][i] + (1.0 - B2) * g * g;
                let step = (self.m[b][i] / c1) / ((self.v[b][i] / c2).sqrt() + 1e-8);
                params[b][i] -= rate * step;
            }
        }
    }
}

fn clip(grad: &mut [Vec<f64>], max: f64) {
    let norm = grad.iter().flatten().map(|g| g * g).sum::<f64>().sqrt();
    if norm <= max {
        return;
    }
    let scale = max / norm;
    grad.iter_mut().flatten().for_each(|g| *g *= scale);
}

fn sigmoid(a: f64) -> f64 {
    1.0 / (1.0 + (-a).exp())
}

/// `W·v`, with `W` row-major and as many columns as `v` has entries.
fn matvec(w: &[f64], v: &[f64]) -> Vec<f64> {
    w.chunks(v.len())
        .map(|row| row.iter().zip(v).map(|(a, b)| a * b).sum())
        .collect()
}

/// `Wᵀ·e`, for `W` with `cols` columns.
fn transpose_matvec(w: &[f64], e: &[f64], cols: usize) -> Vec<f64> {
    let mut out = vec![0.0; cols];
    for (row, ei) in w.chunks(cols).zip(e) {
        for (o, wij) in out.iter_mut().zip(row) {
            *o += wij * ei;
        }
    }
    out
}

/// `G += a·bᵀ`.
fn outer_add(g: &mut [f64], a: &[f64], b: &[f64]) {
    for (row, ai) in g.chunks_mut(b.len()).zip(a) {
        for (gij, bj) in row.iter_mut().zip(b) {
            *gij += ai * bj;
        }
    }
}

fn add(a: &mut [f64], b: &[f64]) {
    a.iter_mut().zip(b).for_each(|(x, y)| *x += y);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples(n: usize, seed: u64) -> Vec<Sample> {
        let mut rng = Rng::new(seed);
        (0..n)
            .map(|_| Sample {
                input: (0..3).map(|_| rng.unit() - 0.5).collect(),
                target: (0..2).map(|_| rng.unit() - 0.5).collect(),
            })
            .collect()
    }

    #[test]
    fn gradient_matches_finite_differences() {
        let g = Gru::new(3, 4, 2, 1);
        let data = samples(5, 2);
        let h0: Vec<f64> = vec![0.1, -0.2, 0.3, 0.0];
        let (_, grad, _) = g.chunk_gradient(&h0, &data);
        let eps = 1e-6;
        for (b, block) in grad.iter().enumerate() {
            for (i, &g_bi) in block.iter().enumerate() {
                let mut up = g.clone();
                up.params[b][i] += eps;
                let mut down = g.clone();
                down.params[b][i] -= eps;
                let numeric = (up.chunk_gradient(&h0, &data).0 - down.chunk_gradient(&h0, &data).0)
                    / (2.0 * eps);
                // The loss is a plain sum of squares, so its gradient is twice
                // the backward pass's (which drops the factor 2).
                let analytic = 2.0 * g_bi;
                assert!(
                    (numeric - analytic).abs() < 1e-5 * (1.0 + numeric.abs()),
                    "block {b} index {i}: numeric {numeric} analytic {analytic}"
                );
            }
        }
    }

    #[test]
    fn learns_to_echo_the_input_from_two_steps_back() {
        // Needs state: the target is the first input from two steps ago.
        let mut rng = Rng::new(3);
        let xs: Vec<f64> = (0..3_000)
            .map(|_| if rng.chance(0.5) { 1.0 } else { -1.0 })
            .collect();
        let life: Vec<Sample> = (0..xs.len())
            .map(|t| Sample {
                input: vec![xs[t]],
                target: vec![if t >= 2 { xs[t - 2] } else { 0.0 }],
            })
            .collect();
        let mut g = Gru::new(1, 8, 1, 4);
        let how = Training {
            epochs: 30,
            chunk: 20,
            rate: 0.01,
            clip: 1.0,
        };
        let mse = g.train(&life, &how);
        assert!(mse < 0.1, "mse {mse}");
    }
}
