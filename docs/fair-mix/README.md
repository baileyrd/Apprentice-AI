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

## Results (seeds 1–100 and 111–210, one run)

`cargo run --release --bin fair_mix -- --judge` prints the verdicts
([judging.txt](judging.txt)) and writes [fair_mix.csv](fair_mix.csv).
About 3.5 hours on 4 cores.

| Rule | Result | Verdict |
|---|---|---|
| 1a. Bush sleeps, H vs A ≤ +10% | +46% [+33%, +59%] | **missed** |
| 1b. Barren sleeps, H vs A ≤ +10% | −18% [−21%, −16%] | **met** |
| 1c. Barren false hopes, H vs A ≤ +10% | +72% [+47%, +101%] | **missed** |
| 1d. Home-test wrong (100 steps), H vs A ≤ +10% | −20% [−24%, −16%] | **met** |
| 2. Barren false hopes, H vs C ≤ −50% | −56% [−65%, −44%] | **unclear** |
| 3a. Bush sleeps, K vs A ≤ +10% | +126% [+103%, +151%] | **missed** |
| 3b. Barren sleeps, K vs A ≤ +10% | −0% [−2%, +2%] | **met** |
| 3c. Barren false hopes, K vs A ≤ +10% | −7% [−19%, +6%] | **met** |
| 3d. Home-test wrong (100 steps), K vs A ≤ +10% | −0% [−6%, +6%] | **met** |
| 4. Patterns taught in the bush world, K vs A ≤ −50% | −54% [−55%, −53%] | **met** |

Means (A, C, H, E, K): bush sleeps 14.2, 12.8, 19.6, 19.4, 36.2; barren
false hopes 1.7, 17.0, 4.0, 66.3, 2.0; barren sleeps 56.3, 25.9, 46.9,
28.7, 56.6; barren meals 32.5, 104.3, 58.7, 97.1, 37.4; bush meals 279,
282, 268, 267, 234; diary writes in the bush world 36,000, 6,344, 4,590.

## What it shows

- **The lead holds for false hopes.** A fair mix removes nearly all of
  the budget's false hopes (66 → 2.0, default 1.7) and most of the gate's
  (17 → 4.0). The network expects eaten squares to grow back when its
  lessons tilt toward changes, by amount (a budget of surprises) or by
  order (surprises taught last).
- **But the tilt was also feeding it.** Today's gate has *fewer* bush
  sleeps than the default (12.8 against 14.2), and fair mix takes that
  away (19.6). A network that leans toward "it grows back" returns to
  bushes sooner, which pays in a bush world and costs in a barren one.
  Fair mix trades one for the other; it does not remove a mistake.
- **The gate's odd barren life was mostly an age bug.** Confirmations
  taught at their age group's start, not their own age, gave the gated
  apprentice half the barren sleeps and three times the barren meals. With
  their own age it lives close to the default there (sleeps −18%, meals
  59 against 33), and keeps the gate's better home test (−20%).
- **A budget starves the network of regrowth.** With a fair split, a
  budget loses nothing in the barren world or the home test but more than
  doubles bush sleeps (36 against 14). Fewer samples mean fewer regrowths
  seen; tuning showed the share of changes is not the fix (half and half
  did no better). The network learns at a fixed rate per sample, so
  teaching less means learning slower.

Neither the gate nor a network budget can go on yet. Defaults unchanged:
`fair_mix` stays `false`. Kept regardless: the gate's confirmations are
now kept in the order first seen, so gate runs are reproducible.

What would let the gate go on is a network that learns "grows back" from
regrowths without being told it more often than it happens: for example
weighting each sample by how rare its outcome is, or a rate that rises
with surprise. Both change how the network learns, not what sleep feeds
it, and are not tried here.
