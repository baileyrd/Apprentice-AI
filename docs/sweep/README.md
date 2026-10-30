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

`Drives::sweep: bool` (default `false`: as now). With it on, a plan's food
is summed along the path: every square a path steps on adds its expected
food, discounted by its distance, instead of only the best square counting.
Curiosity still counts the best square only. The world model and patterns
are untouched.

## Test

The life of PRs #19–#24 (`apprentice::life`), now also reporting every
world: the four upbringing worlds of both sequences (barren-first and
bush-first) and the two test worlds. Two settings per seed: **A**, today's
default; **S**, A with sweeping.

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

My guess before running: rules 1 and 2 met if the lead is right; rule 3
at risk in bush worlds, where summing food along a path may pull it toward
clusters of bushes not yet regrown; rule 4 unclear.
