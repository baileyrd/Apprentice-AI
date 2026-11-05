//! Clones: a learned map of hidden states that holds a fact across steps.
//!
//! Each *view* (a learned look prototype, plus which ring unit is active
//! when there is a ring) owns a fixed number of hidden *clones*. A belief
//! over clones is carried from step to step by the fired motor's
//! transition weights and narrowed to the clones of the view it then sees.
//! Two clones of one view can stand for two places that look alike, so the
//! belief holds what the view alone can't.
//!
//! Learning is local (docs/hold, QA.md § Learners):
//! - **Looks.** A new prototype is recruited when no prototype is within
//!   `recruit` (squared distance); otherwise the nearest moves to the
//!   running mean of what it has matched.
//! - **Same view.** A step that keeps the view adds belief-before ×
//!   weight to each transition count into the view's clones (normalised).
//! - **View change (view-change credit).** A step that changes the view
//!   leaves the same product as a tag on each synapse into the new view's
//!   clones. When the view next changes, each of those clones scores its own
//!   forecast of that change from its own outgoing weights, and the tag
//!   becomes a count change in proportion to it. No signal passes through
//!   other clones or back over time.
//!
//! Counts start at `persist` on the diagonal for every motor (things tend to
//! stay as they are; no motor is labelled still) plus tiny random counts
//! (`jitter`) to break symmetry. Weights are counts over their row's sum.
//! Pure; no I/O.

use crate::rng::Rng;

/// Fixed settings, disclosed in docs/hold.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    /// Noisy look numbers at the front of a percept.
    pub look: usize,
    /// One-hot ring units after them (0 for none).
    pub ring: usize,
    pub motors: usize,
    /// Hidden clones per view.
    pub clones: usize,
    /// Squared distance beyond which a percept recruits a new look.
    pub recruit: f64,
    /// Starting count on each clone's own transition, every motor.
    pub persist: f64,
    /// Largest starting random count on every other transition.
    pub jitter: f64,
}

/// A view-changing step awaiting the next change to credit it.
#[derive(Debug, Clone, PartialEq)]
struct Tag {
    belief: Vec<f64>,
    motor: usize,
    view: usize,
}

/// The learner: looks, transition counts, belief.
#[derive(Debug, Clone, PartialEq)]
pub struct Clones {
    s: Settings,
    /// Look prototypes and how many percepts each has matched.
    looks: Vec<(Vec<f64>, u64)>,
    /// View id → (look, ring unit), in order of first sight.
    views: Vec<(usize, usize)>,
    /// Per motor, rows of transition counts over all clones.
    counts: Vec<Vec<Vec<f64>>>,
    /// Per motor, each row's sum.
    sums: Vec<Vec<f64>>,
    /// Over all clones; empty before the first percept.
    belief: Vec<f64>,
    view: Option<usize>,
    tag: Option<Tag>,
    rng: Rng,
}

impl Clones {
    pub fn new(s: Settings, seed: u64) -> Self {
        Self {
            s,
            looks: Vec::new(),
            views: Vec::new(),
            counts: vec![Vec::new(); s.motors],
            sums: vec![Vec::new(); s.motors],
            belief: Vec::new(),
            view: None,
            tag: None,
            rng: Rng::new(seed),
        }
    }

    /// Clones so far.
    pub fn size(&self) -> usize {
        self.views.len() * self.s.clones
    }

    /// Views seen so far.
    pub fn views(&self) -> usize {
        self.views.len()
    }

    /// Belief over clones now (empty before the first percept).
    pub fn belief(&self) -> &[f64] {
        &self.belief
    }

    /// `belief` carried one step by `motor`'s weights.
    pub fn forecast(&self, belief: &[f64], motor: usize) -> Vec<f64> {
        let mut out = vec![0.0; self.size()];
        for (i, &b) in belief.iter().enumerate().filter(|&(_, &b)| b > 0.0) {
            let (row, sum) = (&self.counts[motor][i], self.sums[motor][i]);
            for (o, &c) in out.iter_mut().zip(row) {
                *o += b * c / sum;
            }
        }
        out
    }

    /// The percept of the view `belief` puts most weight on: its look
    /// prototype, then its ring unit. Zeros when it knows no view.
    pub fn expected(&self, belief: &[f64]) -> Vec<f64> {
        let mut out = vec![0.0; self.s.look + self.s.ring];
        let mass = |v: usize| -> f64 { belief[self.span(v)].iter().sum() };
        let Some(best) = (0..self.views.len()).max_by(|&a, &b| mass(a).total_cmp(&mass(b))) else {
            return out;
        };
        let (look, unit) = self.views[best];
        out[..self.s.look].copy_from_slice(&self.looks[look].0);
        if self.s.ring > 0 {
            out[self.s.look + unit] = 1.0;
        }
        out
    }

