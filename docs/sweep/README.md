# Sweeping: why barren worlds got harder, and plans that count food along the way

Nano asked how the apprentice does in each world of a life, not only the
last one. Looking world by world (`cargo run --release --example
sequences`, seeds 1–40) showed that bush worlds get easier with experience
(sleeps per 1,000 steps 8 → 4) but barren worlds get *harder* (22 → 29).
No rule so far measured it: every verdict looked at the final world, and
upbringing only as one total.

## Diagnosis (seeds 101–140, first 1,000 steps of each world)

`cargo run --release --example barren_why` lives the barren-first sequence
(barren, bush, barren, bush, barren) and reports each world.

- **Experience hurts in barren worlds.** A newborn put in the same later
  barren worlds (`-- --newborn`) does better than the experienced
  apprentice: sleeps 19.9 and 21.5 against 25.8 and 25.9, meals 30 and 28
  against 22 and 24. So it is not the worlds' layouts.
- **Not the map's general expectation.** A new map inherits its general
  food guess from the last one (0.07 in world 1, 0.02 in later barren
  worlds). Resetting it to even odds for every new map changed nothing.
- **The patterns.** With patterns off (`-- --no-patterns`) barren worlds
  become easy and get easier with experience: sleeps 11.1, 8.9, 7.0 and
  meals 54, 60, 65 in worlds 1, 3 and 5 (with patterns: 21.1, 25.8, 25.9
  and 28, 22, 24). Bush worlds barely change (6–8 sleeps either way).

The lead: patterns say, correctly, that a square seen empty is unlikely to
hold food now. In a barren world food reappears on a random empty square,
so each such square has only a small chance. But the planner values a path
by the single best square it reaches, so many small chances never add up,
and the apprentice stops searching places it has seen. Without patterns,
squares fade back toward the map's general guess (about 0.13 food), each
looks worth a visit, and it keeps sweeping.

## What changes

Changed during tuning, before any judged run: the first fix tried
(sweeping plans alone) helped too little, and tuning found a deeper cause.
The rules below are unchanged; setting **S** is now both parts.

- **Patterns see how long a square went unseen**
  (`PatternNet::unseen`, default `false`). Patterns judged a square only
  by how long ago it last *changed*. For a bush that is right: it regrows a
  fixed time after it was eaten. For food that pops up anywhere it is
  wrong: what matters is how long the apprentice has not *looked*. A square
  seen empty a moment ago is almost surely still empty, whatever its age,
  and almost every sample the patterns learn from is such a glance. So they
  said a square seen empty long ago and not looked at since had no chance
  of food (0.000–0.007 at 200 and 800 steps, after the bush worlds). The
  network now also gets the time since the square was last seen; the table
  and word patterns ignore it.
- **Plans sweep** (`Drives::sweep`, default `false`). A plan's food is
  summed along the path: every square a path steps on adds its expected
  food, discounted by its distance, instead of only the best square
  counting. Curiosity still counts the best square only. Eating now counts
  the best plan after it, since a sweeping plan counts every meal on its
  way.

## Test

The life of PRs #19–#24 (`apprentice::life`), now also reporting every
world: the four upbringing worlds of both sequences (barren-first and
bush-first) and the two test worlds. Two settings per seed: **A**, today's
default; **S**, A with patterns that see time unseen and sweeping plans.

## Rules fixed before any run

Tuning (if any knob is needed) on seeds 101–110 only. Judged once on seeds
1–100 and 111–210, each comparison paired seed by seed, read from the 90%
interval as met, missed or unclear with `apprentice::judge` (PR #18).

1. **Barren worlds get easier.** Barren test-world sleeps, S against A: at
   most −25%.
2. **In every barren world.** Sleeps summed over the four barren
   upbringing worlds (two in each sequence), S against A: at most −25%.
3. **No regressions**, S against A, each at most +10%: bush test-world
   sleeps, sleeps summed over the four bush upbringing worlds, barren false
   hopes, home-test wrong answers after 100 steps.
4. **Experience no longer hurts in barren worlds.** With S, sleeps per
   1,000 steps in the barren test world against the first barren world of
   the same life: at most +0%.

Reported but not judged: meals in every world, futile returns, rule 4 for
A.

My guess when the rules were written (before either fix): rules 1 and 2 met if the lead is right; rule 3
at risk in bush worlds, where summing food along a path may pull it toward
clusters of bushes not yet regrown; rule 4 unclear.

## A second false-hope mark (Nano, 2026-10-30, fixed before the verdict)

Added while the 200-seed run was going, before its verdict was read, and
amended once (still before the verdict) to measure steps in each arm
instead of fixed costs. It is a second judgement, not a rewrite: rule 3's
10% mark on barren false hopes is still reported as it stands. The other
rule 3 marks and rule 4 are unchanged.

**3b′. False hopes cost less than half of what the fix saves.** In the
barren test world of the barren-first life, per 1,000 steps, paired by
seed:

    (S's wasted steps − A's wasted steps) − ½ × (A's steps lost to sleeps − S's)  ≤  0

judged on the mean over seeds, met only if the 90% interval's upper end is
at most 0 (missed if its lower end is above 0).

- **Steps wasted on false hopes**: steps taken toward the hoped-for square
  that a straight route to the next real goal would not have taken. A
  false hope is an eaten square in view, empty, given more than even odds
  of food, in the world's first 1,000 steps. The steps closing on it are
  the run just before the sighting, and just after it, in which the walk
  (around walls) to that square shrinks every step. Each such step is
  wasted unless it also shrinks the walk to the next square the apprentice
  eats from; with no later meal, all are wasted. Noticed in passing on the
  way to a meal counts 0. Each step counts once per square.
- **Steps lost to sleeps**: a sleep takes no steps in this world; it wakes
  on the start square, so each sleep loses the walk from there back to
  where it fell asleep. Counted over all 2,000 steps, halved.

`cargo run --release --bin hope_steps -- --judge` lives the barren-first
life of each judging seed under A and S step by step, prints the verdict
and each arm's W (wasted steps per false hope) and S (steps lost per
sleep), and writes `hope_steps.csv`.

