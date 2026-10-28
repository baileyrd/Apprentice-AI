# Patterns: knowledge that carries to new worlds

## Rules fixed before any run

These were written before any run, tuning seeds included. Settings may be
tuned on seeds 101–110 against these rules; the final check is one run on
seeds 1–10. "With patterns" and "without" are the same apprentice (memory,
meal replay, patience) with the pattern layer on or off.

**Transfer test.** The apprentice lives 2,000 steps in each of 3 bush worlds,
then 4,000 steps in a fresh bush world it has never seen.

1. **Anticipation at first encounter.** In the fresh world, the first time it
   sees a bush it ate from grown back, the chance of food it predicted for
   that square just before seeing it is at least 0.5 (median) with patterns,
   and at least 0.2 above without.
2. **Settling faster.** In the fresh world, the steps until its surprise
   (mean over 100 steps) falls to 1.5× the level it had at the end of the
   previous world are at least 25% fewer with patterns (median).
3. **Lessons transfer.** Rerunning the lessons exam with patterns, exam
   sleeps after lessons are at least 20% below no schooling (median).
4. **No regression.** Over 8,000 steps in the bush world, median sleeps
   with patterns are no more than 10% above without.

## How it works

Nano's framing: short-term memory feeding long-term memory, plus pattern
recognition. "Bushes regrow. I've seen that before, so I might anticipate it
somewhere else."

- **A third, slowest store.** Besides the diary (today) and the maps (one per
  world), the apprentice now keeps a single table of **patterns**. These
  describe how squares change in any world, not what is where.
- **What a pattern is keyed on.** The note suggested keying on what things
  look like nearby. That can't work here: bushes are placed at random, so
  nothing near a bush gives it away. What gives a bush away is its history,
  so I keyed patterns on that. A square's state is how it last changed: it
  came to look like a wall, floor, food or hazard, or the apprentice ate from
  it. The state also carries how long ago that was, grouped by powers of two
  (1, 2–3, 4–7, … 128+), so nothing is tuned to the 30-step bush. Each
  pattern predicts what a square in that state, that old, looks like now.
- **Fed only in sleep, from every world.** When the diary is replayed in
  sleep, each sighting teaches its pattern, by a delta rule at rate 0.05. A
  pattern predicts once it has 20 samples. There is one table for all
  worlds.
- **Used for any square seen in the current world.** Where a pattern is
  known, it gives the prediction for that square; otherwise the diary or map
  does. So after eating from a bush in a world it has never seen, the
  apprentice expects it to be food again about 30 steps later.
- **One fix made on the tuning seeds.** In a new world, the apprentice
  starts a new map a few dozen steps in. At first the new map didn't inherit
  what had just been seen, including the first meal. Now, when it switches
  maps, the square states sensed since the old map stopped fitting move
  across with it. Before this fix, anticipation was 0.26 on tuning seeds;
  after it, 0.91. The rules were unchanged.
- **One bug fixed after the final run.** At first those states were copied,
  not moved, so the old map's record kept the new world's squares. That is
  wrong, and it showed up while looking into the slower return to A (below).
  All numbers below are from the rerun after the fix; the first run's numbers
  are in the git history. The verdicts are the same, except that lessons now
  do slightly worse than no schooling, not slightly better.

Every update is still a local delta rule, with no gradients and no ML
library.

Reproduce:

```sh
cargo run --release --bin transfer -- --out docs/patterns
cargo run --release --bin lessons -- --out docs/patterns/lessons
```

Per-seed numbers are in [transfer.csv](transfer.csv) and
[lessons/lessons.csv](lessons/lessons.csv). All numbers are medians over seeds
1–10.

## Results

| | Without patterns | With patterns |
|---|--:|--:|
| Food it predicted at the first regrown bush it ate from, fresh world | 0.12 | **0.93** |
| Steps for surprise to settle, fresh world | 318 | 290 |
| Sleeps, first 4,000 steps in the fresh world | 18 | **3** |
| Sleeps over 8,000 steps in one bush world | 38.5 | **10.5** |

