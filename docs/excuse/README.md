# A narrower regrowth excuse

Card #565 (Mill Board, project apprentice_ai). Diagnosis only: no default
changes here.

## Why

PR #11 judges a square the apprentice ate from by when patterns say it
grows back, not by the map: an expected change is no sign of being
elsewhere. PR #34 (docs/home-extras, exploratory, seeds 241–300) found
this is the likely door for strangers taken as home: in a stranger, a
square it ate from at home is mostly empty, reads as "not grown back
yet", and scores low error, so no doubt builds. Turning the excuse off in
trials took strangers taken as home from 23 to 11 of 180, at a cost of 2
of 120 home trials.

## What changes

One new `Recall` field, `excuse`, with three values:

- **Always** (default, today's behaviour): an eaten square is judged by
  patterns' regrowth whenever patterns have a guess.
- **Never** (arm X): no regrowth-judged fit; every square is judged by
  the map.
- **WhileFit** (arm N): each map also keeps a running error over the
  sightings it judged by the map alone (same rate, `fit_rate`). An eaten
  square is excused only while that error is at most `misfit` (0.3), that
  is, while the rest of the map fits. Otherwise it is judged by the map.

The rule is local to each map and uses only what the apprentice senses.
It is never told which world it is in. No new number: the threshold is
the existing `misfit`, so there is no tuning and no tuning seeds.

## Plan, fixed before running

**Arms**: A (main's default, `Always`), X (`Never`), N (`WhileFit`). All
else is main's default (revisit 1/128, patterns network).

**Seeds**: 501–700 (200 fresh seeds), each arm lived once per seed in
`apprentice::life` (upbringing, bush and barren tests, home test). Run
once. Seeds 1–500 are used and are not touched.

**Measures**, per seed:

- Home test, answers read at 100 steps (`ASKED`), five trials:
  strangers taken as home (0–3), home or changed home taken as a
  stranger (0–2), total wrong (0–5).
- Life: bush test sleeps, barren test sleeps per 1,000 steps, bush
  upbringing sleeps, barren upbringing sleeps.

All comparisons are paired seed by seed with a 90% interval
(`apprentice::judge`). Home-test numbers are judged on the raw
difference per test; life numbers on the typical % change, raw
difference reported next to it.

**Marks** (met: whole interval clears the mark; missed: whole interval
falls short; unclear otherwise):

0. **The door reproduces on main** (precondition, not a mark on N):
   X − A, strangers taken as home, raw, at most 0 (whole interval below
   0). If this is not met, the gap N is meant to close is not
   established on main, and mark 1 is reported but not read as closing
   it.
1. **N closes at least half the gap**: per seed,
   (N − A) − ½ (X − A) in strangers taken as home; its mean at most 0.
2. **N keeps home**: N − A, home or changed home taken as a stranger,
   raw, at most +0.10 per test.
3. **Home-test hard limit** (Nano's rule, binding): N − A, total wrong,
   raw, upper end of the 90% interval at most +0.25 per test.
4. **No regressions**, N vs A, each typical change at most +10%: bush
   test sleeps, barren test sleeps per 1,000 steps, bush upbringing
   sleeps, barren upbringing sleeps.

**Reading**: N works if marks 0, 1 and 3 are met and no mark in 2 or 4
is missed. Whatever the result, nothing becomes a default without
Nano's OK, and a default flip goes in its own PR.

**Reported only, never a mark**: every measure for N vs X and X vs A,
and the arm means.

**Guess before running**: mark 1 missed or unclear. PR #34's probe found
that in stranger trials taken as home, the home map's error on squares it
judged by the map was 0.23, below `misfit`, so the rest of the map
already "fits" there and N will usually still grant the excuse. Marks 0,
3 and 4 met.

## How to run

```sh
cargo run --release --bin excuse            # seeds 501-700, all three arms
```

Writes `runs/excuse/excuse.csv` (one row per seed and arm) and prints the
marks.

A smoke and timing run on seeds 1–4 (already used seeds, not judging)
was made before the judging run, to check the bin; nothing was tuned.

## Results (seeds 501–700, one run)

Full output: [judging.txt](judging.txt); per seed and arm:
[excuse.csv](excuse.csv).

**Means per home test (answers at 100 steps)**

| Arm | Strangers taken as home (0–3) | Homes taken as stranger (0–2) | Total wrong (0–5) |
|---|---|---|---|
| A (default) | 0.595 | 0.000 | 0.595 |
| X (excuse off) | 0.295 | 0.105 | 0.400 |
| N (narrower) | 0.440 | 0.010 | 0.450 |

**Marks** (raw paired difference, 90% interval; % change next to it)

| Mark | Result | Verdict |
|---|---|---|
| 0. Door reproduces: X − A strangers | −0.300 [−0.382, −0.218] (−16%) | **met** |
| 1. N closes half the gap: (N−A) − ½(X−A) | −0.005 [−0.047, +0.037] | **unclear** |
| 2. N keeps home: N − A homes as stranger | +0.010 [−0.002, +0.022] (+1%) | **met** |
| 3. Hard limit: N − A total wrong ≤ +0.25 | −0.145 [−0.194, −0.096] (−8%) | **met** |
| 4. Bush test sleeps | +0% [−2%, +2%], raw −0.02 | met |
| 4. Barren test sleeps/1k | +0% [−3%, +3%], raw −0.06 | met |
| 4. Bush upbringing sleeps | −0% [−1%, +1%], raw −0.05 | met |
| 4. Barren upbringing sleeps | −0% [−1%, +1%], raw −0.04 | met |

**Reading, as pinned**: marks 0 and 3 met, 2 and 4 met, mark 1
unclear, so N does not pass the pinned reading. It closes about half
the gap (N − A strangers −0.155 [−0.202, −0.108] against the gap of
−0.300), right on the line, and 200 seeds cannot tell which side.

**Reported only**

- N vs A: strangers taken as home −0.155 [−0.202, −0.108]; total wrong
  −0.145 [−0.194, −0.096]. N is better than today's default on the home
  test, with no detectable cost to sleeps.
- X vs A: X removes twice as many stranger errors (−0.300) but costs
  homes: +0.105 [+0.062, +0.148] homes taken as stranger per test, where
  A and N almost never miss home. Total wrong −0.195 [−0.287, −0.103].
- N vs X: N keeps home better (−0.095 [−0.140, −0.050]) and takes more
  strangers as home (+0.145 [+0.076, +0.214]); total wrong +0.050
  [−0.029, +0.129], no detectable difference.
- Neither N nor X changes life sleeps by more than 3% either way.

**The guess was wrong.** I expected N to barely differ from A, because
the PR #34 probe found the home map's error on map-judged squares in
wrong stranger trials was 0.23, under `misfit`. On fresh seeds N removes
about half of the stranger errors that X removes. A possible reason,
not checked: the probe's 0.23 was a mean over whole trials on another
commit, and a running error that crosses 0.3 for part of a trial is
enough to withdraw the excuse then.

## Decision (Nano, 2026-10-31): excuse off by default

First decided: the narrower excuse (`WhileFit`) becomes the default. Mark
1 stayed unclear (0.440 against the 0.445 target, interval straddles),
but N beat the old default on total wrong (−0.145 [−0.194, −0.096]) with
homes missed at 0.010 and no change in sleeps; mark 1 measured ambition,
not safety.

Then Nano's rule on the error-cost check: if N's missed homes are bad in
every arm, it is the world; if only under N, excuse-off becomes the
default. Only N was bad (seed 571: 47 sleeps under N, 0 under A and X;
see the error-cost section), so `Excuse::Never` is the default from the PR that flips it
(its own PR; revert it alone to undo). Against the old default, X has
total wrong −0.195 [−0.287, −0.103], homes missed +0.105, and no
detectable change in sleeps.

## What PR #36 did not change

At PR #36, `Excuse::Always` stayed the default. Nothing becomes a default without
Nano's OK, and a flip would go in its own PR. Choosing between N and X
is a trade: X gets fewer wrong answers overall by a margin too small to
detect here (0.05 per test), and it pays for that with home trials
answered wrong. N keeps home almost as well as today.

## What each error costs (reported only)

Asked by Nano when the narrower excuse became the default. For every
home-test trial on seeds 501–700, the trial runs on for 1,000 steps
after the answer is read at step 100, counting sleeps and meals. A wrong
answer is compared with right answers in the same kind of place (home or
changed home; stranger). Each trial is the same as in the judging run up
to step 100: the counts of wrong answers match it exactly. No interval
is computed; this is a description, not a mark.

`cargo run --release --example error_cost -- 501 700` →
[error_cost.csv](error_cost.csv).

| Arm | Error | Trials wrong | Sleeps/1k, wrong − right | Meals/1k, wrong − right |
|---|---|---|---|---|
| A | stranger taken as home | 119 | −0.11 (1.34 vs 1.46) | +0.15 (75.5 vs 75.4) |
| X | stranger taken as home | 59 | +0.44 (1.86 vs 1.42) | −1.65 (73.9 vs 75.5) |
| N | stranger taken as home | 88 | −0.46 (1.03 vs 1.49) | +0.93 (76.2 vs 75.3) |
| A | home taken as stranger | 0 | — | — |
| X | home taken as stranger | 21 | +0.05 (0.81 vs 0.75) | +1.43 (77.3 vs 75.9) |
| N | home taken as stranger | 2 | +22.8 (23.5 vs 0.67) | −32.1 (44.0 vs 76.1) |

Reading: a stranger taken as home costs nothing visible in sleeps or
meals over the next 1,000 steps, in any arm (differences under half a
sleep). A home taken as a stranger also costs nothing under X (21
trials). N's two missed homes were costly (23.5 sleeps and 44 meals per
1,000 steps, against 0.7 and 76), but two trials are anecdotes: why
they went badly is not checked.

### The same missed-home trials in every arm

Nano asked whether N's missed homes are bad because of the world. The
probe now also lists every home or changed-home trial some arm answered
wrong, with all three arms on the same seed and world (second block of
[error_cost.csv](error_cost.csv); summary rows unchanged on rerun).

| Seed, place | A sleeps (meals) | X sleeps (meals) | N sleeps (meals) |
|---|---|---|---|
| 571, changed home (N wrong) | 0 (79) | 0 (77) | **47 (11)** |
| 586, changed home (N wrong) | 0 (76) | 0 (78) | 0 (77) |

Only one of N's two missed homes was bad, and it was bad only under N:
the same world cost the other arms nothing. The 23.5 mean above is that
one trial (47 sleeps) averaged with a free one. Of X's 21 missed homes,
the only costly one (seed 684) was about as costly in every arm (4–7
sleeps), so that is the world. Why seed 571 went so badly under N is not
checked; it is one trial.
