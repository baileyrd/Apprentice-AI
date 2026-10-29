# Patterns network: long-term patterns held in a network, not a table

The network phase (PR #20) showed that a network cannot beat the map at
predicting the next view: the map remembers places, and in these small
worlds it has seen nearly every square. What the map cannot do is guess a
square it has no reason to know about, for example a bush it ate from 40
steps ago. That guess comes from the **patterns**: the slowest memory,
learned in sleep from every world, keyed on a square's last change and how
long ago it was ("eaten, 32–63 steps ago → food again with chance 0.8").

Today the patterns are a table: one belief per (last change × age group),
8 age groups of doubling length, each entry learned on its own by a delta
rule. Nothing learned about "eaten 16–31 steps ago" carries to "eaten
32–63 steps ago", and the table has no idea what happens between groups.

This slice swaps the table for the PR #20 network, with its hidden layer
and its local, predictive-coding learning (no backprop). It sees the last
change (one-hot), the age group (one-hot) and the age itself (log2(age)/8),
and predicts what the square holds now. Everything else stays: the same
samples from sleep, the same evidence gate (a pattern speaks only after
`pattern_min` samples), and word patterns and trust stay tables.

This is the network as the map's prior where it can matter: the map asks
the patterns what an unseen-lately square holds, and the patterns are now
the network.

## Test

The gate phase's life (PR #19), now shared in `apprentice::life`: four
upbringing worlds of 1,000 steps (bush and barren in turn), then a new
bush world for 4,000 steps; from a second upbringing, a new barren world
for 2,000 steps; and the home test (home, home as left, home with a wall
moved, three strangers). Each seed is lived twice: patterns in a table
(today's apprentice, `Recall::default()`) and patterns in a network. The
active-memory gate is off in both.

## Rules fixed before any run

Tuning on seeds 101–110 only. Judged once on seeds 1–100 and 111–210, each
comparison paired seed by seed, read from the 90% interval as met, missed
or unclear with `apprentice::judge` (the method of PR #18).

1. **It learns sooner.** A network shares what it learns between nearby
   ages and kinds of square, so it should need fewer samples. Sleeps in
   the four upbringing worlds: typical change at most −10%.
2. **It anticipates regrowth at least as well.** Bush-world sleeps: at
   most +10%.
3. **No regressions**, each at most +10%: barren-world sleeps, barren
   false hopes, home-test wrong answers after 100 steps.

Reported but not judged: meals in both worlds, futile returns in the
barren world, home-test wrong answers after 300 steps.

My guess before running: rule 1 unclear (four 1,000-step worlds give the
table plenty of samples already), rule 2 met, rule 3 met for sleeps and the
home test and at risk for false hopes, since a network smooths between age
groups and may blur "stays gone" into "grows back".

## Tuning (seeds 101–110)

Hidden units 8, 16, 32, 64 and 128 at rates 0.02 to 0.3. Small or slow
networks were worse than the table everywhere except false hopes: at 16
units and rate 0.05, upbringing sleeps rose 18% and barren sleeps 12%. They
learn too slowly to keep up with the table's per-entry delta rule. Bigger
and faster closed the gap: upbringing sleeps went from +24% (8 units, 0.02)
to +0% (64 units, 0.2). Past that it stopped helping: 128 units gave the
same, and rate 0.3 became unstable (false hopes up, bush sleeps 36).
**Chosen: 64 hidden units, rate 0.2**, the best bush sleeps (−35%) with
upbringing and barren sleeps level. No setting cut upbringing sleeps, so
rule 1 looked likely to miss before judging.

## Results (seeds 1–100 and 111–210, one run)

`cargo run --release --bin patterns_net -- --judge` prints the verdicts
([judging.txt](judging.txt)) and writes
[patterns_net.csv](patterns_net.csv). About 30 minutes on 4 cores.

| Rule | Result | Verdict |
|---|---|---|
| 1. Upbringing sleeps ≤ −10% | −3% [−5%, −2%] | **missed** |
| 2. Bush sleeps ≤ +10% | −7% [−17%, +6%] | **met** |
| 3a. Barren sleeps ≤ +10% | +3% [+1%, +4%] | **met** |
| 3b. Barren false hopes ≤ +10% | −85% [−88%, −82%] | **met** |
| 3c. Home-test wrong answers (100 steps) ≤ +10% | +13% [+7%, +19%] | **unclear** |

Means, table → network: bush sleeps 22.7 → 14.2, bush meals 246 → 279
(+24%), barren false hopes 21.9 → 1.7, barren futile returns 24.2 → 13.1
(−45%), home-test wrong answers per seed (of 5 trials) 0.3 → 0.6.

## What it shows

- **The network is much better at "stays gone".** False hopes in the
  barren world fall by 85% and futile returns by 45%, and it eats 24% more
  in the bush world. This is the opposite of my guess: I expected
  smoothing between age groups to blur "stays gone" into "grows back". The
  table's weakness here was already known (PR #14/#15: fixed-rate delta
  rule, last world wins). The network shares one set of weights across
  every age and state, so one barren world does not overwrite what bush
  worlds taught; the kind of square and the age both shape its answer.
- **It does not learn sooner.** Upbringing sleeps fall 3%, not 10%. Four
  1,000-step worlds already give the table enough samples, as guessed.
- **The home test got a little worse, cause unchecked.** About one more
  wrong answer per three seeds. The home test judges eaten bushes by
  pattern regrowth (PR #11), so a different regrowth belief changes which
  map fits; which trials flipped is not yet looked at.
