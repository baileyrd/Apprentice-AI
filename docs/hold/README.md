# Hold: keeping a fact across steps (card #633)

Status: approach approved by Nano (2026-11-05). Marks below were pinned
before any run (D13); design rows are D36–D39. The learning rule changed after
tuning showed the approved one fails (see § Changes before judging); that
change awaits Nano's OK. Nothing has run on the judging seeds.

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
  the nearest prototype moves toward the percept. Local. (As built: recruited
  by a distance threshold, uncapped; see § Changes before judging.)
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

## Changes before judging, disclosed

Everything in this section ran on tuning seeds 201–210 only.

**The approved rule fails.** A spike (Python, discrete views, not committed)
of the rule above, forward credit only, gave matching 0.77 median one-ahead
(0.76–0.84 over prior and clone settings). Cause: forward credit can't split
clones. Once one clone has taken both north and south, every later step
reinforces the mix, and nothing that comes after can undo it.

**Fixed-lag smoothing works but isn't local.** Passing credit back through the
transition weights over the last L steps gave matching 0.99 (L = 4–10) and
walking 0.91–0.92 (L = 10, 100,000 steps). That is credit flowing through
other units over later steps, so it is not used (QA.md § Learners; using it
would be Q11, Nano's call).

**View-change credit (proposed instead, ruled local):**

- A step that keeps the view is learned at once, as before.
- A step that changes the view leaves a tag on each synapse `i → j` into the
  new view's clones: belief in `i` before × `T[m][i][j]`.
- When the view next changes, each clone `j` scores its own forecast of that
  change from its own outgoing weights (`Σ_k T[m'][j][k]` over the clones `k`
  of the view it then sees). The tag becomes a count change in proportion,
  normalised.
- This assumes a clone persists while the view doesn't change. It is a
  synapse-local tag times a unit-local signal, the same three-factor shape as
  e-prop. No signal passes through other units.
- Spike: matching 1.00 on 10/10 seeds; walking 0.97–0.99 at 400,000 steps.

**Also needed and disclosed (D39):**

- **Persistence prior.** Counts start at `persist` on every clone's own
  transition, for every motor (no motor is labelled still), plus random counts
  up to `jitter` to break symmetry. Without the prior, matching stays at 0.81.
- **Views.** A view is (look prototype, active ring unit). Looks are
  recruited when no prototype is within squared distance `recruit`; otherwise
  the nearest moves to the running mean of what it has matched. The ring is
  noise-free one-hot, so its active unit is read exactly. There is no cap on
  looks.

**Tuning (Rust, `hold`, seeds 201–210; 400,000 walking steps).** Seeds
passing, then median one-ahead:

| Setting changed | Matching | Walking |
|---|---|---|
| none (clones 16, recruit 4, persist 1, jitter 0.001) | 10/10, 1.00 | 10/10, 0.99 |
| clones 8 | 10/10, 1.00 | 10/10, 0.99 |
| clones 32 | 10/10, 1.00 | 9/10, 0.97 |
| recruit 3 | 8/10, 1.00 | 0/10, 0.82 |
| recruit 5 | 10/10, 1.00 | 10/10, 0.99 |
| recruit 6 | 10/10, 1.00 | 10/10, 0.99 |
| persist 0.3 | 10/10, 1.00 | 8/10, 0.98 |
| persist 3 | 10/10, 1.00 | 10/10, 0.98 |
| jitter 0.0001 | 10/10, 1.00 | 10/10, 0.99 |
| jitter 0.01 | 10/10, 1.00 | 0/10, 0.94 |

Turning (8,000 steps) passes 10/10 at the starting settings.

**Picked: clones 16, recruit 5, persist 1, jitter 0.001.** Recruit 5 is the
middle of the passing range 4–6. Below that range, noisy glances recruit
spurious looks (recruit 3 collapses walking). Fragile spots, stated before
judging:

- Jitter must stay small. At 0.01, early random counts outweigh learning and
  walking fails.
- A weak prior (0.3) loses 2 of 10 walking seeds.

Walking: clones' centre-direction probe 0.98 (GRU 0.90 in docs/facing).

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

Tune on 201–210 (done, above); smoke on 101–102; judge once on 701–900:
`cargo run --release --bin hold -- --stage matching|walking|turning --judge
--from 701 --seeds 200`, plus `facing --stage walking --gru --from 701
--seeds 200` for the ceiling. Afterwards 701–900 are spent; next
fresh 901–1000, then 1401+.

## Layout (D38)

`src/clones.rs`, plugged into `src/facing.rs` like `EpropLearner`; bin
`cargo run --release --bin hold -- --from F --seeds N`. Existing facing
outputs stay byte-identical (D30). Off everywhere else; using it in the grid
apprentice would be a D1 change.
