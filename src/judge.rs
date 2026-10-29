//! Judging results across seeds: paired typical changes, shares and levels,
//! each with a 90% interval and a three-way verdict.
//!
//! A rule's mark is met only when the whole interval clears it, missed when
//! the whole interval falls short, and unclear otherwise. Pure; no I/O.

use std::fmt;

/// Standard normal quantile for a two-sided 90% interval.
const Z90: f64 = 1.645;

/// Why a set of per-seed values cannot be judged.
#[derive(Debug, Clone, PartialEq)]
pub enum JudgeError {
    /// Fewer than two seeds: no spread to judge from.
    TooFew(usize),
    /// Baseline and change cover different numbers of seeds.
    Unpaired { base: usize, test: usize },
    /// A count below zero or not a number.
    NotACount(f64),
}

impl fmt::Display for JudgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFew(n) => write!(f, "{n} seeds; need at least 2"),
            Self::Unpaired { base, test } => {
                write!(f, "{base} baseline seeds against {test} changed ones")
            }
            Self::NotACount(x) => write!(f, "{x} is not a count"),
        }
    }
}

impl std::error::Error for JudgeError {}

/// Whether a mark is met.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Met,
    Missed,
    Unclear,
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Met => "met",
            Self::Missed => "missed",
            Self::Unclear => "unclear",
        })
    }
}

/// An estimate and its 90% interval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interval {
    pub lo: f64,
    pub mid: f64,
    pub hi: f64,
}

impl Interval {
    /// Typical change from `base` to `test`, paired seed by seed: the
    /// geometric mean of `(test + 1) / (base + 1)`, minus 1.
    pub fn change(base: &[f64], test: &[f64]) -> Result<Self, JudgeError> {
        if base.len() != test.len() {
            return Err(JudgeError::Unpaired {
                base: base.len(),
                test: test.len(),
            });
        }
        let logs = base
            .iter()
            .zip(test)
            .map(|(&b, &t)| Ok(((count(t)? + 1.0) / (count(b)? + 1.0)).ln()))
            .collect::<Result<Vec<f64>, JudgeError>>()?;
        let (mean, se) = mean_and_error(&logs)?;
        let back = |x: f64| x.exp() - 1.0;
        Ok(Self {
            lo: back(mean - Z90 * se),
            mid: back(mean),
            hi: back(mean + Z90 * se),
        })
    }

    /// Share of `hits` in `n` seeds, with a Wilson interval.
    pub fn share(hits: usize, n: usize) -> Result<Self, JudgeError> {
        if n < 2 {
            return Err(JudgeError::TooFew(n));
        }
        let (n, p) = (n as f64, hits.min(n) as f64 / n as f64);
        let z2 = Z90 * Z90;
        let centre = (p + z2 / (2.0 * n)) / (1.0 + z2 / n);
        let half = Z90 / (1.0 + z2 / n) * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt();
        Ok(Self {
            lo: (centre - half).max(0.0),
            mid: p,
            hi: (centre + half).min(1.0),
        })
    }

    /// Median of `xs`, with the distribution-free interval between the
    /// order statistics that cover it 90% of the time.
    pub fn median(xs: &[f64]) -> Result<Self, JudgeError> {
        let n = xs.len();
        if n < 2 {
            return Err(JudgeError::TooFew(n));
        }
        let mut xs = xs.to_vec();
        xs.sort_by(f64::total_cmp);
        let mid = if n % 2 == 1 {
            xs[n / 2]
        } else {
            (xs[n / 2 - 1] + xs[n / 2]) / 2.0
        };
        let nf = n as f64;
        let k = ((nf - Z90 * nf.sqrt()) / 2.0).floor().max(0.0) as usize;
        Ok(Self {
            lo: xs[k],
            mid,
            hi: xs[n - 1 - k],
        })
    }

    /// Whether the estimate is at most `mark`.
    pub fn at_most(&self, mark: f64) -> Verdict {
        match () {
            _ if self.hi <= mark => Verdict::Met,
            _ if self.lo > mark => Verdict::Missed,
            _ => Verdict::Unclear,
        }
    }

