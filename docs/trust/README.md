# Trust: who said it matters

Nano's point after the language phase: the apprentice doesn't know anyone is
talking to it, so it can't learn that one speaker is worth less than another.
A child learns which grown-ups to believe. Nano's pick: learned trust per
speaker.

## The change

- **Words come with a voice.** Each mentor has its own voice, so the
  apprentice can tell who spoke. It knows nothing else about them.
- **A word means what it meant from that speaker.** Word patterns are keyed
  by voice and word, so the same word from two speakers is learned
  separately.
- **Trust is learned.** In sleep, each time a told square is replayed, the
  apprentice checks whether that speaker's word predicted what the square
  became better than its general pattern did. Trust in that voice moves
  toward 1 when it did and toward 0 when it didn't, by a delta rule. A
  stranger starts at 0.5.
- **Trust weighs words.** A told square's belief is the general pattern,
  moved toward the word's pattern by the speaker's trust. A speaker who adds
  nothing ends up near no weight; one who is right ends up near full weight.

Nothing about any speaker is built in: honesty is learned only from what
their words turned out to say.

## Test

The language test (bush and barren worlds in turn, then a new one of each),
with two new minds that hear **two mentors at once** after every meal: the
honest one and the random one, using the same two words.

- **One voice**: both mentors sound the same, so the apprentice can't tell
  them apart. This is what the apprentice could do before this change.
- **Two voices**: it can.

## Rules fixed before any run

The trust rate may be tuned on seeds 101–110; the final check is one run on
seeds 1–10. Medians.

1. **Trust is learned.** With two voices, after the upbringing, trust in the
   honest voice is at least 0.3 above trust in the random one, in at least 9
   of 10 seeds.
2. **Honest words still pay through the noise.** With two voices, barren
   false hopes are at most 2, and bush sleeps no more than 10% or 2 sleeps
   (whichever is larger) above the honest mentor alone.
3. **Knowing who spoke helps.** With two voices, barren false hopes are at
   least 50% below one voice.
4. **No regressions.** No median sleep count in the language test (all
   minds that exist on main) or the mentor test rises by more than 10% or 2
   sleeps, whichever is larger, against main. Home test rules 1–3 hold.

## Tuning, disclosed

On seeds 101–110, before the final run:

- **Mentors take turns.** At first both mentors spoke after every meal, and
  the second word overwrote the first, so the honest one was never heard.
  Now one speaks per meal, in turn. Each voice is heard about half the
  squares.
- **Trust is judged only on disagreements.** At first every replayed glance
  compared the word's prediction with the general pattern's. Nearly all of
  those are ties in substance (both say "still empty"), and the general
  pattern, with more samples, won them by hair's breadths, so even the
  honest voice ended near 0. Now a sighting only counts when the word and
  the general pattern disagree about what is most likely.
- **Trust rate 0.2**, faster than patterns learn, since disagreements are
  rare. At 0.05 trust barely moved from 0.5 (0.55 vs 0.49).

## Results (seeds 1–10, one run)

| Mind | Trust: honest / other voice | Barren false hopes | Bush sleeps |
|---|--:|--:|--:|
| No mentor | – | 27.5 | 8.5 |
| Honest mentor alone | 0.99 | 0 | 4.5 |
| Random mentor alone | 0.27 | 25 | 12.5 |
| Both, one voice | 0.14 | 16.5 | 11.5 |
| Both, two voices | 0.51 / 0.49 | 6.5 | 9.5 |

| Rule | Result | Met? |
|---|---|---|
| 1. Honest trust ≥ 0.3 above random, ≥ 9/10 | 4/10; in 4 seeds neither voice was ever judged (both 0.50) | **no** |
| 2. Two voices: false hopes ≤ 2; bush sleeps within 10% or 2 of honest alone | 6.5; 9.5 vs 4.5 | **no** |
| 3. Two voices: false hopes ≥ 50% below one voice | 6.5 vs 16.5, −61% | yes |
| 4. No regressions | language and mentor sleeps within bounds; home 10/10, 10/10, 28/30 | yes |

## A longer upbringing (a check, not judged)

The same seeds with each upbringing world lasting 3,000 steps instead of
1,000:

| Mind | Trust: honest / other | Words learned | Barren false hopes | Bush sleeps |
|---|--:|--:|--:|--:|
| Honest alone | 1.00 | 7/10 | 0 | 3 |
| Random alone | 0.01 | – | 25.5 | 13.5 |
| Both, one voice | 0.02 | 0/10 | 43.5 | 18.5 |
| Both, two voices | **0.99 / 0.03** | **9/10** | 38.5 | **5.5** |

Honest trust is at least 0.3 above random in 10 of 10 seeds.

## What it shows

- **Trust is learned, given enough to judge.** With 1,000 steps per world
  and the honest mentor speaking about every other meal, its words often
  never got enough samples to disagree with anything, so trust stayed at the
  stranger's 0.5. Given 3,000 steps per world, every seed learned to trust
  the honest voice (0.99) and not the random one (0.03).
- **Being able to tell them apart pays.** In a bush world, two voices cost
  5.5 sleeps against 18.5 with one voice, close to the honest mentor alone
  (3). With one voice, the random half poisons the honest half and the
  words lose all trust.
- **Distrust means falling back, not knowing better.** In the barren world,
  squares the random mentor spoke about are believed as if nobody had said
  anything, so false hopes there are what no mentor gets. Rule 2 asked for
  the honest mentor's result while it speaks about only half the squares;
  that was the wrong bar.
- **A random speaker alone is now mostly ignored** (trust 0.27, or 0.01 with
  the longer upbringing), so its words no longer pass as facts.
