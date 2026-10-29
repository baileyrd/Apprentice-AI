# Evidence: patterns that slow down as they learn

The counting phase ([docs/counting](../counting/README.md)) found that a
pattern says mostly what the last world taught: it learns by a delta rule at a
fixed rate of 0.05, so everything older than a few dozen samples is
forgotten. A mentor's random words therefore read as whatever the last world
did, not as noise. Nano's pick: let learning slow as evidence builds up.

## The change

Each pattern learns at rate max(floor, 1/n), where n is how many samples it
has had. Early on it averages everything it has seen; once 1/n falls to the
floor it keeps learning at the floor, so it can still change its mind. The
floor (`Recall::pattern_rate`, 0.05 today) is the one setting.

This is the "stubbornness" from the unlearning test: a pattern held longer
needs more evidence to move. The cost is adaptation, which is the success
measure, so rule 3 guards it.

## Rules fixed before any run

The floor may be tuned on seeds 101–110; the final check is one run on seeds
1–10, against main on the same seeds.

1. **Noise reads as noise.** In the language test, with the random mentor,
   both words give a square eaten 32–63 steps ago between 0.25 and 0.75
   chance of food, in at least 8 of 10 seeds. On main: 0 of 10.
2. **Noise does no harm.** With the random mentor, median sleeps in the
   language test's new bush world are no more than 10% above no mentor.
   On main: 11.5 vs 8.5.
3. **Adaptation is not lost.** In the unlearning test, median steps to
   unlearn are no more than twice main's (108, 135, 185 after 1,000, 3,000
   and 9,000 steps raised), and every counted seed still unlearns.
4. **No regressions, in rules or in the numbers under them.** Home test rules
   1–3 hold. No median sleep count rises by more than 10% or 2 sleeps,
   whichever is larger, in: the transfer test (fresh world and home, with
   patterns), the language test (barren and bush, all three minds), and the
   mentor test (steps 1–4,000, all five minds).

Also reported, not judged: the four language rules, transfer anticipation,
and A/B/A savings and mix-ups. Prediction: unlearning gets slower, and more so
the longer it was raised, which is the human-like stubbornness Nano asked
about. That is the trade.

## Tuning, disclosed

On seeds 101–110 I tried floors of 0.05 (today's rate), 0.01, 0.002 and none
(pure 1/n). With no floor, unlearning after 9,000 steps took 655 steps
against 198, failing rule 3; 0.002 took 588. I picked 0.01, the lowest floor
that kept unlearning within twice main's.

## Results (seeds 1–10, one run, floor 0.01): not shipped

| Rule | Result | Met? |
|---|---|---|
| 1. Random words read 0.25–0.75 in ≥ 8/10 | 0/10 (0.01–0.14) | **no** |
| 2. Random mentor bush sleeps ≤ +10% | 13 vs 7 | **no** |
| 3. Unlearning ≤ 2× main, all unlearn | 204, 208, 182 vs 108, 135, 185 | yes |
| 4. No regressions | home strangers 26/30 (rule needs 27); every sleep count within bounds | **no** |

Per-seed numbers: [foretell.csv](foretell.csv), [unlearn.csv](unlearn.csv).

## What it shows

- **Random words now match the general pattern, and that pattern is low.**
  With pure 1/n on the tuning seeds, each random word read within a few
  hundredths of the general pattern in most seeds: a word with no
  information adds none, which is right. But the general pattern itself
  reads 0.07–0.28, not 0.5, because every glance at a square that stays
  empty counts. So rule 1 asked for the wrong number: noise should read as
  the base rate, and the base rate is low.
- **A floor of 0.01 is still mostly recency.** It forgets over about 100
  samples, so the words still track the last world, and the bush-world
  harm from noise stays.
- **Stubbornness only appears without a floor.** With pure 1/n, unlearning
  took 197, 262 and 655 steps after 1,000, 3,000 and 9,000 steps raised:
  the longer it held a belief, the longer it took to drop, like people. With
  a floor, every upbringing reaches the floor and unlearns at the same pace.
- **The trade is real.** Averaging over a lifetime makes noise harmless
  but makes adaptation slow, and adaptation is the success measure. A
  single learning rate can't get both.

No code changes are kept; patterns still learn at a fixed 0.05.
