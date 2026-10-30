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

Added while the 200-seed run was going, before its verdict was read. It
is a second judgement, not a rewrite: rule 3's 10% mark on barren false
hopes is still reported as it stands. The other rule 3 marks and rule 4
are unchanged.

**3b′. False hopes cost less than half of what the fix saves.** In the
barren test world, per 1,000 steps, paired by seed:

    (extra false hopes) × W  ≤  0.5 × (sleeps saved) × S

judged on the mean of each seed's left side less right side, met only if
the 90% interval's upper end is at most 0 (missed if its lower end is
above 0). False hopes are counted over the world's first 1,000 steps;
sleeps over all 2,000, halved.

Costs, measured once on today's default apprentice (the A arm) in the
barren test world of seeds 101–110 (`cargo run --release --example
hope_costs`), and not to be adjusted afterwards:

- **W = 0.50 steps per false hope**: steps spent closing on the square,
  in the run just before the sighting in which its distance fell every
  step, plus steps after while it still fell. Only 6 false hopes in those
  10 lives, so W rests on little: most false hopes are noticed in passing.
- **S = 3.69 steps per sleep**: a sleep takes no steps in this world; it
  wakes on the start square, so S is the shortest walk around walls from
  there back to where it fell asleep (562 sleeps).

`cargo run --release --bin hope_mark -- runs/sweep/sweep.csv` prints the
verdict from the judging run's CSV.

One candidate only: the combined fix (S). Time-unseen alone is not judged
on these seeds.

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
