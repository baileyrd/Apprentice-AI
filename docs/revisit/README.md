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

Judged once on seeds 1–100 and 141–240 (Nano, 2026-10-31: 111–140 were
used in the decline diagnosis), **R** (the tuned `revisit`) against
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

## Alongside the judging, not marks (Nano, 2026-10-31), fixed before running

1. **A hand-coded patroller**, no learning, in the same test worlds (barren
   test: seed + 5000, 2,000 steps; bush test: seed + 6000, 4,000 steps) on
   the same seeds. It eats when it stands on food. Otherwise it walks the
   shortest path, through squares it has seen and that are neither walls
   nor hazards, toward the
   square it has gone longest without seeing among those it can reach
   (never-seen squares next to known open ones count as unseen forever;
   ties go to the nearest, then to a fixed order). It remembers what each
   square looked like when last seen; it learns nothing about values.
   Reported: sleeps and meals per 1,000 steps, against A and R.
2. **Energy cost per step against energy per meal** in each world type,
   from the world's settings: cost of a step resting and moving, energy per
   meal, and the meals per 1,000 steps that would pay for walking every
   step (break-even), next to each arm's measured meals.

## Results (seeds 1–100 and 141–240, one run)

`cargo run --release --bin revisit -- --judge 0.0078125` prints
[judging.txt](judging.txt) and writes [revisit.csv](revisit.csv).

| Mark | R vs A | Raw difference | Verdict |
|---|---|---|---|
| 1. Barren test sleeps/1k ≤ −10% | −88% [−88%, −87%] | −25.3 [−25.7, −25.0] (28.1 → 2.8) | **met** |
| 2. Barren test meals/1k ≥ +10% | +366% [+341%, +392%] | +55.5 [+54.5, +56.4] (16.4 → 71.8) | **met** |
| 3. R barren test vs first barren world ≤ 0% | −58% [−60%, −56%] | −5.0 [−5.4, −4.6] (7.8 → 2.8) | **met** |
| 4a. Bush test sleeps ≤ +10% | −35% [−40%, −29%] | −6.8 [−9.0, −4.6] (14.8 → 8.0) | **met** |
| 4b. Bush upbringing sleeps ≤ +10% | −21% [−25%, −18%] | −8.3 [−9.8, −6.8] | **met** |
| 4c. Barren upbringing sleeps ≤ +10% | −77% [−77%, −76%] | −73.6 [−74.7, −72.5] | **met** |
| 4d. Home-test wrong (100 steps) ≤ +10% | +1% [−5%, +7%] | +0.01 [−0.09, +0.11] | **met** |

Reported only, R vs A: barren false hopes −3% (1.7 → 1.4), bush false
hopes +55% (2.8 → 4.8), bush meals +14% (277 → 292), barren futile eating
−84% (13.6 → 2.0).

All marks met. With revisit, experience helps in barren worlds: the test
world (2.8 sleeps per 1,000 steps) is easier than its first barren world
(7.8). Without it, the test world was harder (28.1 against 20.9).

### Patroller and energy (reported only)

`cargo run --release --bin patrol -- --against runs/revisit/revisit.csv`
([patrol.txt](patrol.txt), [patrol.csv](patrol.csv)).

Both world types charge and pay the same: a step costs 1 resting or eating,
2 walking (12 into a hazard), and a meal gives 25. Walking every step
breaks even at 80 meals per 1,000 steps.

Per 1,000 steps:

| | Barren sleeps | Barren meals | Bush sleeps | Bush meals |
|---|---|---|---|---|
| Patroller | 34.2 | 10.7 | 22.0 | 34.1 |
| A (default) | 28.1 | 16.4 | 3.7 | 69.4 |
| R (revisit 1/128) | 2.8 | 71.8 | 2.0 | 73.1 |

The patroller is worse than both in both worlds: it eats only what it
happens to stand on, and never steps toward food it can see. R's meals sit
just under break-even for walking every step (72 of 80), so R barely runs
down. Patrolling alone is not what R learned: R patrols *and* goes for food
it sees or expects.

## Follow-ups (Nano, 2026-10-31), fixed before running

Revisit 1/128 is now the default. Both runs below are reported only, on
seeds 1–100 and 141–240.

1. **Patrol + walk to visible food**, no learning: the patroller above,
   except that when food is in view and not underfoot it steps toward the
   nearest food in view (fewest steps, through squares that are neither
   walls nor hazards; ties broken north, east, south, west). Barren and
   bush test worlds. Reported against A (no revisit) and R (revisit 1/128)
   from [revisit.csv](revisit.csv), as paired raw differences per 1,000
   steps.
