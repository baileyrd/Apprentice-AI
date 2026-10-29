# Judging: making test results trustworthy

Until now each phase was judged on one run of 10 seeds (seeds 1–10), by
comparing medians. A *seed* is the number that decides everything random in
one life: the worlds' layouts, where food grows, and the apprentice's own
coin flips. Different seeds are different lives.

In the peers phase (PR #17) sleeps in the bush world ranged from 2 to 133
between seeds, and a stranger whose trust never moved still shifted the
median from 8.5 to 13. With spread like that, a 10-seed median can pass or
fail a rule by luck. This phase changes how results are judged, then
re-judges the last three phases to see which verdicts were luck.

## The method

1. **Paired, seed by seed.** Every condition runs on the same seeds, so on
   each seed the change and its baseline face the same worlds. The question
   becomes "on the same life, did the change help?", which removes the
   differences between worlds from the comparison.
2. **Typical change.** For each seed, the ratio `(change + 1) / (baseline + 1)`
   of the measure (sleeps, false hopes). The *typical change* is the
   geometric mean of those ratios, minus 1: "−30%" means a typical seed has
   30% fewer. Ratios, not differences, so a seed with 133 sleeps weighs no
   more than one with 5; the `+ 1` keeps a seed with 0 from dividing by 0.
3. **A 90% interval.** From the spread of the per-seed ratios, the range the
   typical change would most likely land in if the test were rerun on
   fresh seeds (mean of the log ratios ± 1.645 standard errors). Wide when
   seeds disagree or are few; narrow when they agree or are many.
4. **Three verdicts, not two.** A rule's mark is **met** when the whole
   interval is on the right side of it, **missed** when the whole interval
   is on the wrong side, and **unclear** when the interval straddles it.
   A change that does nothing is wrongly called "met" at most 5% of the
   time.
5. **More seeds.** Judged on many seeds (how many is fixed below, from
   calibration), not 10.

Rules that are not comparisons are judged the same way with their own
interval: a share of seeds ("words learned in at least 9 of 10") uses a
Wilson interval on that share, and a level ("false hopes at most 2") uses
the 90% interval of the median from the sorted per-seed values.

"Within bounds" changes from "no more than 10% or 2 above" to "typical
change no more than +10%": the `+ 1` already gives small counts room.

## Rules fixed before any run

**Calibration seeds 2001–2200** are used to choose the number of judging
seeds and to test the method itself. No PR's verdict is read from them.
**Judging seeds 1–N** then re-judge PRs #14, #16 and #17, each on its own
code (commits b43f789, 38dc9e9, 986ee8b), under the rules written in its
README, read as above.

**Number of judging seeds N:** the smallest of 50, 100 and 200 at which, on
the calibration seeds, the interval for bush sleeps is no wider than 30
points (high end minus low end) for both "true mentor vs no mentor" and
"elder ahead vs alone". If none, 200, and the widths are reported.

The method itself passes if:

1. **Few false alarms.** Comparing the no-mentor apprentice with itself on
   other seeds (two random halves of the 200 calibration seeds, 200
   times), the method calls "at least 15% fewer bush sleeps" met in at most
   5% of the splits. Reported alongside: how often the old method (median
   of 10 seeds against median of 10 others) passes the same rule.
2. **Reproducible.** For every rule of PRs #14, #16 and #17 that can be read
   on today's code, the verdicts on seeds 2001–2100 and 2101–2200 never
   disagree as met against missed. Reported alongside: how often the old
   method's verdicts flip between disjoint groups of 10 seeds.

My guess before running: the old method will pass a do-nothing change on
bush sleeps about a third of the time, and the rules that hinge on bush
sleeps (#14 rule 3, #16 rule 2, all four of #17) will mostly become
unclear rather than flip outright.

## Calibration (seeds 2001–2200)

`cargo run --release --bin judging` reads the CSVs in this folder and
prints everything below. Calibration CSVs: [calibration/](calibration/),
written by `foretell` and `peers` on today's code.

**Interval width for bush sleeps**, high end minus low end:

| Seeds | True mentor vs none | Elder ahead vs alone |
|--:|--:|--:|
| 50 | 0.22 | 0.42 |
| 100 | 0.15 | 0.303 |
| 200 | 0.10 | 0.21 |

At 100 seeds the elder comparison is just over the 0.30 limit, so by the
rule **N = 200**.

One change, made here before any judging run: the judging seeds are **1–100
and 111–210**, not 1–200, so that seeds 101–110, which past phases tuned
on, play no part in judging them.

| Check | Old method | New method | Met? |
|---|--:|--:|---|
| 1. A do-nothing change called "at least 15% fewer bush sleeps" | 44% of splits | 0% of splits | yes |
| 2. Met/missed flips between two seed groups, over 11 rules | 2.9 on average (groups of 10) | 0 (groups of 100) | yes |

So under the old method, a change that does nothing passed a "15% fewer
bush sleeps" rule almost half the time, and on average about 3 of 11
verdicts reversed between two sets of 10 seeds. As a further check, the new
interval left out "no change" in 5% of the do-nothing splits; a 90%
interval should do so about 10% of the time or less, so it errs on the
careful side.
