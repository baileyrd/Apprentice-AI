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
