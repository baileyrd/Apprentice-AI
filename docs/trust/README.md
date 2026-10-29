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
