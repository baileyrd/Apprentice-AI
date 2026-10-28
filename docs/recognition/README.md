# Recognising a world: stronger evidence before going back

## Rules fixed before any run

These were written before any run, tuning seeds included. Settings may be
tuned on seeds 101–110 against these rules; the final check is one run on
seeds 1–10. The apprentice is never told which world it is in, and gets no
cue for it; it can only compare what it senses with its own maps.

A **mix-up** is a seed where, in the A/B/A test, the apprentice spends more
than 100 of its 3,000 steps in world B using the map it made in world A.

1. **Coming back to A.** Median relearn savings (second A time / first A
   time) are at most 0.30, from 0.46.
2. **Fewer mix-ups.** At most 2 of 10 seeds, from 4.
3. **No regression.** Every earlier rule still holds:
   - B still gets learned in 10 of 10 seeds;
   - the patterns rules (anticipation ≥ 0.5 and ≥ 0.2 above without;
     bush-world sleeps with patterns ≤ 10% above without);
   - re-mapping moved walls when fed in the bush world, ≥ 9 of 10;
   - phase 2 wall test, median ≤ 798 steps.

## The change

Before, when the map in use stopped fitting, it went back to any old map
whose running error was under the misfit line. That running error rests on
about 50 sightings (5 or 6 steps), and in a stretch of open ground almost any
map fits. So a few steps in world B that happened to look like A were enough
to go back to A's map, and B's sightings were then written into A's map.

Now an old map is recalled only if it fits **and** its running error is at
least 0.15 lower than the active map's (`Recall::margin`). A near tie is not
evidence. Nothing else changed. It still gets no label or cue for the world;
it only compares its own maps with what it senses.

**Tuning, disclosed.** On seeds 101–110 I tried two ideas: waiting for the
active map to misfit for 100 or 300 sightings before any recall, and the
margin (0.05 to 0.25). Waiting slowed the return to A (savings 0.50 at 300).
The margin alone took spare-seed mix-ups from 3 to 0 and savings from 0.25
to 0.14, for any margin from 0.1 to 0.25. I picked 0.15, the middle of that
range, and dropped the wait.

Reproduce:

```sh
cargo run --release --bin relearn -- --out docs/recognition/relearn
cargo run --release --bin transfer -- --out docs/recognition
cargo run --release --bin adapt -- --change walls --bushes --out docs/recognition/walls-bushes
cargo run --release --bin adapt -- --change walls --out docs/recognition/walls
```

## Results (seeds 1–10, one run)

| Rule | Before | After | Met? |
|---|--:|--:|---|
| 1. Median relearn savings ≤ 0.30 | 0.46 | 0.37 | **no** |
| 2. Mix-ups ≤ 2 of 10 | 4 | 3 | **no** |
| 3. B learned in 10 of 10 | 10 | 10 | yes |
| 3. Anticipation ≥ 0.5 and ≥ 0.2 above without | 0.93 vs 0.12 | 0.77 vs 0.13 | yes |
| 3. Bush-world sleeps ≤ 10% above without | 10.5 vs 38.5 | 10.5 vs 38.5 | yes |
| 3. Walls re-mapped when fed, bush world, ≥ 9 of 10 | 10 (222 steps) | 10 (333 steps) | yes |
| 3. Phase 2 wall test, median ≤ 798 steps | 427 | 427 | yes |

Steps spent in world B on A's map, per seed (more than 100 is a mix-up):

| Seed | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
|---|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| Before | | | | 234 | 3000 | 707 | | 2998 | | |
| After | 71 | 36 | 34 | **186** | **3000** | **142** | 36 | 35 | 35 | 35 |

Per-seed numbers are in [relearn/relearn.csv](relearn/relearn.csv) (column
`mixed_up`), [transfer.csv](transfer.csv), and the `walls` folders.

## What it means

- **Going back on weak evidence was a real cause, and the margin helps.**
  Seed 8 no longer goes back to A's map in B (2,998 steps before, 35 now).
  Seeds 4 and 6 still go back, but briefly: 186 and 142 steps, against 234
  and 707. Median savings fell from 0.46 to 0.37, though the median time
  back to A barely moved (48 steps against 42): the gain is in the per-seed
  ratios. Both headline rules still missed.
- **Seed 5 is a different failure.** It spends all 3,000 B steps on A's map:
  it never decides B is a new world at all, because A's map keeps fitting
  well enough. Stronger evidence for going back cannot fix that; it needs
  better evidence for "this is somewhere new".
- **The spare seeds flattered the fix.** On 101–110 it removed every
  mix-up. On 1–10 it removed one of four.
- **One cost.** Re-mapping moved walls in the bush world now takes 333 steps
  against 222. Still 10 of 10, well within the rule.
