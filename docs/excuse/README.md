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

## Results

Not yet run.
