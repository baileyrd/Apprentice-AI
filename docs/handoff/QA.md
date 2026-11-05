# Implementer Q&A — apprentice-ai
Harvest pack: starts empty. The expert adds a question here only when one was actually asked and answered from sources.

## Seeds
- Q (2026-11-03, card #707 stiffness): Can tuning use some of the judging seeds? — A: No. Tuning happens on a disclosed sub-range that plays no part in judging; marks pinned first, judge once (DECISIONS.md D13; docs/judging/README.md § From now on). Ranges for #707 are D32.

## Learners
- Q (2026-11-05, card #633): Does an online clone learner (discrete states, each tied to a look prototype learned by competitive learning; motor-conditioned transitions learned by a Hebbian rule on belief(t−1) × posterior(t), row-normalised; forward filtering with divisive normalisation; no backprop, no BPTT, no replay) count as local learning, so it can be #633's main-learner candidate without Q11? — A: Yes. Every weight change reads only its own pre- and post-synaptic activity (and its own row's sum for normalisation), with no error carried back through units or time; that is "local weight updates" under D2, reaffirmed by D33. Q11/D35 only arise if a non-local learner would become the main one. Becoming a default anywhere is still a D1 change.
- Q (2026-11-05, card #633): Does anything in the record say #633 must start from the GRU baseline or from e-prop variants, or limit built-in state counts? — A: No. Card #633 lists two open ideas (an innate place sense; a local rule carrying credit across units over time) and only rules out backprop as main learner (D2, D33). D8 covers the built-in time counter and labelled moves and squares, not state counts; the clone count rule is in D36. Nano chose "no innate place sense; find a learner that keeps state" (docs/facing/README.md § GRU ceiling check), which this fits.
