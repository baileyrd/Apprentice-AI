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
