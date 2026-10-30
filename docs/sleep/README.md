# Selective sleep: a replay budget

Until now, every sleep and every nap replays the whole diary into the map
in use: every square seen since the last sleep, in order. Nano's design asks
for **selective consolidation**: sleep replays only part of the diary,
chosen by "surprise" and stakes, and what is not replayed fades. It can be
learned again later, when the square is seen again.

This slice also asks whether that lets the **memory gate** (PR #19) go on.
The gate cut diary writes by 88%, but its life got worse in one way: bush
sleeps +24% and barren false hopes +39%. The suspected cause was never
checked: with the gate, the map learns mostly in rare long sleeps, not in
naps every 64 steps.

## Words used here

- **"Surprise"** of a sighting: its prediction error (squared, per square)
  just before it was seen, as in every phase. A plain number, not a feeling.
- **Replay budget**: the most diary entries one sleep or nap replays into
  the map.

## What changes

`Recall::replay_budget: Option<usize>` (default `None`, which replays
everything as now). With a budget of B:

- Meals are always replayed, with their extra replays, as now: they are what
  is at stake.
- Of the other diary entries it is sure belong to the map in use, the B
  most "surprising" are replayed, in the order they were seen. Ties go to
  the most recent.
- The rest are dropped. Squares not replayed and not confirmed fade toward
  the prior, as unrehearsed squares already do.
- Patterns still learn from every entry. They are statistics over many
  worlds, and PR #19 showed that teaching them only surprises biases them
  toward change.

Not in this slice: sleep length setting the budget (an energy sleep and a
nap get the same budget), and interleaving important older memories.

## Test

The life of PR #19 and PR #21 (`apprentice::life`): four upbringing worlds,
a new bush world, from a second upbringing a new barren world, and the
home test. Four settings per seed, all with the patterns network (today's
default):

- **A**: today's default: gate off, whole diary replayed.
- **B**: gate off, replay budget.
- **C**: gate on, whole diary replayed.
- **D**: gate on, replay budget.

## Rules fixed before any run

Budget tuned on seeds 101–110 only. Judged once on seeds 1–100 and
111–210, each comparison paired seed by seed, read from the 90% interval as
met, missed or unclear with `apprentice::judge` (the method of PR #18).

1. **Sleep does less work.** Diary entries replayed into the map in the
   bush world, B against A: at most −50%.
2. **Nothing that matters is lost.** B against A, each at most +10%: bush
   sleeps, barren sleeps, barren false hopes, home-test wrong answers after
   100 steps.
3. **The gate can go on.** D against A, each at most +10%: the same four.
4. **Selective sleep is what fixes the gate.** Bush sleeps, D against C:
   at most −10%.

Reported but not judged: meals, upbringing sleeps, naps, home-test wrong
answers after 300 steps.

My guess before running: rule 1 met by construction once the budget is
small enough. Rule 2 likely met for a moderate budget, since most diary
entries confirm what the map already knows. Rules 3 and 4 the most likely
misses: a budget makes each sleep do less, while the gate's problem is
suspected to be sleeping too rarely, which a budget does not change.

## Tuning (seeds 101–110)

Budgets of 18, 72, 144 and 288 entries per sleep (2, 8, 16 and 32 steps'
worth of sightings). Every budget raised bush sleeps over replaying
everything (A: 7.0; B: 54 at 18, 11.7 at 72, 12.0 at 144, 9.3 at 288). 288
did not cut replays by half (−48%). None brought the gate's barren false
hopes down (C: 10–16; D: 19–47, worse with smaller budgets). **Chosen:
144**, the largest budget that meets rule 1, so the least harm for the
work it saves. Before judging, rules 2 (bush sleeps), 3 and 4 looked likely
to miss.

## Results (seeds 1–100 and 111–210, one run, budget 144)

`cargo run --release --bin sleep -- --judge` prints the verdicts
([judging.txt](judging.txt)) and writes [sleep.csv](sleep.csv). About 95
minutes on 4 cores.

| Rule | Result | Verdict |
|---|---|---|
| 1. Bush replays, B vs A ≤ −50% | −73% [−73%, −72%] | **met** |
| 2a. Bush sleeps, B vs A ≤ +10% | +1% [−7%, +10%] | **met** |
| 2b. Barren sleeps, B vs A ≤ +10% | +0% [−1%, +2%] | **met** |
| 2c. Barren false hopes, B vs A ≤ +10% | −3% [−15%, +10%] | **met** |
| 2d. Home-test wrong (100 steps), B vs A ≤ +10% | −5% [−10%, +1%] | **met** |
| 3a. Bush sleeps, D vs A ≤ +10% | +34% [+25%, +45%] | **missed** |
| 3b. Barren sleeps, D vs A ≤ +10% | −52% [−53%, −51%] | **met** |
| 3c. Barren false hopes, D vs A ≤ +10% | +578% [+454%, +730%] | **missed** |
| 3d. Home-test wrong (100 steps), D vs A ≤ +10% | −17% [−21%, −12%] | **met** |
| 4. Bush sleeps, D vs C ≤ −10% | +20% [+13%, +27%] | **missed** |

Means (A, B, C, D): bush sleeps 14.2, 14.1, 13.6, 16.0; barren false
hopes 1.7, 1.0, 19.6, 21.4; barren meals 32.5, 32.3, 102.9, 102.8; bush
naps 55.3, 55.5, 6.4, 6.0.

## What it shows

- **Sleep can do a quarter of the work and lose nothing.** Replaying the
  144 most "surprising" entries, not all of them, cuts replays by 73% with
  every measure level. Most of the diary confirms what the map already
  knows. Tuning on 10 seeds made it look harmful (bush sleeps 7 → 12);
  on 200 seeds that was noise.
- **It does not let the gate go on.** With the gate, the budget makes bush
  sleeps worse, not better (+20% against the gate alone). The gate's
  problem is not how much or how often sleep consolidates.
- **With the network patterns, the gate's false hopes are much worse than
  in PR #19.** Gate alone: 1.7 → 19.6 per barren world (PR #19, with the
  table: +39%). It also eats three times as much there, as in PR #19, so
  part of this is more meals giving more chances to hope. The rest is
  unchecked. A lead: with the gate, expected sightings teach the patterns
  only through counted confirmations (by age group), not one by one, and
  the network may learn those differently from the table.
- **The gate still halves barren sleeps and improves the home test**
  (−17% wrong answers), as in PR #19.

The default is unchanged: `replay_budget` stays `None` and the gate stays
off. Turning the budget on is a small choice (less work, same life); I
left it off because nothing yet needs the saving.
