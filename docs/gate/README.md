# Gate: active memory decides what reaches the diary

Nano's memory design has three tiers in a loop: **active memory** (small,
what is in mind now) → **short-term** (the diary) → **long-term** (the maps
and patterns, written in sleep). Long-term memory's predictions flow back
into active memory and decide what is "surprising" enough to be kept.

Today there is no gate. Every square in view, every step, goes into the
diary: nine sightings a step, most of them a wall or floor it has seen a
hundred times. The diary fills in 64 steps and forces a nap, and sleep
replays all of it.

This phase is the first slice of the network phase, on today's tables. The
hidden-layer network comes in a later slice and will sit behind the same
gate.

## Words used here

- **"Surprise"**: prediction error. For one sighting, the squared
  difference between what memory predicted for the square and what was
  seen, as in every phase since phase 1. It is how the idea might show up
  in code, not a claim that the apprentice feels anything.
- **Long-term's guess** for a square: what patterns expect of it, or else
  what the map in use holds. It is the guess before anything seen since the
  last sleep.

## The change

- **Active memory.** Every sighting goes into active memory first: a
  small store of the squares seen most recently (18, two views' worth),
  each with its working belief. When it is full the oldest square drops
  out. It answers predictions first. Sleep clears it.
- **The gate.** A sighting is written into the diary only if one of these
  holds:
  - long-term memory does not know the square yet;
  - it is "surprising": its error against long-term's guess is above a
    threshold;
  - it is at stake: a square it ate from, still waiting to grow back
    (patterns about regrowth learn from these).
- **Confirmations.** A sighting that is not written still leaves a trace
  in short-term memory: "this square held". In sleep a confirmed square is
  rehearsed (it does not fade, and grows stiffer) without being replayed.
- Everything else is as before: the diary answers before the map, fills,
  forces a nap when full, and is replayed into the map in sleep.

The gate is a setting of the memory (`Recall::gate`), off by default, so
every earlier test gives the same numbers as before. The test below turns
it on.

## Test

Two lives, each run twice from the same seed, gate off and gate on:

1. **The language test's life, with no mentor** (as in PR #14): four
   upbringing worlds of 1,000 steps, bush and barren in turn, then a new
   bush world (4,000 steps) and, from a second upbringing, a new barren
   world (2,000 steps).
2. **The home test** (as in PR #11): 3,000 steps at home, then 300 steps
   in home as it left it, home with one wall moved, or each of three
   strangers.

## Rules fixed before any run

Tuning on seeds 101–110 only (the threshold and active memory's size).
Judged once on seeds 1–100 and 111–210, each rule paired seed by seed,
read from the 90% interval as met, missed or unclear with
`apprentice::judge` (the method of PR #18).

1. **The diary carries much less.** In the new bush world, sightings
   written into the diary: typical change at most −50% against gate off.
2. **Fewer naps.** In the new bush world, naps forced by a full diary:
   typical change at most −30%.
3. **What is new still gets through.** Over the new bush world, of the
   sightings where long-term's most likely kind is not what was seen, the
   share written into the diary, per seed; the median over seeds is at
   least 0.95.
4. **No regressions** (typical change against gate off at most +10%
   each): sleeps in the new bush world, sleeps in the new barren world,
   false hopes in the new barren world, and wrong answers in the home
   test (out of five trials per seed, read after 100 steps).

Reported but not judged: the same numbers on the whole upbringing, and
the home test's answers after 300 steps.

My guess before running: rules 1 and 2 met by a wide margin, since in a
known world nearly every sighting is a confirmation. Rule 3 met almost by
construction. Rule 4 is the real question: fewer replays per sleep mean
the general patterns learn from fewer sightings of squares that did not
change, which could tilt them toward "things change", and confirmations
stiffen a square once per sleep instead of once per sighting, so known
squares may fade differently.