    /// Whether it has seen anything yet.
    pub fn started(&self) -> bool {
        self.view.is_some()
    }

    /// Sees its first percept: belief spread over that view's clones.
    pub fn start(&mut self, percept: &[f64]) {
        let view = self.observe(percept);
        self.belief = self.uniform(view);
        self.view = Some(view);
    }

    /// Fired `motor`, then saw `next`: learns, and moves the belief on.
    /// Before [`Clones::start`], it starts on `next` instead.
    pub fn step(&mut self, motor: usize, next: &[f64]) {
        let Some(last) = self.view else {
            self.start(next);
            return;
        };
        let view = self.observe(next);
        let forecast = self.forecast(&self.belief, motor);
        if view == last {
            let credit = self.credit_into(&self.belief, motor, view, |_| 1.0);
            self.add(motor, credit);
        } else {
            if let Some(tag) = self.tag.take() {
                let own = |j: usize| self.forecast_of(j, motor, view);
                let credit = self.credit_into(&tag.belief, tag.motor, tag.view, own);
                self.add(tag.motor, credit);
            }
            self.tag = Some(Tag {
                belief: self.belief.clone(),
                motor,
                view,
            });
        }
        self.belief = self.narrow(forecast, view);
        self.view = Some(view);
    }

    /// The view of `percept`, recruiting a look or a view if new; trains
    /// the matched look.
    fn observe(&mut self, percept: &[f64]) -> usize {
        let (sense, ring) = percept.split_at(self.s.look);
        let look = match self.nearest(sense) {
            Some((l, d)) if d <= self.s.recruit => {
                let (proto, n) = &mut self.looks[l];
                *n += 1;
                let rate = 1.0 / *n as f64;
                for (p, x) in proto.iter_mut().zip(sense) {
                    *p += rate * (x - *p);
                }
                l
            }
            _ => {
                self.looks.push((sense.to_vec(), 1));
                self.looks.len() - 1
            }
        };
        let unit = active(ring);
        match self.views.iter().position(|&v| v == (look, unit)) {
            Some(v) => v,
            None => self.add_view(look, unit),
        }
    }

    fn nearest(&self, sense: &[f64]) -> Option<(usize, f64)> {
        self.looks
            .iter()
            .map(|(p, _)| p.iter().zip(sense).map(|(a, b)| (a - b).powi(2)).sum())
            .enumerate()
            .min_by(|a: &(usize, f64), b| a.1.total_cmp(&b.1))
    }

    /// Adds a view's clones: new columns on every row, then new rows.
    fn add_view(&mut self, look: usize, unit: usize) -> usize {
        self.views.push((look, unit));
        let (old, new) = (self.size() - self.s.clones, self.size());
        for m in 0..self.s.motors {
            for i in 0..old {
                for _ in old..new {
                    let c = self.s.jitter * self.rng.unit();
                    self.counts[m][i].push(c);
                    self.sums[m][i] += c;
                }
            }
            for i in old..new {
                let row: Vec<f64> = (0..new)
                    .map(|j| {
                        let c = self.s.jitter * self.rng.unit();
                        if i == j { c + self.s.persist } else { c }
                    })
                    .collect();
                self.sums[m].push(row.iter().sum());
                self.counts[m].push(row);
            }
        }
        self.belief.resize(new, 0.0);
        if let Some(tag) = &mut self.tag {
            tag.belief.resize(new, 0.0);
        }
        self.views.len() - 1
    }

    /// Clone `j`'s own forecast that `motor` shows `view`.
    fn forecast_of(&self, j: usize, motor: usize, view: usize) -> f64 {
        let row = &self.counts[motor][j];
        row[self.span(view)].iter().sum::<f64>() / self.sums[motor][j]
    }

