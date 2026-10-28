//! Deterministic, serialisable PRNG.
//!
//! SplitMix64 rather than the `rand` crate: its whole state is one `u64`, so
//! checkpoints restore the exact stream and runs replay bit-for-bit.

use serde::{Deserialize, Serialize};

/// SplitMix64 generator. Not cryptographic; plenty for a grid world.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Creates a generator from a seed.
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Next raw 64-bit value.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform integer in `0..n`. `n` must be non-zero.
    pub fn below(&mut self, n: usize) -> usize {
        debug_assert!(n > 0, "below(0) is meaningless");
        (self.next_u64() % n as u64) as usize
    }

    /// True with probability `p` (clamped to `0.0..=1.0`).
    pub fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }

    /// Uniform float in `0.0..1.0`.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Picks one item uniformly, or `None` when empty.
    pub fn pick<T: Copy>(&mut self, items: &[T]) -> Option<T> {
        if items.is_empty() {
            return None;
        }
        Some(items[self.below(items.len())])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let (mut a, mut b) = (Rng::new(7), Rng::new(7));
        assert!((0..100).all(|_| a.next_u64() == b.next_u64()));
    }

    #[test]
    fn below_stays_in_range() {
        let mut r = Rng::new(1);
        assert!((0..1000).all(|_| r.below(5) < 5));
    }

    #[test]
    fn chance_extremes() {
        let mut r = Rng::new(3);
        assert!((0..100).all(|_| !r.chance(0.0)));
        assert!((0..100).all(|_| r.chance(1.0)));
    }

    #[test]
    fn pick_empty_is_none() {
        let empty: [u8; 0] = [];
        assert_eq!(Rng::new(0).pick(&empty), None);
    }
}