| Rule | Result | Met? |
|---|---|---|
| 1. Anticipation ≥ 0.5, and ≥ 0.2 above without | 0.93 vs 0.12 | yes |
| 2. Surprise settles ≥ 25% faster | 290 vs 318 steps, −9% | **no** |
| 3. Lessons: exam sleeps ≥ 20% below no schooling | 57 vs 55, +4% | **no** |
| 4. Bush-world sleeps ≤ 10% above without | 10.5 vs 38.5, −73% | yes |

Lessons exam, with patterns (medians):

| Mind | Exam sleeps | Meals | Hazards | Steps before exam |
|---|--:|--:|--:|--:|
| No schooling | 55 | 180 | 12 | 0 |
| Same steps in hard worlds | **44** | 196 | 4 | 683 |
| Lessons | 57 | 168 | 10 | 683 |

Without patterns the same three gave 75.5, 72 and 74.5 exam sleeps.

## What it means

- **It anticipates regrowth in worlds where it has never seen it.** The
  first time a bush it ate from has grown back, it already expects food there
  in 9 of 10 seeds (above 0.5). Without patterns it does so in 4 of 10, and
  only when a sleep happened to replay the meal first.
- **Knowing that bushes regrow nearly ends running out of energy.** It goes
  back to bushes when they should have regrown, rather than when it last saw
  them full. In a single bush world it sleeps about a quarter as often (10.5
  against 38.5). In a fresh world it sleeps 3 times in 4,000 steps against
  18. This is the largest gain of any phase so far.
- **Patterns don't make it map a new world faster.** Settling depends on
  learning where the walls are, and walls are places, not patterns. Rule 2
  missed by a long way.
- **Experience now carries over, but easy lessons don't.** With patterns,
  every mind does better in the exam. Lessons did no better than no
  schooling (57 against 55). Time in hard worlds did best (44 sleeps, 4
  hazards stepped on), probably because the safe lessons taught nothing
  about hazards. A course
  that teaches what the exam needs would be the next thing to try.
- **Side effects on earlier tests.** I reran these tests with patterns on.
  The outputs are in [relearn/](relearn/), [walls-bushes/](walls-bushes/),
  [walls/](walls/) and [energy/](energy/). Every earlier rule still holds.
  - **Re-mapping moved walls when fed, bush world.** Still 10 of 10, at 222
    steps.
  - **Phase 2 wall test.** Median adaptation is 427 steps, 9 of 10 seeds,
    against 204 after the energy fix. That is still within the 798-step bar.
  - **Relearning A (phase 4, rule 1).** Savings are now 0.46, against 0.19
    after the energy fix. That still meets "≤ half", but only just: coming
    back to A takes 42 steps instead of 18. The next section explains why.

## Why coming back to A got slower

Nano asked whether this is memories degrading, or the apprentice learning
not to expect the same thing twice. I tested both on seeds 1–10:

- **Not degrading memory.** With forgetting switched off, savings were 0.45
  with patterns and 0.19 without, about the same as with forgetting on.
- **Not doubt about old squares.** The patterns for squares seen long ago are
  certain: a wall seen 1,000 steps ago is predicted to be a wall with
  probability 1.00. If anything it expects the same thing too firmly.
- **What it is: mixing up the two worlds.** In some seeds the apprentice,
  while in world B, keeps switching back to A's map. What it sees in B then
  gets written into A's map, so when it returns to A, part of A's map is
  really B. This happens without patterns too, in 2 of 10 seeds. With
  patterns it happens in 4 of 10. In those seeds it spent 234, 707 and 3,000
  of its 3,000 steps in B on A's map, plus one seed that mixes up worlds
  with or without patterns. With only 10 seeds, two extra mix-ups are enough
  to move the median.
- **Why patterns make mix-ups more likely is not proven.** It explores
  about 7% less in B with patterns (it lives off bushes it can predict).
  Squares near its usual routes look alike in A and B, so A's map can seem to
  fit. That is my best guess, and it is untested.
- The moved-not-copied bug above also played a small part (0.50 before the
  fix, 0.46 after).

The fix would be in how it recognises a world: it should need stronger
evidence before going back to an old map. That would be its own change.