    /// Credit for each transition from `belief` by `motor` into `view`'s
    /// clones, each target weighted by `score`, normalised to sum 1.
    /// Entries are (from, to, amount).
    fn credit_into(
        &self,
        belief: &[f64],
        motor: usize,
        view: usize,
        score: impl Fn(usize) -> f64,
    ) -> Vec<(usize, usize, f64)> {
        let span = self.span(view);
        let scores: Vec<f64> = span.clone().map(&score).collect();
        let mut credit = Vec::new();
        for (i, &b) in belief.iter().enumerate().filter(|&(_, &b)| b > 0.0) {
            let (row, sum) = (&self.counts[motor][i], self.sums[motor][i]);
            for (j, s) in span.clone().zip(&scores) {
                credit.push((i, j, b * row[j] / sum * s));
            }
        }
        let total: f64 = credit.iter().map(|c| c.2).sum();
        if total <= f64::MIN_POSITIVE {
            return Vec::new();
        }
        credit.iter().map(|&(i, j, x)| (i, j, x / total)).collect()
    }

    fn add(&mut self, motor: usize, credit: Vec<(usize, usize, f64)>) {
        for (i, j, x) in credit {
            self.counts[motor][i][j] += x;
            self.sums[motor][i] += x;
        }
    }

    /// `forecast` kept to `view`'s clones and normalised; uniform over them
    /// when the forecast gave the view nothing.
    fn narrow(&self, forecast: Vec<f64>, view: usize) -> Vec<f64> {
        let span = self.span(view);
        let total: f64 = forecast[span.clone()].iter().sum();
        if total <= f64::MIN_POSITIVE {
            return self.uniform(view);
        }
        let mut out = vec![0.0; forecast.len()];
        for j in span {
            out[j] = forecast[j] / total;
        }
        out
    }

    fn uniform(&self, view: usize) -> Vec<f64> {
        let mut out = vec![0.0; self.size()];
        for j in self.span(view) {
            out[j] = 1.0 / self.s.clones as f64;
        }
        out
    }

    fn span(&self, view: usize) -> std::ops::Range<usize> {
        view * self.s.clones..(view + 1) * self.s.clones
    }
}

/// Index of the active (largest) ring unit; 0 with no ring.
fn active(ring: &[f64]) -> usize {
    (0..ring.len())
        .max_by(|&a, &b| ring[a].total_cmp(&ring[b]))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Settings {
        Settings {
            look: 2,
            ring: 0,
            motors: 2,
            clones: 4,
            recruit: 0.5,
            persist: 1.0,
            jitter: 0.001,
        }
    }

    #[test]
    fn recruits_a_view_per_distinct_look_and_merges_near_ones() {
        let mut c = Clones::new(settings(), 1);
        c.step(0, &[1.0, 1.0]);
        c.step(0, &[1.1, 0.9]);
        c.step(0, &[-1.0, 1.0]);
        assert_eq!(c.views(), 2);
        assert_eq!(c.size(), 8);
        let total: f64 = c.belief().iter().sum();
        assert!((total - 1.0).abs() < 1e-12);
    }

    #[test]
    fn ring_unit_splits_views_of_one_look() {
        let s = Settings {
            ring: 2,
            ..settings()
        };
        let mut c = Clones::new(s, 1);
        c.step(0, &[1.0, 1.0, 1.0, 0.0]);
        c.step(0, &[1.0, 1.0, 0.0, 1.0]);
        assert_eq!(c.views(), 2);
        let p = c.expected(c.belief());
        assert_eq!(&p[2..], &[0.0, 1.0]);
    }

    #[test]
    fn expected_is_zero_before_any_view() {
        let c = Clones::new(settings(), 1);
        assert_eq!(c.expected(&[]), vec![0.0, 0.0]);
    }

    /// A ring of four places A B A C; motor 0 moves on, motor 1 stays.
    /// From an A, what comes next depends on which A it is, which only the
    /// view before it told. After learning, moves from either A are
    /// predicted: the belief holds which A it is through the stays.
    #[test]
    fn learns_to_tell_look_alikes_apart() {
        let looks = [[1.0, 1.0], [-1.0, 1.0], [1.0, 1.0], [1.0, -1.0]];
        let mut c = Clones::new(settings(), 3);
        let mut rng = Rng::new(9);
        let mut at = 0;
        c.step(1, &looks[at]);
        let (mut hits, mut moves) = (0, 0);
        for t in 0..4_000 {
            let motor = if rng.chance(0.4) { 0 } else { 1 };
            let guess = c.expected(&c.forecast(c.belief(), motor));
            if motor == 0 {
                at = (at + 1) % 4;
            }
            if t >= 3_000 && motor == 0 {
                moves += 1;
                hits += usize::from(guess == looks[at]);
            }
            c.step(motor, &looks[at]);
        }
        assert!(hits as f64 > 0.95 * moves as f64, "hits {hits} of {moves}");
    }
}
