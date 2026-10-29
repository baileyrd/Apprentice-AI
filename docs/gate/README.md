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

## Tuning (seeds 101–110)

The first tuning run turned up a flaw in the design above, fixed before
judging. With only "surprising" sightings replayed, the general patterns
learned mostly from squares that changed, so they came to expect change:
an empty square seen 4–7 steps ago was given an 8–9% chance of being food
(0–4% with the gate off), and up to 43% after 128 steps. **Change:**
confirmations now also teach the patterns they confirm. Short-term memory
keeps a tally per pattern (state, age group, word, what was seen), not an
entry per sighting, and sleep replays the tally before the diary. That
brought the patterns back near gate-off levels.

Then, with the tally: threshold 0.05, 0.1 and 0.2; active memory of 9, 18
and 36 squares. All met rules 1–3; bush sleeps on 10 seeds ranged from
−29% to +42%, all unclear. Chosen: **threshold 0.05, 18 squares**, the
best bush sleeps. A threshold of 0 writes everything and gives exactly
the gate-off numbers, and gate off gives exactly PR #14's numbers (checked
on seeds 1–4).

## Results (seeds 1–100 and 111–210, one run)

`cargo run --release --bin gate -- --judge` prints the verdicts
([judging.txt](judging.txt)) and writes [gate.csv](gate.csv).

| Rule | Result | Verdict |
|---|---|---|
| 1. Diary carries much less (≤ −50%) | −88% [−88%, −87%] | **met** |
| 2. Fewer naps (≤ −30%) | −90% [−91%, −89%] | **met** |
| 3. Unexpected sightings written (median ≥ 0.95) | 1.000 | **met** |
| 4. No regressions (each ≤ +10%) | | **missed** |
| – bush sleeps | +24% [+10%, +40%] | missed |
| – barren sleeps | −49% [−51%, −47%] | met |
| – barren false hopes | +39% [+11%, +74%] | missed |
| – home-test wrong answers (100 steps) | −14% [−17%, −10%] | met |

Numbers are the typical change against gate off, with the 90% interval in
brackets. Reported only: in the upbringing, diary writes −80%, naps −92%,
sleeps −27%; home-test wrong answers after 300 steps −9%.

## What it shows

- **The gate does its job.** In a world it has started to know, nearly
  nine sightings in ten were confirmations, and nothing unexpected was
  dropped. The diary fills about ten times more slowly, so naps almost
  stop (50.7 → 4.1 per bush world, on average).
- **It evens lives out rather than improving them.** Bush sleeps, by how
  the seed did with the gate off:

  | Gate-off bush sleeps | Seeds | Off (mean) | On (mean) | Typical change |
  |---|--:|--:|--:|--:|
  | 0–5 | 68 | 3.3 | 12.8 | +158% |
  | 6–15 | 77 | 9.2 | 16.3 | +44% |
  | 16–40 | 25 | 26.7 | 19.8 | −30% |
  | over 40 | 30 | 97.7 | 33.6 | −74% |

  The mean falls (22.7 → 18.1) but the typical seed does worse, so the
  rule is missed. Lives that went well get worse, and lives that went
  badly get much better.
- **The false-hope miss comes from eating more.** In the barren world it
  eats three times as often (30.9 → 94.0 meals on average) and wastes far
  fewer tries on squares it ate from (futile returns 24.2 → 4.6), so more
  eaten squares come back into view. Per meal, false hopes fall from 0.71
  to 0.28. The gate-off apprentice starves in barren worlds (about one
  sleep every 35 steps); with the gate it sleeps half as often. The rule
  counted false hopes, not false hopes per meal, so it stands as missed.
- **Likely cause of both, unchecked:** the map now learns mostly in the
  long sleeps, not in naps every 64 steps. Where quick updating pays (a
  good life in a bush world, learning where bushes are) that costs; where
  stale beliefs hurt (expecting a barren world's eaten squares to regrow),
  learning less often from fewer, surprising sightings seems to help. The
  next card in Nano's design, sleep with a replay budget chosen by
  "surprise" and stakes, is where to test this.

The gate stays off by default (`Recall::gate: None`), so no earlier result
changes.
