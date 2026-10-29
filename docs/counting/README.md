# Counting: one piece of evidence per change, not per glance

Found in the language phase ([docs/language](../language/README.md)):
patterns learned from every sighting. A square that stays empty is seen
again and again while it stays empty, so it teaches "stays empty" many
times. A bush that grows back teaches "food again" once, and then its state
ends. So any pattern fed by both kinds of square leaned hard toward empty,
and a mentor's random words were learned as "empty" rather than as noise.

## The change

A square in one state now teaches patterns at most once per age group
(1, 2–3, 4–7, … steps old), plus once whenever it changes. A square empty
for an hour is one piece of evidence per age, not one per glance. Nothing
else changes: same delta rule, same rates, same everything else.

## Rules fixed before any run

Seeds 1–10, one run each, compared with main (PR #14) on the same seeds.
Nothing may be tuned: the change has no settings.

1. **Noise reads as noise.** With the random mentor (language test), both
   words give a square eaten 32–63 steps ago between 0.25 and 0.75 chance of
   food, in at least 8 of 10 seeds. On main: 0 of 10.
2. **Noise does no harm.** With the random mentor, median sleeps in the new
   bush world are no more than 10% above no mentor. On main: 11.5 vs 8.5.
3. **Patterns anticipate.** In the transfer test, the chance of food given
   the first regrown bush is at least 0.2 higher with patterns than without.
   Open since the home test: 0.88 vs 0.74.
4. **No regressions.** Every rule met on main still holds: the home test
   (rules 1–3), the transfer test (anticipation with patterns ≥ 0.5; bush
   sleeps with patterns no more than 10% above without), and language rule 3
   (bush sleeps with the true mentor no more than 10% above no mentor).

Also reported, not judged: the four language rules again, A/B/A savings and
mix-ups, and how long a disproven belief takes to unlearn. My prediction for
unlearning: little change, since a bush world never piled up "food again"
samples; the pile was always on the side of "empty".

## Results: not shipped

**First design, as fixed above** (seeds 1–10, the only run on them). Each
square in one state taught each age group once, plus every change.

| Rule | Result | Met? |
|---|---|---|
| 1. Random words read 0.25–0.75 in ≥ 8/10 | 9/10 | yes |
| 2. Random mentor bush sleeps ≤ +10% | 58.5 vs 76 | yes |
| 3. Anticipation ≥ 0.2 above no patterns | 0.98 vs 0.74 | yes |
| 4. Listed earlier rules hold | home 10/10, 10/10, 27/30; others hold relative to their baselines | yes |

Every rule is met, and the apprentice is far worse. Bush-world sleeps with
no mentor went from 8.5 to 76, fresh-world sleeps in the transfer test from
3.5 to 58.5, and unlearning a disproven belief from 108 to 413 steps. My
rule 4 checked rules, not the baselines under them, and the baselines
collapsed. The cause: one sample per age group has no resolution inside a
group. A bush regrows at age 31, inside the 16–31 group, so that group read
half food where it had read 0.12, and the apprentice went back to bushes
too early, over and over. Counting every glance had been weighting each
age group by how long the square spent looking that way, and that was
doing real work.

**Second design** (tuning seeds 101–110 only). Keep counting every glance,
but after a square changes by itself, keep teaching the state the change
ended from what the square looks like now. A bush seen an hour after it
regrew was also food an hour after it was eaten. Fresh-world sleeps in the
transfer test went from 3 to 27. A handful of spurious changes (a square
seen as floor, then wall, around a world switch) then taught "floor becomes
wall" on every later glance of that wall, forever, so the planner stopped
trusting floor it hadn't seen lately. Limited to eaten squares, it no
longer hurt (5 sleeps vs 3), but random words still read near 0.

## What it shows

- **The lean toward "empty" was mostly recency, not counting.** The same
  random words, read after an upbringing that ends in a bush world rather
  than a barren one, came out anywhere from 0.02 to 0.83, tracking the
  general pattern (tuning seeds, on main). A delta rule at rate 0.05 forgets
  everything older than a few dozen samples, so a pattern says mostly what
  the last world taught. The language write-up blamed counting; this
  corrects it.
- **Counting each glance is closer to right than it looked.** It weights by
  time spent looking a given way, which is what the planner needs. The real
  gap is censoring: once a square changes, nothing more is learned about the
  state that ended. Fixing that safely needs a way to tell a real change
  from a mistaken one.
- **The fix for noise is evidence, not counting.** A word whose outcomes are
  a coin toss should settle near 0.5. That needs a learning rate that
  shrinks with evidence (rate 1/n), so a pattern averages over everything it
  has seen. That is the "stubbornness" idea from the unlearning test, and it
  would slow adaptation, which is the success measure.

No code changes; the rules above were committed before the first run.