One candidate only: the combined fix (S). Time-unseen alone is not judged
on these seeds. Bush false hopes are reported, not judged, from the next
run of `sweep` on (not in this one).

## Tuning (seeds 101–110)

`cargo run --release --bin sweep` (two settings; tuning compared four).
Means per life, A against each:

| | A | sweep only | unseen only | both (S) |
|---|---|---|---|---|
| Barren test sleeps | 56.2 | 51.8 | 38.8 | 44.5 |
| Barren upbringing sleeps (4 worlds) | 97.4 | 88.3 | 81.4 | 61.9 |
| Bush test sleeps | 7.0 | 7.5 | 9.1 | 6.3 |
| Bush upbringing sleeps (4 worlds) | 35.4 | 23.9 | 31.8 | 25.0 |
| Barren false hopes | 0.6 | 0.6 | 1.0 | 2.0 |
| Barren test meals | 36 | 48 | 80 | 69 |

- **Sweeping alone helps little** (barren sleeps −8%), and as first built
  it never ate: a sweep of 20 squares outweighed one meal here, so eating
  had to count the sweep after it.
- **Time unseen is most of it**: barren test meals double.
- **Both** help most across the upbringing and in bush worlds. **Chosen:
  both.** False hopes rise from very few (0.6 → 2.0), a risk for rule 3.

The diagnosis above used seeds 101–140, 30 of which are judging seeds; it
looked only at today's apprentice, not at either fix.

## Results (seeds 1–100 and 111–210, one run each)

`cargo run --release --bin sweep -- --judge` ([judging.txt](judging.txt),
[sweep.csv](sweep.csv)), about 85 minutes on 4 cores, and `cargo run
--release --bin hope_steps -- --judge` ([hope_steps.txt](hope_steps.txt),
[hope_steps.csv](hope_steps.csv)), about 20 minutes. The judging run's
binary predates the bush-false-hopes column, so it has none.

| Rule | A → S, means per life (raw change) | Change [90%] | Verdict |
|---|---|---|---|
| 1. Barren test sleeps ≤ −25% | 56.3 → 46.5 (−9.8) | −24% [−28%, −20%] | **unclear** |
| 2. Barren upbringing sleeps ≤ −25% | 95.9 → 62.8 (−33.1) | −36% [−39%, −34%] | **met** |
| 3a. Bush test sleeps ≤ +10% | 14.2 → 9.4 (−4.8) | −35% [−40%, −30%] | **met** |
| 3b. Bush upbringing sleeps ≤ +10% | 32.7 → 23.2 (−9.5) | −30% [−34%, −27%] | **met** |
| 3c. Barren false hopes ≤ +10% | 1.7 → 3.3 (+1.6) | +42% [+22%, +65%] | **missed** |
| 3d. Home-test wrong (100 steps) ≤ +10% | 0.6 → 0.5 (−0.1) | −2% [−7%, +4%] | **met** |
| 3b′. Extra wasted steps − ½ sleep steps saved ≤ 0, per 1,000 | wasted +0.61, saved 30.5 | −14.6 [−16.4, −12.8] steps | **met** |
| 4. Barren world 5 vs world 1 (S), per 1,000 steps ≤ +0% | 12.9 → 23.3 sleeps | +87% [+71%, +105%] | **missed** |

Rule 4 for A (not judged): 21.1 → 28.2 sleeps per 1,000 steps, +33%
[+30%, +36%].

Each arm's costs (3b′): A wastes 0.58 steps per false hope and loses 3.78
steps per sleep; S wastes 0.49 and loses 3.27. Barren test meals 32.5 →
64.1; bush test meals 279 → 291; barren futile returns 13.1 → 5.2.

## What it shows

- **The root cause was the patterns' clock.** Judging a square by how
  long since it changed, not since it was last seen, made every square
  out of sight look empty in a world where food pops up anywhere. With
  time unseen, plus plans that add up food along the way, the apprentice
  eats twice as much in the barren test world, sleeps a third less across
  the barren upbringing, and does better in bush worlds too.
- **False hopes rise but cost almost nothing.** 1.6 more per barren
  world, which misses the 10% mark; but most are noticed in passing, so
  they waste 0.6 extra steps per 1,000 against 30 steps saved on sleeps.
- **Experience still makes barren worlds harder**, with or without the
  fix. The fix helps the first barren world most (sleeps 21.1 → 12.9), so
  the gap to the test world grows. Something learned in between still
  hurts there; not yet understood.
- **Barren test sleeps** fell 24%, a hair short of the 25% mark (unclear).

Defaults unchanged: `PatternNet::unseen` and `Drives::sweep` stay off.
