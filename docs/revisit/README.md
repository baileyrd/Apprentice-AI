# Revisit: a planner-side pull toward long-unseen squares

## Why

The decline diagnosis (docs/decline) found that a newborn beats an
experienced apprentice in a new barren world by making three times as many
hopeful trips, and that those trips pay mostly on the way (en-route meals
are 47% of the meal gap, meals at the hoped-for square 11%). Experience
takes the trips away. False hopes near zero are now a symptom, not a goal.

The planner already has a pull toward squares not looked at for a while:
curiosity × staleness, where staleness = min(1, volatility × steps
unseen). Volatility, learned from how often squares change while unwatched,
settles near 0.0003 after an upbringing, so a square has to go unseen for
about 3,000 steps before the pull is felt. In practice the experienced
apprentice has no reason to go back to a square it has seen.

## What changes

`Drives::revisit: f64`, default 0 (today's behaviour). Staleness grows at
least this fast:

    staleness = min(1, max(volatility, revisit) × steps unseen)

So a square unseen for 1/revisit steps pulls like a square never seen. The
pull is curiosity, not food: the apprentice goes to look, and eats what it
finds on the way. Nothing tells it which world it is in. Clearing its
memory of when it saw each square at a world's start would do that, so it
is not an option.

## Tuning (seeds 101–110 only)

`revisit` from {1/64, 1/128, 1/256, 1/512}: the one with the fewest barren
test sleeps whose bush test sleeps are no more than 10% above the default's.
Only that value is judged.

## Marks, fixed before judging

Judged once on seeds 1–100 and 111–210, **R** (the tuned `revisit`) against
**A** (today's default), paired by seed, 90% interval, `apprentice::judge`.
Raw differences are reported next to percentages.

1. **Fewer sleeps in a new barren world**: barren test sleeps per 1,000
   steps, R vs A, at most −10%.
2. **More meals there**: barren test meals per 1,000 steps, R vs A, at least
   +10%.
3. **Experience no longer makes barren worlds harder** (as in PR #26): R's
   barren test sleeps per 1,000 steps against its first barren upbringing
   world, at most 0%.
4. **No regressions**, R vs A, each at most +10%: bush test sleeps, bush
   upbringing sleeps, barren upbringing sleeps, home-test wrong answers at
   100 steps.

Reported only, never a mark: false hopes (barren and bush), bush meals,
and, from `decline --revisit RATE` on seeds 101–140, hopeful trips with
their at-target and en-route meals (experienced and newborn, against the
same arms without revisit).

My guess before running: 2 met; 1 unclear; 3 missed (PR #26 suggests the
decline has more than one part); 4 at risk on bush sleeps, since looking
around costs steps in a world where the known bushes already feed it.

## Tuning (seeds 101–110)

`cargo run --release --bin revisit` (means per life):

| revisit | Barren test sleeps/1k | Barren test meals/1k | Bush test sleeps |
|---|---|---|---|
| 0 (default) | 28.1 | — | 7.0 |
| 1/64 | 4.1 | 69.2 | 14.2 (over the guard) |
| **1/128** | **2.8** | **71.6** | **6.5** |
| 1/256 | 3.2 | 71.4 | 5.5 |
| 1/512 | 7.0 | 65.7 | 7.6 |

The rule picks **1/128**: fewest barren sleeps with bush sleeps within 10%
of the default (the guard is 7.7). Only 1/128 is judged. The effect is far
larger than the decline it was aimed at (a newborn sleeps 25 per 1,000
steps): in a barren world, food regrows anywhere, so a patrol that keeps
going back to squares it hasn't seen for a while keeps finding it.

### Hopeful trips with revisit 1/128 (seeds 101–140, reported only)

`decline --revisit 0.0078125` ([decline-revisit.txt](decline-revisit.txt)),
world 5 per 1,000 steps, experienced (E) against newborn (N), both with
revisit:

| | N | E | E − N |
|---|---|---|---|
| Sleeps | 4.40 | 2.71 | −1.69 [−1.96, −1.41] |
| Meals | 69.0 | 71.9 | +2.85 [+2.33, +3.37] |
| Hopeful trips | 198 | 203 | +5.2 [+0.4, +9.9] |
| Meals at target | 15.3 | 16.4 | +1.11 [+0.35, +1.87] |
| Meals en route | 16.2 | 16.5 | +0.33 [−0.38, +1.03] |
| Meals off any trip | 37.6 | 39.0 | +1.41 [+0.62, +2.20] |

Without revisit the experienced apprentice slept 28.4 and the newborn 25.0
on these seeds. With revisit both sleep about a tenth as much, and **the
decline reverses**: experience now helps (−33% sleeps against a newborn).
Hopeful trips go from 6–19 to about 200 per 1,000 steps: it patrols.