    /// Whether the estimate is at least `mark`.
    pub fn at_least(&self, mark: f64) -> Verdict {
        match () {
            _ if self.lo >= mark => Verdict::Met,
            _ if self.hi < mark => Verdict::Missed,
            _ => Verdict::Unclear,
        }
    }

    /// Width of the interval, high end minus low end.
    pub fn width(&self) -> f64 {
        self.hi - self.lo
    }
}

fn count(x: f64) -> Result<f64, JudgeError> {
    if x.is_nan() || x < 0.0 {
        return Err(JudgeError::NotACount(x));
    }
    Ok(x)
}

/// Mean and standard error of the mean.
fn mean_and_error(xs: &[f64]) -> Result<(f64, f64), JudgeError> {
    let n = xs.len();
    if n < 2 {
        return Err(JudgeError::TooFew(n));
    }
    let nf = n as f64;
    let mean = xs.iter().sum::<f64>() / nf;
    let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (nf - 1.0);
    Ok((mean, (var / nf).sqrt()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halving_every_seed_is_a_sure_half() {
        let base = [9.0, 19.0, 39.0];
        let test = [4.0, 9.0, 19.0];
        let i = Interval::change(&base, &test).unwrap();
        assert!((i.mid + 0.5).abs() < 1e-12);
        assert!(i.width() < 1e-12);
        assert_eq!(i.at_most(-0.25), Verdict::Met);
        assert_eq!(i.at_most(-0.6), Verdict::Missed);
    }

    #[test]
    fn disagreeing_seeds_are_unclear() {
        let base = [10.0, 10.0, 10.0, 10.0];
        let test = [2.0, 40.0, 3.0, 30.0];
        let i = Interval::change(&base, &test).unwrap();
        assert!(i.lo < -0.15 && i.hi > 0.0);
        assert_eq!(i.at_most(-0.15), Verdict::Unclear);
    }

    #[test]
    fn zeros_compare_through_the_plus_one() {
        let i = Interval::change(&[0.0, 0.0], &[1.0, 1.0]).unwrap();
        assert!((i.mid - 1.0).abs() < 1e-12);
    }

    #[test]
    fn change_rejects_bad_input() {
        assert_eq!(Interval::change(&[1.0], &[1.0]), Err(JudgeError::TooFew(1)));
        assert_eq!(
            Interval::change(&[1.0, 2.0], &[1.0]),
            Err(JudgeError::Unpaired { base: 2, test: 1 })
        );
        assert_eq!(
            Interval::change(&[1.0, -1.0], &[1.0, 1.0]),
            Err(JudgeError::NotACount(-1.0))
        );
    }

    #[test]
    fn share_interval_holds_the_share_and_stays_in_bounds() {
        let all = Interval::share(10, 10).unwrap();
        assert_eq!(all.mid, 1.0);
        assert_eq!(all.hi, 1.0);
        assert!(all.lo > 0.7 && all.lo < 0.9);
        let some = Interval::share(70, 100).unwrap();
        assert!(some.lo < 0.7 && some.hi > 0.7);
        assert_eq!(some.at_least(0.9), Verdict::Missed);
        assert_eq!(some.at_least(0.5), Verdict::Met);
    }

    #[test]
    fn median_interval_brackets_the_median() {
        let xs: Vec<f64> = (0..100).map(f64::from).collect();
        let m = Interval::median(&xs).unwrap();
        assert_eq!(m.mid, 49.5);
        assert!(m.lo < 49.5 && m.hi > 49.5);
        assert!(m.lo >= 39.0 && m.hi <= 60.0);
        assert_eq!(m.at_most(70.0), Verdict::Met);
        assert_eq!(m.at_most(30.0), Verdict::Missed);
        assert_eq!(Interval::median(&[1.0]), Err(JudgeError::TooFew(1)));
    }
}