2. **Revisit + time-unseen + path-counting (RUS) against revisit alone
   (R)**, in the whole life. `PatternNet::unseen` and `Drives::sweep` on,
   with revisit 1/128. Measures: the seven used in the marks above, as
   paired change and raw difference with 90% intervals.
   **They add something** if at least one of barren test sleeps, bush test
   sleeps, barren upbringing sleeps or bush upbringing sleeps falls with the
   whole interval below 0%, and none of the seven rises with the whole
   interval above +10%. Otherwise they add nothing, and I propose removing
   both (deleting needs Nano's OK).

## Follow-up results (seeds 1–100 and 141–240)

### 1. Patrol + walk to visible food

`patrol --seek --against docs/revisit/revisit.csv` ([seek.txt](seek.txt),
[seek.csv](seek.csv)). Per 1,000 steps:

| | Barren sleeps | Barren meals | Bush sleeps | Bush meals |
|---|---|---|---|---|
| Plain patroller | 34.2 | 10.7 | 22.0 | 34.1 |
| Seeking patroller | 4.2 | 98.9 | 8.8 | 60.3 |
| A (no revisit) | 28.1 | 16.4 | 3.7 | 69.4 |
| R (revisit 1/128) | 2.8 | 71.8 | 2.0 | 73.1 |

Seeking patroller minus R: barren sleeps +1.4 [+0.4, +2.4], barren meals
+27.0 [+23.8, +30.2], bush sleeps +6.8 [+6.4, +7.3], bush meals −12.8
[−13.8, −11.7].

Walking to food it can see is what the plain patroller lacked: in barren
worlds it gets close to R (4.2 against 2.8 sleeps) and eats more (99
against 72, above the 80 break-even). R still sleeps less in both worlds,
and much less in bush worlds (2.0 against 8.8), where knowing which
squares grow back pays.

### 2. Revisit + time-unseen + path-counting (RUS) against revisit alone (R)

`revisit --extras` ([extras.txt](extras.txt), [extras.csv](extras.csv)):

| Measure | RUS vs R | Raw difference |
|---|---|---|
| Barren test sleeps/1k | −27% [−30%, −23%] | −0.81 [−1.02, −0.60] |
| Barren test meals/1k | +2% [+1%, +3%] | +1.83 [+1.42, +2.24] |
| Bush test sleeps | −23% [−26%, −19%] | −1.44 [−2.18, −0.69] |
| Bush upbringing sleeps | −35% [−37%, −33%] | −8.28 [−8.73, −7.83] |
| Barren upbringing sleeps | −50% [−52%, −48%] | −10.6 [−11.3, −9.9] |
| Home-test wrong (100 steps) | +13% [+6%, +20%] | +0.24 [+0.13, +0.35] |
| RUS barren test vs its first barren world | −12% [−19%, −4%] | −0.54 [−0.98, −0.10] |

By the reading fixed above, **they add something**: every sleep count falls
with the whole interval below 0, and no measure rises by more than 10% for
sure. The home test is the one cost: +13%, interval [+6%, +20%], about one
more wrong answer every four seeds. So no removal is proposed. Making RUS
the default would need its own marks and judging seeds.

## Removed (Nano, 2026-10-31)

Time-unseen (`PatternNet::unseen`), path-counting (`Drives::sweep`) and
`revisit --extras` are deleted. The last commit with them is c16c8bc; the
results above came from it.

Why:

- **They break the home-test limit.** Nano set it after the run above: a
  default change may add at most +0.25 wrong answers per home test, at
  the upper end of the raw 90% interval. RUS adds +0.24 [+0.13, +0.35],
  so its upper end is over. The limit is a hard one, not a cost weighed
  against gains.
- **Experience pays less with them.** The test world against the first
  barren world: R −58% [−60%, −56%] (raw −5.01 sleeps per 1,000), RUS
  −12% [−19%, −4%] (raw −0.54). They help the first barren world most,
  which narrows what later worlds add.

What is given up: barren upbringing sleeps −50% (raw −10.6) and bush
upbringing sleeps −35% (raw −8.3). In the test worlds the gain was small:
−0.81 sleeps per 1,000 in barren, −1.44 sleeps in bush.

## What barren and bush worlds tell us

- **Barren worlds are solved by patrolling and sight.** The seeking
  patroller has no learning, yet it gets close to R there: 4.2 against 2.8
  sleeps per 1,000 steps (raw +1.4 [+0.4, +2.4]), and it eats more (98.9
  against 71.8). So barren worlds tell learners apart only weakly.
- **Learning shows in bush worlds.** There R sleeps 2.0 times per 1,000
  steps against the seeking patroller's 8.8 (raw +6.8 [+6.4, +7.3]),
  because it knows which squares grow back. Judging a learning change
  should lean on bush worlds.

## Open question

**Why do time-unseen and path-counting harm the home test?** Neither is
part of telling home from a stranger: the home test asks whether the map
fits. Unchecked leads: patterns that see time unseen change how eaten
squares are judged in the map-fit check (regrowth-judged fit), or sweeping
plans change where it walks during a trial and so what it sees. The same
kind of harm came with the patterns network (PR #21), and that cause is
also still open.
