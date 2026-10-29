//! A peer: another apprentice, heard as just another voice.
//!
//! When its companion eats, an apprentice says one of the two foretelling
//! words about that square, picked by its own patterns: "grows back" if a
//! square eaten [`HORIZON`] steps ago is more likely food than not, "stays
//! gone" otherwise. How to speak is built in; what it says is only what it
//! has learned, so it can be wrong. The listener learns what those words mean,
//! and how far to trust them, as it does for any voice.

use crate::memory::Memory;
use crate::mentor::{GROWS_BACK, STAYS_GONE};
use crate::model::{Belief, kind};
use crate::patterns::Last;
use crate::world::Cell;

use crate::body::Word;

/// How long after a meal it asks whether food is back: long enough for a
/// bush to regrow.
pub const HORIZON: u64 = 32;

/// The word for an eaten square, given what it expects such a square to be
/// `HORIZON` steps on; `None` while it has no pattern.
pub fn word_for(expected: Option<Belief>) -> Option<Word> {
    let food = expected?[kind(Cell::Food)];
    Some(if food > 0.5 { GROWS_BACK } else { STAYS_GONE })
}

/// The word `model` says about a square its companion has just eaten from.
pub fn foretell(model: &Memory) -> Option<Word> {
    word_for(model.expects(Last::Ate, HORIZON))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::one_hot;

    #[test]
    fn quiet_without_a_pattern() {
        assert_eq!(word_for(None), None);
    }

    #[test]
    fn says_what_it_expects() {
        assert_eq!(word_for(Some(one_hot(Cell::Food))), Some(GROWS_BACK));
        assert_eq!(word_for(Some(one_hot(Cell::Empty))), Some(STAYS_GONE));
    }

    #[test]
    fn even_odds_are_not_enough_to_promise_food() {
        let even = [0.5, 0.0, 0.5, 0.0];
        assert_eq!(word_for(Some(even)), Some(STAYS_GONE));
    }
}
