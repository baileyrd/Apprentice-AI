# Hold: keeping a fact across steps (card #633)

Status: **proposed, awaiting Nano's OK. Nothing has been run.** Marks below
are pinned before any run (D13); design rows are D36–D38.

## The problem

The facing track (docs/facing) ended at one wall: when the world needs a fact
the current percept doesn't carry (heading in matching, place in walking),
every online local learner stops at 0.75–0.87 one-ahead, while a GRU trained
offline by BPTT reaches 1.00. What the local learners lack is a state they keep
and rewrite with each move.

## The learner: clones (local, online)

A small learned map of hidden states, in the spirit of clone-structured
cognitive graphs (a hidden Markov model learned online).

- **Looks.** A fixed set of look prototypes learned by competitive learning:
  the nearest prototype moves toward the percept. Local.
- **Clones.** Each look gets the same fixed number of hidden states (clones),
  generous and disclosed, never set to the world's true count (D7, D36).
- **Belief.** A distribution over clones, carried from step to step by the
  fired motor's transition weights `T[m]`, then sharpened by how well each
  clone's look matches what it sees. Normalising across units is lateral
  inhibition.
- **Learning.** `T[m][i][j]` grows by belief in `i` before the step × belief
  in `j` after it (Hebbian), each row normalised by its own sum. No error is
  sent back through units or time; one online pass, no replay.
- **Prediction.** Next percept = (belief moved by `T[m]`) × look prototypes.

Why it might work where e-prop didn't: the fact is held explicitly as which
clone is believed, and moves rewrite it through learned transitions. Two
clones of one dark look can stand for "dark, centre behind me" and "dark,
centre to my left". Ruled local by the expert (QA.md § Learners); not a
non-local learner, so Q11 is untouched.

## Worlds

From `src/facing.rs`, unchanged:

1. **Matching without the ring**, 20,000 steps: heading must be held.
2. **Walking + ring**, 400,000 steps: place must be held.
3. **Turning**, 8,000 steps: sanity row.

## Marks (pinned)

Per world, on judging seeds 701–900 (200 seeds), one run:

1. **Absolute.** A seed passes if one-ahead ≥ 95%, two-ahead ≥ 90% and after
   stares ≥ 5 ≥ 90% (scoring as docs/facing). Share of passing seeds
   `Interval::share(..).at_least(0.95)`: met only if the whole 90% interval is
   at or above 0.95 (about 196/200 or better).
2. **Paired.** Wrong one-ahead predictions over the last 100 scored steps,
   clones vs the best earlier local learner on the same seed (gated context
   for matching, gated e-prop for walking). `Interval::change(..).at_most(-0.5)`:
   met if the whole interval is at or below −50%. Raw difference reported
   beside the %.
3. **Leak check.** No-memory control on matching: median one-ahead ≤ 80%,
   else stop and find the leak.

Reading: #633 is met if marks 1 and 2 are met in both worlds 1 and 2. Any
miss or unclear is reported as such; if clones miss, Q11 goes to Nano (D35).

Reported, not judged: turning row, GRU+BPTT ceiling, heading and
centre-direction probes read off the belief (frozen in the second half).

## Seeds (D37)

Tune (clones per look, rates) on 201–210, disclosed here before judging;
smoke on 101–102; judge once on 701–900. Afterwards 701–900 are spent; next
fresh 901–1000, then 1401+.

## Layout (D38)

`src/clones.rs`, plugged into `src/facing.rs` like `EpropLearner`; bin
`cargo run --release --bin hold -- --from F --seeds N`. Existing facing
outputs stay byte-identical (D30). Off everywhere else; using it in the grid
apprentice would be a D1 change.
