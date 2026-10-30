# Fair mix: what sleep teaches the patterns, and in what order

PR #23 found that the patterns' network, taught in sleep only the day's
most "surprising" sightings, comes to expect eaten squares to grow back
where they never do: barren false hopes rose from 1.7 to 66 per barren
world. The memory gate (PR #19) has the same shape of harm (false hopes
1.7 → 19.6 with the network patterns). The lead: in both, the network
learns mostly from surprises, and surprises in these worlds are mostly
changes. A network fed mostly on changes learns that things change.

The two cases differ in how they tilt the mix:

- **The gate tilts the order, not the amount.** Every sighting still
  teaches the patterns: the ones it writes into the diary as entries, the
  rest as confirmations counted by the pattern they confirm. But in sleep
  all confirmations are taught first and the day's diary last, so the
  network (rate 0.2, one set of weights) ends each sleep on a run of
  surprises. Its last lessons are the ones it remembers best.
- **A budget tilts the amount.** It keeps the 144 most "surprising"
  entries of the day, nearly all of them changes, and drops the rest.

This slice tests whether keeping the mix fair removes the harm.

## What changes

`Recall::fair_mix: bool` (default `false`: today's behaviour). With it on:

- **Order.** A sleep's samples are spread evenly through the sleep: each
  confirmation's copies and the day's diary samples are interleaved at
  even spacing, instead of all confirmations first. The diary keeps its
  own order.
- **Amount**, with a network budget (`net_replay`): the budget is split
  between changes (what was seen differs from how the square last looked)
  and non-changes in the day's own proportions, and within each the most
  "surprising" are kept. Rehearsed older samples are spread through the
  sleep the same way.

With the gate off and no budget, sleep already teaches every sighting in
the order seen, so fair mix changes nothing there.

One fix that is not part of the test: the gate's confirmations were kept
in a hash map whose order changes from run to run, so gate runs with the
network were not reproducible. They are now kept in the order first seen,
with or without fair mix.

## Test

The life of PRs #19–#23 (`apprentice::life`). Five settings per seed:

- **A**: today's default: gate off, network taught everything.
- **C**: gate on (threshold 0.05, 18 squares), today's order.
- **H**: C with fair mix.
- **E**: network budget 144 with 64 rehearsed (PR #23), today's choice.
- **K**: E with fair mix.

## Rules fixed before any run

Fair-mix details (spacing, how a budget is split) tuned on seeds 101–110
only. Judged once on seeds 1–100 and 111–210, each comparison paired seed
by seed, read from the 90% interval as met, missed or unclear with
`apprentice::judge` (PR #18).

1. **Fair mix lets the gate go on.** H against A, each at most +10%: bush
   sleeps, barren sleeps, barren false hopes, home-test wrong answers after
   100 steps.
2. **The order was the gate's harm.** Barren false hopes, H against C: at
   most −50%.
3. **A fair budget loses nothing.** K against A, each at most +10%: the
   same four as rule 1.
4. **And still saves work.** Patterns taught in the bush world, K against
   A: at most −50%.

Reported but not judged: diary writes, upbringing sleeps, meals, futile
returns.

My guess before running: rule 4 met by construction. Rule 2 met: the
order should matter to a network at rate 0.2. Rule 1 unclear: PR #19's
gate cost bush sleeps (+24%) with the table, before the network, so order
may not be all of it. Rule 3 unclear: proportions fix the base rate of
change, but the most "surprising" non-changes may still be odd ones.

## Tuning (seeds 101–110)

As first built (spread evenly, budget split in proportion), fair mix did
what the lead predicted for false hopes: with the gate 26.6 → 2.1 per
barren world, with a budget 45.4 → 0.0 (today's default: 0.6). But bush
sleeps rose (gate 10.2 → 15.3, budget 13.0 → 20.6; default 7.0), and the
gate kept its odd barren shape: half the sleeps, three times the meals.

- **Confirmations at their own age.** The gate counts a confirmation by
  its age group's start (age 37 is taught as 32), while diary entries are
  taught at their own age. A table keyed by age group never noticed; the
  network takes age as an input and does. Teaching confirmations at their
  own age, with fair mix only, brought the gated apprentice close to the
  default in the barren world (sleeps 25.0 → 50.8, default 56.2; meals 107
  → 52, default 36) and cut bush sleeps to 12.3, though false hopes rose
  to 5.3. **Kept**: it makes fair mix mean "taught as if every sighting
  were in the diary". This goes beyond "spacing and how a budget is
  split" named in the rules above; the rules themselves are unchanged.
- **A budget split half and half** between changes and non-changes,
  instead of in proportion: bush sleeps 20.6 → 22.4, false hopes 0.0 →
  0.2. No better; the bush cost of a budget is not the share of changes
  but how few samples the network sees. **Kept: in proportion.**

Before judging, rule 2 looked likely, rule 1 likely to miss on bush
sleeps, rule 3 likely to miss on bush sleeps, rule 4 met.
